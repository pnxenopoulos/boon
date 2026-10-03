"""Python bindings for the Boon Deadlock demo parser."""

from importlib.metadata import version

from boon import (
    ability_stats,
    data,
    hero_stats,
    item_purchases,
    player_states,
    rulesets,
    snapshots,
    stats,
)
from boon._boon import (
    Demo,
    game_mode_names,
    hitgroup_names,
    lifestate_names,
    patron_phase_names,
    team_names,
)
from boon.ability_stats import AbilityStat, AbilityStatResult, ImbueResult
from boon.errors import (
    DemoHeaderError,
    DemoInfoError,
    DemoMessageError,
    InvalidDemoError,
    NotStreetBrawlError,
)
from boon.hero_stats import CalculationError, HeroStat, StatMode, StatResult
from boon.names import (
    ability_display_names,
    ability_names,
    breakable_names,
    hero_names,
    modifier_names,
)

__version__ = version("boon-deadlock")

# Expose Python dataset and analysis helpers on the native Demo type.
Demo.get_item_purchases = item_purchases.get_item_purchases
# The extension class is mutable; ty treats the stub property as read-only.
Demo.item_purchases = property(item_purchases.get_item_purchases)  # ty: ignore[invalid-assignment]
Demo.snapshots = snapshots.snapshots
Demo.player_states = player_states.player_states
Demo.imbues = ability_stats.imbues
Demo.calculate_ability_stats = ability_stats.calculate_ability_stats
Demo.calculate_hero_stats = hero_stats.calculate_hero_stats
Demo.in_combat = stats.in_combat
Demo.kill_participation = stats.kill_participation
Demo.teamfights = stats.teamfights
Demo.time_dead = stats.time_dead

__all__ = [
    "AbilityStat",
    "AbilityStatResult",
    "ImbueResult",
    "ability_stats",
    "CalculationError",
    "HeroStat",
    "StatMode",
    "StatResult",
    "hero_stats",
    "rulesets",
    "Demo",
    "DemoHeaderError",
    "DemoInfoError",
    "DemoMessageError",
    "InvalidDemoError",
    "NotStreetBrawlError",
    "ability_display_names",
    "ability_names",
    "breakable_names",
    "data",
    "game_mode_names",
    "hero_names",
    "hitgroup_names",
    "lifestate_names",
    "modifier_names",
    "patron_phase_names",
    "stats",
    "team_names",
]
