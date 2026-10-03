"""Hero-swap regression tests for match 100655353."""

import polars as pl
import pytest
from boon import Demo
from conftest import FIXTURES_DIR

FIXTURE_PATH = FIXTURES_DIR / "100655353.dem"


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("100655353.dem fixture not available")
    replay = Demo(str(FIXTURE_PATH), preload=False)
    replay.load("chat", "item_purchases")
    return replay


def test_players_uses_victor_after_silver_swap(demo: Demo) -> None:
    roster = demo.players
    player = roster.filter(pl.col("steam_id") == 76561198853347303)
    assert player.select("player_name", "hero_id").rows() == [("jejaimeb", 66)]
    assert roster.filter(pl.col("hero_id") == 80).is_empty()


def test_chat_uses_victor_after_silver_swap(demo: Demo) -> None:
    chat = demo.chat
    victor = chat.filter(pl.col("hero_id") == 66)
    assert len(victor) == 24
    assert victor["tick"].min() == 45268
    assert victor["tick"].max() == 140024
    assert chat.filter(pl.col("hero_id") == 80).is_empty()


def test_item_purchases_use_victor_after_silver_swap(demo: Demo) -> None:
    purchases = demo.item_purchases
    victor = purchases.filter(pl.col("hero_id") == 66)
    assert len(victor) == 37
    assert victor["tick"].min() == 7224
    assert victor["tick"].max() == 155515
    assert purchases.filter(pl.col("hero_id") == 80).is_empty()
