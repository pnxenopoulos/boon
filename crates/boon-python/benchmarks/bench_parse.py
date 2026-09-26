"""Measure Boon's public API in isolated processes with checked outputs.

Run --list for cases. See README.md for builds, timing boundaries and comparisons.
Only the standard library is imported before thread settings are applied.
"""

from __future__ import annotations

import argparse
import gc
import hashlib
import inspect
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from typing import Any
from unittest.mock import patch

GROUPS = {
    "combat": ("kills", "damage", "abilities"),
    "economy": ("chat", "item_purchases", "ability_upgrades"),
    "mixed": ("player_ticks", "world_ticks", "kills", "damage"),
}
DERIVED = ("kill_participation", "in_combat", "time_dead", "teamfights")
NAME_FUNCTIONS = (
    "hero_names",
    "ability_names",
    "ability_display_names",
    "modifier_names",
    "breakable_names",
)
SNAPSHOTS = ("single", "sparse", "many", "window", "sampled", "kills")
DEFAULT_CASES = (
    "construct",
    "load/combat",
    "load/economy",
    "dataset/ability_ticks",
    "snapshot/single",
    "snapshot/sampled",
    "cached/combat",
    "summary",
)
ROOT = Path(__file__).resolve().parents[3]


def cases() -> list[str]:
    from boon import Demo

    return [
        "construct",
        "construct/combat",
        "summary",
        "names/cold",
        "names/warm",
        *(f"dataset/{name}" for name in (*Demo.available_datasets(), "players", "banned_heroes")),
        *(f"{mode}/{name}" for mode in ("load", "sequential", "cached") for name in GROUPS),
        *(f"snapshot/{name}" for name in SNAPSHOTS),
        *(f"derived/{name}" for name in DERIVED),
    ]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def warm_file(path: Path) -> None:
    with path.open("rb") as stream:
        while stream.read(8 * 1024 * 1024):
            pass


def snapshot_args(demo: Any, kind: str) -> dict:
    end = demo.total_ticks
    if kind == "single":
        return {"ticks": end // 2}
    if kind in ("sparse", "many"):
        count = 3 if kind == "sparse" else 8
        return {"ticks": [end * i // (count + 1) for i in range(1, count + 1)]}
    if kind == "window":
        return {"start_tick": end // 2, "end_tick": min(end, end // 2 + 64)}
    if kind == "sampled":
        return {"seconds": 1.0}
    return {"events": "kills"}


def open_demo(path: Path, preload: bool = False) -> Any:
    """Keep fresh loads lazy; also support measuring revisions before preloading."""
    from boon import Demo

    if "preload" in inspect.signature(Demo).parameters:
        return Demo(str(path), preload=preload)
    demo = Demo(str(path))
    if preload:
        demo.load(*GROUPS["combat"])
    return demo


def execute(demo: Any, case: str) -> Any:
    import boon

    mode, _, name = case.partition("/")
    if mode == "construct":
        metadata = {
            key: getattr(demo, key) for key in ("match_id", "build", "total_ticks", "game_mode")
        }
        if name == "combat":
            return {"metadata": metadata, **execute(demo, "cached/combat")}
        return metadata
    if mode == "dataset":
        return {name: getattr(demo, name)}
    if mode in ("load", "cached", "sequential"):
        if mode == "load":
            demo.load(*GROUPS[name])
        return {dataset: getattr(demo, dataset) for dataset in GROUPS[name]}
    if mode == "snapshot":
        return {"player_ticks": demo.snapshots(**snapshot_args(demo, name))}
    if mode == "summary":
        return demo.summary()
    if mode == "derived":
        return {name: getattr(demo, name)()}
    if mode == "names":
        return {function: getattr(boon, function)() for function in NAME_FUNCTIONS}
    raise ValueError(f"unknown benchmark case: {case}")


def describe(value: Any) -> tuple[Any, int]:
    """Fingerprint values outside timing; hashes detect drift, not prove equivalence."""
    import polars as pl

    if isinstance(value, pl.DataFrame):
        row_hashes = value.hash_rows(seed=0)
        signature = {
            "rows": value.height,
            "schema": [(name, str(dtype)) for name, dtype in value.schema.items()],
            "row_hash": row_hashes.sum(),
            # Index the narrow hash column, not the whole frame: aligning a new
            # index with wide segmented output can otherwise copy every column.
            "ordered_row_hash": pl.DataFrame({"hash": row_hashes})
            .with_row_index()
            .hash_rows(seed=0)
            .sum(),
        }
        if "tick" in value.columns:
            signature.update(tick_min=value["tick"].min(), tick_max=value["tick"].max())
        return signature, value.estimated_size()
    if isinstance(value, dict):
        result, size = {}, 0
        for key, item in value.items():
            if key == "stat_modifier_events" and isinstance(item, pl.DataFrame):
                # HashMap iteration makes simultaneous stat events unordered.
                if not item["tick"].is_sorted():
                    raise RuntimeError("stat_modifier_events ticks are not chronological")
                item = item.sort(item.columns)
            signature, item_size = describe(item)
            result[key] = signature
            size += item_size
        return result, size
    return value, 0


def measure(path: Path, case: str, repeat: int, warmup: int) -> dict:
    from boon import names

    warm_file(path)
    samples, expected, sizes = [], None, []
    for iteration in range(-warmup, repeat):
        gc.collect()
        wall, cpu = time.perf_counter_ns(), time.process_time_ns()
        demo = open_demo(path, preload=case == "construct/combat")
        init_wall = (time.perf_counter_ns() - wall) / 1e9
        init_cpu = (time.process_time_ns() - cpu) / 1e9
        if case.startswith("cached/"):
            demo.load(*GROUPS[case.split("/")[1]])
        if case == "names/cold":
            names._read_names.cache_clear()
        elif case == "names/warm":
            execute(demo, case)
        cpu, start = time.process_time_ns(), time.perf_counter_ns()
        result = execute(demo, case)
        finish, cpu_end = time.perf_counter_ns(), time.process_time_ns()
        output, size = describe(result)
        if expected is not None and output != expected:
            raise RuntimeError(
                f"output changed between repetitions of {case}: "
                f"expected {expected!r}, got {output!r}"
            )
        expected = output
        if iteration >= 0:
            samples.append(
                {
                    "init_seconds": init_wall,
                    "init_cpu_seconds": init_cpu,
                    "seconds": init_wall
                    if case.startswith("construct")
                    else (finish - start) / 1e9,
                    "cpu_seconds": init_cpu
                    if case.startswith("construct")
                    else (cpu_end - cpu) / 1e9,
                    "work_start_ns": start,
                    "work_end_ns": finish,
                }
            )
            sizes.append(size)
        del result, demo
    rss = None
    if sys.platform in ("linux", "darwin"):
        import resource

        rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
        rss *= 1024 if sys.platform == "linux" else 1
    seconds = [sample["seconds"] for sample in samples]
    return {
        "case": case,
        "status": "ok",
        "samples": samples,
        "median_seconds": statistics.median(seconds),
        "min_seconds": min(seconds),
        "max_seconds": max(seconds),
        "stdev_seconds": statistics.stdev(seconds) if repeat > 1 else None,
        "process_peak_rss_bytes": rss,
        "output_bytes": max(sizes),
        "output": expected,
    }


@contextmanager
def offline_catalogs(case: str) -> Iterator[dict]:
    """Never allow an implicit download to contaminate a benchmark."""
    from boon import data

    error = data.DataError("benchmark requires local catalogs; run `boon get` before measuring")
    with patch("boon.data._request", side_effect=error):
        catalogs = {}
        if case.startswith("names/") or case in ("dataset/breakables", "dataset/banned_heroes"):
            entries = data.local_versions()
            if not entries:
                raise data.DataError("no verified local catalogs; run `boon get` before measuring")
            catalogs = {entry["client_version"]: entry["artifacts"] for entry in entries}
        yield catalogs


def cpu_model() -> str:
    if sys.platform == "linux":
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            key, _, value = line.partition(":")
            if key.strip() == "model name":
                return value.strip()
    return os.environ.get("PROCESSOR_IDENTIFIER", platform.processor())


def environment() -> dict:
    import boon
    import boon._boon as native
    import polars as pl

    return {
        "python": platform.python_version(),
        "platform": platform.platform(),
        "machine": platform.machine(),
        "processor": cpu_model(),
        "host": platform.node(),
        "cpu_count": os.cpu_count(),
        "boon": boon.__version__,
        "polars": pl.__version__,
        "extension": str(native.__file__),
        "extension_sha256": sha256(Path(native.__file__)),
        "threads": {
            key: os.environ.get(key)
            for key in (
                "BOON_TICK_SEGMENTS",
                "POLARS_MAX_THREADS",
                "RAYON_NUM_THREADS",
            )
        },
    }


def worker(args: argparse.Namespace) -> None:
    case = args.case[0]
    with offline_catalogs(case) as catalogs:
        demo = open_demo(args.demo[0])
        metadata = {
            key: getattr(demo, key) for key in ("match_id", "build", "total_ticks", "game_mode")
        }
        del demo
        if case.startswith("dataset/street_brawl") and metadata["game_mode"] != 4:
            result = {"case": case, "status": "skipped", "reason": "requires Street Brawl"}
        else:
            result = measure(args.demo[0], case, args.repeat, args.warmup)
        result.update(environment=environment(), catalogs=catalogs, demo_metadata=metadata)
        print(json.dumps(result))


def git_state() -> dict:
    def git(*args: str) -> bytes:
        result = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, check=True)
        return result.stdout

    try:
        return {
            "commit": git("rev-parse", "HEAD").decode().strip(),
            "dirty": bool(git("status", "--porcelain")),
            "diff_sha256": hashlib.sha256(git("diff", "HEAD")).hexdigest(),
        }
    except (OSError, subprocess.CalledProcessError):
        return {}


def run(args: argparse.Namespace) -> int:
    selected = cases() if args.all else args.case or list(DEFAULT_CASES)
    unknown = set(selected) - set(cases())
    if unknown:
        raise ValueError(f"unknown cases: {sorted(unknown)}; use --list")
    if len(set(selected)) != len(selected):
        raise ValueError("duplicate benchmark cases")
    demos = [path.resolve(strict=True) for path in args.demo]
    if len(set(demos)) != len(demos):
        raise ValueError("duplicate demo paths")
    report = {
        "git": git_state(),
        "harness_sha256": sha256(Path(__file__)),
        "build_profile": args.build_profile,
        "repeat": args.repeat,
        "warmup": args.warmup,
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "runs": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    failed = False
    for path in demos:
        demo_info = {"name": path.name, "sha256": sha256(path), "bytes": path.stat().st_size}
        for case in selected:
            print(f"{path.name}: {case} ...", file=sys.stderr, flush=True)
            command = [
                sys.executable,
                str(Path(__file__).resolve()),
                "--worker",
                "--demo",
                str(path),
                "--case",
                case,
                "--repeat",
                str(args.repeat),
                "--warmup",
                str(args.warmup),
                "--threads",
                str(args.threads),
                "--segments",
                str(args.segments),
            ]
            result: dict[str, Any]
            try:
                completed = subprocess.run(
                    command, capture_output=True, text=True, timeout=args.timeout
                )
                if completed.returncode:
                    raise RuntimeError(completed.stderr.strip() or completed.stdout.strip())
                result = json.loads(completed.stdout)
                if completed.stderr:
                    result["warnings"] = completed.stderr
            except (subprocess.TimeoutExpired, RuntimeError, json.JSONDecodeError) as error:
                failed = True
                result = {"case": case, "status": "error", "error": str(error)}
            result["demo"] = demo_info
            report["runs"].append(result)
            args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
            if result["status"] == "ok":
                print(
                    f"  {result['median_seconds']:.6f}s median; {result['output_bytes'] / 2**20:.1f} MiB output",
                    file=sys.stderr,
                    flush=True,
                )
            elif result["status"] == "skipped":
                print(f"  SKIP: {result['reason']}", file=sys.stderr, flush=True)
            else:
                print(f"  ERROR: {result['error']}", file=sys.stderr, flush=True)
    print(f"Report: {args.output}")
    return int(failed)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--demo", type=Path, action="append", default=[])
    parser.add_argument("--case", action="append", default=[])
    parser.add_argument(
        "--all", action="store_true", help="every case, including large per-tick frames"
    )
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--repeat", type=int, default=5)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--threads", type=int, default=4, help="Polars and Rayon thread limits")
    parser.add_argument("--segments", type=int, default=4, help="Boon snapshot segments")
    parser.add_argument("--timeout", type=float, default=1800, help="seconds per isolated case")
    parser.add_argument(
        "--build-profile",
        choices=("release", "debug"),
        default="release",
        help="record the installed extension's build profile; does not build it",
    )
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if min(args.repeat, args.threads, args.segments) < 1 or args.warmup < 0 or args.timeout <= 0:
        parser.error("repeat/threads/segments/timeout must be positive; warmup must be nonnegative")
    os.environ.update(
        POLARS_MAX_THREADS=str(args.threads),
        RAYON_NUM_THREADS=str(args.threads),
        BOON_TICK_SEGMENTS=str(args.segments),
    )
    if args.list:
        print("\n".join(cases()))
        return 0
    if not args.demo or (not args.worker and args.output is None):
        parser.error("--demo and --output are required")
    if args.all and args.case:
        parser.error("choose --all or --case")
    try:
        if args.worker:
            worker(args)
            return 0
        return run(args)
    except (OSError, ValueError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    raise SystemExit(main())
