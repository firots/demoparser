//! Qualifies slot resolution without changing the legacy inventory columns.
//! Status 0 means all current slots resolve, not that a u64 mask represents
//! every item/count (e.g. custom knives, R8, flash quantity or C4 ownership).
use super::{entities::Entity, parser_settings::SecondPassParser, variants::Variant};
use crate::first_pass::prop_controller::MY_WEAPONS_OFFSET;

const INDEX_MASK: u32 = (1 << 14) - 1;
const INVALID_HANDLE: u32 = (1 << 24) - 1;

// Stable source-status contract: 0 resolved, 1 life unavailable, 2 nonalive,
// 3 length unavailable, 4 length exceeds bound, 5 slot unavailable,
// 6 entity unavailable, 7 serial mismatch, 8 item definition unavailable.
fn qualify(pawn: Option<&Entity>, life: Option<u32>, item: Option<u32>, entities: &[Option<Entity>]) -> u32 {
    let Some(pawn) = pawn else { return 1; };
    match life.and_then(|id| pawn.props.get(&id)) {
        Some(Variant::U32(0)) => {},
        Some(Variant::U32(_)) => return 2,
        _ => return 1,
    }
    let len = match pawn.props.get(&MY_WEAPONS_OFFSET) {
        Some(Variant::U32(len)) => *len,
        _ => return 3,
    };
    // Bounds malformed vectors; no allocation and no truncation-as-success.
    if len > 64 { return 4; }
    for slot in 1..=len {
        let handle = match pawn.props.get(&(MY_WEAPONS_OFFSET + slot)) {
            Some(Variant::U32(handle)) => *handle,
            _ => return 5,
        };
        // Explicit empty slots differ from a missing property.
        if handle == INVALID_HANDLE || handle == u32::MAX { continue; }
        let Some(entity) = entities.get((handle & INDEX_MASK) as usize).and_then(Option::as_ref) else { return 6; };
        // Demo wire handles use a 14-bit index, not native-server CHandle's
        // 15-bit index. Compare the entire remaining serial; never mask it.
        if entity.serial != handle >> 14 { return 7; }
        if !matches!(item.and_then(|id| entity.props.get(&id)), Some(Variant::U32(def)) if *def > 0) { return 8; }
    }
    0
}

impl SecondPassParser<'_> {
    pub(crate) fn inventory_source_status(&self, entity_id: i32) -> u32 {
        qualify(self.entities.get(entity_id as usize).and_then(Option::as_ref),
            self.prop_controller.special_ids.life_state, self.prop_controller.special_ids.item_def, &self.entities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::entities::EntityType;
    fn entity(index: i32, serial: u32) -> Entity {
        Entity { cls_id: 0, entity_id: index, serial, props: Default::default(), entity_type: EntityType::Normal }
    }
    fn fixture() -> (Entity, Vec<Option<Entity>>) {
        let mut pawn = entity(1, 1);
        pawn.props.insert(1, Variant::U32(0));
        pawn.props.insert(MY_WEAPONS_OFFSET, Variant::U32(1));
        pawn.props.insert(MY_WEAPONS_OFFSET + 1, Variant::U32((8193 << 14) | 2));
        let mut weapon = entity(2, 8193);
        weapon.props.insert(2, Variant::U32(7));
        (pawn, vec![None, None, Some(weapon)])
    }
    fn status(pawn: &Entity, entities: &[Option<Entity>]) -> u32 { qualify(Some(pawn), Some(1), Some(2), entities) }
    #[test]
    fn source_absence_is_not_empty_inventory() {
        let (mut pawn, entities) = fixture();
        assert_eq!(status(&pawn, &entities), 0);
        pawn.props.remove(&(MY_WEAPONS_OFFSET + 1));
        assert_eq!(status(&pawn, &entities), 5);
        pawn.props.insert(MY_WEAPONS_OFFSET, Variant::U32(0));
        assert_eq!(status(&pawn, &entities), 0);
        pawn.props.remove(&1);
        assert_eq!(status(&pawn, &entities), 1);
        pawn.props.insert(1, Variant::U32(2));
        assert_eq!(status(&pawn, &entities), 2);
    }
    #[test]
    fn stale_serial_missing_entity_and_item_are_unavailable() {
        let (pawn, mut entities) = fixture();
        entities[2].as_mut().unwrap().serial = 1; // same low ten bits is NOT equal
        assert_eq!(status(&pawn, &entities), 7);
        entities[2].as_mut().unwrap().serial = 8193;
        entities[2].as_mut().unwrap().props.clear();
        assert_eq!(status(&pawn, &entities), 8);
        entities[2] = None;
        assert_eq!(status(&pawn, &entities), 6);
    }
    #[test]
    fn explicit_empty_slots_and_duplicate_handles_are_supported() {
        let (mut pawn, entities) = fixture();
        pawn.props.insert(MY_WEAPONS_OFFSET, Variant::U32(4));
        for (slot, handle) in [(2, (8193 << 14) | 2), (3, INVALID_HANDLE), (4, u32::MAX)] {
            pawn.props.insert(MY_WEAPONS_OFFSET + slot, Variant::U32(handle));
        }
        assert_eq!(status(&pawn, &entities), 0);
        pawn.props.insert(MY_WEAPONS_OFFSET, Variant::U32(u32::MAX));
        assert_eq!(status(&pawn, &entities), 4);
        pawn.props.remove(&MY_WEAPONS_OFFSET);
        assert_eq!(status(&pawn, &entities), 3);
    }
}
