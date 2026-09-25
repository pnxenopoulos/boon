"""Python bindings for the Boon Deadlock demo parser."""

from importlib.metadata import version

from boon import data, stats
from boon._boon import (
    Demo,
    game_mode_names,
    hitgroup_names,
    lifestate_names,
    patron_phase_names,
    team_names,
)
from boon.errors import (
    DemoHeaderError,
    DemoInfoError,
    DemoMessageError,
    InvalidDemoError,
    NotStreetBrawlError,
)
from boon.names import ability_display_names, ability_names, hero_names, modifier_names

__version__ = version("boon-deadlock")

# Surface derived datasets as convenience methods on Demo. The implementations live in
# ``boon.stats``; these are thin delegators so ``demo.teamfights()``
# and ``boon.stats.teamfights(demo)`` are the same computation.
Demo.in_combat = stats.in_combat
Demo.kill_participation = stats.kill_participation
Demo.teamfights = stats.teamfights
Demo.time_dead = stats.time_dead

__all__ = [
    "Demo",
    "DemoHeaderError",
    "DemoInfoError",
    "DemoMessageError",
    "InvalidDemoError",
    "NotStreetBrawlError",
    "ability_display_names",
    "ability_names",
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
