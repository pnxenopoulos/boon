# ⚠️ Known Issues

Valve changes Deadlock and its demo format frequently. This page lists known Boon limitations that result from these changes.

Report other problems on [GitHub Issues](https://github.com/pnxenopoulos/boon/issues) or in [Discord](https://discord.gg/WmjZHxWrCD).

## Boon calculates effective stats

The demo does not contain a final server value for every stat.
`demo.stat_ticks(...)` calculates these stats from the recorded player state,
active modifiers, and generated VData formulas.

A `*_complete` value of `true` means that Boon evaluated every matching effect
in its current catalog. It does not confirm that the result is identical to
the game server. Engine-only rules can change the result. These rules can
include caps, operation order, and values that depend on live game conditions.
Effects that are absent from VData are also absent from the catalog.

Use `demo.stat_effects(...)` to examine each source that Boon used.

## Banned heroes are frequently absent

`demo.banned_heroes` reads the `k_EUserMsg_BannedHeroes` user message. Its
`msg_type` is 366. The server can send this message once before the match.
GOTV recordings do not always contain the message. Some older demos contain
it. None of the newer tested demos contain it.

An empty frame means that the demo contains no ban data. It does not prove
that the match had no bans. The demo cannot distinguish these cases:

- The match had no bans.
- The server build did not send the message.

Two demos from the same server version can differ. One demo can contain the
message while the other demo does not contain it.

The message contains only hero IDs. It does not contain the team, banning
player, or draft order. Boon can list unavailable heroes, but it cannot build
the draft order.

## Ability upgrades empty on older demos

Valve renamed `m_nUpgradeBits` to `m_nUpgradeInfo` and changed its encoding.
Boon uses `m_nUpgradeInfo`. Therefore, `ability_upgrades` returns an empty
DataFrame for demos that Valve recorded before this change.
