# ⚠️ Known Issues

Deadlock is in active development, and Valve frequently changes the demo file format. This page documents known issues and limitations in boon that stem from these changes.

If you encounter a problem not listed here, please report it on [GitHub Issues](https://github.com/pnxenopoulos/boon/issues) or in the [Discord](https://discord.gg/tWCwmHDy2u).

## Banned heroes

The `k_EUserMsg_BannedHeroes` (msg_type 366) event is not reliably present in GOTV demo recordings. It appeared in older builds but is absent in newer ones. Because of this, boon does not expose banned hero data. If Valve restores this event, banned hero support will be re-added.

## Ability upgrades empty on older demos

Valve renamed the entity field `m_nUpgradeBits` to `m_nUpgradeInfo` and changed its encoding. Boon uses the current field name (`m_nUpgradeInfo`), so `ability_upgrades` will return an empty DataFrame when parsing demos recorded before this change. Demos from current builds work correctly.
