//! Resolve a command's original pawn, including its entity generation.
//! Full packets may replay old commands after the pawn's slot has been reused.
use super::{entities::Entity, other_netmessages::Class};

pub(super) fn resolve<'a>(
    handle: Option<u32>,
    entities: &'a mut [Option<Entity>],
    classes: &[Class],
) -> Option<&'a mut Entity> {
    let handle = handle?;
    if handle == u32::MAX || handle == 0x00ff_ffff {
        return None;
    }
    // Demo wire handles use 14 index bits; the entire remaining serial matters.
    let index = (handle & 0x3fff) as usize;
    let entity = entities.get_mut(index)?.as_mut()?;
    if entity.entity_id != index as i32
        || entity.serial != handle >> 14
        || classes.get(entity.cls_id as usize)?.name != "CCSPlayerPawn"
    {
        return None;
    }
    // Do not gate by life state: a valid pawn can receive commands while dead.
    Some(entity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{first_pass::sendtables::Serializer, second_pass::{entities::EntityType, variants::Variant}};

    fn classes() -> Vec<Class> {
        ["CCSPlayerPawn", "CBaseModelEntity"].iter().enumerate().map(|(i, name)| Class {
            class_id: i as i32, name: name.to_string(),
            serializer: Serializer { name: name.to_string(), fields: vec![] },
        }).collect()
    }
    fn entity(index: i32, serial: u32, cls_id: u32) -> Entity {
        Entity { cls_id, entity_id: index, serial, props: Default::default(), entity_type: EntityType::Normal }
    }

    #[test]
    fn usercmd_entity_rejects_reused_slot_and_accepts_recreated_pawn() {
        let classes = classes();
        let mut entities = vec![None, Some(entity(1, 227, 0))];
        let old = Some((227 << 14) | 1);
        assert!(resolve(old, &mut entities, &classes).is_some());
        // Wrong class must fail even if a serial coincidentally matches.
        entities[1] = Some(entity(1, 227, 1));
        assert!(resolve(old, &mut entities, &classes).is_none());
        entities[1] = Some(entity(1, 229, 1));
        assert!(resolve(old, &mut entities, &classes).is_none());
        entities[1] = Some(entity(1, 230, 0));
        assert!(resolve(old, &mut entities, &classes).is_none());
        assert!(resolve(Some((230 << 14) | 1), &mut entities, &classes).is_some());
    }

    #[test]
    fn usercmd_entity_missing_or_invalid_handle_does_not_touch_properties() {
        let classes = classes();
        let mut entities = vec![Some(entity(0, 0, 0)), Some(entity(1, 8193, 0))];
        entities[1].as_mut().unwrap().props.insert(42, Variant::U32(7));
        for handle in [None, Some(u32::MAX), Some(0x00ff_ffff), Some(2), Some((1 << 14) | 1)] {
            assert!(resolve(handle, &mut entities, &classes).is_none());
        }
        assert_eq!(entities[1].as_ref().unwrap().props.get(&42), Some(&Variant::U32(7)));
        assert!(resolve(Some((8193 << 14) | 1), &mut entities, &classes).is_some());
    }

    #[test]
    fn usercmd_entity_uses_full_index_and_does_not_require_alive_properties() {
        let classes = classes();
        let mut entities = vec![None; 2050];
        entities[1] = Some(entity(1, 3, 0));
        // An index above 11 bits must not alias slot 1.
        assert!(resolve(Some((3 << 14) | 2049), &mut entities, &classes).is_none());
        entities[2049] = Some(entity(2049, 3, 0));
        assert_eq!(resolve(Some((3 << 14) | 2049), &mut entities, &classes).unwrap().entity_id, 2049);
        entities[1].as_mut().unwrap().cls_id = 99;
        assert!(resolve(Some((3 << 14) | 1), &mut entities, &classes).is_none());
    }
}
