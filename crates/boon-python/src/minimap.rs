//! Team minimap columns from the shared Rust replay reader.

use crate::*;
use boon_parser::replay_inputs::{EnemyMinimap, MinimapObservation};

#[derive(Default)]
struct MinimapCols {
    tick: Vec<i32>,
    observer_team: Vec<i64>,
    target_team: Vec<i64>,
    target_hero_id: Vec<Option<i64>>,
    target_steam_id: Vec<Option<u64>>,
    target_entity_index: Vec<Option<i32>>,
    target_entity_serial: Vec<Option<u32>>,
    fow_entity_index_raw: Vec<i64>,
    visible_on_map: Vec<Option<bool>>,
    map_x: Vec<Option<i64>>,
    map_y: Vec<Option<i64>>,
    tick_hidden_raw: Vec<Option<i64>>,
    last_seen_tick: Vec<Option<i32>>,
    last_seen_x: Vec<Option<i64>>,
    last_seen_y: Vec<Option<i64>>,
}

impl MinimapCols {
    fn collect(&mut self, rows: &[MinimapObservation]) {
        for row in rows {
            self.tick.push(row.tick);
            self.observer_team.push(row.observer_team);
            self.target_team.push(row.target_team);
            self.target_hero_id.push(row.target_hero_id);
            self.target_steam_id.push(row.target_steam_id);
            self.target_entity_index
                .push(row.target_entity.map(|id| id.index));
            self.target_entity_serial
                .push(row.target_entity.map(|id| id.serial));
            self.fow_entity_index_raw.push(row.fow_entity_index_raw);
            self.visible_on_map.push(row.visible_on_map);
            self.map_x.push(row.position.map(|p| p[0]));
            self.map_y.push(row.position.map(|p| p[1]));
            self.tick_hidden_raw.push(row.tick_hidden_raw);
            self.last_seen_tick
                .push(row.last_seen.map(|(tick, _)| tick));
            self.last_seen_x.push(row.last_seen.map(|(_, p)| p[0]));
            self.last_seen_y.push(row.last_seen.map(|(_, p)| p[1]));
        }
    }

    fn into_frame(self) -> PyResult<DataFrame> {
        df_from_columns(vec![
            Column::new("tick".into(), self.tick),
            Column::new("observer_team".into(), self.observer_team),
            Column::new("target_team".into(), self.target_team),
            Column::new("target_hero_id".into(), self.target_hero_id),
            Column::new("target_steam_id".into(), self.target_steam_id),
            Column::new("target_entity_index".into(), self.target_entity_index),
            Column::new("target_entity_serial".into(), self.target_entity_serial),
            Column::new("fow_entity_index_raw".into(), self.fow_entity_index_raw),
            Column::new("visible_on_map".into(), self.visible_on_map),
            Column::new("map_x".into(), self.map_x),
            Column::new("map_y".into(), self.map_y),
            Column::new("tick_hidden_raw".into(), self.tick_hidden_raw),
            Column::new("last_seen_tick".into(), self.last_seen_tick),
            Column::new("last_seen_x".into(), self.last_seen_x),
            Column::new("last_seen_y".into(), self.last_seen_y),
        ])
        .map_err(|error| {
            InvalidDemoError::new_err(format!("Failed to create minimap DataFrame: {error}"))
        })
    }
}

#[pymethods]
impl Demo {
    /// Recorded enemy hero icons on each team's minimap.
    ///
    /// `observer_team` is the shared team information channel. Coordinates are
    /// native 0..255 map bytes and are populated only while the icon is shown.
    /// Last-seen coordinates come only from a prior shown row with the same
    /// pawn entity identity; unresolved identities keep those fields null.
    /// `target_steam_id` requires one active controller bound to that pawn.
    /// Pass `ticks` to sample exact replay ticks, or `every` for a tick stride.
    #[pyo3(signature = (*, ticks=None, every=None))]
    pub(crate) fn enemy_minimap_ticks(
        &self,
        py: Python<'_>,
        ticks: Option<Vec<i32>>,
        every: Option<i32>,
    ) -> PyResult<PyDataFrame> {
        if ticks.is_some() && every.is_some() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "use ticks or every, not both",
            ));
        }
        if every.is_some_and(|step| step <= 0) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "every must be positive",
            ));
        }
        py.detach(|| {
            let selected = ticks.map(|items| items.into_iter().collect::<HashSet<_>>());
            if selected.as_ref().is_some_and(HashSet::is_empty) {
                return MinimapCols::default().into_frame().map(PyDataFrame);
            }
            let init = self.parser.parse_init().map_err(to_py_err)?;
            let mut reader = EnemyMinimap::new(init.serializers());
            let filter = HashSet::from([
                "CCitadelTeam",
                "CCitadelPlayerPawn",
                "CCitadelPlayerController",
            ]);
            let mut cols = MinimapCols::default();
            let mut last_sample: Option<i32> = None;
            self.parser
                .run_to_end_filtered(&filter, |ctx| {
                    let tick = ctx.tick();
                    let selected_tick = selected.as_ref().is_none_or(|set| set.contains(&tick));
                    let stride_tick =
                        every.is_none_or(|step| last_sample.is_none_or(|last| tick - last >= step));
                    let emit = selected_tick && stride_tick;
                    if emit && every.is_some() {
                        last_sample = Some(tick);
                    }
                    cols.collect(reader.update(ctx, emit));
                })
                .map_err(to_py_err)?;
            cols.into_frame().map(PyDataFrame)
        })
    }
}
