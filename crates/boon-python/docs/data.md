# Game data downloads

```bash
boon versions
boon get 6712
boon get 6795 --images map --images heroes --images items
boon versions --local
```

Boon downloads versioned JSON catalogs from
[boon-data](https://github.com/pnxenopoulos/boon-data). Use Boon without a Deadlock installation.
These commands download catalogs and optional images, check file integrity, and store the files locally.

<a id="automatic-name-lookups"></a>

## Resolve Boon names from boon-data

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

<a id="download-a-client-version"></a>

## Download boon-data with boon get

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

<a id="download-images"></a>

## Download Boon image groups with boon get --images

Image downloads are optional. Select groups with repeated `--images` options:

```bash
boon get 6795 --images map --images heroes
boon get 6795 --images items --images icons
boon get 6795 --images all
```

| Selection | Images |
| --- | --- |
| `map` | The three Midtown minimap layers |
| `heroes` | Hero minimap icons, small icons, and cards |
| `abilities` | Ability icons and declared alternate icons |
| `items` | Item shop images |
| `icons` | Stat/property icons, such as `fire_rate` and `ammo_clip_size` |
| `all` | All five groups |

Use a boon-data release that includes images. Catalog-only releases report an
error if images are requested. If your local metadata is older, use
`boon get VERSION --images all --force` to fetch it again.

Boon verifies `images.json` and `images.zip` against the version index and
manifest. It also checks each extracted file's size and SHA-256 checksum.
The release has one ZIP: selecting a group downloads the complete archive,
then extracts only that group. The archive stays in the cache. Repeated requests
for an explicit version work offline; adding groups uses the cached archive.
Earlier selections remain installed. `--force` fetches fresh files and retains
previous selections when their receipt can be read.

Files are stored under `~/.boon/VERSION/images/`. Read `images.json` for each
file's relative `path`. Shared images can use a path from another group.
The index keys are hero IDs, ability IDs for items and abilities, and game
filenames for stat icons. Images retain their PNG or SVG format.

```python
import json
from boon import data

folder = data.update("6795", images=["map", "icons"])
image_dir = folder / "images"
index = json.loads((image_dir / "images.json").read_text())
fire_rate_icon = image_dir / index["stats"]["fire_rate"]["path"]
print(fire_rate_icon)
```

`images` accepts a group string or a sequence of group strings. Enum members
also work, for example `images=[data.ImageGroup.ICONS]`. The members are `ALL`,
`MAP`, `HEROES`, `ABILITIES`, `ITEMS`, and `ICONS`, with the string values above.
Catalog/name lookups do not download or validate images. Catalog installation
and image installation each use verified staging; failed image updates retain
the previous image selection. `boon remove VERSION` also removes its images.

<a id="remove-a-client-version"></a>

## Remove local boon-data with boon remove

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

<a id="local-storage"></a>

## Configure the Boon local data cache

The default cache is `~/.boon/<client-version>/`:

```text
~/.boon/6712/
├── abilities.json
├── heroes.json
├── modifiers.json
├── misc.json
├── manifest.json
├── .install.json
└── images/                    # optional
    ├── images.json
    ├── images.zip
    ├── .install.json          # selected image groups
    └── ...                    # selected PNG and SVG files
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

<a id="python-access"></a>

## Read catalogs with the Boon Python data API

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
| `data.update(version=None, force=False, images=())` | Acquire catalogs and selected image groups for the specified version, or latest; return its directory |
| `data.remove(version)` | Delete one local version offline, including corrupt caches; return its former directory |
| `data.latest_version()` | Read the latest client version from the published index |
| `data.available_versions()` | Return published version metadata dictionaries, newest first |
| `data.local_versions()` | Return verified installed version metadata dictionaries, newest first; no network |
| `data.resolve_version(version=None)` | Validate an explicit version, otherwise choose newest installed, otherwise latest published |
| `data.manifest(version=None)` | Read the manifest, downloading missing files first |
| `data.available_files(version=None)` | List the selected release's catalog JSON names, downloading missing files first |
| `data.catalog_path(name, version=None)` | Return a verified local JSON path; accepted names are listed below |

`catalog_path()` accepts `abilities`, `heroes`, `modifiers`, `misc`, and `manifest`.
Each name also accepts its `.json` suffix. Other names cause `DataError`.
`version` is a numeric client-version string from `boon versions`, such as `"6712"`.
It is not Boon's package version or a replay tick.

Read helpers without a version prefer the newest installed client version.
`data.update()` without a version reads the online index for the latest version.
`data.DataError` reports network, metadata, filesystem, and integrity failures.
Set `data.BOON_DATA_DIR` to a `Path` to redirect an existing process.

<a id="relationship-to-replays"></a>

## Select boon-data for a Deadlock replay

A Deadlock `ClientVersion` differs from Boon's package version.
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
