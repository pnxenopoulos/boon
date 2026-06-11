# Name Tables (`.vdata`)

A demo never spells out the name of an ability or a modifier. It refers to them by a
32-bit hash token — a Source 2 `CUtlStringToken`. To turn those tokens back into
human-readable names (`"spectral_wall"`, `"modifier_tentacle_debuff"`), boon ships
two static lookup tables generated ahead of time from Deadlock's game data. Those
tables are built from Valve's `.vdata` files.

## What `.vdata` files are

`.vdata` files are part of Deadlock's shipped game data, packed inside the game's VPK
archives (`pak01_dir`). They are written in Valve's **KV3** format — a non-standard,
JSON-like text structure where indentation and braces define nested objects. They are
extracted from the VPKs with [Source2Viewer / ValveResourceFormat](https://github.com/ValveResourceFormat/ValveResourceFormat).

boon uses exactly two of them:

| File | Provides | Surfaced as |
|------|----------|-------------|
| `abilities.vdata` | Every hero ability and item, plus the modifier subclasses they spawn | `ability_names()`, part of `modifier_names()` |
| `modifiers.vdata` | The generic/global modifiers (shop zones, capture auras, boss invulnerability, …) | part of `modifier_names()` |

These files live upstream at `game/citadel/pak01_dir/scripts/` in
[SteamDatabase/GameTracking-Deadlock](https://github.com/SteamDatabase/GameTracking-Deadlock),
which is where boon fetches them from.

## Why they matter

The demo stream identifies abilities and modifiers **by token, not by name**:

- An ability is referenced by the `CUtlStringToken` of its subclass name.
- A modifier is referenced by the `modifier_subclass` token on each
  `CModifierTableEntry` — the hash of the modifier's `_my_subclass_name`.

That token is a **MurmurHash2** of the name string, computed with Source 2's seed
`0x31415926`:

```
token = MurmurHash2(subclass_name, seed = 0x31415926)
```

The hash is one-way — given only the demo, there is no way to recover the original
name. The `.vdata` files are the *only* place those names exist in plain text. So they
are the source of truth that makes `ability_id` and `modifier_id` resolvable at all.
Without them, the parser would still extract the IDs, but every name would be unknown.

This is also why those columns ship as raw integers (see {doc}`../faq`): the demo
carries the token, and resolving it to a name is a separate, optional lookup against
these tables.

## How names are extracted

The generator hashes every candidate name and writes a `hash → name` table to a
generated Rust source file. Two tables come out of it:

**Abilities** (`abilities.rs`) — the ability names are simply the **top-level keys**
of `abilities.vdata`. Each key is one ability or item.

**Modifiers** (`modifiers.rs`) — a single demo can reference a modifier defined in
either file, so the modifier table is the **union of three sources**:

1. every top-level key in `modifiers.vdata` — the generic/global modifiers;
2. every nested `_my_subclass_name` in `modifiers.vdata`;
3. the `_my_subclass_name` of each modifier subclass nested in `abilities.vdata`.

Source 3 needs a filter. `abilities.vdata` interleaves three kinds of nested
subclass — modifiers, scale-functions, and abilities/items — and all of them carry a
`_my_subclass_name`. The discriminator is each block's own `_class`: only blocks whose
`_class` starts with `modifier_` are modifiers (scale-functions are `scale_function_*`,
abilities `citadel_ability_*` / `citadel_item`, …). The bulk of real gameplay
modifiers live here, nested under the ability that grants them, **not** in
`modifiers.vdata`.

## Coverage

As of game build `6557`, the tables hold:

| Table | Entries |
|-------|---------|
| Abilities | 794 |
| Modifiers | 917 |

Many modifiers are registered in engine/C++ code and appear in **no** `.vdata` file. A
demo can still reference those by token, but there is no name string to recover, so a
meaningful share of `modifier_id` values resolve to `MODIFIER_NOT_FOUND`. This is a
limitation of the name list, not of the hashing — every token still hashes correctly;
some simply have no published name.

## Regeneration

The tables are checked into the repo as generated Rust (`crates/boon/src/abilities.rs`
and `crates/boon/src/modifiers.rs`), each carrying a `Last updated:` date in its
header. They are *not* regenerated at build time. To refresh them after a Deadlock
patch:

```bash
scripts/sync-name-tables.sh
```

That script clones the upstream GameTracking-Deadlock repo, copies the two `.vdata`
files to the repo root, runs `scripts/generate-name-tables` to rebuild the tables, and
cleans up. Pin a specific build with `DEADLOCK_REF=<branch|tag|commit>`.

The lookups are exposed from the core crate as `ability_name(id)` / `modifier_name(id)`
and from Python as `ability_names()` / `modifier_names()` (see {doc}`../api`).
