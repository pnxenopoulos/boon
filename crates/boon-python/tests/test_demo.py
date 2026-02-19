"""Tests for boon.Demo against real demo fixtures."""

import re
import tempfile
from pathlib import Path

import polars as pl
import pytest
from boon import Demo, InvalidDemoError

# ── Expected columns per dataset ──

PLAYER_TICKS_COLUMNS = {
    "tick", "hero_id", "x", "y", "z", "pitch", "yaw", "roll",
    "in_regen_zone", "death_time", "last_spawn_time", "respawn_time",
    "health", "max_health", "lifestate", "souls", "spent_souls",
    "in_combat_end_time", "in_combat_last_damage_time", "in_combat_start_time",
    "player_damage_dealt_end_time", "player_damage_dealt_last_damage_time",
    "player_damage_dealt_start_time", "player_damage_taken_end_time",
    "player_damage_taken_last_damage_time", "player_damage_taken_start_time",
    "time_revealed_by_npc", "build_id", "is_alive", "has_rebirth",
    "has_rejuvenator", "has_ultimate_trained", "health_regen",
    "ultimate_cooldown_start", "ultimate_cooldown_end",
    "ap_net_worth", "gold_net_worth", "denies", "hero_damage",
    "hero_healing", "objective_damage", "self_healing", "kill_streak",
    "last_hits", "level", "kills", "deaths", "assists",
}

WORLD_TICKS_COLUMNS = {"tick", "is_paused", "next_midboss"}

KILLS_COLUMNS = {"tick", "victim_hero_id", "attacker_hero_id", "assister_hero_ids"}

DAMAGE_COLUMNS = {
    "tick", "damage", "pre_damage", "victim_hero_id", "attacker_hero_id",
    "victim_health_new", "hitgroup_id", "crit_damage",
    "attacker_class", "victim_class",
}

FLEX_SLOTS_COLUMNS = {"tick", "team_num"}

RESPAWNS_COLUMNS = {"tick", "hero_id"}

PURCHASES_COLUMNS = {"tick", "hero_id", "ability_id", "ability", "sell", "quickbuy"}

PLAYERS_COLUMNS = {
    "player_name", "steam_id", "hero", "hero_id", "team", "team_num", "start_lane",
}

TEAMS_COLUMNS = {"team_num", "team_name"}

DATASET_COLUMNS = {
    "player_ticks": PLAYER_TICKS_COLUMNS,
    "world_ticks": WORLD_TICKS_COLUMNS,
    "kills": KILLS_COLUMNS,
    "damage": DAMAGE_COLUMNS,
    "flex_slots": FLEX_SLOTS_COLUMNS,
    "respawns": RESPAWNS_COLUMNS,
    "purchases": PURCHASES_COLUMNS,
}

ALL_DATASETS = list(DATASET_COLUMNS.keys())


# ── Metadata tests ──


def test_total_ticks_positive(demo: Demo) -> None:
    assert demo.total_ticks > 0


def test_map_name_nonempty(demo: Demo) -> None:
    assert isinstance(demo.map_name, str)
    assert len(demo.map_name) > 0


def test_match_id_positive(demo: Demo) -> None:
    assert demo.match_id > 0


def test_tick_rate(demo: Demo) -> None:
    assert demo.tick_rate > 0


def test_total_seconds(demo: Demo) -> None:
    assert demo.total_seconds > 0


def test_total_clock_time_format(demo: Demo) -> None:
    assert re.match(r"\d+:\d{2}", demo.total_clock_time)


def test_build_positive(demo: Demo) -> None:
    assert demo.build > 0


# ── Player / team tests ──


def test_players_shape(demo: Demo) -> None:
    players = demo.players
    assert players.shape[0] == 12
    assert players.shape[1] == 7


def test_players_columns(demo: Demo) -> None:
    players = demo.players
    assert set(players.columns) == PLAYERS_COLUMNS


def test_teams_shape(demo: Demo) -> None:
    teams = demo.teams
    assert teams.shape[0] > 0
    assert set(teams.columns) == TEAMS_COLUMNS


# ── Dataset tests (parametrized over all 7 datasets) ──


@pytest.mark.parametrize("dataset", ALL_DATASETS)
def test_dataset_loads(demo: Demo, dataset: str) -> None:
    df = getattr(demo, dataset)
    assert isinstance(df, pl.DataFrame)


@pytest.mark.parametrize("dataset", ALL_DATASETS)
def test_dataset_nonempty(demo: Demo, dataset: str) -> None:
    df = getattr(demo, dataset)
    assert len(df) > 0


@pytest.mark.parametrize("dataset", ALL_DATASETS)
def test_dataset_columns(demo: Demo, dataset: str) -> None:
    df = getattr(demo, dataset)
    assert set(df.columns) == DATASET_COLUMNS[dataset]


# ── Functional tests ──


def test_tick_to_seconds(demo: Demo) -> None:
    t1 = demo.tick_to_seconds(100)
    t2 = demo.tick_to_seconds(200)
    assert isinstance(t1, float)
    assert t2 > t1


def test_tick_to_clock_time(demo: Demo) -> None:
    result = demo.tick_to_clock_time(100)
    assert isinstance(result, str)
    assert re.match(r"\d+:\d{2}", result)


def test_verify(demo: Demo) -> None:
    assert demo.verify() is True


# ── Error handling tests (no demo fixture needed) ──


def test_file_not_found() -> None:
    with pytest.raises(FileNotFoundError):
        Demo("/nonexistent/path/to/demo.dem")


def test_invalid_demo() -> None:
    with tempfile.NamedTemporaryFile(suffix=".dem", delete=False) as f:
        f.write(b"\x00" * 128)
        f.flush()
        with pytest.raises(InvalidDemoError):
            Demo(f.name)


def test_load_invalid_dataset() -> None:
    """Calling load() with a bogus dataset name should raise ValueError."""
    # We need a real Demo instance for this, but the test is about the
    # validation before any parsing happens. If no fixtures exist, skip.
    fixtures_dir = Path(__file__).parent / "fixtures"
    dems = sorted(fixtures_dir.glob("*.dem")) if fixtures_dir.is_dir() else []
    if not dems:
        pytest.skip("No demo fixtures available")
    demo = Demo(str(dems[0]))
    with pytest.raises(ValueError):
        demo.load("not_a_real_dataset")
