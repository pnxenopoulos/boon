# Game data downloads

Boon acquires versioned JSON catalogs from
[boon-data](https://github.com/pnxenopoulos/boon-data). No Deadlock installation
is required. These commands download, verify, and store the JSON files locally.

## Automatic name lookups

`hero_names()`, `ability_names()`, `ability_display_names()`, `modifier_names()`,
and `breakable_names()`
read these catalogs. They use the newest verified local client version. If none
is installed, they download the latest version automatically. An explicit
`version="6698"` selects that client version and downloads it if missing.
Existing installations work offline; lookup calls do not check for newer releases.
`boon get` explicitly downloads the latest published version.

The CLI, `demo.banned_heroes`, and `demo.breakables` follow the same policy when resolving names.
Importing Boon or parsing datasets that contain raw IDs does not download data.

Rust uses the same cache and download verification:

```rust
let names = boon::CatalogNames::load(None)?;
println!("{}", names.hero_name(1));
// Pin a client version:
let historical = boon::CatalogNames::load(Some("6698"))?;
```

Keep the returned Rust object for repeated lookups. `CatalogNames::from_directory`
reads a locally built catalog directory without downloading or checking manifest
hashes. `CatalogNames::load` verifies installed release files before reading them.

## Download a client version

```bash
boon versions                 # client versions, build date/time, installed status
boon get                      # latest client version in the published index
boon get 6698                 # a specific Deadlock client version
boon versions --local         # installed versions, without network access
boon versions --json          # machine-readable version metadata and status
boon get 6698 --force          # repair or replace an installation after verification
boon remove 6698               # remove an installed version without network access
```

`boon versions` reads the published
[versions.json](https://raw.githubusercontent.com/pnxenopoulos/boon-data/data-index/versions.json).
It shows the Deadlock client version, **VersionDate** and **VersionTime** from
`steam.inf`, and whether its files are installed locally. Source dates and times
are preserved as written, without assuming a timezone. Missing values appear as `-` in text and `null` in JSON.
JSON output includes `version_date`, `version_time`, and the boon-data release
timestamp (`released_at`); offline listings read these from the installation
receipt. Versions are sorted numerically, newest first. A corrupt or incomplete installation is not marked installed.
If the index is unavailable, the command reports the error and lists verified
local installations; it exits with an error if neither is available.
`--local` never contacts the network.

`boon get` defaults to the index's `latest` client version. An explicit version
with a verified installation is reused without a network request. An unknown
version fails rather than downloading a different version.

## Remove a client version

`boon remove VERSION` deletes that version's local directory, including its
installation receipt. A version is required; the command never selects one
implicitly or contacts GitHub. It also removes corrupt or incomplete caches.
Missing installations produce an error, and symlinked/redirected version
directories are rejected. `--json` reports the version, path, and `removed: true`.

```bash
boon remove 6698
boon get 6698                   # download a fresh copy
```

Removal uses the configured `BOON_DATA_DIR` and does not affect remote releases.

## Local storage

The default cache is `~/.boon/<client-version>/`:

```text
~/.boon/6698/
├── abilities.json
├── heroes.json
├── modifiers.json
├── misc.json
├── manifest.json
└── .install.json
```

Published JSON files are stored unchanged. Releases contain all five JSON files.
The local `.install.json` receipt records the requested client version, source
build date and time, publication timestamp, snapshot, and expected checksums so
offline listings can verify the installation.

Set `BOON_DATA_DIR` before importing Boon or invoking the CLI to choose a
different cache root. For example:

```bash
BOON_DATA_DIR=/path/to/data boon get 6698
```

Downloads are staged under the cache root. All files listed by the release must match the sizes
and SHA-256 checksums in the version index, and the manifest must agree with
that index, before the installation is made available. A failed `--force`
download preserves the existing installation. Ordinary downloads never replace
an invalid installation automatically; use `--force` to repair it.

Different client versions can share an unchanged published snapshot. Boon
resolves the exact download URLs through the index and stores files under the
**requested client version**. The downloaded manifest retains the snapshot's
original client version and source commit.

## Python access

```python
from boon import data

folder = data.update("6698")
print(folder)
print(data.available_files("6698"))
print(data.catalog_path("abilities", "6698"))
print(data.manifest("6698")["source"]["commit"])

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
| `data.manifest(version=None)` | Read the manifest, acquiring missing files first |
| `data.available_files(version=None)` | List the selected release's JSON asset names, acquiring missing files first |
| `data.catalog_path(name, version=None)` | Return a verified local path; accepts `abilities` or `abilities.json` |

Read helpers without a version prefer the newest installed client version.
`data.update()` always checks the online index for the latest version.
`data.DataError` reports network, metadata, filesystem, and integrity failures.
Set `data.BOON_DATA_DIR` to a `Path` to redirect an existing process.

## Relationship to replays

A Deadlock `ClientVersion` is independent of Boon's package version and has not
been mapped to `demo.build`. Opening a `Demo` does not select or download these
catalogs. Downloading them does not change the parser's bundled name tables or
recorded datasets, or add calculated resistance/lifesteal percentages. The
catalogs are available for callers to inspect and use independently.

If the published index has no designated `latest` (for example, after only
historical backfills), automatic downloads use the highest published client
version. An empty index produces an error without installing anything.
