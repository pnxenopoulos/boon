"""Name maps read from verified boon-data catalogs on first access."""

from functools import lru_cache
from pathlib import Path
from typing import TypeAlias

from boon import data
from boon._boon import _read_catalog_names

_NameMaps: TypeAlias = tuple[
    dict[int, str], dict[int, str], dict[str, str], dict[int, str], dict[int, str]
]


@lru_cache(maxsize=8)
def _read_names(directory: Path, signature: tuple[tuple[int, int], ...]) -> _NameMaps:
    # File signatures prevent stale maps after a forced reinstall.
    """Cache native name maps for one catalog-file signature.

    Args:
        directory: Directory with the four name catalogs.
        signature: File modification times and lengths used to invalidate the cache.

    Returns:
        Hero, ability, display-name, modifier, and breakable dictionaries.
    """
    return _read_catalog_names(directory)


def _names(version: str | None) -> _NameMaps:
    """Resolve a catalog version and read its verified name maps.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        Hero, ability, display-name, modifier, and breakable dictionaries.

    Raises:
        data.DataError: The selected catalogs cannot be installed or read.
    """
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
    """Map Deadlock hero IDs to localized names.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        New dict mapping hero IDs to display names. Internal names are used when localization is missing.

    Raises:
        data.DataError: The catalog cannot be installed, read, or resolved without conflicts.
    """
    return _names(version)[0].copy()


def ability_names(version: str | None = None) -> dict[int, str]:
    """Map Deadlock ability and item IDs to internal names.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        New dict mapping MurmurHash2 IDs to exact internal ability or item names.

    Raises:
        data.DataError: The catalog cannot be installed, read, or resolved without conflicts.
    """
    return _names(version)[1].copy()


def ability_display_names(version: str | None = None) -> dict[str, str]:
    """Map internal Deadlock ability/item names to English display names.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        New dict keyed by exact internal names. Entries without localization are omitted.

    Raises:
        data.DataError: The catalog cannot be installed, read, or resolved without conflicts.
    """
    return _names(version)[2].copy()


def modifier_names(version: str | None = None) -> dict[int, str]:
    """Map unqualified and owner-qualified modifier IDs to names.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        New dict containing both supported modifier-ID forms.

    Raises:
        data.DataError: The catalog cannot be installed, read, or resolved without conflicts.

    Notes:
        Duplicate definitions with the same ID and name share one entry.
        Conflicting names raise DataError.
    """
    return _names(version)[3].copy()


def breakable_names(version: str | None = None) -> dict[int, str]:
    """Map breakable subclass IDs to internal names in misc.json.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        New dict of records whose definition._class is citadel_breakable_prop.

    Raises:
        data.DataError: The catalog cannot be installed, read, or resolved without conflicts.
    """
    return _names(version)[4].copy()
