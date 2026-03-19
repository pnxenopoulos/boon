//! Team number to name mapping for Deadlock.
//!
//! Last updated: 2026-03-19

/// All known team (number, name) pairs sorted by number.
const TEAMS: &[(i64, &str)] = &[(1, "Spectator"), (2, "Hidden King"), (3, "Archmother")];

/// Look up a team name by number. Returns `"TEAM_NOT_FOUND"` for unknown numbers.
pub fn team_name(id: i64) -> &'static str {
    TEAMS
        .iter()
        .find(|&&(k, _)| k == id)
        .map(|&(_, v)| v)
        .unwrap_or("TEAM_NOT_FOUND")
}

/// Return all known (team number, team name) pairs.
pub fn all_teams() -> &'static [(i64, &'static str)] {
    TEAMS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_team_spectator() {
        assert_eq!(team_name(1), "Spectator");
    }

    #[test]
    fn known_team_hidden_king() {
        assert_eq!(team_name(2), "Hidden King");
    }

    #[test]
    fn known_team_archmother() {
        assert_eq!(team_name(3), "Archmother");
    }

    #[test]
    fn unknown_team() {
        assert_eq!(team_name(99), "TEAM_NOT_FOUND");
    }

    #[test]
    fn all_teams_count() {
        assert_eq!(all_teams().len(), 3);
    }
}
