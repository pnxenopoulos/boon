# ⚠️ Known Issues

Deadlock updates can change the demo format and game rules.

## Stat calculations

Some stat calculations can give incorrect values. This can occur even when
`status` is `calculated`. Compare the results with the demo viewer.

Report incorrect values on [GitHub](https://github.com/pnxenopoulos/boon/issues)
or [Discord](https://discord.gg/WmjZHxWrCD). Include the demo, tick, hero, stat
name, and boon-data version. Give the calculated value and the value in the demo
viewer.

Some untimed modifier rows remain after their effects end. Stat queries exclude
rows when all states declared by the catalog are absent from the pawn's recorded
masks. Rows without these state declarations can still cause incorrect values.

## Client 6712 stat inputs

Boon reads the new primary weapon block and uses controller ticks when pawn
simulation time is absent. Stat queries use recorded stat types for corruption
penalties and permanent range/radius pickups. Shop bonuses use the new cost
tables even when the old category table is absent.

Corruption bonuses can vary per match. Catalog values are not the exact rolled
values. The replay records some results, such as penalties and effect durations,
but Boon does not yet apply corrupted property upgrades. Affected properties
keep their known subtotal and report a partial result. The controller's stat
table can also name the wrong source item for a penalty. Active modifier records
can identify the source item. Do not count both records as separate penalties.

The legacy `stat_modifier_*` snapshot columns use enum IDs from older clients.
Their totals can be incorrect for this client. Keep the raw recorded entries
when you check these values. An unresolved result does not mean zero.

## Ammo and barrier snapshots

Ammo counts use calculated capacity. If the capacity is partial, the count is
also partial. Haze and Yamato can have null counts when spirit scaling cannot
be resolved. The recorded ammo fraction and unlimited-ammo state remain separate.

In some recent demos, the existing `barrier` snapshot value differs between a
direct tick query and a full pass. Check this value against the demo viewer.

## Battle Vest health condition

Boon does not yet support Battle Vest's stat bonuses that depend on the wearer's
health. The VData contains `LifeThreshold` and `BonusFireRate`, but no explicit
rule that links the bonus to health above the threshold. The intrinsic modifier
can be present above or below the threshold. Its presence does not prove that
the bonus is active.

When this condition affects a `calculate_hero_stats()` fire-rate result:

- The default `strict=True` raises `CalculationError`.
- With `strict=False`, the affected row has a null `value`,
  `status="unresolved"`, and a `diagnostic` that explains the missing condition.

A null result does not mean zero fire-rate bonus. Boon does not assume that the
bonus is always active. Battle Vest item and modifier data remain available.

See [hero stat calculations](hero-stats.md) for supported rules and result fields.

Battle Vest's weapon bonus also lacks an explicit stat mapping in the catalog.
Weapon and melee queries return the known subtotal as `partial` and report the
missing `BaseAttackDamagePercent` mapping.

## Weapon damage coverage

`weapon_damage` is a global bonus percentage. It excludes flat bullet damage,
critical hits, range effects, target resistance, and target-specific damage.

Some declared weapon bonuses have no modifier or counter binding. Examples in
catalog 6694 include Intensifying Magazine's firing ramp and Assassinate's weapon
bonus per kill. An intrinsic usage flag alone does not supply a count. Battle
Vest's `BaseAttackDamagePercent` also lacks a stat mapping. Boon returns the known
subtotal as `partial`, with a diagnostic.

New boon-data catalogs bind Bloodscent's recorded kills and assists to its weapon
bonus. Older catalogs lack this binding and still return partial values. This
curated relationship reads the bonus and assist weight from the catalog. A target
marker does not grant or diagnose the source ability's unbound owner rewards.
Opening Rounds' enemy-health bonus is outside the global stat; its ordinary
weapon bonus still applies. Melee uses these same inputs and exclusions.

Shop bonuses need the catalog's cost thresholds and item prices in `misc.json`.
Older releases must be rebuilt with `generic_data.vdata`. Then run
`boon get VERSION --force`. Engine effects absent from the catalog can still be missing. Legacy weapon
power has no verified conversion to a percentage and is reported when nonzero.
Unsupported bound scaling or modifier stacks remain unresolved. For example,
Plot Armor's weapon bonus uses `scale_function_tech_damage`, which this rule does
not yet evaluate. It needs the caster's spirit value and a verified scaling rule.

## Melee damage coverage

Melee damage now uses the weapon-damage resolver. In `106996573.dem` at tick
50707, Battle Vest leaves Venator partial, and Intensifying Magazine leaves
Wraith partial. Opening Rounds' target-health bonus is outside the nominal stat.
Bloodscent target markers do not transfer their owner's rewards or diagnostics.
With a catalog counter binding, Drifter's own Bloodscent reward is included.

The rules return nominal light and heavy melee damage. They exclude next-hit
procs such as Melee Charge's extra heavy damage, target immunity, and resistance.
Paige's hero spirit scaling comes from the catalog. The heavy/light boon ratio
and the order of spirit scaling still need comparison with the demo viewer.
Registered melee or all-damage multipliers with nonzero values remain unresolved.

## Movement and evasion stat coverage

Slide distance and bullet evasion prefer explicit effect bindings. Declared stat
properties without an activation binding produce partial values and a diagnostic.

For an undeclared `EvasionPercent`, bullet evasion assumes that the owning
ability's unique effect modifier activates the property. Cast-delay and intrinsic
modifiers do not qualify. This supports Haze's Bullet Dance, including its upgrades,
but VData does not prove the link. Active results are partial and identify the
assumption. More than one candidate modifier leaves the contribution unbound.

Other undeclared properties, such as Electric Slippers' `EvasionWhileSliding`,
remain unsupported. Boon does not use substring matching to detect them.
Even a calculated zero can therefore omit engine effects.

V1 does not combine multiple nonzero evasion chances. Those combinations remain
unresolved. Gravity scale returns only the recorded pawn field, which need not
include all effective gravity changes.
See [the stat rules](hero-stats.md#slide-distance-bullet-evasion-and-gravity-scale).

## Stamina and dash stat coverage

V1 does not combine multiple nonzero recovery percentages or multiple dash-distance
percentages. It also does not combine flat and percentage recovery changes.
These stacking rules are not verified. Affected values are unresolved.

An explicit stamina recovery pause produces an unresolved cooldown. There is no
finite refill time while the pause applies. Free dashes leave stamina capacity
finite. Neither stat returns the current resource or remaining refill time.

Dash speeds are nominal distance divided by duration. Distance effects change
speed, with no change to the catalog duration. Replay checks support ordinary
air-dash timing, but do not verify all movement abilities. Flight, movement
curves, interrupts, and ability-specific dashes are outside V1.

These stats require explicit stat bindings. An unbound effect is omitted with
a partial diagnostic. The same applies when an owned item has a catalog intrinsic
binding but no effective modifier in the parsed state. The table-slot reuse fix
restores Ivy's Stamina Mastery at tick 76466 in `106996573.dem`; the calculated
capacity is six points. Engine effects absent from the catalog can still be missing.
See [the rules and inputs](hero-stats.md#stamina-and-ordinary-dashes).

## Move and sprint speed coverage

Move speed and sprint speed are nominal stats. Their sum is full sprint speed.
They do not simulate firing, crouching, slows, speed limits, sprint eligibility,
acceleration, or sprint ramp-up. `player_states()` reports recorded state flags
separately. Those flags do not adjust the calculated speeds.

V1 does not combine multiple movement percentages or flat movement penalties
with other flat adjustments. It does not extrapolate the diminishing-return rule
for a single flat bonus above 12 m/s. These cases remain unresolved.

Older catalogs omit Trophy Collector's per-stack sprint binding. These catalogs
produce partial rows. New catalogs declare the link to the ability entity's
`m_iTrophyCount`. Boon uses the recorded count and the catalog bonus while the
linked modifier is active. The catalog contains this manually supplied binding.
VData does not declare the link.

Other missing bindings and missing intrinsic modifiers produce partial rows.
Other multi-stack effects and unsupported property scaling remain unresolved.
Effects absent from the catalog can still be missing.

At tick 50707 in `106996573.dem`, Victor has two modifiers absent from VData:
`1215616703` (`modifier_player_pinged`) and `2693099904`
(`modifier_entity_pinged`). Their IDs match hashes of the names in the tracking
repository's client and server strings for catalog 6694. Both have six-second
durations in the replay. No stat effect was found. Boon excludes these two ping
markers from stat calculations for all heroes. They remain in the modifier
datasets and do not make calculated values partial. Other unknown modifiers
still produce diagnostics.
See [the equations and catalog inputs](hero-stats.md#move-speed-and-sprint-speed).

## Damage resistance coverage

Resistance queries do not calculate damage taken. `melee_resist` is the separate
melee component; bullet resistance and shared weapon shred remain in
`bullet_resist`. There is no special-case adjustment for Venator or another hero.
NPC-only resistance, immunity, and general damage multipliers are excluded.

Unbound resistance properties with values produce partial values. Empty type
and display declarations without bindings, activation flags, or upgrades are
ignored. Examples of unsupported bonuses include Burrow's
conditional resistance and Escalating Resilience's per-stack bonus. Boon does
not infer stack behavior or activation from an item name. Bound modifiers with
unsupported stack counts or scaling remain unresolved. Unknown modifiers still
produce partial diagnostics.

Resistance-reduction properties with upgrades require a resolved caster. Missing
caster state produces an unresolved value. The resolver does not substitute the
victim's upgrades. Positive values in reduction fields also remain unsupported;
the known hero debuffs encode reductions as negative percentages.

## Lifesteal coverage

Lifesteal values are nominal percentages, before healing adjustments and creep
effectiveness. Bullet and spirit lifesteal require explicit stat bindings or
intrinsic usage flags. Some declarations lack activation bindings. For example,
Vampiric Burst's active bonus and Mystic Reverb's imbued-ability bonus are omitted
with partial diagnostics. They are not applied to all damage from item ownership.

Kudzu Connection uses `scale_function_healing_spirit_scale`. Its spirit-scaling
equation is not verified. An active contribution with this scaling is unresolved.

Melee lifesteal uses the exact `MeleeLifesteal` property on an owned passive.
VData does not register that property as a modifier value, so this interpretation
is stated in a partial diagnostic. Target-dependent effects, such as Riposte's
`TargetLifesteal`, remain unresolved when nonzero. Melee Lifesteal and Lifestrike
item procs are excluded because their healing is gated by cooldowns. A zero
melee-lifesteal result does not mean that melee attacks cannot heal the player.

## Debuff resistance coverage

`debuff_resist.v1` multiplies remaining-duration factors from known sources.
It does not infer immunity, cleansing, or which debuffs ignore resistance.
Unbound conditional properties produce partial values. In catalog 6694, for
example, Scourge's (`upgrade_discord`) `StatusResistancePercent` has no binding, so its
conditional contribution is skipped and reported. Unsupported modifier stacks
and scaling remain unresolved. See [the rules and inputs](hero-stats.md#debuff-resistance).

## Barrier pool snapshots are not grant events

`player_ticks.barrier` mirrors the replicated modifier tracker. A full-packet
snapshot can introduce a partial pool before the ordinary grant update.
For example, match 100655353 shows a partial pool at tick 149761 and the grant
update at tick 150923. Do not treat every pool rise as a new barrier grant or
every fall as absorption. Exact grant lifecycles remain unresolved.

## Player stat modifiers are not final stats

`demo.player_ticks` reads `m_vecStatViewerModifierValues` from each player
controller. The `stat_modifier_*` columns contain signed sums for the value
types that Boon knows. They do not include base hero stats, all item values, or
all temporary effects. Do not use these columns as final damage mitigation or
effective player stats.

`stat_modifier_values_available` is false when the demo serializer does not
contain this vector. `unknown_stat_modifier_count` is the number of vector
entries with a nonzero `EModifierValue` that this Boon version does not know.

Valve can renumber these values between client versions. The decoder contains
aliases observed in tested demos; it does not select a layout by client version.
An unknown-count value of zero does not prove that all aliases are correct for
a new client. New boon-data catalogs include `modifier_value_types` for ability stat queries.
The fixed `player_ticks.stat_modifier_*` decoder does not use that map.

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


## Ability bonuses and Arcane Surge

See [ability stats](ability-stats.md) for the scope of these calculations.
An Arcane Surge watcher does not by itself identify the affected ability.
At tick 50707 in `106996573.dem`, this leaves Paradox's range, radius, and duration
bonuses partial. The extra bonus is omitted and listed in the explanation.
Ready next-cast bonuses are also kept separate from applied effects.

Dynamic values need `modifier_value_types` from the matching boon-data revision.
Catalogs without that field cannot decode those values. Unsupported scaling,
missing charge counts, and unknown apply filters remain unresolved. The API
returns bonus percentages; it does not yet calculate individual ability property
values in seconds or metres, or reconstruct the inputs to an earlier cast.
