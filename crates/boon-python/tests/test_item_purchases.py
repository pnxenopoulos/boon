"""Catalog-backed item upgrade ancestry without changing recorded events."""

import json
from pathlib import Path
from types import SimpleNamespace

import polars as pl
import pytest
from boon import data
from boon.item_purchases import _upgrade_sources, get_item_purchases
from polars.testing import assert_frame_equal


@pytest.fixture
def components():
    return {10: (20, 21), 11: (20,), 30: (30,)}


@pytest.mark.parametrize(
    ("events", "expected"),
    [
        ([(1, 100, 10, "purchased"), (1, 100, 20, "sold")], [[20], []]),
        (
            [(1, 100, 21, "sold"), (1, 100, 20, "sold"), (1, 100, 10, "purchased")],
            [[], [], [20, 21]],
        ),
        (
            [(1, 100, 10, "purchased"), (1, 200, 20, "sold"), (2, 100, 21, "sold")],
            [[], [], []],
        ),
        (
            [
                (1, 100, 10, "purchased"),
                (1, 100, 11, "purchased"),
                (1, 100, 20, "sold"),
            ],
            [[], [], []],
        ),
        (
            [
                (1, 100, 10, "purchased"),
                (1, 100, 10, "purchased"),
                (1, 100, 20, "sold"),
            ],
            [[], [], []],
        ),
        (
            [(1, 100, 10, "purchased"), (1, 100, 20, "sold"), (1, 100, 20, "sold")],
            [[], [], []],
        ),
        ([(1, None, 10, "purchased"), (1, None, 20, "sold")], [[], []]),
        ([(1, 0, 10, "purchased"), (1, 0, 20, "sold")], [[], []]),
        ([(1, 100, 10, "upgraded"), (1, 100, 20, "sold")], [[], []]),
        ([(1, 100, 99, "purchased"), (1, 100, 20, "sold")], [[], []]),
        ([(1, 100, 30, "purchased"), (1, 100, 30, "sold")], [[], []]),
        ([], []),
    ],
    ids=[
        "component",
        "multiple-components-sale-first",
        "different-player-or-tick",
        "shared-sale",
        "duplicate-purchase",
        "duplicate-sale",
        "missing-steam-id",
        "zero-steam-id",
        "in-place-upgrade",
        "unknown-item",
        "self-link",
        "empty",
    ],
)
def test_components_need_a_unique_player_and_tick_match(components, events, expected):
    frame = pl.DataFrame(
        events,
        schema={
            "tick": pl.Int32,
            "steam_id": pl.UInt64,
            "ability_id": pl.UInt32,
            "change": pl.String,
        },
        orient="row",
    )
    sources = _upgrade_sources(frame, components)
    assert sources.dtype == pl.List(pl.UInt32)
    assert sources.to_list() == expected


def test_catalog_selection_and_refresh(monkeypatch, tmp_path):
    path = tmp_path / "abilities.json"
    catalog = json.loads(
        (Path(__file__).parent / "catalogs" / "item-components.json").read_text()
    )
    path.write_text(json.dumps(catalog))
    versions = []

    def catalog_path(name, version):
        assert name == "abilities"
        versions.append(version)
        return path

    monkeypatch.setattr(data, "catalog_path", catalog_path)
    raw = pl.DataFrame(
        {
            "tick": [1, 1],
            "steam_id": [100, 100],
            "hero_id": [13, 13],
            "ability_id": [2356412290, 1548066885],
            "change": ["purchased", "sold"],
        }
    )
    demo = SimpleNamespace(_item_purchases=raw)
    result = get_item_purchases(demo)
    assert result["upgraded_from_ability_ids"].to_list() == [[1548066885], []]
    assert_frame_equal(result.drop("upgraded_from_ability_ids"), raw)
    catalog["records"][1]["definition"]["m_vecComponentItems"] = []
    path.write_text(json.dumps(catalog))
    assert get_item_purchases(demo, data_version="6712")[
        "upgraded_from_ability_ids"
    ].to_list() == [[], []]
    assert versions == [None, "6712"]
    path.write_text("{}")
    with pytest.raises(data.DataError, match="could not read item components"):
        get_item_purchases(demo)


def test_empty_events_do_not_need_catalogs():
    demo = SimpleNamespace(_item_purchases=pl.DataFrame(schema={"tick": pl.Int32}))
    result = get_item_purchases(demo)
    assert result.is_empty()
    assert result.schema["upgraded_from_ability_ids"] == pl.List(pl.UInt32)


def test_haze_titanic_magazine_consumes_extended_magazine(demo, monkeypatch):
    path = Path(__file__).parent / "catalogs" / "item-components.json"
    monkeypatch.setattr(data, "catalog_path", lambda name, version: path)
    purchases = demo.get_item_purchases(data_version="6712")
    pair = purchases.filter(
        (pl.col("tick") == 23567) & (pl.col("steam_id") == 76561199122465399)
    )
    assert pair.select(
        "hero_id", "ability_id", "change", "upgraded_from_ability_ids"
    ).rows() == [
        (13, 2356412290, "purchased", [1548066885]),
        (13, 1548066885, "sold", []),
    ]
    assert pair.schema["steam_id"] == pl.UInt64
    assert pair.schema["upgraded_from_ability_ids"] == pl.List(pl.UInt32)
    assert_frame_equal(demo.item_purchases, purchases)
