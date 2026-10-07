# Calculate hero stats

Use `demo.calculate_hero_stats()` for values at selected demo ticks.
Boon uses replay state, a boon-data catalog, and a calculation rule.
Select stats with these strings or enum members. Names are case-sensitive.

| String | Python enum | Rust enum | Unit |
| --- | --- | --- | --- |
| `clip_size` | `HeroStat.CLIP_SIZE` | `HeroStat::ClipSize` | `rounds` |
| `bullet_velocity` | `HeroStat.BULLET_VELOCITY` | `HeroStat::BulletVelocity` | `m/s` |
| `weapon_damage` | `HeroStat.WEAPON_DAMAGE` | `HeroStat::WeaponDamage` | `%` |
| `fire_rate` | `HeroStat.FIRE_RATE` | `HeroStat::FireRate` | `%` |
| `reload_time` | `HeroStat.RELOAD_TIME` | `HeroStat::ReloadTime` | `s` |
| `falloff_start` | `HeroStat.FALLOFF_START` | `HeroStat::FalloffStart` | `m` |
| `falloff_end` | `HeroStat.FALLOFF_END` | `HeroStat::FalloffEnd` | `m` |
| `light_melee_damage` | `HeroStat.LIGHT_MELEE_DAMAGE` | `HeroStat::LightMeleeDamage` | `damage` |
| `heavy_melee_damage` | `HeroStat.HEAVY_MELEE_DAMAGE` | `HeroStat::HeavyMeleeDamage` | `damage` |
| `melee_distance` | `HeroStat.MELEE_DISTANCE` | `HeroStat::MeleeDistance` | `%` |
| `move_speed` | `HeroStat.MOVE_SPEED` | `HeroStat::MoveSpeed` | `m/s` |
| `sprint_speed` | `HeroStat.SPRINT_SPEED` | `HeroStat::SprintSpeed` | `m/s` |
| `slide_distance` | `HeroStat.SLIDE_DISTANCE` | `HeroStat::SlideDistance` | `%` |
| `gravity_scale` | `HeroStat.GRAVITY_SCALE` | `HeroStat::GravityScale` | `multiplier` |
| `stamina` | `HeroStat.STAMINA` | `HeroStat::Stamina` | `points` |
| `stamina_cooldown` | `HeroStat.STAMINA_COOLDOWN` | `HeroStat::StaminaCooldown` | `s` |
| `dash_speed` | `HeroStat.DASH_SPEED` | `HeroStat::DashSpeed` | `m/s` |
| `dash_duration` | `HeroStat.DASH_DURATION` | `HeroStat::DashDuration` | `s` |
| `air_dash_speed` | `HeroStat.AIR_DASH_SPEED` | `HeroStat::AirDashSpeed` | `m/s` |
| `air_dash_duration` | `HeroStat.AIR_DASH_DURATION` | `HeroStat::AirDashDuration` | `s` |
| `bullet_evasion` | `HeroStat.BULLET_EVASION` | `HeroStat::BulletEvasion` | `%` |
| `debuff_resist` | `HeroStat.DEBUFF_RESIST` | `HeroStat::DebuffResist` | `%` |
| `bullet_resist` | `HeroStat.BULLET_RESIST` | `HeroStat::BulletResist` | `%` |
| `spirit_resist` | `HeroStat.SPIRIT_RESIST` | `HeroStat::SpiritResist` | `%` |
| `melee_resist` | `HeroStat.MELEE_RESIST` | `HeroStat::MeleeResist` | `%` |
| `bullet_lifesteal` | `HeroStat.BULLET_LIFESTEAL` | `HeroStat::BulletLifesteal` | `%` |
| `spirit_lifesteal` | `HeroStat.SPIRIT_LIFESTEAL` | `HeroStat::SpiritLifesteal` | `%` |
| `melee_lifesteal` | `HeroStat.MELEE_LIFESTEAL` | `HeroStat::MeleeLifesteal` | `%` |


`HeroStat.AMMO` is an alias for `HeroStat.CLIP_SIZE`. The string `"ammo"` is not accepted.
Without `stats`, the query selects capacity. Use `stats=list(HeroStat)` to select all supported stats.
Percentage values use percentage points: `20` means 20%.
Capacity stays finite during unlimited ammo. Use [ammo snapshots](examples.md#remaining-rounds-and-unlimited-ammo) for remaining rounds.

## Select the game data

```bash
boon versions
boon get GAME_VERSION
```

Set `data_version` to the replay's Deadlock client version.
Boon does integrity checks of installed files or downloads that exact version.
Boon does not select the version from the demo header.
Catalogs must include `record_key`, `definition_path`, and `stat_changes`.
After a catalog release changes, use `boon get VERSION --force` to replace local files.

## Python

```python
from boon import Demo, HeroStat, StatMode

demo = Demo("108575009.dem", preload=False)
result = demo.calculate_hero_stats(
    ticks=[187554, 187600],
    steam_ids=[76561198037652386],  # McGinnis in this demo.
    data_version="6712",
    stats=[HeroStat.CLIP_SIZE, HeroStat.FIRE_RATE],
    mode=StatMode.CURRENT,
    explain=True,
    strict=False,
)
print(result.values)
print(result.contributions)
print(result.metadata)
```

Without `rulesets`, each stat uses its supported `v1` rule.
An explicit mapping must contain one supported rule for each requested stat:

```python
from boon import rulesets

selected_rules = {
    HeroStat.CLIP_SIZE: rulesets.clip_size.v1,
    HeroStat.FIRE_RATE: rulesets.fire_rate.v1,
}
# Use rulesets=selected_rules with these two stats.
```

Each rule has a name, version, and documentation date. The date is not a game patch date.

## Select baseline or current effects

The two hero and ability queries accept these modes:

| String | Python enum | Rust enum | Inputs |
| --- | --- | --- | --- |
| `current` | `StatMode.CURRENT` | `StatMode::Current` | Supported active effects, including temporary buffs and debuffs. Default. |
| `baseline` | `StatMode.BASELINE` | `StatMode::Baseline` | Hero values, owned passive effects, and permanent changes. |

The two modes use the selected tick and keep permanent penalties.
They also apply to dependent inputs, such as spirit power for ammo.
Items sold before that tick do not contribute.
Unknown effect roles give partial baseline values with diagnostics.
An untimed modifier does not prove a passive effect.

Movement values do not simulate firing, crouching, bullet-hit slows, or sprint acceleration.
`gravity_scale` has only a recorded current value.
A baseline gravity query raises an error, or returns null with `strict=False`.

## Select ticks and players

`ticks` accepts an integer or a sequence of integers from 0 through 2147483646.
Boon removes duplicate ticks and reads state after each tick.
Missing ticks cause an error. Multiple ticks share a parser pass.
Use the tick passed to `demo_gototick`; the viewer's pause message can show a different server tick.

Use `steam_ids` for Steam accounts or `heroes` for hero IDs at each tick.
Get Steam IDs from `demo.players`. The two filters apply when set.
Without filters, the query selects all players with a hero. An empty filter selects no players.
A requested Steam ID without a selected hero causes an error at that tick.

## Results

| `result.values` column | Contents |
| --- | --- |
| `mode` | `current` or `baseline` |
| `tick` | Demo tick (Int32) |
| `steam_id` | Steam ID (UInt64), or null |
| `hero_id` | Hero ID at this tick (Int64) |
| `stat` | Stat string |
| `value` | Result (Float64), or null |
| `unit` | Unit from the stat table |
| `ruleset` | Equation ID, such as `clip_size.v1` |
| `status` | `calculated`, `partial`, or `unresolved` |
| `diagnostic` | Missing inputs or assumed links; null when none apply |

`explain=True` adds source rows to `result.contributions`.
Rows identify inputs, catalog paths, values, and modifier serials.
Intermediate inputs, such as `spirit_power`, are not extra stat bonuses.
Purchase-cost rows use souls, not percentage points.
`metadata` records the mode, selected client version, source commit, catalog snapshot, and rules.

Join player results with `steam_id`. Include `tick` for sampled rows and `stat` for stat rows.
Do not join null Steam IDs as one player.

## Shared inputs

Hero bases, growth, scaling, item prices, and effect values come from boon-data.
Bound properties contribute once through their effective modifier.
Boon uses source-ability upgrades; recipient upgrades do not replace missing caster state.
Recorded stat totals count once. Pickup amounts do not multiply those totals.

Shop bonuses use item costs and the highest reached `m_MapModCostBonuses` threshold.
Prices come from `misc.json` and `generic_data.m_nItemPricePerTier`.
The old tier table applies only when the cost table is missing.
Catalog counter bindings can use recorded ability fields, such as Trophy Collector's `m_iTrophyCount`.
Missing counts are not zero.

Modifier state follows packet changes from the start; relay keyframes can contain future modifier state.
Timers use recorded game time and do not include pauses.
An intrinsic effect ends after a recorded deletion of its observed ability handle.
Missing entities do not prove expiry. Other effects can continue.
State masks can end effects, but shared or missing states leave some expiry times unknown.
See [Known Issues](known-issues.md#stat-calculations).

Global spirit inputs use:

```text
(base + sum(flat bonuses)) * product(1 + each percentage bonus / 100)
    + sum(post-multiplier flat bonuses)
```

Ability-only spirit does not enter the global total.
The catalog supplies calculation stages and activation links.
For Ice Path, the caster's flat spirit bonus applies after percentage multipliers.
Spirit Snatch uses different caster and victim bindings with normalized recorded counts.
These supplied links stay in diagnostics.
Active property scaling functions still have [limits](known-issues.md#spirit-power-and-modifier-bindings).

## Weapon damage

```text
weapon_damage = sum(weapon damage percentage bonuses)
```

The result is a global bonus percentage, before rounding.
Inputs include shop bonuses, boon growth, recorded totals, and mapped effects.
Counter bindings can add earned rewards, such as Bloodscent's kills and assists.
Markers on another player do not transfer the owner's reward.

The result does not include flat bullet damage, critical hits, falloff, and target-specific bonuses.
See [weapon limits](known-issues.md#weapon-damage-coverage).

## Ammo equation

```text
ceil((base ammo + flat bonuses) * (1 + sum(percent bonuses) / 100))
```

For base 20, flat bonus 10, and percentage bonuses 15 and 4, capacity is 36 rounds.
Weapon values come from the hero's primary weapon record, including `m_mapWeaponInfos.primary`.
Hero spirit scaling comes from `m_mapScalingStats.EClipSize`.
Boon does not substitute an alternate weapon for a missing primary definition.

Permanent bonuses and penalties use recorded stat types.
Corrupted ability bonuses are unsupported and are not detected.
Powerup values use catalog ranges and match time at application.
See [ammo limits](known-issues.md#ammo-and-barrier-snapshots).

## Bullet velocity equation

```text
base speed in Source units/s * (1 + sum(percent bonuses) / 100) * 0.0254
```

Boon reads `m_flBulletSpeed` and `MODIFIER_VALUE_BONUS_BULLET_SPEED_PERCENT`.
Results use m/s. The conversion factor changes inches to meters.
For base 8000 and bonuses 60% and 25%, the result is 375.92 m/s.

The result describes the primary gun, without flight curves, beam behavior, or alternate fire.
A nonzero base-speed override stays unresolved because its priority rule is unknown.

## Falloff range

```text
multiplier = 1 + percent_bonus / 100
start_meters = base_start_source_units * 0.0254 * multiplier
end_meters = base_end_source_units * 0.0254 * multiplier
```

Bases use `m_flDamageFalloffStartRange` and `m_flDamageFalloffEndRange`.
Bonuses use `MODIFIER_VALUE_BONUS_ATTACK_RANGE_PERCENT`.
V1 supports at most one nonzero bonus. Multiple bonuses stay unresolved.
Missing, negative, or reversed base ranges also stay unresolved.

Start is where damage begins to decrease. End is where the falloff penalty reaches its maximum.
End is not maximum bullet travel. Ability range does not change these endpoints.
Compare the equal scaling model with viewer values, particularly for beams.

## Melee damage

```text
boon_growth = standard_boon_count * light_melee_gain_per_boon
base_with_growth = base_damage + boon_growth * (base_damage / base_light_damage)
spirit_bonus = spirit_power * hero_scaling_coefficient
final_damage = (base_with_growth + spirit_bonus)
             * (1 + (weapon_bonus_percent / 2 + melee_bonus_percent) / 100)
```

Bases use `ELightMeleeDamage` and `EHeavyMeleeDamage`.
Growth uses `m_mapLevelInfo` and `MODIFIER_VALUE_BASE_MELEE_DAMAGE_FROM_LEVEL`.
Spirit coefficients come from the matching `m_mapScalingStats` entries.
Heavy melee uses the catalog's heavy/light base ratio.
For light melee, that ratio is 1.

The weapon-damage inputs also apply at half strength.
Melee bonuses use `MODIFIER_VALUE_MELEE_DAMAGE_INCREASE`.
Results do not include target resistance, immunity, and effects that occur on a hit.
Nonzero melee or all-damage multipliers stay unresolved.

The equation follows player observations of [weapon scaling](https://forums.playdeadlock.com/threads/misleading-calculation-for-melee-damage-in-the-shop-stats-breakdown-the-actual-damage-has-no-bug.80970/)
and [boon growth](https://forums.playdeadlock.com/threads/characters-with-higher-base-light-melee-damage-apollo-bebop-calico-rem-have-decreased-heavy-melee-boon-scaling.126781/).
The engine equation and scaling order still have unverified assumptions.
Rust uses `rulesets::melee_damage::LIGHT_V1` and `HEAVY_V1`.

## Melee distance bonus

```text
sum(heavy-melee travel percentage bonuses)
```

Inputs use `MODIFIER_VALUE_MELEE_TRAVEL_DISTANCE_PERCENTAGE`.
The result is a percentage bonus, not meters or hitbox reach.
Travel also depends on the attack, movement curves, and collisions.

## Reload time

```text
base reload seconds * (1 + percent adjustment / 100)
```

Bases use `m_reloadDuration`; adjustments use `MODIFIER_VALUE_RELOAD_SPEED`.
A negative adjustment reduces time. For base 2 seconds and -10%, the result is 1.8 seconds.
V1 supports at most one nonzero adjustment.
Multiple adjustments, dynamic overrides, and hero reload scaling stay unresolved.

With `m_bReloadSingleBullets=true`, the result is seconds for each round.
It does not include the initial delay. Other weapons return a magazine's reload duration.
This value is not remaining reload time or instant ammo restoration.

## Fire-rate increase or decrease

```text
remaining = product(1 - each_slow_percent / 100)
modifier_percent = max(-50, sum(bonus_percent) - 100 * (1 - remaining))
```

Positive `MODIFIER_VALUE_FIRE_RATE` values add.
Negative values and `MODIFIER_VALUE_FIRE_RATE_SLOW` supply individual slow factors.
No effects gives 0. The result is the modifier percentage, not shots for each second.
For +18%, +20%, -20%, and -30%, the result is -6%.

A positive modifier multiplies base rate by `1 + modifier_percent / 100`.
A negative modifier divides base rate by `1 + abs(modifier_percent) / 100`.
Rust provides `rulesets::fire_rate::rate_multiplier` for this conversion.
Hero spirit scaling uses `m_mapScalingStats.EFireRate`.

Boon prefers explicit bindings. A unique timed modifier can activate an owner's unbound conditional property.
This assumed link gives partial values with a diagnostic, including Full Auto's bonus and upgrades.
Battle Vest's intrinsic modifier does not prove its health condition.
See [Battle Vest](known-issues.md#battle-vest-health-condition).

## Slide distance, bullet evasion, and gravity scale

### Slide distance

```text
slide_distance = 100 * (product(1 + each_bonus_percent / 100) - 1)
```

Inputs use `MODIFIER_VALUE_MOVEMENT_SLIDE_DISTANCE_SCALE`.
For +35% and +50%, the result is +102.5%.
The result does not describe travel in meters, friction, or slide-turn effects.

### Bullet evasion

Inputs use `MODIFIER_VALUE_BULLET_EVASION`.
V1 supports one nonzero chance from 0% through 100%.
Multiple chances stay unresolved.

The exact `EvasionPercent` property can use an owner's unique timed effect modifier.
That activation link is an assumption and gives partial values.
Other unbound evasion properties stay unsupported.

### Gravity scale

The result is the pawn's recorded `m_flGravityScale` multiplier.
Boon does not add modifier adjustments or assume 1 for a missing field.
Only current mode has this value; it can lack effective gravity changes.

## Stamina and ordinary dashes

### Stamina

```text
capacity = base EStamina + sum(MODIFIER_VALUE_STAMINA)
flat recovery:    seconds = 1 / (base_rate + flat_rate)
percent recovery: seconds = 1 / (base_rate * (1 + percent / 100))
```

`stamina` is capacity, not remaining resource. Free dashes do not make capacity infinite.
`stamina_cooldown` is seconds to recover one point, not remaining refill time.
Its base is `EStaminaRegenPerSecond`.
Rate effects use `MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_ADDITIVE` or `MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_PERCENTAGE`.

V1 supports flat recovery changes or one nonzero percentage change.
Mixed adjustments, multiple percentages, paused recovery, and nonpositive rates stay unresolved.
A +25% recovery bonus changes a five-second refill to four seconds.

### Dash speed and duration

| Movement | Base distance | Base duration |
| --- | --- | --- |
| Ground | `EGroundDashDistanceInMeters` | `EGroundDashDuration` |
| Air | `EAirDashDistanceInMeters` | `EAirDashDuration` |

```text
speed = base_distance * (1 + distance_percent / 100) / duration
```

Speed is a nominal average. Duration stays at the catalog value.
Ground distance uses `MODIFIER_VALUE_MOVEMENT_GROUND_DASH_INCREASE_PERCENT` and `MODIFIER_VALUE_MOVEMENT_GROUND_DASH_REDUCTION_PERCENT`.
Air distance uses `MODIFIER_VALUE_AIR_MOVE_DISTANCE_INCREASE_PERCENT`.
Reductions have negative values.

V1 supports one nonzero distance adjustment for each movement type.
Multiple adjustments make speed unresolved but do not prevent a duration result.
Flight, movement curves, interrupts, and ability-specific dashes are outside this model.
These stats use only explicit effect bindings.

## Move speed and sprint speed

### Equations

```text
move_speed = (base_move_speed + sum(flat_move_adjustments)) * (1 + move_percent / 100)
sprint_speed = base_sprint_speed + sum(sprint_bonuses)
full_sprint_speed = move_speed + sprint_speed
```

Flat movement bonuses and penalties add. Sprint bonuses also add.
For base movement 6.4 m/s and bonuses +2 and +3, movement is 11.4 m/s.
For base sprint 1.6 m/s and bonuses +2 and +1.5, the additional sprint component is 5.1 m/s.
Their sum is 16.5 m/s, not measured velocity.

V1 supports at most one nonzero movement percentage.
That percentage affects movement, including flat adjustments, but not the sprint component.
Multiple percentages or negative final speeds stay unresolved.
Values do not simulate movement states; [state queries](player-states.md) report those states in another query.

### Catalog inputs and coverage

Bases use `EMaxMoveSpeed` and `ESprintSpeed`, with catalog hero spirit scaling.
Flat effects use `MODIFIER_VALUE_MOVEMENT_SPEED_MAX` and `MODIFIER_VALUE_SPRINT_SPEED_BONUS`.
Percentages use `MODIFIER_VALUE_MOVEMENT_SPEED_MAX_PERCENT`.
Bare numeric modifier amounts use Source units/s; metric properties use m/s.
Boon converts upgrade amounts to the same units.

Trophy Collector uses a supplied catalog binding to `m_iTrophyCount`.
Missing bindings, intrinsic modifiers, counts, or scaling can prevent a complete value.
See [movement limits](known-issues.md#move-and-sprint-speed-coverage).

## Debuff resistance

```text
debuff_resist = 100 * (1 - product(1 - source_percent / 100))
affected_duration = original_duration * (1 - debuff_resist / 100)
```

The base uses `EDebuffResist`; effects use `MODIFIER_VALUE_STATUS_RESISTANCE`.
A missing base contributes zero. Negative resistance increases duration.
Two 25% sources give 43.75%. A 30% result reduces ten seconds to seven seconds.
The result does not identify immunity, cleansing, or debuffs that ignore resistance.

## Damage resistance

```text
innate = base + boon growth + spirit scaling
resistance = 100 * (1 - (1 - innate / 100) * product(1 - other_source / 100))
reduction = 100 * (1 - product(1 - reduction_source / 100))
result = resistance - reduction
```

| Stat | Hero base | Resistance modifier | Reduction modifier |
| --- | --- | --- | --- |
| `bullet_resist` | `EBulletArmorDamageReduction` | `MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST` | `MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION` |
| `spirit_resist` | `ETechArmorDamageReduction` | `MODIFIER_VALUE_TECH_RESIST` | `MODIFIER_VALUE_TECH_RESIST_REDUCTION` |
| `melee_resist` | `EMeleeResist` | `MODIFIER_VALUE_MELEE_RESIST` | `MODIFIER_VALUE_MELEE_RESIST_REDUCTION` |

For resistance sources 40% and 20%, the result before reductions is 52%.
Reductions of 25% and 20% combine to 40%, which leaves 12% resistance.
A missing base is zero. Negative final values stay negative.
Reductions apply to the recipient. Property upgrades use the caster's state.
Catalog reductions use negative values; unsupported positive reductions stay unresolved.
`melee_resist` reports only the melee component. Shared bullet/melee resistance stays in `bullet_resist`.

## Lifesteal

```text
lifesteal = 100 * (1 - product(1 - source_percent / 100))
```

Each damage type has its own calculation. Source percentages must be finite and between 0 and 100.
For 22% and 30%, the result is 45.4%.
Bullet bases use `EBulletLifesteal`; spirit bases use `ETechLifesteal`.
Effects use `MODIFIER_VALUE_BULLET_LIFESTEAL` and `MODIFIER_VALUE_TECH_LIFESTEAL`.
A missing base is zero.

Melee uses the exact `MeleeLifesteal` property on an owned passive ability, with its upgrades.
This assumed property link gives a partial value.
Target-dependent `TargetLifesteal` stays unresolved.
Cooldown-based melee healing items are not included.
Results do not include healing adjustments and creep effectiveness; they do not predict health gained.

## Unresolved inputs

| Status | Meaning |
| --- | --- |
| `calculated` | The rule calculated a value from supported inputs. |
| `partial` | The value has missing effects or an assumed link. |
| `unresolved` | The value is null because the rule cannot calculate it. |

Unknown modifiers and assumed activation links give partial values with either `strict` setting.
Other missing inputs raise `CalculationError` with `strict=True`.
With `strict=False`, those rows have null values and diagnostics.
Invalid queries, missing ticks, and download failures still cause errors.
Even a calculated value cannot include effects missing from the catalog.

Boon ignores the two known ping markers for stat calculations. They stay in modifier datasets.
See [Known Issues](known-issues.md) for binding, scaling, and lifetime limits.

## Rust

Rust queries use `HeroStat` enum members and `u64` Steam IDs.

```rust
use boon::{
    Parser,
    hero_stats::{HeroStat, HeroStatQuery, Ruleset, StatCatalog, StatMode},
    rulesets,
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = Parser::from_file(Path::new("108575009.dem"))?;
    let catalog = StatCatalog::load("6712")?;
    let query = HeroStatQuery::new(
        [187554, 187600],
        [HeroStat::ClipSize, HeroStat::FireRate],
    )
    .steam_ids([76561198037652386])
    .mode(StatMode::Current)
    .explain(true)
    .strict(false);
    let rules = Ruleset::new()
        .with(rulesets::clip_size::V1)
        .with(rulesets::fire_rate::V1);
    let result = parser.calculate_hero_stats(&query, &catalog, &rules)?;
    println!("{:?}", result.values);
    Ok(())
}
```

Reuse `StatCatalog` across queries. `StatCatalog::load` uses the verified cache
from `boon get`. `StatCatalog::from_directory` reads local files without downloads
or checksum checks.
