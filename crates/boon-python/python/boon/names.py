"""Name maps read from verified boon-data catalogs on first access."""

from functools import lru_cache
from pathlib import Path

from boon import data
from boon._boon import _read_catalog_names


@lru_cache(maxsize=8)
def _read_names(directory: Path, signature: tuple[tuple[int, int], ...]):
    # File signatures prevent stale maps after a forced reinstall.
    return _read_catalog_names(directory)


def _names(version: str | None):
    directory = data.update(data.resolve_version(version))
    try:
        signature = tuple(
            (stat.st_mtime_ns, stat.st_size)
            for name in ("heroes", "abilities", "modifiers", "misc")
            for stat in [(directory / f"{name}.json").stat()]
        )
        return _read_names(directory, signature)
    except (OSError, ValueError) as error:
        raise data.DataError(
            f"could not read boon-data name catalogs: {error}"
        ) from error


def hero_names(version: str | None = None) -> dict[int, str]:
    """Hero IDs to localized names (internal names when localization is absent).

    Use the newest installed boon-data version, or download latest if none is
    installed. An explicit client version is downloaded if missing.
    """
    return _names(version)[0].copy()


def ability_names(version: str | None = None) -> dict[int, str]:
    """Ability/item IDs to internal names from the selected boon-data version."""
    return _names(version)[1].copy()


def ability_display_names(version: str | None = None) -> dict[str, str]:
    """Internal ability/item names to English names; omit unlocalized entries."""
    return _names(version)[2].copy()


def modifier_names(version: str | None = None) -> dict[int, str]:
    """Unqualified and owner-qualified modifier IDs to their corresponding names.

    Repeated definitions with the same ID and name share one entry. Conflicting
    names raise DataError rather than choosing a definition arbitrarily.
    """
    return _names(version)[3].copy()


def breakable_names(version: str | None = None) -> dict[int, str]:
    """Breakable subclass IDs to internal names from misc.json.

    Includes only records whose definition._class is citadel_breakable_prop.
    Uses the same version selection and automatic acquisition as hero_names().
    """
    return _names(version)[4].copy()
