//! Patron phase ID to name mapping for Deadlock.
//!
//! Names follow the `CNPC_Boss_Tier3.m_ePhase` netvar values empirically
//! observed in demos and the related modifier names (`modifier_tier3_boss_invuln`,
//! `modifier_citadel_can_damage_tier3phase2_boss`, `modifier_t3boss_phase1`):
//!
//! - `0` — `normal`: shielded by walkers; cannot take damage.
//! - `1` — `final`: last killable state.
//! - `2` — `shields_down`: vulnerable to damage but still has a regenerating barrier.
//!
//! Non-patron objectives report phase `0` by default.

/// All known patron phase (ID, name) pairs sorted by ID.
const PATRON_PHASES: &[(i64, &str)] = &[(0, "normal"), (1, "final"), (2, "shields_down")];

/// Look up a patron phase name by ID. Returns `"PATRON_PHASE_NOT_FOUND"` for unknown IDs.
pub fn patron_phase_name(id: i64) -> &'static str {
    PATRON_PHASES
        .iter()
        .find(|&&(k, _)| k == id)
        .map(|&(_, v)| v)
        .unwrap_or("PATRON_PHASE_NOT_FOUND")
}

/// Return all known (patron phase ID, patron phase name) pairs.
pub fn all_patron_phases() -> &'static [(i64, &'static str)] {
    PATRON_PHASES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_phase_normal() {
        assert_eq!(patron_phase_name(0), "normal");
    }

    #[test]
    fn known_phase_final() {
        assert_eq!(patron_phase_name(1), "final");
    }

    #[test]
    fn known_phase_shields_down() {
        assert_eq!(patron_phase_name(2), "shields_down");
    }

    #[test]
    fn unknown_phase() {
        assert_eq!(patron_phase_name(99), "PATRON_PHASE_NOT_FOUND");
    }

    #[test]
    fn all_patron_phases_count() {
        assert_eq!(all_patron_phases().len(), 3);
    }
}
