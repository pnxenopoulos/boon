"""Match recorded item changes to catalog component links."""

import json
from collections import Counter
from functools import lru_cache
from pathlib import Path
from typing import TYPE_CHECKING

import polars as pl

from boon import data

if TYPE_CHECKING:
    from boon import Demo


@lru_cache(maxsize=8)
def _components(path: Path, signature: tuple[int, int]) -> dict[int, tuple[int, ...]]:
    # File signatures prevent stale links after a catalog reinstall.
    catalog = json.loads(path.read_text())
    if catalog["catalog"] != "abilities":
        raise ValueError("expected an abilities catalog")
    records = catalog["records"]
    ids = {record["ability_name"]: record["ability_id"] for record in records}
    return {
        record["ability_id"]: tuple(
            sorted(
                {
                    ids[name]
                    for name in record.get("definition", {}).get(
                        "m_vecComponentItems", []
                    )
                    if name in ids
                }
            )
        )
        for record in records
    }


def _upgrade_sources(
    frame: pl.DataFrame, components: dict[int, tuple[int, ...]]
) -> pl.Series:
    events = frame.select("tick", "steam_id", "ability_id", "change").rows()
    counts = Counter(events)
    candidates = [
        [
            component
            for component in components.get(ability_id, ())
            if component != ability_id
            and counts[tick, steam_id, component, "sold"] == 1
        ]
        if steam_id and change == "purchased"
        else []
        for tick, steam_id, ability_id, change in events
    ]
    # A component sale must belong to only one candidate purchase. Neither event
    # order nor a matching hero ID proves which player made the transaction.
    claims = Counter(
        (tick, steam_id, component)
        for (tick, steam_id, _, _), sources in zip(events, candidates, strict=True)
        for component in sources
    )
    return pl.Series(
        "upgraded_from_ability_ids",
        [
            [
                component
                for component in sources
                if claims[tick, steam_id, component] == 1
            ]
            if counts[tick, steam_id, ability_id, change] == 1
            else []
            for (tick, steam_id, ability_id, change), sources in zip(
                events, candidates, strict=True
            )
        ],
        dtype=pl.List(pl.UInt32),
    )


def get_item_purchases(
    demo: "Demo", /, *, data_version: str | None = None
) -> pl.DataFrame:
    """Read item changes and match components sold by the same player and tick.

    Keep the recorded change values. upgraded_from_ability_ids contains catalog
    component IDs for unambiguous purchase/sale pairs, or an empty list. Missing
    Steam IDs and component links do not produce inferred matches.

    Use the newest installed boon-data version by default; download latest if
    none is installed. An explicit data_version is downloaded if missing.
    """
    frame = demo._item_purchases
    if frame.is_empty():
        return frame.with_columns(
            pl.Series("upgraded_from_ability_ids", [], dtype=pl.List(pl.UInt32))
        )
    path = data.catalog_path("abilities", data_version)
    try:
        stat = path.stat()
        components = _components(path, (stat.st_mtime_ns, stat.st_size))
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise data.DataError(f"could not read item components: {error}") from error
    return frame.with_columns(_upgrade_sources(frame, components))
