# Optional command snapshot provenance

These columns supplement `usercmd_input_history`, `usercmd_subtick_moves` and
the scalar `usercmd_*` columns. Existing numeric columns and nested output
structures are unchanged. The extra entity properties are constructed only
when at least one of these new columns is requested.

| Column | Meaning |
| --- | --- |
| `usercmd_observed_tick` | Demo tick at which the last successfully reconstructed command was applied to this pawn entity. Not a client timestamp. |
| `usercmd_observed_sequence` | Count of applied commands for this entity since creation, including multiple commands within one demo tick. Not a game command number or globally unique identifier. |
| `usercmd_player_slot` | Nullable slot from the containing server command message. Missing does not become slot zero. Not a Steam ID or proof of human ownership. |
| `usercmd_source_kind` | Bit 0: a full `data` payload; bit 1: a `delta_data` payload. Values 1, 2 or 3. |
| `usercmd_legacy_command_number` | Nullable reconstructed protobuf value. |
| `usercmd_client_tick` | Nullable reconstructed protobuf value; do not assume the demo-tick origin. |
| `usercmd_pawn_entity_handle` | Full reconstructed pawn handle, without masking away its serial bits. |
| `usercmd_attack1_start_history_index`, `usercmd_attack2_start_history_index` | Nullable reconstructed indices, including explicit `-1` sentinels. Never clamped to a valid history row. |
| `usercmd_base_field_presence` | Presence mask for existing scalar command columns, defined below. |
| `usercmd_input_history_presence` | One mask for each element of `usercmd_input_history`, in the same order. |
| `usercmd_subtick_moves_presence` | One mask for each element of `usercmd_subtick_moves`, in the same order. |
| `usercmd_attack1_observations` | Retained, undelivered applied commands with attack1 index >= 0, including each command's original tick, resolved history entry and provenance. Records can arrive on a later output row. An array, not a single last-command latch. |
| `usercmd_attack1_observations_seen` | Cumulative number of those nonnegative-index observations since entity creation. Includes unsupported/out-of-range indices. Measures capture/export completeness only. |

Presence means present **after delta reconstruction**, not necessarily sent again
in that packet, finite, valid, or physically measured. An omitted delta field
inherits its baseline. An explicit protobuf clear becomes a present default.
`Some(0)`, `Some(false)` and nonfinite float values all count as present; clients
must apply their own finite/range checks. Optional scalar provenance fields are
removed when absent in the next reconstructed command rather than remaining stale.

Masks use the following bits, in order from bit 0:

- Base: buttons parent, buttonstate1, buttonstate2, buttonstate3, viewangles
  parent, viewangles.x, viewangles.y, viewangles.z, forwardmove, leftmove,
  impulse, mousedx, mousedy, weaponselect, consumed_server_angle_changes,
  left_hand_desired.
- History: view_angles parent, x, y, z, render_tick_count,
  render_tick_fraction, player_tick_count, player_tick_fraction.
- Subtick move: when, button, pressed, analog_forward_delta,
  analog_left_delta, pitch_delta, yaw_delta.

No command is synthesized when decoding fails, a delta has no baseline, the
base is absent, or the addressed entity is absent. The observed tick therefore
remains older (or missing). An absent/invalid pawn handle invalidates the new
provenance if legacy handling reaches an existing default-index entity. Existing
legacy handling is preserved. Entity deletion/recreation clears entity-local
properties and restarts the sequence.

The two repeated columns use the existing parser vector representation, which
uses an empty vector for both a missing row and an empty list. A consumer **must
join the scalar observed tick/handle/sequence**, not interpret an empty vector
alone as an observed empty command. Rows are last-command snapshots; earlier
commands within the same tick can be overwritten in the legacy snapshot columns.
Between consecutive rows for the same entity, a sequence jump larger than one
indicates additional successfully applied commands overwritten in the snapshot.
Across a row gap it also includes applied commands in the unexported interval.
It **does not** detect malformed messages, failed full/delta decoding or missing
delta baselines; those paths never increment it. Client/legacy clocks can assist
continuity diagnostics when present but also do not prove complete delivery.

A failed full payload currently leaves the older baseline intact, so a later
accepted delta can have an uncertain baseline. This pre-existing behavior is
unchanged. No failure-since-full counter or complete delivery guarantee is provided.
Use the records as supported source observations, not as a trusted click ledger.

The scalar attack indices must be requested with `usercmd_input_history` and its
presence/freshness fields from the **same row**. The new attack-observation array
instead binds each index to its own command's history inside capture. It retains
all nonnegative indices since delivery, including two attacks in one tick.
An invalid index has null history and zero history presence; an in-range entry
can still have missing or invalid fields. Inherited positive indices are retained
as observations too: the array is not a count of unique physical clicks.

Each record contains observed tick, applied sequence, optional player slot,
source kind, full pawn handle, optional client/legacy clocks, original index,
history length, optional resolved history and its presence mask. No later command
can replace that history. The array is drained only after a row for that player
is actually written. A command arriving after entity collection can therefore
be delivered on a later row, even across a short collection gap. Its observed
tick is never restamped to the output tick. On a new command tick, records more
than 64 ticks old are pruned; at most 1,024 pending records are retained. The
cumulative counter still includes pruned records, so loss remains observable.
For adjacent rows without entity reset, its difference must equal the number of
exported attack records. A counter decrease is an identity boundary. Missing
row intervals and the first/last collection boundaries are censored, not proof
of complete delivery. Filtering can delay delivery or reach the retention bound.

Consumers needing complete capture must request every output tick, before any
window/player/life/round filtering. Otherwise the output is explicitly a bounded
sample, with gaps and censored boundaries, never a complete attack ledger.
Preserve records without a nearby weapon-fire event: the parser does not classify
why a command did or did not produce a shot.

For optional grouping, a supported history player tick and exact finite fraction
can identify a repeated source moment within the same demo, actor, full handle
and continuity segment. Both presence bits must be set; valid zero differs from
missing zero. Never deduplicate across life/handle changes, resets or gaps, and
do not discard the original records. Conflicting supported payloads at one key
remain ambiguous. Neither the key nor the attack index proves a physical click.
Missing angles stay unavailable through their mask, even if the stored numeric
default is zero.

Only requested mask families are built; the attack list/counter are built only
when either attack property is requested. The nested attack type has matching
Rust serialization and Python conversion. Existing nested types remain unchanged.

Requesting these properties selects continuous single-thread parsing in Normal
mode because delta baselines cross full-packet boundaries. Forcing multithread
parsing overrides that safeguard and is not supported for complete provenance.
The parser's existing behavior when only legacy columns are requested is unchanged.

The entity serial read from network creation is currently discarded by the
parser. The exported command handle therefore cannot be validated against the
entity's current serial here. Analyzer-side pawn/life/round continuity is still
required and does not turn this into an ownership fix or a serial-checked source.

These fields do not establish physical mouse-click times, server acceptance,
weapon readiness, an attack-to-shot association, or which human controlled a bot.

## Generated sources

Normal builds consume checked-in protobufs, message mappings and item maps.
Regeneration is a deliberate maintenance operation: prepare a known
`GameTracking-CS2` revision under `src/csgoproto`, record that revision, then
opt in with `CS2PARSER_REGENERATE_PROTOS` and/or `CS2PARSER_REGENERATE_MAPS`.
Review and commit any generated source differences together with the source
revision before repinning a consumer. No network clone runs implicitly during
regeneration, and failed map generation fails the build.
