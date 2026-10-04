# Entity Classes

Use `boon-dev` to list the entity classes in a demo.
Build the tool from source; see {doc}`../cli`.

```bash
boon-dev classes match.dem --filter Citadel
```

## Player Entities

Player data is split across two entity types linked by an entity handle.

### `CCitadelPlayerController`

The controller supplies player identity, counters, and match state.

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `m_iszPlayerName` | String | Display name |
| `m_steamID` | U64 | Steam ID |
| `m_iTeamNum` | U64 | Team number (see [Teams](teams.md)) |
| `m_nOriginalLaneAssignment` | I64 | Starting lane |
| `m_hPawn` | U32 | Entity handle to the player's pawn |
| `m_hHeroPawn` | U32 | Entity handle to the hero pawn when recorded |
| `m_PlayerDataGlobal.m_nHeroID` | U64 | Hero ID (see [Heroes](heroes.md)) |
| `m_PlayerDataGlobal.m_bAlive` | Bool | Alive status |
| `m_PlayerDataGlobal.m_iPlayerKills` | I64 | Kill count |
| `m_PlayerDataGlobal.m_iDeaths` | I64 | Death count |
| `m_PlayerDataGlobal.m_iPlayerAssists` | I64 | Assist count |
| `m_PlayerDataGlobal.m_iLevel` | I64 | Player level |
| `m_PlayerDataGlobal.m_iGoldNetWorth` | I64 | Gold net worth |
| `m_PlayerDataGlobal.m_iAPNetWorth` | I64 | Ability power net worth |
| `m_PlayerDataGlobal.m_iHeroDamage` | I64 | Total hero damage dealt |
| `m_PlayerDataGlobal.m_iHeroHealing` | I64 | Total hero healing |
| `m_PlayerDataGlobal.m_iObjectiveDamage` | I64 | Total objective damage |
| `m_PlayerDataGlobal.m_iSelfHealing` | I64 | Total self healing |
| `m_PlayerDataGlobal.m_iLastHits` | I64 | Last hit count |
| `m_PlayerDataGlobal.m_iDenies` | I64 | Deny count |
| `m_PlayerDataGlobal.m_iKillStreak` | I64 | Current kill streak |
| `m_PlayerDataGlobal.m_flHealthRegen` | F32 | Health regen rate |
| `m_PlayerDataGlobal.m_bHasRebirth` | Bool | Has rebirth item |
| `m_PlayerDataGlobal.m_bHasRejuvenator` | Bool | Has rejuvenator item |
| `m_PlayerDataGlobal.m_bUltimateTrained` | Bool | Ultimate ability trained |
| `m_PlayerDataGlobal.m_flUltimateCooldownStart` | F32 | Ultimate cooldown start |
| `m_PlayerDataGlobal.m_flUltimateCooldownEnd` | F32 | Ultimate cooldown end |

### `CCitadelPlayerPawn`

The hero pawn supplies position, health, and combat state.

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX` | F32 | X cell offset |
| `CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY` | F32 | Y cell offset |
| `CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ` | F32 | Z cell offset |
| `m_angClientCamera` | QAngle | Camera angles (pitch, yaw, roll) |
| `m_iHealth` | I64 | Current health |
| `m_iMaxHealth` | I64 | Maximum health |
| `m_lifeState` | I64 | Life state (0 = alive, 1 = dying, 2 = dead, 3 = respawnable, 4 = respawning); resolve with `lifestate_names()` |
| `m_flDeathTime` | F32 | Time of death |
| `m_flLastSpawnTime` | F32 | Time of last spawn |
| `m_flRespawnTime` | F32 | Recorded respawn time |
| `m_bInRegenerationZone` | Bool | In a regen zone |
| `m_nCurrencies.m_nCurrencies` | I64 | Current souls |
| `m_nSpentCurrencies.m_nSpentCurrencies` | I64 | Spent souls |
| `m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID` | I64 | Hero ID |
| `m_unHeroBuildID` | I64 | Hero build ID |
| `m_sInCombat.m_flStartTime` | F32 | In-combat timer start |
| `m_sInCombat.m_flEndTime` | F32 | In-combat timer end |
| `m_sInCombat.m_flLastDamageTime` | F32 | Last damage taken/dealt |
| `m_sPlayerDamageDealt.m_flStartTime` | F32 | Player damage dealt start |
| `m_sPlayerDamageDealt.m_flEndTime` | F32 | Player damage dealt end |
| `m_sPlayerDamageTaken.m_flStartTime` | F32 | Player damage taken start |
| `m_sPlayerDamageTaken.m_flEndTime` | F32 | Player damage taken end |
| `m_timeRevealedOnMinimapByNPC` | F32 | Minimap reveal time |

Origin components are cell offsets. Combine them with cell indices for full world positions.
Boon snapshot positions use full world coordinates.

### Controller-to-Pawn Link

Controller pawn fields contain entity handles.
Mask the lower 15 bits to get the entity index:

```
pawn_entity_index = m_hPawn & 0x7FFF
```

For a hero handle, use `m_hHeroPawn` when available.
During death, `m_hPawn` can identify a spectator pawn.
Stat and state queries prefer the hero pawn.
Handle serials also matter when entity indices are reused.

## World State

### `CCitadelGameRulesProxy`

The **game rules** entity — tracks global match state. There is exactly one for each demo.

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `m_pGameRules.m_bGamePaused` | Bool | Whether the game is paused |
| `m_pGameRules.m_tNextMidBossSpawnTime` | F32 | Next midboss spawn time |
| `m_pGameRules.m_unMatchID` | U64 | Match ID |

## Other Entity Classes

Some class data has Python datasets; inspect raw entities for other fields.

| Class | Description |
|-------|-------------|
| `CCitadel_BreakableProp` | Destruction events in `demo.breakables` |
| `CCitadelMinimapComponent` | Minimap state and visibility |
| `CCitadelTeam` | Team-level aggregated data |
| `CCitadel_Ability_*` | Ability state in `demo.ability_ticks`; selected inputs in stat queries |
| `CCitadel_Item_*` | Purchasable items |
| `CCitadelProjectile` | In-flight projectiles |
| `CNPC_*` | Selected state in `demo.troopers`, `demo.neutrals`, and objective datasets |
| `CWorld` | World root entity |

Use the CLI's `entities` and `send-tables` commands to explore the full set of
classes and their fields in any demo.
