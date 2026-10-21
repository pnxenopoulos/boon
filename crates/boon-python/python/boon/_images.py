"""Validate boon-data image indexes and extract selected, verified files."""

from __future__ import annotations

import hashlib
import re
import stat
import zipfile
from collections.abc import Sequence
from enum import StrEnum
from pathlib import Path, PurePosixPath


class ImageGroup(StrEnum):
    """Image groups accepted by Boon downloads.

    Use ALL/all, MAP/map, HEROES/heroes, ABILITIES/abilities, ITEMS/items,
    or ICONS/icons. The icons group contains stat icons.
    """

    ALL = "all"
    MAP = "map"
    HEROES = "heroes"
    ABILITIES = "abilities"
    ITEMS = "items"
    ICONS = "icons"


GROUPS = {group.value for group in ImageGroup} - {"all"}
ARTIFACTS = {"images.json", "images.zip"}
_MAX_UNPACKED_BYTES = 512 * 1024 * 1024


def selected(values: str | Sequence[str]) -> set[str]:
    """Validate image-group names and expand the all selection.

    Args:
        values: One group string or a sequence of strings or ImageGroup members.

    Returns:
        Set of concrete groups. The all value expands to every supported group.

    Raises:
        ValueError: A group value is unknown.
    """
    if isinstance(values, str):
        values = [values]
    try:
        groups = {ImageGroup(value).value for value in values}
    except (ValueError, TypeError) as error:
        raise ValueError(
            "image groups must be all, map, heroes, abilities, items, or icons"
        ) from error
    return GROUPS.copy() if "all" in groups else groups


def files(payload: dict, manifest: dict) -> dict[str, dict[str, dict]]:
    """Validate image metadata and group the publisher paths.

    Args:
        payload: Decoded images.json index.
        manifest: Verified catalog manifest for the same snapshot.

    Returns:
        Group names mapped to relative image paths and their bytes/sha256
        metadata. Shared image paths keep the publisher identity.

    Raises:
        ValueError: Snapshot identity, file paths, checksums, or size limits are invalid.
    """
    try:
        if (
            payload["client_version"] != manifest["client_version"]
            or payload["source_commit"] != manifest["source"]["commit"]
        ):
            raise ValueError("image index does not match the catalog snapshot")
        sections = {
            "map": [payload["map"]["images"]],
            "icons": [payload.get("stats", {})],
            **{
                group: [record["images"] for record in payload[group].values()]
                for group in ("heroes", "abilities", "items")
            },
        }
        result: dict[str, dict[str, dict]] = {}
        shared = {}
        for group, records in sections.items():
            result[group] = {}
            for images in records:
                for image in images.values():
                    if image is None:
                        continue
                    name = image["path"]
                    path = PurePosixPath(name)
                    if (
                        path.is_absolute()
                        or path.as_posix() != name
                        or any(part in {".", ".."} for part in path.parts)
                        or re.search(r"[\\:\x00-\x1f]", name)
                        or path.parts[0]
                        not in {"maps", "heroes", "abilities", "items", "stats"}
                        or path.suffix not in {".png", ".svg"}
                    ):
                        raise ValueError(f"invalid image path: {name}")
                    size, digest = image["bytes"], image["sha256"]
                    if (
                        type(size) is not int
                        or not 0 <= size <= _MAX_UNPACKED_BYTES
                        or not isinstance(digest, str)
                        or not re.fullmatch(r"[0-9a-f]{64}", digest)
                    ):
                        raise ValueError(f"invalid image fingerprint: {name}")
                    details = {"bytes": size, "sha256": digest}
                    if name in shared and shared[name] != details:
                        raise ValueError(f"conflicting image fingerprints: {name}")
                    shared[name] = result[group][name] = details
        if sum(image["bytes"] for image in shared.values()) > _MAX_UNPACKED_BYTES:
            raise ValueError("images exceed the unpacked size limit")
        return result
    except (KeyError, TypeError, AttributeError, IndexError) as error:
        raise ValueError(f"invalid image index: {error}") from error


def extract(
    archive_path: Path,
    destination: Path,
    records: dict[str, dict[str, dict]],
    groups: set[str],
) -> None:
    """Extract declared regular image files after archive validation.

    Args:
        archive_path: Local verified image ZIP.
        destination: Temporary extraction directory.
        records: Validated paths grouped by image group.
        groups: Concrete groups to extract.

    Returns:
        None.

    Raises:
        ValueError: An archive entry, size, or SHA-256 does not match the index.
        OSError: The archive or output files cannot be read or written.

    Notes:
        Archive paths do not create symbolic links or undeclared files.
    """
    all_files = {
        name: details for group in records.values() for name, details in group.items()
    }
    chosen = {
        name: details for group in groups for name, details in records[group].items()
    }
    with zipfile.ZipFile(archive_path) as archive:
        members = archive.infolist()
        if len(members) != len(all_files) or {item.filename for item in members} != set(
            all_files
        ):
            raise ValueError("image archive does not match its index")
        for item in members:
            if (
                item.file_size != all_files[item.filename]["bytes"]
                or stat.S_IFMT(item.external_attr >> 16) not in {0, stat.S_IFREG}
                or item.flag_bits & 1
            ):
                raise ValueError(f"invalid image archive entry: {item.filename}")
        for name, details in chosen.items():
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            size = 0
            digest = hashlib.sha256()
            with archive.open(name) as source, target.open("wb") as output:
                while chunk := source.read(1024 * 1024):
                    size += len(chunk)
                    if size > details["bytes"]:
                        raise ValueError(f"image exceeds declared size: {name}")
                    digest.update(chunk)
                    output.write(chunk)
            if size != details["bytes"] or digest.hexdigest() != details["sha256"]:
                raise ValueError(f"image checksum mismatch: {name}")
