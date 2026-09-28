"""Stat query validation, acquisition, result types, and native errors."""

import io
import json

import polars as pl
import pytest
from boon import CalculationError, Demo, HeroStat, data, rulesets
from boon.hero_stats import calculate_hero_stats
from catalog_helpers import VERSION, release


class RecordedResult:
    def __init__(self):
        self.calls = []

    def _calculate_hero_stats(self, directory, ticks, **kwargs):
        self.calls.append((directory, ticks, kwargs))
        return json.dumps(
            {
                "values": [
                    {
                        "tick": ticks[0],
                        "player_slot": 0,
                        "hero_id": 999,
                        "stat": "clip_size",
                        "value": 36,
                        "unit": "rounds",
                        "ruleset": "clip_size.v1",
                        "status": "calculated",
                        "diagnostic": None,
                    }
                ],
                "contributions": [],
                "metadata": {"source_commit": "a" * 40, "snapshot_version": VERSION},
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
        demo, ticks=50, players=[0], data_version=VERSION, explain=True
    )
    assert result.values["value"].item() == 36
    assert result.values.schema["value"] == pl.Float64
    assert result.metadata["data_version"] == VERSION
    assert result.contributions.schema["modifier_serial"] == pl.UInt32
    assert demo.calls[0] == (
        tmp_path / VERSION,
        [50],
        {
            "stats": ["clip_size"],
            "players": [0],
            "heroes": None,
            "explain": True,
            "strict": True,
        },
    )
    downloads = len(requests)
    calculate_hero_stats(demo, ticks=[50, 60], data_version=VERSION)
    assert len(requests) == downloads


@pytest.mark.parametrize(
    "options",
    [
        {"ticks": []},
        {"ticks": -1},
        {"ticks": [True]},
        {"ticks": 1, "stats": []},
        {"ticks": 1, "stats": ["unimplemented_stat"]},
        {"ticks": 1, "players": [-1]},
        {"ticks": 1, "rulesets": {}},
        {
            "ticks": 1,
            "rulesets": {
                HeroStat.CLIP_SIZE: rulesets.Rule(
                    "clip_size", "clip_size.v2", 2, "future"
                )
            },
        },
    ],
)
def test_invalid_queries_fail_before_download(monkeypatch, options):
    def fail(*args, **kwargs):
        pytest.fail("invalid query attempted a download")

    monkeypatch.setattr(data, "update", fail)
    with pytest.raises(ValueError):
        calculate_hero_stats(RecordedResult(), data_version=VERSION, **options)


def test_unknown_version_does_not_fall_back():
    with pytest.raises(data.DataError):
        calculate_hero_stats(RecordedResult(), ticks=50, data_version="latest")


def test_ammo_alias():
    assert HeroStat.AMMO is HeroStat.CLIP_SIZE


def test_native_error_is_exposed_as_calculation_error(monkeypatch, tmp_path):
    class Unresolved:
        def _calculate_hero_stats(self, *args, **kwargs):
            raise ValueError("unsupported property scaling")

    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    with pytest.raises(CalculationError, match="unsupported property scaling"):
        calculate_hero_stats(Unresolved(), ticks=50, data_version=VERSION)


def test_old_catalog_has_actionable_native_error(demo_paths):
    if not demo_paths:
        pytest.skip("no demo fixtures")
    demo = Demo(str(demo_paths[0]), preload=False)
    with pytest.raises(CalculationError, match="boon versions.*boon get"):
        demo.calculate_hero_stats(ticks=1000, data_version=VERSION)


def test_native_missing_ticks_and_empty_result(demo_paths, tmp_path, monkeypatch):
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
        heroes=[],
        data_version=VERSION,
        stats=[
            HeroStat.AMMO,
            HeroStat.BULLET_VELOCITY,
            HeroStat.WEAPON_DAMAGE,
            HeroStat.MELEE_DISTANCE,
            HeroStat.RELOAD_TIME,
            HeroStat.FIRE_RATE,
            HeroStat.FALLOFF_START,
            HeroStat.FALLOFF_END,
            HeroStat.LIGHT_MELEE_DAMAGE,
            HeroStat.HEAVY_MELEE_DAMAGE,
            HeroStat.SLIDE_DISTANCE,
            HeroStat.BULLET_EVASION,
            HeroStat.DEBUFF_RESIST,
            HeroStat.BULLET_RESIST,
            HeroStat.SPIRIT_RESIST,
            HeroStat.MELEE_RESIST,
            HeroStat.BULLET_LIFESTEAL,
            HeroStat.SPIRIT_LIFESTEAL,
            HeroStat.MELEE_LIFESTEAL,
            HeroStat.GRAVITY_SCALE,
            HeroStat.STAMINA,
            HeroStat.STAMINA_COOLDOWN,
            HeroStat.DASH_SPEED,
            HeroStat.DASH_DURATION,
            HeroStat.AIR_DASH_SPEED,
            HeroStat.AIR_DASH_DURATION,
            HeroStat.MOVE_SPEED,
            HeroStat.SPRINT_SPEED,
        ],
    )
    assert {rule["id"] for rule in result.metadata["rulesets"]} == {
        "clip_size.v1",
        "bullet_velocity.v1",
        "weapon_damage.v1",
        "melee_distance.v1",
        "reload_time.v1",
        "fire_rate.v1",
        "falloff_start.v1",
        "falloff_end.v1",
        "light_melee_damage.v1",
        "heavy_melee_damage.v1",
        "slide_distance.v1",
        "bullet_evasion.v1",
        "debuff_resist.v1",
        "bullet_resist.v1",
        "spirit_resist.v1",
        "melee_resist.v1",
        "bullet_lifesteal.v1",
        "spirit_lifesteal.v1",
        "melee_lifesteal.v1",
        "gravity_scale.v1",
        "stamina.v1",
        "stamina_cooldown.v1",
        "dash_speed.v1",
        "dash_duration.v1",
        "air_dash_speed.v1",
        "air_dash_duration.v1",
        "move_speed.v1",
        "sprint_speed.v1",
    }
    assert result.values.is_empty()
    assert result.values.schema["tick"] == pl.Int32
    assert result.contributions.is_empty()


def test_data_version_cannot_silently_select_latest(monkeypatch):
    def fail(*args, **kwargs):
        pytest.fail("query attempted an implicit latest download")

    monkeypatch.setattr(data, "update", fail)
    with pytest.raises(ValueError, match="explicit client version"):
        calculate_hero_stats(RecordedResult(), ticks=50, data_version=None)


@pytest.mark.parametrize(
    "stats",
    [
        [HeroStat.BULLET_VELOCITY],
        [HeroStat.WEAPON_DAMAGE],
        [HeroStat.MELEE_DISTANCE],
        [HeroStat.RELOAD_TIME],
        [HeroStat.FIRE_RATE],
        [HeroStat.DEBUFF_RESIST],
        [HeroStat.FALLOFF_START],
        [HeroStat.FALLOFF_END],
        [HeroStat.LIGHT_MELEE_DAMAGE, HeroStat.HEAVY_MELEE_DAMAGE],
        [HeroStat.SLIDE_DISTANCE, HeroStat.BULLET_EVASION, HeroStat.GRAVITY_SCALE],
        [HeroStat.FALLOFF_START, HeroStat.FALLOFF_END],
        list(HeroStat),
        [HeroStat.AMMO, HeroStat.BULLET_VELOCITY, HeroStat.MELEE_DISTANCE],
        [HeroStat.AMMO, HeroStat.BULLET_VELOCITY, HeroStat.CLIP_SIZE],
    ],
)
def test_selected_stats_reach_native_query_once(monkeypatch, tmp_path, stats):
    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    demo = RecordedResult()
    calculate_hero_stats(demo, ticks=[50, 60], data_version=VERSION, stats=stats)
    assert len(demo.calls) == 1
    assert demo.calls[0][2]["stats"] == list(
        dict.fromkeys(stat.value for stat in stats)
    )


@pytest.mark.parametrize(
    "chosen",
    [
        {HeroStat.BULLET_VELOCITY: rulesets.clip_size.v1},
        {
            HeroStat.BULLET_VELOCITY: rulesets.Rule(
                "bullet_velocity", "bullet_velocity.v2", 2, "future"
            )
        },
        {
            HeroStat.CLIP_SIZE: rulesets.clip_size.v1,
            HeroStat.BULLET_VELOCITY: rulesets.bullet_velocity.v1,
        },
    ],
)
def test_velocity_rule_validation_precedes_download(monkeypatch, chosen):
    def fail(version):
        pytest.fail("invalid rules attempted a download")

    monkeypatch.setattr(data, "update", fail)
    with pytest.raises(ValueError):
        calculate_hero_stats(
            RecordedResult(),
            ticks=50,
            data_version=VERSION,
            stats=[HeroStat.BULLET_VELOCITY],
            rulesets=chosen,
        )


@pytest.mark.parametrize(
    "stat",
    [
        HeroStat.MELEE_DISTANCE,
        HeroStat.WEAPON_DAMAGE,
        HeroStat.RELOAD_TIME,
        HeroStat.FIRE_RATE,
        HeroStat.FALLOFF_START,
        HeroStat.FALLOFF_END,
        HeroStat.LIGHT_MELEE_DAMAGE,
        HeroStat.HEAVY_MELEE_DAMAGE,
        HeroStat.SLIDE_DISTANCE,
        HeroStat.BULLET_EVASION,
        HeroStat.DEBUFF_RESIST,
        HeroStat.GRAVITY_SCALE,
        HeroStat.MOVE_SPEED,
        HeroStat.SPRINT_SPEED,
    ],
)
@pytest.mark.parametrize("wrong_version", [False, True])
def test_stat_rejects_wrong_rule_before_download(monkeypatch, stat, wrong_version):
    rule = (
        rulesets.Rule(stat.value, f"{stat}.v2", 2, "future")
        if wrong_version
        else rulesets.clip_size.v1
    )

    def fail(version):
        pytest.fail("invalid rules attempted a download")

    monkeypatch.setattr(data, "update", fail)
    with pytest.raises(ValueError, match=f"{stat}.v1"):
        calculate_hero_stats(
            RecordedResult(),
            ticks=50,
            data_version=VERSION,
            stats=[stat],
            rulesets={stat: rule},
        )


@pytest.mark.parametrize("strict", [True, False])
def test_partial_values_and_diagnostics_are_retained(monkeypatch, tmp_path, strict):
    class PartialResult(RecordedResult):
        def _calculate_hero_stats(self, *args, **kwargs):
            payload = json.loads(super()._calculate_hero_stats(*args, **kwargs))
            payload["values"][0].update(
                status="partial",
                diagnostic="ignored modifiers: unresolved modifier ID 123",
            )
            return json.dumps(payload)

    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    demo = PartialResult()
    result = calculate_hero_stats(demo, ticks=50, data_version=VERSION, strict=strict)
    assert result.values["value"].item() == 36
    assert result.values["status"].item() == "partial"
    assert "123" in result.values["diagnostic"].item()
    assert demo.calls[0][2]["strict"] is strict
    assert not demo.calls[0][2]["explain"]


@pytest.mark.parametrize("stat", [HeroStat.FALLOFF_START, HeroStat.FALLOFF_END])
def test_falloff_result_retains_metres_and_fractional_values(
    monkeypatch, tmp_path, stat
):
    class FalloffResult(RecordedResult):
        def _calculate_hero_stats(self, *args, **kwargs):
            payload = json.loads(super()._calculate_hero_stats(*args, **kwargs))
            payload["values"][0].update(
                stat=stat.value, unit="m", value=24.125, ruleset=f"{stat}.v1"
            )
            return json.dumps(payload)

    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    result = calculate_hero_stats(
        FalloffResult(), ticks=50, data_version=VERSION, stats=[stat]
    )
    assert result.values["value"].item() == 24.125
    assert result.values["unit"].item() == "m"
    assert result.values["stat"].item() == stat.value


@pytest.mark.parametrize(
    "stat", [HeroStat.LIGHT_MELEE_DAMAGE, HeroStat.HEAVY_MELEE_DAMAGE]
)
def test_melee_result_retains_damage_units_and_fractional_values(
    monkeypatch, tmp_path, stat
):
    class MeleeResult(RecordedResult):
        def _calculate_hero_stats(self, *args, **kwargs):
            payload = json.loads(super()._calculate_hero_stats(*args, **kwargs))
            payload["values"][0].update(
                stat=stat.value, unit="damage", value=152.012, ruleset=f"{stat}.v1"
            )
            return json.dumps(payload)

    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    result = calculate_hero_stats(
        MeleeResult(), ticks=50, data_version=VERSION, stats=[stat]
    )
    assert result.values["value"].item() == 152.012
    assert result.values["unit"].item() == "damage"
    assert result.values["stat"].item() == stat.value


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
    assert result.values.sort("player_slot")["value"].equals(
        trace.sort("player_slot")["value"]
    )


@pytest.mark.parametrize(
    "stat",
    [
        HeroStat.STAMINA,
        HeroStat.STAMINA_COOLDOWN,
        HeroStat.DASH_SPEED,
        HeroStat.DASH_DURATION,
        HeroStat.AIR_DASH_SPEED,
        HeroStat.AIR_DASH_DURATION,
        HeroStat.MOVE_SPEED,
        HeroStat.SPRINT_SPEED,
    ],
)
def test_movement_rule_selection(monkeypatch, tmp_path, stat):
    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    demo = RecordedResult()
    rule = getattr(rulesets, stat.value).v1
    calculate_hero_stats(
        demo, ticks=50707, data_version=VERSION, stats=[stat], rulesets={stat: rule}
    )
    assert demo.calls[0][2]["stats"] == [stat.value]
    assert rule.documented_on == "2026-09-28"
    with pytest.raises(ValueError, match="select boon.rulesets"):
        calculate_hero_stats(
            demo,
            ticks=50707,
            data_version=VERSION,
            stats=[stat],
            rulesets={stat: rulesets.clip_size.v1},
        )


def test_debuff_resist_rule_and_percentage_output(monkeypatch, tmp_path):
    class Result:
        def _calculate_hero_stats(self, directory, ticks, **kwargs):
            assert kwargs["stats"] == ["debuff_resist"]
            payload = json.loads(
                RecordedResult()._calculate_hero_stats(directory, ticks)
            )
            payload["values"][0].update(
                stat="debuff_resist", value=-8.0, unit="%", ruleset="debuff_resist.v1"
            )
            return json.dumps(payload)

    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    result = calculate_hero_stats(
        Result(),
        ticks=50707,
        data_version=VERSION,
        stats=[HeroStat.DEBUFF_RESIST],
        rulesets={HeroStat.DEBUFF_RESIST: rulesets.debuff_resist.v1},
    )
    assert result.values["value"].item() == -8.0
    assert result.values["unit"].item() == "%"
    assert rulesets.debuff_resist.v1.documented_on == "2026-09-28"


@pytest.mark.parametrize(
    "stat",
    [HeroStat.BULLET_LIFESTEAL, HeroStat.SPIRIT_LIFESTEAL, HeroStat.MELEE_LIFESTEAL],
)
def test_lifesteal_rule_and_percentage_output(monkeypatch, tmp_path, stat):
    class Result:
        def _calculate_hero_stats(self, directory, ticks, **kwargs):
            assert kwargs["stats"] == [stat.value]
            payload = json.loads(
                RecordedResult()._calculate_hero_stats(directory, ticks)
            )
            payload["values"][0].update(
                stat=stat.value, value=45.4, unit="%", ruleset=f"{stat.value}.v1"
            )
            return json.dumps(payload)

    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    rule = getattr(rulesets, stat.value).v1
    result = calculate_hero_stats(
        Result(), ticks=50707, data_version=VERSION, stats=[stat], rulesets={stat: rule}
    )
    assert result.values["value"].item() == 45.4
    assert result.values["unit"].item() == "%"
    assert rule.documented_on == "2026-09-28"
    with pytest.raises(ValueError, match="select boon.rulesets"):
        calculate_hero_stats(
            Result(),
            ticks=50707,
            data_version=VERSION,
            stats=[stat],
            rulesets={stat: rulesets.clip_size.v1},
        )


@pytest.mark.parametrize(
    "stat",
    [
        HeroStat.BULLET_RESIST,
        HeroStat.SPIRIT_RESIST,
        HeroStat.MELEE_RESIST,
        HeroStat.WEAPON_DAMAGE,
    ],
)
def test_rule_retains_negative_percentages(monkeypatch, tmp_path, stat):
    class Result:
        def _calculate_hero_stats(self, directory, ticks, **kwargs):
            assert kwargs["stats"] == [stat.value]
            payload = json.loads(
                RecordedResult()._calculate_hero_stats(directory, ticks)
            )
            payload["values"][0].update(
                stat=stat.value, value=-30.0, unit="%", ruleset=f"{stat.value}.v1"
            )
            return json.dumps(payload)

    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    rule = getattr(rulesets, stat.value).v1
    result = calculate_hero_stats(
        Result(), ticks=50707, data_version=VERSION, stats=[stat], rulesets={stat: rule}
    )
    assert result.values["value"].item() == -30.0
    assert result.values["unit"].item() == "%"
    assert rule.documented_on == "2026-09-28"
    with pytest.raises(ValueError, match="select boon.rulesets"):
        calculate_hero_stats(
            Result(),
            ticks=50707,
            data_version=VERSION,
            stats=[stat],
            rulesets={stat: rulesets.clip_size.v1},
        )
