"""State query validation, catalog acquisition, and native replay checks."""

import io
import json
from typing import cast

import polars as pl
import pytest
from boon import Demo, data
from boon.player_states import player_states
from catalog_helpers import VERSION, release
from polars.testing import assert_frame_equal


class RecordedResult:
    """Native method double, cast to Demo only at the public boundary."""

    def __init__(self):
        self.calls = []

    def _player_states(self, directory, *, ticks, steam_ids):
        self.calls.append((directory, ticks, steam_ids))
        return pl.DataFrame({"states": [["SPRINTING"]]})


def test_download_and_reuse_with_steam_ids(monkeypatch, tmp_path):
    index, files = release()
    requests = []

    def request(url):
        requests.append(url)
        return io.BytesIO(
            json.dumps(index).encode() if url == data.INDEX_URL else files[url]
        )

    monkeypatch.setattr(data, "BOON_DATA_DIR", tmp_path)
    monkeypatch.setattr(data, "_request", request)
    recorded = RecordedResult()
    result = player_states(
        cast(Demo, recorded), ticks=50, steam_ids=[2**63 + 1], data_version=VERSION
    )
    assert result.to_dicts() == [{"states": ["SPRINTING"]}]
    assert recorded.calls == [(tmp_path / VERSION, [50], [2**63 + 1])]
    count = len(requests)
    player_states(cast(Demo, recorded), data_version=VERSION)
    assert recorded.calls[-1] == (tmp_path / VERSION, None, None)
    assert len(requests) == count


@pytest.mark.parametrize(
    "options",
    [
        {"ticks": -1},
        {"ticks": True},
        {"ticks": [1.5]},
        {"ticks": [2**31 - 1]},
        {"steam_ids": [0]},
        {"steam_ids": [-1]},
        {"steam_ids": [True]},
        {"steam_ids": [2**64]},
        {"steam_ids": [1.5]},
        {"data_version": ""},
    ],
)
def test_invalid_query_is_rejected_before_download(monkeypatch, options):
    def download(version):
        pytest.fail("invalid input must not download data")

    monkeypatch.setattr(data, "update", download)
    with pytest.raises(ValueError):
        player_states(
            cast(Demo, RecordedResult()), **({"data_version": VERSION} | options)
        )


@pytest.fixture
def state_demo(demo_paths, tmp_path, monkeypatch):
    if not demo_paths:
        pytest.skip("no demo fixtures")
    # Synthetic names verify lookup behavior without a build-specific enum table.
    (tmp_path / "modifiers.json").write_text(
        json.dumps(
            {
                "catalog": "modifiers",
                "client_version": VERSION,
                "source_commit": "a" * 40,
                "modifier_states": {"0": "MODIFIER_STATE_EXAMPLE"},
            }
        )
    )
    monkeypatch.setattr(data, "update", lambda version: tmp_path)
    return Demo(str(demo_paths[0]), preload=False)


def test_native_empty_schema_and_missing_tick(state_demo):
    for selection in ({"ticks": []}, {"steam_ids": []}):
        result = state_demo.player_states(data_version=VERSION, **selection)
        assert result.is_empty()
        assert "player_slot" not in result.columns
        assert result.schema["steam_id"] == pl.UInt64
        assert result.schema["states"] == pl.List(pl.String)
        assert result.schema["unknown_states"] == pl.List(pl.UInt32)
    with pytest.raises(ValueError, match="absent from the demo"):
        state_demo.player_states(ticks=2_000_000_000, data_version=VERSION)


def test_native_full_query_matches_exact_ticks_and_preserves_unknown_bits(state_demo):
    steam_id = state_demo.players["steam_id"][0]
    full = state_demo.player_states(steam_ids=[steam_id], data_version=VERSION)
    assert full.height > 0
    assert full["steam_id"].unique().to_list() == [steam_id]
    ticks = full["tick"].gather([0, full.height // 2, full.height - 1]).to_list()
    selected = state_demo.player_states(
        ticks=[*reversed(ticks), ticks[0]], steam_ids=[steam_id], data_version=VERSION
    )
    assert_frame_equal(selected, full.filter(pl.col("tick").is_in(ticks)))
    assert full["unknown_states"].list.len().max() > 0


def test_old_catalog_gives_actionable_error(demo_paths):
    if not demo_paths:
        pytest.skip("no demo fixtures")
    demo = Demo(str(demo_paths[0]), preload=False)
    with pytest.raises(ValueError, match="boon versions.*boon get"):
        demo.player_states(ticks=1000, data_version=VERSION)
