//! Run: cargo run --manifest-path scripts/generate-name-tables/Cargo.toml
//!
//! Generates ability/modifier/breakable names, English ability/item display
//! names, resistance inputs, and the stat effect catalog from Deadlock's
//! abilities.vdata, modifiers.vdata, heroes.vdata, misc.vdata, and English
//! localization files.
//!
//! These files come from Deadlock's VPK game data, extracted using
//! Source2Viewer (ValveResourceFormat). They use Valve's KV3 format — a
//! non-standard JSON-like structure where top-level keys (indented one tab)
//! are identifiers.
//!
//! Source 2 uses CUtlStringToken (MurmurHash2 with seed 0x31415926) for both
//! ability subclass IDs and modifier subclass IDs. In a demo a modifier is
//! identified by the `modifier_subclass` token on `CModifierTableEntry` — the
//! hash of the modifier's *subclass name*, i.e. its `_my_subclass_name` (or,
//! for the generic modifiers in modifiers.vdata, its top-level key).
//!
//! Ability names are the top-level keys of abilities.vdata. English display
//! names are the exact top-level keys shared by abilities.vdata and Valve's
//! hero/item localization catalogs. Intersecting the two sources excludes
//! description, search-alias, modifier, and stale localization tokens while
//! covering `ability_*`, `upgrade_*`, `citadel_ability_*`, and future schemes.
//!
//! The modifier name table is the union of three vdata-derived sources:
//!   1. every top-level key in modifiers.vdata — the generic/global modifiers;
//!   2. every nested `_my_subclass_name` in modifiers.vdata;
//!   3. the `_my_subclass_name` of each modifier `subclass:` block nested in
//!      abilities.vdata — those whose own `_class` starts with `modifier_`.
//!
//! For source 3, abilities.vdata interleaves modifier, scale-function and
//! ability/item subclasses — all carrying a `_my_subclass_name` — so the
//! `_class` prefix is the discriminator: modifiers are `modifier_*`,
//! scale-functions `scale_function_*`, abilities `citadel_ability_*` /
//! `citadel_item`, etc.

mod stats;

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const SEED: u32 = 0x31415926;

/// MurmurHash2 (32-bit) matching Source 2's CUtlStringToken implementation.
fn murmur_hash2(key: &[u8]) -> u32 {
    const M: u32 = 0x5BD1E995;
    const R: i32 = 24;

    let len = key.len();
    let mut h: u32 = SEED ^ (len as u32);
    let mut i = 0;

    while i + 4 <= len {
        let mut k = u32::from_le_bytes([key[i], key[i + 1], key[i + 2], key[i + 3]]);
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M);
        h ^= k;
        i += 4;
    }

    let remaining = len - i;
    if remaining >= 3 {
        h ^= (key[i + 2] as u32) << 16;
    }
    if remaining >= 2 {
        h ^= (key[i + 1] as u32) << 8;
    }
    if remaining >= 1 {
        h ^= key[i] as u32;
        h = h.wrapping_mul(M);
    }

    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

/// Parse a KV3 `key = "value"` assignment line, returning the unquoted value
/// when `line` is exactly that assignment for `key`. Returns `None` for a
/// different key or a non-string / empty value. `line` should already be
/// trimmed of surrounding whitespace.
fn kv3_string_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    let value = rest.strip_prefix('"')?.strip_suffix('"')?;
    (!value.is_empty()).then_some(value)
}

/// Extract top-level keys from a vdata file.
///
/// Matches lines like `\tkey_name = ` — one tab of indent followed by
/// a word-character key. Skips metadata keys (`generic_data_type`, `_include`).
fn extract_top_level_keys(content: &str) -> Vec<&str> {
    let skip = ["generic_data_type", "_include"];
    let mut names = Vec::new();

    for line in content.lines() {
        // Must start with exactly one tab, then a word char (not another tab)
        let Some(rest) = line.strip_prefix('\t') else {
            continue;
        };
        if rest.starts_with('\t') {
            continue;
        }

        // Find the key: contiguous word characters before whitespace/`=`
        let key_end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        if key_end == 0 {
            continue;
        }
        let key = &rest[..key_end];

        // Check that what follows is ` = ` (with optional whitespace)
        let after = rest[key_end..].trim_start();
        if !after.starts_with('=') {
            continue;
        }

        if skip.contains(&key) {
            continue;
        }

        names.push(key);
    }

    names
}

/// Extract all quoted strings from one Valve localization line.
///
/// Valve's localization format is KV1-like rather than KV3. Most lines hold a
/// quoted token and quoted value, but a few upstream lines contain more than
/// one pair. This scanner handles both cases, escaped quotes, and `//` comments
/// without treating quoted `//` text as a comment.
fn quoted_localization_values(line: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'/') {
            break;
        }
        if ch != '"' {
            continue;
        }

        let mut value = String::new();
        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    values.push(value);
                    break;
                }
                '\\' => {
                    let Some(escaped) = chars.next() else {
                        value.push('\\');
                        break;
                    };
                    match escaped {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        other => {
                            value.push('\\');
                            value.push(other);
                        }
                    }
                }
                other => value.push(other),
            }
        }
    }

    values
}

/// Join exact top-level VData keys to their English localization values.
/// Missing localization is valid for hidden, test, and retired entries, so
/// those names are omitted rather than synthesized.
fn extract_ability_display_names(
    ability_names: &[&str],
    localization_contents: &[&str],
) -> Vec<(String, String)> {
    let known_names: HashSet<&str> = ability_names.iter().copied().collect();
    let mut display_names = BTreeMap::new();

    for content in localization_contents {
        for line in content.lines() {
            let values = quoted_localization_values(line);
            for pair in values.chunks_exact(2) {
                let internal_name = &pair[0];
                let display_name = &pair[1];
                if !known_names.contains(internal_name.as_str()) || display_name.is_empty() {
                    continue;
                }

                if let Some(previous) =
                    display_names.insert(internal_name.clone(), display_name.clone())
                {
                    assert_eq!(
                        previous, *display_name,
                        "conflicting English display names for {internal_name}"
                    );
                }
            }
        }
    }

    display_names.into_iter().collect()
}

/// Extract top-level object names whose direct `_class` matches `target_class`.
///
/// `misc.vdata` is a flat catalog: each entity template is a top-level object,
/// and its discriminator is a direct child two tabs deep. Restricting the class
/// check to that depth prevents a nested child class from classifying its parent.
fn extract_top_level_keys_by_class<'a>(content: &'a str, target_class: &str) -> Vec<&'a str> {
    let mut names = Vec::new();
    let mut current_name: Option<&str> = None;
    let mut current_matches = false;

    let flush = |name: &mut Option<&'a str>, matches: &mut bool, names: &mut Vec<&'a str>| {
        if *matches
            && let Some(name) = name.take()
        {
            names.push(name);
        }
        *name = None;
        *matches = false;
    };

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix('\t')
            && !rest.starts_with('\t')
        {
            let key_end = rest
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(rest.len());
            let after = rest[key_end..].trim_start();
            if key_end > 0 && after.starts_with('=') {
                flush(&mut current_name, &mut current_matches, &mut names);
                let key = &rest[..key_end];
                if !["generic_data_type", "_include"].contains(&key) {
                    current_name = Some(key);
                }
                continue;
            }
        }

        if current_name.is_some()
            && line.starts_with("\t\t")
            && !line.starts_with("\t\t\t")
            && kv3_string_value(line.trim(), "_class") == Some(target_class)
        {
            current_matches = true;
        }
    }

    flush(&mut current_name, &mut current_matches, &mut names);
    names
}

/// Extract every `_my_subclass_name` value in a vdata file, at any nesting
/// depth and regardless of the enclosing `_class`. Used for modifiers.vdata,
/// whose nested subclasses are all modifiers.
fn extract_subclass_names(content: &str) -> Vec<&str> {
    content
        .lines()
        .filter_map(|line| kv3_string_value(line.trim(), "_my_subclass_name"))
        .collect()
}

/// Extract the `_my_subclass_name` of each modifier `subclass:` block nested in
/// abilities.vdata.
///
/// abilities.vdata interleaves three kinds of subclass — modifiers,
/// scale-functions and abilities/items — all of which carry a
/// `_my_subclass_name`, so only the modifier blocks belong in the modifier
/// table. The reliable discriminator is the block's own `_class`: modifiers are
/// `modifier_*` (`modifier_base`, `modifier_slow_base`, …), scale-functions
/// `scale_function_*`, abilities `citadel_ability_*`/`citadel_item`/….
///
/// The walk tracks object scopes by brace depth — in this KV3 text dump every
/// `{`/`}` sits alone on its line and `_class`/`_my_subclass_name` never share a
/// line with a brace — recording each scope's `_class` and `_my_subclass_name`
/// independently and emitting the name when the scope closes iff its own
/// `_class` is a modifier. Scoping per-block this way is order-independent (the
/// two fields appear in either order) and stops a modifier `_class` from leaking
/// onto a nested scale-function child or a sibling block.
fn extract_modifier_subclass_names(content: &str) -> Vec<&str> {
    // One entry per open object scope: (its `_class` is a modifier, its
    // `_my_subclass_name` if seen yet).
    // Runtime tokens may use a concrete modifier class or a named subclass.
    let mut stack: Vec<(Option<&str>, Option<&str>)> = Vec::new();
    let mut names = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        match trimmed {
            "{" => stack.push((None, None)),
            "}" | "}," => {
                if let Some((class, subclass)) = stack.pop() {
                    if let Some(class) = class.filter(|class| class.starts_with("modifier_")) {
                        names.push(class);
                        if let Some(subclass) = subclass {
                            names.push(subclass);
                        }
                    }
                }
            }
            _ => {
                if let Some(value) = kv3_string_value(trimmed, "_class")
                    && let Some(scope) = stack.last_mut()
                {
                    scope.0 = Some(value);
                } else if let Some(value) = kv3_string_value(trimmed, "_my_subclass_name")
                    && let Some(scope) = stack.last_mut()
                {
                    scope.1 = Some(value);
                }
            }
        }
    }

    names
}

/// Hash a list of names and return the (hash, name) pairs sorted by hash.
fn hash_entries<'a>(names: &[&'a str]) -> Vec<(u32, &'a str)> {
    let mut entries: Vec<(u32, &str)> = names
        .iter()
        .map(|&name| (murmur_hash2(name.as_bytes()), name))
        .collect();
    entries.sort_by_key(|&(h, _)| h);
    entries
}

/// Generate a Rust source file with a hash → name lookup function and an
/// `all_*()` function returning all entries as a static slice.
fn write_hash_table(
    output_path: &Path,
    entries: &[(u32, &str)],
    source_file: &str,
    fn_name: &str,
    all_fn_name: &str,
    not_found: &str,
    today: &str,
) {
    let mut out = fs::File::create(output_path).expect("failed to create output file");

    writeln!(
        out,
        "//! Auto-generated by scripts/generate-name-tables from {source_file}"
    )
    .unwrap();
    writeln!(
        out,
        "//! Maps MurmurHash2(name, seed=0x31415926) \u{2192} name string."
    )
    .unwrap();
    writeln!(out, "//!").unwrap();
    writeln!(out, "//! Last updated: {today}").unwrap();
    writeln!(out).unwrap();

    // Static slice for all_*()
    writeln!(out, "/// All known (hash, name) pairs sorted by hash.").unwrap();
    writeln!(out, "const ENTRIES: &[(u32, &str)] = &[").unwrap();
    for (hash, name) in entries {
        writeln!(out, "    ({hash}, \"{name}\"),").unwrap();
    }
    writeln!(out, "];").unwrap();
    writeln!(out).unwrap();

    // Lookup function
    writeln!(out, "/// Look up a name by its MurmurHash2 ID.").unwrap();
    writeln!(out, "pub fn {fn_name}(id: u32) -> &'static str {{").unwrap();
    writeln!(out, "    match id {{").unwrap();
    for (hash, name) in entries {
        writeln!(out, "        {hash} => \"{name}\",").unwrap();
    }
    writeln!(out, "        _ => \"{not_found}\",").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // all_*() function
    writeln!(out, "/// Return all known (hash, name) pairs.").unwrap();
    writeln!(
        out,
        "pub fn {all_fn_name}() -> &'static [(u32, &'static str)] {{"
    )
    .unwrap();
    writeln!(out, "    ENTRIES").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Tests
    writeln!(out, "#[cfg(test)]").unwrap();
    writeln!(out, "mod tests {{").unwrap();
    writeln!(out, "    use super::*;").unwrap();
    writeln!(out).unwrap();

    if let Some(&(first_hash, first_name)) = entries.first() {
        writeln!(out, "    #[test]").unwrap();
        writeln!(out, "    fn known_first_entry() {{").unwrap();
        writeln!(
            out,
            "        assert_eq!({fn_name}({first_hash}), \"{first_name}\");"
        )
        .unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out).unwrap();
    }

    if entries.len() > 1 {
        let mid = entries.len() / 2;
        let (mid_hash, mid_name) = entries[mid];
        writeln!(out, "    #[test]").unwrap();
        writeln!(out, "    fn known_mid_entry() {{").unwrap();
        writeln!(
            out,
            "        assert_eq!({fn_name}({mid_hash}), \"{mid_name}\");"
        )
        .unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out).unwrap();
    }

    writeln!(out, "    #[test]").unwrap();
    writeln!(out, "    fn unknown_id_zero() {{").unwrap();
    writeln!(out, "        assert_eq!({fn_name}(0), \"{not_found}\");").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "    #[test]").unwrap();
    writeln!(out, "    fn unknown_id_max() {{").unwrap();
    writeln!(
        out,
        "        assert_eq!({fn_name}(u32::MAX), \"{not_found}\");"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "    #[test]").unwrap();
    writeln!(out, "    fn {all_fn_name}_not_empty() {{").unwrap();
    writeln!(out, "        assert!(!{all_fn_name}().is_empty());").unwrap();
    writeln!(out, "    }}").unwrap();

    writeln!(out, "}}").unwrap();

    eprintln!(
        "Wrote {} with {} entries",
        output_path.display(),
        entries.len()
    );
}

/// Generate the exact internal-name → English display-name table.
fn write_ability_display_name_table(
    output_path: &Path,
    entries: &[(String, String)],
    today: &str,
) {
    let mut out = fs::File::create(output_path).expect("failed to create output file");

    writeln!(
        out,
        "//! Auto-generated by scripts/generate-name-tables from abilities.vdata"
    )
    .unwrap();
    writeln!(
        out,
        "//! and Deadlock's English hero/item localization catalogs."
    )
    .unwrap();
    writeln!(out, "//! Maps exact internal ability/item names to English display names.")
        .unwrap();
    writeln!(out, "//!").unwrap();
    writeln!(out, "//! Last updated: {today}").unwrap();
    writeln!(out).unwrap();

    writeln!(
        out,
        "/// All known (internal name, English display name) pairs sorted by internal name."
    )
    .unwrap();
    writeln!(out, "const ENTRIES: &[(&str, &str)] = &[").unwrap();
    for (internal_name, display_name) in entries {
        writeln!(out, "    ({internal_name:?}, {display_name:?}),").unwrap();
    }
    writeln!(out, "];").unwrap();
    writeln!(out).unwrap();

    writeln!(
        out,
        "/// Look up an English display name by exact internal ability/item name."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn ability_display_name(internal_name: &str) -> Option<&'static str> {{"
    )
    .unwrap();
    writeln!(out, "    match internal_name {{").unwrap();
    for (internal_name, display_name) in entries {
        writeln!(
            out,
            "        {internal_name:?} => Some({display_name:?}),"
        )
        .unwrap();
    }
    writeln!(out, "        _ => None,").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(
        out,
        "/// Return all exact internal-name to English display-name pairs."
    )
    .unwrap();
    writeln!(
        out,
        "pub fn all_ability_display_names() -> &'static [(&'static str, &'static str)] {{"
    )
    .unwrap();
    writeln!(out, "    ENTRIES").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[cfg(test)]").unwrap();
    writeln!(out, "mod tests {{").unwrap();
    writeln!(out, "    use super::*;").unwrap();
    if let Some((internal_name, display_name)) = entries.first() {
        writeln!(out).unwrap();
        writeln!(out, "    #[test]").unwrap();
        writeln!(out, "    fn known_entry() {{").unwrap();
        writeln!(
            out,
            "        assert_eq!(ability_display_name({internal_name:?}), Some({display_name:?}));"
        )
        .unwrap();
        writeln!(out, "    }}").unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out, "    #[test]").unwrap();
    writeln!(out, "    fn unknown_name() {{").unwrap();
    writeln!(
        out,
        "        assert_eq!(ability_display_name(\"not_a_real_ability\"), None);"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();

    eprintln!(
        "Wrote {} with {} localized entries",
        output_path.display(),
        entries.len()
    );
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct HeroResistanceRow {
    hero_id: i64,
    base_bullet_resist: f32,
    base_spirit_resist: f32,
    base_spirit_power: f32,
    bullet_resist_per_level: f32,
    spirit_resist_per_level: f32,
    spirit_power_per_level: f32,
    bullet_resist_per_spirit_power: f32,
    spirit_resist_per_spirit_power: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ItemResistanceRow {
    ability_id: u32,
    bullet_resist: f32,
    spirit_resist: f32,
}

impl HeroResistanceRow {
    fn has_resistance(self) -> bool {
        self.base_bullet_resist != 0.0
            || self.base_spirit_resist != 0.0
            || self.bullet_resist_per_level != 0.0
            || self.spirit_resist_per_level != 0.0
            || self.bullet_resist_per_spirit_power != 0.0
            || self.spirit_resist_per_spirit_power != 0.0
    }
}

#[derive(Default)]
struct Kv3Scope {
    name: Option<String>,
    scale: Option<f32>,
    scaling_stat: Option<String>,
    value: Option<f32>,
    provided_property_type: Option<String>,
    stats_usage_flags: Option<String>,
}

fn kv3_assignment(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    let key = key.trim().trim_matches('"');
    (!key.is_empty()).then_some((key, value.trim()))
}

/// Extract the hero inputs that affect bullet/spirit resistance.
fn extract_hero_resistances(content: &str) -> Vec<HeroResistanceRow> {
    let mut scopes: Vec<Kv3Scope> = Vec::new();
    let mut pending_scope: Option<String> = None;
    let mut current = HeroResistanceRow::default();
    let mut rows = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        match trimmed {
            "{" => {
                scopes.push(Kv3Scope {
                    name: pending_scope.take(),
                    ..Default::default()
                });
                continue;
            }
            "}" | "}," => {
                let Some(scope) = scopes.pop() else {
                    continue;
                };
                if scopes
                    .last()
                    .and_then(|parent| parent.name.as_deref())
                    == Some("m_mapScalingStats")
                    && scope.scaling_stat.as_deref() == Some("ETechPower")
                {
                    match scope.name.as_deref() {
                        Some("EBulletArmorDamageReduction") => {
                            current.bullet_resist_per_spirit_power =
                                scope.scale.unwrap_or(0.0);
                        }
                        Some("ETechArmorDamageReduction") => {
                            current.spirit_resist_per_spirit_power =
                                scope.scale.unwrap_or(0.0);
                        }
                        _ => {}
                    }
                }
                if scopes.len() == 1 && scope.name.is_some() {
                    if current.hero_id != 0 && current.has_resistance() {
                        rows.push(current);
                    }
                    current = HeroResistanceRow::default();
                }
                continue;
            }
            _ => {}
        }

        let Some((key, value)) = kv3_assignment(trimmed) else {
            continue;
        };
        if value.is_empty() {
            pending_scope = Some(key.to_string());
            continue;
        }

        if scopes.len() == 2 && key == "m_HeroID" {
            current.hero_id = value.parse().unwrap_or(0);
            continue;
        }

        match scopes.last().and_then(|scope| scope.name.as_deref()) {
            Some("m_mapStartingStats") => match key {
                "EBulletArmorDamageReduction" => {
                    current.base_bullet_resist = value.parse().unwrap_or(0.0)
                }
                "ETechArmorDamageReduction" => {
                    current.base_spirit_resist = value.parse().unwrap_or(0.0)
                }
                "ETechPower" => current.base_spirit_power = value.parse().unwrap_or(0.0),
                _ => {}
            },
            Some("m_mapStandardLevelUpUpgrades") => match key {
                "MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST" => {
                    current.bullet_resist_per_level = value.parse().unwrap_or(0.0)
                }
                "MODIFIER_VALUE_TECH_RESIST" => {
                    current.spirit_resist_per_level = value.parse().unwrap_or(0.0)
                }
                "MODIFIER_VALUE_TECH_POWER" => {
                    current.spirit_power_per_level = value.parse().unwrap_or(0.0)
                }
                _ => {}
            },
            _ => {
                if let Some(scope) = scopes.last_mut() {
                    match key {
                        "flScale" => scope.scale = value.parse().ok(),
                        "eScalingStat" => {
                            scope.scaling_stat = Some(value.trim_matches('"').to_string())
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    rows.sort_by_key(|row| row.hero_id);
    rows
}

/// Extract unconditional resistance supplied by purchasable items.
///
/// Resistance that is registered only by a conditional modifier (for example,
/// while a barrier is active) is deliberately excluded. Those effects cannot
/// be inferred from the inventory alone and must be tracked through the active
/// modifier table.
fn extract_item_resistances(content: &str) -> Vec<ItemResistanceRow> {
    let mut scopes: Vec<Kv3Scope> = Vec::new();
    let mut pending_scope: Option<String> = None;
    let mut current_name: Option<String> = None;
    let mut current_is_item = false;
    let mut current = ItemResistanceRow::default();
    let mut rows = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        match trimmed {
            "{" => {
                let name = pending_scope.take();
                if scopes.len() == 1 {
                    current_name = name.clone();
                    current_is_item = false;
                    current = ItemResistanceRow::default();
                }
                scopes.push(Kv3Scope {
                    name,
                    ..Default::default()
                });
                continue;
            }
            "}" | "}," => {
                let Some(scope) = scopes.pop() else {
                    continue;
                };

                if scopes
                    .last()
                    .and_then(|parent| parent.name.as_deref())
                    == Some("m_mapAbilityProperties")
                    && !scope
                        .stats_usage_flags
                        .as_deref()
                        .is_some_and(|flags| flags.contains("ConditionallyApplied"))
                {
                    let value = scope.value.unwrap_or(0.0);
                    match scope.provided_property_type.as_deref() {
                        Some("MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST") => {
                            current.bullet_resist += value;
                        }
                        Some("MODIFIER_VALUE_TECH_RESIST") => {
                            current.spirit_resist += value;
                        }
                        _ => {}
                    }
                }

                if scopes.len() == 1 && scope.name.is_some() {
                    if current_is_item
                        && (current.bullet_resist != 0.0 || current.spirit_resist != 0.0)
                    {
                        current.ability_id =
                            murmur_hash2(current_name.as_deref().unwrap_or_default().as_bytes());
                        rows.push(current);
                    }
                    current_name = None;
                    current_is_item = false;
                    current = ItemResistanceRow::default();
                }
                continue;
            }
            _ => {}
        }

        let Some((key, value)) = kv3_assignment(trimmed) else {
            continue;
        };
        if value.is_empty() {
            pending_scope = Some(key.to_string());
            continue;
        }

        if scopes.len() == 2 && key == "m_eAbilityType" {
            current_is_item = value.trim_matches('"') == "EAbilityType_Item";
        }

        if let Some(scope) = scopes.last_mut() {
            match key {
                "m_strValue" => {
                    scope.value = value.trim_matches('"').parse().ok();
                }
                "m_eProvidedPropertyType" => {
                    scope.provided_property_type = Some(value.trim_matches('"').to_string());
                }
                "m_eStatsUsageFlags" => {
                    scope.stats_usage_flags = Some(value.trim_matches('"').to_string());
                }
                _ => {}
            }
        }
    }

    rows.sort_by_key(|row| row.ability_id);
    rows
}

fn write_resistance_table(
    output_path: &Path,
    hero_rows: &[HeroResistanceRow],
    item_rows: &[ItemResistanceRow],
    today: &str,
) {
    let mut out = fs::File::create(output_path).expect("failed to create output file");
    writeln!(
        out,
        "//! Auto-generated by `scripts/generate-name-tables` from `heroes.vdata` and `abilities.vdata`."
    )
    .unwrap();
    writeln!(out, "//!").unwrap();
    writeln!(out, "//! Hero and equipped-item inputs used to reconstruct passive bullet and spirit resistance.").unwrap();
    writeln!(out, "//!").unwrap();
    writeln!(out, "//! Last updated: {today}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "/// Hero-defined resistance and spirit-power progression, in percentage points.").unwrap();
    writeln!(out, "#[derive(Clone, Copy, Debug, Default, PartialEq)]").unwrap();
    writeln!(out, "pub struct HeroResistanceStats {{").unwrap();
    for field in [
        "base_bullet_resist", "base_spirit_resist", "base_spirit_power",
        "bullet_resist_per_level", "spirit_resist_per_level", "spirit_power_per_level",
        "bullet_resist_per_spirit_power", "spirit_resist_per_spirit_power",
    ] {
        writeln!(out, "    pub {field}: f32,").unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "/// Return the resistance inputs for a hero.").unwrap();
    writeln!(out, "pub fn hero_resistance_stats(hero_id: i64) -> HeroResistanceStats {{").unwrap();
    writeln!(out, "    match hero_id {{").unwrap();
    for row in hero_rows {
        writeln!(out, "        {} => HeroResistanceStats {{", row.hero_id).unwrap();
        for (field, value) in [
            ("base_bullet_resist", row.base_bullet_resist),
            ("base_spirit_resist", row.base_spirit_resist),
            ("base_spirit_power", row.base_spirit_power),
            ("bullet_resist_per_level", row.bullet_resist_per_level),
            ("spirit_resist_per_level", row.spirit_resist_per_level),
            ("spirit_power_per_level", row.spirit_power_per_level),
            ("bullet_resist_per_spirit_power", row.bullet_resist_per_spirit_power),
            ("spirit_resist_per_spirit_power", row.spirit_resist_per_spirit_power),
        ] {
            if value != 0.0 {
                writeln!(out, "            {field}: {value:?},").unwrap();
            }
        }
        writeln!(out, "            ..HeroResistanceStats::default()").unwrap();
        writeln!(out, "        }},").unwrap();
    }
    writeln!(out, "        _ => HeroResistanceStats::default(),").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "/// Unconditional resistance supplied by an equipped item.").unwrap();
    writeln!(out, "#[derive(Clone, Copy, Debug, Default, PartialEq)]").unwrap();
    writeln!(out, "pub struct ItemResistanceStats {{").unwrap();
    writeln!(out, "    pub bullet_resist: f32,").unwrap();
    writeln!(out, "    pub spirit_resist: f32,").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "/// Return the unconditional resistance supplied by an item.").unwrap();
    writeln!(out, "pub fn item_resistance_stats(ability_id: u32) -> ItemResistanceStats {{").unwrap();
    writeln!(out, "    match ability_id {{").unwrap();
    for row in item_rows {
        writeln!(out, "        {} => ItemResistanceStats {{", row.ability_id).unwrap();
        if row.bullet_resist != 0.0 {
            writeln!(out, "            bullet_resist: {:?},", row.bullet_resist).unwrap();
        }
        if row.spirit_resist != 0.0 {
            writeln!(out, "            spirit_resist: {:?},", row.spirit_resist).unwrap();
        }
        if row.bullet_resist == 0.0 || row.spirit_resist == 0.0 {
            writeln!(out, "            ..ItemResistanceStats::default()").unwrap();
        }
        writeln!(out, "        }},").unwrap();
    }
    writeln!(out, "        _ => ItemResistanceStats::default(),").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    eprintln!(
        "Wrote {} with {} hero and {} item entries",
        output_path.display(),
        hero_rows.len(),
        item_rows.len()
    );
}

/// Read a vdata file if it exists, returning its contents (so the borrowed
/// `&str` names taken from it outlive their use).
fn read_optional_vdata(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    Some(fs::read_to_string(path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display())))
}

fn main() {
    let today = chrono_free_today();

    let vdata_dir = std::env::var_os("BOON_VDATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let abilities_path = vdata_dir.join("abilities.vdata");
    let modifiers_path = vdata_dir.join("modifiers.vdata");
    let heroes_path = vdata_dir.join("heroes.vdata");
    let misc_path = vdata_dir.join("misc.vdata");
    let hero_localization_path = vdata_dir.join("citadel_heroes_english.txt");
    let item_localization_path = vdata_dir.join("citadel_gc_mod_names_english.txt");
    let abilities_output = Path::new("crates/boon/src/abilities.rs");
    let ability_display_names_output = Path::new("crates/boon/src/ability_display_names.rs");
    let breakables_output = Path::new("crates/boon/src/breakables.rs");
    let modifiers_output = Path::new("crates/boon/src/modifiers.rs");
    let resistances_output = Path::new("crates/boon/src/resistances.rs");
    let stat_catalog_output = Path::new("crates/boon/src/stat_catalog.rs");

    // Read both vdata files up front so their contents outlive the borrowed
    // `&str` names taken below.
    let abilities_content = read_optional_vdata(&abilities_path);
    let modifiers_content = read_optional_vdata(&modifiers_path);
    let heroes_content = read_optional_vdata(&heroes_path);
    let misc_content = read_optional_vdata(&misc_path);
    let hero_localization_content = read_optional_vdata(&hero_localization_path);
    let item_localization_content = read_optional_vdata(&item_localization_path);

    // --- abilities.vdata → abilities.rs ---
    if std::env::var_os("BOON_STATS_ONLY").is_some() {
        let content = abilities_content
            .as_deref()
            .expect("BOON_STATS_ONLY requires abilities.vdata");
        stats::generate(content, stat_catalog_output, &today);
        return;
    }

    // Ability names are simply the top-level keys.
    if let Some(content) = &abilities_content {
        let names = extract_top_level_keys(content);
        eprintln!("Extracted {} ability names from abilities.vdata", names.len());
        let entries = hash_entries(&names);
        write_hash_table(
            abilities_output,
            &entries,
            "abilities.vdata",
            "ability_name",
            "all_abilities",
            "ABILITY_NOT_FOUND",
            &today,
        );

        let localization_contents: Vec<&str> = [
            hero_localization_content.as_deref(),
            item_localization_content.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect();
        if localization_contents.is_empty() {
            eprintln!(
                "English hero/item localization not found (skipping ability_display_names.rs)"
            );
        } else {
            let display_entries = extract_ability_display_names(&names, &localization_contents);
            eprintln!(
                "Localized {} of {} top-level ability/item names",
                display_entries.len(),
                names.len()
            );
            write_ability_display_name_table(
                ability_display_names_output,
                &display_entries,
                &today,
            );
        }
        stats::generate(content, stat_catalog_output, &today);
    } else {
        eprintln!("abilities.vdata not found (skipping abilities.rs)");
    }

    // Breakable subclass IDs are CUtlStringToken hashes of the top-level
    // misc.vdata names whose gameplay class is citadel_breakable_prop.
    if let Some(content) = &misc_content {
        let names = extract_top_level_keys_by_class(content, "citadel_breakable_prop");
        eprintln!(
            "Extracted {} breakable subclass names from misc.vdata",
            names.len()
        );
        let entries = hash_entries(&names);
        write_hash_table(
            breakables_output,
            &entries,
            "misc.vdata (`citadel_breakable_prop` entries)",
            "breakable_name",
            "all_breakables",
            "BREAKABLE_NOT_FOUND",
            &today,
        );
    } else {
        eprintln!("misc.vdata not found (skipping breakables.rs)");
    }

    // --- modifiers → modifiers.rs ---
    //
    // The modifier table is the union of three vdata-derived sources:
    //   1. modifiers.vdata top-level keys      (generic/global modifiers)
    //   2. modifiers.vdata nested `_my_subclass_name` values
    //   3. modifier subclasses nested in abilities.vdata (those whose `_class`
    //      starts with `modifier_`).
    if let (Some(hero_content), Some(ability_content)) =
        (&heroes_content, &abilities_content)
    {
        let hero_rows = extract_hero_resistances(hero_content);
        let item_rows = extract_item_resistances(ability_content);
        write_resistance_table(resistances_output, &hero_rows, &item_rows, &today);
    } else {
        eprintln!("heroes.vdata or abilities.vdata not found (skipping resistances.rs)");
    }

    if abilities_content.is_none() && modifiers_content.is_none() {
        eprintln!(
            "No modifier sources found: need modifiers.vdata and/or abilities.vdata at the repo root."
        );
        eprintln!("Run this from the repo root (see scripts/sync-name-tables.sh).");
        return;
    }

    let modifiers_str = modifiers_content.as_deref().unwrap_or_default();
    let abilities_str = abilities_content.as_deref().unwrap_or_default();

    // Collect candidate names in priority order, then deduplicate.
    let mut seen = HashSet::new();
    let mut all_names: Vec<&str> = Vec::new();

    let top_level = extract_top_level_keys(modifiers_str);
    let nested = extract_subclass_names(modifiers_str);
    eprintln!(
        "Extracted {} top-level + {} nested subclass names from modifiers.vdata",
        top_level.len(),
        nested.len()
    );
    for name in top_level.iter().chain(nested.iter()) {
        if seen.insert(*name) {
            all_names.push(name);
        }
    }

    let ability_modifiers = extract_modifier_subclass_names(abilities_str);
    let before = all_names.len();
    for name in &ability_modifiers {
        if seen.insert(*name) {
            all_names.push(name);
        }
    }
    eprintln!(
        "Extracted {} modifier class/subclass names from abilities.vdata ({} new)",
        ability_modifiers.len(),
        all_names.len() - before
    );

    let entries = hash_entries(&all_names);

    write_hash_table(
        modifiers_output,
        &entries,
        "modifiers.vdata + modifier classes/subclasses in abilities.vdata",
        "modifier_name",
        "all_modifiers",
        "MODIFIER_NOT_FOUND",
        &today,
    );
}

/// Return today's date as YYYY-MM-DD without pulling in chrono.
fn chrono_free_today() -> String {
    // Use std::process::Command to get the date
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .expect("failed to run date command");
    String::from_utf8(output.stdout)
        .expect("invalid utf8 from date")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_localization_pairs_and_escapes() {
        let line = r#"	"ability_test" "A \"Quoted\" // Name" "upgrade_test" "Line\nTwo" // "ignored" "Ignored""#;

        assert_eq!(
            quoted_localization_values(line),
            vec![
                "ability_test",
                "A \"Quoted\" // Name",
                "upgrade_test",
                "Line\nTwo",
            ]
        );
    }

    #[test]
    fn display_names_require_exact_vdata_keys() {
        let ability_names = vec![
            "ability_test",
            "upgrade_test",
            "ability_missing",
            "citadel_ability_test",
        ];
        let hero_localization = concat!(
            "	\"ability_test\" \"Hero Ability\"\n",
            "	\"ability_test_desc\" \"Not a display name\"\n",
            "	\"ability_stale\" \"No matching VData entry\"\n",
            "	\"citadel_ability_test\" \"Class-prefixed Ability\"\n",
        );
        let item_localization = "	\"upgrade_test\" \"Shop Item\"\n";

        assert_eq!(
            extract_ability_display_names(
                &ability_names,
                &[hero_localization, item_localization]
            ),
            vec![
                ("ability_test".to_string(), "Hero Ability".to_string()),
                (
                    "citadel_ability_test".to_string(),
                    "Class-prefixed Ability".to_string(),
                ),
                ("upgrade_test".to_string(), "Shop Item".to_string()),
            ]
        );
    }

    #[test]
    fn extracts_only_direct_breakable_classes() {
        let content = concat!(
            "{\n",
            "\tgeneric_data_type = \"misc\"\n",
            "\tcitadel_breakable_prop_wooden_crate =\n",
            "\t{\n",
            "\t\t_class = \"citadel_breakable_prop\"\n",
            "\t}\n",
            "\tnon_breakable_parent =\n",
            "\t{\n",
            "\t\t_class = \"some_other_class\"\n",
            "\t\tchild =\n",
            "\t\t{\n",
            "\t\t\t_class = \"citadel_breakable_prop\"\n",
            "\t\t}\n",
            "\t}\n",
            "\tvehicle_car_01 =\n",
            "\t{\n",
            "\t\t_class = \"citadel_breakable_prop\"\n",
            "\t}\n",
            "}\n",
        );

        assert_eq!(
            extract_top_level_keys_by_class(content, "citadel_breakable_prop"),
            vec!["citadel_breakable_prop_wooden_crate", "vehicle_car_01"]
        );
    }
}
