"""Nominal bullet speed: base Source units/s * (1 + percent / 100) * 0.0254."""

from ._rule import Rule

v1 = Rule("bullet_velocity", "bullet_velocity.v1", 1, "2026-09-27")
