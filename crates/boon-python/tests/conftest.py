"""Shared pytest fixtures for boon tests."""

from pathlib import Path

import pytest
from boon import Demo

FIXTURES_DIR = Path(__file__).parent / "fixtures"
PRIMARY_DEMO = FIXTURES_DIR / "108575009.dem"

ALL_DATASETS = [
    "abilities",
    "ability_upgrades",
    "ability_ticks",
    "active_modifiers",
    "chat",
    "damage",
    "flex_slots",
    "item_purchases",
    "kills",
    "mid_boss",
    "neutrals",
    "breakables",
    "sinners_sacrifice",
    "objectives",
    "player_ticks",
    "rift",
    "stat_modifier_events",
    "troopers",
    "urn",
    "world_ticks",
]


def _demo_files() -> list[Path]:
    """Use the current-format replay for general API tests."""
    return [PRIMARY_DEMO] if PRIMARY_DEMO.is_file() else []


@pytest.fixture(scope="session")
def demo_paths() -> list[Path]:
    """List of all .dem fixture file paths."""
    return _demo_files()


@pytest.fixture(scope="session", params=_demo_files(), ids=lambda p: p.name)
def demo(request: pytest.FixtureRequest) -> Demo:
    """Share loaded replay data for read-only assertions.

    Tests that check parsing, caches, or decoder settings use fresh Demo instances.
    """
    parsed = Demo(str(request.param), preload=False)
    # Share one serial reference across seek and parallel comparisons.
    with pytest.MonkeyPatch.context() as patch:
        patch.setenv("BOON_TICK_SEGMENTS", "1")
        parsed.load(*ALL_DATASETS)
    return parsed


def _require_demo_fixture() -> Path:
    """Return the first fixture path, or skip the test if none available."""
    dems = _demo_files()
    if not dems:
        pytest.skip("No demo fixtures available")
    return dems[0]


@pytest.fixture(scope="session")
def name_catalog_cache(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """A verified catalog installation isolates replay tests from GitHub and user data."""
    import json

    from catalog_helpers import VERSION, release

    root = tmp_path_factory.mktemp("boon-data")
    records = json.loads(Path(__file__).with_name("name-catalogs.json").read_text())
    index, files = release(records=records)
    directory = root / VERSION
    directory.mkdir()
    entry = index["versions"][VERSION]
    for name, artifact in entry["artifacts"].items():
        (directory / name).write_bytes(files[artifact["url"]])
    (directory / ".install.json").write_text(json.dumps({"version": entry}))
    return root


@pytest.fixture(autouse=True)
def offline_catalogs(monkeypatch: pytest.MonkeyPatch, name_catalog_cache: Path) -> None:
    from boon import data

    monkeypatch.setattr(data, "BOON_DATA_DIR", name_catalog_cache)

    def offline(url):
        raise AssertionError(f"unexpected network request in test: {url}")

    monkeypatch.setattr(data, "_request", offline)
