"""Checks for timing isolation and comparison guardrails, without replay downloads."""

import copy

import boon
import polars as pl
import pytest

import bench_parse
from compare import compare


def test_fingerprints_detect_changed_values_order_and_schema():
    frame = pl.DataFrame({"tick": [1, 2], "health": [80, 60]})
    original, size = bench_parse.describe({"players": frame})
    assert size > 0
    changed, _ = bench_parse.describe({"players": frame.with_columns(pl.col("health") + 1)})
    assert changed != original
    reordered, _ = bench_parse.describe({"players": frame.reverse()})
    assert reordered["players"]["row_hash"] == original["players"]["row_hash"]
    assert reordered != original
    recast, _ = bench_parse.describe({"players": frame.cast(pl.Int32)})
    assert recast != original


@pytest.mark.parametrize("case", ["load/combat", "cached/combat"])
def test_each_sample_has_a_fresh_demo_and_only_cached_case_preloads(monkeypatch, tmp_path, case):
    instances = []

    class Demo:
        def __init__(self, path):
            self.loads = 0
            instances.append(self)

        def load(self, *datasets):
            self.loads += 1

    def execute(demo, name):
        assert demo.loads == int(name.startswith("cached/"))
        demo.load()
        return {"frame": pl.DataFrame({"tick": [1, 2]})}

    path = tmp_path / "demo.dem"
    path.write_bytes(b"test input")
    monkeypatch.setattr(boon, "Demo", Demo)
    monkeypatch.setattr(bench_parse, "execute", execute)
    result = bench_parse.measure(path, case, repeat=3, warmup=1)
    assert len(instances) == 4
    assert len(result["samples"]) == 3
    assert result["output"]["frame"]["rows"] == 2


def report():
    return {
        "harness_sha256": "harness",
        "build_profile": "release",
        "repeat": 3,
        "warmup": 1,
        "runs": [
            {
                "demo": {"name": "match.dem", "sha256": "demo"},
                "case": "load/combat",
                "status": "ok",
                "median_seconds": 1.0,
                "process_peak_rss_bytes": 100,
                "output": {"kills": {"rows": 12, "row_hash": 123}},
                "catalogs": {},
                "environment": {
                    key: "same"
                    for key in (
                        "python",
                        "polars",
                        "platform",
                        "machine",
                        "processor",
                        "host",
                        "cpu_count",
                        "threads",
                    )
                },
            }
        ],
    }


def test_comparison_reports_change_and_rejects_wrong_outputs():
    baseline = report()
    candidate = copy.deepcopy(baseline)
    candidate["runs"][0]["median_seconds"] = 0.75
    assert compare(baseline, candidate)[0]["change_percent"] == -25
    candidate["runs"][0]["output"]["kills"]["row_hash"] += 1
    with pytest.raises(ValueError, match="fingerprint"):
        compare(baseline, candidate, allow_config_change=True)


@pytest.mark.parametrize("field", ["polars", "threads", "cpu_count"])
def test_comparison_guards_environment(field):
    baseline = report()
    candidate = copy.deepcopy(baseline)
    candidate["runs"][0]["environment"][field] = "different"
    with pytest.raises(ValueError, match="environment"):
        compare(baseline, candidate)
    assert compare(baseline, candidate, allow_config_change=True)


def test_comparison_rejects_missing_cases_and_mismatched_harness():
    baseline = report()
    candidate = copy.deepcopy(baseline)
    candidate["runs"] = []
    with pytest.raises(ValueError, match="same demo"):
        compare(baseline, candidate)
    candidate = copy.deepcopy(baseline)
    candidate["harness_sha256"] = "different"
    with pytest.raises(ValueError, match="harness"):
        compare(baseline, candidate)


def test_offline_guard_prevents_implicit_download(monkeypatch, tmp_path):
    from boon import data

    monkeypatch.setattr(data, "BOON_DATA_DIR", tmp_path)
    with (
        pytest.raises(data.DataError, match="boon get"),
        bench_parse.offline_catalogs("names/warm"),
    ):
        pytest.fail("missing catalogs must fail before measurement")
    with (
        bench_parse.offline_catalogs("load/combat"),
        pytest.raises(data.DataError, match="boon get"),
    ):
        data._request("https://example.invalid")


def test_stat_events_allow_only_order_changes_within_a_tick():
    frame = pl.DataFrame({"tick": [1, 1, 2], "amount": [10, 20, 30]})
    original, _ = bench_parse.describe({"stat_modifier_events": frame})
    reordered, _ = bench_parse.describe({"stat_modifier_events": frame[[1, 0, 2]]})
    assert reordered == original
    changed, _ = bench_parse.describe(
        {"stat_modifier_events": frame.with_columns(pl.col("amount") + 1)}
    )
    assert changed != original
    with pytest.raises(RuntimeError, match="chronological"):
        bench_parse.describe({"stat_modifier_events": frame.reverse()})


@pytest.mark.parametrize("case", ["load/combat", "construct/combat"])
def test_constructor_preload_is_explicit_in_measurements(monkeypatch, tmp_path, case):
    class Demo:
        def __init__(self, path, *, preload=True):
            self.preloaded = preload

    def execute(demo, name):
        assert demo.preloaded == (name == "construct/combat")
        return {"frame": pl.DataFrame({"tick": [1, 2]})}

    path = tmp_path / "demo.dem"
    path.write_bytes(b"test input")
    monkeypatch.setattr(boon, "Demo", Demo)
    monkeypatch.setattr(bench_parse, "execute", execute)
    assert bench_parse.measure(path, case, repeat=1, warmup=0)["status"] == "ok"
