"""Catalog-backed names, automatic acquisition, and version selection."""

import io
import json
import urllib.error
from pathlib import Path

import pytest
from boon import ability_display_names, ability_names, data, hero_names, modifier_names
from catalog_helpers import VERSION, release


@pytest.fixture
def upstream(monkeypatch, tmp_path):
    records = json.loads(Path(__file__).with_name("name-catalogs.json").read_text())
    index, files = release(records=records)
    state = {"index": index, "files": files, "requests": [], "records": records}
    monkeypatch.setattr(data, "BOON_DATA_DIR", tmp_path / "cache")

    def request(url):
        state["requests"].append(url)
        return io.BytesIO(
            json.dumps(state["index"]).encode()
            if url == data.INDEX_URL
            else state["files"][url]
        )

    monkeypatch.setattr(data, "_request", request)
    return state


def test_name_access_downloads_once_and_reuses_verified_local_files(upstream):
    assert hero_names()[1] == "Infernus"
    assert data.local_versions()[0]["client_version"] == VERSION
    requests = len(upstream["requests"])
    assert ability_names()[46922526] == "inherent_base"
    assert ability_display_names()["citadel_ability_hook"] == "Grapple Arm"
    assert modifier_names()[1364211883] == "ability_afterburn/modifier_afterburn_dot"
    assert modifier_names()[3285944580] == "modifier_afterburn_dot"
    assert len(upstream["requests"]) == requests
    names = hero_names()
    names[1] = "caller changed this"
    assert hero_names()[1] == "Infernus"


def test_newest_local_and_explicit_versions_use_their_own_names(upstream):
    hero_names()
    records = upstream["records"]
    records["heroes"][0]["display_name"] = "Changed hero"
    changed_id = records["heroes"][0]["hero_id"]
    upstream["index"], upstream["files"] = release("6701", records=records)
    assert (
        hero_names()[changed_id] != "Changed hero"
    )  # existing local version, no update
    assert hero_names("6701")[changed_id] == "Changed hero"
    assert hero_names()[changed_id] == "Changed hero"
    assert hero_names(VERSION)[changed_id] != "Changed hero"


def test_forced_reinstall_invalidates_cached_names(upstream):
    hero_names()
    records = upstream["records"]
    records["heroes"][0]["display_name"] = "Replacement hero name"
    upstream["index"], upstream["files"] = release(records=records)
    data.update(VERSION, force=True)
    assert hero_names()[records["heroes"][0]["hero_id"]] == "Replacement hero name"
    data.remove(VERSION)
    requests = len(upstream["requests"])
    assert hero_names()[records["heroes"][0]["hero_id"]] == "Replacement hero name"
    assert len(upstream["requests"]) > requests


def test_corruption_is_not_hidden_by_cached_names(upstream, monkeypatch):
    hero_names()
    (data.BOON_DATA_DIR / VERSION / "heroes.json").write_text("corrupt")
    with pytest.raises(data.DataError, match="boon get.*--force"):
        hero_names(VERSION)
    monkeypatch.setattr(
        data,
        "_request",
        lambda url: (_ for _ in ()).throw(urllib.error.URLError("offline")),
    )
    with pytest.raises(data.DataError, match="offline"):
        hero_names()


def test_unlocalized_hero_uses_internal_name_and_invalid_catalog_fails(upstream):
    records = upstream["records"]
    records["heroes"][0]["display_name"] = None
    upstream["index"], upstream["files"] = release(records=records)
    hero = records["heroes"][0]
    assert hero_names()[hero["hero_id"]] == hero["hero_name"]
    records["heroes"].append({**hero, "display_name": "Conflict"})
    upstream["index"], upstream["files"] = release(records=records)
    data.update(VERSION, force=True)
    with pytest.raises(data.DataError, match="conflicting"):
        hero_names()


def test_removed_datasets_are_not_public():
    import boon

    assert not hasattr(boon.Demo, "healing")
    assert not hasattr(boon.Demo, "barriers")
    assert {"healing", "barriers"}.isdisjoint(boon.Demo.available_datasets())
    with pytest.raises(ModuleNotFoundError):
        __import__("boon.barriers")


def test_cli_players_reports_catalog_download_failure(upstream, monkeypatch, tmp_path):
    from types import SimpleNamespace

    import polars as pl
    from boon import cli
    from typer.testing import CliRunner

    path = tmp_path / "demo.dem"
    path.touch()
    monkeypatch.setattr(
        cli,
        "_open",
        lambda path: SimpleNamespace(players=pl.DataFrame({"hero_id": [1]})),
    )

    def offline(url):
        raise urllib.error.URLError("offline")

    monkeypatch.setattr(data, "_request", offline)
    result = CliRunner().invoke(cli.app, ["players", str(path)])
    assert result.exit_code == 1
    assert "error:" in result.output
    assert "offline" in result.output
