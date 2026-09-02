use crate::*;

/// Return a mapping of hero ID to hero name.
///
/// Returns:
///     A dict mapping hero IDs (int) to hero names (str).
#[pyfunction]
pub(super) fn hero_names() -> HashMap<i64, &'static str> {
    boon_parser::all_heroes()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of team number to team name.
///
/// Returns:
///     A dict mapping team numbers (int) to team names (str).
#[pyfunction]
pub(super) fn team_names() -> HashMap<i64, &'static str> {
    boon_parser::all_teams()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of ability hash ID to ability name.
///
/// Returns:
///     A dict mapping MurmurHash2 ability IDs (int) to ability names (str).
#[pyfunction]
pub(super) fn ability_names() -> HashMap<u32, &'static str> {
    boon_parser::all_abilities()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return exact internal ability/item names mapped to English display names.
///
/// Includes every exact top-level abilities.vdata name present in Valve's
/// English hero/item localization catalogs, including ``ability_*``,
/// ``upgrade_*``, and ``citadel_ability_*``. Hidden, test, retired, or
/// otherwise unlocalized entries are omitted rather than assigned a
/// synthesized display name.
///
/// Returns:
///     A dict mapping internal names (str) to English in-game names (str).
#[pyfunction]
pub(super) fn ability_display_names() -> HashMap<&'static str, &'static str> {
    boon_parser::all_ability_display_names()
        .iter()
        .copied()
        .collect()
}

/// Return a mapping of game mode ID to game mode name.
///
/// Returns:
///     A dict mapping game mode IDs (int) to game mode names (str).
#[pyfunction]
pub(super) fn game_mode_names() -> HashMap<i64, &'static str> {
    boon_parser::all_game_modes()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of modifier hash ID to modifier name.
///
/// Returns:
///     A dict mapping MurmurHash2 modifier IDs (int) to modifier names (str).
#[pyfunction]
pub(super) fn modifier_names() -> HashMap<u32, &'static str> {
    boon_parser::all_modifiers()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of patron phase ID to phase name.
///
/// Phases are the values of the ``CNPC_Boss_Tier3.m_ePhase`` netvar:
/// ``0=normal`` (shielded), ``1=final`` (killable), ``2=transforming``
/// (vulnerable). Non-patron objectives report ``0`` by default.
///
/// Returns:
///     A dict mapping patron phase IDs (int) to phase names (str).
#[pyfunction]
pub(super) fn patron_phase_names() -> HashMap<i64, &'static str> {
    boon_parser::all_patron_phases()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of hit group ID to hit group name.
///
/// Hit group IDs are the values of Source 2's ``HitGroup_t`` enum, used by the
/// ``hitgroup_id`` column on the ``damage`` frame: ``0=generic``, ``1=head``,
/// ``2=chest``, ``3=stomach``, the limbs (``4``–``7``), ``8=neck``,
/// ``10=gear``, ``11=special``, the tier-2 / drone boss weakpoints
/// (``12``–``18``), ``19=head_no_resist``, and ``-1=invalid``.
///
/// Returns:
///     A dict mapping hit group IDs (int) to hit group names (str).
#[pyfunction]
pub(super) fn hitgroup_names() -> HashMap<i64, &'static str> {
    boon_parser::all_hitgroups()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of life state ID to life state name.
///
/// Life state IDs are the values of Source 2's ``LifeState_t`` enum, used by
/// the ``lifestate`` column on ``player_ticks``: ``0=alive``, ``1=dying``,
/// ``2=dead``, ``3=respawnable``, ``4=respawning``.
///
/// Returns:
///     A dict mapping life state IDs (int) to life state names (str).
#[pyfunction]
pub(super) fn lifestate_names() -> HashMap<i64, &'static str> {
    boon_parser::all_lifestates()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}
