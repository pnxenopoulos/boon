"""Shared pytest fixtures for boon tests."""

from pathlib import Path

import pytest
from boon import Demo

FIXTURES_DIR = Path(__file__).parent / "fixtures"


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
    """Yield a Demo instance for each fixture file."""
    return Demo(str(request.param))
