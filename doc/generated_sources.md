# Frozen generated sources

The checked-in protobufs and message dispatch enum were regenerated from
SteamDatabase/GameTracking-CS2 revision
`d45f52d4b0a2c1450bffd5904a7d550be29d3b07` (2026-09-23), using
`prost-build` 0.13.3 and `protoc` 34.0. Input and normalized output hashes are
recorded in `generated_sources.json`. Item maps are identical to the earlier
September 7 generation.

Compared with `9407c5a695eb3954d0046bb22f011db15edd4e2a`, the command schemas
`cs_usercmd.proto`, `usercmd.proto` and `demo.proto` are unchanged.
`networkbasetypes.proto` adds clan flags and game-session float-encoding metadata.
Other transitive generator inputs add metadata/messages and legacy enum names.
Regeneration makes their wire fields representable; it does not implement new
game-session decoding policies or execute the newly declared remote-command
messages.

The item preview's scalar `customname` field became repeated `customnames` at
the same wire tag. The existing scalar item API keeps the last occurrence,
matching the earlier protobuf decoding behavior, including explicit empty text.
A wire-level regression test covers this compatibility rule.

Regeneration is manual and uses deliberately prepared, pinned upstream inputs.
Regenerate protobufs first, rebuild the generator against those bindings, then
regenerate maps and the message enum. Normal builds do not fetch upstream or
regenerate these files. Review input/output hashes and rebuild the bindings
before changing the source revision.

The Node crate previously requested a nonexistent parser `voice` feature. That
obsolete feature request was removed; voice parsing is already unconditional in
the current parser. The lockfile removes only its stale audio dependency graph.
