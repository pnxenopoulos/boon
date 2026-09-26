"""Default combat preloading must agree with explicit lazy loading."""

from pathlib import Path

import pytest
from boon import Demo
from polars.testing import assert_frame_equal


def test_default_preload_matches_grouped_lazy_load(demo_paths: list[Path]) -> None:
    if not demo_paths:
        pytest.skip("No demo fixtures available")
    for path in demo_paths:
        eager = Demo(str(path))
        lazy = Demo(str(path), preload=False)
        assert eager.match_id == lazy.match_id
        lazy.load("kills", "damage", "abilities")
        for dataset in ("kills", "damage", "abilities"):
            assert_frame_equal(getattr(eager, dataset), getattr(lazy, dataset))
        eager.load("kills", "damage", "abilities")
        assert_frame_equal(eager.damage, lazy.damage)
