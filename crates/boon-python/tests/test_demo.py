"""Tests for boon.Demo against real demo fixtures."""

import re
import tempfile
from pathlib import Path

import polars as pl
import pytest
from boon import (
    Demo,
    InvalidDemoError,
    ability_names,
    game_mode_names,
    hero_names,
    hitgroup_names,
    lifestate_names,
    modifier_names,
    patron_phase_names,
    team_names,
)

from conftest import FIXTURES_DIR, _require_demo_fixture

# ---------------------------------------------------------------------------
# Expected columns per dataset
# ---------------------------------------------------------------------------

PLAYER_TICKS_COLUMNS = {
    "tick", "hero_id", "x", "y", "z", "pitch", "yaw", "roll",
    "in_regen_zone", "in_item_shop",
    "death_time", "last_spawn_time", "respawn_time",
    "health", "max_health", "lifestate", "souls", "spent_souls",
    "in_combat_end_time", "in_combat_last_damage_time", "in_combat_start_time",
    "player_damage_dealt_end_time", "player_damage_dealt_last_damage_time",
    "player_damage_dealt_start_time", "player_damage_taken_end_time",
    "player_damage_taken_last_damage_time", "player_damage_taken_start_time",
    "time_revealed_by_npc", "build_id", "is_alive", "has_rebirth",
    "has_rejuvenator", "has_ultimate_trained", "health_regen",
    "ultimate_cooldown_start", "ultimate_cooldown_end",
    "ap_net_worth", "gold_net_worth", "denies", "hero_damage",
    "hero_healing", "objective_damage", "self_healing", "kill_streak",
    "last_hits", "level", "kills", "deaths", "assists",
}

WORLD_TICKS_COLUMNS = {"tick", "is_paused", "next_midboss"}

KILLS_COLUMNS = {"tick", "victim_hero_id", "attacker_hero_id", "assister_hero_ids"}

DAMAGE_COLUMNS = {
    "tick", "damage", "pre_damage", "victim_hero_id", "attacker_hero_id",
    "victim_health_new", "hitgroup_id", "crit_damage",
    "attacker_class", "victim_class",
}

FLEX_SLOTS_COLUMNS = {"tick", "team_num"}

ABILITIES_COLUMNS = {"tick", "hero_id", "ability"}

ABILITY_UPGRADES_COLUMNS = {"tick", "hero_id", "ability_id", "tier"}

ITEM_PURCHASES_COLUMNS = {"tick", "hero_id", "ability_id", "change"}

CHAT_COLUMNS = {"tick", "hero_id", "text", "chat_type"}

OBJECTIVES_COLUMNS = {
    "tick", "objective_type", "team_num", "lane", "health", "max_health", "phase",
    "x", "y", "z", "entity_id",
}

MID_BOSS_COLUMNS = {"tick", "team_num", "event"}

TROOPERS_COLUMNS = {
    "tick", "trooper_type", "team_num", "lane", "health", "max_health",
    "x", "y", "z", "entity_id",
}

NEUTRALS_COLUMNS = {
    "tick", "team_num", "health", "max_health",
    "x", "y", "z", "entity_id",
}

BREAKABLES_COLUMNS = {
    "tick", "entity_id", "team_num", "health", "max_health", "lifestate",
    "x", "y", "z",
}

SINNERS_SACRIFICE_COLUMNS = {
    "tick", "entity_id", "team_num", "health", "max_health", "lifestate",
    "x", "y", "z",
}

STAT_MODIFIER_EVENTS_COLUMNS = {"tick", "hero_id", "stat_type", "amount"}

ACTIVE_MODIFIERS_COLUMNS = {
    "tick", "hero_id", "event", "modifier_id", "ability_id",
    "duration", "caster_hero_id", "stacks",
}

URN_COLUMNS = {
    "tick", "event", "hero_id", "team_num", "x", "y", "z",
}

RIFT_COLUMNS = {
    "rift_num", "announce_tick", "active_tick", "capture_tick", "expire_tick",
    "winning_team", "lane", "x", "y", "z",
}

ABILITY_TICKS_COLUMNS = {
    "tick", "hero_id", "ability_id", "slot", "cooldown_start", "cooldown_end",
    "remaining_charges", "charge_recharge_start", "charge_recharge_end",
}

PLAYERS_COLUMNS = {
    "player_name", "steam_id", "hero_id", "team_num", "start_lane", "rank",
}

BANNED_HEROES_COLUMNS = {"hero_id", "hero_name"}

# Bans are recorded per match, so the expected values are per fixture. Demos
# absent from this map are only checked against the schema-level invariants.
# `84133142` and `70537442` are the demos observed to carry the one-shot
# `BannedHeroes` message; `70555151` shares a server version with `70537442`
# and carries no bans, which is what makes the empty case a real match state
# rather than an unsupported build.
EXPECTED_BANS = {
    "84133142.dem": [69, 63],
    "70537442.dem": [2, 69],
    "70555151.dem": [],
    "94366136.dem": [],
}

# Maps dataset name -> expected column set for parameterized tests.
DATASET_COLUMNS = {
    "player_ticks": PLAYER_TICKS_COLUMNS,
    "world_ticks": WORLD_TICKS_COLUMNS,
    "kills": KILLS_COLUMNS,
    "damage": DAMAGE_COLUMNS,
    "flex_slots": FLEX_SLOTS_COLUMNS,
    "abilities": ABILITIES_COLUMNS,
    "ability_upgrades": ABILITY_UPGRADES_COLUMNS,
    "item_purchases": ITEM_PURCHASES_COLUMNS,
    "chat": CHAT_COLUMNS,
    "objectives": OBJECTIVES_COLUMNS,
    "mid_boss": MID_BOSS_COLUMNS,
    "troopers": TROOPERS_COLUMNS,
    "neutrals": NEUTRALS_COLUMNS,
    "breakables": BREAKABLES_COLUMNS,
    "sinners_sacrifice": SINNERS_SACRIFICE_COLUMNS,
    "stat_modifier_events": STAT_MODIFIER_EVENTS_COLUMNS,
    "active_modifiers": ACTIVE_MODIFIERS_COLUMNS,
    "ability_ticks": ABILITY_TICKS_COLUMNS,
    "urn": URN_COLUMNS,
    "rift": RIFT_COLUMNS,
}

ALL_DATASETS = list(DATASET_COLUMNS.keys())


# ===================================================================
# Metadata
# ===================================================================


class TestMetadata:
    """Tests for scalar metadata properties."""

    def test_total_ticks_positive(self, demo: Demo) -> None:
        assert demo.total_ticks > 0

    def test_map_name_nonempty(self, demo: Demo) -> None:
        assert isinstance(demo.map_name, str)
        assert len(demo.map_name) > 0

    def test_match_id_positive_or_none(self, demo: Demo) -> None:
        assert demo.match_id is None or demo.match_id > 0

    def test_tick_rate_positive(self, demo: Demo) -> None:
        assert demo.tick_rate > 0

    def test_total_seconds_positive(self, demo: Demo) -> None:
        assert demo.total_seconds > 0

    def test_total_clock_time_format(self, demo: Demo) -> None:
        assert re.match(r"\d+:\d{2}", demo.total_clock_time)

    def test_build_positive(self, demo: Demo) -> None:
        assert demo.build > 0

    def test_game_mode_positive(self, demo: Demo) -> None:
        assert demo.game_mode > 0

    def test_path_is_pathlib(self, demo: Demo) -> None:
        assert isinstance(demo.path, Path)

    def test_verify(self, demo: Demo) -> None:
        assert demo.verify() is True


# ===================================================================
# Game result
# ===================================================================


class TestGameResult:
    """Tests for game result properties (winning team, game over tick)."""

    def test_winning_team_num_is_int_or_none(self, demo: Demo) -> None:
        result = demo.winning_team_num
        assert result is None or isinstance(result, int)

    def test_game_over_tick_is_int_or_none(self, demo: Demo) -> None:
        result = demo.game_over_tick
        assert result is None or isinstance(result, int)

    def test_game_over_tick_within_range(self, demo: Demo) -> None:
        tick = demo.game_over_tick
        if tick is not None:
            assert 0 < tick <= demo.total_ticks



# ===================================================================
# Players and teams
# ===================================================================


class TestPlayersAndTeams:
    """Tests for player and team DataFrames."""

    def test_players_shape(self, demo: Demo) -> None:
        players = demo.players
        assert players.shape[0] > 0
        assert players.shape[1] == len(PLAYERS_COLUMNS)

    def test_players_columns(self, demo: Demo) -> None:
        assert set(demo.players.columns) == PLAYERS_COLUMNS

    def test_players_hero_ids_unique(self, demo: Demo) -> None:
        hero_ids = demo.players["hero_id"].to_list()
        assert len(hero_ids) == len(set(hero_ids))

    def test_players_steam_ids_nonzero(self, demo: Demo) -> None:
        steam_ids = demo.players["steam_id"].to_list()
        assert all(sid > 0 for sid in steam_ids)

    def test_players_team_nums_valid(self, demo: Demo) -> None:
        team_nums = demo.players["team_num"].to_list()
        for t in team_nums:
            assert t in (1, 2, 3)

    def test_player_ranks_nonnegative(self, demo: Demo) -> None:
        assert all(rank >= 0 for rank in demo.players["rank"])

    def test_player_ticks_covers_all_players(self, demo: Demo) -> None:
        """Every hero in `players` must appear in `player_ticks`.

        `players` reads the hero ID straight off each player controller, while
        `player_ticks` reaches it through the controller's pawn handle. A bad
        handle mask drops players from `player_ticks` only, so the set of unique
        hero IDs must match between the two datasets.
        """
        player_heroes = set(demo.players["hero_id"].to_list())
        tick_heroes = set(demo.player_ticks["hero_id"].to_list())
        assert tick_heroes == player_heroes


# ===================================================================
# Health invariants
# ===================================================================


class TestHealthInvariants:
    """Tests for player health sanity across all ticks."""

    # A player's current health can momentarily read above max_health — e.g. a
    # transient overheal effect, or health and max_health being networked on
    # different ticks so a snapshot catches them mid-update. It is real but
    # rare, so we cap the share of offending (player, tick) rows rather than
    # forbidding it outright. Observed across fixtures: ~0.01%-0.13%, so 1%
    # leaves comfortable headroom while still catching a regression.
    MAX_OVERHEALTH_RATE = 0.01

    def test_health_rarely_exceeds_max(self, demo: Demo) -> None:
        ticks = demo.player_ticks

        # Only rows with a known, positive max_health carry a meaningful bound;
        # max_health == 0 is an un-networked / dead-state artifact, not a cap a
        # player could exceed.
        valid = ticks.filter(pl.col("max_health") > 0)
        assert len(valid) > 0, "no player ticks with a positive max_health"

        over = valid.filter(pl.col("health") > pl.col("max_health"))
        rate = len(over) / len(valid)

        assert rate <= self.MAX_OVERHEALTH_RATE, (
            f"{rate:.2%} of player ticks have health > max_health "
            f"({len(over)}/{len(valid)}), exceeding the "
            f"{self.MAX_OVERHEALTH_RATE:.0%} tolerance"
        )


# ===================================================================
# Name lookups
# ===================================================================


class TestNameLookups:
    """Tests for module-level name lookup functions."""

    def test_hero_names_is_dict(self) -> None:
        names = hero_names()
        assert isinstance(names, dict)
        assert len(names) > 0

    def test_hero_names_contains_infernus(self) -> None:
        names = hero_names()
        assert names[1] == "Infernus"

    def test_team_names_is_dict(self) -> None:
        names = team_names()
        assert isinstance(names, dict)
        assert names == {1: "Spectator", 2: "Hidden King", 3: "Archmother"}

    def test_ability_names_is_dict(self) -> None:
        names = ability_names()
        assert isinstance(names, dict)
        assert len(names) > 0

    def test_ability_names_contains_known(self) -> None:
        names = ability_names()
        assert 46922526 in names
        assert names[46922526] == "inherent_base"

    def test_modifier_names_is_dict(self) -> None:
        names = modifier_names()
        assert isinstance(names, dict)
        assert len(names) > 0

    def test_modifier_names_contains_known(self) -> None:
        names = modifier_names()
        assert 2059539911 in names
        assert names[2059539911] == "timer"

    def test_game_mode_names_is_dict(self) -> None:
        names = game_mode_names()
        assert isinstance(names, dict)
        assert len(names) > 0

    def test_game_mode_names_contains_known(self) -> None:
        names = game_mode_names()
        assert names[1] == "6v6"
        assert names[4] == "street_brawl"

    def test_patron_phase_names_is_dict(self) -> None:
        names = patron_phase_names()
        assert isinstance(names, dict)
        assert names == {0: "normal", 1: "final", 2: "transforming"}

    def test_hitgroup_names_is_dict(self) -> None:
        names = hitgroup_names()
        assert isinstance(names, dict)
        assert len(names) > 0

    def test_hitgroup_names_contains_known(self) -> None:
        names = hitgroup_names()
        assert names[0] == "generic"
        assert names[1] == "head"
        assert names[-1] == "invalid"
        assert names[19] == "head_no_resist"
        assert 20 not in names  # HITGROUP_COUNT sentinel is omitted

    def test_lifestate_names_is_dict(self) -> None:
        names = lifestate_names()
        assert isinstance(names, dict)
        assert names == {
            0: "alive",
            1: "dying",
            2: "dead",
            3: "respawnable",
            4: "respawning",
        }


# ===================================================================
# Datasets (parameterized)
# ===================================================================


class TestDatasets:
    """Parameterized tests for all dataset properties."""

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_loads_as_dataframe(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        assert isinstance(df, pl.DataFrame)

    # Datasets that may be empty depending on game mode
    # "rift" is empty on demos from builds predating the Rift objective.
    POSSIBLY_EMPTY = {"ability_upgrades", "flex_slots", "mid_boss", "neutrals", "stat_modifier_events", "urn", "rift"}

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_nonempty(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        if dataset in self.POSSIBLY_EMPTY:
            assert len(df) >= 0
        else:
            assert len(df) > 0

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_columns(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        assert set(df.columns) == DATASET_COLUMNS[dataset]

    @pytest.mark.parametrize("dataset", ALL_DATASETS)
    def test_tick_column_nonnegative(self, demo: Demo, dataset: str) -> None:
        df = getattr(demo, dataset)
        if "tick" in df.columns and len(df) > 0:
            assert df["tick"].min() >= 0  # type: ignore[operator]


class TestAbilityTicks:
    """Semantics of the change-only ability_ticks frame."""

    def test_charges_nonnegative(self, demo: Demo) -> None:
        at = demo.ability_ticks
        assert at["remaining_charges"].min() >= 0

    def test_hero_ids_on_roster(self, demo: Demo) -> None:
        roster = set(demo.players["hero_id"].to_list())
        assert set(demo.ability_ticks["hero_id"].unique().to_list()) <= roster

    def test_ability_ids_resolve(self, demo: Demo) -> None:
        # At least some emitted ability_ids resolve to known ability names.
        import boon

        names = boon.ability_names()
        seen = set(demo.ability_ticks["ability_id"].unique().to_list())
        assert len(seen & set(names)) > 0


class TestActiveModifiers:
    """Semantics of the active_modifiers event stream (applied/changed/removed)."""

    INSTANCE_KEYS = ["hero_id", "modifier_id", "ability_id", "caster_hero_id"]

    def test_event_values_in_domain(self, demo: Demo) -> None:
        events = set(demo.active_modifiers["event"].unique().to_list())
        assert events <= {"applied", "changed", "removed"}

    def test_stacks_nonnegative(self, demo: Demo) -> None:
        am = demo.active_modifiers
        if len(am) > 0:
            assert am["stacks"].min() >= 0

    def test_changed_events_reflect_varying_stacks(self, demo: Demo) -> None:
        # A "changed" row is emitted only when a live modifier's stack count
        # moves (regression: stacks used to be frozen at first sighting, so a
        # debuff climbing 2 -> 4 stayed reported as 2). Any modifier group with
        # a "changed" event must therefore show more than one distinct stack
        # value, and must have an "applied" event. Grouping by these keys can
        # merge concurrent instances (there is no serial column), so this uses
        # set/containment properties rather than row adjacency.
        am = demo.active_modifiers
        if "changed" not in am["event"].to_list():
            pytest.skip("no stack-change events in this demo")
        for _, grp in am.group_by(self.INSTANCE_KEYS):
            events = grp["event"].to_list()
            if "changed" in events:
                assert "applied" in events, "changed event without an applied"
                assert grp["stacks"].n_unique() > 1


class TestRift:
    """Semantics of the one-row-per-Rift lifecycle frame."""

    def test_rift_num_is_sequential(self, demo: Demo) -> None:
        rift = demo.rift
        if len(rift) == 0:
            pytest.skip("no rifts in this demo")
        assert rift["rift_num"].to_list() == list(range(1, len(rift) + 1))

    def test_exactly_one_outcome_per_rift(self, demo: Demo) -> None:
        # A Rift either gets captured or expires uncaptured, never both and
        # never neither.
        rift = demo.rift
        if len(rift) == 0:
            pytest.skip("no rifts in this demo")
        for row in rift.iter_rows(named=True):
            captured = row["capture_tick"] is not None
            expired = row["expire_tick"] is not None
            assert captured != expired, f"rift {row['rift_num']} has both/neither outcome"

    def test_winning_team_set_iff_captured(self, demo: Demo) -> None:
        rift = demo.rift
        if len(rift) == 0:
            pytest.skip("no rifts in this demo")
        for row in rift.iter_rows(named=True):
            if row["capture_tick"] is not None:
                assert row["winning_team"] in (2, 3)
            else:
                assert row["winning_team"] is None

    def test_tick_ordering(self, demo: Demo) -> None:
        # announce (when present) precedes active, which precedes the outcome.
        rift = demo.rift
        if len(rift) == 0:
            pytest.skip("no rifts in this demo")
        for row in rift.iter_rows(named=True):
            active = row["active_tick"]
            assert active >= 0
            if row["announce_tick"] is not None:
                assert row["announce_tick"] <= active
            outcome = row["capture_tick"] if row["capture_tick"] is not None else row["expire_tick"]
            assert outcome >= active

    def test_rifts_do_not_overlap(self, demo: Demo) -> None:
        rift = demo.rift
        if len(rift) < 2:
            pytest.skip("fewer than two rifts in this demo")
        rows = rift.iter_rows(named=True)
        prev_end = -1
        for row in rows:
            assert row["active_tick"] > prev_end
            outcome = row["capture_tick"] if row["capture_tick"] is not None else row["expire_tick"]
            prev_end = outcome

    def test_lane_in_domain(self, demo: Demo) -> None:
        # 0 means "location is not a known Rift site"; otherwise it must be a
        # real lane id.
        rift = demo.rift
        if len(rift) == 0:
            pytest.skip("no rifts in this demo")
        assert set(rift["lane"].unique().to_list()) <= {0, 1, 3, 4, 6}

    def test_position_is_finite_and_on_map(self, demo: Demo) -> None:
        # The game clears the cash-in location to FLT_MAX once a Rift resolves;
        # a row must carry the real position, not that sentinel.
        rift = demo.rift
        if len(rift) == 0:
            pytest.skip("no rifts in this demo")
        for axis in ("x", "y", "z"):
            assert rift[axis].abs().max() < 1.0e6, f"{axis} looks like a sentinel"

    def test_lane_resolves_when_position_known(self, demo: Demo) -> None:
        # Every Rift site seen so far maps to a lane; a 0 here means a new site
        # has appeared and RIFT_LANE_SITES needs extending.
        rift = demo.rift
        if len(rift) == 0:
            pytest.skip("no rifts in this demo")
        unmapped = rift.filter(pl.col("lane") == 0)
        assert len(unmapped) == 0, f"unmapped rift site(s): {unmapped.select(['x', 'y']).to_dicts()}"


# ===================================================================
# Breakables
# ===================================================================


class TestBreakables:
    """The breakables dataset: prop state changes, including the break row."""

    def test_schema(self, demo: Demo) -> None:
        assert set(demo.breakables.columns) == BREAKABLES_COLUMNS

    def test_has_break_rows(self, demo: Demo) -> None:
        # Props get broken over a match, so there are health==0 (break) rows,
        # and max_health is always positive.
        df = demo.breakables
        assert df.height > 0
        assert (df["max_health"] > 0).all()
        assert df.filter(pl.col("health") == 0).height > 0

    def test_positions_finite(self, demo: Demo) -> None:
        df = demo.breakables
        assert df["x"].is_finite().all()
        assert df["y"].is_finite().all()


class TestSinnersSacrifice:
    """The sinners_sacrifice dataset: machine spawn + health-change rows."""

    def test_schema(self, demo: Demo) -> None:
        assert set(demo.sinners_sacrifice.columns) == SINNERS_SACRIFICE_COLUMNS

    def test_max_health_positive(self, demo: Demo) -> None:
        df = demo.sinners_sacrifice
        if df.height:
            assert (df["max_health"] > 0).all()


# ===================================================================
# Banned heroes
# ===================================================================


class TestBannedHeroes:
    """Semantics of the banned-hero frame."""

    def test_columns(self, demo: Demo) -> None:
        assert set(demo.banned_heroes.columns) == BANNED_HEROES_COLUMNS

    def test_dtypes(self, demo: Demo) -> None:
        # hero_id must stay Int64 so it joins to `players.hero_id` without a
        # cast, including on the empty (no-bans) frame.
        banned = demo.banned_heroes
        assert banned.schema["hero_id"] == pl.Int64
        assert banned.schema["hero_name"] == pl.String

    def test_matches_expected_bans(self, demo: Demo) -> None:
        name = Path(demo.path).name
        if name not in EXPECTED_BANS:
            pytest.skip(f"no recorded ban expectation for {name}")
        assert demo.banned_heroes["hero_id"].to_list() == EXPECTED_BANS[name]

    def test_hero_names_match_lookup(self, demo: Demo) -> None:
        names = hero_names()
        for row in demo.banned_heroes.iter_rows(named=True):
            assert row["hero_name"] == names.get(row["hero_id"], "HERO_NOT_FOUND")

    def test_hero_ids_are_known(self, demo: Demo) -> None:
        # A HERO_NOT_FOUND here means a ban referenced a hero the bundled table
        # doesn't have, i.e. heroes.rs needs regenerating.
        unknown = demo.banned_heroes.filter(pl.col("hero_name") == "HERO_NOT_FOUND")
        assert len(unknown) == 0, f"unknown banned hero id(s): {unknown['hero_id'].to_list()}"

    def test_no_duplicate_bans(self, demo: Demo) -> None:
        ids = demo.banned_heroes["hero_id"].to_list()
        assert len(ids) == len(set(ids))

    def test_banned_heroes_were_not_played(self, demo: Demo) -> None:
        # The point of a ban is that the hero is unavailable, so no banned hero
        # may appear on the roster. This is the check that would catch the
        # message being misread as something other than a ban list.
        banned = set(demo.banned_heroes["hero_id"].to_list())
        played = set(demo.players["hero_id"].to_list())
        assert banned & played == set(), f"banned hero(es) also played: {banned & played}"

    def test_repeated_access_is_stable(self, demo: Demo) -> None:
        assert demo.banned_heroes.equals(demo.banned_heroes)


class TestBannedHeroesScanPaths:
    """The two code paths that populate bans must agree.

    Bans are collected either by the lightweight events-only scan (a bare
    property access) or opportunistically by the full ``load()`` entity pass.
    The ban message is sent very early — before the match starts — so a pass
    that skipped signon-era packets would silently return no bans, which is
    indistinguishable from a ban-free match. These build fresh ``Demo``
    instances rather than using the session fixture, which has already loaded
    everything.
    """

    @staticmethod
    def _fixture_with_bans() -> Path:
        for name, ids in EXPECTED_BANS.items():
            path = FIXTURES_DIR / name
            if ids and path.is_file():
                return path
        pytest.skip("no fixture with recorded bans available")

    def test_load_pass_agrees_with_events_scan(self) -> None:
        path = self._fixture_with_bans()
        expected = EXPECTED_BANS[path.name]

        # Bare property access -> events-only scan.
        assert Demo(str(path)).banned_heroes["hero_id"].to_list() == expected

        # After a load() that needs events -> the entity pass collects them.
        loaded = Demo(str(path))
        loaded.load("kills")
        assert loaded.banned_heroes["hero_id"].to_list() == expected

    def test_load_that_skips_events_still_resolves(self) -> None:
        # `world_ticks` needs no user messages, so the entity pass collects no
        # bans. The property must fall back to its own scan instead of caching
        # an empty result from that pass.
        path = self._fixture_with_bans()
        demo = Demo(str(path))
        demo.load("world_ticks")
        assert demo.banned_heroes["hero_id"].to_list() == EXPECTED_BANS[path.name]


# ===================================================================
# Tick conversion
# ===================================================================


class TestTickConversion:
    """Tests for tick_to_seconds and tick_to_clock_time."""

    def test_tick_to_seconds_type(self, demo: Demo) -> None:
        assert isinstance(demo.tick_to_seconds(100), float)

    def test_tick_to_seconds_monotonic(self, demo: Demo) -> None:
        t1 = demo.tick_to_seconds(10000)
        t2 = demo.tick_to_seconds(20000)
        assert t2 > t1

    def test_tick_to_seconds_zero(self, demo: Demo) -> None:
        assert demo.tick_to_seconds(0) == 0.0

    def test_tick_to_clock_time_type(self, demo: Demo) -> None:
        assert isinstance(demo.tick_to_clock_time(100), str)

    def test_tick_to_clock_time_format(self, demo: Demo) -> None:
        result = demo.tick_to_clock_time(100)
        assert re.match(r"\d+:\d{2}", result)


# ===================================================================
# Bulk loading
# ===================================================================


class TestBulkLoad:
    """Tests for the load() method."""

    def test_load_multiple_datasets(self, demo: Demo) -> None:
        demo.load("kills", "damage")
        assert isinstance(demo.kills, pl.DataFrame)
        assert isinstance(demo.damage, pl.DataFrame)

    def test_load_invalid_dataset_raises(self) -> None:
        path = _require_demo_fixture()
        d = Demo(str(path))
        with pytest.raises(ValueError):
            d.load("not_a_real_dataset")

    def test_load_idempotent(self, demo: Demo) -> None:
        """Loading the same dataset twice should not error."""
        demo.load("kills")
        demo.load("kills")
        assert isinstance(demo.kills, pl.DataFrame)


# ===================================================================
# Error handling (no demo fixture needed)
# ===================================================================


class TestErrors:
    """Tests for error handling with invalid inputs."""

    def test_file_not_found(self) -> None:
        with pytest.raises(FileNotFoundError):
            Demo("/nonexistent/path/to/demo.dem")

    def test_invalid_demo(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".dem", delete=False) as f:
            f.write(b"\x00" * 128)
            f.flush()
            with pytest.raises(InvalidDemoError):
                Demo(f.name)

    def test_all_error_types_importable(self) -> None:
        from boon import DemoHeaderError, DemoInfoError, DemoMessageError, InvalidDemoError  # noqa: F401

    def test_not_street_brawl_error_importable(self) -> None:
        from boon import NotStreetBrawlError  # noqa: F401
