"""Identity of a versioned stat equation."""

from dataclasses import dataclass


@dataclass(frozen=True)
class Rule:
    """Identify one version of a Boon stat equation.

    Attributes:
        stat: Stat string accepted by the matching query.
        id: Stable equation identifier included in result metadata.
        version: Equation version number, independent of the client data version.
        documented_on: ISO date on which this equation was documented.
    """

    stat: str
    id: str
    version: int
    documented_on: str
