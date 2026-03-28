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
    "item_purchases",
    "kills",
    "mid_boss",
    "neutrals",
    "objectives",
    "player_ticks",
    "stat_modifier_events",
    "troopers",
    "urn",
    "world_ticks",
]

STREET_BRAWL_DATASETS = ["street_brawl_ticks", "street_brawl_rounds"]


def _demo_files() -> list[Path]:
    """Return all .dem files in the fixtures directory."""
    if not FIXTURES_DIR.is_dir():
        return []
    return sorted(FIXTURES_DIR.glob("*.dem"))


# Session-scoped cache: filename → Demo instance (parsed once, reused everywhere)
_demo_cache: dict[str, Demo] = {}


def get_demo(path: Path) -> Demo:
    """Get or create a fully-loaded Demo instance, cached for the session."""
    key = path.name
    if key not in _demo_cache:
        d = Demo(str(path))
        datasets = list(ALL_DATASETS)
        if d.game_mode == 4:
            datasets.extend(STREET_BRAWL_DATASETS)
        d.load(*datasets)
        _demo_cache[key] = d
    return _demo_cache[key]


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
    return get_demo(request.param)


def _require_demo_fixture() -> Path:
    """Return the first fixture path, or skip the test if none available."""
    dems = _demo_files()
    if not dems:
        pytest.skip("No demo fixtures available")
    return dems[0]
