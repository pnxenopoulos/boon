"""Shared pytest fixtures for boon tests."""

from pathlib import Path

import pytest
from boon import Demo

FIXTURES_DIR = Path(__file__).parent / "fixtures"

ALL_DATASETS = [
    "abilities",
    "ability_upgrades",
    "active_modifiers",
    "boss_kills",
    "chat",
    "damage",
    "flex_slots",
    "kills",
    "mid_boss",
    "neutrals",
    "objectives",
    "player_ticks",
    "purchases",
    "respawns",
    "shop_events",
    "stat_modifiers",
    "troopers",
    "world_ticks",
]


def _demo_files() -> list[Path]:
    """Return all .dem files in the fixtures directory."""
    if not FIXTURES_DIR.is_dir():
        return []
    return sorted(FIXTURES_DIR.glob("*.dem"))


@pytest.fixture(scope="session")
def demo_paths() -> list[Path]:
    """List of all .dem fixture file paths."""
    return _demo_files()


@pytest.fixture(scope="session", params=_demo_files(), ids=lambda p: p.name)
def demo(request: pytest.FixtureRequest) -> Demo:
    """Yield a fully-loaded Demo instance for each fixture file.

    All datasets are loaded eagerly in a single parse pass so that
    individual tests only check cached DataFrames.
    """
    d = Demo(str(request.param))
    d.load(*ALL_DATASETS)
    return d


def _require_demo_fixture() -> Path:
    """Return the first fixture path, or skip the test if none available."""
    dems = _demo_files()
    if not dems:
        pytest.skip("No demo fixtures available")
    return dems[0]
