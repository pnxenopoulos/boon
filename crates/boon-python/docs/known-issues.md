# ⚠️ Known Issues

Use Boon **0.10.0 or earlier** for demos recorded before the **City Never
Sleeps** update (**September 29, 2026**). Use Boon **0.11.0 or later** for
demos recorded with that update.
Deadlock updates can change the demo format and game rules.

## Stat calculations

Some stat calculations can give incorrect values. This can occur even when
`status` is `calculated`. Compare the results with the demo viewer.

Report incorrect values on [GitHub](https://github.com/pnxenopoulos/boon/issues)
or [Discord](https://discord.gg/WmjZHxWrCD). Include the demo, tick, hero, stat
name, query mode, and boon-data version. Give both the calculated value and the
viewer value.

Some untimed modifier rows remain after their effects end. Stat queries exclude
rows when all states declared by the catalog are absent from the pawn's recorded
masks. After an observed present-to-absent transition, the same application stays
ended. A new application timestamp can restore it. Missing state declarations or
overlapping state sources can still leave old effects in calculations.

Engine strings can identify a modifier without defining its effects. Diagnostics
include such names when the ID has one name. A name does not prove that a modifier
is harmless or has expired. Some rows track general game state. For example,
`modifier_citadel_pre_match_wait` remains in all 12 players' raw records in demo
`108575009.dem`, although the recorded `PREMATCH` state clears at tick 2. Use
`player_states()` for recorded states; row presence alone does not prove a stat
bonus. These unclassified rows can still cause a partial result.

Baseline mode excludes effects whose passive or active role is unknown and
reports a partial value. Current mode retains the existing calculation limits.
Movement values do not apply crouching, sprint acceleration, or bullet-hit slows,
even in current mode. The movement rules do not yet read and combine those inputs.
Baseline gravity scale is unavailable because the replay supplies only the current pawn value.

## Spirit power and modifier bindings

Percentage spirit bonuses multiply the complete global flat total, including
supported temporary sources. Ability-only spirit stays separate. Explanations
list flat and percentage inputs. Active property scaling functions remain
unsupported. Hero and ability queries share checks that read catalog defaults
and `m_bFunctionDisabled`. Scaling errors show the class, stat, and coefficient.

New boon-data builds bind Spirit Snatch's separate caster buff and victim debuff.
Boon multiplies their upgraded flat spirit and resistance values by the normalized
recorded count. This curated link uses replay evidence and stays in diagnostics.
Recorded removals and count changes end contributions; Boon does not guess a
separate target-death rule. Older catalogs lack this link.
Ice Path's `BonusSpiritPct` still lacks an explicit stat type and modifier binding
in catalog 6712.

New boon-data builds resolve explicit non-embedded modifier references. For
example, Escalating Exposure and Spirit Burn reference the same spirit-resist
debuff. Boon selects its property through the recorded source ability. Missing
source identity leaves the value unresolved; it does not select another item.
At tick 187554 in `108575009.dem`, these recorded applications lower the known
spirit-resist totals by 8 points. Some have a friendly caster. These new totals
still need a viewer check; the source link alone does not verify all effect rules.

New boon-data builds include Ice Path's friendly aura bindings and a curated
Mercurial Magnum fire-rate binding. The latter uses an inferred activation link
and keeps the result partial. It applies only while the fire-rate buff is active.
The separate bullet-damage buff does not grant fire rate. To use these bindings,
rebuild the required boon-data release, then run `boon get VERSION --force`.

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

## Ammo and barrier snapshots

Ammo counts use calculated capacity. If the capacity is partial, the count is
also partial. Missing spirit inputs can leave counts null. Percentage spirit
bonuses no longer block Haze's capacity. Older catalogs without the Spirit Snatch
binding can still leave Yamato's current capacity unresolved. The recorded ammo
fraction and unlimited-ammo state remain separate.
Snapshot ammo joins require a unique Steam ID at each tick. Missing or duplicate
IDs leave `ammo`, `max_ammo`, and `unlimited_ammo` null with a diagnostic.

Barrier snapshots report pool values. See [barrier pool limits](#barrier-pool-snapshots-are-not-grant-events).

## Battle Vest health condition

Boon does not yet support Battle Vest's stat bonuses that depend on the wearer's
health. The VData contains `LifeThreshold` and `BonusFireRate`, but no explicit
rule that links the bonus to health above the threshold. The intrinsic modifier
can be present above or below the threshold. Its presence does not prove that
the bonus is active.

An unbound bonus can leave a `partial` subtotal with a diagnostic. If a declared
condition cannot be resolved, `strict=True` raises `CalculationError`.
With `strict=False`, that row has a null `value` and `status="unresolved"`.
The outcome depends on the selected catalog's effect bindings and usage flags.

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
`boon get VERSION --force`. Effects absent from the catalog can still be missing.
Legacy weapon power has no verified conversion to a percentage and is reported when nonzero.
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

`player_ticks.barrier` reads the recorded modifier tracker from packet changes.
It excludes relay keyframe modifier tables, which can have a different capture
time. Do not treat every pool rise as a new barrier grant or every fall as
absorption. Direct seeks and full passes use the same cached history. These values do not
identify individual grants or absorbed damage.

## Recorded stat bonuses

`stat_modifier_events` uses enum definitions from the newest installed boon-data
catalog. Use data for the replay's client version. It records changes to known
bonus types, not final hero stats. Use `calculate_hero_stats()` for calculated
values. The old `stat_modifier_*` snapshot columns have been removed.

## Banned heroes are frequently absent

`demo.banned_heroes` reads the `k_EUserMsg_BannedHeroes` message (type 366).
The server can send it before the match. A recording can omit this message,
even when another recording from the same server version contains it.

An empty frame means no recorded ban data. It does not establish that the match
had no bans. The message supplies hero IDs only, without teams, players, or
draft order.

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
