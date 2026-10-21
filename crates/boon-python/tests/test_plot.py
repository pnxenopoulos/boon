"""Plot behavior uses small tables and local images; no replay parse or network."""

import json
import subprocess
import sys
from dataclasses import dataclass, field

import matplotlib
import matplotlib.pyplot as plt
import numpy as np
import polars as pl
import pytest
from boon import data, plot, team_names
from matplotlib.collections import LineCollection, PathCollection
from matplotlib.colors import to_rgba
from matplotlib.offsetbox import AnnotationBbox, DrawingArea
from matplotlib.patches import Rectangle
from matplotlib.text import Annotation

matplotlib.use("Agg")


@dataclass
class Match:
    players: pl.DataFrame
    totals: pl.DataFrame
    positions: pl.DataFrame
    match_id: int = 42
    map_name: str = "dl_midtown"
    requested_tick: int | None = None
    requested_datasets: list[str] | None = None
    winning_team_num: int | None = 2
    regulation_clock_time: str | None = "02:00"
    game_over_tick: int | None = 100
    objectives: pl.DataFrame = field(default_factory=pl.DataFrame)
    troopers: pl.DataFrame = field(default_factory=pl.DataFrame)
    ziplines: pl.DataFrame = field(default_factory=pl.DataFrame)
    purchases: pl.DataFrame = field(
        default_factory=lambda: pl.DataFrame(
            schema={
                "tick": pl.Int32,
                "steam_id": pl.UInt64,
                "ability_id": pl.UInt32,
                "change": pl.String,
                "upgraded_from_ability_ids": pl.List(pl.UInt32),
            }
        )
    )

    def summary(self):
        return {"snapshots": self.totals}

    def snapshots(self, datasets=None, *, ticks):
        self.requested_tick = ticks
        self.requested_datasets = datasets
        frames = {
            "player_ticks": self.positions,
            "troopers": self.troopers,
            "ziplines": self.ziplines,
        }
        return {name: frames[name] for name in datasets} if datasets else self.positions

    def get_item_purchases(self, *, data_version):
        return self.purchases

    def tick_to_match_clock(self, tick):
        return "01:23"


@pytest.fixture
def match():
    roster = pl.DataFrame(
        {
            "steam_id": [11, 12, 31],
            "hero_id": [1, 2, 3],
            "team_num": [2, 2, 3],
            "player_name": ["First", "Second", "Third"],
        }
    )
    totals = pl.DataFrame(
        [
            {
                "steam_id": steam,
                "hero_id": hero,
                "snapshot_time_s": time,
                "net_worth": time + steam,
                "kills": time // 60 + hero,
                "deaths": hero,
                "assists": hero + 1,
                "player_damage": time * hero,
                "boss_damage": hero * 20,
                "player_healing": hero * 7,
                "barrier_absorption": hero * 9,
            }
            for time in [120, 60]  # Input order must not select an older snapshot.
            for steam, hero in [(11, 1), (12, 2), (31, 3)]
        ]
    )
    positions = pl.DataFrame(
        {
            "tick": [100, 100, 100],
            "steam_id": [11, 12, 31],
            "hero_id": [1, 2, 3],
            "x": [0.0, 10.0, 20.0],
            "y": [0.0, 10.0, 20.0],
            "z": [-5.0, 0.0, 5.0],
            "health": [80, 0, 50],
            "max_health": [100, 100, 100],
            "is_alive": [True, False, True],
            "yaw": [0.0, 90.0, 180.0],
        }
    )
    return Match(roster, totals, positions)


@pytest.fixture
def assets(tmp_path, monkeypatch):
    folder = tmp_path / "9"
    image_dir = folder / "images"
    image_dir.mkdir(parents=True)
    image = np.ones((16, 16, 4))
    index = {
        "map": {"name": "dl_midtown", "bounds": [-10, -20, 30, 40], "images": {}},
        "heroes": {},
        "items": {},
    }
    for layer, stem in {
        "surface": "minimap_midtown_mid_psd",
        "tunnels": "minimap_midtown_mid_tunnels_psd",
        "rat_tunnels": "minimap_midtown_rat_tunnels_psd",
    }.items():
        name = f"{layer}.png"
        plt.imsave(image_dir / name, image)
        index["map"]["images"][layer] = {
            "path": name,
            "resource": f"panorama/images/minimap/base/{stem}.vtex_c",
        }
    for hero_id in [1, 2, 3]:
        name = f"hero{hero_id}.png"
        plt.imsave(image_dir / name, image)
        index["heroes"][str(hero_id)] = {
            "display_name": f"Test hero {hero_id}",
            "images": {"icon": {"path": name}, "minimap": {"path": name}},
        }
    for item_id in [101, 102, 103]:
        name = f"item{item_id}.png"
        plt.imsave(image_dir / name, image)
        index["items"][str(item_id)] = {
            "display_name": f"Test Item {item_id}",
            "images": {"shop": {"path": name}},
        }
    (image_dir / "images.json").write_text(json.dumps(index))
    (folder / "abilities.json").write_text(
        json.dumps(
            {
                "records": [
                    {"ability_id": item_id, "definition": {"m_eItemSlotType": slot}}
                    for item_id, slot in [
                        (101, "EItemSlotType_WeaponMod"),
                        (102, "EItemSlotType_Armor"),
                        (103, "EItemSlotType_Tech"),
                    ]
                ],
            }
        )
    )
    calls = []

    def update(version=None, *, images=(), **kwargs):
        calls.append((version, set(images)))
        return folder

    monkeypatch.setattr(data, "update", update)
    monkeypatch.setattr(data, "resolve_version", lambda version=None: version or "9")
    return image_dir, index, calls


@pytest.fixture(autouse=True)
def close_figures():
    yield
    plt.close("all")


def labels(ax):
    return [text.get_text() for text in ax.texts]


def team_line(ax):
    return next(line for line in ax.lines if line.get_label() == team_names()[2])


def test_summary_uses_latest_totals_for_each_player(match):
    _, ax = plot.summary(match, columns=["net_worth"], portraits=False)
    assert {"131", "132", "151"} <= set(labels(ax))
    assert "71" not in labels(ax)


def test_summary_keeps_healing_and_barrier_absorption_separate(match):
    _, ax = plot.summary(
        match, columns=["player_healing", "barrier_absorption"], portraits=False
    )
    assert {"7", "9"} <= set(labels(ax))
    assert "16" not in labels(ax)


def test_summary_uses_steam_ids_for_roster_names(match):
    match.players = match.players.reverse()
    _, ax = plot.summary(match, portraits=False)
    assert {"First", "Second", "Third"} <= set(labels(ax))


def test_summary_uses_the_current_hero_after_a_swap(match):
    match.totals = match.totals.with_columns(
        pl.when((pl.col("steam_id") == 11) & (pl.col("snapshot_time_s") == 120))
        .then(99)
        .otherwise(pl.col("hero_id"))
        .alias("hero_id")
    )
    _, ax = plot.summary(match, portraits=False)
    assert "Hero 99" in labels(ax)


def test_unknown_steam_ids_do_not_merge_different_heroes(match):
    match.totals = match.totals.with_columns(
        pl.lit(None, dtype=pl.Int64).alias("steam_id")
    )
    _, ax = plot.summary(match, portraits=False)
    assert {"Hero 1", "Hero 2", "Hero 3"} <= set(labels(ax))


def test_text_summary_does_not_request_images(match, assets):
    plot.summary(match, portraits=False)
    assert assets[2] == []


def test_summary_requests_only_hero_images(match, assets):
    plot.summary(match, version="9")
    assert assets[2] == [("9", {"heroes"})]


def test_missing_hero_portrait_keeps_the_player_row(match, assets):
    image_dir, index, _ = assets
    index["heroes"].pop("1")
    (image_dir / "images.json").write_text(json.dumps(index))
    _, ax = plot.summary(match)
    assert "First" in labels(ax)


def test_summary_draws_into_an_existing_axes(match):
    fig, ax = plt.subplots()
    assert plot.summary(match, portraits=False, ax=ax) == (fig, ax)


def test_empty_summary_is_reported(match):
    match.totals = match.totals.clear()
    with pytest.raises(ValueError, match="no recorded"):
        plot.summary(match, portraits=False)


@pytest.mark.parametrize("columns", [[], ["kills", "kills"], ["steam_id"], ["unknown"]])
def test_invalid_summary_metrics_are_rejected(match, columns):
    with pytest.raises(ValueError):
        plot.summary(match, columns=columns, portraits=False)


def test_timeline_sums_recorded_totals_by_team(match):
    _, ax = plot.timeline(match)
    line = team_line(ax)
    assert np.asarray(line.get_xdata()).tolist() == [1, 2]
    assert np.asarray(line.get_ydata()).tolist() == [143, 263]


def test_timeline_keeps_unknown_totals_as_gaps(match):
    match.totals = match.totals.with_columns(
        pl.when((pl.col("steam_id") == 11) & (pl.col("snapshot_time_s") == 120))
        .then(None)
        .otherwise(pl.col("net_worth"))
        .alias("net_worth")
    )
    _, ax = plot.timeline(match)
    assert np.isnan(np.asarray(team_line(ax).get_ydata())[1])


def test_timeline_keeps_missing_players_as_gaps(match):
    match.totals = match.totals.filter(
        ~((pl.col("steam_id") == 11) & (pl.col("snapshot_time_s") == 120))
    )
    _, ax = plot.timeline(match)
    assert np.isnan(np.asarray(team_line(ax).get_ydata())[1])


def test_timeline_does_not_overflow_unsigned_totals(match):
    match.totals = match.totals.with_columns(
        pl.lit(3_000_000_000, dtype=pl.UInt32).alias("net_worth")
    )
    _, ax = plot.timeline(match)
    assert np.asarray(team_line(ax).get_ydata()).tolist() == [
        6_000_000_000,
        6_000_000_000,
    ]


def test_timeline_does_not_request_images(match, assets):
    plot.timeline(match, "player_damage")
    assert assets[2] == []


def test_radar_uses_versioned_bounds_and_world_y_direction(assets):
    _, ax = plot.radar(version="9")
    assert list(ax.images[0].get_extent()) == [-10, 30, -20, 40]
    assert ax.images[0].origin == "upper"
    assert ax.get_ylim() == (-20, 40)


@pytest.mark.parametrize("layer", list(plot.MapLayer))
def test_each_minimap_layer_is_available(assets, layer):
    _, ax = plot.radar(layer=layer)
    assert len(ax.images) == 1


def test_unknown_minimap_layer_is_rejected(assets):
    with pytest.raises(ValueError, match="layer must be"):
        plot.radar(layer="other")


def test_missing_bounds_require_explicit_calibration(assets):
    image_dir, index, _ = assets
    index["map"]["bounds"] = None
    (image_dir / "images.json").write_text(json.dumps(index))
    with pytest.raises(ValueError, match="no map bounds"):
        plot.radar()


def test_explicit_bounds_support_older_image_bundles(assets):
    image_dir, index, _ = assets
    index["map"]["bounds"] = None
    (image_dir / "images.json").write_text(json.dumps(index))
    _, ax = plot.radar(bounds=plot.MapBounds(-1, -2, 3, 4))
    assert list(ax.images[0].get_extent()) == [-1, 3, -2, 4]


@pytest.mark.parametrize(
    "values", [(0, 0, 0, 1), (2, 0, 1, 2), (0, 0, 1, float("inf"))]
)
def test_invalid_map_bounds_are_rejected(values):
    with pytest.raises(ValueError, match="map bounds"):
        plot.MapBounds(*values)


def test_frame_draws_world_positions_without_rescaling(match, assets):
    _, ax = plot.frame(match.positions, portraits=False)
    assert [
        tuple(np.asarray(marker.get_offsets())[0]) for marker in ax.collections
    ] == [(0, 0), (10, 10), (20, 20)]


def test_position_labels_join_by_steam_id(match, assets):
    _, ax = plot.positions(match, tick=100, portraits=False, labels=True)
    assert {"First", "Second", "Third"} <= set(labels(ax))


def test_positions_queries_the_requested_exact_tick(match, assets):
    plot.positions(match, tick=100, portraits=False)
    assert match.requested_tick == 100


def test_positions_rejects_a_different_replay_map(match, assets):
    with pytest.raises(ValueError, match="does not match replay map"):
        plot.positions(match, tick=100, map_name="different_map")


def test_frame_rejects_mixed_ticks(match, assets):
    rows = pl.concat(
        [
            match.positions,
            match.positions.with_columns(pl.lit(101, dtype=pl.Int64).alias("tick")),
        ]
    )
    with pytest.raises(ValueError, match="one tick"):
        plot.frame(rows)
    assert assets[2] == []


def test_empty_frame_still_draws_the_map(match, assets):
    _, ax = plot.frame(match.positions.clear(), portraits=False)
    assert len(ax.images) == 1 and len(ax.collections) == 0


def test_nonfinite_positions_are_omitted(match, assets):
    rows = match.positions.with_columns(pl.lit(float("nan")).alias("x"))
    _, ax = plot.frame(rows, portraits=False)
    assert len(ax.collections) == 0


def test_altitude_filter_is_explicit_and_half_open(match, assets):
    _, ax = plot.frame(match.positions, z_range=(0, 5), portraits=False)
    assert len(ax.collections) == 1
    assert tuple(np.asarray(ax.collections[0].get_offsets())[0]) == (10, 10)


def test_facing_ticks_use_world_yaw(match, assets):
    _, ax = plot.frame(
        match.positions.head(1), portraits=False, labels=False, facing=True
    )
    direction = next(
        text
        for text in ax.texts
        if isinstance(text, Annotation) and text.arrow_patch is not None
    )
    assert direction.xyann == pytest.approx((18, 0))


def test_health_bars_use_recorded_health_fraction(match, assets):
    _, ax = plot.frame(match.positions.head(1), portraits=False)
    box = next(
        artist
        for artist in ax.artists
        if isinstance(artist, AnnotationBbox)
        and isinstance(artist.offsetbox, DrawingArea)
    )
    fill = box.offsetbox.get_children()[1]
    assert isinstance(fill, Rectangle)
    assert fill.get_width() == pytest.approx(24 * 0.8)


@pytest.mark.parametrize("kind", ["summary", "timeline", "positions"])
def test_plot_can_be_saved(match, assets, tmp_path, kind):
    if kind == "summary":
        fig, _ = plot.summary(match)
    elif kind == "timeline":
        fig, _ = plot.timeline(match)
    else:
        fig, _ = plot.positions(match, tick=100)
    output = tmp_path / f"{kind}.png"
    fig.savefig(output)
    assert output.read_bytes().startswith(b"\x89PNG")


def test_loading_map_in_replay_header_does_not_choose_the_background(match, assets):
    match.map_name = "start"
    _, ax = plot.positions(match, tick=100)
    assert len(ax.images) == 1


def test_importing_boon_needs_no_plotting_extra():
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "import sys; sys.modules['matplotlib'] = None; import boon; assert 'boon.plot' not in sys.modules",
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr


def test_missing_plotting_extra_has_an_install_hint():
    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "import sys; sys.modules['matplotlib'] = None; from boon import plot",
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0
    assert "boon-deadlock[plot]" in result.stderr


def test_filtering_players_keeps_team_colors(match, assets):
    _, full = plot.frame(match.positions, roster=match.players, portraits=False)
    _, selected = plot.frame(
        match.positions.tail(1), roster=match.players, portraits=False
    )
    np.testing.assert_array_equal(
        full.collections[-1].get_facecolor(), selected.collections[0].get_facecolor()
    )


@pytest.mark.parametrize(
    ("value", "label"), [(500000, "500k"), (18300, "18.3k"), (1000, "1k"), (999, "999")]
)
def test_summary_formats_thousands_with_k(match, value, label):
    match.totals = match.totals.with_columns(pl.lit(value).alias("net_worth"))
    _, ax = plot.summary(match, columns=["net_worth"], portraits=False)
    assert label in labels(ax)


@pytest.mark.parametrize("title", [None, "League Final"])
def test_summary_title_can_be_customized(match, title):
    _, ax = plot.summary(match, portraits=False, title=title)
    assert ax.get_title() == ("Match 42" if title is None else title)


def test_summary_shows_recorded_winner_and_match_duration(match):
    _, ax = plot.summary(match, portraits=False)
    assert f"Winner: {team_names()[2]} · Duration: 02:00" in labels(ax)


def test_incomplete_match_does_not_guess_a_winner_or_duration(match):
    match.winning_team_num = None
    match.regulation_clock_time = None
    _, ax = plot.summary(match, portraits=False)
    assert "Winner: Unknown · Duration: Unknown" in labels(ax)


def test_summary_portrait_centers_match_row_centers(match, assets):
    _, ax = plot.summary(match)
    centers = [
        patch.get_y() + patch.get_height() / 2
        for patch in ax.patches
        if isinstance(patch, Rectangle) and patch.get_width() > 10
    ]
    assert [
        artist.xy[1] for artist in ax.artists if isinstance(artist, AnnotationBbox)
    ] == pytest.approx(centers)


def test_timeline_uses_title_case_labels(match):
    _, ax = plot.timeline(match, "player_damage")
    assert ax.get_xlabel() == "Time Since Start"
    assert ax.get_title(loc="left") == "Player Damage by Team"


def test_timeline_axis_formats_thousands_with_k(match):
    _, ax = plot.timeline(match)
    assert ax.yaxis.get_major_formatter()(500000, 0) == "500k"


@pytest.fixture
def shop(match):
    # Includes a sale, a repeated upgrade, a failed purchase, and post-game activity.
    match.purchases = pl.DataFrame(
        {
            "tick": [1, 2, 3, 4, 4, 5, 6, 101],
            "steam_id": [11] * 8,
            "ability_id": [101, 102, 101, 102, 103, 103, 101, 103],
            "change": [
                "purchased",
                "purchased",
                "sold",
                "sold",
                "purchased",
                "upgraded",
                "failure",
                "sold",
            ],
            "upgraded_from_ability_ids": [[], [], [], [], [102], [], [], []],
        }
    )
    return match


def test_summary_builds_keep_owned_items_only(shop, assets):
    _, ax = plot.summary(shop, builds=True, portraits=False)
    icons = [artist for artist in ax.artists if isinstance(artist, AnnotationBbox)]
    assert len(icons) == 1  # Item 103 remains; the upgrade must not duplicate it.


def test_summary_builds_request_only_item_images(shop, assets):
    plot.summary(shop, builds=True, portraits=False, version="9")
    assert assets[2] == [("9", {"items"})]


def test_missing_item_image_keeps_its_name(shop, assets):
    folder, index, _ = assets
    index["items"]["103"]["images"]["shop"] = None
    (folder / "images.json").write_text(json.dumps(index))
    _, ax = plot.summary(shop, builds=True, portraits=False)
    assert "Test Item\n103" in labels(ax)


def test_facing_directions_are_disabled_by_default(match, assets):
    _, ax = plot.frame(match.positions, portraits=False)
    assert not any(
        isinstance(text, Annotation) and text.arrow_patch is not None
        for text in ax.texts
    )


@pytest.fixture
def map_entities(match):
    match.objectives = pl.DataFrame(
        {
            "tick": [10, 20, 30, 110],
            "entity_id": [7, 8, 7, 8],
            "entity_serial": [1, 1, 2, 1],
            "objective_type": ["walker", "patron", "walker", "patron"],
            "team_num": [2, 3, 2, 3],
            "health": [0, 50, 80, 0],
            "max_health": [100] * 4,
            "phase": [0, 1, 0, 1],
            "x": [0.0, 10.0, 20.0, 10.0],
            "y": [0.0, 10.0, 20.0, 10.0],
            "z": [0.0] * 4,
        }
    )
    match.troopers = pl.DataFrame(
        {
            "tick": [100],
            "team_num": [2],
            "x": [5.0],
            "y": [5.0],
            "z": [0.0],
            "health": [20],
        }
    )
    return match


def test_objectives_use_the_latest_recorded_entity_lifetime(map_entities, assets):
    _, ax = plot.positions(map_entities, tick=100, objectives=True, portraits=False)
    assert "Walker · 80%" in labels(ax)
    assert "Walker · Destroyed" not in labels(ax)


def test_objectives_do_not_read_future_state(map_entities, assets):
    _, ax = plot.positions(map_entities, tick=100, objectives=True, portraits=False)
    assert any(label.startswith("Patron · 50%") for label in labels(ax))


def test_destroyed_objectives_use_destroyed_state(map_entities, assets):
    map_entities.objectives = map_entities.objectives.head(1)
    _, ax = plot.positions(map_entities, tick=100, objectives=True)
    assert "Walker · Destroyed" in labels(ax)


def test_troopers_share_the_exact_tick_player_query(map_entities, assets):
    _, ax = plot.positions(map_entities, tick=100, troopers=True, portraits=False)
    assert map_entities.requested_datasets == ["player_ticks", "troopers"]
    assert tuple(np.asarray(ax.collections[0].get_offsets())[0]) == (5, 5)


def test_frame_rejects_overlays_from_a_different_tick(map_entities, assets):
    with pytest.raises(ValueError, match="one tick"):
        plot.frame(
            map_entities.positions,
            troopers=map_entities.troopers.with_columns(pl.lit(99).alias("tick")),
        )


def test_altitude_filter_applies_to_map_overlays(map_entities, assets):
    _, ax = plot.positions(
        map_entities,
        tick=100,
        objectives=True,
        troopers=True,
        portraits=False,
        z_range=(1, 6),
    )
    assert len(ax.collections) == 1  # Only the player at z=5 remains.


def test_summary_build_upgrade_removes_linked_components(shop, assets):
    shop.purchases = shop.purchases.filter(
        ~((pl.col("ability_id") == 102) & (pl.col("change") == "sold"))
    )
    _, ax = plot.summary(shop, builds=True, portraits=False)
    assert (
        len([artist for artist in ax.artists if isinstance(artist, AnnotationBbox)])
        == 1
    )


def test_builds_ignore_unmatched_accounts(shop, assets):
    shop.purchases = shop.purchases.with_columns(
        pl.lit(None, dtype=pl.UInt64).alias("steam_id")
    )
    _, ax = plot.summary(shop, builds=True, portraits=False)
    assert not ax.artists


def test_objective_with_unknown_health_is_not_destroyed(map_entities, assets):
    map_entities.objectives = map_entities.objectives.head(1).with_columns(
        pl.lit(None, dtype=pl.Int64).alias("health")
    )
    _, ax = plot.positions(map_entities, tick=100, objectives=True)
    assert "Walker · Unknown" in labels(ax)


def test_respawned_objective_replaces_old_entity_at_the_same_site(map_entities, assets):
    original = map_entities.objectives.head(1)
    map_entities.objectives = pl.concat(
        [
            original,
            original.with_columns(
                pl.lit(50, dtype=pl.Int64).alias("tick"),
                pl.lit(20, dtype=pl.Int64).alias("entity_id"),
                pl.lit(90, dtype=pl.Int64).alias("health"),
            ),
        ]
    )
    _, ax = plot.positions(map_entities, tick=100, objectives=True)
    assert "Walker · 90%" in labels(ax)
    assert "Walker · Destroyed" not in labels(ax)


def test_nearby_objective_captions_do_not_overlap(map_entities, assets):
    rows = map_entities.objectives.head(2).with_columns(
        pl.lit(100).alias("tick"), pl.lit(0.0).alias("x"), pl.lit(0.0).alias("y")
    )
    fig, ax = plot.frame(map_entities.positions.clear(), objectives=rows)
    fig.canvas.draw()
    captions = [text for text in ax.texts if text.get_text()]
    assert not captions[0].get_window_extent().overlaps(captions[1].get_window_extent())


@pytest.mark.parametrize(
    ("count", "row_sizes"), [(10, [5, 5]), (11, [5, 6]), (12, [6, 6])]
)
def test_inline_builds_keep_items_in_two_rows(match, assets, count, row_sizes):
    folder, index, _ = assets
    item_ids = list(range(1000, 1000 + count))
    for item_id in item_ids:
        index["items"][str(item_id)] = index["items"]["101"]
    (folder / "images.json").write_text(json.dumps(index))
    match.purchases = pl.DataFrame(
        {
            "tick": [1] * count,
            "steam_id": [11] * count,
            "ability_id": item_ids,
            "change": ["purchased"] * count,
            "upgraded_from_ability_ids": [[]] * count,
        }
    )
    _, ax = plot.summary(match, builds=True, portraits=False)
    positions = [
        artist.xybox for artist in ax.artists if isinstance(artist, AnnotationBbox)
    ]
    assert (
        sorted(
            len({x for x, row in positions if row == y})
            for y in {row for _, row in positions}
        )
        == row_sizes
    )


@pytest.mark.parametrize(
    ("slot_type", "color"),
    [
        ("EItemSlotType_WeaponMod", "#df9b46"),
        ("EItemSlotType_Armor", "#77b255"),
        ("EItemSlotType_Tech", "#c27ce0"),
        ("EItemSlotType_Invalid", "#9caabe"),
    ],
)
def test_item_border_uses_the_catalog_type(shop, assets, slot_type, color):
    catalog = assets[0].parent / "abilities.json"
    records = json.loads(catalog.read_text())
    records["records"][-1]["definition"]["m_eItemSlotType"] = slot_type
    catalog.write_text(json.dumps(records))
    _, ax = plot.summary(shop, builds=True, portraits=False)
    icon = next(artist for artist in ax.artists if isinstance(artist, AnnotationBbox))
    assert icon.patch.get_visible()
    assert icon.patch.get_edgecolor() == to_rgba(color)


def test_item_borders_do_not_change_team_colors(shop, assets):
    shop.purchases = shop.purchases.with_columns(pl.lit(12).alias("steam_id"))
    _, plain = plot.summary(shop, portraits=False)
    _, builds = plot.summary(shop, builds=True, portraits=False)
    assert [
        patch.get_facecolor()
        for patch in builds.patches
        if isinstance(patch, Rectangle) and patch.get_width() == 0.04
    ] == [
        patch.get_facecolor()
        for patch in plain.patches
        if isinstance(patch, Rectangle) and patch.get_width() == 0.04
    ]


@pytest.mark.parametrize("kind", ["summary", "timeline", "positions"])
def test_plot_background_uses_the_dark_palette(match, assets, kind):
    if kind == "summary":
        _, ax = plot.summary(match)
    elif kind == "timeline":
        _, ax = plot.timeline(match)
    else:
        _, ax = plot.positions(match, tick=100)
    assert ax.get_facecolor() == to_rgba("#101720")


@pytest.mark.parametrize("kind", ["summary", "timeline", "positions"])
def test_plot_does_not_change_global_styles(match, assets, kind):
    before = dict(matplotlib.rcParams)
    if kind == "summary":
        plot.summary(match)
    elif kind == "timeline":
        plot.timeline(match)
    else:
        plot.positions(match, tick=100)
    assert dict(matplotlib.rcParams) == before


def test_unknown_team_uses_neutral_color(match, assets):
    rows = match.positions.head(1).with_columns(pl.lit(99).alias("team_num"))
    _, ax = plot.frame(rows, portraits=False)
    np.testing.assert_array_equal(
        ax.collections[0].get_facecolor()[0],
        to_rgba("#9caabe"),
    )


@pytest.fixture
def zipline_match(map_entities):
    map_entities.ziplines = pl.DataFrame(
        {
            "tick": [100] * 4,
            "node_handle": [16385, 16386, 16387, 16388],
            "team_num": [2, 3, 0, None],
            "lane": [1, 4, 6, None],
            "x": [-5.0, 0.0, 5.0, 15.0],
            "y": [0.0, 5.0, 0.0, 15.0],
            "z": [0.0, 0.0, 10.0, 0.0],
            "connections": [[16386], [16385], [], None],
        },
        schema_overrides={"node_handle": pl.UInt32, "connections": pl.List(pl.UInt32)},
    )
    return map_entities


def zipline_lines(ax):
    return [line for line in ax.collections if isinstance(line, LineCollection)]


def test_positions_does_not_request_ziplines_by_default(match, assets):
    plot.positions(match, tick=100, portraits=False)
    assert match.requested_datasets is None


@pytest.mark.parametrize("troopers", [False, True])
def test_positions_adds_ziplines_to_the_exact_tick_query(
    zipline_match, assets, troopers
):
    plot.positions(
        zipline_match, tick=100, ziplines=True, troopers=troopers, portraits=False
    )
    expected = (
        ["player_ticks", "troopers", "ziplines"]
        if troopers
        else ["player_ticks", "ziplines"]
    )
    assert zipline_match.requested_datasets == expected


def test_zipline_routes_use_recorded_ownership_colors(zipline_match, assets):
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    np.testing.assert_allclose(
        zipline_lines(ax)[1].get_colors(),
        [to_rgba("#dfb85b"), to_rgba("#7ab6e9")],
    )


@pytest.mark.parametrize("team", [None, 0, 99])
def test_unknown_zipline_owners_use_neutral_routes(zipline_match, assets, team):
    zipline_match.ziplines = zipline_match.ziplines.with_columns(
        pl.lit(team).alias("team_num")
    )
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    np.testing.assert_allclose(
        zipline_lines(ax)[1].get_colors(), [to_rgba("#9caabe")] * 2
    )


@pytest.mark.parametrize(
    ("lane", "name", "color"),
    [
        (1, "York", "#fbbf24"),
        (4, "Broadway", "#60a5fa"),
        (6, "Greenwich", "#34d399"),
        (None, "Unknown Lane", "#9caabe"),
        (99, "Lane 99", "#9caabe"),
    ],
)
def test_zipline_badges_use_lane_names_and_colors(
    zipline_match, assets, lane, name, color
):
    zipline_match.ziplines = zipline_match.ziplines.with_columns(
        pl.lit(lane).alias("lane")
    )
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    badge = next(text for text in ax.texts if text.get_text() == name)
    patch = badge.get_bbox_patch()
    assert patch is not None
    assert patch.get_facecolor() == to_rgba(color)


def test_zipline_connections_draw_each_recorded_pair_once(zipline_match, assets):
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    np.testing.assert_array_equal(
        zipline_lines(ax)[0].get_segments(),
        [[[-5, 0], [-2.5, 2.5]], [[0, 5], [-2.5, 2.5]]],
    )


def test_zipline_badges_name_only_connected_lanes(zipline_match, assets):
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    assert {text.get_text() for text in ax.texts} == {"York", "Broadway"}


def test_zipline_route_outlines_use_the_background_color(zipline_match, assets):
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    np.testing.assert_allclose(zipline_lines(ax)[0].get_colors(), [to_rgba("#101720")])


def test_zipline_control_boundary_marks_the_connection_midpoint(zipline_match, assets):
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    assert [
        (np.asarray(line.get_xdata())[0], np.asarray(line.get_ydata())[0])
        for line in ax.lines
    ] == [(-2.5, 2.5)]


def test_zipline_control_boundary_is_perpendicular_to_the_route(zipline_match, assets):
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    marker = np.asarray(ax.lines[0].get_marker())
    assert np.dot(marker[1] - marker[0], [5, 5]) == pytest.approx(0)


@pytest.mark.parametrize(("a", "b"), [(2, 2), (None, 3), (2, None)])
def test_equal_or_unknown_owners_do_not_add_control_ticks(zipline_match, assets, a, b):
    zipline_match.ziplines = zipline_match.ziplines.with_columns(
        pl.Series("team_num", [a, b, 0, None])
    )
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    assert not ax.lines


def test_zero_length_zipline_connections_are_omitted(zipline_match, assets):
    zipline_match.ziplines = zipline_match.ziplines.with_columns(
        pl.lit(0.0).alias("x"), pl.lit(0.0).alias("y")
    )
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    assert not zipline_lines(ax)


def test_zipline_overlay_does_not_add_node_markers(zipline_match, assets):
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    assert len([c for c in ax.collections if isinstance(c, PathCollection)]) == 3


def test_missing_lane_column_uses_an_unknown_lane_badge(zipline_match, assets):
    zipline_match.ziplines = zipline_match.ziplines.drop("lane")
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    assert "Unknown Lane" in labels(ax)


def test_zipline_legend_explains_control_boundaries(zipline_match, assets):
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    legend = ax.get_legend()
    assert legend is not None
    assert "Control Boundary" in {text.get_text() for text in legend.get_texts()}


@pytest.mark.parametrize("target", [None, 32770])
def test_zipline_connections_require_observed_full_handles(
    zipline_match, assets, target
):
    # 32770 has the same slot as 16386 but a different serial number.
    zipline_match.ziplines = zipline_match.ziplines.with_columns(
        pl.Series("connections", [[target], [], [], None], dtype=pl.List(pl.UInt32))
    )
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    assert not any(isinstance(c, LineCollection) for c in ax.collections)


def test_frame_rejects_zipline_rows_from_a_different_tick(zipline_match, assets):
    with pytest.raises(ValueError, match="one tick"):
        plot.frame(
            zipline_match.positions,
            ziplines=zipline_match.ziplines.with_columns(pl.lit(99).alias("tick")),
        )


def test_connections_to_filtered_zipline_nodes_are_omitted(zipline_match, assets):
    _, ax = plot.positions(
        zipline_match, tick=100, ziplines=True, portraits=False, z_range=(5, 15)
    )
    assert not any(isinstance(c, LineCollection) for c in ax.collections)


@pytest.mark.parametrize("x", [None, float("nan"), float("inf"), 100.0])
def test_invalid_zipline_positions_are_omitted(zipline_match, assets, x):
    zipline_match.ziplines = zipline_match.ziplines.with_columns(pl.lit(x).alias("x"))
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    assert not any(c.get_zorder() < 1 for c in ax.collections)


def test_zipline_overlays_stay_below_players(zipline_match, assets):
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    assert max(c.get_zorder() for c in zipline_lines(ax)) < min(
        c.get_zorder() for c in ax.collections if isinstance(c, PathCollection)
    )


def test_empty_zipline_frame_keeps_player_positions(zipline_match, assets):
    zipline_match.ziplines = zipline_match.ziplines.clear()
    _, ax = plot.positions(zipline_match, tick=100, ziplines=True, portraits=False)
    assert len(ax.collections) == 3
