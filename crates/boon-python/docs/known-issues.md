# Known Issues

Use Boon **0.10.0 or earlier** for demos before City Never Sleeps (**September 29, 2026**).
Use Boon **0.11.0 or later** for demos from that update or later.
Game updates can change demo formats and calculation rules.

## Stat calculations

Some calculated values can be incorrect, even with `status="calculated"`.
Compare results with the demo viewer.
Report errors on [GitHub](https://github.com/pnxenopoulos/boon/issues) or [Discord](https://discord.gg/WmjZHxWrCD).
Include the demo, tick, hero, stat, mode, catalog version, calculated value, and viewer value.

Untimed modifiers can stay after their effects end.
Missing catalog-declared states can end an effect after an observed transition.
A new application timestamp can restore it.
Missing declarations or shared state sources can leave the expiry time unknown.
Raw row presence does not prove an active stat effect.
Use `player_states()` for recorded flags.

Unknown passive roles give partial baseline values.
Current mode includes supported temporary effects, but does not simulate firing, crouching, bullet-hit slows, or sprint acceleration.
Baseline gravity scale is unavailable.

## Spirit power and modifier bindings

Global spirit percentages multiply ordinary flat inputs. Catalog post-multiplier bonuses apply last.
Ability-only spirit has its own total.
Active property scaling functions stay unsupported.
The shared resolver uses catalog scaling defaults and `m_bFunctionDisabled`.

| Catalog binding | Limit |
| --- | --- |
| Spirit Snatch | Caster/victim effects use normalized counts. The supplied link stays in diagnostics. |
| Ice Path | The caster's flat spirit bonus applies after multipliers. Ally spirit scope and lingering effects have unverified scope. |
| Mercurial Magnum | The supplied fire-rate activation link gives a partial result. The other bullet-damage buff does not grant fire rate. |
| Shared modifier references | The recorded source ability selects the property. A missing source leaves the value unresolved. |

These bindings use updated boon-data files.
After the selected release changes, run `boon get VERSION --force`.

## Client 6712 stat inputs

The resolver reads the primary weapon block, cost-based shop bonuses, and recorded stat types.
Recorded penalties and permanent range/radius pickups count once.

Corrupted property bonuses are incomplete. Catalog defaults do not represent all rolled values.
Affected stats keep a partial subtotal with diagnostics.
A recorded source-item label can also be incorrect. Modifier records can identify the source.
Do not count duplicate penalty records twice.

## Ammo and barrier snapshots

Ammo counts depend on calculated capacity. Partial capacity gives partial counts; missing inputs can give null counts.
The recorded `ammo_fraction` and `unlimited_ammo` flag are different quantities.
Missing or duplicate Steam IDs leave calculated ammo fields null with diagnostics.

Barrier snapshots report pools, not individual grants. See [barrier limits](#barrier-pool-snapshots-are-not-grant-events).

## Battle Vest health condition

VData supplies `LifeThreshold` and `BonusFireRate`, but does not state the direction of the health condition.
The intrinsic modifier can exist above or below the threshold.
Boon does not infer activation from its presence.
The unbound `BaseAttackDamagePercent` weapon bonus also lacks a stat mapping.

Missing links give partial subtotals or unresolved values, depending on the catalog declarations.
`strict=True` raises for unresolved values; `strict=False` keeps null rows.
A null value does not mean zero bonus.

## Weapon damage coverage

`weapon_damage` is a global bonus percentage, not damage for each projectile.
It does not include flat bullet damage, critical hits, falloff, target resistance, and target-specific bonuses.
Opening Rounds' enemy-health bonus is outside this stat; its ordinary weapon bonus applies.

Some rewards lack activation or counter bindings, including Intensifying Magazine's firing ramp and Assassinate's bonus for each kill.
They give partial subtotals. Updated Bloodscent bindings use recorded kills and assists.
Another player's target marker does not transfer the owner's reward.

Missing shop prices, unsupported stacks, and active property scaling can leave values unresolved.
Legacy weapon power has no verified percentage conversion.
Plot Armor's `scale_function_tech_damage` is also unsupported.

## Melee damage coverage

Melee uses the same weapon inputs and exclusions.
It returns nominal light/heavy damage before resistance, immunity, and effects that occur on a hit.
Compare the heavy/light boon ratio and spirit-scaling order with more viewer values.
Nonzero melee or all-damage multipliers stay unresolved.

## Movement and evasion stat coverage

Unbound slide or evasion properties give partial values.
The exact `EvasionPercent` property can use an owner's unique timed effect modifier.
That link is an assumption; the diagnostic identifies it.
Other unbound properties, such as `EvasionWhileSliding`, stay unsupported.
Multiple nonzero evasion chances stay unresolved.

Gravity scale reports only `m_flGravityScale`; it can lack effective gravity changes.
See [the rules](hero-stats.md#slide-distance-bullet-evasion-and-gravity-scale).

## Stamina and dash stat coverage

Multiple recovery percentages, mixed flat/percentage recovery, and multiple dash-distance percentages stay unresolved.
Paused or nonpositive recovery has no finite refill time.
Stamina is capacity, not current resource. Free dashes keep capacity finite.

Dash speeds use distance divided by duration. Distance bonuses do not change the catalog duration.
Movement curves, interrupts, flight, and ability-specific dashes are outside this model.
Missing effect bindings or intrinsic modifiers give partial results.
See [the rules](hero-stats.md#stamina-and-ordinary-dashes).

## Move and sprint speed coverage

Movement and sprint values are nominal components. Their sum is full sprint speed, not measured velocity.
They do not simulate firing, crouching, bullet-hit slows, speed limits, sprint eligibility, or acceleration.
Recorded state flags do not adjust these values.
Multiple movement percentages stay unresolved.

Trophy Collector uses the supplied catalog binding to `m_iTrophyCount`.
Other missing bindings, counts, and unsupported scaling can prevent complete values.
The two known ping markers stay in raw datasets but do not enter stat calculations.
Other unknown modifiers give diagnostics.
See [the rules](hero-stats.md#move-speed-and-sprint-speed).

## Damage resistance coverage

Resistance values do not calculate damage taken, immunity, or NPC-only resistance.
`melee_resist` reports only the melee component; shared bullet/melee effects stay in `bullet_resist`.

Unbound conditions give partial values, including Burrow's resistance and some bonuses for each stack.
Unsupported stacks and scaling stay unresolved.
Upgraded reductions use the caster's state. Positive reduction fields have no supported interpretation.

## Lifesteal coverage

Values are nominal percentages, before healing adjustments and creep effectiveness.
Missing activation bindings give partial values.
Kudzu Connection's `scale_function_healing_spirit_scale` stays unsupported.

Melee uses the exact passive `MeleeLifesteal` property through an assumed link.
Target-dependent lifesteal stays unresolved.
Cooldown-based Melee Lifesteal and Lifestrike healing procs are not included.
Zero continuous lifesteal does not prove that melee attacks cannot heal.

## Debuff resistance coverage

The rule combines remaining-duration factors.
It does not identify immunity, cleansing, or debuffs that ignore resistance.
Unbound conditions give partial values; unsupported stacks and scaling stay unresolved.
See [the rules](hero-stats.md#debuff-resistance).

## Barrier pool snapshots are not grant events

`player_ticks.barrier` reads the recorded modifier pool from packet changes.
Direct seeks and full passes use the same cached history.
Relay keyframe modifier tables can have a different capture time and do not supply this pool history.
A pool rise does not prove a new grant. A fall does not prove absorption.
Use recorded summary `barrier_absorption` for provided barrier totals.

## Recorded stat bonuses

`stat_modifier_events` uses enum definitions from the newest installed boon-data catalog.
Use data for the replay's client version.
The dataset records bonus changes, not final stats.
The old `stat_modifier_*` snapshot columns are unavailable.

## Banned heroes are frequently missing

A demo can lack the `BannedHeroes` message.
An empty `demo.banned_heroes` does not prove that the match had no bans.
The message supplies hero IDs without teams, players, or draft order.

## Item upgrade links

`upgraded_from_ability_ids` matches catalog components to sales by the same Steam ID and tick.
The link is inferred. An unrelated sale at another tick does not qualify.
Missing IDs, missing catalog links, or competing matches give an empty list.
Keep recorded `change` values; an item-path upgrade can have `change="purchased"`.
See {ref}`the API <item-purchases>` and [upgrade example](examples.md#item-upgrades).

## Ability bonuses and Arcane Surge

An Arcane Surge watcher alone does not identify the affected ability.
Ready next-cast bonuses apply only to a recorded target. Missing targets give partial values.

Dynamic values use the matching catalog's `modifier_value_types`.
Unsupported scaling, missing charge counts, and unknown apply filters stay unresolved.
Results are bonus percentages, not final property values or inputs to an earlier cast.
See [ability stats](ability-stats.md).
