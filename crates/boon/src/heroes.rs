//! Resolve the current hero from replay entities.

/// Read the current hero for a zero-based player slot.
///
/// `hero_id_key` must resolve `m_PlayerDataGlobal.m_nHeroID` on
/// `CCitadelPlayerController`. Returns 0 when the slot or hero is unavailable.
/// Read this value when processing an event so hero swaps take effect.
pub fn hero_id_for_player_slot(
    entities: &crate::EntityContainer,
    player_slot: Option<i32>,
    hero_id_key: Option<u64>,
) -> i64 {
    player_slot
        .filter(|&slot| slot >= 0)
        .and_then(|slot| slot.checked_add(1))
        .and_then(|index| entities.get(index))
        .filter(|entity| entity.class_name.as_ref() == "CCitadelPlayerController")
        .map_or(0, |entity| entity.get_i64(hero_id_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller(index: i32, hero_id: i64) -> crate::Entity {
        crate::Entity::from_fields(
            index,
            1,
            0,
            "CCitadelPlayerController",
            true,
            [(42, crate::FieldValue::I64(hero_id))]
                .into_iter()
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn player_slot_hero_follows_swaps_and_late_controllers() {
        let mut entities = crate::EntityContainer::new();
        entities.insert(controller(1, 80)).unwrap();
        let before_swap = hero_id_for_player_slot(&entities, Some(0), Some(42));
        assert_eq!(before_swap, 80);
        assert_eq!(hero_id_for_player_slot(&entities, Some(1), Some(42)), 0);

        entities.insert(controller(1, 66)).unwrap();
        entities.insert(controller(2, 11)).unwrap();
        assert_eq!(hero_id_for_player_slot(&entities, Some(0), Some(42)), 66);
        assert_eq!(hero_id_for_player_slot(&entities, Some(1), Some(42)), 11);
        assert_eq!(before_swap, 80);
    }

    #[test]
    fn player_slot_hero_handles_unavailable_players() {
        let mut entities = crate::EntityContainer::new();
        entities.insert(controller(1, 80)).unwrap();
        for slot in [None, Some(-1), Some(i32::MIN), Some(i32::MAX), Some(12)] {
            assert_eq!(hero_id_for_player_slot(&entities, slot, Some(42)), 0);
        }
        assert_eq!(hero_id_for_player_slot(&entities, Some(0), None), 0);
        assert_eq!(hero_id_for_player_slot(&entities, Some(0), Some(43)), 0);

        entities.insert(controller(1, 0)).unwrap();
        assert_eq!(hero_id_for_player_slot(&entities, Some(0), Some(42)), 0);

        let mut other_entity = controller(1, 66);
        other_entity.class_name = "CCitadelPlayerPawn".into();
        entities.insert(other_entity).unwrap();
        assert_eq!(hero_id_for_player_slot(&entities, Some(0), Some(42)), 0);
    }
}
