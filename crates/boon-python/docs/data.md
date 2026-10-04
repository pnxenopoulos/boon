# Game data downloads

Boon downloads versioned JSON catalogs from
[boon-data](https://github.com/pnxenopoulos/boon-data). Use Boon without a Deadlock installation.
These commands download the JSON files, do integrity checks, and store the files locally.

## Automatic name lookups

`hero_names()`, `ability_names()`, `ability_display_names()`, `modifier_names()`,
and `breakable_names()`
read these catalogs. They use the newest verified local client version. If none
is installed, they download the latest version. An explicit
`version="6712"` selects that client version and downloads it if missing.
Existing installations work offline; lookup calls do not search for newer releases.
`boon get` explicitly downloads the latest published version.

The CLI, `demo.banned_heroes`, and `demo.breakables` use the same name policy.
`demo.item_purchases` uses a catalog for component links.
Use `demo.get_item_purchases(data_version="6712")` to select that catalog explicitly.
Empty item-event tables use no catalog.

Importing Boon does not download data.
Raw combat datasets and `Demo.load()` use no catalogs for their IDs.
Stat, state, imbue, and calculated-ammo queries download their selected version if missing.

Rust uses the same cache and download verification:

```rust
let names = boon::CatalogNames::load(None)?;
println!("{}", names.hero_name(1));
// Pin a client version:
let historical = boon::CatalogNames::load(Some("6712"))?;
```

Keep the returned Rust object for repeated lookups. `CatalogNames::from_directory`
reads a locally built catalog directory without downloads or manifest hash checks. `CatalogNames::load` does integrity checks of installed release files before reading them.

## Download a client version

```bash
boon versions                 # client versions, build date/time, installed status
boon get                      # latest client version in the published index
boon get 6712                 # a specific Deadlock client version
boon versions --local         # installed versions, without network access
boon versions --json          # machine-readable version metadata and status
boon get 6712 --force          # repair or replace an installation after verification
boon remove 6712               # remove an installed version without network access
```

`boon versions` reads the published
[versions.json](https://raw.githubusercontent.com/pnxenopoulos/boon-data/data-index/versions.json).
It shows the Deadlock client version, **VersionDate** and **VersionTime** from
`steam.inf`, and whether its files are installed locally. Source dates and times
are kept as written, without assuming a timezone. Missing values appear as `-` in text and `null` in JSON.
JSON output includes `version_date`, `version_time`, and the boon-data release
timestamp (`released_at`); offline listings read these from the installation
receipt. Versions are sorted numerically, newest first. A corrupt or incomplete installation is not marked installed.
If the index is unavailable, the command reports an error and lists verified local installations.
The command fails if the index and local installations are unavailable.
`--local` never contacts the network.

`boon get` defaults to the index's `latest` client version. An explicit version
with a verified installation is reused without a network request. An unknown
version fails rather than downloading a different version.

## Remove a client version

`boon remove VERSION` deletes that version's local directory, including its
installation receipt. Set a version; the command never selects one
implicitly or contacts GitHub. It also removes corrupt or incomplete caches.
Missing installations produce an error, and symlinked/redirected version
directories are rejected. `--json` reports the version, path, and `removed: true`.

```bash
boon remove 6712
boon get 6712                   # download a fresh copy
```

Removal uses the configured `BOON_DATA_DIR` and does not affect remote releases.

## Local storage

The default cache is `~/.boon/<client-version>/`:

```text
~/.boon/6712/
├── abilities.json
├── heroes.json
├── modifiers.json
├── misc.json
├── manifest.json
└── .install.json
```

Published JSON files are stored unchanged. Releases contain all five JSON files.
The local `.install.json` receipt records the requested client version and
source build date and time. It also stores the publication timestamp, snapshot,
and expected checksums. Offline listings use this receipt for file-integrity checks.

Set `BOON_DATA_DIR` before you import Boon or run the CLI to choose a
different cache root. For example:

```bash
BOON_DATA_DIR=/path/to/data boon get 6712
```

Downloads are staged under the cache root. Each file must match the size and SHA-256 checksum in the version index.
The manifest must also agree with the index. Boon makes the installation
available only after these checks pass. A failed `--force`
download keeps the existing installation. Ordinary downloads never replace
an invalid installation; use `--force` to repair it.

Different client versions can share an unchanged published snapshot. Boon
resolves the exact download URLs through the index and stores files under the
**requested client version**. The downloaded manifest retains the snapshot's
original client version and source commit.

## Python access

```python
from boon import data

folder = data.update("6712")
print(folder)
print(data.available_files("6712"))
print(data.catalog_path("abilities", "6712"))
print(data.manifest("6712")["source"]["commit"])

for entry in data.local_versions():
    print(entry["client_version"], entry["released_at"])
```

| Function | Behavior |
| --- | --- |
| `data.update(version=None, force=False)` | Acquire the specified client version, or latest; return its directory |
| `data.remove(version)` | Delete one local version offline, including corrupt caches; return its former directory |
| `data.latest_version()` | Read the latest client version from the published index |
| `data.available_versions()` | Return published version metadata dictionaries, newest first |
| `data.local_versions()` | Return verified installed version metadata dictionaries, newest first; no network |
| `data.resolve_version(version=None)` | Validate an explicit version, otherwise choose newest installed, otherwise latest published |
| `data.manifest(version=None)` | Read the manifest, downloading missing files first |
| `data.available_files(version=None)` | List the selected release's JSON asset names, downloading missing files first |
| `data.catalog_path(name, version=None)` | Return a verified local JSON path; accepted names are listed below |

`catalog_path()` accepts `abilities`, `heroes`, `modifiers`, `misc`, and `manifest`.
Each name also accepts its `.json` suffix. Other names cause `DataError`.
`version` is a numeric client-version string from `boon versions`, such as `"6712"`.
It is not Boon's package version or a replay tick.

Read helpers without a version prefer the newest installed client version.
`data.update()` without a version reads the online index for the latest version.
`data.DataError` reports network, metadata, filesystem, and integrity failures.
Set `data.BOON_DATA_DIR` to a `Path` to redirect an existing process.

## Relationship to replays

A Deadlock `ClientVersion` is different of Boon's package version.
Boon does not map it to `demo.build`. Opening a `Demo` does not select a
catalog version. Name lookups use the selection policy described above.
Select `version=` explicitly to get names from a particular client.

Catalogs provide names and definitions. {doc}`hero-stats` explains how to use
them to calculate the supported weapon, movement, resistance, and lifesteal stats.
Use {doc}`ability-stats` for ability bonuses and imbues, and {doc}`player-states`
for recorded state flags.
Calculations do not change raw demo datasets.

If the published index has no designated `latest` (for example, after only
historical backfills), automatic downloads use the highest published client
version. An empty index produces an error without installing anything.
