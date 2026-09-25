//! Runtime name lookups from boon-data JSON catalogs.

use crate::data::{DataError, Result};
use serde::Deserialize;
use std::{
    collections::{HashMap, hash_map::Entry},
    fs,
    hash::Hash,
    path::Path,
};

/// Names from one boon-data snapshot. Load once and reuse for per-event lookups.
#[derive(Debug, Default)]
pub struct CatalogNames {
    pub heroes: HashMap<i64, String>,
    pub abilities: HashMap<u32, String>,
    pub ability_display_names: HashMap<String, String>,
    /// Both unqualified and owner-qualified modifier tokens.
    pub modifiers: HashMap<u32, String>,
}

#[derive(Deserialize)]
struct Catalog<T> {
    catalog: String,
    client_version: String,
    source_commit: String,
    records: Vec<T>,
}

#[derive(Deserialize)]
struct Hero {
    hero_id: Option<i64>,
    hero_name: String,
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct Ability {
    ability_id: u32,
    ability_name: String,
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct Modifier {
    modifier_id: u32,
    modifier_name: String,
    qualified_modifier_id: u32,
    qualified_modifier_name: String,
}

fn read<T: serde::de::DeserializeOwned>(directory: &Path, name: &str) -> Result<Catalog<T>> {
    let catalog: Catalog<T> =
        serde_json::from_slice(&fs::read(directory.join(format!("{name}.json")))?)?;
    if catalog.catalog != name {
        return Err(DataError::Invalid(format!(
            "expected {name} catalog, found {}",
            catalog.catalog
        )));
    }
    Ok(catalog)
}

fn insert<K: Eq + Hash>(map: &mut HashMap<K, String>, key: K, name: String) -> Result<()> {
    match map.entry(key) {
        Entry::Vacant(entry) => {
            entry.insert(name);
        }
        Entry::Occupied(entry) if entry.get() != &name => {
            return Err(DataError::Invalid(format!(
                "conflicting catalog names: {} and {name}",
                entry.get()
            )));
        }
        Entry::Occupied(_) => {}
    }
    Ok(())
}

impl CatalogNames {
    /// Use the newest local data, downloading latest when none is installed.
    /// Pass a client version to select it explicitly and download it if missing.
    pub fn load(version: Option<&str>) -> Result<Self> {
        Self::from_directory(&crate::data::catalog_dir(version)?)
    }

    /// Read locally built or already verified catalogs without using the network.
    /// Files must belong to the same snapshot; checksum verification is the caller's responsibility.
    pub fn from_directory(directory: &Path) -> Result<Self> {
        let heroes: Catalog<Hero> = read(directory, "heroes")?;
        let abilities: Catalog<Ability> = read(directory, "abilities")?;
        let modifiers: Catalog<Modifier> = read(directory, "modifiers")?;
        if heroes.client_version != abilities.client_version
            || heroes.source_commit != abilities.source_commit
            || heroes.client_version != modifiers.client_version
            || heroes.source_commit != modifiers.source_commit
        {
            return Err(DataError::Invalid(
                "name catalogs belong to different snapshots".into(),
            ));
        }
        let mut names = Self::default();
        for hero in heroes.records {
            if let Some(id) = hero.hero_id {
                insert(
                    &mut names.heroes,
                    id,
                    hero.display_name
                        .filter(|name| !name.is_empty())
                        .unwrap_or(hero.hero_name),
                )?;
            }
        }
        for ability in abilities.records {
            if let Some(display) = ability.display_name.filter(|name| !name.is_empty()) {
                insert(
                    &mut names.ability_display_names,
                    ability.ability_name.clone(),
                    display,
                )?;
            }
            insert(
                &mut names.abilities,
                ability.ability_id,
                ability.ability_name,
            )?;
        }
        for modifier in modifiers.records {
            insert(
                &mut names.modifiers,
                modifier.modifier_id,
                modifier.modifier_name,
            )?;
            insert(
                &mut names.modifiers,
                modifier.qualified_modifier_id,
                modifier.qualified_modifier_name,
            )?;
        }
        Ok(names)
    }

    pub fn hero_name(&self, id: i64) -> &str {
        self.heroes
            .get(&id)
            .map_or("HERO_NOT_FOUND", String::as_str)
    }

    pub fn ability_name(&self, id: u32) -> &str {
        self.abilities
            .get(&id)
            .map_or("ABILITY_NOT_FOUND", String::as_str)
    }

    pub fn ability_display_name(&self, name: &str) -> Option<&str> {
        self.ability_display_names.get(name).map(String::as_str)
    }

    pub fn modifier_name(&self, id: u32) -> &str {
        self.modifiers
            .get(&id)
            .map_or("MODIFIER_NOT_FOUND", String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn write(directory: &Path, catalog: &str, records: Value) {
        fs::write(directory.join(format!("{catalog}.json")), serde_json::to_vec(&json!({
            "catalog": catalog, "client_version": "42", "source_commit": "a".repeat(40), "records": records,
        })).unwrap()).unwrap();
    }

    #[test]
    fn reads_ids_localization_and_qualified_modifiers_from_catalogs() {
        let directory = tempfile::tempdir().unwrap();
        write(
            directory.path(),
            "heroes",
            json!([
                {"hero_id": 123, "hero_name": "hero_new", "display_name": "New Hero"},
                {"hero_id": 456, "hero_name": "hero_hidden", "display_name": null},
                {"hero_id": null, "hero_name": "template", "display_name": null},
            ]),
        );
        write(
            directory.path(),
            "abilities",
            json!([
                {"ability_id": 123, "ability_name": "new_item", "display_name": "New Item"},
                {"ability_id": 456, "ability_name": "hidden_item", "display_name": null},
            ]),
        );
        let modifier = json!({"modifier_id": 123, "modifier_name": "buff", "qualified_modifier_id": 456, "qualified_modifier_name": "new_item/buff"});
        write(directory.path(), "modifiers", json!([modifier, modifier]));
        let names = CatalogNames::from_directory(directory.path()).unwrap();
        assert_eq!(names.hero_name(123), "New Hero");
        assert_eq!(names.hero_name(456), "hero_hidden");
        assert_eq!(names.hero_name(1), "HERO_NOT_FOUND");
        assert_eq!(names.ability_name(123), "new_item");
        assert_eq!(names.ability_display_name("new_item"), Some("New Item"));
        assert_eq!(names.ability_display_name("hidden_item"), None);
        assert_eq!(names.modifier_name(123), "buff");
        assert_eq!(names.modifier_name(456), "new_item/buff");
        assert_eq!(names.modifiers.len(), 2);
        write(
            directory.path(),
            "modifiers",
            json!([modifier, {
                "modifier_id": 123, "modifier_name": "different_buff", "qualified_modifier_id": 789, "qualified_modifier_name": "new_item/different_buff"
            }]),
        );
        assert!(
            CatalogNames::from_directory(directory.path())
                .unwrap_err()
                .to_string()
                .contains("conflicting")
        );
    }

    #[test]
    fn rejects_mixed_snapshots_and_wrong_catalog_types() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["heroes", "abilities", "modifiers"] {
            write(directory.path(), name, json!([]));
        }
        let path = directory.path().join("heroes.json");
        let original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for field in ["client_version", "source_commit", "catalog"] {
            let mut bad = original.clone();
            bad[field] = json!("different");
            fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
            assert!(CatalogNames::from_directory(directory.path()).is_err());
        }
    }
}
