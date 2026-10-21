use crate::*;

/// Return a mapping of team number to team name.
///
/// Returns:
///     dict[int, str]: A new dictionary of recorded enum IDs and readable names.
#[pyfunction]
pub(super) fn team_names() -> HashMap<i64, &'static str> {
    boon_parser::all_teams()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of game mode ID to game mode name.
///
/// Returns:
///     dict[int, str]: A new dictionary of recorded enum IDs and readable names.
#[pyfunction]
pub(super) fn game_mode_names() -> HashMap<i64, &'static str> {
    boon_parser::all_game_modes()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of patron phase ID to phase name.
///
/// Returns:
///     dict[int, str]: A new dictionary of recorded enum IDs and readable names.
///
/// Notes:
///     Phases of ``CNPC_Boss_Tier3.m_ePhase``: ``0=normal`` (shielded),
///     ``1=final`` (killable), ``2=transforming`` (vulnerable). Non-patron
///     objectives report ``0`` by default.
#[pyfunction]
pub(super) fn patron_phase_names() -> HashMap<i64, &'static str> {
    boon_parser::all_patron_phases()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of hit group ID to hit group name.
///
/// Returns:
///     dict[int, str]: A new dictionary of recorded enum IDs and readable names.
///
/// Notes:
///     Hit group IDs are Source 2's ``HitGroup_t`` enum values, used by the
///     ``hitgroup_id`` column on the ``damage`` frame (``0=generic``, ``1=head``,
///     ``2=chest`` ... ``19=head_no_resist``; ``-1=invalid``).
#[pyfunction]
pub(super) fn hitgroup_names() -> HashMap<i64, &'static str> {
    boon_parser::all_hitgroups()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of life state ID to life state name.
///
/// Returns:
///     dict[int, str]: A new dictionary of recorded enum IDs and readable names.
///
/// Notes:
///     Life state IDs are Source 2's ``LifeState_t`` enum values, used by the
///     ``lifestate`` column on ``player_ticks`` (``0=alive``, ``1=dying``,
///     ``2=dead``, ``3=respawnable``, ``4=respawning``).
#[pyfunction]
pub(super) fn lifestate_names() -> HashMap<i64, &'static str> {
    boon_parser::all_lifestates()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Read native ID/name maps from one local boon-data snapshot.
///
/// Args:
///     directory: Local catalog directory already verified by the caller.
///
/// Returns:
///     Tuple of hero, ability, display-name, modifier, and breakable dictionaries.
///
/// Raises:
///     ValueError: Catalog definitions are missing or contain conflicting names.
#[pyfunction]
pub(super) fn _read_catalog_names(py: Python<'_>, directory: PathBuf) -> PyResult<Py<PyAny>> {
    let names = py
        .detach(|| boon_parser::CatalogNames::from_directory(&directory))
        .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
    (
        names.heroes,
        names.abilities,
        names.ability_display_names,
        names.modifiers,
        names.breakables,
    )
        .into_py_any(py)
}
