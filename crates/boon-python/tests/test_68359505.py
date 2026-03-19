"""Fixture-specific tests for match 68359505.

These tests validate exact known values from the demo file
68359505.dem, ensuring the parser produces correct game data
— not just structurally valid output.
"""

from pathlib import Path

import polars as pl
import pytest
from boon import Demo

from conftest import FIXTURES_DIR

FIXTURE_PATH = FIXTURES_DIR / "68359505.dem"


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("68359505.dem fixture not available")
    return Demo(str(FIXTURE_PATH))


# ===================================================================
# Match metadata
# ===================================================================


class TestMatchMetadata:
    def test_match_id(self, demo: Demo) -> None:
        assert demo.match_id == 68359505

    def test_map_name(self, demo: Demo) -> None:
        assert demo.map_name == "start"

    def test_total_ticks(self, demo: Demo) -> None:
        assert demo.total_ticks == 148382

    def test_tick_rate(self, demo: Demo) -> None:
        assert demo.tick_rate == 64

    def test_build(self, demo: Demo) -> None:
        assert demo.build == 10725

    def test_total_seconds(self, demo: Demo) -> None:
        assert demo.total_seconds == pytest.approx(2318.47, abs=0.1)

    def test_total_clock_time(self, demo: Demo) -> None:
        assert demo.total_clock_time == "38:38"


# ===================================================================
# Game result
# ===================================================================


class TestGameResult:
    def test_winning_team(self, demo: Demo) -> None:
        assert demo.winning_team == "Hidden King"

    def test_winning_team_num(self, demo: Demo) -> None:
        assert demo.winning_team_num == 2

    def test_game_over_tick(self, demo: Demo) -> None:
        assert demo.game_over_tick == 145823

    def test_no_bans(self, demo: Demo) -> None:
        assert demo.banned_hero_ids == []
        assert demo.banned_heroes == []


# ===================================================================
# Players and teams
# ===================================================================


EXPECTED_PLAYERS = [
    {"hero_id": 20, "hero": "Ivy", "team_num": 2, "start_lane": 6},
    {"hero_id": 27, "hero": "Yamato", "team_num": 2, "start_lane": 6},
    {"hero_id": 19, "hero": "Shiv", "team_num": 3, "start_lane": 4},
    {"hero_id": 81, "hero": "Celeste", "team_num": 2, "start_lane": 1},
    {"hero_id": 16, "hero": "Calico", "team_num": 2, "start_lane": 4},
    {"hero_id": 79, "hero": "Rem", "team_num": 3, "start_lane": 4},
    {"hero_id": 25, "hero": "Warden", "team_num": 2, "start_lane": 4},
    {"hero_id": 12, "hero": "Kelvin", "team_num": 2, "start_lane": 1},
    {"hero_id": 76, "hero": "Graves", "team_num": 3, "start_lane": 6},
    {"hero_id": 18, "hero": "Mo and Krill", "team_num": 3, "start_lane": 1},
    {"hero_id": 64, "hero": "Drifter", "team_num": 3, "start_lane": 6},
    {"hero_id": 17, "hero": "Grey Talon", "team_num": 3, "start_lane": 1},
]


class TestPlayers:
    def test_player_count(self, demo: Demo) -> None:
        assert demo.players.shape[0] == 12

    def test_hero_roster(self, demo: Demo) -> None:
        heroes = sorted(demo.players["hero"].to_list())
        expected = sorted(p["hero"] for p in EXPECTED_PLAYERS)
        assert heroes == expected

    def test_hero_ids(self, demo: Demo) -> None:
        hero_ids = set(demo.players["hero_id"].to_list())
        expected = {p["hero_id"] for p in EXPECTED_PLAYERS}
        assert hero_ids == expected

    def test_team_composition(self, demo: Demo) -> None:
        """Hidden King has 6 players, Archmother has 6."""
        teams = demo.players.group_by("team_num").len().sort("team_num")
        counts = dict(zip(teams["team_num"].to_list(), teams["len"].to_list()))
        assert counts[2] == 6  # Hidden King
        assert counts[3] == 6  # Archmother

    def test_lane_assignments(self, demo: Demo) -> None:
        players = demo.players
        for expected in EXPECTED_PLAYERS:
            row = players.filter(pl.col("hero_id") == expected["hero_id"])
            assert row.shape[0] == 1, f"hero_id {expected['hero_id']} not found"
            assert row["start_lane"][0] == expected["start_lane"]

    def test_teams(self, demo: Demo) -> None:
        teams = demo.teams.sort("team_num")
        assert teams.shape[0] == 3
        names = teams["team_name"].to_list()
        assert names == ["Spectator", "Hidden King", "Archmother"]


# ===================================================================
# Kills
# ===================================================================


# hero_id -> expected kill count
EXPECTED_KILLS_PER_HERO = {
    12: 3,   # Kelvin
    16: 9,   # Calico
    17: 3,   # Grey Talon
    19: 5,   # Shiv
    20: 4,   # Ivy
    25: 16,  # Warden
    27: 3,   # Yamato
    64: 5,   # Drifter
    76: 1,   # Graves
}


class TestKills:
    def test_total_kills(self, demo: Demo) -> None:
        assert len(demo.kills) == 49

    def test_kills_per_attacker(self, demo: Demo) -> None:
        kills = demo.kills
        counts = (
            kills.group_by("attacker_hero_id")
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
        assert len(demo.chat) == 7

    def test_all_chat_type(self, demo: Demo) -> None:
        assert (demo.chat["chat_type"] == "all").all()

    def test_first_message(self, demo: Demo) -> None:
        first = demo.chat.sort("tick").head(1)
        assert first["text"][0] == "i love it when ppl get along"

    def test_last_message(self, demo: Demo) -> None:
        last = demo.chat.sort("tick").tail(1)
        assert last["text"][0] == "gg"


# ===================================================================
# Boss kills
# ===================================================================


class TestBossKills:
    def test_total_boss_kills(self, demo: Demo) -> None:
        assert len(demo.boss_kills) == 18

    def test_entity_classes(self, demo: Demo) -> None:
        classes = set(demo.boss_kills["entity_class"].to_list())
        assert classes == {"walker", "titan", "titan_shield_generator", "core", "mid_boss", "barracks"}

    def test_first_boss_kill_is_walker(self, demo: Demo) -> None:
        first = demo.boss_kills.sort("tick").head(1)
        assert first["entity_class"][0] == "walker"
        assert first["tick"][0] == 34568

    def test_last_boss_kill_is_titan(self, demo: Demo) -> None:
        last = demo.boss_kills.sort("tick").tail(1)
        assert last["entity_class"][0] == "titan"
        assert last["tick"][0] == 145509


# ===================================================================
# Mid boss
# ===================================================================


class TestMidBoss:
    def test_total_events(self, demo: Demo) -> None:
        assert len(demo.mid_boss) == 16

    def test_event_types(self, demo: Demo) -> None:
        events = set(demo.mid_boss["event"].to_list())
        assert events == {"spawned", "killed", "picked_up", "used", "expired"}

    def test_spawn_count(self, demo: Demo) -> None:
        spawns = demo.mid_boss.filter(pl.col("event") == "spawned")
        assert len(spawns) == 2

    def test_kill_count(self, demo: Demo) -> None:
        killed = demo.mid_boss.filter(pl.col("event") == "killed")
        assert len(killed) == 2


# ===================================================================
# Flex slots
# ===================================================================


class TestFlexSlots:
    def test_total(self, demo: Demo) -> None:
        assert len(demo.flex_slots) == 4

    def test_teams(self, demo: Demo) -> None:
        teams = sorted(demo.flex_slots["team_num"].to_list())
        assert teams == [2, 2, 2, 3]


# ===================================================================
# Tick conversion
# ===================================================================


class TestTickConversion:
    def test_tick_1000(self, demo: Demo) -> None:
        assert demo.tick_to_seconds(1000) == pytest.approx(0.140625)
        assert demo.tick_to_clock_time(1000) == "0:00"

    def test_tick_50000(self, demo: Demo) -> None:
        assert demo.tick_to_seconds(50000) == pytest.approx(765.765625)
        assert demo.tick_to_clock_time(50000) == "12:45"
