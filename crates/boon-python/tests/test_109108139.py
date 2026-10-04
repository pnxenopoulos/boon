"""Street Brawl regressions for match 109108139."""

import pytest
from boon import Demo
from conftest import FIXTURES_DIR

FIXTURE_PATH = FIXTURES_DIR / "109108139.dem"


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.is_file():
        pytest.skip("109108139.dem fixture not available")
    replay = Demo(str(FIXTURE_PATH), preload=False)
    replay.load("street_brawl_ticks", "street_brawl_rounds", "world_ticks")
    return replay


def test_street_brawl_roster_and_mode(demo: Demo) -> None:
    assert (demo.match_id, demo.game_mode, demo.winning_team_num) == (109108139, 4, 3)
    assert demo.players.height == 8
    assert demo.players.group_by("team_num").len().sort("team_num").rows() == [
        (2, 4),
        (3, 4),
    ]


def test_street_brawl_ticks(demo: Demo) -> None:
    ticks = demo.street_brawl_ticks
    assert ticks.height == demo.world_ticks.height == 85801
    assert ticks["tick"].is_sorted()
    assert ticks.tail(1).select(
        "round", "state", "amber_score", "sapphire_score"
    ).rows() == [(4, 5, 2, 3)]


def test_street_brawl_round_transitions(demo: Demo) -> None:
    assert demo.street_brawl_rounds.select(
        "round", "tick", "scoring_team", "amber_score", "sapphire_score"
    ).rows() == [
        (1, 16462, 3, 0, 1),
        (2, 25203, 3, 0, 2),
        (3, 38696, 2, 1, 2),
        (4, 55751, 2, 2, 2),
    ]
