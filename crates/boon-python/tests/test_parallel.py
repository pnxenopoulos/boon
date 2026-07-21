"""Parallel snapshot decode must exactly match the serial decode.

`player_ticks` / `world_ticks` / `troopers` are per-tick full-snapshot datasets.
Each entity class is re-keyframed at every `DEM_FullPacket`, so the demo can be
split at those keyframes and each segment decoded on its own thread — byte-for-
byte identical to a serial pass. Loading several together runs a single parallel
pass. `BOON_TICK_SEGMENTS` forces the segment count (`1` = serial). Skips when no
`.dem` fixture is present.
"""

from pathlib import Path

import pytest
from boon import Demo

from conftest import FIXTURES_DIR

SNAPSHOT_DATASETS = ["player_ticks", "world_ticks", "troopers"]


def _fixture() -> str:
    dems = sorted(FIXTURES_DIR.glob("*.dem")) if FIXTURES_DIR.is_dir() else []
    if not dems:
        pytest.skip("No demo fixtures available")
    return str(dems[0])


@pytest.mark.parametrize("dataset", SNAPSHOT_DATASETS)
def test_parallel_matches_serial(
    dataset: str, monkeypatch: pytest.MonkeyPatch
) -> None:
    demo_path = _fixture()

    monkeypatch.setenv("BOON_TICK_SEGMENTS", "1")
    serial = getattr(Demo(demo_path), dataset)

    monkeypatch.setenv("BOON_TICK_SEGMENTS", "4")
    parallel = getattr(Demo(demo_path), dataset)

    assert serial.shape == parallel.shape
    assert serial.columns == parallel.columns
    assert serial.equals(parallel)


def test_batched_snapshot_load_is_one_pass_and_exact(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    demo_path = _fixture()

    # Reference: original serial run_to_end_filtered. Requesting a non-snapshot
    # dataset (kills) forces the serial pass rather than the parallel one.
    ref = Demo(demo_path)
    ref.load(*SNAPSHOT_DATASETS, "kills")

    # Parallel: a single keyframe-segmented pass collects all three snapshots.
    monkeypatch.setenv("BOON_TICK_SEGMENTS", "4")
    par = Demo(demo_path)
    par.load(*SNAPSHOT_DATASETS)

    for ds in SNAPSHOT_DATASETS:
        assert getattr(ref, ds).equals(getattr(par, ds)), ds
