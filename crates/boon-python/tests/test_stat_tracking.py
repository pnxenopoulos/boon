"""Selective native, baseline, and effective player stat tracking."""

import os

import polars as pl
import pytest
from boon import Demo

from conftest import FIXTURES_DIR


STAT_EFFECT_COLUMNS = {
    "tick",
    "hero_id",
    "event",
    "stat",
    "operation",
    "value",
    "source_type",
    "layer",
    "ability_id",
    "ability_name",
    "modifier_id",
    "modifier_name",
    "serial",
    "caster_hero_id",
    "provider_hero_id",
    "stacks",
    "duration",
    "active",
    "complete",
}


def _fixture() -> str:
    demos = sorted(FIXTURES_DIR.glob("*.dem")) if FIXTURES_DIR.is_dir() else []
    if not demos:
        pytest.skip("No demo fixtures available")
    return str(demos[0])


@pytest.fixture(scope="module")
def demo() -> Demo:
    return Demo(_fixture())


def test_stat_ticks_schema_and_single_tick_seek(demo: Demo) -> None:
    tick = int(demo.snapshots(every=640)["tick"][1])
    frame = demo.stat_ticks(
        ["bullet_resist", "spirit_resist", "fire_rate_bonus"],
        ticks=tick,
    )

    assert set(frame.columns) == {
        "tick",
        "hero_id",
        "bullet_resist_native",
        "bullet_resist_baseline",
        "bullet_resist_effective",
        "bullet_resist_complete",
        "spirit_resist_native",
        "spirit_resist_baseline",
        "spirit_resist_effective",
        "spirit_resist_complete",
        "fire_rate_bonus_native",
        "fire_rate_bonus_baseline",
        "fire_rate_bonus_effective",
        "fire_rate_bonus_complete",
    }
    assert set(frame["tick"].unique().to_list()) == {tick}


def test_stat_ticks_serial_parallel_parity(demo: Demo) -> None:
    previous = os.environ.get("BOON_TICK_SEGMENTS")
    try:
        os.environ["BOON_TICK_SEGMENTS"] = "1"
        serial = demo.stat_ticks(
            ["bullet_resist", "spirit_resist"],
            every=2048,
        )
        os.environ["BOON_TICK_SEGMENTS"] = "4"
        parallel = demo.stat_ticks(
            ["bullet_resist", "spirit_resist"],
            every=2048,
        )
    finally:
        if previous is None:
            os.environ.pop("BOON_TICK_SEGMENTS", None)
        else:
            os.environ["BOON_TICK_SEGMENTS"] = previous

    order = ["tick", "hero_id"]
    assert serial.sort(order).equals(parallel.sort(order))


def test_stat_effects_schema_and_filter(demo: Demo) -> None:
    frame = demo.stat_effects("bullet_resist")
    assert set(frame.columns) == STAT_EFFECT_COLUMNS
    if not frame.is_empty():
        assert frame["stat"].unique().to_list() == ["bullet_resist"]
        assert set(frame["layer"].unique().to_list()) <= {
            "baseline",
            "effective",
        }


def test_stat_tracking_validation(demo: Demo) -> None:
    with pytest.raises(ValueError):
        demo.stat_ticks([], every=64)
    with pytest.raises(ValueError):
        demo.stat_ticks("not_a_stat", every=64)
    with pytest.raises(ValueError):
        demo.stat_ticks("bullet_resist")
    with pytest.raises(ValueError):
        demo.stat_ticks("bullet_resist", every=64, seconds=1.0)
    with pytest.raises(ValueError):
        demo.stat_effects([])
    with pytest.raises(ValueError):
        demo.stat_effects("not_a_stat")
