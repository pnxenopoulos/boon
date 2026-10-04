# Name lookups

Hero, ability/item, modifier, and breakable names come from versioned
[boon-data](https://github.com/pnxenopoulos/boon-data) JSON catalogs.
Boon does not embed these maps in its source or compiled package.

## IDs and labels

- `heroes.json`: `hero_id` comes from `m_HeroID`. Names use `display_name`,
  falling back to `hero_name` when localization is missing.
- `abilities.json`: `ability_id` is the Source 2 string token of `ability_name`.
  `ability_names()` returns internal names. `ability_display_names()` includes
  entries with an English `display_name`.
- `modifiers.json`: `modifier_id` and `qualified_modifier_id` are accepted.
  `modifier_id` resolves to `modifier_name`.
  `qualified_modifier_id` resolves to `qualified_modifier_name`.
- `misc.json`: records with `definition._class = "citadel_breakable_prop"`
  supply `misc_id` to `misc_name` lookups. The ID matches the replay's raw
  breakable `m_nSubclassID`; names are not inferred from a prefix.

String tokens are unsigned MurmurHash2 hashes with seed `0x31415926`.
Boon consumes the IDs already computed by boon-data; it does not rehash names.
Repeated definitions can share an ID and name. A name map deduplicates those
labels, but does not select a gameplay definition. Conflicting labels are errors.
Use boon-data's record indexes to examine multiple modifier definitions.
IDs missing from the selected catalog stay unresolved; no names are invented.

## Acquisition and caching

Python name functions accept `version=None`. The default chooses the newest
verified local client version. With no complete local installation, Boon downloads
latest from the version index. An explicit version is downloaded if missing.
The Python CLI, `demo.banned_heroes`, and `demo.breakables` use these same functions.

Rust callers use `CatalogNames::load(None)` or `CatalogNames::load(Some("6712"))`.
`boon-dev` also uses this loader. Keep the returned maps across a parse so each
name lookup is an in-memory operation. Local files and Rust downloads share
`~/.boon/<client-version>/` with Python, including `.install.json` receipts.
`BOON_DATA_DIR` overrides that cache root.

Downloads include integrity checks of all five JSON assets, including sizes, SHA-256 hashes, and
manifest consistency, before installation. A failed download leaves no partial
installation. Installed versions work offline. Python caches parsed name maps
and does file checks before reuse. Reinstalling a version invalidates its maps.

`CatalogNames::from_directory` reads locally built JSONs directly. It compares catalog
names and source revisions. The caller must do checksum checks. This method does
not use the network.

## Other lookup tables

Protocol enums for teams, game modes, hitgroups, life states, and patron phases
stay in Boon. All VData name lookups come from boon-data; no name-table
generation script is necessary.

If the published index has no designated `latest` (for example, after only
historical backfills), automatic downloads use the highest published client
version. An empty index produces an error without installing anything.
