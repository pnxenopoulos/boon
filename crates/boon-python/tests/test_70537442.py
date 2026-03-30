"""Fixture-specific tests for match 70537442 (street brawl).

These tests validate exact known values from the demo file
70537442.dem, a street brawl match on a newer build. Covers
street brawl-specific datasets and the 4v4 player format.
"""

import polars as pl
import pytest
from boon import Demo

from conftest import FIXTURES_DIR, get_demo

FIXTURE_PATH = FIXTURES_DIR / "70537442.dem"


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("70537442.dem fixture not available")
    return get_demo(FIXTURE_PATH)


# ===================================================================
# Match metadata
# ===================================================================


class TestMatchMetadata:
    def test_match_id(self, demo: Demo) -> None:
        assert demo.match_id == 70537442

    def test_map_name(self, demo: Demo) -> None:
        assert demo.map_name == "start"

    def test_game_mode(self, demo: Demo) -> None:
        assert demo.game_mode == 4

    def test_total_ticks(self, demo: Demo) -> None:
        assert demo.total_ticks == 33257

    def test_tick_rate(self, demo: Demo) -> None:
        assert demo.tick_rate == 64

    def test_build(self, demo: Demo) -> None:
        assert demo.build == 10725

    def test_total_seconds(self, demo: Demo) -> None:
        assert demo.total_seconds == pytest.approx(519.64, abs=0.1)

    def test_total_clock_time(self, demo: Demo) -> None:
        assert demo.total_clock_time == "8:39"


# ===================================================================
# Game result
# ===================================================================


class TestGameResult:
    def test_winning_team_num(self, demo: Demo) -> None:
        assert demo.winning_team_num == 3

    def test_game_over_tick(self, demo: Demo) -> None:
        assert demo.game_over_tick == 30697


# ===================================================================
# Players and teams
# ===================================================================


EXPECTED_PLAYERS = [
    {"hero_id": 77, "team_num": 3, "start_lane": 4},
    {"hero_id": 27, "team_num": 2, "start_lane": 4},
    {"hero_id": 11, "team_num": 2, "start_lane": 6},
    {"hero_id": 31, "team_num": 2, "start_lane": 1},
    {"hero_id": 25, "team_num": 3, "start_lane": 4},
    {"hero_id": 80, "team_num": 3, "start_lane": 1},
    {"hero_id": 65, "team_num": 2, "start_lane": 4},
    {"hero_id": 20, "team_num": 3, "start_lane": 6},
]


class TestPlayers:
    def test_player_count(self, demo: Demo) -> None:
        assert demo.players.shape[0] == 8

    def test_hero_ids(self, demo: Demo) -> None:
        hero_ids = set(demo.players["hero_id"].to_list())
        expected = {p["hero_id"] for p in EXPECTED_PLAYERS}
        assert hero_ids == expected

    def test_team_composition(self, demo: Demo) -> None:
        """4v4 street brawl."""
        teams = demo.players.group_by("team_num").len().sort("team_num")
        counts = dict(zip(teams["team_num"].to_list(), teams["len"].to_list()))
        assert counts[2] == 4
        assert counts[3] == 4


# ===================================================================
# Kills
# ===================================================================


EXPECTED_KILLS_PER_HERO = {20: 1, 25: 4, 65: 1, 77: 5, 80: 4}


class TestKills:
    def test_total_kills(self, demo: Demo) -> None:
        assert len(demo.kills) == 15

    def test_kills_per_attacker(self, demo: Demo) -> None:
        counts = (
            demo.kills.group_by("attacker_hero_id")
            .len()
            .sort("attacker_hero_id")
        )
        result = dict(zip(
            counts["attacker_hero_id"].to_list(),
            counts["len"].to_list(),
        ))
        assert result == EXPECTED_KILLS_PER_HERO


# ===================================================================
# Chat
# ===================================================================


class TestChat:
    def test_total_messages(self, demo: Demo) -> None:
        assert len(demo.chat) == 2

    def test_first_message(self, demo: Demo) -> None:
        first = demo.chat.sort("tick").head(1)
        assert first["text"][0] == "plenty of time"
        assert first["chat_type"][0] == "team"

    def test_last_message(self, demo: Demo) -> None:
        last = demo.chat.sort("tick").tail(1)
        assert last["text"][0] == "gg"
        assert last["chat_type"][0] == "all"


# ===================================================================
# Mid boss / flex slots (empty in street brawl)
# ===================================================================


class TestEmptyDatasets:
    def test_no_mid_boss(self, demo: Demo) -> None:
        assert len(demo.mid_boss) == 0

    def test_no_flex_slots(self, demo: Demo) -> None:
        assert len(demo.flex_slots) == 0

    def test_no_urn(self, demo: Demo) -> None:
        assert len(demo.urn) == 0

    def test_no_neutrals(self, demo: Demo) -> None:
        assert len(demo.neutrals) == 0

    def test_no_stat_modifier_events(self, demo: Demo) -> None:
        assert len(demo.stat_modifier_events) == 0


# ===================================================================
# Damage
# ===================================================================


class TestDamage:
    def test_total_count(self, demo: Demo) -> None:
        assert len(demo.damage) == 5889


# ===================================================================
# Item purchases
# ===================================================================


class TestItemPurchases:
    def test_total_count(self, demo: Demo) -> None:
        assert len(demo.item_purchases) == 88

    def test_change_types(self, demo: Demo) -> None:
        changes = set(demo.item_purchases["change"].to_list())
        assert changes == {"purchased", "sold"}

    def test_all_heroes_present(self, demo: Demo) -> None:
        hero_ids = sorted(demo.item_purchases["hero_id"].unique().to_list())
        assert hero_ids == [11, 20, 25, 27, 31, 65, 77, 80]


# ===================================================================
# Ability upgrades
# ===================================================================


class TestAbilityUpgrades:
    def test_total_count(self, demo: Demo) -> None:
        assert len(demo.ability_upgrades) == 90

    def test_all_heroes_present(self, demo: Demo) -> None:
        hero_ids = sorted(demo.ability_upgrades["hero_id"].unique().to_list())
        assert hero_ids == [11, 20, 25, 27, 31, 65, 77, 80]


# ===================================================================
# Objectives
# ===================================================================


class TestObjectives:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.objectives) == 446

    def test_objective_types(self, demo: Demo) -> None:
        types = set(demo.objectives["objective_type"].to_list())
        assert types == {"barracks", "patron", "shrine", "walker"}

    def test_team_nums(self, demo: Demo) -> None:
        teams = set(demo.objectives["team_num"].to_list())
        assert teams == {2, 3}


# ===================================================================
# Player ticks
# ===================================================================


class TestPlayerTicks:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.player_ticks) == 32_039

    def test_hero_ids(self, demo: Demo) -> None:
        """Street brawl local replay — only local player's pawn."""
        hero_ids = sorted(demo.player_ticks["hero_id"].unique().to_list())
        assert hero_ids == [77]

    def test_tick_range(self, demo: Demo) -> None:
        assert demo.player_ticks["tick"].min() == 1
        assert demo.player_ticks["tick"].max() == 33_257

    def test_hero_77_final_stats(self, demo: Demo) -> None:
        h = demo.player_ticks.filter(pl.col("hero_id") == 77)
        assert h["kills"].max() == 5
        assert h["deaths"].max() == 1
        assert h["assists"].max() == 9
        assert h["souls"].max() == 22_400


# ===================================================================
# World ticks
# ===================================================================


class TestWorldTicks:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.world_ticks) == 33_257

    def test_tick_range(self, demo: Demo) -> None:
        assert demo.world_ticks["tick"].min() == 1
        assert demo.world_ticks["tick"].max() == 33_257

    def test_no_paused_ticks(self, demo: Demo) -> None:
        assert not demo.world_ticks["is_paused"].any()


# ===================================================================
# Street brawl ticks
# ===================================================================


class TestStreetBrawlTicks:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.street_brawl_ticks) == 33_257

    def test_row_count_matches_world_ticks(self, demo: Demo) -> None:
        assert len(demo.street_brawl_ticks) == len(demo.world_ticks)

    def test_final_scores(self, demo: Demo) -> None:
        last = demo.street_brawl_ticks.sort("tick").tail(1)
        assert last["amber_score"][0] == 0
        assert last["sapphire_score"][0] == 3


# ===================================================================
# Street brawl rounds
# ===================================================================


class TestStreetBrawlRounds:
    def test_round_count(self, demo: Demo) -> None:
        assert len(demo.street_brawl_rounds) == 2

    def test_rounds_sequential(self, demo: Demo) -> None:
        rounds = demo.street_brawl_rounds["round"].to_list()
        assert rounds == [1, 2]

    def test_ticks_monotonic(self, demo: Demo) -> None:
        ticks = demo.street_brawl_rounds["tick"].to_list()
        assert ticks == sorted(ticks)

    def test_sapphire_wins_both(self, demo: Demo) -> None:
        """Team Sapphire (3) won both recorded rounds."""
        assert (demo.street_brawl_rounds["scoring_team"] == 3).all()

    def test_round_1(self, demo: Demo) -> None:
        r1 = demo.street_brawl_rounds.filter(pl.col("round") == 1)
        assert r1["tick"][0] == 12151
        assert r1["amber_score"][0] == 0
        assert r1["sapphire_score"][0] == 1

    def test_round_2(self, demo: Demo) -> None:
        r2 = demo.street_brawl_rounds.filter(pl.col("round") == 2)
        assert r2["tick"][0] == 18886
        assert r2["amber_score"][0] == 0
        assert r2["sapphire_score"][0] == 2
