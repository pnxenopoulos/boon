//! Hit group ID to name mapping for Deadlock.
//!
//! Values are Source 2's `HitGroup_t` enum, taken from
//! `GameTracking-Deadlock/DumpSource2/schemas/client/HitGroup_t.h`:
//!
//! ```text
//! enum HitGroup_t : uint32_t
//! {
//!     HITGROUP_INVALID = -1,
//!     HITGROUP_GENERIC = 0,
//!     HITGROUP_HEAD = 1,
//!     HITGROUP_CHEST = 2,
//!     HITGROUP_STOMACH = 3,
//!     HITGROUP_LEFTARM = 4,
//!     HITGROUP_RIGHTARM = 5,
//!     HITGROUP_LEFTLEG = 6,
//!     HITGROUP_RIGHTLEG = 7,
//!     HITGROUP_NECK = 8,
//!     HITGROUP_UNUSED = 9,
//!     HITGROUP_GEAR = 10,
//!     HITGROUP_SPECIAL = 11,
//!     HITGROUP_T2_BOSS_FRONT_LEFT_LEG_WEAKPOINT = 12,
//!     HITGROUP_T2_BOSS_FRONT_RIGHT_LEG_WEAKPOINT = 13,
//!     HITGROUP_T2_BOSS_REAR_LEFT_LEG_WEAKPOINT = 14,
//!     HITGROUP_T2_BOSS_REAR_RIGHT_LEG_WEAKPOINT = 15,
//!     HITGROUP_T2_BOSS_HEAD_WEAKPOINT = 16,
//!     HITGROUP_T2_BOSS_BACK_WEAKPOINT = 17,
//!     HITGROUP_DRONE_BOSS_DRONE_WEAKPOINT = 18,
//!     HITGROUP_HEAD_NO_RESIST = 19,
//!     HITGROUP_COUNT = 20,
//! };
//! ```
//!
//! `HITGROUP_COUNT` (20) is a sentinel for the number of enum members, not a
//! real hit group, so it is omitted.

/// All known hit group (ID, name) pairs sorted by ID.
const HITGROUPS: &[(i64, &str)] = &[
    (-1, "invalid"),
    (0, "generic"),
    (1, "head"),
    (2, "chest"),
    (3, "stomach"),
    (4, "left_arm"),
    (5, "right_arm"),
    (6, "left_leg"),
    (7, "right_leg"),
    (8, "neck"),
    (9, "unused"),
    (10, "gear"),
    (11, "special"),
    (12, "t2_boss_front_left_leg_weakpoint"),
    (13, "t2_boss_front_right_leg_weakpoint"),
    (14, "t2_boss_rear_left_leg_weakpoint"),
    (15, "t2_boss_rear_right_leg_weakpoint"),
    (16, "t2_boss_head_weakpoint"),
    (17, "t2_boss_back_weakpoint"),
    (18, "drone_boss_drone_weakpoint"),
    (19, "head_no_resist"),
];

/// Look up a hit group name by ID. Returns `"HITGROUP_NOT_FOUND"` for unknown IDs.
pub fn hitgroup_name(id: i64) -> &'static str {
    HITGROUPS
        .iter()
        .find(|&&(k, _)| k == id)
        .map(|&(_, v)| v)
        .unwrap_or("HITGROUP_NOT_FOUND")
}

/// Return all known (hit group ID, hit group name) pairs.
pub fn all_hitgroups() -> &'static [(i64, &'static str)] {
    HITGROUPS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_hitgroup_generic() {
        assert_eq!(hitgroup_name(0), "generic");
    }

    #[test]
    fn known_hitgroup_head() {
        assert_eq!(hitgroup_name(1), "head");
    }

    #[test]
    fn known_hitgroup_invalid() {
        assert_eq!(hitgroup_name(-1), "invalid");
    }

    #[test]
    fn known_hitgroup_head_no_resist() {
        assert_eq!(hitgroup_name(19), "head_no_resist");
    }

    #[test]
    fn count_sentinel_not_present() {
        assert_eq!(hitgroup_name(20), "HITGROUP_NOT_FOUND");
    }

    #[test]
    fn unknown_hitgroup() {
        assert_eq!(hitgroup_name(99), "HITGROUP_NOT_FOUND");
    }

    #[test]
    fn all_hitgroups_count() {
        assert_eq!(all_hitgroups().len(), 21);
    }
}
