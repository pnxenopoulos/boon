//! Life state ID to name mapping for Deadlock.
//!
//! Values are Source 2's `LifeState_t` enum, taken from
//! `GameTracking-Deadlock/DumpSource2/schemas/client/LifeState_t.h`:
//!
//! ```text
//! enum LifeState_t : uint32_t
//! {
//!     LIFE_ALIVE = 0,
//!     LIFE_DYING = 1,
//!     LIFE_DEAD = 2,
//!     LIFE_RESPAWNABLE = 3,
//!     LIFE_RESPAWNING = 4,
//!     NUM_LIFESTATES = 5,
//! };
//! ```
//!
//! `NUM_LIFESTATES` (5) is a sentinel for the number of enum members, not a
//! real life state, so it is omitted.

/// All known life state (ID, name) pairs sorted by ID.
const LIFESTATES: &[(i64, &str)] = &[
    (0, "alive"),
    (1, "dying"),
    (2, "dead"),
    (3, "respawnable"),
    (4, "respawning"),
];

/// Look up a life state name by ID. Returns `"LIFESTATE_NOT_FOUND"` for unknown IDs.
pub fn lifestate_name(id: i64) -> &'static str {
    LIFESTATES
        .iter()
        .find(|&&(k, _)| k == id)
        .map(|&(_, v)| v)
        .unwrap_or("LIFESTATE_NOT_FOUND")
}

/// Return all known (life state ID, life state name) pairs.
pub fn all_lifestates() -> &'static [(i64, &'static str)] {
    LIFESTATES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_lifestate_alive() {
        assert_eq!(lifestate_name(0), "alive");
    }

    #[test]
    fn known_lifestate_dead() {
        assert_eq!(lifestate_name(2), "dead");
    }

    #[test]
    fn known_lifestate_respawning() {
        assert_eq!(lifestate_name(4), "respawning");
    }

    #[test]
    fn count_sentinel_not_present() {
        assert_eq!(lifestate_name(5), "LIFESTATE_NOT_FOUND");
    }

    #[test]
    fn unknown_lifestate() {
        assert_eq!(lifestate_name(99), "LIFESTATE_NOT_FOUND");
    }

    #[test]
    fn all_lifestates_count() {
        assert_eq!(all_lifestates().len(), 5);
    }
}
