"""Viewer-verified scoreboard and fight state for match 108575009."""

import gzip
import json
from pathlib import Path

import polars as pl
import pytest
from boon import Demo, data
from conftest import FIXTURES_DIR

FIXTURE_PATH = FIXTURES_DIR / "108575009.dem"
TICK = 187554
MCGINNIS = 76561198037652386
KELVIN = 76561198055516822
PAIGE = 76561198295494515
SCOREBOARD = json.loads(
    Path(__file__).with_name("108575009-scoreboard.json").read_text()
)


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("108575009.dem fixture not available")
    return Demo(str(FIXTURE_PATH), preload=False)


@pytest.fixture(scope="module")
def fight(demo: Demo) -> pl.DataFrame:
    frame = demo.snapshots(ticks=TICK)
    assert isinstance(frame, pl.DataFrame)
    return frame


@pytest.fixture(scope="module")
def final_players(demo: Demo) -> pl.DataFrame:
    # Pawns can disappear after death. Retain each player's last recorded row,
    # including the post-game interval, rather than require a pawn at game over.
    frame = demo.snapshots(every=64)
    assert isinstance(frame, pl.DataFrame)
    return frame.sort("tick").group_by("steam_id").last()


@pytest.fixture(scope="module")
def final_summary(demo: Demo) -> pl.DataFrame:
    return (
        demo.summary()["snapshots"].sort("snapshot_time_s").group_by("steam_id").last()
    )


@pytest.fixture(scope="module")
def catalog_dir(tmp_path_factory: pytest.TempPathFactory) -> Path:
    directory = tmp_path_factory.mktemp("108575009-catalog")
    source = Path(__file__).parent / "catalogs" / "108575009"
    for name in ("heroes", "abilities", "modifiers", "misc"):
        (directory / f"{name}.json").write_bytes(
            gzip.decompress((source / f"{name}.json.gz").read_bytes())
        )
    return directory


@pytest.fixture
def catalog(monkeypatch: pytest.MonkeyPatch, catalog_dir: Path) -> None:
    def installed(version: str) -> Path:
        assert version == "6712"
        return catalog_dir

    monkeypatch.setattr(data, "update", installed)


def assert_displayed_total(actual: int, displayed: str) -> None:
    # The viewer rounds large totals to thousands or tenths of a thousand.
    value = displayed.removesuffix("k")
    precision = 100 if "." in value else 1000
    assert actual == pytest.approx(float(value) * 1000, rel=0, abs=precision / 2)


@pytest.mark.parametrize("expected", SCOREBOARD, ids=lambda row: row["hero"])
def test_final_scoreboard(demo, final_players, final_summary, expected):
    selection = pl.col("steam_id") == expected["steam_id"]
    roster = demo.players.filter(selection).row(0, named=True)
    assert (roster["hero_id"], roster["team_num"]) == (
        expected["hero_id"],
        expected["team_num"],
    )
    if expected["player_name"] is not None:
        assert roster["player_name"] == expected["player_name"]
    live = final_players.filter(selection).row(0, named=True)
    summary = final_summary.filter(selection).row(0, named=True)
    # Post-match interval counters can count kills/deaths differently from the
    # scoreboard. The live controller counters match the viewer's final K/D/A.
    for name in ("kills", "deaths", "assists"):
        assert live[name] == expected[name]
    assert_displayed_total(summary["net_worth"], expected["souls"])
    assert_displayed_total(summary["player_damage"], expected["player_damage"])
    assert_displayed_total(live["objective_damage"], expected["objective_damage"])
    # The supplied Healing display includes barrier absorption. Keep the
    # recorded components separate in the API and combine only for this check.
    assert_displayed_total(
        summary["player_healing"] + summary["barrier_absorption"],
        expected["healing_and_barriers"],
    )


def test_fight_clock_health_and_rejuvenator(demo: Demo, fight: pl.DataFrame) -> None:
    assert demo.match_id == 108575009
    assert demo.tick_to_match_clock(TICK) == "48:21"
    assert fight.height == fight["steam_id"].n_unique() == 12
    assert "player_slot" not in fight.columns
    assert fight.filter(pl.col("steam_id") == KELVIN).select(
        "health", "max_health"
    ).rows() == [(5039, 5039)]
    mcginnis = fight.filter(pl.col("steam_id") == MCGINNIS)
    assert mcginnis.select("health", "max_health").rows() == [(1132, 3837)]
    assert mcginnis["ammo_fraction"].item() == pytest.approx(86 / 89, abs=1e-6)
    hidden_king = demo.players.filter(pl.col("team_num") == 2)["steam_id"]
    holders = fight.filter(
        pl.col("steam_id").is_in(hidden_king.implode()) & pl.col("has_rejuvenator")
    )
    assert set(holders["steam_id"]) == set(hidden_king) - {76561198074690964}


def test_mcginnis_silence_and_fixation(demo: Demo, catalog: None) -> None:
    states = demo.player_states(ticks=TICK, steam_ids=[MCGINNIS], data_version="6712")
    assert "SILENCED" in states["states"].item().to_list()
    # Catalog ID for modifier_passive_haze_stacking_damage, owned by Fixation.
    fixation = (
        demo.active_modifiers.filter(
            (pl.col("tick") <= TICK)
            & (pl.col("hero_id") == 8)
            & (pl.col("modifier_id") == 637809255)
        )
        .sort("tick")
        .group_by("serial")
        .last()
        .filter(pl.col("event") != "removed")
    )
    assert fixation.select("ability_id", "caster_hero_id", "stacks").rows() == [
        (1080948381, 13, 34)
    ]


def test_mcginnis_ammo(demo: Demo, catalog: None) -> None:
    frame = demo.snapshots(ticks=TICK, data_version="6712")
    assert isinstance(frame, pl.DataFrame)
    assert frame.filter(pl.col("steam_id") == MCGINNIS).select(
        "ammo", "max_ammo", "unlimited_ammo"
    ).rows() == [(86, 89, False)]


def test_paige_healing_and_barrier_absorption(final_summary: pl.DataFrame) -> None:
    assert final_summary.filter(pl.col("steam_id") == PAIGE).select(
        "player_healing", "barrier_absorption"
    ).rows() == [(6081, 21516)]
    assert final_summary.schema["barrier_absorption"] == pl.UInt32
