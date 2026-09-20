//! Hero ID to name mapping for Deadlock.
//!
//! Last updated: 2026-06-08

/// All known hero (ID, name) pairs sorted by ID.
const HEROES: &[(i64, &str)] = &[
    (0, "Base"),
    (1, "Infernus"),
    (2, "Seven"),
    (3, "Vindicta"),
    (4, "Lady Geist"),
    (6, "Abrams"),
    (7, "Wraith"),
    (8, "McGinnis"),
    (10, "Paradox"),
    (11, "Dynamo"),
    (12, "Kelvin"),
    (13, "Haze"),
    (14, "Holliday"),
    (15, "Bebop"),
    (16, "Calico"),
    (17, "Grey Talon"),
    (18, "Mo and Krill"),
    (19, "Shiv"),
    (20, "Ivy"),
    (21, "Kali"),
    (25, "Warden"),
    (27, "Yamato"),
    (31, "Lash"),
    (35, "Viscous"),
    (38, "Gunslinger"),
    (39, "The Boss"),
    (46, "Generic Person"),
    (47, "Tokamak"),
    (48, "Wrecker"),
    (49, "Rutger"),
    (50, "Pocket"),
    (51, "Thumper"),
    (52, "Mirage"),
    (53, "Fathom"),
    (54, "Cadence"),
    (55, "Target Dummy"),
    (56, "Bomber"),
    (57, "Shield Guy"),
    (58, "Vyper"),
    (59, "Vandal"),
    (60, "Sinclair"),
    (61, "Trapper"),
    (62, "Raven"),
    (63, "Mina"),
    (64, "Drifter"),
    (65, "Venator"),
    (66, "Victor"),
    (67, "Paige"),
    (68, "Boho"),
    (69, "The Doorman"),
    (70, "Skyrunner"),
    (71, "Swan"),
    (72, "Billy"),
    (73, "Druid"),
    (74, "Graf"),
    (75, "Fortuna"),
    (76, "Graves"),
    (77, "Apollo"),
    (78, "Airheart"),
    (79, "Rem"),
    (80, "Silver"),
    (81, "Celeste"),
    (82, "Opera"),
    (83, "Test Hero"),
];

/// Look up a hero name by ID. Returns `"HERO_NOT_FOUND"` for unknown IDs.
pub fn hero_name(id: i64) -> &'static str {
    HEROES
        .iter()
        .find(|&&(k, _)| k == id)
        .map(|&(_, v)| v)
        .unwrap_or("HERO_NOT_FOUND")
}

/// Return all known (hero ID, hero name) pairs.
pub fn all_heroes() -> &'static [(i64, &'static str)] {
    HEROES
}

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

    #[test]
    fn known_hero_infernus() {
        assert_eq!(hero_name(1), "Infernus");
    }

    #[test]
    fn known_hero_base() {
        assert_eq!(hero_name(0), "Base");
    }

    #[test]
    fn known_hero_raven() {
        assert_eq!(hero_name(62), "Raven");
    }

    #[test]
    fn known_hero_last() {
        assert_eq!(hero_name(83), "Test Hero");
    }

    #[test]
    fn unknown_hero() {
        assert_eq!(hero_name(999), "HERO_NOT_FOUND");
    }

    #[test]
    fn all_heroes_not_empty() {
        assert!(!all_heroes().is_empty());
    }

    #[test]
    fn all_heroes_contains_infernus() {
        assert!(
            all_heroes()
                .iter()
                .any(|&(id, name)| id == 1 && name == "Infernus")
        );
    }
}
