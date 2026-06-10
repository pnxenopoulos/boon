"""Tests for boon.Demo against real demo fixtures."""

import re
import tempfile
from pathlib import Path

import polars as pl
import pytest
from boon import (
    Demo,
    InvalidDemoError,
    ability_names,
    game_mode_names,
    hero_names,
    hitgroup_names,
    lifestate_names,
    modifier_names,
    patron_phase_names,
    team_names,
)

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

ABILITIES_COLUMNS = {"tick", "hero_id", "ability"}

ABILITY_UPGRADES_COLUMNS = {"tick", "hero_id", "ability_id", "tier"}

ITEM_PURCHASES_COLUMNS = {"tick", "hero_id", "ability_id", "change"}

CHAT_COLUMNS = {"tick", "hero_id", "text", "chat_type"}

OBJECTIVES_COLUMNS = {
    "tick", "objective_type", "team_num", "lane", "health", "max_health", "phase",
    "x", "y", "z", "entity_id",
}

MID_BOSS_COLUMNS = {"tick", "team_num", "event"}

TROOPERS_COLUMNS = {
    "tick", "trooper_type", "team_num", "lane", "health", "max_health",
    "x", "y", "z", "entity_id",
}

NEUTRALS_COLUMNS = {
    "tick", "team_num", "health", "max_health",
    "x", "y", "z", "entity_id",
}

STAT_MODIFIER_EVENTS_COLUMNS = {"tick", "hero_id", "stat_type", "amount"}

ACTIVE_MODIFIERS_COLUMNS = {
    "tick", "hero_id", "event", "modifier_id", "ability_id",
    "duration", "caster_hero_id", "stacks",
}

URN_COLUMNS = {
    "tick", "event", "hero_id", "team_num", "x", "y", "z",
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
    "abilities": ABILITIES_COLUMNS,
    "ability_upgrades": ABILITY_UPGRADES_COLUMNS,
    "item_purchases": ITEM_PURCHASES_COLUMNS,
    "chat": CHAT_COLUMNS,
    "objectives": OBJECTIVES_COLUMNS,
    "mid_boss": MID_BOSS_COLUMNS,
    "troopers": TROOPERS_COLUMNS,
    "neutrals": NEUTRALS_COLUMNS,
    "stat_modifier_events": STAT_MODIFIER_EVENTS_COLUMNS,
    "active_modifiers": ACTIVE_MODIFIERS_COLUMNS,
    "urn": URN_COLUMNS,
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

    def test_game_mode_positive(self, demo: Demo) -> None:
        assert demo.game_mode > 0

    def test_path_is_pathlib(self, demo: Demo) -> None:
        assert isinstance(demo.path, Path)

    def test_verify(self, demo: Demo) -> None:
        assert demo.verify() is True


# ===================================================================
# Game result
# ===================================================================


class TestGameResult:
    """Tests for game result properties (winning team, game over tick)."""

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



# ===================================================================
# Players and teams
# ===================================================================


class TestPlayersAndTeams:
    """Tests for player and team DataFrames."""

    def test_players_shape(self, demo: Demo) -> None:
        players = demo.players
        assert players.shape[0] > 0
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

    def test_player_ticks_covers_all_players(self, demo: Demo) -> None:
        """Every hero in `players` must appear in `player_ticks`.

        `players` reads the hero ID straight off each player controller, while
        `player_ticks` reaches it through the controller's pawn handle. A bad
        handle mask drops players from `player_ticks` only, so the set of unique
        hero IDs must match between the two datasets.
        """
        player_heroes = set(demo.players["hero_id"].to_list())
        tick_heroes = set(demo.player_ticks["hero_id"].to_list())
        assert tick_heroes == player_heroes


# ===================================================================
# Health invariants
# ===================================================================


class TestHealthInvariants:
    """Tests for player health sanity across all ticks."""

    # A player's current health can momentarily read above max_health — e.g. a
    # transient overheal effect, or health and max_health being networked on
    # different ticks so a snapshot catches them mid-update. It is real but
    # rare, so we cap the share of offending (player, tick) rows rather than
    # forbidding it outright. Observed across fixtures: ~0.01%-0.13%, so 1%
    # leaves comfortable headroom while still catching a regression.
    MAX_OVERHEALTH_RATE = 0.01

    def test_health_rarely_exceeds_max(self, demo: Demo) -> None:
        ticks = demo.player_ticks

        # Only rows with a known, positive max_health carry a meaningful bound;
        # max_health == 0 is an un-networked / dead-state artifact, not a cap a
        # player could exceed.
        valid = ticks.filter(pl.col("max_health") > 0)
        assert len(valid) > 0, "no player ticks with a positive max_health"

        over = valid.filter(pl.col("health") > pl.col("max_health"))
        rate = len(over) / len(valid)

        assert rate <= self.MAX_OVERHEALTH_RATE, (
            f"{rate:.2%} of player ticks have health > max_health "
            f"({len(over)}/{len(valid)}), exceeding the "
            f"{self.MAX_OVERHEALTH_RATE:.0%} tolerance"
        )


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

    def test_game_mode_names_is_dict(self) -> None:
        names = game_mode_names()
        assert isinstance(names, dict)
        assert len(names) > 0

    def test_game_mode_names_contains_known(self) -> None:
        names = game_mode_names()
        assert names[1] == "6v6"
        assert names[4] == "street_brawl"

    def test_patron_phase_names_is_dict(self) -> None:
        names = patron_phase_names()
        assert isinstance(names, dict)
        assert names == {0: "normal", 1: "final", 2: "transforming"}

    def test_hitgroup_names_is_dict(self) -> None:
        names = hitgroup_names()
        assert isinstance(names, dict)
        assert len(names) > 0

    def test_hitgroup_names_contains_known(self) -> None:
        names = hitgroup_names()
        assert names[0] == "generic"
        assert names[1] == "head"
        assert names[-1] == "invalid"
        assert names[19] == "head_no_resist"
        assert 20 not in names  # HITGROUP_COUNT sentinel is omitted

    def test_lifestate_names_is_dict(self) -> None:
        names = lifestate_names()
        assert isinstance(names, dict)
        assert names == {
            0: "alive",
            1: "dying",
            2: "dead",
            3: "respawnable",
            4: "respawning",
        }


# ===================================================================
# Datasets (parameterized)
# ===================================================================


class TestDatasets:
    """Parameterized tests for all dataset properties."""

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_loads_as_dataframe(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        assert isinstance(df, pl.DataFrame)

    # Datasets that may be empty depending on game mode
    POSSIBLY_EMPTY = {"ability_upgrades", "flex_slots", "mid_boss", "neutrals", "stat_modifier_events", "urn"}

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_nonempty(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        if dataset in self.POSSIBLY_EMPTY:
            assert len(df) >= 0
        else:
            assert len(df) > 0

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_columns(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        assert set(df.columns) == DATASET_COLUMNS[dataset]

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_tick_column_nonnegative(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        if "tick" in df.columns and len(df) > 0:
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

    def test_not_street_brawl_error_importable(self) -> None:
        from boon import NotStreetBrawlError  # noqa: F401
