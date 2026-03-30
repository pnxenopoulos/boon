# Entity Classes

Deadlock demos contain hundreds of entity classes. This page documents the most
important ones for data analysis. Use the CLI to discover all classes in a specific
demo:

```bash
boon-cli classes match.dem --filter Citadel
```

## Player Entities

Player data is split across two entity types linked by an entity handle.

### `CCitadelPlayerController`

The player's **controller** — holds identity, stats, and game-level data. There is
one per player in the match.

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `m_iszPlayerName` | String | Display name |
| `m_steamID` | U64 | Steam ID |
| `m_iTeamNum` | U64 | Team number (see [Teams](teams.md)) |
| `m_nOriginalLaneAssignment` | I64 | Starting lane |
| `m_hPawn` | U32 | Entity handle to the player's pawn |
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

The player's **pawn** — represents the physical character in the game world. Holds
position, health, and combat state.

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX` | F32 | X position |
| `CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY` | F32 | Y position |
| `CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ` | F32 | Z position |
| `m_angClientCamera` | QAngle | Camera angles (pitch, yaw, roll) |
| `m_iHealth` | I64 | Current health |
| `m_iMaxHealth` | I64 | Maximum health |
| `m_lifeState` | I64 | Life state (0 = alive, 1 = dying, 2 = dead) |
| `m_flDeathTime` | F32 | Time of death |
| `m_flLastSpawnTime` | F32 | Time of last spawn |
| `m_flRespawnTime` | F32 | Respawn timer |
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

### Controller-to-Pawn Link

The controller's `m_hPawn` field is an **entity handle**. To find the corresponding
pawn, mask the lower 15 bits to get the entity index:

```
pawn_entity_index = m_hPawn & 0x7FFF
```

This is how the `player_ticks` property joins data from both entity types into a single
DataFrame row.

## World State

### `CCitadelGameRulesProxy`

The **game rules** entity — tracks global match state. There is exactly one per demo.

**Key fields:**

| Field | Type | Description |
|-------|------|-------------|
| `m_pGameRules.m_bGamePaused` | Bool | Whether the game is paused |
| `m_pGameRules.m_tNextMidBossSpawnTime` | F32 | Next midboss spawn time |
| `m_pGameRules.m_unMatchID` | U64 | Match ID |

## Other Entity Classes

These are commonly present in demos but not currently exposed through the Python API:

| Class | Description |
|-------|-------------|
| `CCitadel_BreakableProp` | Destructible environment objects (crates, boxes) |
| `CCitadelMinimapComponent` | Minimap state and visibility |
| `CCitadelTeam` | Team-level aggregated data |
| `CCitadel_Ability_*` | Individual hero abilities |
| `CCitadel_Item_*` | Purchasable items |
| `CCitadelProjectile` | In-flight projectiles |
| `CNPC_*` | Non-player characters (creeps, bosses) |
| `CWorld` | World root entity |

Use the CLI's `entities` and `send-tables` commands to explore the full set of
classes and their fields in any demo.
