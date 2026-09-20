"""Healing intervals validated against recorded match statistics."""

import polars as pl
import pytest
from boon import Demo

from conftest import FIXTURES_DIR


@pytest.fixture(params=[
    ("100655353.dem", 394, 226267, 186088, 2160, 2338),
    ("103129247.dem", 219, 127628, 159574, 1800, 1975),
    ("70537442.dem", 69, 27758, 18984, 360, 480),
])
def recorded_summary(request):
    filename, *expected = request.param
    path = FIXTURES_DIR / filename
    if not path.exists():
        pytest.skip(f"{filename} fixture not available")
    return Demo(str(path)).summary(), expected


def test_healing_and_regeneration_keep_separate_recorded_totals(recorded_summary) -> None:
    summary, (count, total, regen_total, final_start, final_end) = recorded_summary
    intervals = summary["healing"]
    healing = intervals.filter(pl.col("stat_type") == "healing")
    regen = intervals.filter(pl.col("stat_type") == "regen")
    assert healing.height == count
    assert healing["amount"].sum() == total
    assert regen["amount"].sum() == regen_total
    assert set(intervals["stat_type"]) == {"healing", "regen"}
    assert intervals["amount"].min() > 0
    assert intervals["interval_end_s"].is_sorted()
    assert intervals.filter(pl.col("interval_end_s") == final_end)["interval_start_s"].unique().to_list() == [final_start]
    assert intervals.select(pl.struct(pl.all()).is_duplicated().any()).item() is False
    assert intervals.schema == {
        "interval_start_s": pl.UInt32, "interval_end_s": pl.UInt32,
        "healer_player_slot": pl.UInt32, "healer_hero_id": pl.UInt32,
        "target_player_slot": pl.UInt32, "target_hero_id": pl.UInt32,
        "source_name": pl.String, "stat_type": pl.String, "amount": pl.UInt32,
    }


def test_sparse_healing_uses_matrix_boundaries_not_previous_heal() -> None:
    path = FIXTURES_DIR / "100655353.dem"
    if not path.exists():
        pytest.skip("100655353.dem fixture not available")
    intervals = Demo(str(path)).summary()["healing"]
    rescue = intervals.filter(
        (pl.col("source_name") == "upgrade_rescue_beam")
        & (pl.col("healer_player_slot") == 10)
        & (pl.col("target_player_slot") == 10)
    )
    assert rescue.select("interval_start_s", "interval_end_s", "amount").rows() == [
        (1080, 1260, 337), (1980, 2160, 428), (2160, 2338, 713),
    ]
    teammate = intervals.filter(
        (pl.col("source_name") == "upgrade_rescue_beam")
        & (pl.col("healer_player_slot") == 1)
        & (pl.col("target_player_slot") == 6)
    )
    assert teammate.select("healer_hero_id", "target_hero_id", "amount").rows() == [
        (11, 66, 455), (11, 66, 518),
    ]


@pytest.mark.parametrize("first_access", ["property", "load", "summary"])
def test_healing_dataset_shares_summary_in_any_access_order(first_access: str) -> None:
    path = FIXTURES_DIR / "100655353.dem"
    if not path.exists():
        pytest.skip("100655353.dem fixture not available")
    demo = Demo(str(path))
    if first_access == "load":
        demo.load("healing")
    expected = demo.summary()["healing"] if first_access == "summary" else demo.healing
    assert "healing" in Demo.available_datasets()
    assert expected.equals(demo.healing)
    assert expected.equals(demo.summary()["healing"])
    demo.load("healing", "healing", "damage")
    assert expected.equals(demo.healing)
    assert demo.damage.height == 77378
    with pytest.raises(ValueError, match="interval statistics"):
        demo.snapshots(events="healing")
