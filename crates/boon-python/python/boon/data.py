"""Acquire versioned boon-data JSON catalogs, independently of demo parsing.

Client versions are resolved through boon-data's version index. Downloads are
verified before installation in ``~/.boon/<client-version>/``. Catalog contents
are stored unchanged. Optional image groups use the same release and checksums.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import tempfile
import urllib.error
import urllib.request
import zipfile
from collections.abc import Sequence
from datetime import datetime
from http.client import HTTPResponse
from pathlib import Path
from typing import Any

from boon import _images
from boon._images import ImageGroup as ImageGroup

DATA_REPO = "pnxenopoulos/boon-data"
BOON_DATA_DIR = Path(os.environ.get("BOON_DATA_DIR", "~/.boon")).expanduser()
INDEX_URL = f"https://raw.githubusercontent.com/{DATA_REPO}/data-index/versions.json"
_DOWNLOAD_URL = f"https://github.com/{DATA_REPO}/releases/download"
_SOURCE_REPO = "SteamTracking/GameTracking-Deadlock"
_VERSION = re.compile(r"[0-9]+")
_FILES = (
    "abilities.json",
    "heroes.json",
    "manifest.json",
    "misc.json",
    "modifiers.json",
)
_RECEIPT = ".install.json"
_MAX_METADATA_BYTES = 16 * 1024 * 1024
_MAX_FILE_BYTES = 128 * 1024 * 1024


class DataError(RuntimeError):
    """Catalog metadata or files could not be fetched, verified, or cached."""


def _version(version: str) -> str:
    """Validate a numeric Deadlock client-version string.

    Args:
        version: Client version supplied by the caller.

    Returns:
        The validated version string.

    Raises:
        DataError: The version is not a numeric string.
    """
    if not isinstance(version, str) or not _VERSION.fullmatch(version):
        raise DataError(
            "expected a numeric client version from `boon versions`, e.g. 6698"
        )
    return version


def _request(url: str) -> HTTPResponse:
    """Open a boon-data HTTPS request with a 30-second timeout.

    Args:
        url: Metadata or artifact URL from the trusted repository.

    Returns:
        HTTP response stream. The caller must close it.

    Raises:
        OSError: The connection or HTTP request fails.
    """
    return urllib.request.urlopen(
        urllib.request.Request(url, headers={"User-Agent": "boon"}), timeout=30
    )


def _json(url: str) -> Any:
    """Read a size-limited JSON response before validating its structure.

    Args:
        url: boon-data metadata URL.

    Returns:
        Decoded JSON value. Callers validate its required fields.

    Raises:
        DataError: The request fails, the payload is too large, or JSON is invalid.
    """
    try:
        with _request(url) as response:
            content = response.read(_MAX_METADATA_BYTES + 1)
        if len(content) > _MAX_METADATA_BYTES:
            raise DataError("catalog metadata exceeds the size limit")
        return json.loads(content)
    except (OSError, ValueError) as error:
        raise DataError(f"could not read boon-data metadata: {error}") from error


def _fingerprint(value: dict) -> None:
    """Validate declared artifact length and SHA-256 metadata.

    Args:
        value: Artifact metadata with bytes and sha256 fields.

    Returns:
        None.

    Raises:
        DataError: The length or digest is invalid.
    """
    if not isinstance(value, dict):
        raise DataError("invalid artifact metadata")
    size, digest = value.get("bytes"), value.get("sha256")
    if type(size) is not int or not 0 <= size <= _MAX_FILE_BYTES:
        raise DataError("invalid artifact size")
    if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise DataError("invalid artifact SHA-256")


def _validate_entry(entry: dict, version: str) -> dict:
    """Validate one complete boon-data release entry.

    Args:
        entry: Version-index entry with catalog and optional image artifacts.
        version: Expected numeric client version.

    Returns:
        The validated entry, without changing its contents.

    Raises:
        DataError: The version, release tag, date, artifact set, or trusted URL is invalid.
    """
    try:
        if entry["client_version"] != _version(version):
            raise DataError("mismatched client version in catalog metadata")
        tag = entry["snapshot"]
        if not isinstance(tag, str) or not _VERSION.fullmatch(tag):
            raise DataError("invalid snapshot tag")
        timestamp = datetime.fromisoformat(entry["released_at"])
        if timestamp.tzinfo is None:
            raise DataError("release timestamp is missing its timezone")
        artifacts = entry["artifacts"]
        if not isinstance(artifacts, dict) or set(artifacts) != set(_FILES):
            raise DataError("release must contain all five JSON files")
        plotting = entry.get("plotting_artifacts", {})
        if not isinstance(plotting, dict) or (
            plotting and set(plotting) != _images.ARTIFACTS
        ):
            raise DataError("release must contain both image artifacts")
        for name, metadata in {**artifacts, **plotting}.items():
            _fingerprint(metadata)
            if metadata["url"] != f"{_DOWNLOAD_URL}/{tag}/{name}":
                raise DataError(f"unexpected download URL for {name}")
    except (KeyError, TypeError, ValueError) as error:
        raise DataError(f"invalid catalog version metadata: {error}") from error
    return entry


def _index() -> dict:
    """Download and validate the boon-data version index.

    Returns:
        Index with validated version entries.

    Raises:
        DataError: The version, metadata, download, or local files are invalid or unavailable.
    """
    payload = _json(INDEX_URL)
    try:
        if payload["source_repository"] != _SOURCE_REPO:
            raise DataError("unexpected source repository in boon-data version index")
        entries = payload["versions"]
        if not isinstance(entries, dict):
            raise DataError("invalid version list")
        for version, entry in entries.items():
            _validate_entry(entry, version)
            snapshot = payload["snapshots"][entry["snapshot"]]
            if not isinstance(snapshot, dict):
                raise DataError("invalid snapshot metadata in version index")
            if any(
                snapshot.get(key) != entry.get(key)
                for key in ("artifacts", "released_at", "plotting_artifacts")
            ):
                raise DataError("inconsistent release metadata in version index")
        if payload["latest"] is not None and payload["latest"] not in entries:
            raise DataError("latest client version is missing from the index")
    except (KeyError, TypeError) as error:
        raise DataError(f"invalid boon-data version index: {error}") from error
    return payload


def available_versions() -> list[dict]:
    """List published boon-data client versions, newest first.

    Returns:
        Release metadata dictionaries with client_version, released_at,
        source dates/times, and artifact checksums.

    Raises:
        DataError: The version, metadata, download, or local files are invalid or unavailable.

    Notes:
        This function reads the remote version index. It does not install data.
    """
    entries = _index()["versions"]
    return [entries[version] for version in sorted(entries, key=int, reverse=True)]


def _latest_version(index: dict) -> str:
    # Backfill-only indexes can contain versions before a latest is designated.
    """Select the index latest version, or the highest listed client version.

    Args:
        index: Validated version-index payload.

    Returns:
        Numeric client version string.

    Raises:
        DataError: The index has no usable version.
    """
    version: str | None = index["latest"]
    if version is None:
        versions: dict[str, dict] = index["versions"]
        version = max(versions, key=int, default=None)
    if version is None:
        raise DataError("no client versions have been published")
    return version


def latest_version() -> str:
    """Read the latest published boon-data client version.

    Returns:
        Numeric client version string.

    Raises:
        DataError: The version, metadata, download, or local files are invalid or unavailable.
    """
    return _latest_version(_index())


def _check_file(path: Path, metadata: dict) -> None:
    """Check a local file against its declared length and SHA-256.

    Args:
        path: Local artifact path.
        metadata: Validated bytes and sha256 fields.

    Returns:
        None.

    Raises:
        DataError: The file does not match its declared size or digest.
        OSError: The file cannot be read.
    """
    if (
        path.is_symlink()
        or not path.is_file()
        or path.stat().st_size != metadata["bytes"]
    ):
        raise DataError(f"invalid size or missing file: {path.name}")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    if digest.hexdigest() != metadata["sha256"]:
        raise DataError(f"checksum mismatch for {path.name}")


def _local_json(path: Path) -> Any:
    """Read local JSON without following a symbolic link.

    Args:
        path: Metadata path under the cache.

    Returns:
        Decoded JSON value before caller validation.

    Raises:
        DataError: The path is a symbolic link or exceeds the metadata limit.
        OSError: The file cannot be read.
        ValueError: The file is not valid JSON.
    """
    if path.is_symlink() or path.stat().st_size > _MAX_METADATA_BYTES:
        raise DataError(f"invalid cached metadata: {path.name}")
    return json.loads(path.read_bytes())


def _validate_manifest(payload: dict, entry: dict) -> dict:
    """Check release-manifest identity and artifacts against the version index.

    Args:
        payload: Decoded release manifest.
        entry: Validated version-index entry.

    Returns:
        The validated manifest.

    Raises:
        DataError: The source identity or artifact metadata does not match.
    """
    try:
        if (
            payload["release_key"] != entry["snapshot"]
            or payload["client_version"] != entry["snapshot"]
        ):
            raise DataError("mismatched snapshot in release manifest")
        source = payload["source"]
        if source["repository"] != _SOURCE_REPO or not re.fullmatch(
            r"[0-9a-f]{40}", source["commit"]
        ):
            raise DataError("invalid source in release manifest")
        artifacts = payload["artifacts"]
        if not isinstance(artifacts, dict) or set(artifacts) != set(
            entry["artifacts"]
        ) - {"manifest.json"}:
            raise DataError("manifest must describe every catalog in the version index")
        for name, metadata in artifacts.items():
            if any(
                metadata[key] != entry["artifacts"][name][key]
                for key in ("bytes", "sha256")
            ):
                raise DataError(f"manifest and version index disagree for {name}")
        plotting = payload.get("plotting_artifacts", {})
        if not isinstance(plotting, dict) or set(plotting) != set(
            entry.get("plotting_artifacts", {})
        ):
            raise DataError("manifest and version index disagree for image artifacts")
        for name, metadata in plotting.items():
            if any(
                metadata[key] != entry["plotting_artifacts"][name][key]
                for key in ("bytes", "sha256")
            ):
                raise DataError(f"manifest and version index disagree for {name}")
    except (KeyError, TypeError) as error:
        raise DataError(f"invalid release manifest: {error}") from error
    return payload


def _cached_entry(version: str) -> dict:
    """Verify an installed catalog and its receipt without network access.

    Args:
        version: Numeric client version under BOON_DATA_DIR.

    Returns:
        Validated version-index entry from the local receipt.

    Raises:
        DataError: The installation is missing, incomplete, redirected, or corrupt.
    """
    directory = BOON_DATA_DIR / _version(version)
    try:
        if directory.is_symlink():
            raise DataError("cache directory must not be a symlink")
        receipt = _local_json(directory / _RECEIPT)
        entry = _validate_entry(receipt["version"], version)
        for name, metadata in entry["artifacts"].items():
            _check_file(directory / name, metadata)
        _validate_manifest(_local_json(directory / "manifest.json"), entry)
        return entry
    except (OSError, ValueError, KeyError, TypeError, DataError) as error:
        raise DataError(
            f"invalid cache for {version}; run `boon get {version} --force`: {error}"
        ) from error


def local_versions() -> list[dict]:
    """List verified local boon-data versions without network access.

    Returns:
        Release metadata sorted by numeric client version, newest first.
        Missing caches return an empty list. Invalid installations are omitted.

    Raises:
        DataError: The cache directory cannot be listed.
    """
    if not BOON_DATA_DIR.exists():
        return []
    try:
        directories = list(BOON_DATA_DIR.iterdir())
    except OSError as error:
        raise DataError(f"could not list the boon-data cache: {error}") from error
    entries = []
    for directory in directories:
        if (
            directory.is_symlink()
            or not directory.is_dir()
            or not _VERSION.fullmatch(directory.name)
        ):
            continue
        try:
            entries.append(_cached_entry(directory.name))
        except DataError:
            continue
    return sorted(entries, key=lambda entry: int(entry["client_version"]), reverse=True)


def resolve_version(version: str | None = None) -> str:
    """Select an explicit, local, or latest published client version.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        Numeric version string. This function does not install catalogs.

    Raises:
        DataError: The version, metadata, download, or local files are invalid or unavailable.
    """
    if version is not None:
        return _version(version)
    local = local_versions()
    return local[0]["client_version"] if local else latest_version()


def _download(url: str, destination: Path, metadata: dict) -> None:
    """Stream one artifact and check its size and SHA-256.

    Args:
        url: Trusted artifact URL.
        destination: File to write in the temporary installation.
        metadata: Expected bytes and sha256 fields.

    Returns:
        None.

    Raises:
        DataError: The response exceeds its declared size or fails verification.
        OSError: The response or destination cannot be read or written.
    """
    size = 0
    with _request(url) as response, destination.open("wb") as output:
        while chunk := response.read(1024 * 1024):
            size += len(chunk)
            if size > metadata["bytes"]:
                raise DataError(f"download exceeds declared size: {destination.name}")
            output.write(chunk)
    _check_file(destination, metadata)


def _update_catalogs(version: str | None, *, force: bool) -> Path:
    """Install verified JSON catalogs without adding image groups.

    Args:
        version: Client version to install. None selects the latest published version.
        force: Replace a verified installation after a fresh download when True.

    Returns:
        Installed version directory.

    Raises:
        DataError: The version, metadata, download, or local files are invalid or unavailable.
    """
    if version is not None:
        _version(version)
        destination = BOON_DATA_DIR / version
        if destination.exists() and not force:
            _cached_entry(version)
            return destination
    index = _index()
    version = version if version is not None else _latest_version(index)
    if version not in index["versions"]:
        raise DataError(
            f"client version {version} is not available; check `boon versions`"
        )
    entry = index["versions"][version]
    destination = BOON_DATA_DIR / version
    if destination.is_symlink():
        raise DataError(f"cache directory must not be a symlink: {destination}")
    try:
        if destination.exists() and not force:
            if _cached_entry(version)["artifacts"] != entry["artifacts"]:
                raise DataError(
                    f"cache differs from the published version; run `boon get {version} --force`"
                )
            return destination
        BOON_DATA_DIR.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(
            prefix=".download-", dir=BOON_DATA_DIR
        ) as temporary:
            temporary = Path(temporary)
            stage = temporary / "snapshot"
            stage.mkdir()
            for name, metadata in entry["artifacts"].items():
                _download(metadata["url"], stage / name, metadata)
            _validate_manifest(_local_json(stage / "manifest.json"), entry)
            previous_images = destination / "images"
            if previous_images.exists():
                if previous_images.resolve() != destination.resolve() / "images":
                    raise DataError(
                        "image cache must not be a symlink or redirected path"
                    )
                if previous_images.is_dir():
                    shutil.copytree(previous_images, stage / "images", symlinks=True)
            (stage / _RECEIPT).write_text(
                json.dumps({"version": entry}, indent=2) + "\n",
                encoding="utf-8",
            )
            backup = temporary / "previous"
            if destination.exists():
                if not force:
                    if _cached_entry(version)["artifacts"] != entry["artifacts"]:
                        raise DataError(
                            "another download installed a different snapshot"
                        )
                    return destination
                if destination.is_symlink():
                    raise DataError("cache directory must not be a symlink")
                destination.rename(backup)
            try:
                stage.rename(destination)
            except BaseException:
                if backup.exists():
                    backup.rename(destination)
                raise
        return destination
    except (OSError, ValueError) as error:
        raise DataError(f"could not cache client version {version}: {error}") from error


def _update_images(directory: Path, groups: set[str], *, force: bool) -> None:
    """Add verified image groups to an installed catalog version.

    Args:
        directory: Verified client-version directory.
        groups: Selected map, heroes, abilities, items, or icons groups.
        force: Download the image artifacts again when True.

    Returns:
        None. Existing selected groups remain available.

    Raises:
        DataError: The version, metadata, download, or local files are invalid or unavailable.
    """
    entry = _cached_entry(directory.name)
    artifacts = entry.get("plotting_artifacts")
    if not artifacts:
        raise DataError(
            f"client version {directory.name} has no image bundle; choose a release with images "
            "or use --force to refresh cached metadata"
        )
    destination = directory / "images"
    if destination.resolve() != directory.resolve() / "images":
        raise DataError("image cache must not be a symlink or redirected path")
    previous = set()
    cached = destination.exists() and not force
    try:
        if destination.exists():
            try:
                receipt = _local_json(destination / _RECEIPT)
                previous = _images.selected(receipt["groups"])
            except (OSError, ValueError, KeyError, TypeError, DataError):
                if not force:
                    raise DataError(
                        "invalid image receipt; use --force to repair it"
                    ) from None
            if cached:
                if receipt["artifacts"] != artifacts:
                    raise DataError("image cache differs from the release; use --force")
                for name, metadata in artifacts.items():
                    _check_file(destination / name, metadata)
                records = _images.files(
                    _local_json(destination / "images.json"),
                    _local_json(directory / "manifest.json"),
                )
                for group in previous:
                    for name, metadata in records[group].items():
                        path = destination / name
                        if path.resolve() != destination.resolve() / name:
                            raise DataError("cached image path must not be redirected")
                        _check_file(path, metadata)
                if groups <= previous:
                    return
        groups = groups | previous
        with tempfile.TemporaryDirectory(prefix=".images-", dir=directory) as temporary:
            temporary = Path(temporary)
            stage = temporary / "images"
            stage.mkdir()
            for name, metadata in artifacts.items():
                if cached:
                    shutil.copyfile(destination / name, stage / name)
                    _check_file(stage / name, metadata)
                else:
                    _download(metadata["url"], stage / name, metadata)
            records = _images.files(
                _local_json(stage / "images.json"),
                _local_json(directory / "manifest.json"),
            )
            _images.extract(stage / "images.zip", stage, records, groups)
            (stage / _RECEIPT).write_text(
                json.dumps({"groups": sorted(groups), "artifacts": artifacts}, indent=2)
                + "\n",
                encoding="utf-8",
            )
            backup = temporary / "previous"
            if destination.exists():
                destination.rename(backup)
            try:
                stage.rename(destination)
            except BaseException:
                if backup.exists():
                    backup.rename(destination)
                raise
    except (
        OSError,
        ValueError,
        KeyError,
        TypeError,
        zipfile.BadZipFile,
        RuntimeError,
    ) as error:
        raise DataError(
            f"could not cache images for {directory.name}; use --force to repair them: {error}"
        ) from error


def update(
    version: str | None = None,
    *,
    force: bool = False,
    images: str | Sequence[str] = (),
) -> Path:
    """Download and verify boon-data catalogs and optional game images.

    Args:
        version: Client version to install. None selects the latest published version.
        force: Download fresh files and replace an installation only after verification.
        images: ImageGroup members or strings: all, map, heroes, abilities, items, or icons. Accept one string or a sequence. The default downloads no images.

    Returns:
        Path to BOON_DATA_DIR/<client-version>. JSON catalogs stay unchanged.
        Selected image files are stored under images/.

    Raises:
        DataError: The version, metadata, download, or local files are invalid or unavailable.

    Notes:
        Explicit versions reuse verified local files without a network request.
        The default cache is ~/.boon. Set BOON_DATA_DIR before importing Boon
        to select another root. Image archives remain cached for later group
        selections. force=True preserves readable prior image selections.

    Examples:
        >>> from boon import data
        >>> directory = data.update("6795", images=["map", "heroes"])
        >>> print(directory / "images")
    """
    try:
        groups = _images.selected(images)
    except ValueError as error:
        raise DataError(str(error)) from error
    directory = _update_catalogs(version, force=force)
    if groups:
        _update_images(directory, groups, force=force)
    return directory


def remove(version: str) -> Path:
    """Remove one installed boon-data version without network access.

    Args:
        version: Explicit numeric client version to remove, including its selected images.

    Returns:
        Path of the removed version directory.

    Raises:
        DataError: The version is invalid, missing, redirected, or cannot be removed.

    Notes:
        This removes corrupt installations too. Other client versions remain available.
    """
    directory = BOON_DATA_DIR / _version(version)
    try:
        # Resolve before recursive deletion, rejecting symlinks and junctions that
        # could redirect this version to a different directory.
        if (
            directory.is_symlink()
            or directory.resolve() != BOON_DATA_DIR.resolve() / version
        ):
            raise DataError(
                f"cache directory must not be a symlink or redirected path: {directory}"
            )
        if not directory.is_dir():
            raise DataError(f"client version {version} is not installed at {directory}")
        shutil.rmtree(directory)
    except OSError as error:
        raise DataError(
            f"could not remove client version {version}: {error}"
        ) from error
    return directory


def manifest(version: str | None = None) -> dict:
    """Read verified release metadata for one boon-data version.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        Manifest dictionary with source revision, build metadata, and artifacts.

    Raises:
        DataError: The version, metadata, download, or local files are invalid or unavailable.
    """
    version = resolve_version(version)
    directory = update(version)
    return _local_json(directory / "manifest.json")


def available_files(version: str | None = None) -> list[str]:
    """List verified catalog filenames in a boon-data release.

    Args:
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        Sorted JSON filenames, including manifest.json. Missing catalogs are downloaded.

    Raises:
        DataError: The version, metadata, download, or local files are invalid or unavailable.
    """
    directory = update(resolve_version(version))
    return sorted(_local_json(directory / _RECEIPT)["version"]["artifacts"])


def catalog_path(name: str, version: str | None = None) -> Path:
    """Return a verified local path to one boon-data JSON catalog.

    Args:
        name: abilities, heroes, modifiers, misc, or manifest, with an optional .json suffix.
        version: Client version to select. None uses the newest verified local version, or the latest published version if none is installed.

    Returns:
        Path to the requested catalog. Missing versions are downloaded.

    Raises:
        DataError: The version, metadata, download, or local files are invalid or unavailable.
    """
    filename = name if name.endswith(".json") else f"{name}.json"
    if filename not in _FILES:
        raise DataError(f"unknown catalog filename: {name}")
    return update(resolve_version(version)) / filename
