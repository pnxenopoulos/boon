"""Fixture-specific tests for match 103129247.

This build-10854 match covers the replicated match clock, renumbered
EModifierValue values, breakable subclasses, Sinner's Sacrifice events, melee
damage types, and banned heroes.
"""

import polars as pl
import pytest
from boon import Demo

from conftest import FIXTURES_DIR, get_demo

FIXTURE_PATH = FIXTURES_DIR / "103129247.dem"


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("103129247.dem fixture not available")
    return get_demo(FIXTURE_PATH)


def test_match_metadata(demo: Demo) -> None:
    assert demo.match_id == 103129247
    assert demo.build == 10854
    assert demo.total_ticks == 130852
    assert demo.game_over_tick == 128293
    assert demo.winning_team_num == 3


def test_regulation_time_uses_match_clock(demo: Demo) -> None:
    assert demo.regulation_ticks == 126371
    assert demo.regulation_seconds == pytest.approx(1974.546875)
    assert demo.regulation_clock_time == "32:54"

    # Demo ticks include about 30 seconds before the HUD clock starts.
    raw_seconds = demo.tick_to_seconds(demo.game_over_tick)
    assert raw_seconds == pytest.approx(2004.578125)
    assert raw_seconds - demo.regulation_seconds == pytest.approx(30.03125)


def test_no_pauses(demo: Demo) -> None:
    assert not demo.world_ticks["is_paused"].any()


def test_game_over_matches_patron_death(demo: Demo) -> None:
    patron_deaths = demo.objectives.filter(
        (pl.col("objective_type") == "patron") & (pl.col("health") == 0)
    )
    assert patron_deaths["tick"].to_list() == [demo.game_over_tick]


def test_stat_modifier_events_on_build_10854(demo: Demo) -> None:
    events = demo.stat_modifier_events
    assert len(events) == 219
    assert set(events["stat_type"]) == {
        "ammo",
        "cooldown_reduction",
        "fire_rate",
        "health",
        "spirit_power",
        "weapon_damage",
    }

    first = events.sort("tick").row(0, named=True)
    assert first["tick"] == 13668
    assert first["hero_id"] == 1
    assert first["stat_type"] == "spirit_power"
    assert first["amount"] == pytest.approx(2.0)


def test_sinners_sacrifice(demo: Demo) -> None:
    events = demo.sinners_sacrifice
    counts = {
        row["event"]: row["len"]
        for row in events.group_by("event").len().to_dicts()
    }
    assert counts == {"spawned": 12, "hit": 236, "reset": 36}

    hits = events.filter(pl.col("event") == "hit")
    assert hits["damage"].sum() == 21764
    assert events.select("entity_id", "entity_serial").unique().height == 12


def test_breakable_subclasses(demo: Demo) -> None:
    rows = (
        demo.breakables.group_by("subclass_id", "subclass_name")
        .len()
        .to_dicts()
    )
    counts = {
        (row["subclass_id"], row["subclass_name"]): row["len"] for row in rows
    }
    assert counts == {
        (3719077267, "citadel_breakable_item_container"): 109,
        (3986897915, "citadel_breakable_prop_wooden_crate"): 340,
    }


def test_melee_types(demo: Demo) -> None:
    melee = demo.damage.filter(pl.col("is_melee"))
    counts = {
        row["melee_type"]: row["len"]
        for row in melee.group_by("melee_type").len().to_dicts()
    }
    assert counts == {"heavy": 231, "light": 635, "other": 2627}


def test_player_melee_damage(demo: Demo) -> None:
    teams = dict(
        zip(
            demo.players["hero_id"].to_list(),
            demo.players["team_num"].to_list(),
        )
    )
    roster = list(teams)
    melee = demo.damage.filter(
        pl.col("is_melee")
        & pl.col("attacker_hero_id").is_in(roster)
        & pl.col("victim_hero_id").is_in(roster)
        & (pl.col("attacker_hero_id") != pl.col("victim_hero_id"))
    )
    assert all(
        teams[row["attacker_hero_id"]] != teams[row["victim_hero_id"]]
        for row in melee.iter_rows(named=True)
    )

    rows = (
        melee.group_by("melee_type")
        .agg(pl.len().alias("hits"), pl.col("damage").sum())
        .to_dicts()
    )
    totals = {
        row["melee_type"]: (row["hits"], row["damage"]) for row in rows
    }
    assert totals == {
        "heavy": (100, 9467),
        "light": (60, 9505),
    }


def test_willpower_modifier_uses_its_effective_lifetime(demo: Demo) -> None:
    # Valve leaves Willpower serial 7480 in ActiveModifiers until tick 53761.
    # Its GameTime_t fields give it a five-second effective lifetime, so Boon
    # must end it at tick 51198. This test prevents the raw table cleanup time
    # from leaking into active modifier and derived-stat timelines.
    willpower = demo.active_modifiers.filter(
        (pl.col("hero_id") == 25)
        & (pl.col("ability_id") == 2751689917)
        & (pl.col("serial") == 7480)
    ).select("tick", "event")
    assert willpower.rows() == [(50878, "applied"), (51198, "removed")]

    effects = (
        demo.stat_effects("status_resist")
        .filter(pl.col("serial") == 7480)
        .select("tick", "event", "active")
    )
    assert effects.rows() == [
        (50878, "applied", True),
        (51198, "removed", False),
    ]


def test_banned_heroes(demo: Demo) -> None:
    bans = {
        (row["hero_id"], row["hero_name"])
        for row in demo.banned_heroes.to_dicts()
    }
    assert bans == {
        (67, "Paige"),
        (66, "Victor"),
        (7, "Wraith"),
    }
