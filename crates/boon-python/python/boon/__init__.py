"""Python bindings for the Boon Deadlock demo parser."""

from importlib.metadata import version

__version__ = version("boon-deadlock")

from boon._boon import (
    Demo,
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
from boon import stats

# Surface stats as convenience methods on Demo. The implementation lives in
# ``boon.stats``; these are thin delegators so ``demo.kill_participation()`` and
# ``boon.stats.kill_participation(demo)`` are the same computation.
Demo.in_combat = stats.in_combat
Demo.kill_participation = stats.kill_participation
Demo.time_dead = stats.time_dead

__all__ = [
    "Demo",
    "DemoHeaderError",
    "DemoInfoError",
    "DemoMessageError",
    "InvalidDemoError",
    "NotStreetBrawlError",
    "ability_names",
    "game_mode_names",
    "hero_names",
    "hitgroup_names",
    "lifestate_names",
    "modifier_names",
    "patron_phase_names",
    "stats",
    "team_names",
]
