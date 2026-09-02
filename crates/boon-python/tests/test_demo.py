"""Tests for boon.Demo against real demo fixtures."""

import re
import tempfile
from pathlib import Path

import polars as pl
import pytest
from boon import (
    Demo,
    DemoMessageError,
    InvalidDemoError,
    ability_display_names,
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
    "health", "max_health", "barrier", "bullet_resist_baseline",
    "spirit_resist_baseline",
    "lifestate", "souls", "spent_souls",
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
    "ability_id", "damage_type", "citadel_type", "damage_flags",
    "is_melee", "melee_type",
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
    "tick", "event", "entity_id", "entity_serial", "subclass_id",
    "subclass_name", "team_num", "x", "y", "z",
}

SINNERS_SACRIFICE_COLUMNS = {
    "tick", "event", "entity_id", "entity_serial", "attacker_hero_id",
    "damage", "health", "max_health", "team_num", "x", "y", "z",
}

STAT_MODIFIER_EVENTS_COLUMNS = {"tick", "hero_id", "stat_type", "amount"}

ACTIVE_MODIFIERS_COLUMNS = {
    "tick", "hero_id", "event", "serial", "modifier_id",
    "ability_id", "duration", "caster_hero_id", "stacks",
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

    def test_ability_display_names_are_exact_localized_names(self) -> None:
        display_names = ability_display_names()
        internal_names = set(ability_names().values())

        assert isinstance(display_names, dict)
        assert len(display_names) > 0
        assert set(display_names) <= internal_names
        assert all(display_names.values())
        assert display_names["upgrade_quick_silver"] == "Quicksilver Reload"
        assert display_names["citadel_ability_hook"] == "Grapple Arm"
        assert (
            display_names["ability_unicorn_luminousstrike"] == "Radiant Daggers"
        )

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
    POSSIBLY_EMPTY = {"ability_upgrades", "breakables", "flex_slots", "mid_boss", "neutrals", "sinners_sacrifice", "stat_modifier_events", "urn", "rift"}

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


class TestDamageMelee:
    """Raw damage metadata and flag-based melee classification."""

    LIGHT_MELEE_FLAG = 1 << 33
    HEAVY_MELEE_FLAG = 1 << 34

    def test_fields_present_and_typed(self, demo: Demo) -> None:
        df = demo.damage
        assert df.schema["ability_id"] == pl.UInt32
        assert df.schema["damage_type"] == pl.Int32
        assert df.schema["citadel_type"] == pl.Int32
        assert df.schema["damage_flags"] == pl.UInt64
        assert df.schema["is_melee"] == pl.Boolean
        assert df.schema["melee_type"] == pl.String

    def test_melee_classification_matches_raw_fields(self, demo: Demo) -> None:
        df = demo.damage
        expected_is_melee = df["citadel_type"] == 3
        assert (df["is_melee"] == expected_is_melee).all()

        expected_types: list[str | None] = []
        for citadel_type, flags in df.select(
            "citadel_type", "damage_flags"
        ).iter_rows():
            if citadel_type != 3:
                expected_types.append(None)
                continue
            is_light = bool(flags & self.LIGHT_MELEE_FLAG)
            is_heavy = bool(flags & self.HEAVY_MELEE_FLAG)
            if is_light and not is_heavy:
                expected_types.append("light")
            elif is_heavy and not is_light:
                expected_types.append("heavy")
            else:
                expected_types.append("other")

        assert df["melee_type"].to_list() == expected_types

    def test_basic_melee_types_are_valid(self, demo: Demo) -> None:
        melee = demo.damage.filter(pl.col("is_melee"))
        if len(melee) == 0:
            pytest.skip("no basic melee damage in this demo")
        assert set(melee["melee_type"].unique()) <= {"light", "heavy", "other"}
        assert melee["melee_type"].is_not_null().all()

    def test_known_fixture_has_light_heavy_and_other(self, demo: Demo) -> None:
        if Path(demo.path).name != "96850353.dem":
            pytest.skip("known melee classification belongs to another fixture")
        counts = set(demo.damage["melee_type"].drop_nulls().unique())
        assert counts == {"light", "heavy", "other"}


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

    def test_event_values_in_domain(self, demo: Demo) -> None:
        events = set(demo.active_modifiers["event"].unique().to_list())
        assert events <= {"applied", "changed", "removed"}

    def test_stacks_nonnegative(self, demo: Demo) -> None:
        am = demo.active_modifiers
        if len(am) > 0:
            assert am["stacks"].min() >= 0

    def test_serial_lifecycle_transitions_are_valid(self, demo: Demo) -> None:
        am = demo.active_modifiers
        if len(am) == 0:
            pytest.skip("no modifier events in this demo")
        assert am["serial"].min() > 0
        for _, grp in am.group_by(["hero_id", "serial"], maintain_order=True):
            active = False
            for event in grp["event"]:
                if event == "applied":
                    assert not active
                    active = True
                elif event == "changed":
                    assert active
                else:
                    assert event == "removed"
                    assert active
                    active = False


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
    """Terminal breakable-prop leave events."""

    def test_events_are_breaks(self, demo: Demo) -> None:
        df = demo.breakables
        if len(df) == 0:
            pytest.skip("no breakable events in this demo")
        assert df["event"].eq("broken").all()

    def test_identity_includes_serial(self, demo: Demo) -> None:
        df = demo.breakables
        assert df.schema["entity_id"] == pl.Int32
        assert df.schema["entity_serial"] == pl.UInt32
        identities = list(
            zip(df["entity_id"].to_list(), df["entity_serial"].to_list(), strict=True)
        )
        assert len(identities) == len(set(identities))

    def test_subclasses_are_resolved(self, demo: Demo) -> None:
        df = demo.breakables
        assert df.schema["subclass_id"] == pl.UInt32
        assert df.schema["subclass_name"] == pl.String
        if len(df) == 0:
            pytest.skip("no breakable events in this demo")
        assert df["subclass_id"].gt(0).all()
        assert df["subclass_name"].ne("BREAKABLE_NOT_FOUND").all()

    def test_positions_are_on_map(self, demo: Demo) -> None:
        df = demo.breakables
        if len(df) == 0:
            pytest.skip("no breakable events in this demo")
        for axis in ("x", "y", "z"):
            assert df[axis].is_finite().all()
            assert df[axis].abs().max() < 1.0e6


# ===================================================================
# Sinner's Sacrifice
# ===================================================================


class TestSinnersSacrifice:
    """Lifecycle and exact machine-hit events."""

    def test_event_and_identity_semantics(self, demo: Demo) -> None:
        df = demo.sinners_sacrifice
        assert df.schema["entity_id"] == pl.Int32
        assert df.schema["entity_serial"] == pl.UInt32
        assert set(df["event"].unique()) <= {"spawned", "hit", "reset"}
        spawned = df.filter(pl.col("event") == "spawned")
        identities = list(
            zip(
                spawned["entity_id"].to_list(),
                spawned["entity_serial"].to_list(),
                strict=True,
            )
        )
        assert len(identities) == len(set(identities))

    def test_health_and_damage_semantics(self, demo: Demo) -> None:
        df = demo.sinners_sacrifice
        if len(df) == 0:
            pytest.skip("no Sinner's Sacrifice machines in this demo")
        assert df["health"].gt(0).all()
        assert df["health"].le(df["max_health"]).all()

        hits = df.filter(pl.col("event") == "hit")
        if len(hits) > 0:
            assert hits["damage"].gt(0).all()

        lifecycle = df.filter(pl.col("event") != "hit")
        assert lifecycle["damage"].eq(0).all()
        assert lifecycle["attacker_hero_id"].eq(0).all()

    def test_positions_are_on_map(self, demo: Demo) -> None:
        df = demo.sinners_sacrifice
        if len(df) == 0:
            pytest.skip("no Sinner's Sacrifice machines in this demo")
        for axis in ("x", "y", "z"):
            assert df[axis].is_finite().all()
            assert df[axis].abs().max() < 1.0e6

    def test_known_hit_has_exact_attacker(self, demo: Demo) -> None:
        if Path(demo.path).name != "96850353.dem":
            pytest.skip("known Sinner's Sacrifice hit belongs to another fixture")
        known = demo.sinners_sacrifice.filter(
            (pl.col("tick") == 49270)
            & (pl.col("event") == "hit")
            & (pl.col("entity_id") == 3342)
            & (pl.col("attacker_hero_id") == 69)
            & (pl.col("damage") == 100)
            & (pl.col("health") == 400)
        )
        assert len(known) == 1


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


def test_summary_repeated_access_is_stable() -> None:
    demo = Demo(str(_require_demo_fixture()))
    try:
        first = demo.summary()
    except DemoMessageError:
        pytest.skip("demo has no post-match summary")

    second = demo.summary()
    assert set(first) == {"snapshots", "last_hits", "objectives", "damage"}
    for name in first:
        assert first[name].equals(second[name]), name
