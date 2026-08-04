"""Fixture-specific tests for match 94366136.

This recording's post-game tail outlives the player controllers: the
``CCitadelPlayerController`` entities are torn down a few seconds before the
final recorded tick, so a naive final-tick snapshot of ``.players`` comes back
empty. It is the regression fixture for the game-over-tick fallback in
``players`` (see ``collect_players_at`` in ``src/lib.rs``).

A lightweight ``Demo(...)`` fixture is used (not ``get_demo``) because this is a
large demo and these tests only need the roster and match metadata, not every
dataset.
"""

import pytest
from boon import Demo

from conftest import FIXTURES_DIR

FIXTURE_PATH = FIXTURES_DIR / "94366136.dem"

PLAYERS_COLUMNS = ["player_name", "steam_id", "hero_id", "team_num", "start_lane", "rank"]


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("94366136.dem fixture not available")
    return Demo(str(FIXTURE_PATH))


def test_match_id(demo: Demo) -> None:
    assert demo.match_id == 94366136


def test_game_over_before_final_tick(demo: Demo) -> None:
    # The precondition that makes this demo special: the game ends well before
    # the recording does, and the player controllers are despawned during that
    # post-game gap.
    assert demo.game_over_tick is not None
    assert demo.game_over_tick < demo.total_ticks


def test_players_not_empty(demo: Demo) -> None:
    # Regression: the player controllers are gone at total_ticks, so ``.players``
    # must fall back to the game-over tick instead of returning an empty frame.
    players = demo.players
    assert not players.is_empty()
    assert players.height == 12
    assert players.columns == PLAYERS_COLUMNS
    # Every returned player has a real Steam ID (the zero-id skip still applies).
    assert players["steam_id"].min() > 0
    # Two full 6-player teams.
    assert players["team_num"].n_unique() == 2
