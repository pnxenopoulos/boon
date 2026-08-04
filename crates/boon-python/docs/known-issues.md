# ⚠️ Known Issues

Deadlock is in active development, and Valve frequently changes the demo file format. This page documents known issues and limitations in boon that stem from these changes.

If you encounter a problem not listed here, please report it on [GitHub Issues](https://github.com/pnxenopoulos/boon/issues) or in the [Discord](https://discord.gg/WmjZHxWrCD).

## Banned heroes are frequently absent

`demo.banned_heroes` reads the `k_EUserMsg_BannedHeroes` (msg_type 366) user message, which the server sends once, early in the demo, before the match starts. That message is not reliably present in GOTV recordings: it appears in some older builds and is absent from every newer demo tested so far.

An empty frame therefore means only "no bans were recorded" — it is **not** proof that nothing was banned. Two things are indistinguishable from the demo alone:

- a match that genuinely had no bans, and
- a build that never emits the message at all.

Note that absence is not purely a build property. Two demos recorded on the same server version can differ, with one carrying the message and the other not — so an empty result on a build known to emit it still just means that particular match had no bans.

The message also carries nothing but the hero IDs — no team, no banning player, and no pick/ban ordering — so it cannot be used to reconstruct a draft, only to list which heroes were unavailable.

## Ability upgrades empty on older demos

Valve renamed the entity field `m_nUpgradeBits` to `m_nUpgradeInfo` and changed its encoding. Boon uses the current field name (`m_nUpgradeInfo`), so `ability_upgrades` will return an empty DataFrame when parsing demos recorded before this change.
