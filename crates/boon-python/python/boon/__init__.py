"""Python bindings for the Boon Deadlock demo parser."""

from importlib.metadata import version

__version__ = version("boon-deadlock")

from boon._boon import (
    Demo,
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
from boon.errors import (
    DemoHeaderError,
    DemoInfoError,
    DemoMessageError,
    InvalidDemoError,
    NotStreetBrawlError,
)
from boon import barriers, stats

# Surface derived datasets as convenience methods on Demo. The implementations live in
# ``boon.stats`` / ``boon.barriers``; these are thin delegators so ``demo.teamfights()``
# and ``boon.stats.teamfights(demo)`` are the same computation.
Demo.in_combat = stats.in_combat
Demo.kill_participation = stats.kill_participation
Demo.teamfights = stats.teamfights
Demo.time_dead = stats.time_dead
Demo.barriers = barriers.barriers

__all__ = [
    "Demo",
    "DemoHeaderError",
    "DemoInfoError",
    "DemoMessageError",
    "InvalidDemoError",
    "NotStreetBrawlError",
    "ability_display_names",
    "ability_names",
    "barriers",
    "game_mode_names",
    "hero_names",
    "hitgroup_names",
    "lifestate_names",
    "modifier_names",
    "patron_phase_names",
    "stats",
    "team_names",
]
