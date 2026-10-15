"""Selected-property loads must match the shared full-field reference."""

import pytest
from boon import Demo
from polars.testing import assert_frame_equal

DATASETS = ("kills", "damage", "abilities", "ability_ticks")


@pytest.fixture(scope="module")
def filtered_frames(demo):
    parsed = Demo(str(demo.path), preload=False)
    parsed.load(*DATASETS)
    return {name: getattr(parsed, name) for name in DATASETS}


@pytest.mark.parametrize("dataset", DATASETS)
def test_filtered_load_matches_full_shared_pass(demo, filtered_frames, dataset):
    # The session reference also loads modifiers and the urn, keeping full fields.
    assert_frame_equal(
        getattr(demo, dataset), filtered_frames[dataset], check_exact=True
    )
