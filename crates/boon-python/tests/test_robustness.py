"""Malformed demos must fail with a clean error, never a Rust panic/abort.

A panic inside the extension surfaces as ``pyo3_runtime.PanicException`` (and,
worse, could abort under ``panic=abort``); every corrupt or truncated input
should instead raise one of boon's own exceptions. Skips when no fixture is
present.
"""

import random

from boon import Demo
from conftest import _require_demo_fixture


def test_corrupt_and_truncated_demos_never_panic(tmp_path) -> None:
    data = _require_demo_fixture().read_bytes()
    truncated = tmp_path / "truncated.dem"
    corrupted = tmp_path / "corrupt.dem"
    corrupted.write_bytes(data)
    rng = random.Random(0)

    for i in range(40):
        changes = {}
        if i % 2 == 0:
            path = truncated
            path.write_bytes(memoryview(data)[: rng.randint(16, len(data))])
        else:
            path = corrupted
            for _ in range(rng.randint(1, 250)):
                value = rng.randrange(256)
                changes[rng.randrange(len(data))] = value
            with path.open("r+b") as file:
                for offset, value in changes.items():
                    file.seek(offset)
                    file.write(bytes([value]))
        demo = None
        try:
            demo = Demo(str(path))
            _ = demo.players
            _ = demo.kills
            _ = demo.player_ticks
            _ = demo.damage
        except Exception as e:  # noqa: BLE001 - any *clean* error is acceptable
            assert type(e).__name__ != "PanicException", (
                f"case {i} panicked instead of erroring cleanly: {e}"
            )
        finally:
            # Release the file mapping before restoring the original bytes.
            demo = None
            if changes:
                with path.open("r+b") as file:
                    for offset in changes:
                        file.seek(offset)
                        file.write(data[offset : offset + 1])
