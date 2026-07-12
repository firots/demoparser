use crate::first_pass::read_bits::DemoParserError;
use crate::first_pass::sendtables::Serializer;
use crate::second_pass::parser_settings::EconItem;
use crate::second_pass::parser_settings::PlayerEndMetaData;
use crate::second_pass::parser_settings::SecondPassParser;
use csgoproto::maps::PAINTKITS;
use csgoproto::maps::WEAPINDICIES;
use csgoproto::CcsUsrMsgEndOfMatchAllPlayersData;
use csgoproto::CcsUsrMsgSendPlayerItemDrops;
use prost::Message;

#[derive(Debug, Clone)]
pub struct Class {
    pub class_id: i32,
    pub name: String,
    pub serializer: Serializer,
}

impl<'a> SecondPassParser<'a> {
    pub fn parse_item_drops(&mut self, bytes: &[u8]) -> Result<(), DemoParserError> {
        let drops = match CcsUsrMsgSendPlayerItemDrops::decode(bytes) {
            Ok(msg) => msg,
            Err(_) => return Err(DemoParserError::MalformedMessage),
        };
        let retained_bytes = drops
            .entity_updates
            .len()
            .checked_mul(std::mem::size_of::<EconItem>())
            .and_then(|overhead| overhead.checked_add(bytes.len()))
            .ok_or(DemoParserError::ResourceLimitExceeded(
                "retained item data size overflow",
            ))?;
        self.resource_budget
            .reserve_retained_message_bytes(retained_bytes)?;
        self.item_drops
            .try_reserve(drops.entity_updates.len())
            .map_err(|_| DemoParserError::VectorResizeFailure)?;
        for item in &drops.entity_updates {
            let item_name = match WEAPINDICIES.get(&item.defindex.unwrap_or(u32::MAX)) {
                Some(name) => Some(name.to_string()),
                None => None,
            };
            let skin_name = match PAINTKITS.get(&item.paintindex.unwrap_or(u32::MAX)) {
                Some(name) => Some(name.to_string()),
                None => None,
            };
            self.item_drops.push(EconItem {
                account_id: item.accountid,
                item_id: item.itemid,
                def_index: item.defindex,
                paint_index: item.paintindex,
                rarity: item.rarity,
                quality: item.quality,
                paint_seed: item.paintseed,
                paint_wear: item.paintwear,
                quest_id: item.questid,
                dropreason: item.dropreason,
                custom_name: item.customname.clone(),
                inventory: item.inventory,
                ent_idx: item.entindex,
                steamid: None,
                item_name,
                skin_name,
            });
        }
        Ok(())
    }

    pub fn parse_player_end_msg(&mut self, bytes: &[u8]) -> Result<(), DemoParserError> {
        let end_data = match CcsUsrMsgEndOfMatchAllPlayersData::decode(bytes) {
            Ok(msg) => msg,
            Err(_) => return Err(DemoParserError::MalformedMessage),
        };
        let item_count = end_data
            .allplayerdata
            .iter()
            .try_fold(0usize, |total, player| total.checked_add(player.items.len()))
            .ok_or(DemoParserError::ResourceLimitExceeded(
                "retained player data size overflow",
            ))?;
        let retained_bytes = end_data
            .allplayerdata
            .len()
            .checked_mul(std::mem::size_of::<PlayerEndMetaData>())
            .and_then(|overhead| {
                item_count
                    .checked_mul(std::mem::size_of::<EconItem>())
                    .and_then(|items| overhead.checked_add(items))
            })
            .and_then(|overhead| overhead.checked_add(bytes.len()))
            .ok_or(DemoParserError::ResourceLimitExceeded(
                "retained player data size overflow",
            ))?;
        self.resource_budget
            .reserve_retained_message_bytes(retained_bytes)?;
        self.player_end_data
            .try_reserve(end_data.allplayerdata.len())
            .map_err(|_| DemoParserError::VectorResizeFailure)?;
        self.skins
            .try_reserve(item_count)
            .map_err(|_| DemoParserError::VectorResizeFailure)?;
        /*
        Todo parse "accolade", seems to be the awards at the end like "most mvps in game"
        But seems to only have integers so need to figure out what they mean
        example:

        Accolade {
            eaccolade: Some(
                21,
            ),
            value: Some(
                5100.0,
            ),
            position: Some(
                1,
            ),
        }
        */
        for player in &end_data.allplayerdata {
            self.player_end_data.push(PlayerEndMetaData {
                name: player.name.clone(),
                steamid: player.xuid,
                team_number: player.teamnumber,
            });
            for item in &player.items {
                if item.itemid() != 0 {
                    let item_name = match WEAPINDICIES.get(&item.defindex.unwrap_or(u32::MAX)) {
                        Some(name) => Some(name.to_string()),
                        None => None,
                    };
                    let skin_name = match PAINTKITS.get(&item.paintindex.unwrap_or(u32::MAX)) {
                        Some(name) => Some(name.to_string()),
                        None => None,
                    };
                    self.skins.push(EconItem {
                        account_id: item.accountid,
                        item_id: item.itemid,
                        def_index: item.defindex,
                        paint_index: item.paintindex,
                        rarity: item.rarity,
                        quality: item.quality,
                        paint_seed: item.paintseed,
                        paint_wear: item.paintwear,
                        quest_id: item.questid,
                        dropreason: item.dropreason,
                        custom_name: item.customname.clone(),
                        inventory: item.inventory,
                        ent_idx: item.entindex,
                        steamid: player.xuid,
                        item_name,
                        skin_name,
                    });
                }
            }
        }
        Ok(())
    }
    pub fn parse_player_stats_update(&mut self, _bytes: &[u8]) -> Result<(), DemoParserError> {
        // let upd: CCSUsrMsg_PlayerStatsUpdate = Message::parse_from_bytes(bytes);
        Ok(())
    }
    pub fn parse_file_info(&mut self, _bytes: &[u8]) -> Result<(), DemoParserError> {
        // let _info: CDemoFileInfo = Message::parse_from_bytes(bytes);
        Ok(())
    }
}
