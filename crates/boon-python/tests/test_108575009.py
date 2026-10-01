"""Viewer-verified scoreboard and fight state for match 108575009."""

import gzip
import json
from pathlib import Path

import polars as pl
import pytest
from boon import Demo, data
from conftest import FIXTURES_DIR, get_demo

FIXTURE_PATH = FIXTURES_DIR / "108575009.dem"
TICK = 187554
MCGINNIS = 76561198037652386
KELVIN = 76561198055516822
PAIGE = 76561198295494515
SCOREBOARD = json.loads(
    Path(__file__).with_name("108575009-scoreboard.json").read_text()
)


@pytest.fixture(scope="module")
def demo() -> Demo:
    if not FIXTURE_PATH.exists():
        pytest.skip("108575009.dem fixture not available")
    return get_demo(FIXTURE_PATH)


@pytest.fixture(scope="module")
def fight(demo: Demo) -> pl.DataFrame:
    frame = demo.snapshots(ticks=TICK)
    assert isinstance(frame, pl.DataFrame)
    return frame


@pytest.fixture(scope="module")
def final_players(demo: Demo) -> pl.DataFrame:
    # Pawns can disappear after death. Retain each player's last recorded row,
    # including the post-game interval, rather than require a pawn at game over.
    frame = demo.snapshots(every=64)
    assert isinstance(frame, pl.DataFrame)
    return frame.sort("tick").group_by("steam_id").last()


@pytest.fixture(scope="module")
def final_summary(demo: Demo) -> pl.DataFrame:
    return (
        demo.summary()["snapshots"].sort("snapshot_time_s").group_by("steam_id").last()
    )


@pytest.fixture(scope="module")
def catalog_dir(tmp_path_factory: pytest.TempPathFactory) -> Path:
    directory = tmp_path_factory.mktemp("108575009-catalog")
    source = Path(__file__).parent / "catalogs" / "108575009"
    for name in ("heroes", "abilities", "modifiers", "misc"):
        (directory / f"{name}.json").write_bytes(
            gzip.decompress((source / f"{name}.json.gz").read_bytes())
        )
    return directory


@pytest.fixture
def catalog(monkeypatch: pytest.MonkeyPatch, catalog_dir: Path) -> None:
    def installed(version: str) -> Path:
        assert version == "6712"
        return catalog_dir

    monkeypatch.setattr(data, "update", installed)


def assert_displayed_total(actual: int, displayed: str) -> None:
    # The viewer rounds large totals to thousands or tenths of a thousand.
    value = displayed.removesuffix("k")
    precision = 100 if "." in value else 1000
    assert actual == pytest.approx(float(value) * 1000, rel=0, abs=precision / 2)


@pytest.mark.parametrize("expected", SCOREBOARD, ids=lambda row: row["hero"])
def test_final_scoreboard(demo, final_players, final_summary, expected):
    selection = pl.col("steam_id") == expected["steam_id"]
    roster = demo.players.filter(selection).row(0, named=True)
    assert (roster["hero_id"], roster["team_num"]) == (
        expected["hero_id"],
        expected["team_num"],
    )
    if expected["player_name"] is not None:
        assert roster["player_name"] == expected["player_name"]
    live = final_players.filter(selection).row(0, named=True)
    summary = final_summary.filter(selection).row(0, named=True)
    # Post-match interval counters can count kills/deaths differently from the
    # scoreboard. The live controller counters match the viewer's final K/D/A.
    for name in ("kills", "deaths", "assists"):
        assert live[name] == expected[name]
    assert_displayed_total(summary["net_worth"], expected["souls"])
    assert_displayed_total(summary["player_damage"], expected["player_damage"])
    assert_displayed_total(live["objective_damage"], expected["objective_damage"])
    # The supplied Healing display includes barrier absorption. Keep the
    # recorded components separate in the API and combine only for this check.
    assert_displayed_total(
        summary["player_healing"] + summary["barrier_absorption"],
        expected["healing_and_barriers"],
    )


def test_fight_clock_health_and_rejuvenator(demo: Demo, fight: pl.DataFrame) -> None:
    assert demo.match_id == 108575009
    assert demo.tick_to_match_clock(TICK) == "48:21"
    assert fight.height == fight["steam_id"].n_unique() == 12
    assert "player_slot" not in fight.columns
    assert fight.filter(pl.col("steam_id") == KELVIN).select(
        "health", "max_health"
    ).rows() == [(5039, 5039)]
    mcginnis = fight.filter(pl.col("steam_id") == MCGINNIS)
    assert mcginnis.select("health", "max_health").rows() == [(1132, 3837)]
    assert mcginnis["ammo_fraction"].item() == pytest.approx(86 / 89, abs=1e-6)
    hidden_king = demo.players.filter(pl.col("team_num") == 2)["steam_id"]
    holders = fight.filter(
        pl.col("steam_id").is_in(hidden_king.implode()) & pl.col("has_rejuvenator")
    )
    assert set(holders["steam_id"]) == set(hidden_king) - {76561198074690964}


def test_mcginnis_silence_and_fixation(demo: Demo, catalog: None) -> None:
    states = demo.player_states(ticks=TICK, steam_ids=[MCGINNIS], data_version="6712")
    assert "SILENCED" in states["states"].item().to_list()
    # Catalog ID for modifier_passive_haze_stacking_damage, owned by Fixation.
    fixation = (
        demo.active_modifiers.filter(
            (pl.col("tick") <= TICK)
            & (pl.col("hero_id") == 8)
            & (pl.col("modifier_id") == 637809255)
        )
        .sort("tick")
        .group_by("serial")
        .last()
        .filter(pl.col("event") != "removed")
    )
    assert fixation.select("ability_id", "caster_hero_id", "stacks").rows() == [
        (1080948381, 13, 34)
    ]


def test_mcginnis_ammo(demo: Demo, catalog: None) -> None:
    frame = demo.snapshots(ticks=TICK, data_version="6712")
    assert isinstance(frame, pl.DataFrame)
    assert frame.filter(pl.col("steam_id") == MCGINNIS).select(
        "ammo", "max_ammo", "unlimited_ammo"
    ).rows() == [(86, 89, False)]


def test_paige_healing_and_barrier_absorption(final_summary: pl.DataFrame) -> None:
    assert final_summary.filter(pl.col("steam_id") == PAIGE).select(
        "player_healing", "barrier_absorption"
    ).rows() == [(6081, 21516)]
    assert final_summary.schema["barrier_absorption"] == pl.UInt32


def test_match_metadata_and_end(demo: Demo) -> None:
    assert (
        demo.build,
        demo.total_ticks,
        demo.winning_team_num,
        demo.game_over_tick,
    ) == (10932, 193317, 2, 190758)
    assert demo.regulation_ticks == 188897
    assert demo.regulation_clock_time == "49:11"
    assert demo.regulation_seconds == pytest.approx(2951.515625)
    patron_deaths = demo.objectives.filter(
        (pl.col("objective_type") == "patron") & (pl.col("health") == 0)
    )
    assert patron_deaths["tick"].unique().to_list() == [demo.game_over_tick]


def test_breakables_and_sinners_sacrifice(demo: Demo) -> None:
    assert demo.breakables.group_by("subclass_id").len().sort("subclass_id").rows() == [
        (202631964, 58),
        (3719077267, 142),
        (3986897915, 355),
    ]
    sacrifice = demo.sinners_sacrifice
    assert sacrifice.group_by("event").len().sort("event").rows() == [
        ("hit", 396),
        ("reset", 66),
        ("spawned", 15),
    ]
    assert sacrifice["damage"].sum() == 36848


def test_player_melee_damage(demo: Demo) -> None:
    roster = demo.players["hero_id"]
    melee = demo.damage.filter(
        pl.col("is_melee")
        & pl.col("attacker_hero_id").is_in(roster.implode())
        & pl.col("victim_hero_id").is_in(roster.implode())
        & (pl.col("attacker_hero_id") != pl.col("victim_hero_id"))
    )
    assert melee.group_by("melee_type").agg(
        pl.len().alias("hits"), pl.col("damage").sum()
    ).sort("melee_type").rows() == [("heavy", 150, 27420), ("light", 92, 10814)]


def test_summary_healing_and_regen_intervals(demo: Demo) -> None:
    summary = demo.summary()
    healing = summary["healing"]
    assert (healing["interval_start_s"] < healing["interval_end_s"]).all()
    assert healing["amount"].ge(0).all()
    assert healing["amount"].eq(0).any()
    assert healing.group_by("stat_type").agg(
        pl.len().alias("rows"), pl.col("amount").sum()
    ).sort("stat_type").rows() == [("healing", 670, 231435), ("regen", 1312, 224645)]
    for stat_type in ("healing", "regen"):
        recorded = summary["damage"].filter(
            (pl.col("stat_type") == stat_type) & ~pl.col("is_category")
        )
        assert (
            healing.filter(pl.col("stat_type") == stat_type)["amount"].sum()
            == recorded["damage"].sum()
        )
    for snapshot in summary["snapshots"].iter_rows(named=True):
        rows = healing.filter(
            (pl.col("interval_end_s") == snapshot["snapshot_time_s"])
            & (pl.col("healer_steam_id") == snapshot["steam_id"])
            & (pl.col("stat_type") == "healing")
        )
        if rows.is_empty():
            continue
        assert rows["total"].sum() == snapshot["player_healing"]
        assert (
            rows.filter(pl.col("target_steam_id") == snapshot["steam_id"])[
                "total"
            ].sum()
            == snapshot["self_healing"]
        )


def test_summary_soul_sources_and_identity(
    demo: Demo, final_summary: pl.DataFrame
) -> None:
    player = final_summary.filter(pl.col("steam_id") == MCGINNIS).row(0, named=True)
    assert (
        player["player_healing"],
        player["self_healing"],
        player["teammate_healing"],
    ) == (18346, 13853, 4493)
    summary = demo.summary()
    sources = summary["gold_sources"].filter(
        (pl.col("steam_id") == MCGINNIS) & (pl.col("snapshot_time_s") == 2952)
    )
    assert sources.filter(pl.col("source_id") == 1).select(
        "source_name", "gold", "gold_orbs", "damage"
    ).rows() == [("k_ePlayers", 7385, 0, 52244)]
    assert sources.filter(pl.col("source_id") == 6).select(
        "gold", "gold_orbs"
    ).rows() == [(6774, None)]
    roster = demo.players.select("steam_id", "hero_id")
    for name in ("snapshots", "last_hits", "gold_sources"):
        identities = summary[name].select("steam_id", "hero_id").unique()
        assert identities["steam_id"].null_count() == 0
        assert identities.join(
            roster, on=["steam_id", "hero_id"], how="anti"
        ).is_empty()
