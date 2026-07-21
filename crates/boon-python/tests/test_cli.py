"""Tests for the Boon Typer CLI (``boon.cli``).

Commands that need a demo skip when no ``.dem`` fixture is present (fixtures are
gitignored), matching the rest of the suite. ``--version`` and ``datasets`` need
no demo and always run.
"""

import json
from pathlib import Path

import pytest
from typer.testing import CliRunner

from boon.cli import app

runner = CliRunner()

FIXTURES_DIR = Path(__file__).parent / "fixtures"


def _fixture() -> Path:
    """Return the first demo fixture, or skip if none is available."""
    dems = sorted(FIXTURES_DIR.glob("*.dem")) if FIXTURES_DIR.is_dir() else []
    if not dems:
        pytest.skip("No demo fixtures available")
    return dems[0]


def test_version() -> None:
    result = runner.invoke(app, ["--version"])
    assert result.exit_code == 0
    assert "boon" in result.stdout


def test_no_args_shows_help() -> None:
    result = runner.invoke(app, [])
    # Typer/Click prints usage and exits 2 when no subcommand is given.
    assert result.exit_code == 2
    assert "Usage" in result.output


def test_datasets_lists_kills() -> None:
    result = runner.invoke(app, ["datasets"])
    assert result.exit_code == 0
    assert "kills" in result.stdout


def test_info() -> None:
    result = runner.invoke(app, ["info", str(_fixture())])
    assert result.exit_code == 0
    assert "match_id" in result.stdout


def test_info_json() -> None:
    result = runner.invoke(app, ["info", str(_fixture()), "--json"])
    assert result.exit_code == 0
    data = json.loads(result.stdout)
    assert "match_id" in data and "map_name" in data


def test_players() -> None:
    result = runner.invoke(app, ["players", str(_fixture())])
    assert result.exit_code == 0
    assert "hero" in result.stdout


def test_show_dataset() -> None:
    result = runner.invoke(app, ["show", str(_fixture()), "kills", "--limit", "5"])
    assert result.exit_code == 0
    assert "cols" in result.output


def test_show_unknown_dataset() -> None:
    result = runner.invoke(app, ["show", str(_fixture()), "not_a_dataset"])
    assert result.exit_code == 1


def test_verify() -> None:
    result = runner.invoke(app, ["verify", str(_fixture())])
    assert result.exit_code == 0


def test_missing_file() -> None:
    result = runner.invoke(app, ["info", "does_not_exist.dem"])
    assert result.exit_code != 0
