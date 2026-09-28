# Ability stats and imbues

Use `demo.imbues()` to read item selections. Use `demo.calculate_ability_stats()`
to calculate the bonus percentages for each ability at selected ticks.
The methods use the same replay state and an explicit boon-data version.

```bash
boon versions
boon get GAME_VERSION
```

```python
from boon import AbilityStat, Demo

version = "6694"  # Select the catalog version for your analysis.
demo = Demo("106996573.dem", preload=False)

imbues = demo.imbues(ticks=50707, data_version=version)
print(imbues.bindings)
print(imbues.effects)

result = demo.calculate_ability_stats(
    ticks=[50707, 60000],
    data_version=version,
    stats=[
        AbilityStat.COOLDOWN_REDUCTION,
        AbilityStat.DURATION_BONUS,
        AbilityStat.RANGE_BONUS,
        AbilityStat.RADIUS_BONUS,
    ],
    explain=True,
    strict=False,
)
print(result.values)
print(result.contributions)
```

A missing installation is downloaded and verified. The selected version is never
replaced with the latest version. Dynamic values need a catalog with
`modifier_value_types`, built from the matching tracking revision. Older catalogs
without that mapping report an error when a dynamic value needs it. Use
`boon get VERSION --force` after a release with that field is available.

## Selection and results

`ticks` accepts one integer or a sequence of exact demo ticks. Results describe
state after the tick updates. `players` selects player slots, as in hero stat
queries. Omit it to include all players. Multiple ticks share one parser pass.

By default, stat queries select the current hero's signature abilities. Use
`abilities=[ability_id, ...]` to select owned abilities or items. These are catalog
IDs, not slot numbers. Use `include_items=True` to add all owned items.
An explicit ability ID must belong to a selected player during the query.

`imbues.bindings` has one row per tick, player, source item, and selected ability.
It contains both IDs and names. Missing catalog records do not remove a recorded
selection. `imbues.effects` has one row for each catalog effect restricted to that
selection. It includes the property, value, stat symbol, and `apply_filter`.
These are individual inputs; no stacking rule is applied to this table.
A binding can have no effect rows if its catalog has no mapped stat changes.

Stat `values` contain the tick, player slot, hero ID, ability ID and name, stat,
value, unit, rule, status, and diagnostic. `contributions` also identify the source,
property, scope, activation state, and whether the input was included. Set
`explain=True` to create this table. `metadata` identifies the catalog version,
source revision, and equation versions.

## Percentage rules

All values use percentage points. `0.75` means 0.75%, not 75%.
These are bonus percentages, not final seconds, metres, or current cooldown timers.

| Stat | Rule |
|---|---|
| `cooldown_reduction` | `boon.rulesets.cooldown_reduction.v1` |
| `item_cooldown_reduction` | `boon.rulesets.item_cooldown_reduction.v1` |
| `duration_bonus` | `boon.rulesets.duration_bonus.v1` |
| `range_bonus` | `boon.rulesets.range_bonus.v1` |
| `radius_bonus` | `boon.rulesets.radius_bonus.v1` |

V1 follows the supplied wiki equation for each percentage:

```text
combined = 100 * (1 - product(1 - source_percent / 100))
```

For example, 10% and 20% combine to 28%. Do not round the inputs or intermediate
values. Values above 100%, nonfinite values, and arithmetic overflow are errors.
Negative sources are retained. Each rule has its own name, version, and documented
date. Pass a `rulesets` mapping to select a supported rule for every requested stat.

Ability cooldown reduction does not apply to items. Item cooldown reduction does
not apply to hero abilities. An item whose catalog disables cooldown scaling also
has no applicable item cooldown bonus. Such rows have status `not_applicable`
and a null value. Range and duration bonuses can apply to items.

The charged-ability filter uses the recorded maximum charges when present, or the
catalog charge count with purchased upgrades. Ultimate-only reduction applies to
the hero's fourth signature slot. Charge recharge time and the delay between uses
remain separate properties. Direct timer changes, such as Witchmail's proc, do
not become persistent cooldown-reduction percentages.

## Sources and scope

The resolver combines only inputs that apply to the selected ability:

- Recorded permanent stat totals. A live pickup modifier is not added again.
- Active bound effects, including temporary powerups and supported counters.
- Catalog effects restricted to an imbued or charged ability.
- Recorded dynamic ability values, decoded with the catalog's enum names.
- Supported ability upgrades and conditional effects.

A recorded dynamic value replaces its matching catalog contribution, including
when the value is zero. A value restricted to one ability does not replace a
global bonus on other abilities. Unknown targeting filters are not treated as
global effects. Balance values and numeric enum IDs are not embedded in Boon.

## Conditional casts and limits

A bonus for the next eligible cast is different from an item imbue.
The explanation can show `ready`, `active`, `inactive`, or `unresolved` effects.
Ready bonuses do not increase every ability's value. A recorded target is needed
to apply a dynamic bonus to a particular ability.

For definitions with both a surge-window and an ability-watcher modifier, Boon
infers their next-cast role from those structural fields. This is an assumption,
not engine code. Results affected by this inference have a diagnostic. In
`106996573.dem` at tick 50707, Paradox has an Arcane Surge watcher but no recorded
bonus target. The known range and duration values are partial; the extra bonus
is not assigned to all four abilities.

Unknown modifiers and unresolved activation produce `partial` known subtotals.
Missing required inputs raise `CalculationError` with `strict=True`. With
`strict=False`, affected values are null and have status `unresolved`.
Invalid selections and missing ticks always raise errors.

This API does not calculate the individual cast range, radius, debuff duration,
summon lifetime, or cooldown in seconds. Those properties can have different base
values, upgrades, and scaling rules. Nor does a query at the current tick recreate
the stat values used by a cast that started earlier.

## Rust

```rust
use boon::{Parser, ability_stats::{AbilityStat, AbilityStatQuery, AbilityRuleset, ImbueQuery}, hero_stats::StatCatalog};

fn main() -> Result<(), Box<dyn std::error::Error>> {
let parser = Parser::from_file(std::path::Path::new("106996573.dem"))?;
let catalog = StatCatalog::load("6694")?;
let imbues = parser.imbues(&ImbueQuery::new([50707]), &catalog)?;
let stats = [AbilityStat::CooldownReduction, AbilityStat::DurationBonus,
    AbilityStat::RangeBonus, AbilityStat::RadiusBonus];
let rules = stats.iter().fold(AbilityRuleset::new(), |r, s| r.with(s.rule()));
let query = AbilityStatQuery::new([50707], stats).explain(true).strict(false);
let result = parser.calculate_ability_stats(&query, &catalog, &rules)?;
    Ok(())
}
```

Use `StatCatalog::from_directory()` for a local catalog under development. It does
not download files or verify release checksums. Reuse a loaded catalog for queries.
