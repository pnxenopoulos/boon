import re
from pathlib import Path

project = "boon"
author = "Peter Xenopoulos"

# Read version from Cargo.toml (single source of truth)
_cargo_toml = Path(__file__).resolve().parent.parent / "Cargo.toml"
version = re.search(r'^version\s*=\s*"(.+?)"', _cargo_toml.read_text(), re.MULTILINE).group(1)

extensions = [
    "myst_parser",
    "sphinx_rtd_dark_mode",
]

# Markdown support
source_suffix = {
    ".rst": "restructuredtext",
    ".md": "markdown",
}

# Theme
html_theme = "sphinx_rtd_theme"
