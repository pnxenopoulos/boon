//! Run: cargo run --manifest-path scripts/generate-name-tables/Cargo.toml
//!
//! Generates crates/boon/src/abilities.rs and crates/boon/src/modifiers.rs
//! from Deadlock's abilities.vdata and modifiers.vdata.
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
//! Ability names are the top-level keys of abilities.vdata.
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

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;

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
    let mut stack: Vec<(bool, Option<&str>)> = Vec::new();
    let mut names = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        match trimmed {
            "{" => stack.push((false, None)),
            "}" | "}," => {
                if let Some((is_modifier, Some(name))) = stack.pop()
                    && is_modifier
                {
                    names.push(name);
                }
            }
            _ => {
                if let Some(value) = kv3_string_value(trimmed, "_class")
                    && let Some(scope) = stack.last_mut()
                {
                    scope.0 = value.starts_with("modifier_");
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

    let abilities_path = Path::new("abilities.vdata");
    let modifiers_path = Path::new("modifiers.vdata");
    let abilities_output = Path::new("crates/boon/src/abilities.rs");
    let modifiers_output = Path::new("crates/boon/src/modifiers.rs");

    // Read both vdata files up front so their contents outlive the borrowed
    // `&str` names taken below.
    let abilities_content = read_optional_vdata(abilities_path);
    let modifiers_content = read_optional_vdata(modifiers_path);

    // --- abilities.vdata → abilities.rs ---
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
    } else {
        eprintln!("abilities.vdata not found (skipping abilities.rs)");
    }

    // --- modifiers → modifiers.rs ---
    //
    // The modifier table is the union of three vdata-derived sources:
    //   1. modifiers.vdata top-level keys      (generic/global modifiers)
    //   2. modifiers.vdata nested `_my_subclass_name` values
    //   3. modifier subclasses nested in abilities.vdata (those whose `_class`
    //      starts with `modifier_`).
    if abilities_content.is_none() && modifiers_content.is_none() {
        eprintln!(
            "No modifier sources found: need modifiers.vdata and/or abilities.vdata at the repo root."
        );
        eprintln!("Run this from the repo root (see scripts/sync-name-tables.sh).");
        std::process::exit(1);
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
        "Extracted {} modifier subclass names from abilities.vdata ({} new)",
        ability_modifiers.len(),
        all_names.len() - before
    );

    let entries = hash_entries(&all_names);

    write_hash_table(
        modifiers_output,
        &entries,
        "modifiers.vdata (top-level keys + nested subclasses) + modifier subclasses in abilities.vdata",
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
