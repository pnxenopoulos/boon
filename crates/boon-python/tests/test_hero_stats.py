"""Stat query validation, acquisition, result types, and native errors."""

import io
import json
from typing import cast

import polars as pl
import pytest
from boon import CalculationError, Demo, HeroStat, StatMode, data, rulesets
from boon.hero_stats import calculate_hero_stats
from catalog_helpers import VERSION, release


class RecordedResult:
    """Native-method double; cast to Demo only at the public API boundary."""

    def __init__(self, **values):
        self.calls = []
        self.values = values

    def _calculate_hero_stats(self, directory, ticks, **kwargs):
        self.calls.append((directory, ticks, kwargs))
        return json.dumps(
            {
                "values": [
                    {
                        "mode": kwargs["mode"],
                        "tick": ticks[0],
                        "steam_id": 76561197999389679,
                        "player_slot": 0,
                        "hero_id": 999,
                        "stat": "clip_size",
                        "value": 36,
                        "unit": "rounds",
                        "ruleset": "clip_size.v1",
                        "status": "calculated",
                        "diagnostic": None,
                        **self.values,
                    }
                ],
                "contributions": [],
                "metadata": {
                    "mode": kwargs["mode"],
                    "source_commit": "a" * 40,
                    "snapshot_version": VERSION,
                },
            }
        )


def test_query_uses_verified_download_and_reuses_local_catalog(monkeypatch, tmp_path):
    index, files = release()
    requests = []

    def request(url):
        requests.append(url)
        return io.BytesIO(
            json.dumps(index).encode() if url == data.INDEX_URL else files[url]
        )

    monkeypatch.setattr(data, "BOON_DATA_DIR", tmp_path)
    monkeypatch.setattr(data, "_request", request)
    demo = RecordedResult()
    result = calculate_hero_stats(
        cast(Demo, demo),
        ticks=50,
        steam_ids=[76561197999389679],
        data_version=VERSION,
        explain=True,
    )
    assert result.values["steam_id"].item() == 76561197999389679
    assert result.values.schema["steam_id"] == pl.UInt64
    assert result.values["value"].item() == 36
    assert result.values.schema["value"] == pl.Float64
    assert result.metadata["data_version"] == VERSION
    assert result.contributions.schema["modifier_serial"] == pl.UInt32
    assert demo.calls[0] == (
        tmp_path / VERSION,
        [50],
        {
            "stats": ["clip_size"],
            "mode": "current",
            "steam_ids": [76561197999389679],
            "heroes": None,
            "explain": True,
            "strict": True,
        },
    )
    downloads = len(requests)
    calculate_hero_stats(cast(Demo, demo), ticks=[50, 60], data_version=VERSION)
    assert len(requests) == downloads


@pytest.mark.parametrize(
    "options",
    [
        {"ticks": []},
        {"ticks": -1},
        {"ticks": 1, "mode": "permanent"},
        {"ticks": 1, "mode": None},
        {"ticks": [True]},
        {"ticks": 1, "stats": []},
        {"ticks": 1, "stats": ["unimplemented_stat"]},
        {"ticks": 1, "steam_ids": [-1]},
        {"ticks": 1, "steam_ids": [0]},
        {"ticks": 1, "steam_ids": [True]},
        {"ticks": 1, "steam_ids": [76561197999389679.0]},
        {"ticks": 1, "rulesets": {}},
        {
            "ticks": 1,
            "rulesets": {
                HeroStat.CLIP_SIZE: rulesets.clip_size.v1,
                HeroStat.BULLET_VELOCITY: rulesets.bullet_velocity.v1,
            },
        },
    ],
)
def test_invalid_queries_fail_before_download(monkeypatch, options):
    def fail(*args, **kwargs):
        pytest.fail("invalid query attempted a download")

    monkeypatch.setattr(data, "update", fail)
    with pytest.raises(ValueError):
        calculate_hero_stats(
            cast(Demo, RecordedResult()), data_version=VERSION, **options
        )


def test_unknown_version_does_not_fall_back():
    with pytest.raises(data.DataError):
        calculate_hero_stats(
            cast(Demo, RecordedResult()), ticks=50, data_version="latest"
        )


def test_ammo_alias():
    assert HeroStat.AMMO is HeroStat.CLIP_SIZE


def test_native_error_is_exposed_as_calculation_error(monkeypatch, tmp_path):
    class Unresolved:
        def _calculate_hero_stats(self, *args, **kwargs):
            raise ValueError("unsupported property scaling")

    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    with pytest.raises(CalculationError, match="unsupported property scaling"):
        calculate_hero_stats(cast(Demo, Unresolved()), ticks=50, data_version=VERSION)


def test_old_catalog_has_actionable_native_error(demo_paths):
    if not demo_paths:
        pytest.skip("no demo fixtures")
    demo = Demo(str(demo_paths[0]), preload=False)
    with pytest.raises(CalculationError, match="boon versions.*boon get"):
        demo.calculate_hero_stats(ticks=1000, data_version=VERSION)


@pytest.mark.parametrize("mode", list(StatMode))
def test_native_missing_ticks_and_empty_result(demo_paths, tmp_path, monkeypatch, mode):
    if not demo_paths:
        pytest.skip("no demo fixtures")
    for name in ("heroes", "abilities", "modifiers", "misc"):
        (tmp_path / f"{name}.json").write_text(
            json.dumps(
                {
                    "catalog": name,
                    "client_version": VERSION,
                    "source_commit": "a" * 40,
                    "records": [],
                }
            )
        )
    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    demo = Demo(str(demo_paths[0]), preload=False)
    with pytest.raises(CalculationError, match="demo has no tick"):
        demo.calculate_hero_stats(ticks=2_000_000_000, data_version=VERSION)
    result = demo.calculate_hero_stats(
        ticks=1000,
        mode=mode,
        heroes=[],
        data_version=VERSION,
        stats=list(HeroStat),
    )
    assert {rule["id"] for rule in result.metadata["rulesets"]} == {
        f"{stat.value}.v1" for stat in HeroStat
    }
    assert result.metadata["mode"] == mode
    assert result.values.schema["mode"] == pl.String
    assert result.contributions.schema["mode"] == pl.String
    assert result.values.is_empty()
    assert result.values.schema["tick"] == pl.Int32
    assert result.contributions.is_empty()


def test_data_version_cannot_silently_select_latest(monkeypatch):
    def fail(*args, **kwargs):
        pytest.fail("query attempted an implicit latest download")

    monkeypatch.setattr(data, "update", fail)
    with pytest.raises(ValueError, match="explicit client version"):
        calculate_hero_stats(
            cast(Demo, RecordedResult()),
            ticks=50,
            data_version=None,  # ty: ignore[invalid-argument-type] -- Test runtime validation.
        )


@pytest.mark.parametrize(
    "stats",
    [
        [HeroStat.BULLET_VELOCITY],
        list(HeroStat),
        [HeroStat.AMMO, HeroStat.BULLET_VELOCITY, HeroStat.CLIP_SIZE],
    ],
)
@pytest.mark.parametrize("explicit_rules", [False, True])
def test_selected_stats_reach_native_query_once(
    monkeypatch, tmp_path, stats, explicit_rules
):
    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    demo = RecordedResult()
    chosen = {stat: getattr(rulesets, stat.value).v1 for stat in stats}
    calculate_hero_stats(
        cast(Demo, demo),
        ticks=[50, 60],
        data_version=VERSION,
        stats=stats,
        rulesets=chosen if explicit_rules else None,
    )
    assert len(demo.calls) == 1
    assert demo.calls[0][2]["stats"] == list(
        dict.fromkeys(stat.value for stat in stats)
    )


@pytest.mark.parametrize(
    "rule",
    [
        rulesets.clip_size.v1,
        rulesets.Rule("bullet_velocity", "bullet_velocity.v2", 2, "future"),
    ],
    ids=["wrong-stat", "wrong-version"],
)
def test_stat_rejects_wrong_rule_before_download(monkeypatch, rule):
    def fail(version):
        pytest.fail("invalid rules attempted a download")

    monkeypatch.setattr(data, "update", fail)
    with pytest.raises(ValueError, match="select boon.rulesets.bullet_velocity.v1"):
        calculate_hero_stats(
            cast(Demo, RecordedResult()),
            ticks=50,
            data_version=VERSION,
            stats=[HeroStat.BULLET_VELOCITY],
            rulesets={HeroStat.BULLET_VELOCITY: rule},
        )


@pytest.mark.parametrize("strict", [True, False])
def test_partial_values_and_diagnostics_are_retained(monkeypatch, tmp_path, strict):
    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    demo = RecordedResult(
        status="partial", diagnostic="ignored modifiers: unresolved modifier ID 123"
    )
    result = calculate_hero_stats(
        cast(Demo, demo), ticks=50, data_version=VERSION, strict=strict
    )
    assert result.values["value"].item() == 36
    assert result.values["status"].item() == "partial"
    assert "123" in result.values["diagnostic"].item()
    assert demo.calls[0][2]["strict"] is strict
    assert not demo.calls[0][2]["explain"]


@pytest.mark.parametrize(
    ("stat", "unit", "value"),
    [
        (HeroStat.FALLOFF_START, "m", 24.125),
        (HeroStat.FALLOFF_END, "m", 24.125),
        (HeroStat.LIGHT_MELEE_DAMAGE, "damage", 152.012),
        (HeroStat.HEAVY_MELEE_DAMAGE, "damage", 152.012),
        (HeroStat.DEBUFF_RESIST, "%", -8.0),
        (HeroStat.BULLET_LIFESTEAL, "%", 45.4),
        (HeroStat.SPIRIT_LIFESTEAL, "%", 45.4),
        (HeroStat.MELEE_LIFESTEAL, "%", 45.4),
        (HeroStat.BULLET_RESIST, "%", -30.0),
        (HeroStat.SPIRIT_RESIST, "%", -30.0),
        (HeroStat.MELEE_RESIST, "%", -30.0),
        (HeroStat.WEAPON_DAMAGE, "%", -30.0),
    ],
)
def test_native_values_keep_units_precision_and_sign(
    monkeypatch, tmp_path, stat, unit, value
):
    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    demo = RecordedResult(
        stat=stat.value, unit=unit, value=value, ruleset=f"{stat.value}.v1"
    )
    result = calculate_hero_stats(
        cast(Demo, demo), ticks=50, data_version=VERSION, stats=[stat]
    )
    assert demo.calls[0][2]["stats"] == [stat.value]
    assert result.values.select("stat", "value", "unit", "ruleset").row(0) == (
        stat.value,
        value,
        unit,
        f"{stat.value}.v1",
    )


def test_recorded_gravity_does_not_require_hero_or_modifier_definitions(
    demo_paths, tmp_path, monkeypatch
):
    if not demo_paths:
        pytest.skip("no demo fixtures")
    for name in ("heroes", "abilities", "modifiers", "misc"):
        (tmp_path / f"{name}.json").write_text(
            json.dumps(
                {
                    "catalog": name,
                    "client_version": VERSION,
                    "source_commit": "a" * 40,
                    "records": [],
                }
            )
        )
    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    demo = Demo(str(demo_paths[0]), preload=False)
    result = demo.calculate_hero_stats(
        ticks=10000,
        stats=[HeroStat.GRAVITY_SCALE],
        data_version=VERSION,
        explain=True,
    )
    assert not result.values.is_empty()
    assert result.values["value"].null_count() == 0
    assert result.values["diagnostic"].null_count() == result.values.height
    trace = result.contributions
    assert trace.height == result.values.height
    assert set(trace["source"]) == {"replay/player_pawn"}
    assert set(trace["definition_path"]) == {"m_flGravityScale"}
    assert trace["modifier_serial"].null_count() == trace.height
    assert result.values.sort("steam_id")["value"].equals(
        trace.sort("steam_id")["value"]
    )

    steam_id = result.values["steam_id"][0]
    assert steam_id in demo.players["steam_id"]
    selected = demo.calculate_hero_stats(
        ticks=10000,
        stats=[HeroStat.GRAVITY_SCALE],
        data_version=VERSION,
        steam_ids=[steam_id],
        explain=True,
    )
    assert selected.values.equals(result.values.filter(pl.col("steam_id") == steam_id))
    assert selected.contributions.equals(trace.filter(pl.col("steam_id") == steam_id))
    assert demo.calculate_hero_stats(
        ticks=10000, data_version=VERSION, steam_ids=[]
    ).values.is_empty()
    with pytest.raises(CalculationError, match="Steam ID 1 has no selected hero"):
        demo.calculate_hero_stats(ticks=10000, data_version=VERSION, steam_ids=[1])


def test_missing_steam_id_keeps_slot(monkeypatch, tmp_path):
    monkeypatch.setattr(data, "update", lambda _: tmp_path)
    result = calculate_hero_stats(
        cast(Demo, RecordedResult(steam_id=None)), ticks=50, data_version=VERSION
    )
    assert result.values["steam_id"].item() is None
    assert result.values["player_slot"].item() == 0


def test_old_slot_keyword_is_not_silently_reinterpreted():
    with pytest.raises(TypeError, match="players"):
        calculate_hero_stats(
            cast(Demo, RecordedResult()),
            ticks=50,
            data_version=VERSION,
            players=[0],  # ty: ignore[unknown-argument] -- Reject the old slot keyword.
        )


@pytest.mark.parametrize(
    "mode", ["baseline", StatMode.BASELINE, "current", StatMode.CURRENT]
)
def test_stat_mode_reaches_native_query_and_results(monkeypatch, tmp_path, mode):
    monkeypatch.setattr(data, "update", lambda _: tmp_path)
    demo = RecordedResult()
    result = calculate_hero_stats(
        cast(Demo, demo), ticks=50, data_version=VERSION, mode=mode
    )
    assert demo.calls[0][2]["mode"] == mode
    assert result.values["mode"].item() == mode
    assert result.metadata["mode"] == mode
