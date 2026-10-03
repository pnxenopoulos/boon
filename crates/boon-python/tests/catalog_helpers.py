"""Small, deterministic boon-data releases for offline tests."""

import copy
import gzip
import hashlib
import json
from pathlib import Path

from boon import data

MODIFIER_VALUE_TYPES = json.loads(gzip.decompress(
    (Path(__file__).parent / "catalogs/108575009/abilities.json.gz").read_bytes()
))["modifier_value_types"]

VERSION = "6698"
PUBLISHED = "2026-09-22T00:31:49Z"
VERSION_DATE = "Sep 17 2026"
VERSION_TIME = "16:09:39"
FILES = {
    "abilities.json",
    "heroes.json",
    "modifiers.json",
    "misc.json",
    "manifest.json",
}


def fingerprint(content):
    return {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}


def release(version=VERSION, records=None):
    tag = version
    source = {
        "repository": "SteamTracking/GameTracking-Deadlock",
        "commit": "a" * 40,
        "committed_at": "2026-09-18T12:00:00Z",
    }
    files = {
        name: json.dumps(
            {
                "modifier_value_types": MODIFIER_VALUE_TYPES,
                "catalog": Path(name).stem,
                "client_version": version,
                "source_commit": source["commit"],
                "records": records.get(Path(name).stem, [])
                if records is not None
                else [{"future_property": 42}],
            }
        ).encode()
        for name in FILES - {"manifest.json"}
    }
    manifest = {
        "release_key": tag,
        "client_version": version,
        "source": source,
        "artifacts": {name: fingerprint(content) for name, content in files.items()},
    }
    files["manifest.json"] = json.dumps(manifest).encode()
    artifacts = {
        name: {**fingerprint(content), "url": f"{data._DOWNLOAD_URL}/{tag}/{name}"}
        for name, content in files.items()
    }
    entry = {
        "client_version": version,
        "snapshot": tag,
        "released_at": PUBLISHED,
        "version_date": VERSION_DATE,
        "version_time": VERSION_TIME,
        "source": source,
        "artifacts": artifacts,
    }
    snapshot = copy.deepcopy(entry)
    snapshot.pop("snapshot")
    index = {
        "source_repository": source["repository"],
        "latest": version,
        "versions": {version: entry},
        "snapshots": {tag: snapshot},
    }
    return index, {artifacts[name]["url"]: content for name, content in files.items()}
