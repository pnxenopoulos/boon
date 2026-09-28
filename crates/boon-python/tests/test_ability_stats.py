"""Ability query validation and stable table schemas."""

import json

import polars as pl
import pytest
from boon import AbilityStat, CalculationError, data, rulesets
from boon.ability_stats import calculate_ability_stats, imbues


class RecordedResult:
    def __init__(self):
        self.calls = []

    def _imbues(self, directory, ticks, **kwargs):
        self.calls.append((directory, ticks, kwargs))
        return json.dumps({"bindings": [], "effects": [], "metadata": {}})

    def _calculate_ability_stats(self, directory, ticks, **kwargs):
        self.calls.append((directory, ticks, kwargs))
        return json.dumps({"values": [], "contributions": [], "metadata": {}})


@pytest.mark.parametrize("function", [imbues, calculate_ability_stats])
@pytest.mark.parametrize(
    "options",
    [
        {"ticks": []},
        {"ticks": -1},
        {"ticks": [True]},
        {"ticks": 1, "players": [-1]},
        {"ticks": 1, "players": [2**32]},
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
            RecordedResult(), ticks=50, data_version="1234", **options
        )


def test_empty_tables_keep_types_and_selected_version(monkeypatch, tmp_path):
    versions = []
    monkeypatch.setattr(
        data, "update", lambda version: versions.append(version) or tmp_path
    )
    demo = RecordedResult()
    result = imbues(demo, ticks=50, players=[2], data_version="1234")
    assert result.bindings.schema["ability_id"] == pl.UInt32
    assert result.effects.schema["value"] == pl.Float64
    assert result.metadata == {"data_version": "1234"}
    assert demo.calls[0] == (tmp_path, [50], {"players": [2]})
    stats = calculate_ability_stats(
        demo,
        ticks=[50, 60],
        data_version="1234",
        include_items=True,
        explain=True,
        strict=False,
    )
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
