"""Fixture-specific tests for match 70555151.

These tests validate exact known values from the demo file
70555151.dem, a regular 6v6 match on a newer build that uses
m_nUpgradeInfo for ability upgrades and updated eValType
constants for stat modifiers.
"""

import polars as pl
import pytest
from boon import Demo, NotStreetBrawlError

from conftest import ALL_DATASETS, FIXTURES_DIR

FIXTURE_PATH = FIXTURES_DIR / "70555151.dem"


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("70555151.dem fixture not available")
    d = Demo(str(FIXTURE_PATH))
    d.load(*ALL_DATASETS)
    return d


# ===================================================================
# Match metadata
# ===================================================================


class TestMatchMetadata:
    def test_match_id(self, demo: Demo) -> None:
        assert demo.match_id == 70555151

    def test_map_name(self, demo: Demo) -> None:
        assert demo.map_name == "start"

    def test_game_mode(self, demo: Demo) -> None:
        assert demo.game_mode == 1

    def test_total_ticks(self, demo: Demo) -> None:
        assert demo.total_ticks == 144545

    def test_tick_rate(self, demo: Demo) -> None:
        assert demo.tick_rate == 64

    def test_build(self, demo: Demo) -> None:
        assert demo.build == 10725

    def test_total_seconds(self, demo: Demo) -> None:
        assert demo.total_seconds == pytest.approx(2258.52, abs=0.1)

    def test_total_clock_time(self, demo: Demo) -> None:
        assert demo.total_clock_time == "37:38"


# ===================================================================
# Game result
# ===================================================================


class TestGameResult:
    def test_winning_team_num(self, demo: Demo) -> None:
        assert demo.winning_team_num == 3

    def test_game_over_tick(self, demo: Demo) -> None:
        assert demo.game_over_tick == 141986


# ===================================================================
# Players and teams
# ===================================================================


EXPECTED_PLAYERS = [
    {"hero_id": 12, "team_num": 3, "start_lane": 6},
    {"hero_id": 18, "team_num": 2, "start_lane": 4},
    {"hero_id": 64, "team_num": 3, "start_lane": 6},
    {"hero_id": 81, "team_num": 2, "start_lane": 6},
    {"hero_id": 20, "team_num": 2, "start_lane": 6},
    {"hero_id": 4, "team_num": 2, "start_lane": 4},
    {"hero_id": 80, "team_num": 2, "start_lane": 1},
    {"hero_id": 17, "team_num": 2, "start_lane": 1},
    {"hero_id": 76, "team_num": 3, "start_lane": 4},
    {"hero_id": 50, "team_num": 3, "start_lane": 1},
    {"hero_id": 15, "team_num": 3, "start_lane": 4},
    {"hero_id": 7, "team_num": 3, "start_lane": 1},
]


class TestPlayers:
    def test_player_count(self, demo: Demo) -> None:
        assert demo.players.shape[0] == 12

    def test_hero_ids(self, demo: Demo) -> None:
        hero_ids = set(demo.players["hero_id"].to_list())
        expected = {p["hero_id"] for p in EXPECTED_PLAYERS}
        assert hero_ids == expected

    def test_team_composition(self, demo: Demo) -> None:
        teams = demo.players.group_by("team_num").len().sort("team_num")
        counts = dict(zip(teams["team_num"].to_list(), teams["len"].to_list()))
        assert counts[2] == 6
        assert counts[3] == 6

    def test_lane_assignments(self, demo: Demo) -> None:
        players = demo.players
        for expected in EXPECTED_PLAYERS:
            row = players.filter(pl.col("hero_id") == expected["hero_id"])
            assert row.shape[0] == 1, f"hero_id {expected['hero_id']} not found"
            assert row["start_lane"][0] == expected["start_lane"]


# ===================================================================
# Kills
# ===================================================================


EXPECTED_KILLS_PER_HERO = {
    0: 1,
    4: 5,
    7: 11,
    12: 4,
    15: 7,
    17: 4,
    18: 3,
    50: 3,
    64: 7,
    76: 6,
    81: 8,
}


class TestKills:
    def test_total_kills(self, demo: Demo) -> None:
        assert len(demo.kills) == 59

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
        assert len(demo.chat) == 12

    def test_all_chat_type(self, demo: Demo) -> None:
        assert (demo.chat["chat_type"] == "all").all()

    def test_first_message(self, demo: Demo) -> None:
        first = demo.chat.sort("tick").head(1)
        assert first["text"][0] == "yall might want to try yellow"

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

    def test_first_boss_kill(self, demo: Demo) -> None:
        first = demo.boss_kills.sort("tick").head(1)
        assert first["entity_class"][0] == "walker"
        assert first["tick"][0] == 22806

    def test_last_boss_kill(self, demo: Demo) -> None:
        last = demo.boss_kills.sort("tick").tail(1)
        assert last["entity_class"][0] == "core"
        assert last["tick"][0] == 140217


# ===================================================================
# Mid boss
# ===================================================================


class TestMidBoss:
    def test_total_events(self, demo: Demo) -> None:
        assert len(demo.mid_boss) == 9

    def test_event_types(self, demo: Demo) -> None:
        events = set(demo.mid_boss["event"].to_list())
        assert events == {"spawned", "killed", "picked_up", "used", "expired"}

    def test_spawn_count(self, demo: Demo) -> None:
        spawns = demo.mid_boss.filter(pl.col("event") == "spawned")
        assert len(spawns) == 1

    def test_kill_count(self, demo: Demo) -> None:
        killed = demo.mid_boss.filter(pl.col("event") == "killed")
        assert len(killed) == 1


# ===================================================================
# Flex slots
# ===================================================================


class TestFlexSlots:
    def test_total(self, demo: Demo) -> None:
        assert len(demo.flex_slots) == 5

    def test_teams(self, demo: Demo) -> None:
        teams = demo.flex_slots.group_by("team_num").len().sort("team_num")
        counts = dict(zip(teams["team_num"].to_list(), teams["len"].to_list()))
        assert counts[2] == 2
        assert counts[3] == 3


# ===================================================================
# Damage
# ===================================================================


class TestDamage:
    def test_total_count(self, demo: Demo) -> None:
        assert len(demo.damage) == 67082


# ===================================================================
# Respawns
# ===================================================================


class TestRespawns:
    def test_total_count(self, demo: Demo) -> None:
        assert len(demo.respawns) == 35


# ===================================================================
# Item purchases
# ===================================================================


class TestItemPurchases:
    def test_total_count(self, demo: Demo) -> None:
        assert len(demo.item_purchases) == 290

    def test_change_types(self, demo: Demo) -> None:
        changes = set(demo.item_purchases["change"].to_list())
        assert changes == {"purchased", "sold"}

    def test_all_heroes_present(self, demo: Demo) -> None:
        hero_ids = sorted(demo.item_purchases["hero_id"].unique().to_list())
        assert hero_ids == [4, 7, 12, 15, 17, 18, 20, 50, 64, 76, 80, 81]

    def test_purchase_count(self, demo: Demo) -> None:
        purchased = demo.item_purchases.filter(pl.col("change") == "purchased")
        assert len(purchased) == 211

    def test_sold_count(self, demo: Demo) -> None:
        sold = demo.item_purchases.filter(pl.col("change") == "sold")
        assert len(sold) == 79


# ===================================================================
# Ability upgrades
# ===================================================================


class TestAbilityUpgrades:
    def test_total_count(self, demo: Demo) -> None:
        assert len(demo.ability_upgrades) == 140

    def test_all_heroes_present(self, demo: Demo) -> None:
        hero_ids = sorted(demo.ability_upgrades["hero_id"].unique().to_list())
        assert hero_ids == [4, 7, 12, 15, 17, 18, 20, 50, 64, 76, 80, 81]

    def test_hero_4_upgrades(self, demo: Demo) -> None:
        h = demo.ability_upgrades.filter(pl.col("hero_id") == 4)
        assert len(h) == 11


# ===================================================================
# Stat modifier events
# ===================================================================


class TestStatModifierEvents:
    def test_total_count(self, demo: Demo) -> None:
        assert len(demo.stat_modifier_events) == 306

    def test_stat_types(self, demo: Demo) -> None:
        types = sorted(demo.stat_modifier_events["stat_type"].unique().to_list())
        assert types == ["ammo", "cooldown_reduction", "fire_rate", "health", "spirit_power", "weapon_damage"]

    def test_first_event(self, demo: Demo) -> None:
        first = demo.stat_modifier_events.sort("tick").head(1)
        assert first["tick"][0] == 11929
        assert first["hero_id"][0] == 7
        assert first["stat_type"][0] == "spirit_power"
        assert first["amount"][0] == pytest.approx(3.0)

    def test_amounts_positive(self, demo: Demo) -> None:
        assert (demo.stat_modifier_events["amount"] > 0).all()


# ===================================================================
# Objectives
# ===================================================================


class TestObjectives:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.objectives) == 2_616_146

    def test_objective_types(self, demo: Demo) -> None:
        types = set(demo.objectives["objective_type"].to_list())
        assert types == {"barracks", "mid_boss", "titan", "walker"}

    def test_team_nums(self, demo: Demo) -> None:
        teams = set(demo.objectives["team_num"].to_list())
        assert teams == {2, 3, 4}


# ===================================================================
# Player ticks
# ===================================================================


class TestPlayerTicks:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.player_ticks) == 938_232

    def test_tick_range(self, demo: Demo) -> None:
        assert demo.player_ticks["tick"].min() == 1
        assert demo.player_ticks["tick"].max() == 144_545

    def test_kelvin_final_stats(self, demo: Demo) -> None:
        """Hero 12 (Kelvin) known final stats."""
        h = demo.player_ticks.filter(pl.col("hero_id") == 12)
        assert h["kills"].max() == 4
        assert h["deaths"].max() == 3
        assert h["assists"].max() == 15
        assert h["souls"].max() == 12326

    def test_viscous_final_stats(self, demo: Demo) -> None:
        """Hero 81 (Viscous) known final stats."""
        h = demo.player_ticks.filter(pl.col("hero_id") == 81)
        assert h["kills"].max() == 8
        assert h["deaths"].max() == 6
        assert h["assists"].max() == 5


# ===================================================================
# World ticks
# ===================================================================


class TestWorldTicks:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.world_ticks) == 144_443

    def test_tick_range(self, demo: Demo) -> None:
        assert demo.world_ticks["tick"].min() == 1
        assert demo.world_ticks["tick"].max() == 144_545

    def test_has_paused_ticks(self, demo: Demo) -> None:
        assert demo.world_ticks["is_paused"].any()


# ===================================================================
# Tick conversion
# ===================================================================


class TestTickConversion:
    def test_tick_1000(self, demo: Demo) -> None:
        assert demo.tick_to_seconds(1000) == pytest.approx(0.015625)
        assert demo.tick_to_clock_time(1000) == "0:00"

    def test_tick_50000(self, demo: Demo) -> None:
        assert demo.tick_to_seconds(50000) == pytest.approx(762.28125)
        assert demo.tick_to_clock_time(50000) == "12:42"


# ===================================================================
# Urn
# ===================================================================


class TestUrn:
    def test_total_count(self, demo: Demo) -> None:
        assert len(demo.urn) == 23

    def test_event_types(self, demo: Demo) -> None:
        events = set(demo.urn["event"].to_list())
        assert events == {"delivery_active", "delivery_inactive", "dropped", "picked_up", "returned"}

    def test_hero_events_have_position(self, demo: Demo) -> None:
        hero_events = demo.urn.filter(pl.col("hero_id") != 0)
        assert len(hero_events) == 17
        assert (hero_events["x"] != 0.0).all()
        assert (hero_events["y"] != 0.0).all()

    def test_first_pickup(self, demo: Demo) -> None:
        pickups = demo.urn.filter(pl.col("event") == "picked_up").sort("tick")
        first = pickups.head(1)
        assert first["tick"][0] == 38401
        assert first["hero_id"][0] == 18


# ===================================================================
# NotStreetBrawlError
# ===================================================================


class TestNotStreetBrawlError:
    def test_street_brawl_ticks_raises(self, demo: Demo) -> None:
        with pytest.raises(NotStreetBrawlError):
            _ = demo.street_brawl_ticks

    def test_street_brawl_rounds_raises(self, demo: Demo) -> None:
        with pytest.raises(NotStreetBrawlError):
            _ = demo.street_brawl_rounds
