"""Selected image downloads use small offline bundles and isolated caches."""

import copy
import io
import json
import stat
import zipfile
from pathlib import Path

import pytest
from boon import data
from boon.cli import app
from catalog_helpers import VERSION, catalog_upstream, fingerprint
from typer.testing import CliRunner

PATHS = {
    "map": "maps/minimap.svg",
    "heroes": "heroes/1/icon.svg",
    "abilities": "abilities/2/icon.svg",
    "items": "items/3/shop.svg",
    "icons": "stats/fire_rate.svg",
}


def publish_images(upstream, *, extra=(), symlink=None):
    """Update all release fingerprints, so extraction tests reach the image reader."""
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w") as archive:
        for name, content in [*upstream["image_files"].items(), *extra]:
            entry = zipfile.ZipInfo(name)
            if name == symlink:
                entry.create_system = 3
                entry.external_attr = (stat.S_IFLNK | 0o777) << 16
            archive.writestr(entry, content)
    artifacts = {}
    for name, content in {
        "images.json": json.dumps(upstream["image_index"]).encode(),
        "images.zip": output.getvalue(),
    }.items():
        url = f"{data._DOWNLOAD_URL}/{VERSION}/{name}"
        upstream["files"][url] = content
        artifacts[name] = {"url": url, **fingerprint(content)}
    url = f"{data._DOWNLOAD_URL}/{VERSION}/manifest.json"
    manifest = json.loads(upstream["files"][url])
    manifest["plotting_artifacts"] = {
        name: {key: value for key, value in metadata.items() if key != "url"}
        for name, metadata in artifacts.items()
    }
    upstream["files"][url] = json.dumps(manifest).encode()
    for entry in [
        upstream["index"]["versions"][VERSION],
        upstream["index"]["snapshots"][VERSION],
    ]:
        entry["plotting_artifacts"] = copy.deepcopy(artifacts)
        entry["artifacts"]["manifest.json"].update(fingerprint(upstream["files"][url]))


@pytest.fixture
def image_upstream(monkeypatch, tmp_path):
    upstream = catalog_upstream(monkeypatch, tmp_path / "cache")
    upstream["image_files"] = {
        name: f'<svg xmlns="http://www.w3.org/2000/svg" id="{group}"/>'.encode()
        for group, name in PATHS.items()
    }
    details = {
        group: {"path": name, **fingerprint(upstream["image_files"][name])}
        for group, name in PATHS.items()
    }
    upstream["image_index"] = {
        "client_version": VERSION,
        "source_commit": "a" * 40,
        "map": {"images": {"base": details["map"]}},
        "stats": {"fire_rate": details["icons"]},
        **{
            group: {"1": {"images": {"icon": details[group]}}}
            for group in ("heroes", "abilities", "items")
        },
    }
    publish_images(upstream)
    return upstream


def contents(directory):
    return {
        p.relative_to(directory): p.read_bytes()
        for p in directory.rglob("*")
        if p.is_file()
    }


def extracted(directory):
    return {
        p.relative_to(directory / "images").as_posix()
        for p in (directory / "images").rglob("*.svg")
    }


def test_catalog_download_does_not_fetch_optional_images(image_upstream):
    directory = data.update(VERSION)
    assert not (directory / "images").exists()
    assert not any(
        url.endswith(("images.zip", "images.json"))
        for url in image_upstream["requests"]
    )


def test_only_selected_groups_are_extracted(image_upstream):
    directory = data.update(VERSION, images=["map", "icons"])
    assert extracted(directory) == {PATHS["map"], PATHS["icons"]}


def test_all_selects_every_image_group(image_upstream):
    directory = data.update(VERSION, images="all")
    assert extracted(directory) == set(PATHS.values())


def test_enum_selection_is_supported(image_upstream):
    directory = data.update(VERSION, images=[data.ImageGroup.ITEMS])
    assert extracted(directory) == {PATHS["items"]}


def test_cli_accepts_repeated_image_groups(image_upstream):
    result = CliRunner().invoke(
        app, ["get", VERSION, "--images", "map", "--images", "icons", "--json"]
    )
    assert result.exit_code == 0, result.output
    payload = json.loads(result.stdout)
    assert payload["images"]["groups"] == ["map", "icons"]
    assert extracted(Path(payload["path"])) == {PATHS["map"], PATHS["icons"]}


def test_cli_all_reports_the_image_directory(image_upstream):
    result = CliRunner().invoke(app, ["get", VERSION, "--images", "all"])
    assert result.exit_code == 0, result.output
    assert f"Images: {data.BOON_DATA_DIR / VERSION / 'images'}" in result.stdout


def test_cli_rejects_unknown_groups_before_network_access(image_upstream):
    result = CliRunner().invoke(app, ["get", VERSION, "--images", "unknown"])
    assert result.exit_code == 2
    assert image_upstream["requests"] == []


def test_api_rejects_unknown_groups_before_network_access(image_upstream):
    with pytest.raises(data.DataError, match="image groups"):
        data.update(VERSION, images=["unknown"])
    assert image_upstream["requests"] == []


def test_selected_images_are_reused_offline(image_upstream):
    directory = data.update(VERSION, images=["icons"])
    image_upstream["offline"] = True
    before = len(image_upstream["requests"])
    assert data.update(VERSION, images=["icons"]) == directory
    assert len(image_upstream["requests"]) == before


def test_more_groups_can_be_added_offline(image_upstream):
    directory = data.update(VERSION, images=["icons"])
    image_upstream["offline"] = True
    before = len(image_upstream["requests"])
    data.update(VERSION, images=["items"])
    assert extracted(directory) == {PATHS["icons"], PATHS["items"]}
    assert len(image_upstream["requests"]) == before


def test_force_catalog_download_preserves_images(image_upstream):
    directory = data.update(VERSION, images=["icons"])
    original = contents(directory / "images")
    data.update(VERSION, force=True)
    assert contents(directory / "images") == original


def test_force_image_download_preserves_previous_selections(image_upstream):
    directory = data.update(VERSION, images=["icons"])
    data.update(VERSION, images=["items"], force=True)
    assert extracted(directory) == {PATHS["icons"], PATHS["items"]}
    assert sum(url.endswith("images.zip") for url in image_upstream["requests"]) == 2


def test_release_without_images_reports_a_clear_error(monkeypatch, tmp_path):
    catalog_upstream(monkeypatch, tmp_path / "cache")
    with pytest.raises(data.DataError, match="has no image bundle"):
        data.update(VERSION, images=["map"])


def test_shared_snapshots_use_the_snapshot_image_version(image_upstream):
    entry = copy.deepcopy(image_upstream["index"]["versions"][VERSION])
    entry["client_version"] = "6699"
    image_upstream["index"]["versions"]["6699"] = entry
    directory = data.update("6699", images=["icons"])
    assert directory.name == "6699"
    assert extracted(directory) == {PATHS["icons"]}


def test_failed_force_image_download_preserves_the_installation(image_upstream):
    directory = data.update(VERSION, images=["icons"])
    original = contents(directory)
    url = f"{data._DOWNLOAD_URL}/{VERSION}/images.zip"
    image_upstream["files"][url] = b"invalid"
    with pytest.raises(data.DataError):
        data.update(VERSION, images=["icons"], force=True)
    assert contents(directory) == original


@pytest.mark.parametrize("damage", ["partial", "url", "size", "hash", "snapshot"])
def test_invalid_image_metadata_is_rejected_before_asset_download(
    image_upstream, damage
):
    for entry in [
        image_upstream["index"]["versions"][VERSION],
        image_upstream["index"]["snapshots"][VERSION],
    ]:
        if damage == "partial":
            del entry["plotting_artifacts"]["images.zip"]
        elif damage == "url":
            entry["plotting_artifacts"]["images.zip"]["url"] = (
                "https://example.com/images.zip"
            )
        elif damage == "size":
            entry["plotting_artifacts"]["images.zip"]["bytes"] = -1
        elif damage == "hash":
            entry["plotting_artifacts"]["images.zip"]["sha256"] = "invalid"
    if damage == "snapshot":
        image_upstream["index"]["snapshots"][VERSION]["plotting_artifacts"][
            "images.zip"
        ]["sha256"] = "b" * 64
    with pytest.raises(data.DataError):
        data.update(VERSION, images=["icons"])
    assert image_upstream["requests"] == [data.INDEX_URL]


def test_manifest_and_index_must_agree_on_images(image_upstream):
    for entry in [
        image_upstream["index"]["versions"][VERSION],
        image_upstream["index"]["snapshots"][VERSION],
    ]:
        entry["plotting_artifacts"]["images.zip"]["sha256"] = "b" * 64
    with pytest.raises(data.DataError, match="manifest and version index disagree"):
        data.update(VERSION, images=["icons"])


@pytest.mark.parametrize(
    "field,value", [("client_version", "9999"), ("source_commit", "b" * 40)]
)
def test_image_provenance_must_match_the_catalog(image_upstream, field, value):
    image_upstream["image_index"][field] = value
    publish_images(image_upstream)
    with pytest.raises(data.DataError, match="does not match the catalog"):
        data.update(VERSION, images=["icons"])


@pytest.mark.parametrize(
    "name",
    [
        "../escape.svg",
        "/escape.svg",
        "stats/../escape.svg",
        "stats/./fire_rate.svg",
        "stats\\escape.svg",
        "C:/escape.svg",
    ],
)
def test_image_paths_cannot_escape_the_cache(image_upstream, name):
    image_upstream["image_index"]["stats"]["fire_rate"]["path"] = name
    publish_images(image_upstream)
    with pytest.raises(data.DataError, match="invalid image path"):
        data.update(VERSION, images=["icons"])


def test_archive_symlinks_are_rejected(image_upstream):
    publish_images(image_upstream, symlink=PATHS["map"])
    with pytest.raises(data.DataError, match="invalid image archive entry"):
        data.update(VERSION, images=["icons"])


def test_duplicate_archive_entries_are_rejected(image_upstream):
    with pytest.warns(UserWarning, match="Duplicate name"):
        publish_images(image_upstream, extra=[(PATHS["map"], b"duplicate")])
    with pytest.raises(data.DataError, match="does not match its index"):
        data.update(VERSION, images=["icons"])


def test_undeclared_archive_files_are_rejected(image_upstream):
    publish_images(image_upstream, extra=[("stats/unknown.svg", b"extra")])
    with pytest.raises(data.DataError, match="does not match its index"):
        data.update(VERSION, images=["icons"])


def test_missing_archive_files_are_rejected(image_upstream):
    del image_upstream["image_files"][PATHS["icons"]]
    publish_images(image_upstream)
    with pytest.raises(data.DataError, match="does not match its index"):
        data.update(VERSION, images=["icons"])


def test_individual_image_checksums_are_verified(image_upstream):
    image_upstream["image_index"]["stats"]["fire_rate"]["sha256"] = "b" * 64
    publish_images(image_upstream)
    with pytest.raises(data.DataError, match="image checksum mismatch"):
        data.update(VERSION, images=["icons"])


def test_corrupt_images_do_not_break_catalog_access(image_upstream):
    directory = data.update(VERSION, images=["icons"])
    (directory / "images" / PATHS["icons"]).write_bytes(b"corrupt")
    assert data.catalog_path("heroes", VERSION) == directory / "heroes.json"


def test_corrupt_image_cache_requires_force(image_upstream):
    directory = data.update(VERSION, images=["icons"])
    (directory / "images" / PATHS["icons"]).write_bytes(b"corrupt")
    with pytest.raises(data.DataError, match="--force"):
        data.update(VERSION, images=["icons"])


def test_force_repairs_corrupt_image_files(image_upstream):
    directory = data.update(VERSION, images=["icons"])
    (directory / "images" / PATHS["icons"]).write_bytes(b"corrupt")
    data.update(VERSION, images=["icons"], force=True)
    assert (directory / "images" / PATHS["icons"]).read_bytes() == image_upstream[
        "image_files"
    ][PATHS["icons"]]


def test_image_installation_rolls_back_on_rename_failure(image_upstream, monkeypatch):
    directory = data.update(VERSION, images=["icons"])
    original = contents(directory / "images")
    rename = Path.rename

    def fail_stage(path, target):
        if path.name == "images" and path.parent.name.startswith(".images-"):
            raise OSError("rename failed")
        return rename(path, target)

    monkeypatch.setattr(Path, "rename", fail_stage)
    with pytest.raises(data.DataError, match="rename failed"):
        data.update(VERSION, images=["items"])
    assert contents(directory / "images") == original


def test_image_directory_symlinks_are_rejected(image_upstream, tmp_path):
    directory = data.update(VERSION)
    external = tmp_path / "external"
    external.mkdir()
    (directory / "images").symlink_to(external, target_is_directory=True)
    with pytest.raises(data.DataError, match="symlink or redirected"):
        data.update(VERSION, images=["icons"])
    assert list(external.iterdir()) == []


def test_nested_image_symlinks_are_rejected(image_upstream, tmp_path):
    directory = data.update(VERSION, images=["icons"])
    (directory / "images" / PATHS["icons"]).unlink()
    (directory / "images" / "stats").rmdir()
    external = tmp_path / "external"
    external.mkdir()
    (directory / "images" / "stats").symlink_to(external, target_is_directory=True)
    with pytest.raises(data.DataError, match="redirected"):
        data.update(VERSION, images=["icons"])


def test_force_repairs_an_invalid_image_receipt(image_upstream):
    directory = data.update(VERSION, images=["icons"])
    (directory / "images" / ".install.json").write_text("invalid")
    data.update(VERSION, images=["items"], force=True)
    assert extracted(directory) == {PATHS["items"]}


def test_invalid_snapshot_record_is_reported_as_a_data_error(image_upstream):
    image_upstream["index"]["snapshots"][VERSION] = []
    with pytest.raises(data.DataError, match="invalid snapshot metadata"):
        data.update(VERSION, images=["icons"])
