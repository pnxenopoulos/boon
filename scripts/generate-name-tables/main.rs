//! Run: cargo run --manifest-path scripts/generate-name-tables/Cargo.toml
//!
//! Generates crates/boon/src/abilities.rs from abilities.vdata and
//! crates/boon/src/modifiers.rs from several vdata files.
//!
//! These files come from Deadlock's VPK game data, extracted using
//! Source2Viewer (ValveResourceFormat). They use Valve's KV3 format — a
//! non-standard JSON-like structure where top-level keys (indented one tab)
//! are identifiers.
//!
//! Source 2 uses CUtlStringToken (MurmurHash2 with seed 0x31415926) for both
//! ability subclass IDs and modifier subclass IDs.
//!
//! Ability names are the top-level keys of abilities.vdata. Modifier names
//! come from two places: modifiers.vdata holds the generic/global modifiers
//! (every top-level key and `_my_subclass_name` there is a modifier), while
//! the bulk of gameplay modifiers are defined as nested `subclass:` blocks
//! inside abilities.vdata, npc_units.vdata and misc.vdata. A nested block is
//! a modifier only when its `_class` starts with `modifier_` — the same
//! `_my_subclass_name` field is also used for scale-functions and other
//! subclass types, which must be excluded.

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

/// Extract `_my_subclass_name` values from nested modifier definitions.
///
/// Matches lines like `\t\t\t_my_subclass_name = "name"` at any nesting depth.
fn extract_subclass_names(content: &str) -> Vec<&str> {
    let mut names = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("_my_subclass_name") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim();
                // Strip surrounding quotes
                if let Some(name) = rest.strip_prefix('"')
                    && let Some(name) = name.strip_suffix('"')
                    && !name.is_empty()
                {
                    names.push(name);
                }
            }
        }
    }

    names
}

/// Extract modifier `_my_subclass_name` values from nested `subclass:` blocks.
///
/// Unlike [`extract_subclass_names`], this only keeps subclasses whose
/// `_class` starts with `modifier_` (e.g. `modifier_base`,
/// `modifier_intrinsic_base`, `modifier_slow_base`). The `_my_subclass_name`
/// field is also used for scale-functions and other subclass types, so files
/// like abilities.vdata mix modifier and non-modifier subclasses — the
/// `_class` is the only reliable discriminator. In KV3 the `_class` line
/// always precedes its sibling `_my_subclass_name`.
fn extract_modifier_subclass_names(content: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut current_is_modifier = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Track the enclosing subclass's `_class`.
        if let Some(rest) = trimmed.strip_prefix("_class") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let value = rest.trim().trim_matches('"');
                current_is_modifier = value.starts_with("modifier_");
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("_my_subclass_name") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim();
                if let Some(name) = rest.strip_prefix('"')
                    && let Some(name) = name.strip_suffix('"')
                    && !name.is_empty()
                    && current_is_modifier
                {
                    names.push(name);
                }
            }
            // Reset so a modifier `_class` doesn't leak onto a sibling
            // subclass that lacks its own `_class` line.
            current_is_modifier = false;
        }
    }

    names
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

fn main() {
    let today = chrono_free_today();

    // --- abilities.vdata → abilities.rs ---
    let abilities_path = Path::new("abilities.vdata");
    let abilities_output = Path::new("crates/boon/src/abilities.rs");

    if abilities_path.exists() {
        let content = fs::read_to_string(abilities_path).expect("failed to read abilities.vdata");
        let names = extract_top_level_keys(&content);
        eprintln!(
            "Extracted {} ability names from {}",
            names.len(),
            abilities_path.display()
        );

        let mut entries: Vec<(u32, &str)> = names
            .iter()
            .map(|&name| (murmur_hash2(name.as_bytes()), name))
            .collect();
        entries.sort_by_key(|&(h, _)| h);

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
        eprintln!(
            "abilities.vdata not found at {} (skipping)",
            abilities_path.display()
        );
    }

    // --- modifiers → modifiers.rs ---
    //
    // modifiers.vdata holds the generic/global modifiers (every top-level key
    // and subclass name there is a modifier). The bulk of gameplay modifiers
    // are nested `subclass:` blocks inside these other files, kept only when
    // their `_class` starts with `modifier_`.
    const MODIFIER_SUBCLASS_FILES: &[&str] = &["abilities.vdata", "npc_units.vdata", "misc.vdata"];

    let modifiers_path = Path::new("modifiers.vdata");
    let modifiers_output = Path::new("crates/boon/src/modifiers.rs");

    if !modifiers_path.exists() {
        eprintln!("modifiers.vdata not found at {}", modifiers_path.display());
        eprintln!("Run this from the repo root.");
        if !abilities_path.exists() {
            std::process::exit(1);
        }
        return;
    }

    let modifiers_content =
        fs::read_to_string(modifiers_path).expect("failed to read modifiers.vdata");

    // Read the auxiliary files up front so their contents outlive the
    // borrowed `&str` names below.
    let aux_contents: Vec<(&str, String)> = MODIFIER_SUBCLASS_FILES
        .iter()
        .filter_map(|&file| {
            let path = Path::new(file);
            if path.exists() {
                Some((
                    file,
                    fs::read_to_string(path).expect("failed to read vdata"),
                ))
            } else {
                eprintln!("{file} not found (skipping its modifier subclasses)");
                None
            }
        })
        .collect();

    // Collect candidate names in priority order, then deduplicate.
    let mut seen = HashSet::new();
    let mut all_names: Vec<&str> = Vec::new();

    let top_level = extract_top_level_keys(&modifiers_content);
    let subclass = extract_subclass_names(&modifiers_content);
    eprintln!(
        "Extracted {} top-level + {} subclass names from modifiers.vdata",
        top_level.len(),
        subclass.len()
    );
    for name in top_level.iter().chain(subclass.iter()) {
        if seen.insert(*name) {
            all_names.push(name);
        }
    }

    for (file, content) in &aux_contents {
        let names = extract_modifier_subclass_names(content);
        let before = all_names.len();
        for name in &names {
            if seen.insert(*name) {
                all_names.push(name);
            }
        }
        eprintln!(
            "Extracted {} modifier subclass names from {file} ({} new)",
            names.len(),
            all_names.len() - before
        );
    }

    let mut entries: Vec<(u32, &str)> = all_names
        .iter()
        .map(|&name| (murmur_hash2(name.as_bytes()), name))
        .collect();
    entries.sort_by_key(|&(h, _)| h);

    write_hash_table(
        modifiers_output,
        &entries,
        "modifiers.vdata + nested subclasses in abilities/npc_units/misc.vdata",
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
