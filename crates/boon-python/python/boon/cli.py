"""Inspect Valve Deadlock Source 2 replays with the Boon Typer CLI.

Install boon-deadlock to use the boon command. Demo inspection uses the same Rust
parser as the Python API. Use --json where supported for structured output.
Use boon versions and boon get to select and install game catalogs and images.
"""

from __future__ import annotations

import json as _json
from pathlib import Path
from typing import Annotated

import polars as pl
import typer

from boon import (
    Demo,
    __version__,
    game_mode_names,
    hero_names,
    team_names,
)
from boon import data as game_data

app = typer.Typer(
    help="Boon — inspect Deadlock demos and download versioned catalogs and images.",
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
    """Open a demo file, turning any parse failure into a clean CLI error.

    Args:
        path: Local Deadlock .dem path.

    Returns:
        Boon Demo with combat preloading disabled.

    Raises:
        typer.Exit: The requested file, dataset, calculation, or data operation fails.
    """
    try:
        return Demo(str(path), preload=False)
    except Exception as exc:  # surface any parse error cleanly
        typer.secho(
            f"error: could not open {path}: {exc}", fg=typer.colors.RED, err=True
        )
        raise typer.Exit(1) from exc


def _print_df(
    df: pl.DataFrame, *, limit: int | None, tail: bool, as_json: bool
) -> None:
    """Print a DataFrame as a full-width table, or as JSON when requested.

    Args:
        df: Polars DataFrame to print.
        limit: Maximum rows to show; 0 means all for commands, and None means all for the helper.
        tail: Show the last rows when True.
        as_json: Print JSON instead of a text table when True.

    Returns:
        None. Requested output is written to the terminal.
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
    """Print the installed Boon version and exit (``--version``).

    Args:
        value: Whether the --version flag was requested.

    Returns:
        None. Requested output is written to the terminal.
    """
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
    """Boon — inspect Deadlock demos and download versioned JSON catalogs.

    Args:
        version: Whether --version was requested. The callback prints and exits.

    Returns:
        None. Requested output is written to the terminal.
    """


@app.command()
def info(file: Path = _FILE_ARG, as_json: bool = _JSON_OPT) -> None:
    """Show match metadata: map, mode, build, duration, and result.

    Args:
        file: Local Deadlock .dem path validated by Typer.
        as_json: Print JSON instead of a text table when True.

    Returns:
        None. Requested output is written to the terminal.
    """
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
    """Show the player roster (name, Steam ID, hero, team, start lane).

    Args:
        file: Local Deadlock .dem path validated by Typer.
        as_json: Print JSON instead of a text table when True.

    Returns:
        None. Requested output is written to the terminal.

    Raises:
        typer.Exit: The requested file, dataset, calculation, or data operation fails.
    """
    demo = _open(file)
    roster = demo.players
    try:
        names = hero_names()
    except game_data.DataError as exc:
        typer.secho(f"error: {exc}", fg=typer.colors.RED, err=True)
        raise typer.Exit(1) from exc
    name_df = pl.DataFrame(
        {"hero_id": list(names.keys()), "hero": list(names.values())}
    ).with_columns(pl.col("hero_id").cast(roster.schema["hero_id"]))
    roster = roster.join(name_df, on="hero_id", how="left")
    _print_df(roster, limit=None, tail=False, as_json=as_json)


@app.command()
def datasets() -> None:
    """List the datasets that ``show`` can display.

    Returns:
        None. Requested output is written to the terminal.
    """
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
    """Load and display one dataset from the demo as a table.

    Args:
        file: Local Deadlock .dem path validated by Typer.
        dataset: Dataset name listed by boon datasets.
        limit: Maximum rows to show; 0 means all for commands, and None means all for the helper.
        tail: Show the last rows when True.
        as_json: Print JSON instead of a text table when True.

    Returns:
        None. Requested output is written to the terminal.

    Raises:
        typer.Exit: The requested file, dataset, calculation, or data operation fails.
    """
    available = Demo.available_datasets()
    if dataset not in available:
        typer.secho(
            f"error: unknown dataset '{dataset}'", fg=typer.colors.RED, err=True
        )
        typer.echo("available: " + ", ".join(available), err=True)
        raise typer.Exit(1)
    demo = _open(file)
    try:
        df = getattr(demo, dataset)
    except Exception as exc:  # surface dataset load failures cleanly
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
        help=(
            "Which part to show: snapshots, last_hits, objectives, damage, "
            "healing, gold_sources, or all."
        ),
    ),
    limit: int = _LIMIT_OPT,
    as_json: bool = _JSON_OPT,
) -> None:
    """Show recorded post-match totals for souls, damage, healing, and objectives.

    Args:
        file: Local Deadlock .dem path validated by Typer.
        part: snapshots, last_hits (default), objectives, damage, healing, gold_sources, or all.
        limit: Maximum rows to show; 0 means all for commands, and None means all for the helper.
        as_json: Print JSON instead of a text table when True.

    Returns:
        None. Requested output is written to the terminal.

    Raises:
        typer.Exit: The requested file, dataset, calculation, or data operation fails.
    """
    valid = [
        "snapshots",
        "last_hits",
        "objectives",
        "damage",
        "healing",
        "gold_sources",
        "all",
    ]
    if part not in valid:
        typer.secho(f"error: unknown part '{part}'", fg=typer.colors.RED, err=True)
        typer.echo("valid: " + ", ".join(valid), err=True)
        raise typer.Exit(1)
    demo = _open(file)
    try:
        result = demo.summary()
    except Exception as exc:  # demos without post-match details
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
    """Compute a derived metric from ``boon.stats``.

    Args:
        file: Local Deadlock .dem path validated by Typer.
        metric: kill-participation (default), time-dead, or in-combat.
        limit: Maximum rows to show; 0 means all for commands, and None means all for the helper.
        as_json: Print JSON instead of a text table when True.

    Returns:
        None. Requested output is written to the terminal.

    Raises:
        typer.Exit: The requested file, dataset, calculation, or data operation fails.
    """
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
    except Exception as exc:  # metrics can require a game-over event
        typer.secho(
            f"error: could not compute '{metric}': {exc}", fg=typer.colors.RED, err=True
        )
        raise typer.Exit(1) from exc
    _print_df(df, limit=None if limit == 0 else limit, tail=False, as_json=as_json)


@app.command()
def verify(file: Path = _FILE_ARG) -> None:
    """Check that a file is a valid Deadlock demo.

    Args:
        file: Local Deadlock .dem path validated by Typer.

    Returns:
        None. Requested output is written to the terminal.

    Raises:
        typer.Exit: The requested file, dataset, calculation, or data operation fails.
    """
    try:
        Demo(str(file), preload=False)
    except Exception as exc:  # report invalid demos as a clean failure
        typer.secho(f"invalid: {file}: {exc}", fg=typer.colors.RED, err=True)
        raise typer.Exit(1) from exc
    typer.secho(f"valid: {file}", fg=typer.colors.GREEN)


@app.command(rich_help_panel="Game data")
def get(
    version: str | None = typer.Argument(
        None, help="Deadlock client version from `boon versions`; defaults to latest."
    ),
    force: bool = typer.Option(
        False,
        "--force",
        help="Replace installed files after verifying a fresh download.",
    ),
    images: Annotated[
        list[game_data.ImageGroup] | None,
        typer.Option(
            "--images",
            help="Image group to extract; repeat for multiple groups, or use all.",
        ),
    ] = None,
    as_json: bool = _JSON_OPT,
) -> None:
    """Download catalogs and selected images to ~/.boon/<client-version>/.

    Args:
        version: Deadlock client version. None selects the latest published version where allowed.
        force: Download and verify fresh files before replacing an installation.
        images: Optional ImageGroup list: all, map, heroes, abilities, items, or icons.
        as_json: Print JSON instead of a text table when True.

    Returns:
        None. Requested output is written to the terminal.

    Raises:
        typer.Exit: The requested file, dataset, calculation, or data operation fails.
    """
    try:
        path = game_data.update(version, force=force, images=images or ())
        files = game_data.available_files(path.name)
    except game_data.DataError as exc:
        typer.secho(f"error: {exc}", fg=typer.colors.RED, err=True)
        raise typer.Exit(1) from exc
    result: dict = {
        "client_version": path.name,
        "path": str(path),
        "files": files,
    }
    if images:
        result["images"] = {
            "path": str(path / "images"),
            "groups": [group.value for group in images],
        }
    if as_json:
        typer.echo(_json.dumps(result, indent=2))
    else:
        typer.echo(f"{path.name}: {len(files)} JSON files in {path}")
        if images:
            typer.echo(f"Images: {path / 'images'}")


@app.command(rich_help_panel="Game data")
def remove(
    version: str = typer.Argument(
        ..., help="Installed Deadlock client version to remove."
    ),
    as_json: bool = _JSON_OPT,
) -> None:
    """Remove one installed boon-data version from the local cache; works offline.

    Args:
        version: Explicit installed client version to remove, including its images.
        as_json: Print JSON instead of a text table when True.

    Returns:
        None. Requested output is written to the terminal.

    Raises:
        typer.Exit: The requested file, dataset, calculation, or data operation fails.
    """
    try:
        path = game_data.remove(version)
    except game_data.DataError as exc:
        typer.secho(f"error: {exc}", fg=typer.colors.RED, err=True)
        raise typer.Exit(1) from exc
    if as_json:
        typer.echo(
            _json.dumps(
                {"client_version": path.name, "path": str(path), "removed": True},
                indent=2,
            )
        )
    else:
        typer.echo(f"Removed {path.name} from {path}")


@app.command(rich_help_panel="Game data")
def versions(
    local: bool = typer.Option(
        False,
        "--local",
        help="List installed versions only; do not access the network.",
    ),
    as_json: bool = _JSON_OPT,
) -> None:
    """Show client versions, source build dates/times, and installed status.

    Args:
        local: List verified installed versions without a network request when True.
        as_json: Print JSON instead of a text table when True.

    Returns:
        None. Requested output is written to the terminal.

    Raises:
        typer.Exit: The requested file, dataset, calculation, or data operation fails.
    """
    try:
        cached = {
            entry["client_version"]: entry for entry in game_data.local_versions()
        }
    except game_data.DataError as exc:
        typer.secho(f"error: {exc}", fg=typer.colors.RED, err=True)
        raise typer.Exit(1) from exc
    entries = dict(cached)
    error = None
    if not local:
        try:
            entries.update(
                {
                    entry["client_version"]: entry
                    for entry in game_data.available_versions()
                }
            )
        except game_data.DataError as exc:
            error = str(exc)
            typer.echo(f"could not list published versions: {exc}", err=True)
    rows = []
    for version in sorted(entries, key=int, reverse=True):
        entry = entries[version]
        installed = (
            version in cached and cached[version]["artifacts"] == entry["artifacts"]
        )
        rows.append(
            {
                "client_version": version,
                "released_at": entry["released_at"],
                "version_date": entry.get("version_date"),
                "version_time": entry.get("version_time"),
                "installed": installed,
                "path": str(game_data.BOON_DATA_DIR / version) if installed else None,
            }
        )
    if as_json:
        typer.echo(_json.dumps({"versions": rows, "error": error}, indent=2))
    elif rows:
        typer.echo(
            f"{'Client version':<16} {'Version date':<13} {'Version time':<12} Installed"
        )
        for row in rows:
            typer.echo(
                f"{row['client_version']:<16} {row['version_date'] or '-':<13} "
                f"{row['version_time'] or '-':<12} "
                f"{'yes' if row['installed'] else 'no'}"
            )
    else:
        typer.echo(
            "no installed versions" if local or error else "no published versions"
        )
    if error and not rows:
        raise typer.Exit(1)


if __name__ == "__main__":  # pragma: no cover
    app()
