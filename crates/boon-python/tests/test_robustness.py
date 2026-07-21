"""Malformed demos must fail with a clean error, never a Rust panic/abort.

A panic inside the extension surfaces as ``pyo3_runtime.PanicException`` (and,
worse, could abort under ``panic=abort``); every corrupt or truncated input
should instead raise one of boon's own exceptions. Skips when no fixture is
present.
"""

import random

import pytest
from boon import Demo

from conftest import FIXTURES_DIR


def _fixture_bytes() -> bytes:
    dems = sorted(FIXTURES_DIR.glob("*.dem")) if FIXTURES_DIR.is_dir() else []
    if not dems:
        pytest.skip("No demo fixtures available")
    return dems[0].read_bytes()


def test_corrupt_and_truncated_demos_never_panic(tmp_path) -> None:
    data = _fixture_bytes()
    path = tmp_path / "corrupt.dem"
    rng = random.Random(0)

    for i in range(40):
        if i % 2 == 0:  # random truncation
            blob = data[: rng.randint(16, len(data))]
        else:  # random byte corruption
            b = bytearray(data)
            for _ in range(rng.randint(1, 250)):
                b[rng.randrange(len(b))] = rng.randrange(256)
            blob = bytes(b)
        path.write_bytes(blob)
        try:
            demo = Demo(str(path))
            _ = demo.players
            _ = demo.kills
            _ = demo.player_ticks
            _ = demo.damage
        except Exception as e:  # noqa: BLE001 - any *clean* error is acceptable
            assert (
                type(e).__name__ != "PanicException"
            ), f"case {i} panicked instead of erroring cleanly: {e}"
