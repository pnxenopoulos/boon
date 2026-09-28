"""Identity of a versioned stat equation."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Rule:
    """Equation identity, independent of the client data version."""

    stat: str
    id: str
    version: int
    documented_on: str
