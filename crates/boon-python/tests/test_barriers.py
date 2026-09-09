"""Tests for the boon.barriers analysis layer.

These are demo-agnostic invariants that hold on any match. The barrier *values* for a
specific match are validated by the caller that owns that replay; here we only check that
the dataset is well-formed and internally consistent.
"""

from boon import Demo, barriers

COLUMNS = ["tick", "hero_id", "granted", "absorbed", "expired", "hits"]


class TestBarriers:
    def test_columns(self, demo: Demo) -> None:
        assert barriers.barriers(demo).columns == COLUMNS

    def test_method_matches_function(self, demo: Demo) -> None:
        # Demo.barriers is a thin delegator to barriers.barriers.
        assert demo.barriers().equals(barriers.barriers(demo))

    def test_sorted_by_tick(self, demo: Demo) -> None:
        assert barriers.barriers(demo)["tick"].is_sorted()

    def test_granted_is_positive(self, demo: Demo) -> None:
        # A barrier only opens on a real pool rise, so every row grants something.
        assert (barriers.barriers(demo)["granted"] > 0.0).all()

    def test_absorbed_and_expired_conserve_granted(self, demo: Demo) -> None:
        # Barrier HP is either absorbed or expires; a barrier never books more than it gave.
        bar = barriers.barriers(demo)
        assert (bar["absorbed"] >= 0.0).all()
        assert (bar["expired"] >= 0.0).all()
        assert (bar["absorbed"] + bar["expired"] <= bar["granted"] + 0.5).all()

    def test_hero_is_on_the_roster(self, demo: Demo) -> None:
        roster = set(demo.players["hero_id"].to_list())
        assert set(barriers.barriers(demo)["hero_id"].to_list()) <= roster
