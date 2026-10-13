"""Replay-backed current upgrades and team minimap facts."""

import polars as pl
import pytest
from boon import Demo
from polars.testing import assert_frame_equal


@pytest.fixture(scope="session")
def enemy_icons(demo: Demo) -> pl.DataFrame:
    return demo.enemy_minimap_ticks(every=640)


def test_recorded_upgrade_vector_is_distinct_from_missing(demo: Demo) -> None:
    frame = demo.player_ticks
    assert frame.schema["upgrade_ids"] == pl.List(pl.UInt32)
    assert frame.schema["boon_count"] == pl.Int64
    empty = frame.filter(pl.col("tick") == 1000)
    assert empty.height == 12
    assert empty["upgrade_ids"].list.len().to_list() == [0] * 12
    late = frame.filter(pl.col("tick") == 187554)
    assert late.height == 12
    assert late["upgrade_ids"].null_count() == 0
    assert late["upgrade_ids"].list.len().min() == 11
    assert late["upgrade_ids"].list.len().max() == 12
    assert frame["boon_count"].null_count() == frame.height


def test_enemy_minimap_uses_recorded_team_icons(enemy_icons: pl.DataFrame) -> None:
    frame = enemy_icons
    assert set(frame.columns) == {
        "tick", "observer_team", "target_team", "target_hero_id", "target_steam_id",
        "target_entity_index", "target_entity_serial", "fow_entity_index_raw",
        "visible_on_map", "map_x", "map_y", "tick_hidden_raw",
        "last_seen_tick", "last_seen_x", "last_seen_y",
    }
    assert not frame.is_empty()
    assert set(frame["observer_team"].unique()) == {2, 3}
    assert frame.filter(pl.col("observer_team") == pl.col("target_team")).is_empty()
    assert frame.filter(
        pl.col("target_entity_index").is_null() & pl.col("target_steam_id").is_not_null()
    ).is_empty()
    assert not frame.filter(~pl.col("visible_on_map").fill_null(False)).select(
        pl.col("map_x").is_not_null().any() | pl.col("map_y").is_not_null().any()
    ).item()
    shown = frame.filter(pl.col("visible_on_map").fill_null(False))
    assert shown.filter(
        (pl.col("map_x") < 0) | (pl.col("map_x") > 255)
        | (pl.col("map_y") < 0) | (pl.col("map_y") > 255)
    ).is_empty()
    assert frame.filter(pl.col("last_seen_tick") > pl.col("tick")).is_empty()


def test_enemy_minimap_steam_id_follows_unique_pawn_controller(
    demo: Demo, enemy_icons: pl.DataFrame
) -> None:
    frame = enemy_icons
    assert frame.schema["target_steam_id"] == pl.UInt64
    known = frame.filter(pl.col("target_steam_id").is_not_null())
    assert not known.is_empty()
    assert known.filter(
        pl.col("fow_entity_index_raw") != pl.col("target_entity_index")
    ).is_empty()
    roster = {
        row["steam_id"]: row["hero_id"]
        for row in demo.players.select("steam_id", "hero_id").to_dicts()
    }
    assert set(known["target_steam_id"].to_list()) <= set(roster)
    assert all(
        row["target_hero_id"] is None
        or roster[row["target_steam_id"]] == row["target_hero_id"]
        for row in known.select("target_steam_id", "target_hero_id").to_dicts()
    )
    by_pawn = known.group_by("target_entity_index", "target_entity_serial").agg(
        pl.col("target_steam_id").n_unique().alias("steam_ids")
    )
    assert by_pawn["steam_ids"].max() == 1


def test_selected_minimap_ticks_preserve_last_seen_history(
    demo: Demo, enemy_icons: pl.DataFrame
) -> None:
    sampled = enemy_icons
    hidden = sampled.filter(
        ~pl.col("visible_on_map")
        & pl.col("last_seen_tick").is_not_null()
        & (pl.col("last_seen_tick") < pl.col("tick"))
    )
    assert not hidden.is_empty()
    ticks = hidden["tick"].unique().sort().tail(3).to_list()
    selected = demo.enemy_minimap_ticks(ticks=[*ticks, 187554])
    assert set(selected["tick"].unique().to_list()) == {*ticks, 187554}
    # A selected query must still read prior visible rows when forming memory.
    expected = sampled.filter(pl.col("tick").is_in(ticks))
    assert_frame_equal(
        selected.filter(pl.col("tick").is_in(ticks)).sort(
            ["tick", "observer_team", "fow_entity_index_raw"]
        ),
        expected.sort(["tick", "observer_team", "fow_entity_index_raw"]),
    )
    haze = selected.filter(
        (pl.col("tick") == 187554)
        & (pl.col("observer_team") == 2)
        & (pl.col("target_hero_id") == 13)
    ).row(0, named=True)
    assert haze["target_entity_index"] == 91
    assert haze["visible_on_map"] is True
    assert (haze["map_x"], haze["map_y"]) == (113, 195)


@pytest.mark.parametrize("query", [{"every": 0}, {"every": -1}, {"ticks": [1], "every": 1}])
def test_minimap_rejects_invalid_sampling(demo: Demo, query: dict) -> None:
    with pytest.raises(ValueError):
        demo.enemy_minimap_ticks(**query)


def test_empty_minimap_selection_has_typed_columns(demo: Demo) -> None:
    frame = demo.enemy_minimap_ticks(ticks=[])
    assert frame.is_empty()
    assert frame.schema["target_steam_id"] == pl.UInt64


def test_existing_objective_rows_expose_mid_boss_health(demo: Demo) -> None:
    boss = demo.objectives.filter(pl.col("objective_type") == "mid_boss")
    assert demo.objectives.schema["entity_serial"] == pl.UInt32
    assert not boss.is_empty()
    assert boss.select(pl.col("max_health").min()).item() > 0
    assert boss.filter(pl.col("health") > pl.col("max_health")).is_empty()
