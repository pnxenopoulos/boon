"""Python bindings for the Boon Deadlock demo parser."""

from importlib.metadata import version

__version__ = version("boon-deadlock")

from boon._boon import (
    Demo,
    ability_names,
    game_mode_names,
    hero_names,
    modifier_names,
    team_names,
)
from boon.errors import (
    DemoHeaderError,
    DemoInfoError,
    DemoMessageError,
    InvalidDemoError,
    NotStreetBrawlError,
)

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
    "modifier_names",
    "team_names",
]
