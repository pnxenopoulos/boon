"""Validate tick, catalog version, and Steam account selections."""

from collections.abc import Sequence


def validate_version(version: str) -> None:
    """Require a string client version before a query can download data.

    Args:
        version: Explicit version supplied by the caller.

    Returns:
        None.

    Raises:
        ValueError: The version is not a string.
    """
    if not isinstance(version, str):
        raise ValueError(
            "data_version must be an explicit client version from `boon versions`"
        )


def select_ticks(ticks: int | Sequence[int], *, allow_empty: bool = False) -> list[int]:
    """Validate exact demo ticks and preserve the requested order.

    Args:
        ticks: One integer or a sequence of integers.
        allow_empty: Accept an empty sequence when True.

    Returns:
        List of nonnegative ticks below 2147483647.

    Raises:
        ValueError: The selection is empty when forbidden, or contains an invalid tick.
    """
    selected = [ticks] if isinstance(ticks, int) else list(ticks)
    if (not selected and not allow_empty) or any(
        type(tick) is not int or not 0 <= tick < 2**31 - 1 for tick in selected
    ):
        raise ValueError("provide nonnegative integer ticks below 2147483647")
    return selected


def validate_steam_ids(steam_ids: Sequence[int] | None) -> None:
    """Validate optional unsigned 64-bit Steam account IDs.

    Args:
        steam_ids: None or nonzero integer Steam IDs. Empty sequences are allowed.

    Returns:
        None.

    Raises:
        ValueError: An account ID is not an integer in the valid range.
    """
    if steam_ids is not None and any(
        type(value) is not int or not 0 < value < 2**64 for value in steam_ids
    ):
        raise ValueError("steam_ids must contain nonzero unsigned 64-bit integers")
