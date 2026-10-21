//! Polars views of the shared, cached Rust zipline history.

use boon_parser::ziplines::{ZiplineHistory, ZiplineNode};

use crate::*;

impl Demo {
    fn zipline_history(&self) -> PyResult<&ZiplineHistory> {
        if self.cached_ziplines.get().is_none() {
            let history = ZiplineHistory::build(&self.parser).map_err(to_py_err)?;
            let _ = self.cached_ziplines.set(history);
        }
        Ok(self
            .cached_ziplines
            .get()
            .expect("zipline history initialized"))
    }

    pub(super) fn ensure_zipline_frames(&mut self) -> PyResult<()> {
        if self.cached_datasets[Dataset::ZiplineNodes].is_some()
            && self.cached_datasets[Dataset::ZiplineChanges].is_some()
        {
            return Ok(());
        }
        let history = self.zipline_history()?;
        let mut nodes = NodeColumns::default();
        for node in history.nodes() {
            nodes.push(node.tick, node, None);
        }
        let node_frame = nodes.into_frame(false)?;
        let changes = history.changes();
        let changes_frame = df_from_columns(vec![
            Column::new(
                "tick".into(),
                changes.iter().map(|row| row.tick).collect::<Vec<_>>(),
            ),
            Column::new(
                "node_handle".into(),
                changes
                    .iter()
                    .map(|row| row.node_handle)
                    .collect::<Vec<_>>(),
            ),
            Column::new(
                "change".into(),
                changes
                    .iter()
                    .map(|row| row.change.as_str())
                    .collect::<Vec<_>>(),
            ),
            Column::new(
                "team_num".into(),
                changes.iter().map(|row| row.team_num).collect::<Vec<_>>(),
            ),
        ])
        .map_err(frame_error)?;
        self.cached_datasets[Dataset::ZiplineNodes] = Some(node_frame);
        self.cached_datasets[Dataset::ZiplineChanges] = Some(changes_frame);
        Ok(())
    }

    pub(super) fn zipline_snapshot_frame(&self, predicate: &TickPredicate) -> PyResult<DataFrame> {
        let ticks = self
            .parser
            .distinct_ticks()
            .map_err(to_py_err)?
            .into_iter()
            .filter(|&tick| predicate.matches(tick))
            .collect::<Vec<_>>();
        let mut rows = NodeColumns::default();
        self.zipline_history()?
            .visit_snapshots(&ticks, |tick, node, team| rows.push(tick, node, team));
        rows.into_frame(true)
    }
}

fn frame_error(error: PolarsError) -> PyErr {
    InvalidDemoError::new_err(format!("Failed to build zipline frame: {error}"))
}

struct NodeColumns {
    tick: Vec<i32>,
    handle: Vec<u32>,
    lane: Vec<Option<i32>>,
    position: [Vec<Option<f32>>; 3],
    distance: Vec<Option<f32>>,
    capturable: Vec<Option<bool>>,
    connections: ListPrimitiveChunkedBuilder<UInt32Type>,
    team: Vec<Option<i32>>,
}

impl Default for NodeColumns {
    fn default() -> Self {
        Self {
            tick: Vec::new(),
            handle: Vec::new(),
            lane: Vec::new(),
            position: Default::default(),
            distance: Vec::new(),
            capturable: Vec::new(),
            connections: ListPrimitiveChunkedBuilder::new(
                "connections".into(),
                0,
                0,
                DataType::UInt32,
            ),
            team: Vec::new(),
        }
    }
}

impl NodeColumns {
    fn push(&mut self, tick: i32, node: &ZiplineNode, team: Option<i32>) {
        self.tick.push(tick);
        self.handle.push(node.node_handle);
        self.lane.push(node.geometry.lane);
        for (values, coordinate) in self.position.iter_mut().zip(node.geometry.position) {
            values.push(coordinate);
        }
        self.distance.push(node.geometry.cumulative_distance);
        self.capturable.push(node.geometry.capturable);
        match node.geometry.connections.as_deref() {
            Some(handles) => self.connections.append_slice(handles),
            None => self.connections.append_null(),
        }
        self.team.push(team);
    }

    fn into_frame(mut self, include_team: bool) -> PyResult<DataFrame> {
        let [x, y, z] = self.position;
        let mut columns = vec![
            Column::new("tick".into(), self.tick),
            Column::new("node_handle".into(), self.handle),
            Column::new("lane".into(), self.lane),
            Column::new("x".into(), x),
            Column::new("y".into(), y),
            Column::new("z".into(), z),
            self.connections.finish().into_series().into_column(),
            Column::new("cumulative_distance".into(), self.distance),
            Column::new("capturable".into(), self.capturable),
        ];
        if include_team {
            columns.push(Column::new("team_num".into(), self.team));
        }
        df_from_columns(columns).map_err(frame_error)
    }
}
