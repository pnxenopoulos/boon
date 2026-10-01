"""Street Brawl regressions; general demo tests use match 108575009."""

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


def test_street_brawl_roster_and_mode(demo: Demo) -> None:
    assert demo.game_mode == 4
    assert demo.players.height == 8
    assert demo.players.group_by("team_num").len().sort("team_num").rows() == [(2, 4), (3, 4)]


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
