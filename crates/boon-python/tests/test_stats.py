"""Tests for the boon.stats analysis layer."""

import polars as pl
from boon import Demo, stats

KP_COLUMNS = [
    "hero_id",
    "team_num",
    "kills",
    "assists",
    "team_kills",
    "kill_participation",
]


class TestKillParticipation:
    def test_columns_and_shape(self, demo: Demo) -> None:
        kp = stats.kill_participation(demo)
        assert kp.columns == KP_COLUMNS
        # One row per player on the roster.
        assert kp.height == demo.players.height

    def test_method_matches_function(self, demo: Demo) -> None:
        # Demo.kill_participation is a thin delegator to stats.kill_participation.
        assert demo.kill_participation().equals(stats.kill_participation(demo))

    def test_value_is_kills_plus_assists_over_team_kills(self, demo: Demo) -> None:
        kp = stats.kill_participation(demo)
        recomputed = pl.when(pl.col("team_kills") > 0).then(
            (pl.col("kills") + pl.col("assists")) / pl.col("team_kills")
        ).otherwise(None)
        check = kp.with_columns(recomputed.alias("expected"))
        assert check.select(
            (pl.col("kill_participation") == pl.col("expected"))
            | (pl.col("kill_participation").is_null() & pl.col("expected").is_null())
        ).to_series().all()

    def test_participation_in_unit_interval(self, demo: Demo) -> None:
        kp = stats.kill_participation(demo).drop_nulls("kill_participation")
        assert kp.select(
            (pl.col("kill_participation") >= 0.0) & (pl.col("kill_participation") <= 1.0)
        ).to_series().all()

    def test_team_kills_equals_sum_of_member_kills(self, demo: Demo) -> None:
        # Each team kill has exactly one killer on that team, so a team's
        # total kills equals the sum of its members' individual kills.
        kp = stats.kill_participation(demo)
        agg = kp.group_by("team_num").agg(
            pl.col("kills").sum().alias("sum_kills"),
            pl.col("team_kills").first().alias("team_kills"),
        )
        assert agg.select(pl.col("sum_kills") == pl.col("team_kills")).to_series().all()

    def test_window_full_range_matches_whole_match(self, demo: Demo) -> None:
        full = demo.kill_participation()
        windowed = demo.kill_participation(end_tick=10**12)
        assert windowed.equals(full)

    def test_empty_window_has_no_kills(self, demo: Demo) -> None:
        empty = demo.kill_participation(start_tick=10**12)
        assert empty.height == demo.players.height
        assert empty.select(
            pl.col("kills", "assists", "team_kills").sum()
        ).row(0) == (0, 0, 0)
        assert empty["kill_participation"].is_null().all()


TIME_DEAD_COLUMNS = [
    "hero_id",
    "team_num",
    "ticks_dead",
    "seconds_dead",
    "pct_regulation_dead",
]


class TestTimeDead:
    def test_columns_and_shape(self, demo: Demo) -> None:
        td = stats.time_dead(demo)
        assert td.columns == TIME_DEAD_COLUMNS
        # One row per player on the roster.
        assert td.height == demo.players.height

    def test_method_matches_function(self, demo: Demo) -> None:
        assert demo.time_dead().equals(stats.time_dead(demo))

    def test_seconds_and_pct_derive_from_ticks(self, demo: Demo) -> None:
        td = stats.time_dead(demo)
        reg = demo.regulation_ticks
        rate = demo.tick_rate
        check = td.with_columns(
            (pl.col("ticks_dead") / rate).alias("exp_seconds"),
            (pl.col("ticks_dead") / reg * 100).alias("exp_pct"),
        )
        assert check.select(
            (pl.col("seconds_dead") == pl.col("exp_seconds"))
            & (pl.col("pct_regulation_dead") == pl.col("exp_pct"))
        ).to_series().all()

    def test_within_regulation_bounds(self, demo: Demo) -> None:
        td = stats.time_dead(demo)
        reg = demo.regulation_ticks
        # Dead ticks never exceed regulation; percentage stays in [0, 100].
        assert td.select(
            (pl.col("ticks_dead") >= 0) & (pl.col("ticks_dead") <= reg)
        ).to_series().all()
        assert td.select(
            (pl.col("pct_regulation_dead") >= 0.0)
            & (pl.col("pct_regulation_dead") <= 100.0)
        ).to_series().all()
