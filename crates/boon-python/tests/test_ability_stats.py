"""Ability query validation and stable table schemas."""

import json
from typing import cast

import polars as pl
import pytest
from boon import AbilityStat, CalculationError, Demo, StatMode, data, rulesets
from boon.ability_stats import calculate_ability_stats, imbues


class RecordedResult:
    """Native-method double; cast to Demo only at the public API boundary."""

    def __init__(self):
        self.calls = []

    def _imbues(self, directory, ticks, **kwargs):
        self.calls.append((directory, ticks, kwargs))
        return json.dumps({"bindings": [], "effects": [], "metadata": {}})

    def _calculate_ability_stats(self, directory, ticks, **kwargs):
        self.calls.append((directory, ticks, kwargs))
        return json.dumps(
            {"values": [], "contributions": [], "metadata": {"mode": kwargs["mode"]}}
        )


@pytest.mark.parametrize("function", [imbues, calculate_ability_stats])
@pytest.mark.parametrize(
    "options",
    [
        {"ticks": []},
        {"ticks": -1},
        {"ticks": 2**31 - 1},
        {"ticks": 2**40},
        {"ticks": [True]},
        {"ticks": 1, "steam_ids": [-1]},
        {"ticks": 1, "steam_ids": [0]},
        {"ticks": 1, "steam_ids": [True]},
        {"ticks": 1, "steam_ids": [76561197999389679.0]},
        {"ticks": 1, "steam_ids": [2**64]},
        {"ticks": 1, "data_version": None},
    ],
)
def test_invalid_selection_does_not_download(monkeypatch, function, options):
    monkeypatch.setattr(data, "update", lambda _: pytest.fail("unexpected download"))
    with pytest.raises(ValueError):
        function(RecordedResult(), **({"data_version": "1234"} | options))


@pytest.mark.parametrize(
    "options",
    [
        {"stats": []},
        {"mode": "effective"},
        {"mode": None},
        {"stats": ["unknown"]},
        {"abilities": [-1]},
        {"rulesets": {}},
        {"rulesets": {AbilityStat.RANGE_BONUS: rulesets.range_bonus.v1}},
    ],
)
def test_invalid_stat_query_does_not_download(monkeypatch, options):
    monkeypatch.setattr(data, "update", lambda _: pytest.fail("unexpected download"))
    with pytest.raises(ValueError):
        calculate_ability_stats(
            cast(Demo, RecordedResult()), ticks=50, data_version="1234", **options
        )


def test_empty_tables_keep_types_and_selected_version(monkeypatch, tmp_path):
    versions = []
    monkeypatch.setattr(
        data, "update", lambda version: versions.append(version) or tmp_path
    )
    demo = RecordedResult()
    result = imbues(
        cast(Demo, demo), ticks=50, steam_ids=[76561197999389679], data_version="1234"
    )
    assert result.bindings.schema["steam_id"] == pl.UInt64
    assert result.effects.schema["steam_id"] == pl.UInt64
    assert result.bindings.schema["ability_id"] == pl.UInt32
    assert result.effects.schema["value"] == pl.Float64
    assert result.metadata == {"data_version": "1234"}
    assert demo.calls[0] == (tmp_path, [50], {"steam_ids": [76561197999389679]})
    stats = calculate_ability_stats(
        cast(Demo, demo),
        ticks=[50, 60],
        data_version="1234",
        include_items=True,
        explain=True,
        strict=False,
    )
    assert stats.metadata["mode"] == "current"
    assert demo.calls[1][2]["mode"] == "current"
    assert stats.values.schema["mode"] == pl.String
    assert stats.contributions.schema["mode"] == pl.String
    assert stats.values.schema["steam_id"] == pl.UInt64
    assert stats.contributions.schema["steam_id"] == pl.UInt64
    assert stats.values.schema["value"] == pl.Float64
    assert stats.contributions.schema["included"] == pl.Boolean
    assert stats.contributions.schema["source_ability_id"] == pl.UInt32
    assert versions == ["1234", "1234"]
    assert demo.calls[1][2]["include_items"] is True
    assert demo.calls[1][2]["strict"] is False


@pytest.mark.parametrize("function", [imbues, calculate_ability_stats])
def test_native_errors_keep_their_message(monkeypatch, tmp_path, function):
    monkeypatch.setattr(data, "update", lambda _: tmp_path)

    class Failure:
        def _imbues(self, *args, **kwargs):
            raise ValueError("missing replay selection")

        _calculate_ability_stats = _imbues

    with pytest.raises(CalculationError, match="missing replay selection"):
        function(Failure(), ticks=50, data_version="1234")


def test_native_steam_selection_and_imbue_identity(demo: Demo, tmp_path, monkeypatch):
    # Sample a recorded tick; fixture lengths differ across CI jobs.
    ticks = demo.ability_ticks["tick"].unique().sort()
    tick = ticks[len(ticks) // 2]
    for name in ("heroes", "abilities", "modifiers", "misc"):
        (tmp_path / f"{name}.json").write_text(
            json.dumps(
                {
                    "catalog": name,
                    "client_version": "1234",
                    "source_commit": "a" * 40,
                    "records": [],
                }
            )
        )
    monkeypatch.setattr(data, "update", lambda _: tmp_path)
    all_imbues = demo.imbues(ticks=tick, data_version="1234")
    assert "mode" not in all_imbues.metadata
    assert "player_slot" not in all_imbues.bindings.columns
    assert "player_slot" not in all_imbues.effects.columns
    assert all_imbues.bindings.height > 0
    steam_id = all_imbues.bindings["steam_id"][0]
    assert steam_id in demo.players["steam_id"]
    selected = demo.imbues(ticks=tick, steam_ids=[steam_id], data_version="1234")
    assert selected.bindings.equals(
        all_imbues.bindings.filter(pl.col("steam_id") == steam_id)
    )
    assert selected.effects.equals(
        all_imbues.effects.filter(pl.col("steam_id") == steam_id)
    )
    # Selection validation does not require a mid-match replay pass.
    first_tick = ticks[0]
    for function in (demo.imbues, demo.calculate_ability_stats):
        with pytest.raises(CalculationError, match="Steam ID 1 has no hero"):
            function(ticks=first_tick, steam_ids=[1], data_version="1234")
    assert demo.imbues(
        ticks=first_tick, steam_ids=[], data_version="1234"
    ).bindings.is_empty()
    for mode in StatMode:
        result = demo.calculate_ability_stats(
            ticks=first_tick,
            steam_ids=[],
            data_version="1234",
            mode=mode,
        )
        assert result.values.is_empty()
        assert "player_slot" not in result.values.columns
        assert "player_slot" not in result.contributions.columns
        assert result.metadata["mode"] == mode
        assert result.values.schema["mode"] == pl.String


@pytest.mark.parametrize(
    "mode", ["baseline", StatMode.BASELINE, "current", StatMode.CURRENT]
)
def test_mode_reaches_native_ability_query(monkeypatch, tmp_path, mode):
    monkeypatch.setattr(data, "update", lambda _: tmp_path)
    demo = RecordedResult()
    result = calculate_ability_stats(
        cast(Demo, demo),
        ticks=50,
        data_version="1234",
        mode=mode,
    )
    assert demo.calls[0][2]["mode"] == mode
    assert result.metadata["mode"] == mode
    assert result.values.schema["mode"] == pl.String
    assert result.contributions.schema["mode"] == pl.String
