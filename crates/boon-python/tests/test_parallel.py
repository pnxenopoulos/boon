"""Parallel snapshot decode must exactly match the serial decode.

`player_ticks` / `world_ticks` / `troopers` are per-tick full-snapshot datasets.
Each entity class is re-keyframed at every `DEM_FullPacket`, so the demo can be
split at those keyframes and each segment decoded on its own thread — byte-for-
byte identical to a serial pass. Loading several together runs a single parallel
pass. `BOON_TICK_SEGMENTS` forces the segment count (`1` = serial). Skips when no
`.dem` fixture is present.
"""

import polars as pl
import pytest
from boon import Demo
from conftest import _require_demo_fixture

SNAPSHOT_DATASETS = ["player_ticks", "world_ticks", "troopers"]


@pytest.fixture(scope="module")
def serial_frames(demo: Demo) -> dict[str, pl.DataFrame]:
    """Reuse the session's serial reference; parallel checks use fresh parsers."""
    return {name: getattr(demo, name) for name in (*SNAPSHOT_DATASETS, "kills")}


@pytest.mark.parametrize("dataset", SNAPSHOT_DATASETS)
def test_parallel_matches_serial(
    dataset: str,
    serial_frames: dict[str, pl.DataFrame],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    demo_path = str(_require_demo_fixture())
    serial = serial_frames[dataset]

    monkeypatch.setenv("BOON_TICK_SEGMENTS", "4")
    parallel = getattr(Demo(demo_path, preload=False), dataset)

    assert serial.shape == parallel.shape
    assert serial.columns == parallel.columns
    assert serial.equals(parallel)


def test_mixed_load_keeps_snapshots_parallel_and_exact(
    serial_frames: dict[str, pl.DataFrame],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    demo_path = str(_require_demo_fixture())

    # A mixed request must keep the snapshots on their parallel segmented path
    # while kills uses the filtered event/entity pass.
    monkeypatch.setenv("BOON_TICK_SEGMENTS", "4")
    mixed = Demo(demo_path, preload=False)
    mixed.load(*SNAPSHOT_DATASETS, "kills")

    for ds in SNAPSHOT_DATASETS:
        assert serial_frames[ds].equals(getattr(mixed, ds)), ds
    assert serial_frames["kills"].equals(mixed.kills)
