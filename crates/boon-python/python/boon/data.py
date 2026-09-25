"""Acquire versioned boon-data JSON catalogs, independently of demo parsing.

Client versions are resolved through boon-data's version index. Downloads are
verified before installation in ``~/.boon/<client-version>/``. Catalog contents
are stored unchanged; this module does not interpret gameplay definitions.
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
from datetime import datetime
from pathlib import Path

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
    if not isinstance(version, str) or not _VERSION.fullmatch(version):
        raise DataError(
            "expected a numeric client version from `boon versions`, e.g. 6698"
        )
    return version


def _request(url: str):
    return urllib.request.urlopen(
        urllib.request.Request(url, headers={"User-Agent": "boon"}), timeout=30
    )


def _json(url: str):
    try:
        with _request(url) as response:
            content = response.read(_MAX_METADATA_BYTES + 1)
        if len(content) > _MAX_METADATA_BYTES:
            raise DataError("catalog metadata exceeds the size limit")
        return json.loads(content)
    except (OSError, ValueError) as error:
        raise DataError(f"could not read boon-data metadata: {error}") from error


def _fingerprint(value: dict) -> None:
    if not isinstance(value, dict):
        raise DataError("invalid artifact metadata")
    size, digest = value.get("bytes"), value.get("sha256")
    if type(size) is not int or not 0 <= size <= _MAX_FILE_BYTES:
        raise DataError("invalid artifact size")
    if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
        raise DataError("invalid artifact SHA-256")


def _validate_entry(entry: dict, version: str) -> dict:
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
        for name, metadata in artifacts.items():
            _fingerprint(metadata)
            if metadata["url"] != f"{_DOWNLOAD_URL}/{tag}/{name}":
                raise DataError(f"unexpected download URL for {name}")
    except (KeyError, TypeError, ValueError) as error:
        raise DataError(f"invalid catalog version metadata: {error}") from error
    return entry


def _index() -> dict:
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
            if any(snapshot[key] != entry[key] for key in ("artifacts", "released_at")):
                raise DataError("inconsistent release metadata in version index")
        if payload["latest"] is not None and payload["latest"] not in entries:
            raise DataError("latest client version is missing from the index")
    except (KeyError, TypeError) as error:
        raise DataError(f"invalid boon-data version index: {error}") from error
    return payload


def available_versions() -> list[dict]:
    """Return published client-version metadata, newest version first."""
    entries = _index()["versions"]
    return [entries[version] for version in sorted(entries, key=int, reverse=True)]


def _latest_version(index: dict) -> str:
    # Backfill-only indexes can contain versions before a latest is designated.
    version: str | None = index["latest"]
    if version is None:
        versions: dict[str, dict] = index["versions"]
        version = max(versions, key=int, default=None)
    if version is None:
        raise DataError("no client versions have been published")
    return version


def latest_version() -> str:
    """Return the designated latest, or newest published client version."""
    return _latest_version(_index())


def _check_file(path: Path, metadata: dict) -> None:
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


def _local_json(path: Path):
    if path.is_symlink() or path.stat().st_size > _MAX_METADATA_BYTES:
        raise DataError(f"invalid cached metadata: {path.name}")
    return json.loads(path.read_bytes())


def _validate_manifest(payload: dict, entry: dict) -> dict:
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
    except (KeyError, TypeError) as error:
        raise DataError(f"invalid release manifest: {error}") from error
    return payload


def _cached_entry(version: str) -> dict:
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
    """Return verified installed client-version metadata, newest first; no network."""
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
    """Use an explicit version, newest installed version, or latest if none is installed."""
    if version is not None:
        return _version(version)
    local = local_versions()
    return local[0]["client_version"] if local else latest_version()


def _download(url: str, destination: Path, metadata: dict) -> None:
    size = 0
    with _request(url) as response, destination.open("wb") as output:
        while chunk := response.read(1024 * 1024):
            size += len(chunk)
            if size > metadata["bytes"]:
                raise DataError(f"download exceeds declared size: {destination.name}")
            output.write(chunk)
    _check_file(destination, metadata)


def update(version: str | None = None, *, force: bool = False) -> Path:
    """Download the release JSON assets; default to the index's latest client version.

    Explicit versions reuse a verified installation offline. ``force=True``
    replaces it only after all newly downloaded files pass verification.
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


def remove(version: str) -> Path:
    """Remove one local version, including corrupt installations; never use the network.

    An explicit numeric version is required. Missing installations and redirected
    version directories are errors. The cache root may be configured as usual.
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
    """Read an installed release's provenance, downloading missing files first."""
    version = resolve_version(version)
    directory = update(version)
    return _local_json(directory / "manifest.json")


def available_files(version: str | None = None) -> list[str]:
    """List this release's JSON asset names, downloading missing files first."""
    directory = update(resolve_version(version))
    return sorted(_local_json(directory / _RECEIPT)["version"]["artifacts"])


def catalog_path(name: str, version: str | None = None) -> Path:
    """Return a verified local JSON file path; do not interpret its contents."""
    filename = name if name.endswith(".json") else f"{name}.json"
    if filename not in _FILES:
        raise DataError(f"unknown catalog filename: {name}")
    return update(resolve_version(version)) / filename
