"""Tests for boon.Demo against real demo fixtures."""

import re
import tempfile
from pathlib import Path

import polars as pl
import pytest
from boon import Demo, InvalidDemoError, ability_names, hero_names, modifier_names, team_names

from conftest import _require_demo_fixture

# ---------------------------------------------------------------------------
# Expected columns per dataset
# ---------------------------------------------------------------------------

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

ABILITIES_COLUMNS = {"tick", "hero_id", "ability"}

ABILITY_UPGRADES_COLUMNS = {"tick", "hero_id", "ability_id", "upgrade_bits"}

ITEM_PURCHASES_COLUMNS = {"tick", "hero_id", "ability_id", "change"}

CHAT_COLUMNS = {"tick", "hero_id", "text", "chat_type"}

OBJECTIVES_COLUMNS = {
    "tick", "objective_type", "team_num", "lane", "health", "max_health",
}

BOSS_KILLS_COLUMNS = {
    "tick", "objective_team", "objective_id", "entity_class",
    "gametime",
}

MID_BOSS_COLUMNS = {"tick", "hero_id", "team_num", "event"}

TROOPERS_COLUMNS = {
    "tick", "trooper_type", "team_num", "lane", "health", "max_health",
    "x", "y", "z",
}

NEUTRALS_COLUMNS = {
    "tick", "neutral_type", "team_num", "health", "max_health",
    "x", "y", "z",
}

STAT_MODIFIERS_COLUMNS = {
    "tick", "hero_id", "health", "spirit_power", "fire_rate",
    "weapon_damage", "cooldown_reduction", "ammo",
}

ACTIVE_MODIFIERS_COLUMNS = {
    "tick", "hero_id", "event", "modifier_id", "ability_id",
    "duration", "caster_hero_id", "stacks",
}

PLAYERS_COLUMNS = {
    "player_name", "steam_id", "hero_id", "team_num", "start_lane",
}

# Maps dataset name -> expected column set for parameterized tests.
DATASET_COLUMNS = {
    "player_ticks": PLAYER_TICKS_COLUMNS,
    "world_ticks": WORLD_TICKS_COLUMNS,
    "kills": KILLS_COLUMNS,
    "damage": DAMAGE_COLUMNS,
    "flex_slots": FLEX_SLOTS_COLUMNS,
    "respawns": RESPAWNS_COLUMNS,
    "abilities": ABILITIES_COLUMNS,
    "ability_upgrades": ABILITY_UPGRADES_COLUMNS,
    "item_purchases": ITEM_PURCHASES_COLUMNS,
    "chat": CHAT_COLUMNS,
    "objectives": OBJECTIVES_COLUMNS,
    "boss_kills": BOSS_KILLS_COLUMNS,
    "mid_boss": MID_BOSS_COLUMNS,
    "troopers": TROOPERS_COLUMNS,
    "neutrals": NEUTRALS_COLUMNS,
    "stat_modifiers": STAT_MODIFIERS_COLUMNS,
    "active_modifiers": ACTIVE_MODIFIERS_COLUMNS,
}

ALL_DATASETS = list(DATASET_COLUMNS.keys())


# ===================================================================
# Metadata
# ===================================================================


class TestMetadata:
    """Tests for scalar metadata properties."""

    def test_total_ticks_positive(self, demo: Demo) -> None:
        assert demo.total_ticks > 0

    def test_map_name_nonempty(self, demo: Demo) -> None:
        assert isinstance(demo.map_name, str)
        assert len(demo.map_name) > 0

    def test_match_id_positive(self, demo: Demo) -> None:
        assert demo.match_id > 0

    def test_tick_rate_positive(self, demo: Demo) -> None:
        assert demo.tick_rate > 0

    def test_total_seconds_positive(self, demo: Demo) -> None:
        assert demo.total_seconds > 0

    def test_total_clock_time_format(self, demo: Demo) -> None:
        assert re.match(r"\d+:\d{2}", demo.total_clock_time)

    def test_build_positive(self, demo: Demo) -> None:
        assert demo.build > 0

    def test_path_is_pathlib(self, demo: Demo) -> None:
        assert isinstance(demo.path, Path)

    def test_verify(self, demo: Demo) -> None:
        assert demo.verify() is True


# ===================================================================
# Game result
# ===================================================================


class TestGameResult:
    """Tests for game result properties (winning team, banned heroes)."""

    def test_winning_team_num_is_int_or_none(self, demo: Demo) -> None:
        result = demo.winning_team_num
        assert result is None or isinstance(result, int)

    def test_game_over_tick_is_int_or_none(self, demo: Demo) -> None:
        result = demo.game_over_tick
        assert result is None or isinstance(result, int)

    def test_game_over_tick_within_range(self, demo: Demo) -> None:
        tick = demo.game_over_tick
        if tick is not None:
            assert 0 < tick <= demo.total_ticks

    def test_banned_hero_ids_is_list(self, demo: Demo) -> None:
        result = demo.banned_hero_ids
        assert isinstance(result, list)


# ===================================================================
# Players and teams
# ===================================================================


class TestPlayersAndTeams:
    """Tests for player and team DataFrames."""

    def test_players_shape(self, demo: Demo) -> None:
        players = demo.players
        assert players.shape[0] == 12
        assert players.shape[1] == len(PLAYERS_COLUMNS)

    def test_players_columns(self, demo: Demo) -> None:
        assert set(demo.players.columns) == PLAYERS_COLUMNS

    def test_players_hero_ids_unique(self, demo: Demo) -> None:
        hero_ids = demo.players["hero_id"].to_list()
        assert len(hero_ids) == len(set(hero_ids))

    def test_players_steam_ids_nonzero(self, demo: Demo) -> None:
        steam_ids = demo.players["steam_id"].to_list()
        assert all(sid > 0 for sid in steam_ids)

    def test_players_team_nums_valid(self, demo: Demo) -> None:
        team_nums = demo.players["team_num"].to_list()
        for t in team_nums:
            assert t in (1, 2, 3)


# ===================================================================
# Name lookups
# ===================================================================


class TestNameLookups:
    """Tests for module-level name lookup functions."""

    def test_hero_names_is_dict(self) -> None:
        names = hero_names()
        assert isinstance(names, dict)
        assert len(names) > 0

    def test_hero_names_contains_infernus(self) -> None:
        names = hero_names()
        assert names[1] == "Infernus"

    def test_team_names_is_dict(self) -> None:
        names = team_names()
        assert isinstance(names, dict)
        assert names == {1: "Spectator", 2: "Hidden King", 3: "Archmother"}

    def test_ability_names_is_dict(self) -> None:
        names = ability_names()
        assert isinstance(names, dict)
        assert len(names) > 0

    def test_ability_names_contains_known(self) -> None:
        names = ability_names()
        assert 46922526 in names
        assert names[46922526] == "inherent_base"

    def test_modifier_names_is_dict(self) -> None:
        names = modifier_names()
        assert isinstance(names, dict)
        assert len(names) > 0

    def test_modifier_names_contains_known(self) -> None:
        names = modifier_names()
        assert 2059539911 in names
        assert names[2059539911] == "timer"


# ===================================================================
# Datasets (parameterized)
# ===================================================================


class TestDatasets:
    """Parameterized tests for all dataset properties."""

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_loads_as_dataframe(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        assert isinstance(df, pl.DataFrame)

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_nonempty(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        assert len(df) > 0

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_columns(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        assert set(df.columns) == DATASET_COLUMNS[dataset]

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_tick_column_nonnegative(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        if "tick" in df.columns:
            assert df["tick"].min() >= 0  # type: ignore[operator]


# ===================================================================
# Tick conversion
# ===================================================================


class TestTickConversion:
    """Tests for tick_to_seconds and tick_to_clock_time."""

    def test_tick_to_seconds_type(self, demo: Demo) -> None:
        assert isinstance(demo.tick_to_seconds(100), float)

    def test_tick_to_seconds_monotonic(self, demo: Demo) -> None:
        t1 = demo.tick_to_seconds(10000)
        t2 = demo.tick_to_seconds(20000)
        assert t2 > t1

    def test_tick_to_seconds_zero(self, demo: Demo) -> None:
        assert demo.tick_to_seconds(0) == 0.0

    def test_tick_to_clock_time_type(self, demo: Demo) -> None:
        assert isinstance(demo.tick_to_clock_time(100), str)

    def test_tick_to_clock_time_format(self, demo: Demo) -> None:
        result = demo.tick_to_clock_time(100)
        assert re.match(r"\d+:\d{2}", result)


# ===================================================================
# Bulk loading
# ===================================================================


class TestBulkLoad:
    """Tests for the load() method."""

    def test_load_multiple_datasets(self, demo: Demo) -> None:
        demo.load("kills", "damage")
        assert isinstance(demo.kills, pl.DataFrame)
        assert isinstance(demo.damage, pl.DataFrame)

    def test_load_invalid_dataset_raises(self) -> None:
        path = _require_demo_fixture()
        d = Demo(str(path))
        with pytest.raises(ValueError):
            d.load("not_a_real_dataset")

    def test_load_idempotent(self, demo: Demo) -> None:
        """Loading the same dataset twice should not error."""
        demo.load("kills")
        demo.load("kills")
        assert isinstance(demo.kills, pl.DataFrame)


# ===================================================================
# Error handling (no demo fixture needed)
# ===================================================================


class TestErrors:
    """Tests for error handling with invalid inputs."""

    def test_file_not_found(self) -> None:
        with pytest.raises(FileNotFoundError):
            Demo("/nonexistent/path/to/demo.dem")

    def test_invalid_demo(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".dem", delete=False) as f:
            f.write(b"\x00" * 128)
            f.flush()
            with pytest.raises(InvalidDemoError):
                Demo(f.name)

    def test_all_error_types_importable(self) -> None:
        from boon import DemoHeaderError, DemoInfoError, DemoMessageError, InvalidDemoError  # noqa: F401
