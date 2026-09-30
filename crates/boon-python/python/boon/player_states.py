"""Read recorded player state names from a selected boon-data version."""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING

import polars as pl

from boon import data
from boon._selection import validate_steam_ids

if TYPE_CHECKING:
    from boon import Demo


def player_states(
    demo: Demo,
    /,
    *,
    data_version: str,
    ticks: int | Sequence[int] | None = None,
    steam_ids: Sequence[int] | None = None,
) -> pl.DataFrame:
    """Read state lists for each player at exact ticks, or all ticks if omitted.

    Names come from the selected catalog. A missing version is downloaded and
    verified. ``states`` is the recorded predicted-state mask; enabled and
    disabled masks are separate columns. No mask precedence is inferred.

    Names are sorted. Empty name lists can still have unknown bits; null means
    the mask is unavailable. Unknown bit indices are retained in ``unknown_states``,
    ``unknown_enabled_states``, and ``unknown_disabled_states``.

    Rows include tick, steam_id, and hero_id. Players without a
    Steam ID retain a row with a null ID. Empty selections return no rows.
    Invalid or absent ticks raise ValueError. This does not calculate stats.
    """
    if not isinstance(data_version, str) or not data_version:
        raise ValueError(
            "data_version must be an explicit version from `boon versions`"
        )
    selected = (
        None if ticks is None else [ticks] if isinstance(ticks, int) else list(ticks)
    )
    if selected is not None and any(
        type(tick) is not int or not 0 <= tick < 2**31 - 1 for tick in selected
    ):
        raise ValueError("ticks must contain nonnegative integers below 2147483647")
    validate_steam_ids(steam_ids)
    return demo._player_states(
        data.update(data_version), ticks=selected, steam_ids=steam_ids
    )
