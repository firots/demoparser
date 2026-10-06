//! Optional provenance for reconstructed command snapshots, not physical input events.
//! Presence describes the protobuf after delta reconstruction. Omitted delta fields
//! may therefore remain present; an explicit clear is present with its default value.
use crate::first_pass::prop_controller::*;
use crate::second_pass::variants::{InputHistory, UserCmdAttack1Observation, Variant};
use ahash::AHashMap;
use csgoproto::{CSubtickMoveStep, CsgoInputHistoryEntryPb, CsgoUserCmdPb};

pub(crate) const PROPS: &[&str] = &[
    "usercmd_observed_tick", "usercmd_observed_sequence", "usercmd_player_slot",
    "usercmd_source_kind", "usercmd_legacy_command_number", "usercmd_client_tick",
    "usercmd_pawn_entity_handle", "usercmd_attack1_start_history_index",
    "usercmd_attack2_start_history_index", "usercmd_base_field_presence",
    "usercmd_input_history_presence", "usercmd_subtick_moves_presence",
    "usercmd_attack1_observations", "usercmd_attack1_observations_seen",
];

pub(crate) fn requested(names: &[String]) -> bool {
    names.iter().any(|name| PROPS.contains(&name.as_str()))
}

#[derive(Clone, Copy)]
pub(crate) struct CaptureOptions {
    pub enabled: bool,
    base_presence: bool,
    history_presence: bool,
    move_presence: bool,
    attacks: bool,
}

impl CaptureOptions {
    pub(crate) fn new(names: &[String]) -> Self {
        let has = |name: &str| names.iter().any(|value| value == name);
        Self {
            enabled: requested(names),
            base_presence: has("usercmd_base_field_presence"),
            history_presence: has("usercmd_input_history_presence"),
            move_presence: has("usercmd_subtick_moves_presence"),
            attacks: has("usercmd_attack1_observations") || has("usercmd_attack1_observations_seen"),
        }
    }
}

fn mask<const N: usize>(present: [bool; N]) -> u32 {
    present.into_iter().enumerate().fold(0, |bits, (i, value)| bits | ((value as u32) << i))
}

fn history_presence(input: &CsgoInputHistoryEntryPb) -> u32 {
    mask([
        input.view_angles.is_some(),
        input.view_angles.as_ref().is_some_and(|a| a.x.is_some()),
        input.view_angles.as_ref().is_some_and(|a| a.y.is_some()),
        input.view_angles.as_ref().is_some_and(|a| a.z.is_some()),
        input.render_tick_count.is_some(), input.render_tick_fraction.is_some(),
        input.player_tick_count.is_some(), input.player_tick_fraction.is_some(),
    ])
}

fn move_presence(input: &CSubtickMoveStep) -> u32 {
    mask([
        input.when.is_some(), input.button.is_some(), input.pressed.is_some(),
        input.analog_forward_delta.is_some(), input.analog_left_delta.is_some(),
        input.pitch_delta.is_some(), input.yaw_delta.is_some(),
    ])
}

fn optional(props: &mut AHashMap<u32, Variant>, id: u32, value: Option<Variant>) {
    if let Some(value) = value { props.insert(id, value); } else { props.remove(&id); }
}

const MAX_PENDING_AGE_TICKS: i32 = 64;
const MAX_PENDING_RECORDS: usize = 1024;

/// PacketEntities may precede UserCmds in a demo tick. Drain only after an
/// actual output row, so a later command can survive into the next snapshot.
pub(crate) fn collected(props: &mut AHashMap<u32, Variant>) {
    if let Some(Variant::UserCmdAttack1Observations(records)) = props.get_mut(&USERCMD_ATTACK1_OBSERVATIONS) {
        records.clear();
    }
}

/// Called only when at least one provenance column was requested. The existing
/// command fields and their defaulting behavior are deliberately left untouched.
pub(crate) fn capture(
    props: &mut AHashMap<u32, Variant>, command: &CsgoUserCmdPb,
    tick: i32, player_slot: Option<i32>, source_kind: u32,
    options: CaptureOptions,
) {
    let Some(base) = command.base.as_ref() else { return; };
    let Some(handle) = base.pawn_entity_handle.filter(|handle| *handle & 0x7ff != PLAYER_ENTITY_HANDLE_MISSING as u32) else {
        // Legacy columns can still be overwritten at the default masked entity
        // index. Do not leave a matching-tick provenance marker for those values.
        for name in PROPS {
            props.remove(crate::maps::CUSTOM_PLAYER_PROP_IDS.get(name).unwrap());
        }
        return;
    };
    let previous_tick = match props.get(&USERCMD_OBSERVED_TICK) {
        Some(Variant::I32(value)) => Some(*value), _ => None,
    };
    let sequence = match props.get(&USERCMD_OBSERVED_SEQUENCE) {
        Some(Variant::U64(previous)) => previous + 1,
        _ => 1,
    };
    props.insert(USERCMD_OBSERVED_SEQUENCE, Variant::U64(sequence));
    props.insert(USERCMD_OBSERVED_TICK, Variant::I32(tick));
    optional(props, USERCMD_PLAYER_SLOT, player_slot.map(Variant::I32));
    props.insert(USERCMD_SOURCE_KIND, Variant::U32(source_kind));
    props.insert(USERCMD_PAWN_ENTITY_HANDLE, Variant::U32(handle));
    optional(props, USERCMD_LEGACY_COMMAND_NUMBER, base.legacy_command_number.map(Variant::I32));
    optional(props, USERCMD_CLIENT_TICK, base.client_tick.map(Variant::I32));
    optional(props, USERCMD_ATTACK_START_HISTORY_INDEX_1, command.attack1_start_history_index.map(Variant::I32));
    optional(props, USERCMD_ATTACK_START_HISTORY_INDEX_2, command.attack2_start_history_index.map(Variant::I32));
    if options.base_presence { props.insert(USERCMD_BASE_FIELD_PRESENCE, Variant::U32(mask([
        base.buttons_pb.is_some(),
        base.buttons_pb.as_ref().is_some_and(|b| b.buttonstate1.is_some()),
        base.buttons_pb.as_ref().is_some_and(|b| b.buttonstate2.is_some()),
        base.buttons_pb.as_ref().is_some_and(|b| b.buttonstate3.is_some()),
        base.viewangles.is_some(),
        base.viewangles.as_ref().is_some_and(|a| a.x.is_some()),
        base.viewangles.as_ref().is_some_and(|a| a.y.is_some()),
        base.viewangles.as_ref().is_some_and(|a| a.z.is_some()),
        base.forwardmove.is_some(), base.leftmove.is_some(), base.impulse.is_some(),
        base.mousedx.is_some(), base.mousedy.is_some(), base.weaponselect.is_some(),
        base.consumed_server_angle_changes.is_some(), command.left_hand_desired.is_some(),
    ]))); }
    if options.history_presence {
        props.insert(USERCMD_INPUT_HISTORY_PRESENCE, Variant::U32Vec(command.input_history.iter().map(history_presence).collect()));
    }
    if options.move_presence {
        props.insert(USERCMD_SUBTICK_MOVES_PRESENCE, Variant::U32Vec(base.subtick_moves.iter().map(move_presence).collect()));
    }
    if options.attacks {
        let mut seen = match props.get(&USERCMD_ATTACK1_OBSERVATIONS_SEEN) {
            Some(Variant::U64(value)) => *value, _ => 0,
        };
        let records = props.entry(USERCMD_ATTACK1_OBSERVATIONS)
            .or_insert_with(|| Variant::UserCmdAttack1Observations(vec![]));
        if let Variant::UserCmdAttack1Observations(records) = records {
            // Drain only after emission. Bound pending memory relative to the
            // newest command, not the eventual collection tick. The seen
            // counter still includes records pruned by these resource bounds.
            if previous_tick != Some(tick) {
                records.retain(|record| (tick.saturating_sub(MAX_PENDING_AGE_TICKS)..=tick).contains(&record.observed_tick));
            }
            if let Some(index) = command.attack1_start_history_index.filter(|index| *index >= 0) {
                let entry = command.input_history.get(index as usize);
                let history = entry.map(|entry| {
                    let angles = entry.view_angles.unwrap_or_default();
                    InputHistory {
                        x: angles.x(), y: angles.y(), z: angles.z(),
                        render_tick_count: entry.render_tick_count(),
                        render_tick_fraction: entry.render_tick_fraction(),
                        player_tick_count: entry.player_tick_count(),
                        player_tick_fraction: entry.player_tick_fraction(),
                    }
                });
                records.push(UserCmdAttack1Observation {
                    observed_tick: tick, sequence, player_slot, source_kind,
                    pawn_entity_handle: handle, client_tick: base.client_tick,
                    legacy_command_number: base.legacy_command_number,
                    history_index: index, history_len: command.input_history.len(),
                    history, history_presence: entry.map_or(0, history_presence),
                });
                seen += 1;
            }
            if records.len() > MAX_PENDING_RECORDS {
                records.drain(..records.len() - MAX_PENDING_RECORDS);
            }
        }
        props.insert(USERCMD_ATTACK1_OBSERVATIONS_SEEN, Variant::U64(seen));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use csgoproto::{CBaseUserCmdPb, CInButtonStatePb, CMsgQAngle};

    fn capture(props: &mut AHashMap<u32, Variant>, command: &CsgoUserCmdPb,
               tick: i32, slot: Option<i32>, kind: u32) {
        super::capture(props, command, tick, slot, kind,
            CaptureOptions::new(&PROPS.iter().map(|name| name.to_string()).collect::<Vec<_>>()));
    }

    fn command() -> CsgoUserCmdPb {
        CsgoUserCmdPb { base: Some(CBaseUserCmdPb { pawn_entity_handle: Some(123), ..Default::default() }), ..Default::default() }
    }

    #[test]
    fn presence_distinguishes_missing_zero_false_and_nonfinite() {
        assert_eq!(history_presence(&CsgoInputHistoryEntryPb::default()), 0);
        let history = CsgoInputHistoryEntryPb {
            view_angles: Some(CMsgQAngle { x: Some(0.0), y: Some(f32::NAN), z: None }),
            player_tick_count: Some(0), player_tick_fraction: Some(0.0), ..Default::default()
        };
        assert_eq!(history_presence(&history), 1 | 2 | 4 | 64 | 128);
        let step = CSubtickMoveStep { when: Some(0.0), button: Some(0), pressed: Some(false), ..Default::default() };
        assert_eq!(move_presence(&step), 7);
        assert_eq!(move_presence(&CSubtickMoveStep::default()), 0);
    }

    #[test]
    fn optional_fields_clear_instead_of_reusing_previous_command() {
        let mut props = AHashMap::default();
        let mut first = command();
        first.base.as_mut().unwrap().client_tick = Some(0);
        first.base.as_mut().unwrap().legacy_command_number = Some(17);
        first.attack1_start_history_index = Some(0);
        first.attack2_start_history_index = Some(-1);
        capture(&mut props, &first, 100, Some(3), 1);
        assert_eq!(props[&USERCMD_CLIENT_TICK], Variant::I32(0));
        assert_eq!(props[&USERCMD_ATTACK_START_HISTORY_INDEX_2], Variant::I32(-1));
        capture(&mut props, &command(), 101, Some(3), 1);
        for id in [USERCMD_CLIENT_TICK, USERCMD_LEGACY_COMMAND_NUMBER, USERCMD_ATTACK_START_HISTORY_INDEX_1, USERCMD_ATTACK_START_HISTORY_INDEX_2] {
            assert!(!props.contains_key(&id));
        }
        assert_eq!(props[&USERCMD_OBSERVED_SEQUENCE], Variant::U64(2));
        assert_eq!(props[&USERCMD_OBSERVED_TICK], Variant::I32(101));
    }

    #[test]
    fn empty_history_has_observation_but_missing_command_does_not() {
        let mut props = AHashMap::default();
        capture(&mut props, &CsgoUserCmdPb::default(), 50, Some(0), 1);
        assert!(props.is_empty());
        capture(&mut props, &command(), 51, Some(0), 1);
        assert_eq!(props[&USERCMD_INPUT_HISTORY_PRESENCE], Variant::U32Vec(vec![]));
        assert_eq!(props[&USERCMD_SUBTICK_MOVES_PRESENCE], Variant::U32Vec(vec![]));
        assert_eq!(props[&USERCMD_OBSERVED_TICK], Variant::I32(51));
        assert_eq!(props[&USERCMD_BASE_FIELD_PRESENCE], Variant::U32(0));
    }

    #[test]
    fn absent_or_invalid_pawn_invalidates_provenance() {
        let mut props = AHashMap::default();
        capture(&mut props, &command(), 51, Some(0), 1);
        for handle in [None, Some(0x00ff_ffff), Some(u32::MAX), Some(0x8000_07ff)] {
            let mut invalid = command();
            invalid.base.as_mut().unwrap().pawn_entity_handle = handle;
            capture(&mut props, &invalid, 52, Some(0), 1);
            assert!(!props.contains_key(&USERCMD_OBSERVED_TICK));
        }
    }

    #[test]
    fn repeated_commands_in_one_tick_keep_distinct_sequence() {
        let mut props = AHashMap::default();
        capture(&mut props, &command(), 51, Some(4), 1);
        capture(&mut props, &command(), 51, Some(4), 2);
        assert_eq!(props[&USERCMD_OBSERVED_TICK], Variant::I32(51));
        assert_eq!(props[&USERCMD_OBSERVED_SEQUENCE], Variant::U64(2));
        assert_eq!(props[&USERCMD_PLAYER_SLOT], Variant::I32(4));
        assert_eq!(props[&USERCMD_SOURCE_KIND], Variant::U32(2));
    }

    #[test]
    fn old_fields_are_untouched_even_when_absent_in_new_command() {
        let mut props = AHashMap::default();
        props.insert(USERCMD_BUTTONS_HELD, Variant::U64(16));
        props.insert(USERCMD_VIEWANGLE_X, Variant::F32(12.0));
        capture(&mut props, &command(), 51, Some(0), 1);
        assert_eq!(props[&USERCMD_BUTTONS_HELD], Variant::U64(16));
        assert_eq!(props[&USERCMD_VIEWANGLE_X], Variant::F32(12.0));
        assert_eq!(props[&USERCMD_BASE_FIELD_PRESENCE], Variant::U32(0));
    }

    #[test]
    fn delta_inheritance_and_explicit_clear_retain_presence() {
        let mut initial = command();
        initial.attack1_start_history_index = Some(4);
        initial.base.as_mut().unwrap().buttons_pb = Some(CInButtonStatePb {
            buttonstate1: Some(1), buttonstate2: Some(2), buttonstate3: None,
        });
        // Attack index clear; nested buttonstate2 clear. buttonstate1 is inherited.
        let next = crate::second_pass::usercmd_delta::apply_delta(&initial, &[0x37, 0x0a, 0x03, 0x1a, 0x01, 0x17]).unwrap();
        let mut props = AHashMap::default();
        capture(&mut props, &next, 51, Some(0), 2);
        assert_eq!(props[&USERCMD_ATTACK_START_HISTORY_INDEX_1], Variant::I32(-1));
        assert_eq!(props[&USERCMD_BASE_FIELD_PRESENCE], Variant::U32(7));
        assert_eq!(next.base.unwrap().buttons_pb.unwrap().buttonstate2, Some(0));
    }

    #[test]
    fn missing_slot_is_not_reported_as_slot_zero() {
        let mut props = AHashMap::default();
        capture(&mut props, &command(), 51, Some(0), 1);
        assert_eq!(props[&USERCMD_PLAYER_SLOT], Variant::I32(0));
        capture(&mut props, &command(), 52, None, 1);
        assert!(!props.contains_key(&USERCMD_PLAYER_SLOT));
    }

    #[test]
    fn first_attack_survives_later_non_attack_command_with_different_history() {
        let mut props = AHashMap::default();
        let mut first = command();
        first.attack1_start_history_index = Some(0);
        first.input_history.push(CsgoInputHistoryEntryPb {
            view_angles: Some(CMsgQAngle { x: Some(12.0), y: Some(33.0), z: None }),
            player_tick_count: Some(101), player_tick_fraction: Some(0.25),
            render_tick_count: Some(96), render_tick_fraction: Some(0.5),
            ..Default::default()
        });
        capture(&mut props, &first, 100, Some(0), 1);
        let mut second = command();
        second.attack1_start_history_index = Some(-1);
        capture(&mut props, &second, 100, Some(0), 2);
        let Variant::UserCmdAttack1Observations(records) = &props[&USERCMD_ATTACK1_OBSERVATIONS] else { panic!() };
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].sequence, 1);
        assert_eq!(records[0].history.as_ref().unwrap().y, 33.0);
        assert_eq!(records[0].history.as_ref().unwrap().player_tick_fraction, 0.25);
        assert_eq!(records[0].history_presence, 247);
        assert_eq!(props[&USERCMD_ATTACK_START_HISTORY_INDEX_1], Variant::I32(-1));
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS_SEEN], Variant::U64(1));
    }

    #[test]
    fn retains_all_same_tick_attacks_including_unsupported_indices() {
        let mut props = AHashMap::default();
        let mut first = command();
        first.attack1_start_history_index = Some(0);
        first.input_history.push(CsgoInputHistoryEntryPb::default());
        capture(&mut props, &first, 100, Some(0), 1);
        first.attack1_start_history_index = Some(99);
        capture(&mut props, &first, 100, Some(0), 2);
        let Variant::UserCmdAttack1Observations(records) = &props[&USERCMD_ATTACK1_OBSERVATIONS] else { panic!() };
        assert_eq!(records.len(), 2);
        assert!(records[0].history.is_some());
        assert!(records[1].history.is_none());
        assert_eq!(records[1].history_index, 99);
        assert_eq!(records[1].history_len, 1);
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS_SEEN], Variant::U64(2));
    }

    #[test]
    fn attack_records_do_not_carry_into_later_rows_or_accumulate_over_skipped_ticks() {
        let mut props = AHashMap::default();
        let mut first = command();
        first.attack1_start_history_index = Some(0);
        capture(&mut props, &first, 100, Some(0), 1);
        let Variant::UserCmdAttack1Observations(records) = &props[&USERCMD_ATTACK1_OBSERVATIONS] else { panic!() };
        assert_eq!(records[0].observed_tick, 100);
        collected(&mut props);
        capture(&mut props, &first, 1000, Some(0), 2);
        let Variant::UserCmdAttack1Observations(records) = &props[&USERCMD_ATTACK1_OBSERVATIONS] else { panic!() };
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].observed_tick, 1000);
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS_SEEN], Variant::U64(2));
        collected(&mut props);
        capture(&mut props, &command(), 1001, Some(0), 2);
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS], Variant::UserCmdAttack1Observations(vec![]));
    }

    #[test]
    fn late_same_tick_attack_survives_next_non_attack_snapshot_exactly_once() {
        let mut props = AHashMap::default();
        let mut attack = command(); attack.attack1_start_history_index = Some(0);
        capture(&mut props, &attack, 100, Some(0), 1);
        collected(&mut props); // PacketEntities outputs the first command.
        capture(&mut props, &attack, 100, Some(0), 2); // UserCmds arrives later.
        capture(&mut props, &command(), 101, Some(0), 2);
        let Variant::UserCmdAttack1Observations(records) = &props[&USERCMD_ATTACK1_OBSERVATIONS] else {panic!()};
        let rows = records;
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].observed_tick, rows[0].sequence), (100, 2));
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS_SEEN], Variant::U64(2));
        collected(&mut props);
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS], Variant::UserCmdAttack1Observations(vec![]));
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS_SEEN], Variant::U64(2));
    }

    #[test]
    fn late_attack_survives_missing_collection_tick_and_drains_once() {
        let mut props = AHashMap::default();
        let mut attack = command(); attack.attack1_start_history_index = Some(0);
        capture(&mut props, &attack, 100, Some(0), 1);
        collected(&mut props);
        capture(&mut props, &attack, 100, Some(0), 2);
        capture(&mut props, &command(), 101, Some(0), 2); // No collection.
        capture(&mut props, &command(), 102, Some(0), 2);
        let Variant::UserCmdAttack1Observations(records) = &props[&USERCMD_ATTACK1_OBSERVATIONS] else {panic!()};
        assert_eq!(records.len(), 1);
        assert_eq!((records[0].observed_tick, records[0].sequence), (100, 2));
        collected(&mut props);
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS], Variant::UserCmdAttack1Observations(vec![]));
    }

    #[test]
    fn pending_age_and_count_bounds_preserve_loss_counter() {
        let mut props = AHashMap::default();
        let mut attack = command(); attack.attack1_start_history_index = Some(0);
        for _ in 0..MAX_PENDING_RECORDS+2 {capture(&mut props, &attack, 100, Some(0), 2);}
        let Variant::UserCmdAttack1Observations(records) = &props[&USERCMD_ATTACK1_OBSERVATIONS] else {panic!()};
        assert_eq!(records.len(), MAX_PENDING_RECORDS);
        assert_eq!(records[0].sequence, 3);
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS_SEEN], Variant::U64((MAX_PENDING_RECORDS+2) as u64));
        capture(&mut props, &command(), 164, Some(0), 2);
        let Variant::UserCmdAttack1Observations(records) = &props[&USERCMD_ATTACK1_OBSERVATIONS] else {panic!()};
        assert_eq!(records.len(), MAX_PENDING_RECORDS);
        capture(&mut props, &command(), 165, Some(0), 2);
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS], Variant::UserCmdAttack1Observations(vec![]));
        assert_eq!(props[&USERCMD_ATTACK1_OBSERVATIONS_SEEN], Variant::U64((MAX_PENDING_RECORDS+2) as u64));
    }

    #[test]
    fn recreated_pawn_starts_new_counters_without_old_pending_records() {
        let mut old = AHashMap::default();
        let mut attack = command(); attack.attack1_start_history_index = Some(0);
        capture(&mut old, &attack, 100, Some(0), 2);
        let mut recreated = AHashMap::default();
        capture(&mut recreated, &command(), 101, Some(0), 2);
        assert_eq!(recreated[&USERCMD_OBSERVED_SEQUENCE], Variant::U64(1));
        assert_eq!(recreated[&USERCMD_ATTACK1_OBSERVATIONS_SEEN], Variant::U64(0));
        assert_eq!(recreated[&USERCMD_ATTACK1_OBSERVATIONS], Variant::UserCmdAttack1Observations(vec![]));
    }

    #[test]
    fn narrow_capture_does_not_build_unrequested_presence_vectors() {
        let mut props = AHashMap::default();
        let options = CaptureOptions::new(&["usercmd_attack1_observations".into()]);
        super::capture(&mut props, &command(), 100, Some(0), 1, options);
        for id in [USERCMD_BASE_FIELD_PRESENCE, USERCMD_INPUT_HISTORY_PRESENCE, USERCMD_SUBTICK_MOVES_PRESENCE] {
            assert!(!props.contains_key(&id));
        }
        assert!(props.contains_key(&USERCMD_ATTACK1_OBSERVATIONS));
    }

    #[test]
    fn attack_column_preserves_missing_rows_slice_and_append() {
        use crate::second_pass::variants::{PropColumn, VarVec};
        let mut props = AHashMap::default();
        let mut first = command();
        first.attack1_start_history_index = Some(0);
        capture(&mut props, &first, 100, Some(0), 1);
        let mut column = PropColumn::new();
        column.push(None);
        column.push(Some(props[&USERCMD_ATTACK1_OBSERVATIONS].clone()));
        let mut other = PropColumn::new();
        other.push(None);
        column.extend_from(&mut other);
        let sliced = column.slice_to_new(&[2, 1, 0]).unwrap();
        let Some(VarVec::UserCmdAttack1Observations(rows)) = sliced.data else { panic!() };
        assert_eq!(rows.iter().map(Vec::len).collect::<Vec<_>>(), [0, 1, 0]);
    }

    #[test]
    fn new_columns_are_registered_and_require_continuous_parsing() {
        for name in PROPS {
            let names = vec![name.to_string()];
            assert!(requested(&names));
            assert!(crate::maps::CUSTOM_PLAYER_PROP_IDS.contains_key(name));
            let expected = if *name == "usercmd_attack1_observations" {
                crate::second_pass::collect_data::PropType::Custom
            } else { crate::second_pass::collect_data::PropType::Player };
            assert_eq!(crate::maps::TYPEHM.get(name), Some(&expected));
            assert!(!crate::first_pass::parser_settings::check_multithreadability(&names));
        }
        assert!(!requested(&["usercmd_input_history".into()]));
    }
}
