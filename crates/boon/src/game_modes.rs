//! Game mode ID to name mapping for Deadlock.
//!
//! Last updated: 2026-03-21

/// All known game mode (ID, name) pairs sorted by ID.
const GAME_MODES: &[(i64, &str)] = &[(1, "6v6"), (4, "street_brawl")];

/// Look up a game mode name by ID. Returns `"GAME_MODE_NOT_FOUND"` for unknown IDs.
pub fn game_mode_name(id: i64) -> &'static str {
    GAME_MODES
        .iter()
        .find(|&&(k, _)| k == id)
        .map(|&(_, v)| v)
        .unwrap_or("GAME_MODE_NOT_FOUND")
}

/// Return all known (game mode ID, game mode name) pairs.
pub fn all_game_modes() -> &'static [(i64, &'static str)] {
    GAME_MODES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_game_mode_6v6() {
        assert_eq!(game_mode_name(1), "6v6");
    }

    #[test]
    fn known_game_mode_street_brawl() {
        assert_eq!(game_mode_name(4), "street_brawl");
    }

    #[test]
    fn unknown_game_mode() {
        assert_eq!(game_mode_name(99), "GAME_MODE_NOT_FOUND");
    }

    #[test]
    fn all_game_modes_count() {
        assert_eq!(all_game_modes().len(), 2);
    }
}
