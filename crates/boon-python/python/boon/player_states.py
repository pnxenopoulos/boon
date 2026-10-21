"""Read recorded player state names from a selected boon-data version."""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING

import polars as pl

from boon import data
from boon._selection import select_ticks, validate_steam_ids, validate_version

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
    """Read recorded Deadlock state names from a selected boon-data catalog.

    Args:
        demo: Boon Demo to query.
        data_version: Explicit Deadlock client version from `boon versions`. Download and verify it if missing.
        ticks: One exact tick, a sequence of ticks, or None for every recorded tick. An empty sequence returns no rows.
        steam_ids: Steam account IDs from `demo.players`. None selects all players; an empty sequence selects none.

    Returns:
        Polars DataFrame with tick, steam_id, hero_id, three state-name lists,
        and the corresponding unknown-bit lists. Missing masks and IDs stay null.

    Raises:
        ValueError: A version, tick, or account selection is invalid, or a requested tick is absent.
        data.DataError: The selected state catalog cannot be downloaded or read.

    Notes:
        Names are sorted and omit MODIFIER_STATE_. The states column uses the
        recorded predicted-state mask; Boon does not predict player states.
        Enabled and disabled masks remain separate. No mask priority is inferred.
        An empty name list can still have unknown bits. Read unknown_states,
        unknown_enabled_states, and unknown_disabled_states before assuming no state.

    Examples:
        >>> from boon import Demo
        >>> demo = Demo("108575009.dem", preload=False)
        >>> states = demo.player_states(ticks=187554, data_version="6712")
        >>> print(states.select("steam_id", "hero_id", "states"))
    """
    validate_version(data_version)
    if not data_version:
        raise ValueError("data_version must be a version from `boon versions`")
    selected = None if ticks is None else select_ticks(ticks, allow_empty=True)
    validate_steam_ids(steam_ids)
    return demo._player_states(
        data.update(data_version), ticks=selected, steam_ids=steam_ids
    )
