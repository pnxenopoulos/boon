# Match 108575009 catalog fixture

Client version: `6712`.
Source: `SteamTracking/GameTracking-Deadlock` at
`8580b13d18d5d966430d2c41501f34bdb7c6243a`.

These files retain the boon-data records for McGinnis, her owned abilities,
and the active modifiers and their owners at tick `187554`. They also retain
the stat and state enums and all misc records and generic data.
The misc records identify permanent pickups, so their recorded totals do not
get counted again through the active pickup modifiers. Record definitions and stat changes
are copied without changing their values. Unrelated records and indexes are not included.
The JSON files use gzip to reduce their size.
The subset keeps the ammo regression independent of downloads and user caches.
