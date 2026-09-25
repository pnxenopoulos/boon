"""Types for the native extension; public API declarations live in __init__.pyi."""

from pathlib import Path

from . import (
    Demo as Demo,
)
from . import (
    DemoHeaderError as DemoHeaderError,
)
from . import (
    DemoInfoError as DemoInfoError,
)
from . import (
    DemoMessageError as DemoMessageError,
)
from . import (
    InvalidDemoError as InvalidDemoError,
)
from . import (
    NotStreetBrawlError as NotStreetBrawlError,
)
from . import (
    game_mode_names as game_mode_names,
)
from . import (
    hitgroup_names as hitgroup_names,
)
from . import (
    lifestate_names as lifestate_names,
)
from . import (
    patron_phase_names as patron_phase_names,
)
from . import (
    team_names as team_names,
)

def _read_catalog_names(
    directory: Path,
) -> tuple[dict[int, str], dict[int, str], dict[str, str], dict[int, str]]: ...
