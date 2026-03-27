"""Fixture-specific tests for match 68611977 (street brawl).

These tests validate exact known values from the demo file
68611977.dem, ensuring the parser produces correct game data
for the street brawl game mode.
"""

import polars as pl
import pytest
from boon import Demo

from conftest import ALL_DATASETS, FIXTURES_DIR

FIXTURE_PATH = FIXTURES_DIR / "68611977.dem"

STREET_BRAWL_TICKS_COLUMNS = {
    "tick", "round", "state", "amber_score", "sapphire_score",
    "buy_countdown", "next_state_time", "state_start_time", "non_combat_time",
}

STREET_BRAWL_ROUNDS_COLUMNS = {
    "round", "tick", "scoring_team", "amber_score", "sapphire_score",
}


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("68611977.dem fixture not available")
    d = Demo(str(FIXTURE_PATH))
    d.load(*ALL_DATASETS, "street_brawl_ticks", "street_brawl_rounds")
    return d


# ===================================================================
# Match metadata
# ===================================================================


class TestMatchMetadata:
    def test_match_id(self, demo: Demo) -> None:
        assert demo.match_id == 68611977

    def test_map_name(self, demo: Demo) -> None:
        assert demo.map_name == "start"

    def test_game_mode(self, demo: Demo) -> None:
        assert demo.game_mode == 4

    def test_total_ticks(self, demo: Demo) -> None:
        assert demo.total_ticks == 47491

    def test_build(self, demo: Demo) -> None:
        assert demo.build == 10725


# ===================================================================
# Players
# ===================================================================


class TestPlayers:
    def test_player_count(self, demo: Demo) -> None:
        assert demo.players.shape[0] == 8


# ===================================================================
# Street brawl ticks
# ===================================================================


class TestStreetBrawlTicks:
    def test_loads_as_dataframe(self, demo: Demo) -> None:
        assert isinstance(demo.street_brawl_ticks, pl.DataFrame)

    def test_nonempty(self, demo: Demo) -> None:
        assert len(demo.street_brawl_ticks) > 0

    def test_columns(self, demo: Demo) -> None:
        assert set(demo.street_brawl_ticks.columns) == STREET_BRAWL_TICKS_COLUMNS

    def test_tick_nonnegative(self, demo: Demo) -> None:
        assert demo.street_brawl_ticks["tick"].min() >= 0  # type: ignore[operator]

    def test_row_count_matches_world_ticks(self, demo: Demo) -> None:
        assert len(demo.street_brawl_ticks) == len(demo.world_ticks)


# ===================================================================
# Street brawl rounds
# ===================================================================


class TestStreetBrawlRounds:
    def test_loads_as_dataframe(self, demo: Demo) -> None:
        assert isinstance(demo.street_brawl_rounds, pl.DataFrame)

    def test_nonempty(self, demo: Demo) -> None:
        assert len(demo.street_brawl_rounds) > 0

    def test_columns(self, demo: Demo) -> None:
        assert set(demo.street_brawl_rounds.columns) == STREET_BRAWL_ROUNDS_COLUMNS

    def test_rounds_sequential(self, demo: Demo) -> None:
        rounds = demo.street_brawl_rounds["round"].to_list()
        assert rounds == list(range(1, len(rounds) + 1))

    def test_ticks_monotonic(self, demo: Demo) -> None:
        ticks = demo.street_brawl_rounds["tick"].to_list()
        assert ticks == sorted(ticks)


# ===================================================================
# Player ticks (value-level)
# ===================================================================


class TestPlayerTicks:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.player_ticks) == 217_676

    def test_hero_ids(self, demo: Demo) -> None:
        hero_ids = sorted(demo.player_ticks["hero_id"].unique().to_list())
        assert hero_ids == [2, 12, 31, 63, 65]

    def test_tick_range(self, demo: Demo) -> None:
        assert demo.player_ticks["tick"].min() == 1
        assert demo.player_ticks["tick"].max() == 47_491

    def test_mcginnis_final_stats(self, demo: Demo) -> None:
        """Hero 31 (McGinnis) known final stats."""
        h = demo.player_ticks.filter(pl.col("hero_id") == 31)
        assert h["souls"].max() == 22_400
        assert h["kills"].max() == 1
        assert h["deaths"].max() == 3
        assert h["assists"].max() == 10


# ===================================================================
# World ticks (value-level)
# ===================================================================


class TestWorldTicks:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.world_ticks) == 47_491

    def test_tick_range(self, demo: Demo) -> None:
        assert demo.world_ticks["tick"].min() == 1
        assert demo.world_ticks["tick"].max() == 47_491

    def test_no_paused_ticks(self, demo: Demo) -> None:
        assert not demo.world_ticks["is_paused"].any()


# ===================================================================
# Objectives (value-level)
# ===================================================================


class TestObjectives:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.objectives) == 380_696

    def test_objective_types(self, demo: Demo) -> None:
        types = set(demo.objectives["objective_type"].to_list())
        assert types == {"barracks", "titan", "walker"}

    def test_team_nums(self, demo: Demo) -> None:
        teams = set(demo.objectives["team_num"].to_list())
        assert teams == {2, 3}


# ===================================================================
# Boss kills (value-level)
# ===================================================================


class TestBossKills:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.boss_kills) == 3

    def test_entity_classes(self, demo: Demo) -> None:
        classes = set(demo.boss_kills["entity_class"].to_list())
        assert classes == {"barracks", "walker"}


# ===================================================================
# Item purchases (value-level)
# ===================================================================


class TestItemPurchases:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.item_purchases) == 104

    def test_change_types(self, demo: Demo) -> None:
        changes = set(demo.item_purchases["change"].to_list())
        assert changes == {"purchased", "sold"}

    def test_all_heroes_present(self, demo: Demo) -> None:
        hero_ids = sorted(demo.item_purchases["hero_id"].unique().to_list())
        assert hero_ids == [2, 7, 11, 12, 31, 63, 65, 80]

    def test_purchase_count(self, demo: Demo) -> None:
        purchased = demo.item_purchases.filter(pl.col("change") == "purchased")
        assert len(purchased) == 88

    def test_sold_count(self, demo: Demo) -> None:
        sold = demo.item_purchases.filter(pl.col("change") == "sold")
        assert len(sold) == 16


# ===================================================================
# Ability upgrades (value-level)
# ===================================================================


class TestAbilityUpgrades:
    def test_row_count(self, demo: Demo) -> None:
        assert len(demo.ability_upgrades) == 115

    def test_all_heroes_present(self, demo: Demo) -> None:
        hero_ids = sorted(demo.ability_upgrades["hero_id"].unique().to_list())
        assert hero_ids == [2, 7, 11, 12, 31, 63, 65, 80]

    def test_viscous_upgrades(self, demo: Demo) -> None:
        """Hero 80 (Viscous) had 24 ability upgrades."""
        viscous = demo.ability_upgrades.filter(pl.col("hero_id") == 80)
        assert len(viscous) == 24
