"""Draw recorded match summaries and player positions with versioned boon-data.

Install ``boon-deadlock[plot]`` and import this module explicitly. Importing
``boon`` alone does not load matplotlib or download images. All functions return
matplotlib figures and axes, which can be saved or extended by the caller.
"""

from __future__ import annotations

import json
import math
from collections.abc import Sequence
from dataclasses import dataclass
from enum import StrEnum
from pathlib import Path
from textwrap import fill
from typing import TYPE_CHECKING, Any, cast

import polars as pl

try:
    import matplotlib.image as mpimg
    import matplotlib.pyplot as plt
    from matplotlib.axes import Axes
    from matplotlib.collections import LineCollection
    from matplotlib.figure import Figure
    from matplotlib.lines import Line2D
    from matplotlib.offsetbox import AnnotationBbox, DrawingArea, OffsetImage
    from matplotlib.patches import Rectangle
    from matplotlib.text import Annotation
    from matplotlib.ticker import FuncFormatter
    from matplotlib.transforms import Bbox
except ImportError as error:  # pragma: no cover - the extra is installed in tests
    raise ImportError(
        "boon.plot requires matplotlib; install with uv add 'boon-deadlock[plot]' "
        "or pip install 'boon-deadlock[plot]'"
    ) from error

from boon import data, patron_phase_names, team_names

if TYPE_CHECKING:
    from numpy.typing import NDArray

    from boon import Demo

__all__ = [
    "MapBounds",
    "MapLayer",
    "frame",
    "positions",
    "radar",
    "summary",
    "timeline",
]

# Private plot defaults. See the repository DESIGN.md for the visual guide.
_PALETTE: dict = {
    "background": "#101720",
    "surface": "#18232f",
    "text": "#e8edf2",
    "muted": "#9caabe",
    "axis": "#354251",
    "health_track": "#334050",
    "health": "#8ed29b",
    "teams": {"2": "#dfb85b", "3": "#7ab6e9"},
    "items": {
        "EItemSlotType_WeaponMod": "#df9b46",
        "EItemSlotType_Armor": "#77b255",
        "EItemSlotType_Tech": "#c27ce0",
    },
}
_FONT: dict = {
    "family": "DejaVu Sans",
    "title": 17,
    "section": 11,
    "body": 10,
    "heading": 9,
    "caption": 8,
    "small": 6,
    "item_fallback": 4,
    "heading_weight": "bold",
}
_NUMBER: dict = {
    "thousand": 1000,
    "suffix": "k",
    "large_decimals": 1,
    "small_decimals": 0,
    "missing": "—",
    "unknown": "Unknown",
    "time_axis": "Time Since Start",
}
_SIZE: dict[str, float] = {
    "hero_portrait": 30,
    "hero_minimap": 20,
    "item": 16,
    "border_width": 1,
    "line_width": 2,
    "timeline_marker": 3,
    "player_marker": 300,
    "objective_marker": 90,
    "trooper_marker": 14,
    "zipline_width": 2.4,
    "zipline_outline_width": 4,
    "zipline_boundary_length": 8,
    "zipline_boundary_width": 1.5,
    "player_outline_width": 1.3,
    "leader_width": 0.5,
    "health_bar_width": 24,
    "health_bar_height": 3,
}
_OPACITY: dict[str, float] = {
    "summary_bar": 0.18,
    "grid": 0.12,
    "inactive": 0.35,
    "objective": 0.9,
    "trooper": 0.8,
    "leader": 0.6,
}
_MARKER: dict[str, str] = {
    "player": "o",
    "dead": "x",
    "objective": "s",
    "destroyed": "x",
    "trooper": ".",
    "sample": "o",
}
_SUMMARY: dict = {
    "minimum_width": 11,
    "figure_column_width": 1.25,
    "figure_row_height": 0.55,
    "column_pitch": 1.35,
    "stats_x": 4,
    "stats_x_with_builds": 5.6,
    "right_padding": 0.75,
    "row_height": 1,
    "build_row_height": 0.5,
    "build_padding": 0.3,
    "team_height": 1.1,
    "bottom_margin": 0.1,
    "header_y": 0.8,
    "title_align": "center",
    "title_pad": 36,
    "subtitle_y": 1.015,
    "team_padding": 0.35,
    "team_gap": 0.75,
    "row_gap": 0.2,
    "team_stripe": 0.04,
    "hero_x": 0.38,
    "name_x": 0.8,
    "name_y": -0.13,
    "hero_name_y": 0.16,
    "bar_width": 1.06,
    "bar_height": 0.4,
    "items_x": 3.45,
    "item_step": 18,
    "item_columns": 5,
    "overflow_columns": 6,
    "item_rows": 2,
}
_TIMELINE: dict = {"width": 11, "height": 5, "title_align": "left", "title_pad": 15}
_MAP: dict = {
    "width": 9,
    "height": 9,
    "title_align": "left",
    "title_pad": 14,
    "facing_length": 18,
    "health_offset": -16,
    "label_offset": 16,
    "caption_offset": 8,
    "caption_gap": 2,
    "zipline_badge_pad": 0.3,
    "caption_avoid_width": 36,
    "caption_avoid_height": 44,
    "caption_search_x": [8, -8, 24, -24, 40, -40],
    "caption_search_y": [0, 12, -12, 24, -24, 36, -36, 48, -48, 72, -72, 96, -96],
}
_LANES: dict[str, dict[str, str]] = {
    "1": {"name": "York", "color": "#fbbf24"},
    "4": {"name": "Broadway", "color": "#60a5fa"},
    "6": {"name": "Greenwich", "color": "#34d399"},
}
_BACKGROUND = _PALETTE["background"]
_FOREGROUND = _PALETTE["text"]
_MUTED = _PALETTE["muted"]
_ITEM_COLORS = _PALETTE["items"]
_METRICS = {
    "net_worth": "Souls",
    "kills": "Kills",
    "deaths": "Deaths",
    "assists": "Assists",
    "player_damage": "Player Damage",
    "boss_damage": "Objective Damage",
    "player_healing": "Healing",
    "barrier_absorption": "Barrier Absorption",
}
_LAYERS = {
    "surface": "minimap_midtown_mid_psd",
    "tunnels": "minimap_midtown_mid_tunnels_psd",
    "rat_tunnels": "minimap_midtown_rat_tunnels_psd",
}


class MapLayer(StrEnum):
    """Midtown minimap image layers accepted by Boon plots.

    SURFACE/surface, TUNNELS/tunnels, and RAT_TUNNELS/rat_tunnels select
    images only. Use z_range separately to select world altitudes.
    """

    SURFACE = "surface"
    TUNNELS = "tunnels"
    RAT_TUNNELS = "rat_tunnels"


@dataclass(frozen=True)
class MapBounds:
    """Validated minimap bounds in Source world units, with positive Y upward.

    Attributes:
        x_min: Minimum world X.
        y_min: Minimum world Y.
        x_max: Maximum world X, greater than x_min.
        y_max: Maximum world Y, greater than y_min.

    Raises:
        ValueError: A bound is not finite or the rectangle has no positive area.
    """

    x_min: float
    y_min: float
    x_max: float
    y_max: float

    def __post_init__(self) -> None:
        """Validate finite, increasing world-coordinate bounds.

        Returns:
            None.

        Raises:
            ValueError: A bound is not finite or the rectangle has no positive area.
        """
        if not all(math.isfinite(v) for v in self.extent) or (
            self.x_min >= self.x_max or self.y_min >= self.y_max
        ):
            raise ValueError(
                "map bounds must be finite and have positive width and height"
            )

    @property
    def extent(self) -> tuple[float, float, float, float]:
        """Return the coordinate order required by matplotlib imshow.

        Returns:
            Tuple (x_min, x_max, y_min, y_max) in Source world units.
        """
        return self.x_min, self.x_max, self.y_min, self.y_max


@dataclass
class _Assets:
    """Private paths and metadata for verified boon-data image files.

    Attributes:
        directory: Local images directory.
        index: Decoded images.json metadata for this release.
    """

    directory: Path
    index: dict

    def hero(self, hero_id: int | None) -> dict:
        """Read optional image metadata for a hero ID.

        Args:
            hero_id: Recorded hero ID or None.

        Returns:
            Hero metadata dict, or an empty dict when the ID is absent.
        """
        return self.index["heroes"].get(str(hero_id), {})

    def image(self, metadata: dict) -> NDArray[Any]:
        """Read a catalog image with matplotlib.

        Args:
            metadata: Image metadata with a relative path.

        Returns:
            NumPy pixel array from matplotlib.image.imread.

        Raises:
            OSError: The image cannot be read.
        """
        return mpimg.imread(self.directory / metadata["path"])


def _assets(version: str | None, groups: Sequence[str]) -> _Assets:
    """Read verified boon-data image metadata.

    Args:
        version: Image catalog version. None selects the newest verified local version, or downloads the latest when none is installed.
        groups: Image groups needed by this plot.

    Returns:
        Private _Assets object with local paths and the image index.
    """
    directory = data.update(data.resolve_version(version), images=groups) / "images"
    index = json.loads((directory / "images.json").read_bytes())
    if "items" in groups:
        catalog = json.loads((directory.parent / "abilities.json").read_bytes())
        for record in catalog["records"]:
            item = index["items"].get(str(record["ability_id"]))
            if item is not None:
                item["slot_type"] = record.get("definition", {}).get("m_eItemSlotType")
    return _Assets(directory, index)


def _axes(ax: Axes | None, figsize: tuple[float, float]) -> tuple[Figure, Axes]:
    """Create or reuse axes and apply private Boon plot styles.

    Args:
        ax: Existing matplotlib Axes, or None to create a figure.
        figsize: New figure width and height in inches.

    Returns:
        Matplotlib Figure and Axes. Global matplotlib styles remain unchanged.
    """
    if ax is None:
        fig, ax = plt.subplots(figsize=figsize, facecolor=_BACKGROUND)
    else:
        fig = cast(Figure, ax.figure)
    fig.set_facecolor(_BACKGROUND)
    ax.set_facecolor(_BACKGROUND)
    ax.tick_params(colors=_MUTED, labelsize=_FONT["body"])
    for label in ax.get_xticklabels() + ax.get_yticklabels():
        label.set_fontfamily(_FONT["family"])
    for spine in ax.spines.values():
        spine.set_color(_PALETTE["axis"])
    return fig, ax


def _with_roster(frame: pl.DataFrame, roster: pl.DataFrame) -> pl.DataFrame:
    """Add missing player names and teams by Steam ID.

    Args:
        frame: Polars DataFrame to read or extend.
        roster: One player-info row per Steam ID; adds missing player_name and team_num fields.

    Returns:
        Polars frame with any missing player_name and team_num columns joined.
    """
    extra = [name for name in ("player_name", "team_num") if name not in frame.columns]
    if not extra:
        return frame
    return frame.join(
        roster.select("steam_id", *extra), on="steam_id", how="left", validate="m:1"
    )


def _teams(frame: pl.DataFrame) -> dict[int | None, str]:
    """Resolve plot colors for teams recorded in a Polars frame.

    Args:
        frame: Polars DataFrame to read or extend.

    Returns:
        Team-to-color dict. Unknown teams and None use gray.
    """
    teams = sorted(
        team for team in frame["team_num"].unique().to_list() if team is not None
    )
    return {
        **{team: _PALETTE["teams"].get(str(team), _MUTED) for team in teams},
        None: _MUTED,
    }


def _team_label(team: int | None) -> str:
    """Resolve a recorded team number for a plot legend.

    Args:
        team: Recorded team number, or None for unknown.

    Returns:
        Known team name or a label that keeps the unknown number.
    """
    return "Unknown Team" if team is None else team_names().get(team, f"Team {team}")


def _metric(frame: pl.DataFrame, name: str) -> str:
    """Validate a numeric post-match summary metric.

    Args:
        frame: Polars DataFrame to read or extend.
        name: Numeric summary column to validate.

    Returns:
        Human-readable metric label.

    Raises:
        ValueError: The column is absent, nonnumeric, or an identity/time field.
    """
    if (
        name not in frame.columns
        or name in {"steam_id", "hero_id", "snapshot_time_s", "lane"}
        or not frame.schema[name].is_numeric()
    ):
        raise ValueError(f"{name!r} is not a numeric summary metric")
    return _METRICS.get(name, name.replace("_", " ").title())


def _snapshots(demo: Demo) -> pl.DataFrame:
    """Read post-match summary snapshots and attach player names.

    Args:
        demo: Boon Demo to read. Reuse the object across related plots.

    Returns:
        Polars summary snapshot frame.

    Raises:
        ValueError: Summary snapshots are empty.
        DemoMessageError: PostMatchDetails is absent or invalid.
    """
    frame = demo.summary()["snapshots"]
    if frame.is_empty():
        raise ValueError("the demo has no recorded post-match summary snapshots")
    return _with_roster(frame, demo.players)


def _portrait(
    ax: Axes,
    assets: _Assets,
    metadata: dict | None,
    xy: tuple[float, float],
    size: float,
    border: str | None = None,
    **kwargs: Any,
) -> None:
    """Place one optional raster image on plot axes.

    Args:
        ax: Existing matplotlib Axes, or None to create a figure.
        assets: Verified local image paths and index.
        metadata: Image metadata with a relative path field. None skips an optional image.
        xy: Image center in axes data coordinates.
        size: Target image width in points.
        border: Optional image-border color.
        **kwargs: Additional matplotlib AnnotationBbox keyword arguments.

    Returns:
        None.
    """
    if metadata is not None:
        image = assets.image(metadata)
        ax.add_artist(
            AnnotationBbox(
                OffsetImage(image, zoom=size / image.shape[1]),
                xy,
                frameon=border is not None,
                pad=0,
                bboxprops={
                    "edgecolor": border,
                    "facecolor": "none",
                    "linewidth": _SIZE["border_width"],
                    "boxstyle": "square,pad=0",
                },
                **kwargs,
            )
        )


def _number(value: float | None) -> str:
    """Format metric text with k for thousands.

    Args:
        value: Numeric metric or None for unknown.

    Returns:
        Display text. Missing values use the missing-value marker.
    """
    if value is None:
        return _NUMBER["missing"]
    if abs(value) >= _NUMBER["thousand"]:
        label = f"{value / _NUMBER['thousand']:.{_NUMBER['large_decimals']}f}"
        return (label.rstrip("0").rstrip(".") if "." in label else label) + _NUMBER[
            "suffix"
        ]
    return f"{value:.{_NUMBER['small_decimals']}f}"


def _builds(demo: Demo, assets: _Assets) -> dict[int, list[int]]:
    """Apply recorded shop changes through game over.

    Args:
        demo: Boon Demo to read. Reuse the object across related plots.
        assets: Verified local image paths and index.

    Returns:
        Steam IDs mapped to ordered item IDs. Earlier unrecorded purchases are not inferred.
    """
    changes = demo.get_item_purchases(data_version=assets.directory.parent.name)
    if demo.game_over_tick is not None:
        changes = changes.filter(pl.col("tick") <= demo.game_over_tick)
    items = assets.index["items"]
    owned: dict[int, dict[int, None]] = {}
    for row in changes.sort("tick", maintain_order=True).iter_rows(named=True):
        steam, item = row["steam_id"], row["ability_id"]
        if steam is None or str(item) not in items:
            continue
        build = owned.setdefault(steam, {})
        if row["change"] in {"purchased", "upgraded"}:
            for component in row["upgraded_from_ability_ids"]:
                build.pop(component, None)
            build[item] = None
        elif row["change"] == "sold":
            build.pop(item, None)
    return {steam: list(items) for steam, items in owned.items()}


def summary(
    demo: Demo,
    *,
    version: str | None = None,
    columns: Sequence[str] = tuple(_METRICS),
    portraits: bool = True,
    builds: bool = False,
    title: str | None = None,
    ax: Axes | None = None,
) -> tuple[Figure, Axes]:
    """Draw each player's latest recorded totals, grouped by team.

    Args:
        demo: Boon Demo to read. Reuse the object across related plots.
        version: Image catalog version. None selects the newest verified local version, or downloads the latest when none is installed.
        columns: Nonempty, unique numeric summary columns. Defaults to souls, K/D/A, player damage, objective damage, healing, and barrier absorption.
        portraits: Draw hero images when True. False uses text or plain markers.
        builds: Draw item builds from recorded purchases, upgrades, and sales when True.
        title: Custom summary title. None uses Match <match_id>.
        ax: Existing matplotlib Axes, or None to create a figure.

    Returns:
        Tuple of matplotlib Figure and Axes. Use fig.savefig(...) to save it.
        Close the figure with matplotlib.pyplot.close(fig) after use.

    Raises:
        ValueError: The summary is empty or requested columns are invalid.
        DemoMessageError: PostMatchDetails is absent or invalid.
        data.DataError: Required catalogs or images cannot be installed or verified.

    Notes:
        Values come from demo.summary(). Healing and barrier absorption remain
        separate. Disable portraits and builds to avoid catalog and image downloads.
        Missing item images show the item name.
    """
    snapshots = _snapshots(demo)
    if not columns or len(set(columns)) != len(columns):
        raise ValueError("summary columns must be nonempty and unique")
    titles = [_metric(snapshots, column) for column in columns]
    # A missing Steam ID must not merge different heroes into one player.
    final = (
        snapshots.with_columns(
            pl.when(pl.col("steam_id").is_null())
            .then(pl.col("hero_id"))
            .otherwise(None)
            .alias("_unknown_hero")
        )
        .sort("snapshot_time_s")
        .group_by("steam_id", "_unknown_hero", maintain_order=True)
        .last()
        .drop("_unknown_hero")
        .sort("team_num", "net_worth", descending=[False, True], nulls_last=True)
    )
    groups = (["heroes"] if portraits else []) + (["items"] if builds else [])
    assets = _assets(version, groups) if groups else None
    inventory = _builds(demo, assets) if builds and assets else {}
    colors = _teams(final)
    teams = final["team_num"].unique(maintain_order=True).to_list()
    build_rows = max(
        _SUMMARY["item_rows"],
        math.ceil(
            max(map(len, inventory.values()), default=0) / _SUMMARY["overflow_columns"]
        ),
    )
    row_height = (
        build_rows * _SUMMARY["build_row_height"] + _SUMMARY["build_padding"]
        if builds
        else _SUMMARY["row_height"]
    )
    stats_x = _SUMMARY["stats_x_with_builds"] if builds else _SUMMARY["stats_x"]
    width = (
        stats_x
        + (len(columns) - 1) * _SUMMARY["column_pitch"]
        + _SUMMARY["right_padding"]
    )
    height = (
        len(final) * row_height
        + len(teams) * _SUMMARY["team_height"]
        + _SUMMARY["bottom_margin"]
    )
    fig, ax = _axes(
        ax,
        (
            max(
                _SUMMARY["minimum_width"],
                stats_x + len(columns) * _SUMMARY["figure_column_width"],
            ),
            height * _SUMMARY["figure_row_height"],
        ),
    )
    ax.set_xlim(0, width)
    ax.set_ylim(height, 0)
    ax.set_axis_off()
    ax.set_title(
        f"Match {demo.match_id}" if title is None else title,
        color=_FOREGROUND,
        loc=_SUMMARY["title_align"],
        pad=_SUMMARY["title_pad"],
        fontsize=_FONT["title"],
        fontfamily=_FONT["family"],
        weight=_FONT["heading_weight"],
    )
    winner = demo.winning_team_num
    result = (
        f"Winner: {_team_label(winner)}"
        if winner is not None
        else "Winner: " + _NUMBER["unknown"]
    )
    ax.text(
        0.5,
        _SUMMARY["subtitle_y"],
        f"{result} · Duration: {demo.regulation_clock_time or _NUMBER['unknown']}",
        transform=ax.transAxes,
        color=_MUTED,
        fontsize=_FONT["body"],
        fontfamily=_FONT["family"],
        ha="center",
        va="bottom",
    )
    for i, heading in enumerate(titles):
        ax.text(
            stats_x + i * _SUMMARY["column_pitch"],
            _SUMMARY["header_y"],
            heading.replace(" ", "\n"),
            ha="center",
            va="bottom",
            color=_MUTED,
            fontsize=_FONT["heading"],
            fontfamily=_FONT["family"],
        )
    maxima = {
        column: cast(float | None, final[column].cast(pl.Float64).max())
        for column in columns
    }
    y = _SUMMARY["header_y"]
    for team in teams:
        color = colors[team]
        label = _team_label(team) + (
            "*" if winner is not None and team == winner else ""
        )
        ax.text(
            0.1,
            y,
            label,
            color=color,
            weight=_FONT["heading_weight"],
            fontsize=_FONT["section"],
            fontfamily=_FONT["family"],
        )
        y += _SUMMARY["team_padding"]
        for row in final.filter(pl.col("team_num").eq_missing(team)).iter_rows(
            named=True
        ):
            y += row_height / 2
            ax.add_patch(
                Rectangle(
                    (0, y - (row_height - _SUMMARY["row_gap"]) / 2),
                    width,
                    row_height - _SUMMARY["row_gap"],
                    facecolor=_PALETTE["surface"],
                    edgecolor="none",
                )
            )
            ax.add_patch(
                Rectangle(
                    (0, y - (row_height - _SUMMARY["row_gap"]) / 2),
                    _SUMMARY["team_stripe"],
                    row_height - _SUMMARY["row_gap"],
                    color=color,
                )
            )
            hero = assets.hero(row["hero_id"]) if assets else {}
            if assets and portraits:
                _portrait(
                    ax,
                    assets,
                    hero.get("images", {}).get("icon"),
                    (_SUMMARY["hero_x"], y),
                    _SIZE["hero_portrait"],
                )
            name = row.get("player_name") or str(
                row["steam_id"] or f"Hero {row['hero_id']}"
            )
            ax.text(
                _SUMMARY["name_x"],
                y + _SUMMARY["name_y"],
                name,
                color=_FOREGROUND,
                fontsize=_FONT["body"],
                fontfamily=_FONT["family"],
                va="center",
            )
            ax.text(
                _SUMMARY["name_x"],
                y + _SUMMARY["hero_name_y"],
                hero.get("display_name") or f"Hero {row['hero_id']}",
                color=_MUTED,
                fontsize=_FONT["caption"],
                fontfamily=_FONT["family"],
                va="center",
            )
            for i, column in enumerate(columns):
                value, maximum = row[column], maxima[column]
                x = stats_x + i * _SUMMARY["column_pitch"]
                if value is not None and maximum is not None and maximum > 0:
                    ax.add_patch(
                        Rectangle(
                            (
                                x - _SUMMARY["bar_width"] / 2,
                                y - _SUMMARY["bar_height"] / 2,
                            ),
                            _SUMMARY["bar_width"] * float(value) / maximum,
                            _SUMMARY["bar_height"],
                            facecolor=color,
                            alpha=_OPACITY["summary_bar"],
                            edgecolor="none",
                        )
                    )
                label = _number(value)
                ax.text(
                    x,
                    y,
                    label,
                    ha="center",
                    va="center",
                    color=_FOREGROUND,
                    fontsize=_FONT["body"],
                    fontfamily=_FONT["family"],
                )
            if builds and assets:
                item_ids = inventory.get(row["steam_id"], [])
                build_columns = (
                    _SUMMARY["overflow_columns"]
                    if len(item_ids) > _SUMMARY["item_rows"] * _SUMMARY["item_columns"]
                    else _SUMMARY["item_columns"]
                )
                for i, item_id in enumerate(item_ids):
                    item = assets.index["items"][str(item_id)]
                    grid_row, grid_column = divmod(i, build_columns)
                    offset = (
                        grid_column * _SUMMARY["item_step"],
                        ((build_rows - 1) / 2 - grid_row) * _SUMMARY["item_step"],
                    )
                    image = item.get("images", {}).get("shop")
                    item_color = _ITEM_COLORS.get(item.get("slot_type"), _MUTED)
                    if image:
                        _portrait(
                            ax,
                            assets,
                            image,
                            (_SUMMARY["items_x"], y),
                            _SIZE["item"],
                            border=item_color,
                            xybox=offset,
                            boxcoords="offset points",
                        )
                    else:
                        ax.annotate(
                            fill(item.get("display_name") or str(item_id), 12),
                            (_SUMMARY["items_x"], y),
                            xytext=offset,
                            textcoords="offset points",
                            ha="center",
                            va="center",
                            color=_MUTED,
                            fontsize=_FONT["item_fallback"],
                            fontfamily=_FONT["family"],
                            bbox={
                                "edgecolor": item_color,
                                "linewidth": _SIZE["border_width"],
                                "facecolor": "none",
                                "pad": 0.5,
                            },
                        )
                if not item_ids:
                    ax.text(
                        _SUMMARY["items_x"],
                        y,
                        "No Recorded Items",
                        color=_MUTED,
                        fontsize=_FONT["caption"],
                        fontfamily=_FONT["family"],
                        va="center",
                    )
            y += row_height / 2
        y += _SUMMARY["team_gap"]
    fig.tight_layout()
    return fig, ax


def timeline(
    demo: Demo,
    metric: str = "net_worth",
    *,
    ax: Axes | None = None,
) -> tuple[Figure, Axes]:
    """Draw team totals at the recorded summary times.

    Args:
        demo: Boon Demo to read. Reuse the object across related plots.
        metric: Numeric summary column, such as net_worth (default), player_damage, or player_healing.
        ax: Existing matplotlib Axes, or None to create a figure.

    Returns:
        Tuple of matplotlib Figure and Axes. Use fig.savefig(...) to save it.
        Close the figure with matplotlib.pyplot.close(fig) after use.

    Raises:
        ValueError: The metric is invalid or summary snapshots are empty.
        DemoMessageError: PostMatchDetails is absent or invalid.

    Notes:
        ``metric`` is a numeric summary column, such as ``net_worth``,
        ``player_damage``, ``player_healing``, or ``barrier_absorption``.
        Missing values or players leave gaps instead of producing incomplete totals.
        Times use match-clock minutes. No catalogs or images are required.
    """
    snapshots = _snapshots(demo)
    title = _metric(snapshots, metric)
    colors = _teams(snapshots)
    expected = dict(
        demo.players.group_by("team_num").agg(pl.col("steam_id").n_unique()).iter_rows()
    )
    totals = (
        snapshots.group_by("team_num", "snapshot_time_s")
        .agg(
            pl.col(metric).cast(pl.Float64).sum().alias("total"),
            pl.col(metric).null_count().alias("missing"),
            pl.col("steam_id").n_unique().alias("players"),
        )
        .sort("snapshot_time_s")
    )
    fig, ax = _axes(ax, (_TIMELINE["width"], _TIMELINE["height"]))
    times = sorted(snapshots["snapshot_time_s"].unique().to_list())
    for team, color in colors.items():
        rows = {
            row["snapshot_time_s"]: row
            for row in totals.filter(pl.col("team_num").eq_missing(team)).iter_rows(
                named=True
            )
        }
        if not rows:
            continue
        values = [
            rows[time]["total"]
            if time in rows
            and rows[time]["missing"] == 0
            and rows[time]["players"] == expected.get(team, rows[time]["players"])
            else math.nan
            for time in times
        ]
        ax.plot(
            [time / 60 for time in times],
            values,
            color=color,
            label=_team_label(team),
            marker=_MARKER["sample"],
            markersize=_SIZE["timeline_marker"],
            linewidth=_SIZE["line_width"],
        )
    ax.set_title(
        f"{title} by Team",
        loc=_TIMELINE["title_align"],
        color=_FOREGROUND,
        weight=_FONT["heading_weight"],
        pad=_TIMELINE["title_pad"],
        fontsize=_FONT["title"],
        fontfamily=_FONT["family"],
    )
    ax.set_xlabel(
        _NUMBER["time_axis"],
        color=_MUTED,
        fontsize=_FONT["body"],
        fontfamily=_FONT["family"],
    )
    ax.yaxis.set_major_formatter(FuncFormatter(lambda value, _: _number(value)))
    ax.set_ylabel(
        title, color=_MUTED, fontsize=_FONT["body"], fontfamily=_FONT["family"]
    )
    ax.grid(alpha=_OPACITY["grid"], color=_MUTED)
    ax.legend(
        facecolor=_BACKGROUND,
        edgecolor="none",
        labelcolor=_FOREGROUND,
        prop={"family": _FONT["family"], "size": _FONT["body"]},
    )
    fig.tight_layout()
    return fig, ax


def _radar(
    assets: _Assets,
    layer: str,
    bounds: MapBounds | None,
    ax: Axes | None,
    map_name: str | None,
) -> tuple[Figure, Axes, MapBounds]:
    """Draw a minimap in verified Source world coordinates.

    Args:
        assets: Verified local image paths and index.
        layer: MapLayer member or exact string: surface, tunnels, or rat_tunnels. Selects an image, not a floor.
        bounds: Verified MapBounds. None reads bounds from the image catalog.
        ax: Existing matplotlib Axes, or None to create a figure.
        map_name: Optional expected image-map name, such as dl_midtown.

    Returns:
        Matplotlib Figure, Axes, and resolved MapBounds.
    """
    try:
        layer = MapLayer(layer).value
    except ValueError as error:
        raise ValueError("layer must be surface, tunnels, or rat_tunnels") from error
    info = assets.index["map"]
    if map_name is not None and info.get("name") not in {None, map_name}:
        raise ValueError(
            f"image map {info['name']!r} does not match replay map {map_name!r}"
        )
    if bounds is None:
        if info.get("bounds") is None:
            raise ValueError(
                "this image bundle has no map bounds; pass bounds=MapBounds(x_min, y_min, x_max, y_max)"
            )
        bounds = MapBounds(*info["bounds"])
    images = [
        image
        for image in info["images"].values()
        if image is not None and Path(image["resource"]).stem == _LAYERS[layer]
    ]
    if len(images) != 1:
        raise ValueError(f"the image bundle has no unique {layer} minimap")
    fig, ax = _axes(ax, (_MAP["width"], _MAP["height"]))
    ax.imshow(assets.image(images[0]), extent=bounds.extent, origin="upper", zorder=0)
    ax.set_xlim(bounds.x_min, bounds.x_max)
    ax.set_ylim(bounds.y_min, bounds.y_max)
    ax.set_aspect("equal")
    ax.set_axis_off()
    return fig, ax, bounds


def radar(
    *,
    version: str | None = None,
    layer: str = MapLayer.SURFACE,
    bounds: MapBounds | None = None,
    ax: Axes | None = None,
) -> tuple[Figure, Axes]:
    """Draw a minimap in world coordinates using the bundle's recorded bounds.

    Args:
        version: Image catalog version. None selects the newest verified local version, or downloads the latest when none is installed.
        layer: MapLayer member or exact string: surface, tunnels, or rat_tunnels. Selects an image, not a floor.
        bounds: Verified MapBounds. None reads bounds from the image catalog.
        ax: Existing matplotlib Axes, or None to create a figure.

    Returns:
        Tuple of matplotlib Figure and Axes. Use fig.savefig(...) to save it.
        Close the figure with matplotlib.pyplot.close(fig) after use.

    Raises:
        ValueError: The layer, image, or bounds are invalid or missing.
        data.DataError: The image catalog cannot be installed or verified.

    Notes:
        World coordinates use Source units with positive Y upward.
        Missing image bounds require an explicit verified MapBounds.
    """
    fig, ax, _ = _radar(_assets(version, ["map"]), layer, bounds, ax, None)
    return fig, ax


def _map_rows(rows: pl.DataFrame, z_range: tuple[float, float] | None) -> pl.DataFrame:
    """Require world XY and apply an optional altitude filter.

    Args:
        rows: Polars state rows for the selected frame tick.
        z_range: Optional altitude interval (low, high), selecting low <= z < high for all map rows.

    Returns:
        Rows with team_num and the selected altitude interval.

    Raises:
        ValueError: XY is absent or the altitude interval/Z field is invalid.
    """
    if not {"x", "y"} <= set(rows.columns):
        raise ValueError("positions require x and y columns")
    if "team_num" not in rows.columns:
        rows = rows.with_columns(pl.lit(None, dtype=pl.Int32).alias("team_num"))
    if z_range is not None:
        low, high = z_range
        if (
            not math.isfinite(low)
            or not math.isfinite(high)
            or low >= high
            or "z" not in rows.columns
        ):
            raise ValueError("z_range requires a z column and finite increasing bounds")
        rows = rows.filter((pl.col("z") >= low) & (pl.col("z") < high))
    return rows


def _in_bounds(x: float | None, y: float | None, bounds: MapBounds) -> bool:
    """Check a finite XY point against inclusive image bounds.

    Args:
        x: Source world X or None.
        y: Source world Y or None.
        bounds: Verified MapBounds. None reads bounds from the image catalog.

    Returns:
        True only for a finite point within all four bounds.
    """
    return (
        x is not None
        and y is not None
        and math.isfinite(x)
        and math.isfinite(y)
        and bounds.x_min <= x <= bounds.x_max
        and bounds.y_min <= y <= bounds.y_max
    )


def _lane_style(lane: int | None) -> tuple[str, str]:
    """Resolve a primary lane ID to its private badge style.

    Args:
        lane: Recorded primary lane ID or None.

    Returns:
        Lane label and color. Unknown lane IDs use gray.
    """
    style = _LANES.get(str(lane))
    if style is not None:
        return style["name"], style["color"]
    return "Unknown Lane" if lane is None else f"Lane {lane}", _MUTED


def _ziplines(ax: Axes, rows: pl.DataFrame, bounds: MapBounds) -> list[Annotation]:
    """Draw team-controlled routes and lane-name badges.

    Args:
        ax: Existing matplotlib Axes, or None to create a figure.
        rows: Polars state rows for the selected frame tick.
        bounds: Verified MapBounds. None reads bounds from the image catalog.

    Returns:
        Badge annotations for shared caption placement.

    Notes:
        Connections use full handles. A midpoint tick requires different known
        owners; it is not an exact recorded capture position. Missing targets and
        zero-length connections are omitted.
    """
    nodes = {
        row["node_handle"]: row
        for row in rows.iter_rows(named=True)
        if row["node_handle"] is not None and _in_bounds(row["x"], row["y"], bounds)
    }
    segments, owners, seen, connected = [], [], set(), set()
    teams = _teams(rows)
    for handle, row in nodes.items():
        for target in row["connections"] or []:
            if target not in nodes:
                continue
            edge = tuple(sorted((handle, target)))
            if edge in seen:
                continue
            seen.add(edge)
            other = nodes[target]
            dx, dy = other["x"] - row["x"], other["y"] - row["y"]
            length = math.hypot(dx, dy)
            if not length:
                continue
            connected.update(edge)
            middle = ((row["x"] + other["x"]) / 2, (row["y"] + other["y"]) / 2)
            # Ownership is recorded at nodes. The midpoint is a display boundary,
            # not an inferred capture position between different owners.
            for node in (row, other):
                segments.append([(node["x"], node["y"]), middle])
                owners.append(teams[node["team_num"]])
            if (
                row["team_num"] is not None
                and other["team_num"] is not None
                and row["team_num"] != other["team_num"]
            ):
                ax.plot(
                    [middle[0]],
                    [middle[1]],
                    linestyle="none",
                    marker=[(-dy / length, dx / length), (dy / length, -dx / length)],
                    markersize=_SIZE["zipline_boundary_length"],
                    markeredgecolor=_FOREGROUND,
                    markeredgewidth=_SIZE["zipline_boundary_width"],
                    zorder=0.9,
                )
    if not segments:
        return []
    for colors, width, zorder in (
        (_BACKGROUND, _SIZE["zipline_outline_width"], 0.5),
        (owners, _SIZE["zipline_width"], 0.75),
    ):
        ax.add_collection(
            LineCollection(
                segments,
                colors=colors,
                linewidths=width,
                capstyle="round",
                zorder=zorder,
            ),
            autolim=False,
        )
    by_lane: dict[int | None, list[dict]] = {}
    for handle in connected:
        node = nodes[handle]
        by_lane.setdefault(node.get("lane"), []).append(node)
    center_y = (bounds.y_min + bounds.y_max) / 2
    badges = []
    for lane, group in by_lane.items():
        anchor = min(
            group, key=lambda node: (abs(node["y"] - center_y), node["node_handle"])
        )
        name, color = _lane_style(lane)
        badges.append(
            ax.annotate(
                name,
                (anchor["x"], anchor["y"]),
                xytext=(_MAP["caption_offset"], 0),
                textcoords="offset points",
                va="center",
                color=_BACKGROUND,
                fontsize=_FONT["heading"],
                fontfamily=_FONT["family"],
                weight=_FONT["heading_weight"],
                bbox={
                    "boxstyle": f"round,pad={_MAP['zipline_badge_pad']}",
                    "facecolor": color,
                    "edgecolor": _BACKGROUND,
                    "linewidth": _SIZE["border_width"],
                },
                annotation_clip=True,
                zorder=2,
            )
        )
    return badges


def _troopers(ax: Axes, rows: pl.DataFrame, bounds: MapBounds) -> None:
    """Draw in-bounds trooper positions by recorded team.

    Args:
        ax: Existing matplotlib Axes, or None to create a figure.
        rows: Polars state rows for the selected frame tick.
        bounds: Verified MapBounds. None reads bounds from the image catalog.

    Returns:
        None.
    """
    colors = _teams(rows)
    for team, color in colors.items():
        points = [
            (row["x"], row["y"])
            for row in rows.filter(pl.col("team_num").eq_missing(team)).iter_rows(
                named=True
            )
            if _in_bounds(row["x"], row["y"], bounds)
        ]
        if points:
            x, y = zip(*points, strict=True)
            ax.scatter(
                x,
                y,
                s=_SIZE["trooper_marker"],
                c=color,
                marker=_MARKER["trooper"],
                alpha=_OPACITY["trooper"],
                zorder=1,
            )


def _objectives(ax: Axes, rows: pl.DataFrame, bounds: MapBounds) -> list[Annotation]:
    """Draw recorded objective health and phase labels.

    Args:
        ax: Existing matplotlib Axes, or None to create a figure.
        rows: Polars state rows for the selected frame tick.
        bounds: Verified MapBounds. None reads bounds from the image catalog.

    Returns:
        Objective annotations for shared caption placement.
    """
    captions = []
    colors = _teams(rows)
    phases = patron_phase_names()
    for row in rows.iter_rows(named=True):
        x, y = row["x"], row["y"]
        if not _in_bounds(x, y, bounds):
            continue
        health, maximum = row.get("health"), row.get("max_health")
        destroyed = health is not None and health <= 0
        name = row["objective_type"].replace("_", " ").title()
        if destroyed:
            state = "Destroyed"
        elif health is not None and maximum is not None and maximum > 0:
            state = f"{health / maximum:.0%}"
        else:
            state = _NUMBER["unknown"]
        if row["objective_type"] == "patron" and row.get("phase") in phases:
            state += " · " + phases[row["phase"]].replace("_", " ").title()
        color = colors[row["team_num"]]
        ax.scatter(
            [x],
            [y],
            s=_SIZE["objective_marker"],
            c=color,
            marker=_MARKER["destroyed"] if destroyed else _MARKER["objective"],
            alpha=_OPACITY["inactive"] if destroyed else _OPACITY["objective"],
            zorder=2,
        )
        captions.append(
            ax.annotate(
                f"{name} · {state}",
                (x, y),
                xytext=(_MAP["caption_offset"], 0),
                textcoords="offset points",
                color=color,
                fontsize=_FONT["small"],
                fontfamily=_FONT["family"],
                va="center",
                annotation_clip=True,
                zorder=2,
            )
        )
    return captions


def _place_captions(fig: Figure, ax: Axes, captions: Sequence[Annotation]) -> None:
    """Place map captions clear of nearby labels and player icons.

    Args:
        fig: Matplotlib Figure used to measure and draw artists.
        ax: Existing matplotlib Axes, or None to create a figure.
        captions: Objective and lane-name annotations to move in place.

    Returns:
        None. Caption positions change in place.
    """
    if not captions:
        return
    fig.canvas.draw()
    scale = fig.dpi / 72
    width = _MAP["caption_avoid_width"] * scale
    height = _MAP["caption_avoid_height"] * scale
    occupied = [
        Bbox.from_bounds(x - width / 2, y - height / 2, width, height)
        for marker in ax.collections
        if marker.get_zorder() >= 3
        for x, y in ax.transData.transform(marker.get_offsets())
    ]
    for caption in captions:
        candidates = []
        overlaps = 1
        for dy in _MAP["caption_search_y"]:
            for dx in _MAP["caption_search_x"]:
                caption.set_position((dx, dy))
                caption.set_horizontalalignment("left" if dx > 0 else "right")
                box = caption.get_window_extent().padded(_MAP["caption_gap"])
                if not (
                    ax.bbox.contains(box.x0, box.y0)
                    and ax.bbox.contains(box.x1, box.y1)
                ):
                    continue
                overlaps = sum(box.overlaps(other) for other in occupied)
                candidates.append((overlaps, dx, dy, box))
                if not overlaps:
                    break
            if not overlaps:
                break
        if not candidates:
            continue
        _, dx, dy, box = min(candidates, key=lambda candidate: candidate[0])
        caption.set_position((dx, dy))
        caption.set_horizontalalignment("left" if dx > 0 else "right")
        occupied.append(box)
        if dy or abs(dx) > _MAP["caption_offset"]:
            ax.annotate(
                "",
                caption.xy,
                xytext=(dx, dy),
                textcoords="offset points",
                arrowprops={
                    "arrowstyle": "-",
                    "color": _MUTED,
                    "lw": _SIZE["leader_width"],
                    "alpha": _OPACITY["leader"],
                },
                zorder=1,
            )


def frame(
    players: pl.DataFrame,
    *,
    version: str | None = None,
    layer: str = MapLayer.SURFACE,
    bounds: MapBounds | None = None,
    roster: pl.DataFrame | None = None,
    map_name: str | None = None,
    labels: bool = False,
    portraits: bool = True,
    facing: bool = False,
    objectives: pl.DataFrame | None = None,
    troopers: pl.DataFrame | None = None,
    ziplines: pl.DataFrame | None = None,
    z_range: tuple[float, float] | None = None,
    ax: Axes | None = None,
) -> tuple[Figure, Axes]:
    """Draw snapshot rows with icons and recorded health bars.

    Args:
        players: Polars player rows with x/y. Optional hero_id, team_num, player_name, yaw, health, max_health, and is_alive add detail.
        version: Image catalog version. None selects the newest verified local version, or downloads the latest when none is installed.
        layer: MapLayer member or exact string: surface, tunnels, or rat_tunnels. Selects an image, not a floor.
        bounds: Verified MapBounds. None reads bounds from the image catalog.
        roster: One player-info row per Steam ID; adds missing player_name and team_num fields.
        map_name: Optional expected image-map name, such as dl_midtown.
        labels: Draw player names when True.
        portraits: Draw hero images when True. False uses text or plain markers.
        facing: Draw recorded player viewing directions when True.
        objectives: Optional Polars objective state rows, one per entity/site at the frame tick.
        troopers: Optional exact-tick Polars trooper rows.
        ziplines: Optional exact-tick zipline rows with node_handle, connections, x/y, and optional lane/team_num.
        z_range: Optional altitude interval (low, high), selecting low <= z < high for all map rows.
        ax: Existing matplotlib Axes, or None to create a figure.

    Returns:
        Tuple of matplotlib Figure and Axes. Use fig.savefig(...) to save it.
        Close the figure with matplotlib.pyplot.close(fig) after use.

    Raises:
        ValueError: Rows contain different ticks, required fields are absent, or map/layer/bounds/altitude values are invalid.
        data.DataError: Required catalogs or images cannot be installed or verified.

    Notes:
        Only one tick can be drawn. Positions use Source world coordinates.
        A roster adds names and teams by Steam ID. A layer selects an image; it does
        not select a floor. Missing, invalid, and out-of-bounds positions are omitted.
        Zipline connections use full handles. Lane badges do not change ownership.
    """
    tick_values: set[int] = set()
    for rows in (players, objectives, troopers, ziplines):
        if rows is not None and "tick" in rows.columns:
            tick_values.update(rows["tick"].drop_nulls().to_list())
    if len(tick_values) > 1:
        raise ValueError("frame requires positions from one tick")
    if roster is not None:
        players = _with_roster(players, roster)
    players = _map_rows(players, z_range)
    objectives = _map_rows(objectives, z_range) if objectives is not None else None
    troopers = _map_rows(troopers, z_range) if troopers is not None else None
    ziplines = _map_rows(ziplines, z_range) if ziplines is not None else None
    assets = _assets(version, ["map", "heroes"] if portraits else ["map"])
    fig, ax, bounds = _radar(assets, layer, bounds, ax, map_name)
    colors = _teams(players)
    captions = []
    if ziplines is not None:
        colors.update(_teams(ziplines))
        captions.extend(_ziplines(ax, ziplines, bounds))
    if troopers is not None:
        _troopers(ax, troopers, bounds)
    if objectives is not None:
        captions.extend(_objectives(ax, objectives, bounds))
    for row in players.iter_rows(named=True):
        x, y = row["x"], row["y"]
        if not _in_bounds(x, y, bounds):
            continue
        color = colors[row["team_num"]]
        health = row.get("health")
        alive = row.get("is_alive")
        if alive is None:
            alive = health is None or health > 0
        alpha = 1.0 if alive else _OPACITY["inactive"]
        ax.scatter(
            [x],
            [y],
            s=_SIZE["player_marker"],
            c=[color],
            marker=_MARKER["player"] if alive else _MARKER["dead"],
            edgecolors=_BACKGROUND if alive else None,
            linewidths=_SIZE["player_outline_width"],
            alpha=alpha,
            zorder=3,
        )
        hero = assets.hero(row.get("hero_id"))
        if portraits and alive:
            _portrait(
                ax,
                assets,
                hero.get("images", {}).get("minimap"),
                (x, y),
                _SIZE["hero_minimap"],
                annotation_clip=True,
                zorder=4,
            )
        yaw = row.get("yaw")
        if facing and yaw is not None and math.isfinite(yaw) and alive:
            radians = math.radians(yaw)
            ax.annotate(
                "",
                (x, y),
                xytext=(
                    _MAP["facing_length"] * math.cos(radians),
                    _MAP["facing_length"] * math.sin(radians),
                ),
                textcoords="offset points",
                arrowprops={
                    "arrowstyle": "-",
                    "color": color,
                    "lw": _SIZE["line_width"],
                },
                zorder=5,
            )
        maximum = row.get("max_health")
        if health is not None and maximum is not None and maximum > 0 and alive:
            bar_width, bar_height = (
                _SIZE["health_bar_width"],
                _SIZE["health_bar_height"],
            )
            bar = DrawingArea(bar_width, bar_height)
            bar.add_artist(
                Rectangle(
                    (0, 0),
                    bar_width,
                    bar_height,
                    facecolor=_PALETTE["health_track"],
                    edgecolor="none",
                )
            )
            bar.add_artist(
                Rectangle(
                    (0, 0),
                    bar_width * min(1, max(0, health / maximum)),
                    bar_height,
                    facecolor=_PALETTE["health"],
                    edgecolor="none",
                )
            )
            ax.add_artist(
                AnnotationBbox(
                    bar,
                    (x, y),
                    xybox=(0, _MAP["health_offset"]),
                    boxcoords="offset points",
                    frameon=False,
                    pad=0,
                    annotation_clip=True,
                    zorder=5,
                )
            )
        if labels:
            name = (
                row.get("player_name")
                or hero.get("display_name")
                or str(row.get("hero_id", ""))
            )
            ax.annotate(
                name,
                (x, y),
                xytext=(0, _MAP["label_offset"]),
                textcoords="offset points",
                ha="center",
                color=_FOREGROUND,
                fontsize=_FONT["caption"],
                fontfamily=_FONT["family"],
                alpha=alpha,
                annotation_clip=True,
                zorder=6,
            )
    handles = [
        Line2D(
            [],
            [],
            marker=_MARKER["player"],
            linestyle="none",
            color=color,
            label="Neutral"
            if team == 0 and ziplines is not None
            else _team_label(team),
        )
        for team, color in colors.items()
        if team is not None
    ]
    if objectives is not None and not objectives.is_empty():
        handles.extend(
            [
                Line2D(
                    [],
                    [],
                    marker=_MARKER["objective"],
                    linestyle="none",
                    color=_MUTED,
                    label="Objective",
                ),
                Line2D(
                    [],
                    [],
                    marker=_MARKER["destroyed"],
                    linestyle="none",
                    color=_MUTED,
                    label="Destroyed",
                ),
            ]
        )
    if troopers is not None and not troopers.is_empty():
        handles.append(
            Line2D(
                [],
                [],
                marker=_MARKER["trooper"],
                linestyle="none",
                color=_MUTED,
                label="Trooper",
            )
        )
    if ziplines is not None and not ziplines.is_empty():
        handles.append(
            Line2D(
                [],
                [],
                linestyle="none",
                marker="|",
                color=_FOREGROUND,
                markersize=_SIZE["zipline_boundary_length"],
                markeredgewidth=_SIZE["zipline_boundary_width"],
                label="Control Boundary",
            )
        )
    if handles:
        ax.legend(
            handles=handles,
            loc="lower left",
            facecolor=_BACKGROUND,
            edgecolor="none",
            labelcolor=_FOREGROUND,
            prop={"family": _FONT["family"], "size": _FONT["body"]},
        )
    fig.tight_layout()
    _place_captions(fig, ax, captions)
    return fig, ax


def positions(
    demo: Demo,
    *,
    tick: int,
    version: str | None = None,
    layer: str = MapLayer.SURFACE,
    bounds: MapBounds | None = None,
    map_name: str | None = None,
    labels: bool = False,
    portraits: bool = True,
    facing: bool = False,
    objectives: bool = False,
    troopers: bool = False,
    ziplines: bool = False,
    z_range: tuple[float, float] | None = None,
    ax: Axes | None = None,
) -> tuple[Figure, Axes]:
    """Read one exact tick using the selected image bundle's map.

    Args:
        demo: Boon Demo to read. Reuse the object across related plots.
        tick: Exact nonnegative demo tick.
        version: Image catalog version. None selects the newest verified local version, or downloads the latest when none is installed.
        layer: MapLayer member or exact string: surface, tunnels, or rat_tunnels. Selects an image, not a floor.
        bounds: Verified MapBounds. None reads bounds from the image catalog.
        map_name: Optional expected image-map name, such as dl_midtown.
        labels: Draw player names when True.
        portraits: Draw hero images when True. False uses text or plain markers.
        facing: Draw recorded player viewing directions when True.
        objectives: Add last recorded objective health and phase through tick when True.
        troopers: Add exact-tick trooper dots when True.
        ziplines: Add team-colored routes, lane-name badges, and displayed control boundaries when True.
        z_range: Optional altitude interval (low, high), selecting low <= z < high for all map rows.
        ax: Existing matplotlib Axes, or None to create a figure.

    Returns:
        Tuple of matplotlib Figure and Axes. Use fig.savefig(...) to save it.
        Close the figure with matplotlib.pyplot.close(fig) after use.

    Raises:
        ValueError: The tick, map, layer, bounds, or altitude filter is invalid.
        InvalidDemoError: Requested replay state cannot be decoded.
        data.DataError: Required catalogs or images cannot be installed or verified.

    Notes:
        Replay header names can describe the loading map. They do not select
        the background. Use map_name to check the selected image map.
        Objectives use the last observed state through tick. A missing removal
        record does not prove continued presence. Other overlays use exact-tick
        rows. The first zipline query caches its history. Zipline midpoint
        boundaries are for display, not recorded capture positions.
    """
    datasets = ["player_ticks"]
    if troopers:
        datasets.append("troopers")
    if ziplines:
        datasets.append("ziplines")
    result = (
        demo.snapshots(datasets, ticks=tick)
        if len(datasets) > 1
        else demo.snapshots(ticks=tick)
    )
    troop_rows = result.get("troopers") if isinstance(result, dict) else None
    zipline_rows = result.get("ziplines") if isinstance(result, dict) else None
    players = result["player_ticks"] if isinstance(result, dict) else result
    if not isinstance(players, pl.DataFrame):
        raise TypeError("player snapshots must be a DataFrame")
    objective_rows = None
    if objectives:
        objective_rows = (
            demo.objectives.filter(pl.col("tick") <= tick)
            .sort("tick", maintain_order=True)
            .group_by("entity_id", maintain_order=True)
            .last()
            # Respawned objectives can use a different slot at the same map site.
            .sort("tick", maintain_order=True)
            .unique(
                subset=["objective_type", "x", "y"], keep="last", maintain_order=True
            )
            .with_columns(
                pl.col("tick").alias("last_update_tick"), pl.lit(tick).alias("tick")
            )
        )
    fig, ax = frame(
        players,
        roster=demo.players,
        map_name=map_name,
        version=version,
        layer=layer,
        bounds=bounds,
        labels=labels,
        portraits=portraits,
        facing=facing,
        objectives=objective_rows,
        troopers=troop_rows,
        ziplines=zipline_rows,
        z_range=z_range,
        ax=ax,
    )
    ax.set_title(
        f"Match {demo.match_id} · {demo.tick_to_match_clock(tick)} · Tick {tick}",
        color=_FOREGROUND,
        loc=_MAP["title_align"],
        pad=_MAP["title_pad"],
        fontsize=_FONT["title"],
        fontfamily=_FONT["family"],
        weight=_FONT["heading_weight"],
    )
    return fig, ax
