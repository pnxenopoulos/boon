"""Ammo reconstruction, uncertainty, and optional catalog use."""

from typing import cast

import polars as pl
import pytest
from boon import Demo, HeroStat
from boon.hero_stats import StatResult
from boon.snapshots import snapshots


class AmmoDemo:
    def __init__(self):
        self.calls = []
        self.frame = pl.DataFrame(
            {
                "tick": [7] * 6,
                "player_slot": list(range(6)),
                "steam_id": [
                    76561197999389679,
                    76561197999389680,
                    None,
                    None,
                    None,
                    None,
                ],
                "hero_id": [99] * 6,
                "ammo_fraction": [0.25, 1.2, None, 0.91, 1.0, 0.5],
            },
            schema_overrides={
                "tick": pl.Int32,
                "player_slot": pl.UInt32,
                "steam_id": pl.UInt64,
                "ammo_fraction": pl.Float32,
            },
        )

    def _snapshots(self, datasets=None, **kwargs):
        self.calls.append(("snapshot", kwargs))
        if isinstance(datasets, list):
            return {
                "player_ticks": self.frame,
                "world_ticks": pl.DataFrame({"tick": [7]}),
            }
        return self.frame

    def calculate_hero_stats(self, **kwargs):
        self.calls.append(("capacity", kwargs))
        values = (
            self.frame.select("tick", "player_slot", "hero_id")
            .with_columns(
                pl.Series("value", [10, 10, 15, 100, None, 10], dtype=pl.Float64),
                pl.Series(
                    "status",
                    [
                        "calculated",
                        "partial",
                        "calculated",
                        "partial",
                        "unresolved",
                        "calculated",
                    ],
                ),
                pl.Series(
                    "diagnostic",
                    [
                        None,
                        "unknown effect",
                        None,
                        "unknown effect",
                        "missing scaling",
                        None,
                    ],
                ),
            )
            .reverse()
        )
        return StatResult(values, pl.DataFrame(), {})

    def player_states(self, **kwargs):
        self.calls.append(("states", kwargs))
        return (
            self.frame.select("tick", "player_slot", "hero_id")
            .with_columns(
                pl.Series(
                    "states",
                    [
                        [],
                        ["INFINITE_CLIP"],
                        None,
                        ["INFINITE_CLIP"],
                        ["INFINITE_CLIP"],
                        [],
                    ],
                    dtype=pl.List(pl.String),
                ),
                pl.Series(
                    "unknown_states",
                    [[], [], None, [80], [], [90]],
                    dtype=pl.List(pl.UInt32),
                ),
            )
            .reverse()
        )


def test_rounds_capacity_and_unlimited_state_keep_identity_and_uncertainty():
    demo = AmmoDemo()
    frames = snapshots(
        cast(Demo, demo), ["player_ticks", "world_ticks"], ticks=7, data_version="1234"
    )
    assert isinstance(frames, dict)
    assert frames["world_ticks"].to_dict(as_series=False) == {"tick": [7]}
    frame = frames["player_ticks"].sort("player_slot")
    assert frame["steam_id"].to_list() == demo.frame["steam_id"].to_list()
    assert frame["ammo"].to_list() == [3, 12, None, 91, None, 5]
    assert frame["max_ammo"].to_list() == [10, 10, 15, 100, None, 10]
    assert frame["unlimited_ammo"].to_list() == [False, True, None, True, True, None]
    assert frame["ammo_status"].to_list() == [
        "calculated",
        "partial",
        "unresolved",
        "partial",
        "unresolved",
        "calculated",
    ]
    assert frame["ammo_diagnostic"].to_list() == [
        None,
        "unknown effect",
        "ammo fraction unavailable",
        "unknown effect",
        "missing scaling",
        None,
    ]
    assert frame.schema["ammo"] == frame.schema["max_ammo"] == pl.UInt32
    empty = AmmoDemo()
    empty.frame = empty.frame.clear()
    empty_frame = snapshots(cast(Demo, empty), ticks=7, data_version="1234")
    assert isinstance(empty_frame, pl.DataFrame)
    assert frame.schema == empty_frame.schema
    assert demo.calls[1:] == [
        (
            "capacity",
            {
                "ticks": [7],
                "data_version": "1234",
                "stats": [HeroStat.CLIP_SIZE],
                "strict": False,
            },
        ),
        ("states", {"ticks": [7], "data_version": "1234"}),
    ]


@pytest.mark.parametrize("empty", [False, True])
def test_raw_and_empty_snapshots_need_no_catalog(empty):
    demo = AmmoDemo()
    if empty:
        demo.frame = demo.frame.clear()
    frame = snapshots(cast(Demo, demo), ticks=7, data_version="1234" if empty else None)
    assert isinstance(frame, pl.DataFrame)
    assert len(demo.calls) == 1
    if empty:
        assert frame.is_empty()
        assert frame.schema["max_ammo"] == pl.UInt32
        assert frame.schema["unlimited_ammo"] == pl.Boolean
    else:
        assert frame is demo.frame
        assert "max_ammo" not in frame.columns


@pytest.mark.parametrize(
    "options",
    [{"data_version": ""}, {"data_version": "1234", "datasets": "world_ticks"}],
)
def test_invalid_ammo_requests_fail_before_parsing(options):
    demo = AmmoDemo()
    with pytest.raises(ValueError):
        snapshots(cast(Demo, demo), ticks=7, **options)
    assert not demo.calls
