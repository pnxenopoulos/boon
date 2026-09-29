# Ability stats and imbues

Use `demo.imbues()` to read which items imbue each ability.
Use `demo.calculate_ability_stats()` to calculate each ability's bonus percentages.
Both methods read replay state and a selected boon-data version.

## Python

```python
from boon import AbilityStat, Demo

version = "6694"  # Select the client version for your demo.
steam_id = 76561197999389679  # Venator in this demo.
demo = Demo("106996573.dem", preload=False)

imbues = demo.imbues(ticks=50707, steam_ids=[steam_id], data_version=version)
print(imbues.bindings)
print(imbues.effects)

result = demo.calculate_ability_stats(
    ticks=[50707, 60000],
    steam_ids=[steam_id],
    data_version=version,
    stats=[AbilityStat.COOLDOWN_REDUCTION, AbilityStat.RANGE_BONUS],
    explain=True,
    strict=False,
)
print(result.values)
print(result.contributions)
```

Use `AbilityStat` enum members in Python code. Matching strings are also accepted;
unknown names cause an error. The result's `stat` column contains strings.
Use `HeroStat` with [hero stat queries](hero-stats.md).

List versions with `boon versions`. Install a version with `boon get VERSION`.
A query downloads and verifies a missing version. It does not select another version.
Recorded dynamic values require the catalog's `modifier_value_types` map.
Use `boon get VERSION --force` after a release adds that map.

## Select ticks, players, and abilities

`ticks` accepts one integer or a list. Results describe state after each tick.
Multiple ticks use one parser pass. Missing ticks cause an error.

Use `steam_ids` to select Steam accounts. Get IDs from `demo.players`.
Omit the filter to include all players. An empty filter selects no players.
A requested Steam ID without a hero at a selected tick causes an error.

Stat queries select the hero's signature abilities by default.
Use `abilities=[ability_id, ...]` to select owned abilities or items by catalog ID.
Use `include_items=True` to add all owned items.
Each requested ability ID must belong to a selected player during the query.

## Results

| Table | Contents |
| --- | --- |
| `imbues.bindings` | One row per tick, player, source item, and imbued ability; includes IDs and names. |
| `imbues.effects` | Catalog properties, values, stat symbols, and targeting filters for each binding. No stacking rule is applied. |
| `result.values` | One row per tick, player, ability, and stat; includes the value, unit, rule, status, and diagnostic. |
| `result.contributions` | Input sources, properties, scope, activation state, and whether each input was included. Requires `explain=True`. |

Each table includes `tick`, `steam_id`, `player_slot`, and `hero_id`.
Steam IDs use UInt64. Join to `demo.players` with `steam_id`.
A missing Steam ID is null; its row keeps the raw player slot.
Do not join null Steam IDs. Summary slots can differ from controller slots.

Missing catalog records do not remove recorded imbue bindings.
A binding can have no effect rows when the catalog has no mapped stat changes.
`metadata` gives the selected version, source commit, and rule versions.

## Percentage rules

| Enum member | Rule |
| --- | --- |
| `AbilityStat.COOLDOWN_REDUCTION` | `boon.rulesets.cooldown_reduction.v1` |
| `AbilityStat.ITEM_COOLDOWN_REDUCTION` | `boon.rulesets.item_cooldown_reduction.v1` |
| `AbilityStat.DURATION_BONUS` | `boon.rulesets.duration_bonus.v1` |
| `AbilityStat.RANGE_BONUS` | `boon.rulesets.range_bonus.v1` |
| `AbilityStat.RADIUS_BONUS` | `boon.rulesets.radius_bonus.v1` |

Omit `stats` to select all except item cooldown reduction.
Use `stats=list(AbilityStat)` to include all five stats.
Omit `rulesets` to use the supported `v1` rules.
A `rulesets` mapping must supply one rule for each requested stat.

V1 applies this equation to each stat:

```text
combined = 100 * (1 - product(1 - source_percent / 100))
```

For example, 10% and 20% give 28%. Values use percentage points and are not rounded.
Negative inputs are permitted. Inputs above 100%, nonfinite values, and overflow cause errors.
These results are bonuses, not seconds, metres, or remaining cooldown times.

Ability cooldown reduction applies to hero abilities. Item cooldown reduction applies to items.
An item that disables cooldown scaling has status `not_applicable` and a null value.
Range and duration bonuses can apply to items. Range and radius remain separate stats.

For charged abilities, filters use recorded maximum charges or catalog charges with upgrades.
Ultimate-only filters select the fourth signature slot.
Charge recovery time and the delay between casts are separate properties.
An event that reduces a running cooldown does not become a permanent stat bonus.

## Sources and limits

Boon includes recorded stat totals, active modifiers, catalog effects, and ability upgrades.
It applies imbue and charge filters only to their targets.
It does not add a pickup modifier again when the recorded total includes that pickup.
A recorded dynamic value replaces its matching catalog contribution, including when the value is zero.
A bonus for one ability does not replace a bonus on another ability.

Contributions can have state `ready`, `active`, `inactive`, or `unresolved`.
A ready next-cast bonus requires a recorded target before Boon applies it to an ability.
The resolver assumes next-cast roles from catalog surge-window and ability-watcher fields.
A diagnostic identifies this assumption.

In `106996573.dem` at tick 50707, Paradox has an Arcane Surge watcher without a recorded bonus target.
Boon omits that bonus and marks the known range, radius, and duration values as `partial`.
It does not apply the bonus to all abilities.

Unknown modifiers and unsupported activation produce `partial` values with diagnostics.
Other missing inputs raise `CalculationError` with `strict=True`.
With `strict=False`, those rows have null values and status `unresolved`.
Invalid queries and failed catalog downloads still cause errors.

These methods do not calculate each ability property's final duration or distance.
A query also does not recover the inputs for an earlier cast.
See [Known Issues](known-issues.md#ability-bonuses-and-arcane-surge).

## Rust

```rust
use boon::{
    Parser,
    ability_stats::{AbilityRuleset, AbilityStat, AbilityStatQuery, ImbueQuery},
    hero_stats::StatCatalog,
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parser = Parser::from_file(Path::new("106996573.dem"))?;
    let catalog = StatCatalog::load("6694")?;
    let steam_id = 76561197999389679_u64;
    let imbues = parser.imbues(
        &ImbueQuery::new([50707]).steam_ids([steam_id]),
        &catalog,
    )?;
    let stats = [AbilityStat::CooldownReduction, AbilityStat::RangeBonus];
    let rules = stats.iter().fold(AbilityRuleset::new(), |r, s| r.with(s.rule()));
    let query = AbilityStatQuery::new([50707], stats)
        .steam_ids([steam_id])
        .explain(true)
        .strict(false);
    let result = parser.calculate_ability_stats(&query, &catalog, &rules)?;
    println!("{:?}", imbues.bindings);
    println!("{:?}", result.values);
    Ok(())
}
```

Reuse the loaded catalog for later queries.
`StatCatalog::from_directory()` reads local files without downloads or checksum checks.
