"""Recorded barrier absorption, including replay regressions."""

from types import SimpleNamespace

import polars as pl
import pytest
from boon import Demo, barriers

from conftest import FIXTURES_DIR

COLUMNS = [
    "tick", "server_tick", "hero_id", "victim_entity_id", "attacker_hero_id",
    "ability_id", "absorbed", "remaining", "capacity", "is_secondary_stat",
]
DAMAGE_SCHEMA = {
    "tick": pl.Int32,
    "server_tick": pl.Int32,
    "victim_hero_id": pl.Int64,
    "victim_entity_id": pl.Int32,
    "attacker_hero_id": pl.Int64,
    "ability_id": pl.UInt32,
    "damage_absorbed": pl.Float32,
    "victim_shield_new": pl.Int32,
    "victim_shield_max": pl.Int32,
    "is_secondary_stat": pl.Boolean,
    "damage": pl.Int32,
}


def test_absorption_uses_messages_without_loading_pool_snapshots() -> None:
    # Fully blocked damage still counts. Multiple messages at the same tick
    # remain separate; optional fields and secondary-stat flags stay intact.
    damage = pl.DataFrame([
        (21, 121, 7, 92, 8, 100, 10.5, 30, 100, None, 0),
        (20, None, 7, 92, 8, 101, 4.0, None, None, False, 0),
        (20, 120, 7, 92, 8, 102, 2.0, 40, 100, True, 0),
        (22, 122, 0, 500, 8, 100, 8.0, 0, 100, False, 0),
        (23, 123, 7, 92, 8, 100, None, None, None, None, 20),
        (24, 124, 7, 92, 8, 100, 0.0, None, None, False, 20),
        (25, 125, 7, 92, 8, 100, -1.0, None, None, False, 20),
        (26, 126, 7, 92, 8, 100, float("inf"), None, None, False, 20),
        (27, 127, 7, 92, 8, 100, float("nan"), None, None, False, 20),
    ], schema=DAMAGE_SCHEMA, orient="row")
    loads = []
    demo = SimpleNamespace(damage=damage, load=lambda *names: loads.append(names))
    result = barriers.barriers(demo)
    assert loads == [("damage",)]
    assert result.columns == COLUMNS
    assert result.select("tick", "ability_id", "absorbed", "is_secondary_stat").rows() == [
        (20, 101, 4.0, False), (20, 102, 2.0, True), (21, 100, 10.5, None),
    ]
    assert result.row(0, named=True)["server_tick"] is None
    assert result.row(0, named=True)["remaining"] is None
    assert result.row(0, named=True)["capacity"] is None


def test_empty_absorption_retains_schema() -> None:
    damage = pl.DataFrame(schema=DAMAGE_SCHEMA)
    result = barriers.barriers(SimpleNamespace(damage=damage, load=lambda *args: None))
    assert result.is_empty()
    assert result.columns == COLUMNS
    assert result.schema["absorbed"] == pl.Float32
    assert result.schema["is_secondary_stat"] == pl.Boolean


@pytest.mark.parametrize("filename, count, total", [
    ("100655353.dem", 40, 1589.2372198104858),
    ("103129247.dem", 875, 25085.82179118693),
    ("70537442.dem", 128, 4617.860659122467),
])
def test_recorded_absorption_totals(filename: str, count: int, total: float) -> None:
    path = FIXTURES_DIR / filename
    if not path.exists():
        pytest.skip(f"{filename} fixture not available")
    demo = Demo(str(path))
    result = demo.barriers()
    assert result.columns == COLUMNS
    assert result.height == count
    assert result["absorbed"].cast(pl.Float64).sum() == pytest.approx(total, abs=0.01)
    assert result["tick"].is_sorted()
    assert result.equals(barriers.barriers(demo))
    if filename == "100655353.dem":
        assert result.filter(pl.col("hero_id") == 8)["tick"].to_list() == [
            151470, 152339, 152352,
        ]
        assert result.filter(pl.col("tick") == 100761).is_empty()
        # Integer scoreboard totals floor each recorded absorption separately.
        assert result["absorbed"].floor().sum() == 1571
