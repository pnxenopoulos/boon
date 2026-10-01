"""Test sampled snapshots from `demo.snapshots(...)`.

The selected rows must equal the same rows from a complete per-tick frame. The
test creates only the selected ticks to use less time and memory. The tests skip
when no `.dem` fixture is present.
"""

import threading

import polars as pl
import pytest
from boon import Demo
from conftest import _require_demo_fixture
from polars.testing import assert_frame_equal


@pytest.fixture(scope="module")
def demo() -> Demo:
    return Demo(str(_require_demo_fixture()), preload=False)


def test_specific_ticks_match_full_frame(demo: Demo) -> None:
    full = demo.player_ticks
    some = sorted(full["tick"].unique().to_list())[100:103]
    snap = demo.snapshots(ticks=some)
    assert isinstance(snap, pl.DataFrame)
    expected = full.filter(pl.col("tick").is_in(some))
    assert snap.sort(["tick", "hero_id"]).equals(expected.sort(["tick", "hero_id"]))


def test_barriers_match_seeks_and_segmented_passes(demo: Demo, monkeypatch) -> None:
    full = demo.player_ticks
    populated = full.filter(pl.col("barrier") > 0)["tick"].unique().sort().to_list()
    if not populated:
        pytest.skip("fixture has no recorded barriers")
    # Include ticks throughout the match, rather than only the opening keyframe.
    ticks = populated[:: max(1, len(populated) // 8)][:8]
    expected = full.filter(pl.col("tick").is_in(ticks)).sort(
        ["tick", "steam_id", "hero_id"]
    )
    direct = Demo(str(_require_demo_fixture()), preload=False)
    for tick in ticks:
        snapshot = direct.snapshots(ticks=tick)
        assert isinstance(snapshot, pl.DataFrame)
        assert_frame_equal(
            snapshot.sort(["steam_id", "hero_id"]),
            expected.filter(pl.col("tick") == tick),
        )
    for segments in (1, 4):
        monkeypatch.setenv("BOON_TICK_SEGMENTS", str(segments))
        # A window selects the full-pass path, even for a short fixture.
        sampled = Demo(str(_require_demo_fixture()), preload=False).snapshots(
            start_tick=ticks[0], end_tick=ticks[-1]
        )
        assert isinstance(sampled, pl.DataFrame)
        assert_frame_equal(
            sampled.filter(pl.col("tick").is_in(ticks)).sort(
                ["tick", "steam_id", "hero_id"]
            ),
            expected,
        )


def test_player_positions_match_snapshot_columns(demo: Demo) -> None:
    ticks = sorted(demo.player_ticks["tick"].unique().to_list())[100:103]
    frame = demo.snapshots(ticks=ticks)
    assert isinstance(frame, pl.DataFrame)
    expected = frame.select("tick", "hero_id", "x", "y")
    positions = demo._player_positions(ticks)
    assert positions.sort(["tick", "hero_id"]).equals(
        expected.sort(["tick", "hero_id"])
    )


def test_single_tick_matches_full_frame(demo: Demo) -> None:
    full = demo.player_ticks
    t = sorted(full["tick"].unique().to_list())[500]
    snap = demo.snapshots(ticks=t)
    assert isinstance(snap, pl.DataFrame)
    expected = full.filter(pl.col("tick") == t)
    assert snap.sort("hero_id").equals(expected.sort("hero_id"))


def test_window_matches_full_frame(demo: Demo) -> None:
    full = demo.player_ticks
    snap = demo.snapshots(start_tick=10000, end_tick=11000)
    assert isinstance(snap, pl.DataFrame)
    expected = full.filter((pl.col("tick") >= 10000) & (pl.col("tick") <= 11000))
    assert snap.sort(["tick", "hero_id"]).equals(expected.sort(["tick", "hero_id"]))


def test_stride_downsamples_to_subset(demo: Demo) -> None:
    full = demo.player_ticks
    snap = demo.snapshots(every=640)
    assert isinstance(snap, pl.DataFrame)
    assert 0 < snap.height < full.height
    # Every sampled row is a real row from the full frame.
    expected = full.filter(pl.col("tick").is_in(snap["tick"].unique().to_list()))
    assert snap.sort(["tick", "hero_id"]).equals(expected.sort(["tick", "hero_id"]))


def test_events_align_to_event_ticks(demo: Demo) -> None:
    snap = demo.snapshots(events="kills")
    assert isinstance(snap, pl.DataFrame)
    kill_ticks = set(demo.kills["tick"].to_list())
    assert set(snap["tick"].unique().to_list()) <= kill_ticks


def test_multiple_events_align_to_union_of_event_ticks(demo: Demo) -> None:
    snap = demo.snapshots(events=["kills", "damage"])
    assert isinstance(snap, pl.DataFrame)
    event_ticks = set(demo.kills["tick"].to_list())
    event_ticks.update(demo.damage["tick"].to_list())
    assert set(snap["tick"].unique().to_list()) <= event_ticks


def test_message_only_event_ticks_match_loaded_datasets() -> None:
    events = [
        "kills",
        "damage",
        "flex_slots",
        "abilities",
        "item_purchases",
        "chat",
    ]
    direct_demo = Demo(str(_require_demo_fixture()), preload=False)
    direct = direct_demo.snapshots(events=events)
    assert isinstance(direct, pl.DataFrame)

    loaded_demo = Demo(str(_require_demo_fixture()), preload=False)
    loaded_demo.load(*events)
    loaded = loaded_demo.snapshots(events=events)
    assert isinstance(loaded, pl.DataFrame)

    keys = ["tick", "hero_id"]
    assert direct.sort(keys).equals(loaded.sort(keys))


def test_single_dataset_returns_frame(demo: Demo) -> None:
    out = demo.snapshots("world_ticks", every=640)
    assert isinstance(out, pl.DataFrame)


def test_multiple_datasets_return_dict(demo: Demo) -> None:
    out = demo.snapshots(["player_ticks", "world_ticks", "troopers"], every=640)
    assert isinstance(out, dict)
    assert set(out.keys()) == {"player_ticks", "world_ticks", "troopers"}


def test_validation(demo: Demo) -> None:
    with pytest.raises(ValueError):
        demo.snapshots()  # no selector at all
    with pytest.raises(ValueError):
        demo.snapshots(every=64, seconds=1.0)  # mutually exclusive
    with pytest.raises(ValueError):
        demo.snapshots("not_a_dataset", every=64)
    with pytest.raises(ValueError):
        demo.snapshots(every=0)  # must be >= 1


@pytest.mark.parametrize("cached", [False, True])
@pytest.mark.parametrize("dataset", ["not_a_dataset", "healing", "barriers"])
def test_event_selection_rejects_unknown_names_consistently(
    cached: bool, dataset: str
) -> None:
    parsed = Demo(str(_require_demo_fixture()), preload=False)
    if cached:
        parsed.load("kills")
    with pytest.raises(ValueError, match="Unknown dataset"):
        parsed.snapshots(events=["kills", dataset])


def test_duplicate_snapshot_names_preserve_return_shape(demo: Demo) -> None:
    expected = demo.snapshots("world_ticks", ticks=1000)
    assert isinstance(expected, pl.DataFrame)
    repeated = demo.snapshots(["world_ticks", "world_ticks"], ticks=1000)
    assert isinstance(repeated, dict)
    assert list(repeated) == ["world_ticks"]
    assert repeated["world_ticks"].equals(expected)


def test_snapshots_release_gil() -> None:
    parsed = Demo(str(_require_demo_fixture()), preload=False)
    ready = threading.Event()
    stop = threading.Event()
    progress = [0]

    def worker() -> None:
        ready.set()
        while not stop.is_set():
            progress[0] += 1

    thread = threading.Thread(target=worker)
    thread.start()
    assert ready.wait(timeout=5)
    before = progress[0]
    try:
        parsed.snapshots(
            ["player_ticks", "world_ticks", "troopers"],
            every=640,
        )
        after = progress[0]
    finally:
        stop.set()
        thread.join(timeout=5)

    assert not thread.is_alive()
    assert after > before
