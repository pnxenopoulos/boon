"""Python bindings for the boon Deadlock demo parser."""

from boon._boon import Demo
from boon.errors import InvalidDemoError

__all__ = ["Demo", "InvalidDemoError"]
