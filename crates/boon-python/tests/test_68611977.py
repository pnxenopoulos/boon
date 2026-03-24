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
