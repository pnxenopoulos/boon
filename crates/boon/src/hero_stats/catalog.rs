use super::CalculationError;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashMap, fs, path::Path};

type Result<T> = std::result::Result<T, CalculationError>;

#[derive(Debug, Deserialize)]
pub(super) struct CatalogFile {
    catalog: String,
    pub client_version: String,
    pub source_commit: String,
    records: Vec<Record>,
    #[serde(default)]
    modifier_value_types: HashMap<u32, String>,
    #[serde(default)]
    modifier_states: HashMap<u32, String>,
    #[serde(default)]
    generic_data: Value,
}

#[derive(Debug, Deserialize)]
pub(super) struct Record {
    pub record_key: String,
    pub definition_path: String,
    pub definition: Value,
    pub stat_changes: Vec<Value>,
    pub hero_id: Option<i64>,
    pub ability_id: Option<u32>,
    pub ability_name: Option<String>,
    pub display_name: Option<String>,
    pub modifier_id: Option<u32>,
    pub qualified_modifier_id: Option<u32>,
    pub misc_id: Option<u32>,
}

impl Record {
    /// Borrow the primary weapon block and retain its path for explanations.
    pub(super) fn weapon_info(&self) -> (&Value, &'static str) {
        if let Some(infos) = self.definition.get("m_mapWeaponInfos") {
            (&infos["primary"], "m_mapWeaponInfos/primary")
        } else {
            (&self.definition["m_WeaponInfo"], "m_WeaponInfo")
        }
    }

    pub(super) fn upgrades_property(&self, property: &str) -> bool {
        self.definition["m_vecAbilityUpgrades"]
            .as_array()
            .is_some_and(|tiers| {
                tiers.iter().any(|tier| {
                    tier["m_vecPropertyUpgrades"]
                        .as_array()
                        .is_some_and(|changes| {
                            changes
                                .iter()
                                .any(|change| change["m_strPropertyName"] == property)
                        })
                })
            })
    }

    pub(super) fn is_intrinsic_modifier_of(&self, ability: &Self) -> bool {
        self.definition_path
            .strip_prefix(&ability.definition_path)
            .and_then(|path| path.strip_prefix("/m_AutoIntrinsicModifiers/"))
            .is_some_and(|index| index.parse::<usize>().is_ok())
    }

    pub(super) fn is_effect_modifier_of(&self, ability: &Self) -> bool {
        self.definition_path
            .strip_prefix(&ability.definition_path)
            .and_then(|path| path.strip_prefix('/'))
            .is_some_and(|path| {
                !path
                    .split('/')
                    .any(|part| part == "m_AutoIntrinsicModifiers")
            })
    }
}

/// Parsed definitions from one boon-data snapshot. Reuse across queries.
#[derive(Debug)]
pub struct StatCatalog {
    pub data_version: String,
    pub snapshot_version: String,
    pub source_commit: String,
    pub(super) heroes: HashMap<i64, Record>,
    pub(super) abilities: HashMap<u32, Record>,
    pub(super) ability_names: HashMap<String, u32>,
    pub(super) modifier_value_types: HashMap<u32, String>,
    pub(super) modifier_states: HashMap<String, u32>,
    pub(super) modifiers: Vec<Record>,
    modifier_ids: HashMap<u32, Vec<usize>>,
    conditional_modifiers: HashMap<u32, Option<usize>>,
    pub(super) misc: HashMap<u32, Record>,
    pub(super) generic_data: Value,
}

impl StatCatalog {
    /// Read a verified version installed by `boon get`, downloading it if missing.
    ///
    /// # Errors
    /// Returns an error for unavailable versions, invalid downloads or catalogs.
    pub fn load(version: &str) -> Result<Self> {
        let directory = crate::data::catalog_dir(Some(version))?;
        let mut catalog = Self::from_directory(&directory)?;
        catalog.data_version = version.into();
        Ok(catalog)
    }

    /// Read local catalogs. No network or checksum validation is performed.
    /// All four files must come from the same snapshot and contain stat lookups.
    pub fn from_directory(directory: &Path) -> Result<Self> {
        let read = |name: &str| -> Result<CatalogFile> {
            let bytes = fs::read(directory.join(format!("{name}.json")))?;
            let file: CatalogFile = serde_json::from_slice(&bytes).map_err(|e| CalculationError::Invalid(format!(
                "{name}.json lacks usable stat definitions: {e}; check `boon versions` and install a catalog with `boon get VERSION --force`"
            )))?;
            if file.catalog != name {
                return Err(CalculationError::Invalid(format!(
                    "expected {name} catalog"
                )));
            }
            Ok(file)
        };
        let heroes = read("heroes")?;
        let abilities = read("abilities")?;
        let mut modifiers = read("modifiers")?;
        let misc = read("misc")?;
        for file in [&abilities, &modifiers, &misc] {
            if file.client_version != heroes.client_version
                || file.source_commit != heroes.source_commit
            {
                return Err(CalculationError::Invalid(
                    "catalogs belong to different snapshots".into(),
                ));
            }
        }
        let mut modifier_ids: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, record) in modifiers.records.iter().enumerate() {
            for id in [record.modifier_id, record.qualified_modifier_id]
                .into_iter()
                .flatten()
            {
                let indices = modifier_ids.entry(id).or_default();
                if !indices.contains(&i) {
                    indices.push(i);
                }
            }
        }
        let ability_names = abilities
            .records
            .iter()
            .filter_map(|r| Some((r.ability_name.clone()?, r.ability_id?)))
            .collect();
        let modifier_value_types = abilities.modifier_value_types;
        let mut abilities = indexed(abilities.records, |r| r.ability_id)?;
        bind_undeclared_evasion(&mut abilities, &mut modifiers.records);
        // Assumption: a unique non-intrinsic modifier can activate its owner's
        // unbound conditional properties. VData does not prove this link. Keep
        // ambiguous owners out; the resolver labels uses of this index as inferred.
        let mut conditional_modifiers = HashMap::new();
        for (i, record) in modifiers.records.iter().enumerate() {
            let Some(ability_id) = record.ability_id else {
                continue;
            };
            let Some(owner) = abilities.get(&ability_id) else {
                continue;
            };
            if record.is_effect_modifier_of(owner) {
                conditional_modifiers
                    .entry(ability_id)
                    .and_modify(|candidate| *candidate = None)
                    .or_insert(Some(i));
            }
        }
        Ok(Self {
            data_version: heroes.client_version.clone(),
            snapshot_version: heroes.client_version,
            source_commit: heroes.source_commit,
            heroes: indexed(heroes.records, |r| r.hero_id)?,
            abilities,
            ability_names,
            modifier_value_types,
            modifier_states: modifiers
                .modifier_states
                .into_iter()
                .map(|(id, name)| (name, id))
                .collect(),
            modifiers: modifiers.records,
            modifier_ids,
            conditional_modifiers,
            misc: indexed(misc.records, |r| r.misc_id)?,
            generic_data: misc.generic_data,
        })
    }

    pub(super) fn weapon(&self, hero: &Record) -> Result<&Record> {
        let name = hero
            .definition
            .pointer("/m_mapBoundAbilities/ESlot_Weapon_Primary")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CalculationError::Invalid("hero has no primary weapon definition".into())
            })?;
        self.ability_names
            .get(name)
            .and_then(|id| self.abilities.get(id))
            .ok_or_else(|| CalculationError::Invalid(format!("missing primary weapon {name}")))
    }

    pub(super) fn conditional_modifier(&self, ability: u32) -> Option<&Record> {
        let index = self
            .conditional_modifiers
            .get(&ability)
            .copied()
            .flatten()?;
        Some(&self.modifiers[index])
    }

    pub(super) fn modifier(&self, id: u32, ability: Option<u32>) -> Result<&Record> {
        let candidates = self
            .modifier_ids
            .get(&id)
            .ok_or_else(|| CalculationError::Invalid(format!("unresolved modifier ID {id}")))?;
        let mut records = candidates.iter().map(|&i| &self.modifiers[i]).filter(|r| {
            ability.is_none_or(|a| a == 0 || r.ability_id.is_none() || r.ability_id == Some(a))
        });
        let record = records.next().ok_or_else(|| {
            CalculationError::Invalid(format!("modifier {id} has no matching owner"))
        })?;
        if records.next().is_some() {
            return Err(CalculationError::Invalid(format!(
                "ambiguous modifier ID {id}; an owner is required"
            )));
        }
        Ok(record)
    }
}

/// V1 recognizes the exact EvasionPercent property when VData omits its type.
/// Values and upgrades stay in the catalog; this is not a substring/name search.
fn bind_undeclared_evasion(abilities: &mut HashMap<u32, Record>, modifiers: &mut [Record]) {
    for (&id, ability) in abilities {
        let Some(property) = ability
            .definition
            .pointer("/m_mapAbilityProperties/EvasionPercent")
        else {
            continue;
        };
        // Explicit declarations and bindings always take priority.
        if property.get("m_eProvidedPropertyType").is_some()
            || ability
                .stat_changes
                .iter()
                .any(|e| e["property_name"] == "EvasionPercent")
            || modifiers.iter().any(|m| {
                m.ability_id == Some(id)
                    && m.stat_changes
                        .iter()
                        .any(|e| e["property_name"] == "EvasionPercent")
            })
        {
            continue;
        }
        let mut effect = json!({
            "stat": "MODIFIER_VALUE_BULLET_EVASION",
            "property_name": "EvasionPercent",
            "kind": "ability_property",
            "value": number(&property["m_strValue"]),
            "raw_value": property["m_strValue"],
            "scaling": property["m_subclassScaleFunction"],
            "usage_flags": "ConditionallyApplied",
            "definition_path": format!("{}/m_mapAbilityProperties/EvasionPercent", ability.definition_path),
            "modifier_keys": [],
        });
        let mut candidates = modifiers.iter().enumerate().filter(|(_, modifier)| {
            modifier.ability_id == Some(id)
                && modifier.is_effect_modifier_of(ability)
                && !modifier
                    .definition_path
                    .split('/')
                    .any(|part| part == "m_AutoCastDelayModifier")
        });
        let first = candidates.next().map(|(index, _)| index);
        let unique = candidates.next().is_none();
        if let Some(index) = first.filter(|_| unique) {
            // Assumption: the owner's unique effect modifier activates this
            // property. Cast-delay and intrinsic modifiers do not establish it.
            // Keep the inferred kind so the resolver reports a partial result.
            let modifier = &mut modifiers[index];
            let mut bound = effect.clone();
            bound["kind"] = json!("inferred_property");
            modifier.stat_changes.push(bound);
            effect["modifier_keys"] = json!([modifier.record_key]);
        }
        // With no unique modifier, retain a declared but unbound contribution.
        // The resolver reports that missing activation instead of guessing.
        ability.stat_changes.push(effect);
    }
}

fn indexed<K: Eq + std::hash::Hash>(
    records: Vec<Record>,
    key: impl Fn(&Record) -> Option<K>,
) -> Result<HashMap<K, Record>> {
    let mut result = HashMap::new();
    for record in records {
        if let Some(id) = key(&record)
            && result.insert(id, record).is_some()
        {
            return Err(CalculationError::Invalid("duplicate catalog ID".into()));
        }
    }
    Ok(result)
}

pub(super) fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|v| v.is_finite())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use serde_json::json;

    pub fn fixture() -> tempfile::TempDir {
        let folder = tempfile::tempdir().unwrap();
        let records = [
            (
                "heroes",
                json!([{"record_key":"heroes#/invented", "definition_path":"/invented", "hero_id":999,
                "definition":{"m_mapBoundAbilities":{"ESlot_Weapon_Primary":"test_gun"}, "m_mapScalingStats":{"EClipSize":{"eScalingStat":"ETechPower","flScale":0.75}}}, "stat_changes":[]}]),
            ),
            (
                "abilities",
                json!([{"record_key":"abilities#/test_gun", "definition_path":"/test_gun", "ability_id":123,
                "ability_name":"test_gun", "definition":{"m_WeaponInfo":{"m_iClipSize":20}}, "stat_changes":[]}]),
            ),
            (
                "modifiers",
                json!([
                    {"record_key":"misc#/pickup", "definition_path":"/pickup", "modifier_id":5, "qualified_modifier_id":10,
                        "definition":{}, "stat_changes":[{"stat":"MODIFIER_VALUE_AMMO_CLIP_SIZE_PERCENT","value":4}]},
                    {"record_key":"abilities#/first", "definition_path":"/first", "modifier_id":6, "qualified_modifier_id":11, "ability_id":123,
                        "definition":{}, "stat_changes":[]},
                    {"record_key":"abilities#/second", "definition_path":"/second", "modifier_id":6, "qualified_modifier_id":12, "ability_id":456,
                        "definition":{}, "stat_changes":[]}
                ]),
            ),
            ("misc", json!([])),
        ];
        for (name, records) in records {
            fs::write(folder.path().join(format!("{name}.json")), serde_json::to_vec(&json!({"catalog":name,"client_version":"12345","source_commit":"test-commit","records":records})).unwrap()).unwrap();
        }
        folder
    }

    #[test]
    fn arbitrary_hero_and_weapon_values_come_from_catalog() {
        let folder = fixture();
        let first = StatCatalog::from_directory(folder.path()).unwrap();
        assert_eq!(
            first.weapon(&first.heroes[&999]).unwrap().definition["m_WeaponInfo"]["m_iClipSize"],
            20
        );
        assert_eq!(
            first.heroes[&999].definition["m_mapScalingStats"]["EClipSize"]["flScale"],
            0.75
        );
        let path = folder.path().join("abilities.json");
        let mut json: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        json["records"][0]["definition"]["m_WeaponInfo"]["m_iClipSize"] = json!(31);
        fs::write(path, serde_json::to_vec(&json).unwrap()).unwrap();
        let second = StatCatalog::from_directory(folder.path()).unwrap();
        assert_eq!(
            second.weapon(&second.heroes[&999]).unwrap().definition["m_WeaponInfo"]["m_iClipSize"],
            31
        );
    }

    #[test]
    fn source_ids_disambiguate_without_numeric_stat_enums() {
        let folder = fixture();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert_eq!(
            catalog.modifier(10, None).unwrap().stat_changes[0]["stat"],
            "MODIFIER_VALUE_AMMO_CLIP_SIZE_PERCENT"
        );
        assert!(catalog.modifier(6, None).is_err());
        assert_eq!(
            catalog
                .modifier(6, Some(456))
                .unwrap()
                .qualified_modifier_id,
            Some(12)
        );
        assert!(catalog.modifier(123456, None).is_err());
    }

    #[test]
    fn rejects_mixed_snapshots_and_old_catalogs() {
        let folder = fixture();
        let path = folder.path().join("misc.json");
        fs::write(
            &path,
            r#"{"catalog":"misc","client_version":"1","source_commit":"other","records":[]}"#,
        )
        .unwrap();
        assert!(
            StatCatalog::from_directory(folder.path())
                .unwrap_err()
                .to_string()
                .contains("different snapshots")
        );
        fs::write(folder.path().join("heroes.json"), r#"{"catalog":"heroes","client_version":"12345","source_commit":"test-commit","records":[{"hero_id":999,"definition":{}}]}"#).unwrap();
        let error = StatCatalog::from_directory(folder.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("boon versions") && error.contains("boon get"));
    }
}
