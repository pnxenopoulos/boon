"""Urn events verified against current replay records."""

import polars as pl
from boon import Demo
from conftest import _require_demo_fixture


def test_urn_tracks_carrier_and_completed_deliveries(demo: Demo) -> None:
    """Compare Urn events with carrier, channel, and trigger records in 108575009."""
    events = demo.urn
    assert events.filter(pl.col("event") == "picked_up")["tick"].to_list() == [
        42869,
        63675,
        80613,
        100510,
        101329,
        120442,
        137871,
        160816,
    ]
    assert events.filter(pl.col("event") == "returned")["tick"].to_list() == [
        46108,
        65839,
        82380,
        103965,
        121939,
        139839,
    ]
    assert events.filter(pl.col("event") == "dropped")["tick"].to_list() == [
        101280,
        166576,
    ]
    # Non-carriers enter the return area here; neither completes a delivery.
    assert events.filter(pl.col("tick").is_in([42948, 182712])).is_empty()
    # Loading Urn alone must use the same clock, source classes, and lifetimes.
    standalone = Demo(str(_require_demo_fixture()), preload=False)
    assert standalone.urn.equals(events)
