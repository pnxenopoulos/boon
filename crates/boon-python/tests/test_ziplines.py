"""Recorded zipline state in 108575009.dem, with shared replay fixtures."""

import polars as pl
import pytest
from boon import Demo
from conftest import _require_demo_fixture
from polars.testing import assert_frame_equal

TICKS = [321, 187554]


@pytest.fixture(scope="module")
def zipline_demo():
    # Do not load the large shared combat/player-tick validation fixture.
    return Demo(str(_require_demo_fixture()), preload=False)


@pytest.fixture(scope="module")
def zipline_samples(zipline_demo):
    return zipline_demo.snapshots("ziplines", ticks=TICKS)


def test_initial_map_has_334_full_handles(zipline_demo):
    assert zipline_demo.zipline_nodes["node_handle"].n_unique() == 334


def test_map_lanes_come_from_recorded_nodes(zipline_demo):
    assert zipline_demo.zipline_nodes["lane"].unique().sort().to_list() == [1, 4, 6]


def test_map_has_205_capturable_nodes(zipline_samples):
    first = zipline_samples.filter(pl.col("tick") == TICKS[0])
    assert first["capturable"].sum() == 205


def test_late_ownership_matches_independent_entity_inspection(zipline_samples):
    late = zipline_samples.filter(pl.col("tick") == TICKS[1])
    assert dict(late.group_by("team_num").len().iter_rows()) == {0: 1, 2: 235, 3: 98}


def test_98_nodes_change_team_between_reference_ticks(zipline_samples):
    early = zipline_samples.filter(pl.col("tick") == TICKS[0])
    late = zipline_samples.filter(pl.col("tick") == TICKS[1])
    comparison = early.join(late, on="node_handle", suffix="_late")
    assert comparison.filter(pl.col("team_num") != pl.col("team_num_late")).height == 98


def test_centre_route_node_has_recorded_world_position(zipline_samples):
    node = zipline_samples.filter(
        (pl.col("tick") == TICKS[0]) & (pl.col("x") == 0) & (pl.col("y") == 0)
    )
    assert node.select("lane", "z", "cumulative_distance").rows() == [
        (4, 896.0, pytest.approx(12002.146, abs=0.002))
    ]


def test_ownership_changes_do_not_duplicate_geometry(zipline_demo):
    nodes = zipline_demo.zipline_nodes
    assert nodes.height == nodes["node_handle"].n_unique()


def test_connection_handles_resolve_to_the_same_recorded_map(zipline_demo):
    nodes = zipline_demo.zipline_nodes
    targets = set(nodes["connections"].explode().drop_nulls())
    assert targets <= set(nodes["node_handle"])


def test_handle_columns_keep_uint32_schema(zipline_demo):
    assert zipline_demo.zipline_nodes.schema["connections"] == pl.List(pl.UInt32)
    assert zipline_demo.zipline_changes.schema["node_handle"] == pl.UInt32


@pytest.mark.parametrize("tick", TICKS)
def test_snapshot_ownership_matches_the_serial_change_log(
    zipline_demo, zipline_samples, tick
):
    latest = (
        zipline_demo.zipline_changes.filter(pl.col("tick") <= tick)
        .group_by("node_handle", maintain_order=True)
        .last()
        .filter(pl.col("change").is_in(["observed", "owner_changed"]))
        .select("node_handle", "team_num")
        .sort("node_handle")
    )
    actual = zipline_samples.filter(pl.col("tick") == tick).select(latest.columns)
    assert_frame_equal(actual, latest)


@pytest.mark.parametrize("segments", ["1", "4"])
def test_window_sampling_matches_exact_snapshots(
    zipline_demo, zipline_samples, monkeypatch, segments
):
    monkeypatch.setenv("BOON_TICK_SEGMENTS", segments)
    actual = zipline_demo.snapshots(
        "ziplines", ticks=TICKS, start_tick=321, end_tick=187554
    )
    assert_frame_equal(actual, zipline_samples)


def test_mixed_snapshot_request_preserves_zipline_rows(zipline_demo, zipline_samples):
    frames = zipline_demo.snapshots(["world_ticks", "ziplines"], ticks=TICKS)
    assert_frame_equal(frames["ziplines"], zipline_samples)


def test_seconds_selector_samples_only_the_requested_window(zipline_demo):
    rows = zipline_demo.snapshots(
        "ziplines", seconds=5, start_tick=187000, end_tick=187700
    )
    assert rows["tick"].unique().sort().to_list() == [187201, 187521]


def test_event_selector_uses_zipline_change_ticks(zipline_demo):
    selected = zipline_demo.zipline_changes["tick"].unique().sort().to_list()[:1]
    rows = zipline_demo.snapshots(
        "ziplines", events="zipline_changes", end_tick=selected[0]
    )
    assert rows["tick"].unique().to_list() == selected


def test_absent_tick_returns_a_typed_empty_frame(zipline_demo):
    rows = zipline_demo.snapshots("ziplines", ticks=zipline_demo.total_ticks + 1000)
    assert rows.is_empty()
    assert rows.schema["connections"] == pl.List(pl.UInt32)


def test_zipline_tables_are_available(zipline_demo):
    assert {"zipline_nodes", "zipline_changes"} <= set(
        zipline_demo.available_datasets()
    )


def test_changes_keep_only_the_final_state_of_each_node_tick(zipline_demo):
    groups = zipline_demo.zipline_changes.group_by("tick", "node_handle").len()
    assert groups["len"].max() == 1


def test_player_snapshot_path_works_with_zipline_history(zipline_demo):
    frames = zipline_demo.snapshots(["player_ticks", "ziplines"], ticks=TICKS)
    assert frames["player_ticks"].filter(pl.col("tick") == 187554).height == 12
