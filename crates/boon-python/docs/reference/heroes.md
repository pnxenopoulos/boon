# Hero IDs

Hero IDs appear in `Demo.players`, `Demo.player_ticks`, and event datasets.
Resolve them with the selected boon-data catalog instead of a fixed table:

```python
from boon import hero_names

names = hero_names()                 # newest local catalog, or automatic download
historical = hero_names("6712")      # explicit Deadlock client version
for hero_id, name in sorted(names.items()):
    print(hero_id, name)
```

The catalog's `hero_id` comes from the hero definition's `m_HeroID`.
Names use English localization when available and internal names otherwise.
Templates without a hero ID are not included. Unreleased and test heroes can appear.
Unknown IDs have no mapping in that snapshot.

Names and available heroes can change between client versions. See {doc}`../data`
for version selection, downloads, and offline use.
