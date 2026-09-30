"""Validate tick, catalog version, and Steam account selections."""

from collections.abc import Sequence


def validate_version(version: str) -> None:
    """Require an explicit catalog version before a query can download data."""
    if not isinstance(version, str):
        raise ValueError(
            "data_version must be an explicit client version from `boon versions`"
        )


def select_ticks(ticks: int | Sequence[int], *, allow_empty: bool = False) -> list[int]:
    """Normalize exact ticks and reserve the final Int32 value for the end bound."""
    selected = [ticks] if isinstance(ticks, int) else list(ticks)
    if (not selected and not allow_empty) or any(
        type(tick) is not int or not 0 <= tick < 2**31 - 1 for tick in selected
    ):
        raise ValueError("provide nonnegative integer ticks below 2147483647")
    return selected


def validate_steam_ids(steam_ids: Sequence[int] | None) -> None:
    """Reject absent-account sentinels and values outside the wire integer range."""
    if steam_ids is not None and any(
        type(value) is not int or not 0 < value < 2**64 for value in steam_ids
    ):
        raise ValueError("steam_ids must contain nonzero unsigned 64-bit integers")
