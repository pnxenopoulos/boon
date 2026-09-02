"""Test sampled snapshots from `demo.snapshots(...)`.

The selected rows must equal the same rows from a complete per-tick frame. The
test creates only the selected ticks to use less time and memory. The tests skip
when no `.dem` fixture is present.
"""

import threading

import polars as pl
import pytest
from boon import Demo

from conftest import FIXTURES_DIR


def _fixture() -> str:
    dems = sorted(FIXTURES_DIR.glob("*.dem")) if FIXTURES_DIR.is_dir() else []
    if not dems:
        pytest.skip("No demo fixtures available")
    return str(dems[0])


@pytest.fixture(scope="module")
def demo() -> Demo:
    return Demo(_fixture())


def test_specific_ticks_match_full_frame(demo: Demo) -> None:
    full = demo.player_ticks
    some = sorted(full["tick"].unique().to_list())[100:103]
    snap = demo.snapshots(ticks=some)
    expected = full.filter(pl.col("tick").is_in(some))
    assert snap.sort(["tick", "hero_id"]).equals(expected.sort(["tick", "hero_id"]))


def test_single_tick_matches_full_frame(demo: Demo) -> None:
    full = demo.player_ticks
    t = sorted(full["tick"].unique().to_list())[500]
    snap = demo.snapshots(ticks=t)
    expected = full.filter(pl.col("tick") == t)
    assert snap.sort("hero_id").equals(expected.sort("hero_id"))


def test_window_matches_full_frame(demo: Demo) -> None:
    full = demo.player_ticks
    snap = demo.snapshots(start_tick=10000, end_tick=11000)
    expected = full.filter((pl.col("tick") >= 10000) & (pl.col("tick") <= 11000))
    assert snap.sort(["tick", "hero_id"]).equals(expected.sort(["tick", "hero_id"]))


def test_stride_downsamples_to_subset(demo: Demo) -> None:
    full = demo.player_ticks
    snap = demo.snapshots(every=640)
    assert 0 < snap.height < full.height
    # Every sampled row is a real row from the full frame.
    assert snap.join(full, on=full.columns, how="semi").height == snap.height


def test_events_align_to_event_ticks(demo: Demo) -> None:
    snap = demo.snapshots(events="kills")
    kill_ticks = set(demo.kills["tick"].to_list())
    assert set(snap["tick"].unique().to_list()) <= kill_ticks


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


def test_snapshots_release_gil() -> None:
    parsed = Demo(_fixture())
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
