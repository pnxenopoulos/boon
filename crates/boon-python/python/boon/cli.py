"""Command-line interface for inspecting Deadlock demo files.

This is the Python / `Typer <https://typer.tiangolo.com>`_ counterpart to Boon's
Rust ``boon`` binary. Installing ``boon-deadlock`` (``pip install
boon-deadlock`` or ``uv add boon-deadlock``) exposes a ``boon`` command that
reads a demo through the exact same parser the library uses, so you can inspect
a match straight from the terminal without writing any code::

    boon info match.dem                       # match metadata
    boon players match.dem                    # roster
    boon datasets                             # list inspectable datasets
    boon show match.dem kills --limit 20      # any dataset as a table
    boon summary match.dem                    # post-match summary
    boon stats match.dem -m kill-participation # derived metrics

Every command loads the demo lazily and prints Polars DataFrames. Pass
``--json`` (where available) for machine-readable output.
"""

from __future__ import annotations

import json as _json
from pathlib import Path
from typing import Optional

import polars as pl
import typer

from boon import (
    Demo,
    __version__,
    game_mode_names,
    hero_names,
    team_names,
)

app = typer.Typer(
    help="Boon — inspect Deadlock demo (.dem) files.",
    no_args_is_help=True,
    add_completion=False,
)

# A demo-file positional argument, validated by Typer before the command runs.
_FILE_ARG = typer.Argument(
    ...,
    exists=True,
    dir_okay=False,
    readable=True,
    help="Path to the demo (.dem) file.",
)
_JSON_OPT = typer.Option(False, "--json", help="Emit JSON instead of text.")
_LIMIT_OPT = typer.Option(
    20, "--limit", "-n", min=0, help="Max rows to show (0 = all)."
)


def _open(path: Path) -> Demo:
    """Open a demo file, turning any parse failure into a clean CLI error."""
    try:
        return Demo(str(path))
    except Exception as exc:  # noqa: BLE001 - surface any parse error cleanly
        typer.secho(f"error: could not open {path}: {exc}", fg=typer.colors.RED, err=True)
        raise typer.Exit(1) from exc


def _print_df(
    df: pl.DataFrame, *, limit: Optional[int], tail: bool, as_json: bool
) -> None:
    """Print a DataFrame as a full-width table, or as JSON when requested.

    Args:
        df: The frame to display.
        limit: Max rows to show, or ``None`` for every row.
        tail: Show the last rows instead of the first.
        as_json: Emit row-oriented JSON rather than a table.
    """
    view = df if limit is None else (df.tail(limit) if tail else df.head(limit))
    if as_json:
        typer.echo(view.write_json())
        return
    shown = "" if view.height == df.height else f" (showing {view.height:,})"
    typer.echo(f"{df.height:,} rows × {df.width} cols{shown}")
    with pl.Config(
        tbl_cols=-1, tbl_rows=max(view.height, 1), tbl_hide_dataframe_shape=True
    ):
        typer.echo(str(view))


def _version_callback(value: bool) -> None:
    """Print the installed Boon version and exit (``--version``)."""
    if value:
        typer.echo(f"boon {__version__}")
        raise typer.Exit()


@app.callback()
def _main(
    version: bool = typer.Option(
        False,
        "--version",
        callback=_version_callback,
        is_eager=True,
        help="Show the Boon version and exit.",
    ),
) -> None:
    """Boon — inspect Deadlock demo (.dem) files."""


@app.command()
def info(file: Path = _FILE_ARG, as_json: bool = _JSON_OPT) -> None:
    """Show match metadata: map, mode, build, duration, and result."""
    demo = _open(file)
    modes = game_mode_names()
    teams = team_names()
    winner = demo.winning_team_num
    data = {
        "path": str(demo.path),
        "match_id": demo.match_id,
        "map_name": demo.map_name,
        "game_mode": demo.game_mode,
        "game_mode_name": modes.get(demo.game_mode),
        "build": demo.build,
        "tick_rate": demo.tick_rate,
        "total_ticks": demo.total_ticks,
        "total_seconds": round(demo.total_seconds, 2),
        "total_clock_time": demo.total_clock_time,
        "regulation_clock_time": demo.regulation_clock_time,
        "winning_team_num": winner,
        "winning_team": teams.get(winner) if winner is not None else None,
        "players": demo.players.height,
    }
    if as_json:
        typer.echo(_json.dumps(data, indent=2))
        return
    width = max(len(k) for k in data)
    for key, value in data.items():
        typer.echo(f"{key.rjust(width)} : {'' if value is None else value}")


@app.command()
def players(file: Path = _FILE_ARG, as_json: bool = _JSON_OPT) -> None:
    """Show the player roster (name, Steam ID, hero, team, start lane)."""
    demo = _open(file)
    roster = demo.players
    names = hero_names()
    name_df = pl.DataFrame(
        {"hero_id": list(names.keys()), "hero": list(names.values())}
    ).with_columns(pl.col("hero_id").cast(roster.schema["hero_id"]))
    roster = roster.join(name_df, on="hero_id", how="left")
    _print_df(roster, limit=None, tail=False, as_json=as_json)


@app.command()
def datasets() -> None:
    """List the datasets that ``show`` can display."""
    for name in Demo.available_datasets():
        typer.echo(name)


@app.command()
def show(
    file: Path = _FILE_ARG,
    dataset: str = typer.Argument(..., help="Dataset name (see `boon datasets`)."),
    limit: int = _LIMIT_OPT,
    tail: bool = typer.Option(
        False, "--tail", help="Show the last rows instead of the first."
    ),
    as_json: bool = _JSON_OPT,
) -> None:
    """Load and display one dataset from the demo as a table."""
    available = Demo.available_datasets()
    if dataset not in available:
        typer.secho(f"error: unknown dataset '{dataset}'", fg=typer.colors.RED, err=True)
        typer.echo("available: " + ", ".join(available), err=True)
        raise typer.Exit(1)
    demo = _open(file)
    try:
        df = getattr(demo, dataset)
    except Exception as exc:  # noqa: BLE001 - surface dataset load failures cleanly
        typer.secho(
            f"error: could not load '{dataset}': {exc}", fg=typer.colors.RED, err=True
        )
        raise typer.Exit(1) from exc
    _print_df(df, limit=None if limit == 0 else limit, tail=tail, as_json=as_json)


@app.command()
def summary(
    file: Path = _FILE_ARG,
    part: str = typer.Option(
        "last_hits",
        "--part",
        help="Which part to show: snapshots, last_hits, objectives, damage, or all.",
    ),
    limit: int = _LIMIT_OPT,
    as_json: bool = _JSON_OPT,
) -> None:
    """Show the post-match summary (souls, objectives, damage matrix)."""
    valid = ["snapshots", "last_hits", "objectives", "damage", "all"]
    if part not in valid:
        typer.secho(f"error: unknown part '{part}'", fg=typer.colors.RED, err=True)
        typer.echo("valid: " + ", ".join(valid), err=True)
        raise typer.Exit(1)
    demo = _open(file)
    try:
        result = demo.summary()
    except Exception as exc:  # noqa: BLE001 - demos without post-match details
        typer.secho(
            f"error: no post-match summary available: {exc}",
            fg=typer.colors.RED,
            err=True,
        )
        raise typer.Exit(1) from exc

    parts = valid[:-1] if part == "all" else [part]
    if as_json:
        out = {
            p: (result[p].to_dicts() if result.get(p) is not None else None)
            for p in parts
        }
        typer.echo(_json.dumps(out, indent=2, default=str))
        return
    for p in parts:
        typer.secho(f"# {p}", bold=True)
        frame = result.get(p)
        if frame is None:
            typer.echo("(none)\n")
            continue
        _print_df(frame, limit=None if limit == 0 else limit, tail=False, as_json=False)
        typer.echo("")


@app.command()
def stats(
    file: Path = _FILE_ARG,
    metric: str = typer.Option(
        "kill-participation",
        "--metric",
        "-m",
        help="Metric to compute: kill-participation, time-dead, or in-combat.",
    ),
    limit: int = _LIMIT_OPT,
    as_json: bool = _JSON_OPT,
) -> None:
    """Compute a derived metric from ``boon.stats``."""
    demo = _open(file)
    metrics = {
        "kill-participation": demo.kill_participation,
        "time-dead": demo.time_dead,
        "in-combat": demo.in_combat,
    }
    fn = metrics.get(metric)
    if fn is None:
        typer.secho(f"error: unknown metric '{metric}'", fg=typer.colors.RED, err=True)
        typer.echo("valid: " + ", ".join(metrics), err=True)
        raise typer.Exit(1)
    try:
        df = fn()
    except Exception as exc:  # noqa: BLE001 - metrics can require a game-over event
        typer.secho(
            f"error: could not compute '{metric}': {exc}", fg=typer.colors.RED, err=True
        )
        raise typer.Exit(1) from exc
    _print_df(df, limit=None if limit == 0 else limit, tail=False, as_json=as_json)


@app.command()
def verify(file: Path = _FILE_ARG) -> None:
    """Check that a file is a valid Deadlock demo."""
    try:
        Demo(str(file))
    except Exception as exc:  # noqa: BLE001 - report invalid demos as a clean failure
        typer.secho(f"invalid: {file}: {exc}", fg=typer.colors.RED, err=True)
        raise typer.Exit(1) from exc
    typer.secho(f"valid: {file}", fg=typer.colors.GREEN)


if __name__ == "__main__":  # pragma: no cover
    app()
