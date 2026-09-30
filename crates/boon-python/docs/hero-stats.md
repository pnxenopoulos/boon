# Calculate hero stats

`demo.calculate_hero_stats()` calculates values from replay state and a selected
boon-data version. Supported stats are:

| Stat | Unit |
| --- | --- |
| Ammo capacity (`clip_size`) | rounds |
| Bullet velocity (`bullet_velocity`) | metres per second |
| Weapon-damage bonus (`weapon_damage`) | percentage points |
| Heavy-melee distance bonus (`melee_distance`) | percent |
| Reload time (`reload_time`) | seconds |
| Fire-rate increase or decrease (`fire_rate`) | percent |
| Falloff start and end (`falloff_start`, `falloff_end`) | metres |
| Light and heavy melee damage (`light_melee_damage`, `heavy_melee_damage`) | damage |
| Slide-distance bonus (`slide_distance`) | percent |
| Bullet evasion (`bullet_evasion`) | percent |
| Recorded gravity scale (`gravity_scale`) | multiplier |
| Maximum stamina (`stamina`) | points |
| Recovery time per stamina point (`stamina_cooldown`) | seconds |
| Ground and air dash speed (`dash_speed`, `air_dash_speed`) | metres per second |
| Ground and air dash duration (`dash_duration`, `air_dash_duration`) | seconds |
| Nominal move speed (`move_speed`) | metres per second |
| Additional sprint speed (`sprint_speed`) | metres per second |
| Debuff resistance (`debuff_resist`) | percentage points |
| Bullet, spirit, and melee resistance (`bullet_resist`, `spirit_resist`, `melee_resist`) | percentage points |
| Bullet, spirit, and melee lifesteal (`bullet_lifesteal`, `spirit_lifesteal`, `melee_lifesteal`) | percentage points |

Ammo capacity does not return rounds left in the gun. It stays finite during a
slide or another unlimited-ammo effect.

Use the tick passed to `demo_gototick` for `ticks`. The pause message can show a
server tick instead. In `106996573.dem`, command tick 49000 corresponds to server
tick 50706. Do not use the pause message's number as the query tick.

Modifier calculations replay packet changes from the start. Relay keyframes can
contain future modifier state. A multi-tick query shares this replay pass.
Intrinsic effects end when the owning ability entity is removed. Other effects
can continue after their source ability is removed.

## Select the game data

```bash
boon versions
boon get GAME_VERSION
```

Set `data_version` to a Deadlock client version from this list.
Boon verifies the local files or downloads that exact version.
Boon does not select a client version from the demo header.

The catalogs must contain `record_key`, `definition_path`, and `stat_changes`.
Old catalogs can supply names but lack stat inputs.
Use `boon get VERSION --force` after new files are published for that version.

## Python

Use `HeroStat` enum members to select hero stats. An enum gives each stat a named
constant, such as `HeroStat.CLIP_SIZE`. Python also accepts the matching string
values and rejects unknown names. The `stat` column contains strings.
Use `AbilityStat` with [ability stat queries](ability-stats.md).

```python
from boon import Demo, HeroStat, StatMode

version = "6694"  # Select the client version for your demo.
demo = Demo("106996573.dem", preload=False)
result = demo.calculate_hero_stats(
    ticks=[50707, 50800],
    steam_ids=[76561197999389679],  # Venator in this demo.
    data_version=version,
    stats=[HeroStat.CLIP_SIZE, HeroStat.FIRE_RATE],
    mode=StatMode.CURRENT,  # Default; use BASELINE for passive and permanent inputs.
    explain=True,
    strict=False,
)
print(result.values)
print(result.contributions)
print(result.metadata)
```

`HeroStat.AMMO` and `HeroStat.CLIP_SIZE` select the same stat.
If you omit `stats`, Boon selects ammo capacity. Use `stats=list(HeroStat)` to
select all supported hero stats.

If you omit `rulesets`, Boon selects the supported `v1` rule for each stat.
To select rules, supply one rule per requested stat:

```python
from boon import rulesets

selected_rules = {
    HeroStat.CLIP_SIZE: rulesets.clip_size.v1,
    HeroStat.FIRE_RATE: rulesets.fire_rate.v1,
}
# Pass rulesets=selected_rules with these two stats.
```

## Select baseline or current effects

Use `mode="current"` (default) to include supported active buffs, debuffs,
powerups, and conditional effects. Use `mode="baseline"` for hero values, boons,
owned passive effects, and permanent recorded changes. `StatMode.CURRENT` and
`StatMode.BASELINE` also work.

Both modes use the requested tick. Items sold before that tick do not contribute.
Permanent penalties remain in both modes. The mode also applies to dependent
inputs, such as spirit power used for ammo scaling. Each mode runs the equation
from its selected inputs. Do not subtract temporary bonuses from the final value.

Catalog roles identify passive effects. An untimed modifier is not automatically
passive. Unknown roles are excluded from baseline and reported as partial values.
Missing intrinsic modifiers also retain their diagnostics.

The mode does not change units. For example, ammo is capacity and fire rate is a
bonus percentage. To compare the two results, join on `tick`, `steam_id`, and
`stat`. Keep the `mode` column when you combine reports.

The existing rules still apply. Movement values do not simulate firing, crouching,
slows, speed limits, or sprint acceleration. `gravity_scale` has only a recorded
current value. Baseline gravity scale raises an error; with `strict=False`, its
value is null and its status is `unresolved`.

## Select ticks and players

`ticks` accepts one integer or a list. Boon removes duplicate ticks and reads
state after each tick. Missing ticks cause an error.
The selected stats use the same player inputs in one parser pass.

Use `steam_ids` to select Steam accounts. Get IDs from `demo.players`.
Use `heroes` to select the hero played at each tick. Both filters apply when set.
Omit both filters to include all players with a hero.
An empty filter selects no players. A requested Steam ID without a selected hero
causes an error at that tick. The old `players` slot filter is not supported.

## Results

`result.values` has these columns:

| Column | Contents |
| --- | --- |
| `mode` | `current` or `baseline` (String) |
| `tick` | Requested demo tick (Int32) |
| `steam_id` | Steam account ID (UInt64), or null if missing |
| `hero_id` | Hero at this tick (Int64) |
| `stat` | Stat name, such as `clip_size` (String) |
| `value` | Calculated value (Float64), or null |
| `unit` | `rounds`, `m/s`, `%`, `s`, `m`, `points`, `damage`, or `multiplier` |
| `ruleset` | Equation ID, such as `clip_size.v1` |
| `status` | `calculated`, `partial`, or `unresolved` |
| `diagnostic` | Missing inputs or assumed links; null when none are reported |

`explain=True` adds input rows to `result.contributions`.
Each row gives the mode, input, value, source, property path, and modifier serial when present.
Intermediate rows, such as `input="spirit_power"`, explain other inputs.
Spirit rows separate flat and percentage bonuses. Ability-only bonuses do not
enter global spirit. Percentage spirit calculations remain unsupported, even if
opposing sources add to zero. Do not add intermediate rows to the final stat.

Use `steam_id` to join results to `demo.players`. Use `tick` and `steam_id` to
join state rows to stat rows. A Steam ID stays constant through hero changes.
Rows without a Steam ID retain a null ID. Do not join null Steam IDs.

`metadata` records the mode, client version, catalog snapshot, source commit, and rules.
Each rule has a name, version, and documentation date. The date is not a game
patch date. Catalog updates change inputs; rule versions identify equations.

## Weapon damage

`weapon_damage.v1` returns the global weapon-damage bonus in percentage points.
A value of `45` means +45%; no bonuses gives `0`. Signed bonuses add together.
The result is not rounded.

```text
weapon_damage = sum(weapon damage percentage bonuses)
```

Boon reads `MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE` from effective modifiers and
recorded permanent stat totals. Shop bonuses use each category's total item cost
and the highest reached threshold in `m_MapModCostBonuses`. Item prices come from
`misc.json` → `generic_data.m_nItemPricePerTier`. Boon uses the old tier table only
when the hero has no cost-based table. Missing prices produce an unresolved value.
If the old table is absent, the calculation rules link the weapon category to
weapon damage and the spirit category to spirit power. The catalog supplies all
prices, thresholds, and bonus values.
Catalog-defined weapon-percentage boon growth also applies. No balance amounts
are fixed in code.
Contribution rows with `kind="purchase_cost"` show item prices in souls. Do not
add these prices to percentage bonuses.
Bound properties count through their modifier once. Recorded totals are already
accumulated; Boon does not multiply them by a pickup value.

This stat is separate from damage per projectile. Base bullet damage, its boon
and spirit growth, and flat post-scale damage belong to the bullet-damage equation.
Critical hits, falloff, target resistance, and target- or range-specific bonuses
are outside this global percentage.

```python
result = demo.calculate_hero_stats(
    ticks=50707,
    data_version="6694",  # Select the catalog version for your replay.
    stats=[HeroStat.WEAPON_DAMAGE],
    explain=True,
    strict=False,
)
print(result.values)
print(result.contributions)
```

The Rust query selects `HeroStat::WeaponDamage`. Its rule is
`boon::rulesets::weapon_damage::V1`; Python exposes `rulesets.weapon_damage.v1`.

A catalog `runtime_counts` binding can link an owned ability property to recorded
ability counters. Each term names a field and can include a catalog percentage
property as its weight. Boon sums the weighted counts and multiplies the result
by the effect value, with ability upgrades. Missing, negative, or fractional
counts are errors; a missing field is not zero. The trace includes each count
and percentage weight.

New boon-data catalogs use this for Bloodscent's earned kills and assists.
The property-to-counter relationship is curated, as with Trophy Collector;
it is not an explicit VData registration. Boon contains no Bloodscent ID, counter
name, damage amount, or assist weight. Existing catalogs without this binding
still return a partial Bloodscent result.

Unbound owner rewards do not transfer through markers on other players.
Explicit effects on those modifiers still apply. A property with the explicit
`#EnemyAboveHealthThreshold_conditional` label is excluded from this global stat.
Boon does not evaluate that enemy health condition or identify the item by name.

An unbound property can describe a per-kill reward or conditional bonus, even
when its usage flag says `IntrinsicallyProvidedInAbility`. Boon does not apply
such a value once from ownership. It returns the known subtotal as `partial`
and names the missing input. Unsupported bound scaling or stack rules still
produce an error, or an `unresolved` row with `strict=False`.
See [known weapon-damage limits](known-issues.md#weapon-damage-coverage).

## Ammo equation

```text
ceil((base ammo + flat bonuses) * (1 + sum(percent bonuses) / 100))
```

Percent inputs use percentage points: `19` means +19%. For base ammo 20,
flat ammo 10, and bonuses of 15% and 4%, the result is 36 rounds.

Boon uses the hero's primary weapon reference to read its ability record.
Weapon fields come from `m_mapWeaponInfos.primary`, or `m_WeaponInfo` in older
catalogs. Contribution paths show the source used. Boon does not select an
alternate weapon when the primary definition is absent. It resolves
owned item and ability properties, upgrades, and effective modifier instances.
It counts bound properties through their modifier and does not add them again
from the item. Expired modifiers and modifiers outside their aura do not apply.
Modifier timers use pawn simulation time minus accumulated pause time. If pawn
simulation time is absent, Boon uses `m_nTickBase` from player controllers with
hero pawns. It multiplies ticks by the replay's tick interval, then subtracts
accumulated pause time. Spectator controllers do not supply this clock.

Permanent bonuses and corruption penalties come from the replay's stat-viewer
vector. Boon maps each recorded `m_eValType` through the selected catalog's
`modifier_value_types`. One modifier can supply several stats. Each recorded
value counts once; Boon does not multiply it by a catalog pickup amount.
If enum data is absent, the source must identify one stat.

Corrupted property bonuses are not yet applied. Affected values are partial
subtotals with a diagnostic. Recorded penalties still apply.

Hero ammo scaling comes from `m_mapScalingStats.EClipSize`. The supported spirit
input includes catalog base values, standard level upgrades, shop
bonuses, recorded permanent bonuses, and resolved item/modifier contributions.
No hero IDs, base ammo values, pickup amounts, or scaling coefficients are
embedded in the calculator.

Time-ranged powerup values use their catalog minimum, maximum, and time bounds.
The current resolver interpolates using match minutes at application. Treat
these calculated values as a model to verify against the demo viewer, not as
networked final stats.

## Bullet velocity equation

```text
base speed in Source units/s * (1 + sum(percent bonuses) / 100) * 0.0254
```

`bullet_velocity.v1` adds percentage bonuses and returns metres per second
without rounding. One Source distance unit is one inch; `0.0254` converts inches
to metres. This conversion is part of the rule, not a hero or item balance value.

Boon reads `m_flBulletSpeed` from the primary weapon block and
`MODIFIER_VALUE_BONUS_BULLET_SPEED_PERCENT` from catalog effects. Item ownership,
active modifiers, and ability upgrades use the same resolver as ammo. Bound
properties count only through active modifiers. Ability-projectile speed is a
separate stat and does not count as bullet velocity.

In contribution rows, `input="bullet_velocity"` and `kind="base"` use m/s;
`kind="percent"` uses percentage points. For example, 8000 Source units/s with
60% and 25% bonuses gives 375.92 m/s.

This is nominal primary-weapon speed. It does not simulate projectile flight,
beam travel, speed curves, random variation, or alternate fire. Nonzero
`MODIFIER_VALUE_BASE_BULLET_SPEED_OVERRIDE` effects produce an unresolved result
because their priority rules are not yet supported. Conditional effects use
explicit bindings or the ownership inference described below. Other activation
conditions remain unresolved.

## Falloff range

```text
multiplier = 1 + percent_bonus / 100
start_metres = base_start_source_units * 0.0254 * multiplier
end_metres = base_end_source_units * 0.0254 * multiplier
```

`falloff_start.v1` and `falloff_end.v1` use the same equation. Start is the distance
where weapon damage begins to decrease. End is the distance where the falloff
penalty reaches its maximum. End is not maximum bullet travel distance.
For example, 20-50 metres with a +20% bonus becomes 24-60 metres. Results are not rounded.

Boon reads `m_flDamageFalloffStartRange` and `m_flDamageFalloffEndRange`
from the primary weapon block in
boon-data. Range bonuses use `MODIFIER_VALUE_BONUS_ATTACK_RANGE_PERCENT` and the
shared item, modifier, and upgrade resolver. Bound bonuses count once, only while
their modifier is effective. No hero distances or item bonus values are stored in code.

No final player falloff endpoints were found in the serializers of the inspected
replay, `106996573.dem`. These results are calculated from the catalog and replay
state. The equal scaling of both endpoints is a model to verify in the demo viewer;
VData does not supply the engine equation. V1 accepts no bonus or one nonzero bonus.
Multiple nonzero bonuses remain unresolved until their stacking rule is verified.

Missing, negative, or reversed base distances remain unresolved. Boon does not
use a `-1` sentinel as a distance. These stats describe the nominal primary
weapon. Beam weapons can use engine-specific range behavior; their catalog
endpoints still need in-game verification. These stats do not calculate alternate
weapon modes, damage along the falloff curve, or changes to the curve's bias and
damage scales. They do not use ability
range, maximum bullet travel, or an item's distance condition for extra damage.

Contribution rows use `input="falloff_start"` or `input="falloff_end"`. Base rows
use metres; percentage rows use percentage points. In Rust, select
`rulesets::falloff_range::START_V1` and `rulesets::falloff_range::END_V1`.

## Melee damage

`light_melee_damage.v1` and `heavy_melee_damage.v1` return nominal damage before
resistance and effects that occur on a hit. Results are not rounded.

```text
boon_growth = standard_boon_count * light_melee_gain_per_boon
base_with_growth = base_damage + boon_growth * (base_damage / base_light_damage)
spirit_bonus = spirit_power * hero_scaling_coefficient
final_damage = (base_with_growth + spirit_bonus)
             * (1 + (weapon_bonus_percent / 2 + melee_bonus_percent) / 100)
```

For light melee, `base_damage / base_light_damage` is 1. For heavy melee,
Boon uses the ratio of the two catalog base values. It does not use a fixed
heavy-melee multiplier. Spirit scaling is zero when the hero has no scaling
entry for that attack.

The inputs come from these catalog fields:

- `m_mapStartingStats.ELightMeleeDamage` and `EHeavyMeleeDamage`: base damage.
- `m_mapLevelInfo`: the levels that grant a standard boon, up to the replay level.
- `m_mapStandardLevelUpUpgrades.MODIFIER_VALUE_BASE_MELEE_DAMAGE_FROM_LEVEL`:
  light-melee damage per boon.
- `m_mapScalingStats.ELightMeleeDamage` or `EHeavyMeleeDamage`: spirit coefficient.
- `m_MapModCostBonuses`: shop bonus at each cost threshold.
- `misc.json` → `generic_data.m_nItemPricePerTier`: item prices.
- `MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE` and `MODIFIER_VALUE_MELEE_DAMAGE_INCREASE`:
  resolved item, ability, modifier, and permanent pickup bonuses.

Paige's heavy-melee spirit coefficient comes from her hero record. The same
code supports another hero with the same type of catalog scaling entry.
Bound properties count once, through their effective modifier.

The equation includes engine rules that VData does not encode. The half-strength
weapon bonus follows a [reported in-game calculation](https://forums.playdeadlock.com/threads/misleading-calculation-for-melee-damage-in-the-shop-stats-breakdown-the-actual-damage-has-no-bug.80970/).
The heavy/light boon ratio follows a [reported comparison of hero growth](https://forums.playdeadlock.com/threads/characters-with-higher-base-light-melee-damage-apollo-bebop-calico-rem-have-decreased-heavy-melee-boon-scaling.126781/).
These are player observations, not an engine specification. Check the model in
the demo viewer, including the order of spirit scaling and percentage bonuses.
No final melee-damage field was found in the inspected replay serializers.

Melee uses the same weapon-damage resolver as `weapon_damage`, including catalog
purchase bonuses, recorded stat totals, and owned ability counters. Bloodscent's
kill and assist bonus therefore contributes at half strength. Unbound rewards
from another player's target marker do not affect the result. Opening Rounds'
enemy-health bonus is outside the nominal stat; its ordinary bonus is included.

V1 excludes target resistance, effects triggered on a hit, and bonuses that
require a target's distance or type. Melee Charge's next-heavy-attack proc is
separate from its ordinary melee bonus. A target's Metal Skin immunity is also
outside this offensive stat. Boon does not add an item-name exception for it.

Missing global weapon inputs cause partial melee values. Examples
are Battle Vest's health condition and Intensifying Magazine's firing ramp.
Nonzero registered melee-damage or all-damage multipliers remain unresolved;
the rule does not guess their interaction. Unsupported scaling also remains
unresolved. Effects absent from the catalog can still be missing.
Use `strict=False` to see other players' values and the reasons for null rows.

Contribution rows use the requested melee stat as `input`. `boon_flat` is
light-melee growth before the heavy/light ratio; `base_light_reference` supplies
the denominator for heavy melee. `weapon_percent` is the full weapon bonus,
which the rule halves. Counter and counter-weight rows explain that weapon
input; do not add them as percentage bonuses. `percent` is a melee bonus, and
`flat` is resolved hero spirit scaling. Intermediate `spirit_power` rows are
not extra melee bonuses.
In Rust, select `rulesets::melee_damage::LIGHT_V1` and `HEAVY_V1`.

## Melee distance bonus

```text
sum(heavy-melee travel percentage bonuses)
```

`melee_distance.v1` returns a bonus in percentage points: `50` means +50%, and
`0` means no bonus. It does not return metres or melee hitbox reach. Actual
travel depends on the attack, movement curves, and collisions.

Boon reads `MODIFIER_VALUE_MELEE_TRAVEL_DISTANCE_PERCENTAGE` effects from
boon-data. Item values and modifier bindings come from the selected catalog.
The rule adds resolved bonuses without rounding or clamping. Bound item
properties count once, through their effective modifier. The rule uses no
hero-specific distances or item values from code.

Contribution rows use `input="melee_distance"` and `kind="percent"`. The same
strict mode and unresolved-input checks apply as for the other stats.

## Reload time

```text
base reload seconds * (1 + percent adjustment / 100)
```

`reload_time.v1` reads `m_reloadDuration` from the primary weapon block
and `MODIFIER_VALUE_RELOAD_SPEED` from catalog effects. A negative adjustment
reduces time: 2 seconds with -10% gives 1.8 seconds. The result is not rounded.
V1 supports no adjustment or one nonzero adjustment. The catalogs do not specify
how several adjustments combine. V1 reports such combinations as unresolved.

For weapons with `m_bReloadSingleBullets=true`, this is the time to load one
round. It excludes `m_flReloadSingleBulletsInitialDelay`; it does not multiply
by magazine capacity or rounds missing. For other weapons, it is the time to
reload the magazine. All base values and reload modes come from boon-data.

This is nominal duration at the selected tick. It does not predict remaining
reload time, interruptions, active-reload timing, or instant ammo refills.
A property that restores ammo on a hit or cast does not change this stat.
Dynamic weapon-duration overrides and hero reload scaling are not yet supported.

Contribution rows use `input="reload_time"`. Base rows use seconds; percentage
rows use signed percentage points. Bound effects count once and only while their
modifier is effective. The same partial-result and strict-mode rules apply.

## Fire-rate increase or decrease

```text
remaining = product(1 - each_slow_percent / 100)
modifier_percent = max(-50, sum(bonus_percent) - 100 * (1 - remaining))
```

`fire_rate.v1` returns the UI modifier in percentage points: `20` means +20%,
`-10` means -10%, and no effects give `0`. It does not return shots per second.
Values are not rounded to the UI's whole-number display.

An unbound ability property needs an intrinsic usage flag or a supported
activation link. Ownership alone does not apply a temporary bonus. Boon reports
unsupported nonzero properties in the diagnostic.

Positive `MODIFIER_VALUE_FIRE_RATE` values add together. Each
`MODIFIER_VALUE_FIRE_RATE_SLOW` value is a positive slow percentage and retains
its own factor. Negative `MODIFIER_VALUE_FIRE_RATE` values also count as slows
under this rule. For example, +18%, +20%, -20%, and -30% give -6%. Apply the -50%
minimum after combining both groups. Positive bonuses have no matching cap.

The increase or decrease is not always the change in shots per second. For a
positive modifier, multiply base fire rate by `1 + modifier_percent / 100`.
For a negative modifier, divide base fire rate by `1 + abs(modifier_percent) / 100`.
Thus -10% gives 90.9% of base fire rate, and -50% gives two thirds of base rate.
The Rust rule exposes this conversion as `rulesets::fire_rate::rate_multiplier`.

Hero spirit scaling comes from `m_mapScalingStats.EFireRate`. Boon multiplies
resolved spirit power by the catalog's `flScale` when `eScalingStat` is
`ETechPower`. No hero ID or coefficient is embedded in code. Permanent pickup
bonuses use recorded totals; gun powerups use their catalog range and application
time. The same item, ability-upgrade, modifier-expiry, and binding checks apply.

Before it applies modifier effects, Boon checks the state flags declared in the
catalog. It excludes a modifier when all its declared states are absent from the
pawn's recorded masks. Missing masks or state names do not establish inactivity.
A disabled state also prevents this exclusion. Raw modifier rows remain available.
Stat queries also check these masks throughout the replay. After an observed
present-to-absent transition, the same application stays ended. A new application
timestamp can restore it. Later stack changes or another cast's state flags cannot.
Shared states can still hide an expiry while another source supplies the same flag.

Boon uses explicit catalog property bindings first. If a conditional property
has no binding, Boon looks for one non-intrinsic modifier nested in its owning
ability. It assumes that this modifier activates the owner's conditional bonuses.
The modifier must have a finite positive duration when active. Multiple candidate
modifiers remain unresolved. The earlier stat resolvers use this rule. Slide
distance, stamina, recovery, and dash effects require explicit bindings.
Bullet evasion also uses the exact-property rule below. Gravity scale reads the pawn field directly.

This ownership rule is an assumption, not a confirmed engine rule. Affected
values have `status="partial"`, and `diagnostic` names the property, owning ability,
and modifier. When the modifier is absent, its contribution is zero, and the
result still reports the assumption. With `explain=True`, active contributions
include the modifier serial and property path. No ability names, IDs, or bonus
values are embedded in this rule. Upgrades come from the source ability and caster.

For example, Full Auto's temporary modifier now activates its catalog fire-rate
bonus, including upgrades. Battle Vest's intrinsic modifier does not establish
its health condition, so its unbound fire-rate bonus remains unresolved.
See [Known Issues](known-issues.md#battle-vest-health-condition) for the result
behavior and available data.

Contribution rows use `input="fire_rate"`. `kind="percent"` contains signed
fire-rate inputs; `kind="slow"` contains positive slow magnitudes. Intermediate
`spirit_power` rows explain hero scaling. Do not sum slow rows: their remaining
fractions multiply. Unknown nonzero ability-property scaling and scale upgrades
remain unresolved. A known linear scale with an explicit zero coefficient does
not require a spirit input.

## Slide distance, bullet evasion, and gravity scale

Slide distance and bullet evasion use catalog stat bindings, effective modifiers,
and ability upgrades. Ownership alone does not assign an ability's effects to a player.
An unbound property is included only if the catalog explicitly marks it as
`IntrinsicallyProvidedInAbility`. Other unbound contributions are omitted and
reported in the diagnostic. These rows have `status="partial"` in both strict
modes. A partial zero is the total of known inputs; it does not prove no effect.

### Slide distance

```text
slide_distance = 100 * (product(1 + each_bonus_percent / 100) - 1)
```

`slide_distance.v1` reads `MODIFIER_VALUE_MOVEMENT_SLIDE_DISTANCE_SCALE`.
It returns the bonus in percentage points: 35 means +35%, and 0 means no known
bonus. V1 multiplies the factors, without rounding: +35% and +50% give +102.5%.
It does not calculate travel in metres, slide speed, terrain effects,
or friction. `MODIFIER_VALUE_MOVEMENT_SLIDE_TURN_SCALE` is a separate stat.

Item amounts, modifier bindings, and upgrade values come from the selected
catalog. Bound effects count once, through the effective modifier. A buff
received from another hero can contribute if its binding is explicit.

### Bullet evasion

`bullet_evasion.v1` reads `MODIFIER_VALUE_BULLET_EVASION` and returns a chance
in percent: 30 means 30%, not 0.3. It supports no known effect or one nonzero
chance between 0% and 100%. Multiple nonzero chances remain unresolved until
their stacking rule is verified. A chance does not predict which bullets miss.

Boon prefers the catalog's stat declarations and modifier bindings. The effective
modifier must be present. Upgrade values come from the owning ability's catalog
record and the caster's recorded tiers.

In client version 6694, Mirage's Dust Devil binds `WhirlwindEvasionChance` to its
evasion modifier: 30% base plus 30 percentage points at tier 2. Grey Talon's Rain
of Arrows binds `EvasionPercent` to its in-air modifier: 0% base plus 30 points
at tier 3. These names and values are examples from the catalog, not code rules.

For an undeclared property named exactly `EvasionPercent`, V1 interprets its value
as bullet-evasion percent. This is an explicit property rule, not a substring
search. An existing stat declaration or binding takes priority.

When there is exactly one non-intrinsic effect modifier under the owning ability,
Boon assumes that this modifier activates the property. Cast-delay modifiers are
excluded. The property and its upgrades apply only while that effect modifier is
effective, including modifiers with no fixed duration. Such results have
`status="partial"` and a diagnostic that identifies the inferred link. The
contribution includes the modifier serial and the catalog property path.

This supports Bullet Dance in client version 6694: 30% while active, plus 40
percentage points at tier 3. No hero ID, ability ID, or balance amount is stored
in the resolver. If the effect modifier is absent, this contribution is zero.
If the owner has no unique effect modifier, Boon omits the unbound contribution
and reports the missing activation link.

Other undeclared properties remain unsupported. For example, V1 does not infer
a binding for Electric Slippers' `EvasionWhileSliding`. A zero result does not
prove that the player has no evasion. See [Known Issues](known-issues.md#movement-and-evasion-stat-coverage).

### Gravity scale

```text
gravity_scale = player_pawn.m_flGravityScale
```

`gravity_scale.v1` returns the recorded pawn multiplier unchanged: 1 means
normal gravity. It does not apply ability or modifier gravity adjustments.
The field alone need not describe the player's effective falling acceleration.
A missing or invalid pawn field produces an error; Boon does not assume 1.

Gravity scale does not require hero, weapon, or modifier stat definitions.
The query still takes an explicit boon-data version through the same API.

Contribution rows use `percent` for slide/evasion values. Gravity has one `base`
row with source `replay/player_pawn` and definition path `m_flGravityScale`.

## Stamina and ordinary dashes

```python
result = demo.calculate_hero_stats(
    ticks=[50707, 64500],
    data_version="6694",  # Select the catalog for your demo.
    stats=[
        HeroStat.STAMINA, HeroStat.STAMINA_COOLDOWN,
        HeroStat.DASH_SPEED, HeroStat.DASH_DURATION,
        HeroStat.AIR_DASH_SPEED, HeroStat.AIR_DASH_DURATION,
    ],
    explain=True,
    strict=False,
)
print(result.values)
```

Each stat has a `boon.rulesets.<stat>.v1` rule. Rust uses the same module names
with `V1`, for example `rulesets::stamina::V1` and `rulesets::dash_speed::V1`.
The rule date is 2026-09-28. It is a documentation date, not a game patch date.

### Stamina

`stamina` returns maximum capacity: base `EStamina` plus flat bonuses from
`MODIFIER_VALUE_STAMINA`. It does not return the current fractional resource.
Free air dashes do not make capacity infinite. Values are not rounded.

`stamina_cooldown` returns seconds to recover one point. The base rate is
`EStaminaRegenPerSecond`. V1 supports flat rate changes from
`MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_ADDITIVE`, or one nonzero percentage
from `MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_PERCENTAGE`:

```text
flat adjustment:       seconds = 1 / (base_rate + flat_rate)
percentage adjustment: seconds = 1 / (base_rate * (1 + percent / 100))
```

A +25% recovery bonus changes a 5-second refill to 4 seconds. It does not
subtract 25% from the time. This is not the remaining time on the current refill.
Multiple nonzero recovery percentages, or mixed flat and percentage adjustments,
remain unresolved until their combination rule is verified. Paused recovery and
nonpositive recovery rates also produce unresolved rows, not a finite refill time.

### Dash speed and duration

Base inputs come from `m_mapStartingStats` in the selected hero record:

| Movement | Distance | Duration |
| --- | --- | --- |
| Ground | `EGroundDashDistanceInMeters` | `EGroundDashDuration` |
| Air | `EAirDashDistanceInMeters` | `EAirDashDuration` |

```text
speed = base_distance * (1 + distance_percent / 100) / duration
```

The speed is the nominal average in m/s. It is not instantaneous speed during
the movement curve. The durations remain the catalog values. Distance bonuses
change speed in this model. Replay checks support unchanged ordinary air-dash
duration after a distance bonus; this is not a rule for every movement ability.

Ground dash uses `MODIFIER_VALUE_MOVEMENT_GROUND_DASH_INCREASE_PERCENT` and
`MODIFIER_VALUE_MOVEMENT_GROUND_DASH_REDUCTION_PERCENT`. Reductions are signed
negative values. Air dash uses `MODIFIER_VALUE_AIR_MOVE_DISTANCE_INCREASE_PERCENT`.
V1 supports one nonzero distance adjustment per movement type. It does not
assume how overlapping distance effects combine. These combinations make speed
unresolved; they do not prevent the nominal duration result.

Item amounts, upgrades, hero scaling, and time-dependent powerups come from the
catalog and replay. No hero bucket, item ID, or balance amount is stored in these
rules. Intrinsic properties apply to their owner. Bound effects apply through
an effective modifier and use the caster's upgrades. Unbound effects are omitted
with a partial diagnostic. An owned item's missing intrinsic modifier also produces
a partial diagnostic. Boon does not assume its value from ownership alone.
Known permanent pickup totals use the same resolver.

These stats describe ordinary dashes. They do not model flight, attack movement,
wall jumps, downward dashes, interruptions, terrain, or air-control acceleration.
For example, an ability's own dash range does not change ordinary dash speed
without a stat binding. See [Known Issues](known-issues.md#stamina-and-dash-stat-coverage).

Contribution rows record capacity as `base` and `flat`, recovery as
`base_per_second`, `flat_per_second`, and `percent`, and dash inputs as
`distance_metres`, `duration_seconds`, and `distance_percent`. Each row includes
the source path. Active effects also include their modifier serial.

## Move speed and sprint speed

```python
result = demo.calculate_hero_stats(
    ticks=50707,
    data_version="6694",  # Select the catalog for your demo.
    stats=[HeroStat.MOVE_SPEED, HeroStat.SPRINT_SPEED],
    explain=True,
    strict=False,
)
print(result.values)
print(result.contributions)
```

The Python rules are `rulesets.move_speed.v1` and `rulesets.sprint_speed.v1`.
Rust uses `HeroStat::MoveSpeed`, `HeroStat::SprintSpeed`, and the corresponding
`rulesets::move_speed::V1` and `rulesets::sprint_speed::V1` constants.

Both values are nominal stats in m/s. Nominal values do not include the current movement state. `move_speed` is the movement component.
`sprint_speed` is the additional sprint component, including base sprint speed.
Their sum gives full sprint speed. It is not the player's measured velocity.
Firing, crouching, slows, speed limits, sprint eligibility, acceleration and
sprint ramp-up do not change these nominal values.

### Equations

```text
move_bonus = 12 * (1 - product(1 - each_move_bonus / 12))
move_speed = (base_move_speed + move_bonus) * (1 + move_percent / 100)
sprint_speed = base_sprint_speed + sum(sprint_bonuses)
full_sprint_speed = move_speed + sprint_speed
```

The constant 12 comes from the supplied movement equation. It belongs to the
versioned rule. It is not a hero or item balance amount. Flat movement bonuses
remain separate inputs; Boon does not sum them before this equation. For example,
+2 and +3 m/s give +4.5 m/s. Sprint bonuses add without this reduction.

For example, a hero has base movement of 6.4 m/s and base sprint of 1.6 m/s.
Movement bonuses of +2 and +3 give 10.9 m/s movement speed.
Sprint bonuses of +2 and +1.5 give 5.1 m/s additional sprint speed.
The full sprint speed is 16 m/s. Values are not rounded.

V1 accepts at most one nonzero movement percentage. It applies to the movement
component, including flat bonuses, and does not multiply the sprint component.
V1 supports a single flat movement penalty. The interaction of a penalty with
other flat adjustments is not verified and remains unresolved. Flat bonuses above
12 m/s, multiple movement percentages, and nonfinite or negative final values
also remain unresolved. Boon does not clamp these to an invented value.

### Catalog inputs and coverage

Hero bases come from `m_mapStartingStats.EMaxMoveSpeed` and `ESprintSpeed`.
These values are already in m/s. When `m_mapScalingStats` declares spirit scaling
for either field, Boon adds that scaled amount to the corresponding hero base.
The coefficient comes from the selected catalog; no hero names are required.
Intermediate spirit inputs appear in the contribution table.

Flat effects use `MODIFIER_VALUE_MOVEMENT_SPEED_MAX` and
`MODIFIER_VALUE_SPRINT_SPEED_BONUS`. Movement percentages use
`MODIFIER_VALUE_MOVEMENT_SPEED_MAX_PERCENT`. Boon uses explicit stat bindings,
effective modifiers, intrinsic properties, and the caster's recorded upgrades.

Bare numeric movement modifier values use Source units per second. Property
values such as `2.0m` use metres per second. Boon applies the same unit conversion
to upgrade amounts. All movement contribution rows use m/s, except rows marked
`percent`. Time-dependent movement powerups use their catalog ranges and the
recorded application time. A teleporter's bound speed bonus uses the same path.

Boon reports missing intrinsic modifiers and unbound conditional properties as
partial inputs. It ignores passive property declarations that have no modifier
registration or intrinsic usage flag. For example, an unused `BonusSprintSpeed`
declaration does not add speed when the item's modifier registers other stats.

A catalog effect can specify `runtime_count`, the field name on its owning
ability entity. Boon multiplies the property value, including recorded upgrades,
by that count while the linked modifier is effective. The contribution trace
includes the count and its field name. A missing, negative, or noninteger count
makes the affected stat unresolved; it is not treated as zero.

New boon-data catalogs use this binding for Trophy Collector's
`StackingBonusSprintSpeed` and `m_iTrophyCount`, through `m_GoldModifier`.
The binding is curated engine knowledge (`binding_source: "curated"`), not a
VData registration. The selected catalog supplies the amount. Boon contains no
Trophy Collector ID or bonus value. Older catalogs without the binding still
report the per-stack property as a partial input. Install a catalog built with
the binding to use it.

Other multi-stack effects and unsupported property scaling remain unresolved.
Effects with no catalog stat declaration or binding can be missing.
See [Known Issues](known-issues.md#move-and-sprint-speed-coverage).

## Debuff resistance

Use `HeroStat.DEBUFF_RESIST` (Python) or `HeroStat::DebuffResist` (Rust).
The V1 rules are `rulesets.debuff_resist.v1` and `rulesets::debuff_resist::V1`.
The result is a percentage, without display rounding.

V1 multiplies the duration left after each resistance source:

```text
debuff_resist = 100 * (1 - product(1 - source_percent / 100))
affected_duration = original_duration * (1 - debuff_resist / 100)
```

Thus, two 25% sources give 43.75% resistance. A 30% result reduces a 10-second
debuff to seven seconds. Negative resistance increases duration: -8% gives
10.8 seconds. V1 does not clamp negative results to zero. This is a selected
stacking rule; the catalog supplies values, not the engine's equation.

Boon reads innate `EDebuffResist` from `heroes.json`. An omitted innate value
contributes zero. An explicit invalid value is an error. This includes negative
innate values without a hero-name or ID check.

Effective modifiers contribute their catalog `MODIFIER_VALUE_STATUS_RESISTANCE`
values. Explicit property bindings, recorded ability upgrades, and recorded
permanent values use the shared resolver. Bound item properties are counted
once through their modifier. A conditional property without a modifier binding
is skipped with a partial diagnostic. Unknown modifiers follow the same policy.

This stat does not identify which debuffs ignore resistance, grant immunity,
remove a debuff, or recalculate the end time of an existing debuff. Modifier
stacks and unsupported property scaling remain unresolved. No item-specific or
hero-specific bindings are added for this stat.

```python
result = demo.calculate_hero_stats(
    ticks=50707,
    data_version="6694",  # Select an installed version from boon versions.
    stats=[HeroStat.DEBUFF_RESIST],
    explain=True,
)
print(result.values)
```

## Damage resistance

`bullet_resist.v1`, `spirit_resist.v1`, and `melee_resist.v1` return percentages.
They retain negative values and do not round to the in-game display.

```text
innate = base + boon growth + spirit scaling
resistance = 100 * (1 - (1 - innate / 100) * product(1 - other_source / 100))
reduction = 100 * (1 - product(1 - reduction_source / 100))
result = resistance - reduction
```

For example, 40% and 20% resistance give 52%. Reductions of 25% and 20% give
40% reduction. The result is 12% resistance. If reduction exceeds resistance,
the result is negative. V1 rejects nonfinite inputs, individual sources above
100%, and arithmetic overflow. Negative resistance sources are allowed.

| Stat | Hero starting stat | Resistance modifier | Reduction modifier |
| --- | --- | --- | --- |
| `bullet_resist` | `EBulletArmorDamageReduction` | `MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST` | `MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION` |
| `spirit_resist` | `ETechArmorDamageReduction` | `MODIFIER_VALUE_TECH_RESIST` | `MODIFIER_VALUE_TECH_RESIST_REDUCTION` |
| `melee_resist` | `EMeleeResist` | `MODIFIER_VALUE_MELEE_RESIST` | `MODIFIER_VALUE_MELEE_RESIST_REDUCTION` |

The resolver reads starting stats, `m_mapStandardLevelUpUpgrades`, and
`m_mapScalingStats` from the selected hero record. An absent base is zero; it
does not suppress explicit spirit scaling. Boon growth counts the reached
levels that use standard upgrades. No hero values or scaling coefficients are
embedded in the calculation.

Resistance bonuses use explicit modifier bindings or intrinsic usage flags.
Reductions use effective modifiers on the recipient. Owning a shred item does
not apply its debuff to the owner. When a reduction property has upgrades, Boon
requires the caster's state; it does not use the victim's upgrade tier as a
substitute. Catalog reductions use negative values, which Boon converts to
positive magnitudes for the reduction equation. Contribution rows retain the
catalog sign. Unsupported positive reduction values produce an error.

`melee_resist` is the separate melee component. It excludes bullet resistance
and shared weapon shred, which are reported by `bullet_resist`. It is not total
protection against a melee hit. Do not combine these final values to calculate damage taken.
Engine exceptions and the order of shared reductions require a separate calculation. Boon contains no hero-specific exception for this.

NPC-only resistance, immunity, critical-hit protection, and general incoming
damage multipliers are outside these stats. Unbound properties produce partial
values. Empty declarations with no value, binding, activation flag, or upgrade
are ignored. They do not describe a stat change. Unsupported stacking, property
scaling, or caster state remains unresolved.

```python
result = demo.calculate_hero_stats(
    ticks=50707,
    data_version="6694",  # Select the version for the replay.
    stats=[HeroStat.BULLET_RESIST, HeroStat.SPIRIT_RESIST, HeroStat.MELEE_RESIST],
    strict=False,
    explain=True,
)
print(result.values)
print(result.contributions)
```

## Lifesteal

`bullet_lifesteal.v1`, `spirit_lifesteal.v1`, and `melee_lifesteal.v1` return
nominal percentages. Each damage type is separate. Independent sources combine as:

```text
lifesteal = 100 * (1 - product(1 - source_percent / 100))
```

For example, 22% and 30% give 45.4%. Boon does not round to the in-game display.
V1 requires finite source percentages from 0 through 100.

Bullet and spirit lifesteal use these catalog fields:

| Stat | Hero starting stat | Modifier value |
| --- | --- | --- |
| Bullet lifesteal | `EBulletLifesteal` | `MODIFIER_VALUE_BULLET_LIFESTEAL` |
| Spirit lifesteal | `ETechLifesteal` | `MODIFIER_VALUE_TECH_LIFESTEAL` |

An absent innate value is zero. An invalid explicit value is an error.
Graves' innate bullet lifesteal comes from his selected hero record. Boon has
no hero-specific innate values. Item amounts, ability upgrades, and active
modifier bindings also come from the selected catalog. Bound effects apply only
while their modifiers are effective. Explicit intrinsic properties can apply
from ability ownership. Missing activation bindings produce partial values,
including declarations that have no usage flags.

The values exclude healing boosts, healing reduction, target-specific effects,
and creep effectiveness. The resolver does not apply `EHealingOutput` scaling
to these nominal percentages. Other property scaling remains unsupported; it
is not silently discarded. These stats do not predict health gained.

The catalog has no melee-lifesteal modifier-value symbol. V1 recognizes the exact
`MeleeLifesteal` property on an owned passive ability. This is an explicit property
rule, not a name or hero-ID match. It reads the property and its upgrades from
VData. Nonzero contributions have `status="partial"`; the diagnostic states the
assumed link, and `explain=True` shows the property path. This supports Infernal
Resilience without embedding its amounts or upgrade tiers in Boon.

`TargetLifesteal` requires target and damage-type context. A nonzero value remains
unresolved for melee queries. Cooldown-based healing, such as Melee Lifesteal and
Lifestrike item procs, is excluded. Their healing amounts are not continuous
melee lifesteal, even when the item name includes that term.

```python
result = demo.calculate_hero_stats(
    ticks=50707,
    data_version="6694",  # Select the version for the replay.
    stats=[
        HeroStat.BULLET_LIFESTEAL,
        HeroStat.SPIRIT_LIFESTEAL,
        HeroStat.MELEE_LIFESTEAL,
    ],
    explain=True,
    strict=False,  # Keep null values and diagnostics for unsupported inputs.
)
print(result.values)
print(result.contributions)
```

## Unresolved inputs

| Status | Meaning |
| --- | --- |
| `calculated` | The rule calculated a value from the supported inputs. |
| `partial` | The value uses known inputs but has missing effects or an assumed link. |
| `unresolved` | Boon cannot calculate the value; `value` is null. |

Boon reports unknown modifiers and unclear catalog matches as `partial`.
The diagnostic lists the skipped IDs. Assumed activation links also produce
partial values. These rules apply with either `strict` setting.
A calculated value can still lack effects that the catalog does not describe.

For other unsupported inputs, `strict=True` raises `CalculationError`.
Use `strict=False` to keep those rows with null values and diagnostics.
Invalid queries, missing ticks, and failed catalog downloads still cause errors.

Boon excludes `modifier_player_pinged` and `modifier_entity_pinged` from stat
calculations. These two ID exceptions apply to all heroes.
The ping markers remain in the modifier datasets.

Catalog counter bindings support some effects with stacks, such as Trophy
Collector and Bloodscent. Other stack rules and property scaling can remain
unsupported. See [Known Issues](known-issues.md) before you interpret a result.

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
    let parser = Parser::from_file(Path::new("106996573.dem"))?;
    let catalog = StatCatalog::load("6694")?;
    let query = HeroStatQuery::new(
        [50707, 50800],
        [HeroStat::ClipSize, HeroStat::FireRate],
    )
    .steam_ids([76561197999389679])
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
