# ⚠️ Known Issues

Valve changes Deadlock and its demo format frequently. This page lists known Boon limitations that result from these changes.

Report other problems on [GitHub Issues](https://github.com/pnxenopoulos/boon/issues) or in [Discord](https://discord.gg/WmjZHxWrCD).

## Player stat modifiers are not final stats

`demo.player_ticks` reads `m_vecStatViewerModifierValues` from each player
controller. The `stat_modifier_*` columns contain signed sums for the value
types that Boon knows. They do not include base hero stats, all item values, or
all temporary effects. Do not use these columns as final damage mitigation or
effective player stats.

`stat_modifier_values_available` is false when the demo serializer does not
contain this vector. `unknown_stat_modifier_count` is the number of vector
entries with a nonzero `EModifierValue` that this Boon version does not know.

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
