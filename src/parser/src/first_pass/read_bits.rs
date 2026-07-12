use bitter::BitReader;
use bitter::LittleEndianReader;
use std::fmt;

/// Hard ceiling for any single length-prefixed object copied out of a demo.
/// Normal Source 2 net messages are far smaller; this prevents malformed
/// varints from turning a tiny input into a multi-gigabyte allocation.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

/// Hard ceiling for a single Snappy output buffer. The parser reuses this
/// buffer between frames, so bounding it also bounds peak frame memory.
pub const MAX_DECOMPRESSED_BYTES: usize = 256 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: usize = 256;
const DECOMPRESSION_RATIO_SLACK: usize = 1024 * 1024;

pub fn validate_decompressed_size(
    compressed_len: usize,
    decompressed_len: usize,
) -> Result<(), DemoParserError> {
    if decompressed_len > MAX_DECOMPRESSED_BYTES {
        return Err(DemoParserError::ResourceLimitExceeded(
            "decompressed frame exceeds limit",
        ));
    }
    let ratio_limit = compressed_len
        .saturating_mul(MAX_COMPRESSION_RATIO)
        .max(DECOMPRESSION_RATIO_SLACK);
    if decompressed_len > ratio_limit {
        return Err(DemoParserError::ResourceLimitExceeded(
            "decompression ratio exceeds limit",
        ));
    }
    Ok(())
}

pub struct Bitreader<'a> {
    pub reader: LittleEndianReader<'a>,
    pub bits_left: u32,
    pub bits: u64,
    pub total_bits_left: u32,
}
pub fn read_varint(bytes: &[u8], ptr: &mut usize) -> Result<u32, DemoParserError> {
    let mut result: u32 = 0;
    let mut count: u8 = 0;
    loop {
        if count >= 5 {
            return Ok(result as u32);
        }
        let b = match bytes.get(*ptr) {
            Some(b) => *b as u32,
            None => return Err(DemoParserError::OutOfBytesError),
        };
        *ptr += 1;
        result |= (b & 127) << (7 * count);
        count += 1;
        if b & 0x80 == 0 {
            break;
        }
    }
    Ok(result as u32)
}

impl<'a> Bitreader<'a> {
    pub fn new(bytes: &'a [u8]) -> Bitreader<'a> {
        let b = Bitreader {
            reader: LittleEndianReader::new(bytes),
            bits: 0,
            bits_left: 0,
            total_bits_left: 0,
        };
        b
    }
    #[inline(always)]
    pub fn consume(&mut self, n: u32) {
        self.bits_left -= n;
        self.bits >>= n;
        self.reader.consume(n);
    }
    #[inline(always)]
    pub fn peek(&mut self, n: u32) -> u64 {
        self.bits & ((1 << n) - 1)
    }
    #[inline(always)]
    pub fn refill(&mut self) {
        self.reader.refill_lookahead();
        let refilled = self.reader.lookahead_bits();
        if refilled > 0 {
            self.bits = self.reader.peek(refilled);
        }
        self.bits_left = refilled;
    }
    #[inline(always)]
    pub fn bits_remaining(&mut self) -> Option<usize> {
        Some(self.reader.bits_remaining()?)
    }
    pub fn ensure_bytes_remaining(&mut self, n: usize) -> Result<(), DemoParserError> {
        if n > MAX_MESSAGE_BYTES {
            return Err(DemoParserError::ResourceLimitExceeded(
                "length-prefixed message exceeds limit",
            ));
        }
        let remaining = self
            .reader
            .bits_remaining()
            .ok_or(DemoParserError::OutOfBitsError)?
            .checked_div(8)
            .unwrap_or(0);
        if n > remaining {
            return Err(DemoParserError::FailedByteRead(format!(
                "Failed to read message/command. bytes left in stream: {}, requested bytes: {}",
                remaining, n,
            )));
        }
        Ok(())
    }
    #[inline(always)]
    pub fn read_nbits(&mut self, n: u32) -> Result<u32, DemoParserError> {
        if self.bits_left < n {
            self.refill();
        }
        let b = self.peek(n);
        self.consume(n);
        return Ok(b as u32);
    }
    #[inline(always)]
    pub fn read_u_bit_var(&mut self) -> Result<u32, DemoParserError> {
        let bits = self.read_nbits(6)?;
        match bits & 0b110000 {
            0b10000 => return Ok((bits & 0b1111) | (self.read_nbits(4)? << 4)),
            0b100000 => return Ok((bits & 0b1111) | (self.read_nbits(8)? << 4)),
            0b110000 => return Ok((bits & 0b1111) | (self.read_nbits(28)? << 4)),
            _ => return Ok(bits),
        }
    }
    #[inline(always)]
    pub fn read_varint32(&mut self) -> Result<i32, DemoParserError> {
        let x = self.read_varint()? as i32;
        let mut y = x >> 1;
        if x & 1 != 0 {
            y = !y;
        }
        Ok(y as i32)
    }
    #[inline(always)]
    pub fn read_varint(&mut self) -> Result<u32, DemoParserError> {
        let mut result: u32 = 0;
        let mut count: i32 = 0;
        let mut b: u32;
        loop {
            if count >= 5 {
                return Ok(result);
            }
            b = self.read_nbits(8)?;
            result |= (b & 127) << (7 * count);
            count += 1;
            if b & 0x80 == 0 {
                break;
            }
        }
        Ok(result)
    }
    #[inline(always)]
    pub fn read_varint_u_64(&mut self) -> Result<u64, DemoParserError> {
        let mut result: u64 = 0;
        let mut count: i32 = 0;
        let mut b: u32;
        let mut s = 0;
        loop {
            b = self.read_nbits(8)?;
            if b < 0x80 {
                if count > 9 || count == 9 && b > 1 {
                    return Err(DemoParserError::MalformedMessage);
                }
                return Ok(result | (b as u64) << s);
            }
            result |= ((b as u64) & 127) << s;
            count += 1;
            if b & 0x80 == 0 {
                break;
            }
            s += 7;
        }
        Ok(result)
    }
    #[inline(always)]
    pub fn read_boolean(&mut self) -> Result<bool, DemoParserError> {
        Ok(self.read_nbits(1)? != 0)
    }
    pub fn read_n_bytes(&mut self, n: usize) -> Result<Vec<u8>, DemoParserError> {
        self.ensure_bytes_remaining(n)?;
        let mut bytes = vec![0_u8; n];
        match self.reader.read_bytes(&mut bytes) {
            true => {
                self.refill();
                Ok(bytes)
            }
            false => Err(DemoParserError::FailedByteRead(
                format!(
                    "Failed to read message/command. bytes left in stream: {}, requested bytes: {}",
                    self.reader.bits_remaining().unwrap_or(0).checked_div(8).unwrap_or(0),
                    n,
                )
                .to_string(),
            )),
        }
    }
    pub fn read_n_bytes_mut(&mut self, n: usize, buf: &mut [u8]) -> Result<(), DemoParserError> {
        self.ensure_bytes_remaining(n)?;
        if buf.len() < n {
            return Err(DemoParserError::MalformedMessage);
        }
        match self.reader.read_bytes(&mut buf[..n]) {
            true => {
                self.refill();
                Ok(())
            }
            false => Err(DemoParserError::FailedByteRead(
                format!(
                    "Failed to read message/command. bytes left in stream: {}, requested bytes: {}",
                    self.reader.bits_remaining().unwrap_or(0).checked_div(8).unwrap_or(0),
                    n,
                )
                .to_string(),
            )),
        }
    }
    pub fn read_ubit_var_fp(&mut self) -> Result<u32, DemoParserError> {
        if self.read_boolean()? {
            return Ok(self.read_nbits(2)?);
        }
        if self.read_boolean()? {
            return Ok(self.read_nbits(4)?);
        }
        if self.read_boolean()? {
            return Ok(self.read_nbits(10)?);
        }
        if self.read_boolean()? {
            return Ok(self.read_nbits(17)?);
        }
        return Ok(self.read_nbits(31)?);
    }
    #[inline(always)]
    pub fn read_bit_coord(&mut self) -> Result<f32, DemoParserError> {
        let mut int_val = 0;
        let mut frac_val = 0;
        let i2 = self.read_boolean()?;
        let f2 = self.read_boolean()?;
        if !i2 && !f2 {
            return Ok(0.0);
        }
        let sign = self.read_boolean()?;
        if i2 {
            int_val = self.read_nbits(14)? + 1;
        }
        if f2 {
            frac_val = self.read_nbits(5)?;
        }
        let resol: f64 = 1.0 / (1 << 5) as f64;
        let result: f32 = (int_val as f64 + (frac_val as f64 * resol) as f64) as f32;
        if sign {
            Ok(-result)
        } else {
            Ok(result)
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum DemoParserError {
    ClassMapperNotFoundFirstPass,
    FieldNoDecoder,
    OutOfBitsError,
    OutOfBytesError,
    FailedByteRead(String),
    UnknownPathOP,
    EntityNotFound,
    ClassNotFound,
    MalformedMessage,
    StringTableNotFound,
    Source1DemoError,
    DemoEndsEarly(String),
    UnknownFile,
    IncorrectMetaDataProp,
    UnknownPropName(String),
    GameEventListNotSet,
    PropTypeNotFound(String),
    GameEventUnknownId(String),
    UnknownPawnPrefix(String),
    UnknownEntityHandle(String),
    ClsIdOutOfBounds,
    UnknownGameEventVariant(String),
    FileNotFound(String),
    NoEvents,
    DecompressionFailure(String),
    NoSendTableMessage,
    UserIdNotFound,
    EventListFallbackNotFound(String),
    VoiceDataWriteError(String),
    UnknownDemoCmd(i32),
    IllegalPathOp,
    VectorResizeFailure,
    ImpossibleCmd,
    UnkVoiceFormat,
    MalformedVoicePacket,
    ResourceLimitExceeded(&'static str),
}

impl std::error::Error for DemoParserError {}

impl fmt::Display for DemoParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn length_prefixed_reads_validate_before_allocating() {
        let mut reader = Bitreader::new(&[0_u8; 4]);
        assert!(matches!(
            reader.read_n_bytes(MAX_MESSAGE_BYTES + 1),
            Err(DemoParserError::ResourceLimitExceeded(_))
        ));

        let mut reader = Bitreader::new(&[0_u8; 4]);
        assert!(matches!(
            reader.read_n_bytes(5),
            Err(DemoParserError::FailedByteRead(_))
        ));
    }

    #[test]
    fn decompression_limits_reject_bombs() {
        assert!(validate_decompressed_size(1024, 32 * 1024).is_ok());
        assert!(matches!(
            validate_decompressed_size(8, MAX_DECOMPRESSED_BYTES),
            Err(DemoParserError::ResourceLimitExceeded(_))
        ));
        assert!(matches!(
            validate_decompressed_size(MAX_DECOMPRESSED_BYTES, MAX_DECOMPRESSED_BYTES + 1),
            Err(DemoParserError::ResourceLimitExceeded(_))
        ));
    }
}
