//! Hero ID to name mapping for Deadlock.
//!
//! Last updated: 2026-03-19

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
    (82, "Raven"),
];

/// Look up a hero name by ID. Returns `"NAME_NOT_FOUND"` for unknown IDs.
pub fn hero_name(id: i64) -> &'static str {
    HEROES
        .iter()
        .find(|&&(k, _)| k == id)
        .map(|&(_, v)| v)
        .unwrap_or("NAME_NOT_FOUND")
}

/// Return all known (hero ID, hero name) pairs.
pub fn all_heroes() -> &'static [(i64, &'static str)] {
    HEROES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_hero_infernus() {
        assert_eq!(hero_name(1), "Infernus");
    }

    #[test]
    fn known_hero_base() {
        assert_eq!(hero_name(0), "Base");
    }

    #[test]
    fn known_hero_last() {
        assert_eq!(hero_name(82), "Raven");
    }

    #[test]
    fn unknown_hero() {
        assert_eq!(hero_name(999), "NAME_NOT_FOUND");
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
