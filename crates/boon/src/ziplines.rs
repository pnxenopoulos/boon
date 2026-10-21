//! Recorded zipline geometry and ownership, separate from derived lane pressure.
//!
//! Geometry is stored only when it changes. Ownership and visibility have their
//! own change log. Snapshots replay both logs; they never depend on cold seeks.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::{Context, Entity, EntityChangeKind, EntityId, FieldValue, Parser, Serializer};

const CLASS: &str = "CCitadelZipLineNode";
const ORIGIN: &str = "CBodyComponent.m_skeletonInstance.m_vecOrigin";
const PROPERTIES: &[&str] = &[
    "m_iPrimaryLane",
    "m_iTeamNum",
    "m_vecConnections",
    "m_flCumulativeDistance",
    "m_bCapturable",
    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX",
    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY",
    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ",
    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
];

/// The recorded map data for a zipline node. Distances use world units.
#[derive(Clone, Debug, PartialEq)]
pub struct ZiplineGeometry {
    /// Raw lane assignment, without a lane-name map.
    pub lane: Option<i32>,
    /// World XYZ. Missing coordinates remain unknown.
    pub position: [Option<f32>; 3],
    /// Full target handles. None means the vector is unavailable or incomplete.
    pub connections: Option<Vec<u32>>,
    /// The game's cumulative path distance, without normalization.
    pub cumulative_distance: Option<f32>,
    /// Whether the game marks this node as capturable.
    pub capturable: Option<bool>,
}

/// An initial node definition or a later geometry revision.
#[derive(Clone, Debug)]
pub struct ZiplineNode {
    /// First tick at which this definition was observed.
    pub tick: i32,
    /// Full entity handle, including the serial number.
    pub node_handle: u32,
    /// Geometry at this observation.
    pub geometry: ZiplineGeometry,
}

/// A recorded ownership or visibility transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZiplineChangeKind {
    /// Initial observation, or observation after leaving visibility.
    Observed,
    /// The recorded team changed, including a change to or from unknown.
    OwnerChanged,
    /// The entity left the visible set; it was not deleted.
    LeftPvs,
    /// An explicit entity deletion.
    Removed,
}

impl ZiplineChangeKind {
    /// Name used by the Python change table.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::OwnerChanged => "owner_changed",
            Self::LeftPvs => "left_pvs",
            Self::Removed => "removed",
        }
    }
}

/// One change, without repeated geometry.
#[derive(Clone, Copy, Debug)]
pub struct ZiplineChange {
    /// Tick of the observation or lifecycle transition.
    pub tick: i32,
    /// Full entity handle; slot reuse creates a different identity.
    pub node_handle: u32,
    /// Transition type.
    pub change: ZiplineChangeKind,
    /// Current recorded team; unknown for removal or loss of visibility.
    pub team_num: Option<i32>,
}

#[derive(Clone, Copy, PartialEq)]
enum Visibility {
    Visible,
    Hidden,
    Removed,
}

#[derive(Clone, Copy)]
struct State {
    geometry: usize,
    team: Option<i32>,
    visibility: Visibility,
}

/// Compact zipline history shared by Rust and Python queries.
#[derive(Default)]
pub struct ZiplineHistory {
    nodes: Vec<ZiplineNode>,
    changes: Vec<ZiplineChange>,
}

impl ZiplineHistory {
    /// Read one filtered serial pass. No game-data catalog is required.
    ///
    /// # Errors
    /// Returns an error when the replay cannot be decoded.
    pub fn build(parser: &Parser) -> crate::Result<Self> {
        let initial = parser.parse_init()?;
        let mut reader = Reader::new(initial.serializers().get(CLASS));
        let mut tracker = Tracker::default();
        drop(initial);
        parser.run_to_end_with_properties(
            &HashSet::from([CLASS]),
            &[(CLASS, PROPERTIES)],
            None,
            |ctx, _| tracker.update(ctx, &mut reader),
        )?;
        // Keep command order for transitions on the same node at the same tick.
        tracker
            .history
            .nodes
            .sort_by_key(|row| (row.tick, row.node_handle));
        tracker
            .history
            .changes
            .sort_by_key(|row| (row.tick, row.node_handle));
        Ok(tracker.history)
    }

    /// Initial geometry and later revisions, in tick order.
    pub fn nodes(&self) -> &[ZiplineNode] {
        &self.nodes
    }

    /// Ownership and visibility transitions, in tick order.
    pub fn changes(&self) -> &[ZiplineChange] {
        &self.changes
    }

    /// Visit known visible nodes after each selected tick, sorted by handle.
    ///
    /// Ticks are sorted and deduplicated. Only observations through each tick
    /// are used. Missing fields stay unknown; missing observations do not expire
    /// a node. A left-PVS or deleted node is omitted until it is observed again.
    pub fn visit_snapshots(
        &self,
        ticks: &[i32],
        mut visit: impl FnMut(i32, &ZiplineNode, Option<i32>),
    ) {
        let mut ticks = ticks.to_vec();
        ticks.sort_unstable();
        ticks.dedup();
        let mut nodes = self.nodes.iter().peekable();
        let mut changes = self.changes.iter().peekable();
        let mut geometry = BTreeMap::new();
        let mut owners = HashMap::new();
        for tick in ticks {
            while nodes.peek().is_some_and(|row| row.tick <= tick) {
                let row = nodes.next().expect("peeked node");
                geometry.insert(row.node_handle, row);
            }
            while changes.peek().is_some_and(|row| row.tick <= tick) {
                let row = changes.next().expect("peeked change");
                let visible = matches!(
                    row.change,
                    ZiplineChangeKind::Observed | ZiplineChangeKind::OwnerChanged
                );
                owners.insert(row.node_handle, (visible, row.team_num));
            }
            for (&handle, &node) in &geometry {
                if let Some(&(true, team)) = owners.get(&handle) {
                    visit(tick, node, team);
                }
            }
        }
    }
}

#[derive(Default)]
struct Tracker {
    history: ZiplineHistory,
    states: HashMap<u32, State>,
    seeded: bool,
    pending: HashMap<u32, ZiplineChangeKind>,
}

impl Tracker {
    fn observe(&mut self, tick: i32, handle: u32, geometry: ZiplineGeometry, team: Option<i32>) {
        let old = self.states.get(&handle).copied();
        let index = match old {
            Some(state) if self.history.nodes[state.geometry].geometry == geometry => {
                state.geometry
            }
            _ => {
                let index = self.history.nodes.len();
                self.history.nodes.push(ZiplineNode {
                    tick,
                    node_handle: handle,
                    geometry,
                });
                index
            }
        };
        let change = match old {
            Some(state) if state.visibility == Visibility::Visible && state.team == team => None,
            Some(state) if state.visibility == Visibility::Visible => {
                Some(ZiplineChangeKind::OwnerChanged)
            }
            _ => Some(ZiplineChangeKind::Observed),
        };
        if let Some(change) = change {
            self.history.changes.push(ZiplineChange {
                tick,
                node_handle: handle,
                change,
                team_num: team,
            });
        }
        self.states.insert(
            handle,
            State {
                geometry: index,
                team,
                visibility: Visibility::Visible,
            },
        );
    }

    fn hide(&mut self, tick: i32, handle: u32, change: ZiplineChangeKind) {
        if let Some(state) = self.states.get_mut(&handle) {
            let next = if change == ZiplineChangeKind::Removed {
                Visibility::Removed
            } else {
                Visibility::Hidden
            };
            if state.visibility != next && state.visibility != Visibility::Removed {
                self.history.changes.push(ZiplineChange {
                    tick,
                    node_handle: handle,
                    change,
                    team_num: None,
                });
                state.visibility = next;
            }
        }
    }

    fn update(&mut self, ctx: &Context, reader: &mut Reader) {
        reader.refresh(ctx.serializers().get(CLASS));
        // Signon has no lifecycle log. Seed visible baseline nodes once.
        if !self.seeded {
            for (_, entity) in ctx
                .entities()
                .iter()
                .filter(|(_, e)| e.active && e.class_name.as_ref() == CLASS)
            {
                if let Some(handle) = node_handle(entity.id()) {
                    self.observe(
                        ctx.tick(),
                        handle,
                        reader.geometry(entity),
                        reader.team(entity),
                    );
                }
            }
            self.seeded = true;
        }
        self.pending.clear();
        for change in ctx
            .entities()
            .entity_changes()
            .iter()
            .filter(|c| c.class_name.as_ref() == CLASS)
        {
            let Some(handle) = node_handle(change.id()) else {
                continue;
            };
            let signal = match change.kind {
                EntityChangeKind::Deleted => ZiplineChangeKind::Removed,
                EntityChangeKind::LeftPvs => ZiplineChangeKind::LeftPvs,
                _ => ZiplineChangeKind::Observed,
            };
            self.pending
                .entry(handle)
                .and_modify(|previous| {
                    if signal == ZiplineChangeKind::Removed
                        || *previous == ZiplineChangeKind::Observed
                    {
                        *previous = signal;
                    }
                })
                .or_insert(signal);
        }
        // Keyframes can delete and recreate the same full handle within one
        // tick. This API reports state after the tick, so an unchanged live
        // handle is neither removed nor newly observed. No count/time heuristic.
        let mut pending = std::mem::take(&mut self.pending);
        for (handle, signal) in pending.drain() {
            match ctx.entities().get_by_handle(handle) {
                Some(entity) if entity.active => self.observe(
                    ctx.tick(),
                    handle,
                    reader.geometry(entity),
                    reader.team(entity),
                ),
                Some(_) => self.hide(ctx.tick(), handle, ZiplineChangeKind::LeftPvs),
                None if matches!(
                    signal,
                    ZiplineChangeKind::Removed | ZiplineChangeKind::LeftPvs
                ) =>
                {
                    self.hide(ctx.tick(), handle, signal)
                }
                _ => {} // Missing evidence is not a deletion.
            }
        }
        self.pending = pending;
    }
}

fn node_handle(id: EntityId) -> Option<u32> {
    let index = u32::try_from(id.index).ok()?;
    let bits = crate::ENTITY_HANDLE_INDEX_MASK.count_ones();
    u32::try_from((u64::from(id.serial) << bits) | u64::from(index)).ok()
}

fn integer(value: Option<&FieldValue>) -> Option<i64> {
    match value? {
        FieldValue::I32(v) => Some(i64::from(*v)),
        FieldValue::U32(v) => Some(i64::from(*v)),
        FieldValue::I64(v) => Some(*v),
        FieldValue::U64(v) => i64::try_from(*v).ok(),
        _ => None,
    }
}

fn float(value: Option<&FieldValue>) -> Option<f32> {
    match value? {
        FieldValue::F32(v) if v.is_finite() => Some(*v),
        _ => None,
    }
}

struct Reader {
    serializer: Option<Serializer>,
    lane: Option<u64>,
    team: Option<u64>,
    cells: [Option<u64>; 3],
    offsets: [Option<u64>; 3],
    distance: Option<u64>,
    capturable: Option<u64>,
    connection_count: Option<u64>,
    connections: Vec<u64>,
}

impl Reader {
    fn new(serializer: Option<&Serializer>) -> Self {
        let key = |name: &str| serializer.and_then(|s| s.resolve_field_key(name));
        Self {
            serializer: serializer.cloned(),
            lane: key("m_iPrimaryLane"),
            team: key("m_iTeamNum"),
            cells: ["X", "Y", "Z"].map(|axis| key(&format!("{ORIGIN}.m_cell{axis}"))),
            offsets: ["X", "Y", "Z"].map(|axis| key(&format!("{ORIGIN}.m_vec{axis}"))),
            distance: key("m_flCumulativeDistance"),
            capturable: key("m_bCapturable"),
            connection_count: key("m_vecConnections"),
            connections: Vec::new(),
        }
    }

    fn refresh(&mut self, current: Option<&Serializer>) {
        let same = match (&self.serializer, current) {
            (Some(previous), Some(current)) => {
                previous.fields.len() == current.fields.len()
                    && previous
                        .fields
                        .iter()
                        .zip(&current.fields)
                        .all(|(a, b)| std::sync::Arc::ptr_eq(a, b))
            }
            (None, None) => true,
            _ => false,
        };
        if !same {
            *self = Self::new(current);
        }
    }

    fn team(&self, entity: &Entity) -> Option<i32> {
        i32::try_from(integer(entity.field_value(self.team))?).ok()
    }

    fn geometry(&mut self, entity: &Entity) -> ZiplineGeometry {
        ZiplineGeometry {
            lane: integer(entity.field_value(self.lane)).and_then(|v| i32::try_from(v).ok()),
            position: std::array::from_fn(|i| {
                let cell = i32::try_from(integer(entity.field_value(self.cells[i]))?).ok()?;
                let offset = float(entity.field_value(self.offsets[i]))?;
                Some(crate::cell_to_world(cell, offset))
            }),
            connections: self.connections(entity),
            cumulative_distance: float(entity.field_value(self.distance)),
            capturable: match entity.field_value(self.capturable) {
                Some(FieldValue::Bool(v)) => Some(*v),
                _ => None,
            },
        }
    }

    fn connections(&mut self, entity: &Entity) -> Option<Vec<u32>> {
        let count = usize::try_from(integer(entity.field_value(self.connection_count))?).ok()?;
        if count > self.connections.len() {
            let serializer = self.serializer.as_ref()?;
            serializer.resolve_field_key(&format!("m_vecConnections.{}", count - 1))?;
            for index in self.connections.len()..count {
                self.connections
                    .push(serializer.resolve_field_key(&format!("m_vecConnections.{index}"))?);
            }
        }
        self.connections[..count]
            .iter()
            .map(|&key| entity.get_handle(Some(key)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> ZiplineGeometry {
        ZiplineGeometry {
            lane: Some(4),
            position: [Some(0.0), Some(1.0), Some(2.0)],
            connections: Some(vec![123]),
            cumulative_distance: Some(10.0),
            capturable: Some(true),
        }
    }

    fn snapshot(history: &ZiplineHistory, tick: i32) -> Vec<(u32, Option<i32>, Option<f32>)> {
        let mut rows = Vec::new();
        history.visit_snapshots(&[tick], |_, node, team| {
            rows.push((node.node_handle, team, node.geometry.position[0]))
        });
        rows
    }

    #[test]
    fn unchanged_state_does_not_repeat_rows() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), Some(2));
        tracker.observe(2, 10, geometry(), Some(2));
        assert_eq!(
            (tracker.history.nodes.len(), tracker.history.changes.len()),
            (1, 1)
        );
    }

    #[test]
    fn ownership_changes_do_not_repeat_geometry() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), Some(4));
        tracker.observe(2, 10, geometry(), Some(2));
        assert_eq!(tracker.history.nodes.len(), 1);
        assert_eq!(
            tracker.history.changes[1].change,
            ZiplineChangeKind::OwnerChanged
        );
    }

    #[test]
    fn future_ownership_is_not_visible() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), Some(4));
        tracker.observe(20, 10, geometry(), Some(2));
        assert_eq!(snapshot(&tracker.history, 19)[0].1, Some(4));
    }

    #[test]
    fn changes_apply_at_their_exact_tick() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), Some(4));
        tracker.observe(20, 10, geometry(), Some(2));
        assert_eq!(snapshot(&tracker.history, 20)[0].1, Some(2));
    }

    #[test]
    fn future_nodes_are_not_visible() {
        let mut tracker = Tracker::default();
        tracker.observe(20, 10, geometry(), Some(2));
        assert!(snapshot(&tracker.history, 19).is_empty());
    }

    #[test]
    fn geometry_revisions_apply_only_from_their_tick() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), Some(2));
        let mut moved = geometry();
        moved.position[0] = Some(8.0);
        tracker.observe(20, 10, moved, Some(2));
        assert_eq!(snapshot(&tracker.history, 19)[0].2, Some(0.0));
        assert_eq!(snapshot(&tracker.history, 20)[0].2, Some(8.0));
    }

    #[test]
    fn deletion_ends_visibility() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), Some(2));
        tracker.hide(20, 10, ZiplineChangeKind::Removed);
        assert!(snapshot(&tracker.history, 20).is_empty());
    }

    #[test]
    fn pvs_loss_is_distinct_from_deletion() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), Some(2));
        tracker.hide(20, 10, ZiplineChangeKind::LeftPvs);
        assert_eq!(
            tracker.history.changes[1].change,
            ZiplineChangeKind::LeftPvs
        );
    }

    #[test]
    fn reactivation_restores_the_observed_owner() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), Some(2));
        tracker.hide(20, 10, ZiplineChangeKind::LeftPvs);
        tracker.observe(30, 10, geometry(), Some(3));
        assert_eq!(snapshot(&tracker.history, 30)[0].1, Some(3));
    }

    #[test]
    fn missing_observations_do_not_expire_nodes() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), Some(2));
        assert_eq!(snapshot(&tracker.history, 100_000).len(), 1);
    }

    #[test]
    fn slot_reuse_keeps_full_handles_separate() {
        let old = node_handle(EntityId::new(10, 1)).unwrap();
        let new = node_handle(EntityId::new(10, 2)).unwrap();
        let mut tracker = Tracker::default();
        tracker.observe(1, old, geometry(), Some(2));
        tracker.hide(2, old, ZiplineChangeKind::Removed);
        tracker.observe(3, new, geometry(), Some(3));
        assert_eq!(snapshot(&tracker.history, 3)[0].0, new);
    }

    #[test]
    fn missing_coordinates_are_not_zero() {
        let mut reader = Reader::new(None);
        let entity = Entity::from_fields(10, 1, 1, CLASS, true, Default::default()).unwrap();
        assert_eq!(reader.geometry(&entity).position, [None; 3]);
    }

    #[test]
    fn unknown_owner_stays_unknown() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), None);
        assert_eq!(snapshot(&tracker.history, 1)[0].1, None);
    }

    #[test]
    fn repeated_deletion_does_not_repeat_changes() {
        let mut tracker = Tracker::default();
        tracker.observe(1, 10, geometry(), Some(2));
        tracker.hide(2, 10, ZiplineChangeKind::Removed);
        tracker.hide(3, 10, ZiplineChangeKind::Removed);
        assert_eq!(tracker.history.changes.len(), 2);
    }

    #[test]
    fn missing_connection_entry_keeps_the_vector_unknown() {
        let mut reader = Reader::new(None);
        reader.connection_count = Some(1);
        reader.connections = vec![2, 3];
        let mut entity = Entity::from_fields(10, 1, 1, CLASS, true, Default::default()).unwrap();
        entity.fields.insert(1, FieldValue::U32(2));
        entity.fields.insert(2, FieldValue::U32(123));
        assert_eq!(reader.connections(&entity), None);
    }

    #[test]
    fn recorded_empty_connections_are_an_empty_list() {
        let mut reader = Reader::new(None);
        reader.connection_count = Some(1);
        let mut entity = Entity::from_fields(10, 1, 1, CLASS, true, Default::default()).unwrap();
        entity.fields.insert(1, FieldValue::U32(0));
        assert_eq!(reader.connections(&entity), Some(vec![]));
    }

    #[test]
    fn shrinking_connection_vector_drops_old_targets() {
        let mut reader = Reader::new(None);
        reader.connection_count = Some(1);
        reader.connections = vec![2, 3];
        let mut entity = Entity::from_fields(10, 1, 1, CLASS, true, Default::default()).unwrap();
        entity.fields.insert(1, FieldValue::U32(1));
        entity.fields.insert(2, FieldValue::U32(123));
        entity.fields.insert(3, FieldValue::U32(456));
        assert_eq!(reader.connections(&entity), Some(vec![123]));
    }
}
