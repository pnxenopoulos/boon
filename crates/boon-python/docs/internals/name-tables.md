# Name Tables (`.vdata`)

A demo stores an ability or modifier as a 32-bit Source 2 `CUtlStringToken`.
It does not store the name. Boon includes static tables that map these tokens
to internal names, such as `"spectral_wall"`. A separate generated table maps
internal ability and item names to their English game names.

## What `.vdata` files are

`.vdata` files are in the Deadlock VPK archives (`pak01_dir`). They use
Valve's **KV3** format. KV3 is a text structure that uses indentation and
braces for nested objects. Use
[Source2Viewer / ValveResourceFormat](https://github.com/ValveResourceFormat/ValveResourceFormat)
to extract these files.

Boon uses four files:

| File | Provides | Surfaced as |
|------|----------|-------------|
| `abilities.vdata` | Every hero ability and item, plus the modifier subclasses they spawn | `ability_names()`, part of `modifier_names()` |
| `modifiers.vdata` | The generic/global modifiers (shop zones, capture auras, boss invulnerability, …) | part of `modifier_names()` |
| `heroes.vdata` | Hero baseline resistance and scaling inputs | generated resistance tables |
| `misc.vdata` | Miscellaneous entity templates | `breakables.subclass_name` for `citadel_breakable_prop` entries |

These files are in `game/citadel/pak01_dir/scripts/` in
[SteamDatabase/GameTracking-Deadlock](https://github.com/SteamDatabase/GameTracking-Deadlock).
Boon gets the files from this repository.

## Why they matter
The display-name table also uses Valve's English localization files:


| File | Provides |
|------|----------|
| `citadel_heroes_english.txt` | Hero ability display names |
| `citadel_gc_mod_names_english.txt` | Shop item display names |

Both live under `game/citadel/resource/localization/` in the same upstream repository.

The demo identifies abilities, modifiers, and breakable subclasses **by token**:

- An ability is referenced by the `CUtlStringToken` of its subclass name.
- A modifier uses the `modifier_subclass` token in each
  `CModifierTableEntry`. The token is the hash of `_my_subclass_name`.
- A breakable prop stores the token of its `misc.vdata` template in
  `m_nSubclassID`.

The token is a **MurmurHash2** of the name string. Source 2 uses seed
`0x31415926`:

```
token = MurmurHash2(subclass_name, seed = 0x31415926)
```

The hash is one-way. The demo cannot provide the source name. The `.vdata`
files contain these names as text. Boon uses them to resolve ability, modifier,
and breakable subclass IDs. Without the files, Boon can return the IDs but not
the names.

Boon keeps the integer ID columns for this reason. See {doc}`../faq`.
`breakables.subclass_name` contains the resolved name. A token that is absent
from the bundled table returns `BREAKABLE_NOT_FOUND`.

## How names are extracted

The generator hashes each candidate name. It writes a `hash → name` table to
a generated Rust source file. Boon has these generated tables:

**Abilities** (`abilities.rs`) — Top-level keys in `abilities.vdata`.
Each key identifies one ability or item.

**Ability display names** (`ability_display_names.rs`) — Top-level VData keys
that also occur in the English localization files. The table includes current
`ability_*`, `upgrade_*`, and `citadel_ability_*` names. It excludes
descriptions, aliases, modifiers, stale text, and invalid entries. Some hidden,
test, and retired entries have no localization. Boon does not add a display
name for these entries.

**Modifiers** (`modifiers.rs`) — A demo can use modifiers from two VData
files. The table combines three sources:

1. Each top-level key in `modifiers.vdata`.
2. Each nested `_my_subclass_name` in `modifiers.vdata`.
3. the `_my_subclass_name` of each modifier subclass nested in `abilities.vdata`.

Source 3 needs a filter. `abilities.vdata` contains modifiers, scale functions,
abilities, and items. All can have `_my_subclass_name`. The `_class` field
identifies the type. A modifier class starts with `modifier_`. Scale functions
start with `scale_function_*`. Abilities and items use
`citadel_ability_*` or `citadel_item`. Most gameplay modifiers occur under
their ability in this file. They do not occur in `modifiers.vdata`.

## Coverage

As of the sources synced on 2026-09-02, the tables hold:

| Table | Entries |
|-------|---------|
| Abilities | 794 |
| English ability/item display names | 457 |
| Modifiers | 1106 |
| Breakable prop subclasses | 30 |

The engine registers many modifiers that are absent from `.vdata`. A demo can
use their tokens, but Boon cannot get their names. These `modifier_id` values
resolve to `MODIFIER_NOT_FOUND`. The token is valid, but Valve did not publish
the name.

## Regeneration

The repository contains the generated Rust tables:
`crates/boon/src/abilities.rs`, `ability_display_names.rs`, `breakables.rs`,
and `modifiers.rs`. Each file has a `Last updated:` date. The build does not
regenerate these files. Run this command after a Deadlock update:

```bash
scripts/sync-name-tables.sh
```

The script clones GameTracking-Deadlock. It copies the required `.vdata` and
English localization files to the repository root. Then, it runs
`scripts/generate-name-tables` and removes temporary data. Set
`DEADLOCK_REF=<branch|tag|commit>` to use a specified build.

The core crate provides `ability_name(id)`,
`ability_display_name(internal_name)`, `modifier_name(id)`, and
`breakable_name(id)`. Python provides `ability_names()`,
`ability_display_names()`, and `modifier_names()`. Resolved breakable names are
in the `subclass_name` column of each `demo.breakables` row.
