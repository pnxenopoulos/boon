"""Validation shared by queries that select Steam accounts."""

from collections.abc import Sequence


def validate_steam_ids(steam_ids: Sequence[int] | None) -> None:
    """Reject absent-account sentinels and values outside the wire integer range."""
    if steam_ids is not None and any(
        type(value) is not int or not 0 < value < 2**64 for value in steam_ids
    ):
        raise ValueError("steam_ids must contain nonzero unsigned 64-bit integers")
