"""Regression tests for hero swaps and lethal damage in match 100655353."""

import polars as pl
import pytest
from boon import Demo
from conftest import FIXTURES_DIR

FIXTURE_PATH = FIXTURES_DIR / "100655353.dem"


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("100655353.dem fixture not available")
    replay = Demo(str(FIXTURE_PATH))
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


def test_overkill_remains_damage(demo: Demo) -> None:
    hits = demo.damage.filter(
        (pl.col("tick") == 35323) & (pl.col("victim_hero_id") == 66)
    )
    assert hits.select("damage", "victim_health_new").rows() == [
        (54, -11),
        (10, -21),
    ]


def test_summary_healing_uses_recorded_statistics(demo: Demo) -> None:
    summary = demo.summary()
    healing = summary["healing"]
    assert set(healing.columns) == {
        "interval_start_s",
        "interval_end_s",
        "healer_steam_id",
        "target_steam_id",
        "healer_hero_id",
        "target_hero_id",
        "source_name",
        "stat_type",
        "amount",
        "total",
    }
    assert (healing["interval_start_s"] < healing["interval_end_s"]).all()
    assert healing["amount"].ge(0).all()
    assert healing["amount"].eq(0).any()
    assert set(healing["stat_type"]) == {"healing", "regen"}
    for stat_type in ("healing", "regen"):
        recorded = summary["damage"].filter(
            (pl.col("stat_type") == stat_type) & ~pl.col("is_category")
        )
        assert (
            healing.filter(pl.col("stat_type") == stat_type)["amount"].sum()
            == recorded["damage"].sum()
        )
    heals = healing.filter(pl.col("stat_type") == "healing")
    assert heals.filter(pl.col("amount") > 0).height == 394
    assert heals.height == 607
    assert heals["amount"].sum() == 226267


def test_summary_snapshot_healing_and_soul_sources(demo: Demo) -> None:
    summary = demo.summary()
    steam_id = demo.players.filter(pl.col("hero_id") == 3)["steam_id"].item()
    player = (
        summary["snapshots"]
        .filter((pl.col("steam_id") == steam_id) & (pl.col("snapshot_time_s") == 2338))
        .row(0, named=True)
    )
    assert player["hero_id"] == 3
    assert player["player_healing"] == player["self_healing"] == 4192
    assert player["teammate_healing"] == 0
    assert player["player_damage_taken"] == 20302
    assert player["creep_damage"] == 36345
    assert player["neutral_damage"] == 5481
    assert player["self_damage"] == 6730
    sources = summary["gold_sources"].filter(
        (pl.col("steam_id") == steam_id) & (pl.col("snapshot_time_s") == 2338)
    )
    players = sources.filter(pl.col("source_id") == 1).row(0, named=True)
    assert players["source_name"] == "k_ePlayers"
    assert (players["gold"], players["gold_orbs"], players["damage"]) == (
        8842,
        0,
        53473,
    )
    assists = sources.filter(pl.col("source_id") == 6).row(0, named=True)
    assert assists["source_name"] == "k_eAssists"
    assert assists["gold"] == 908
    assert assists["gold_orbs"] is None


def test_summary_cumulative_healing_matches_player_counters(demo: Demo) -> None:
    summary = demo.summary()
    healing = summary["healing"]
    for snapshot in summary["snapshots"].iter_rows(named=True):
        time = snapshot["snapshot_time_s"]
        if time not in healing["interval_end_s"]:
            continue
        rows = healing.filter(
            (pl.col("interval_end_s") == time)
            & (pl.col("healer_steam_id") == snapshot["steam_id"])
            & (pl.col("stat_type") == "healing")
        )
        assert rows["total"].sum() == snapshot["player_healing"]
        self_healing = rows.filter(pl.col("target_steam_id") == snapshot["steam_id"])
        assert self_healing["total"].sum() == snapshot["self_healing"]


def test_summary_steam_ids_join_the_roster_after_a_hero_swap(demo: Demo) -> None:
    summary = demo.summary()
    roster = demo.players.select("steam_id", "hero_id")
    for name in ("snapshots", "last_hits", "gold_sources"):
        identities = summary[name].select("steam_id", "hero_id").unique()
        assert identities.schema["steam_id"] == pl.UInt64
        assert identities["steam_id"].null_count() == 0
        assert identities.join(
            roster, on=["steam_id", "hero_id"], how="anti"
        ).is_empty()
        assert (
            identities.filter(pl.col("steam_id") == 76561198853347303)["hero_id"].item()
            == 66
        )
    for name, roles in (
        ("damage", ("dealer", "target")),
        ("healing", ("healer", "target")),
    ):
        for role in roles:
            rows = summary[name].filter(pl.col(f"{role}_hero_id").is_not_null())
            assert rows[f"{role}_steam_id"].null_count() == 0
            assert set(rows[f"{role}_steam_id"]) <= set(roster["steam_id"])
