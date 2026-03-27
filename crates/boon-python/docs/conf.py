import re
from pathlib import Path

project = "Boon"
author = "Peter Xenopoulos"

# Read version from Cargo.toml (single source of truth)
_cargo_toml = Path(__file__).resolve().parent.parent / "Cargo.toml"
version = re.search(r'^version\s*=\s*"(.+?)"', _cargo_toml.read_text(), re.MULTILINE).group(1)

extensions = [
    "myst_parser",
]

# Markdown support
source_suffix = {
    ".rst": "restructuredtext",
    ".md": "markdown",
}

# Theme
# colors taken from https://www.color-hex.com/color-palette/1069181
html_theme = "furo"
html_static_path = ["_static"]
html_favicon = "_static/favicon.ico"
html_theme_options = {
    "light_css_variables": {
        "color-brand-primary": "#2f4442",
        "color-brand-content": "#3f5d4d",
        "color-background-primary": "#efdebf",
        "color-background-secondary": "#e5d2af",
        "color-foreground-primary": "#222021",
        "color-foreground-secondary": "#2f4442",
        "color-sidebar-background": "#2f4442",
        "color-sidebar-brand-text": "#efdebf",
        "color-sidebar-caption-text": "#72947f",
        "color-sidebar-text": "#efdebf",
        "color-sidebar-link-text": "#efdebf",
        "color-sidebar-link-text--top-level": "#efdebf",
        "color-sidebar-item-background--hover": "#3f5d4d",
    },
    "dark_css_variables": {
        "color-brand-primary": "#72947f",
        "color-brand-content": "#72947f",
        "color-background-primary": "#222021",
        "color-background-secondary": "#2a2526",
        "color-foreground-primary": "#efdebf",
        "color-foreground-secondary": "#72947f",
        "color-sidebar-background": "#2f4442",
        "color-sidebar-brand-text": "#efdebf",
        "color-sidebar-caption-text": "#72947f",
        "color-sidebar-text": "#efdebf",
        "color-sidebar-link-text": "#efdebf",
        "color-sidebar-link-text--top-level": "#efdebf",
        "color-sidebar-item-background--hover": "#3f5d4d",
    },
}