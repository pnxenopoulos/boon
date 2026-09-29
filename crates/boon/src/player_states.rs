//! Named, recorded player states. No stat equations or mask precedence are inferred.
use crate::entity::field_path::FieldPath;
use crate::{Context, Entity, FieldValue, Parser, Serializer};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::Arc,
};

/// Errors from reading a state catalog or querying a replay.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error(transparent)]
    Data(#[from] crate::data::DataError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Parse(#[from] crate::Error),
    #[error("{0}")]
    Invalid(String),
}
type Result<T> = std::result::Result<T, StateError>;

/// State names from one boon-data snapshot. Reuse this catalog across queries.
#[derive(Debug)]
pub struct StateCatalog {
    pub client_version: String,
    pub source_commit: String,
    names: HashMap<u32, Arc<str>>,
}

impl StateCatalog {
    /// Read a verified installation, downloading the requested version if absent.
    ///
    /// # Errors
    /// Returns an error if the download, catalog, or state mapping is unavailable.
    pub fn load(version: &str) -> Result<Self> {
        Self::from_directory(&crate::data::catalog_dir(Some(version))?)
    }

    /// Read `modifiers.json` from a local directory without downloading or verifying it.
    ///
    /// # Errors
    /// Returns an error for unreadable data or a missing or invalid state mapping.
    pub fn from_directory(directory: &Path) -> Result<Self> {
        Self::from_bytes(&fs::read(directory.join("modifiers.json"))?)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct File {
            catalog: String,
            client_version: String,
            source_commit: String,
            #[serde(default)]
            modifier_states: HashMap<u32, String>,
        }
        let file: File = serde_json::from_slice(bytes)?;
        if file.catalog != "modifiers" || file.modifier_states.is_empty() {
            return Err(StateError::Invalid("modifiers.json has no state mapping; use `boon versions` and `boon get VERSION` to install a catalog that includes modifier states (use --force to replace a local copy)".into()));
        }
        let mut names = HashMap::with_capacity(file.modifier_states.len());
        let mut unique = HashSet::new();
        for (index, name) in file.modifier_states {
            let short = name
                .strip_prefix("MODIFIER_STATE_")
                .filter(|s| {
                    !s.is_empty()
                        && s.bytes()
                            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
                })
                .filter(|s| !matches!(*s, "COUNT" | "INVALID"))
                .ok_or_else(|| {
                    StateError::Invalid(format!("invalid modifier state name: {name}"))
                })?;
            let short: Arc<str> = short.into();
            if !unique.insert(Arc::clone(&short)) {
                return Err(StateError::Invalid(format!(
                    "duplicate modifier state name: {name}"
                )));
            }
            names.insert(index, short);
        }
        Ok(Self {
            client_version: file.client_version,
            source_commit: file.source_commit,
            names,
        })
    }

    fn decode(&self, words: &[u32]) -> DecodedStates {
        let mut names = Vec::new();
        let mut unknown = Vec::new();
        for (word, &value) in words.iter().enumerate() {
            let mut remaining = value;
            while remaining != 0 {
                // Field-path array indices are u8, so this index fits in u32.
                let index = (word as u32) * u32::BITS + remaining.trailing_zeros();
                if let Some(name) = self.names.get(&index) {
                    names.push(Arc::clone(name));
                } else {
                    unknown.push(index);
                }
                remaining &= remaining - 1;
            }
        }
        names.sort_unstable();
        DecodedStates { names, unknown }
    }
}

/// A decoded mask. Unknown values are bit indices, not modifier IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedStates {
    pub names: Vec<Arc<str>>,
    pub unknown: Vec<u32>,
}

/// One player at one exact tick. `None` means the mask was not available.
#[derive(Clone, Debug)]
pub struct PlayerStateRow {
    pub tick: i32,
    pub steam_id: Option<u64>,
    pub player_slot: u32,
    pub hero_id: i64,
    /// Recorded predicted-state mask. This is not a prediction made by Boon.
    pub states: Option<Arc<DecodedStates>>,
    pub enabled_states: Option<Arc<DecodedStates>>,
    pub disabled_states: Option<Arc<DecodedStates>>,
}

/// An optional selection of exact ticks and Steam IDs. Defaults to all players and ticks.
#[derive(Clone, Debug, Default)]
pub struct PlayerStateQuery {
    ticks: Option<Vec<i32>>,
    steam_ids: Option<HashSet<u64>>,
}

impl PlayerStateQuery {
    #[must_use]
    pub fn ticks(mut self, ticks: impl IntoIterator<Item = i32>) -> Self {
        self.ticks = Some(ticks.into_iter().collect());
        self
    }
    #[must_use]
    pub fn steam_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.steam_ids = Some(ids.into_iter().collect());
        self
    }
}

/// Cached wire keys for a fixed array. Its length comes from the demo schema.
struct MaskKeys(Vec<u64>);
impl MaskKeys {
    fn resolve(serializer: &Serializer, path: &str) -> Option<Self> {
        let mut fp = FieldPath::unpack(serializer.resolve_field_key(path)?);
        let indices = fp.data.get(..=fp.last)?;
        let mut field = serializer
            .fields
            .get(usize::from(*indices.first()?))?
            .as_ref();
        for &index in &indices[1..] {
            field = field.get_child(usize::from(index))?;
        }
        let length = field.field_type.array_length?;
        if length == 0 || length > usize::from(u8::MAX) + 1 {
            return None;
        }
        let next = fp.last.checked_add(1)?;
        // pbdems2's dotted-name lookup does not address fixed-array elements.
        // Append a validated wire index using its public FieldPath API instead.
        fp.data.get(next)?;
        let keys = (0..length)
            .map(|index| {
                field.get_child(index)?;
                fp.data[next] = u8::try_from(index).ok()?;
                fp.last = next;
                Some(fp.pack())
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self(keys))
    }

    fn read(&self, pawn: &Entity, words: &mut Vec<u32>) -> Option<()> {
        words.clear();
        for &key in &self.0 {
            words.push(u32::try_from(integer(pawn, Some(key))?).ok()?);
        }
        Some(())
    }
}

fn integer(entity: &Entity, key: Option<u64>) -> Option<u64> {
    match entity.fields.get(&key?)? {
        FieldValue::U32(value) => Some(u64::from(*value)),
        FieldValue::U64(value) => Some(*value),
        FieldValue::I32(value) => u64::try_from(*value).ok(),
        FieldValue::I64(value) => u64::try_from(*value).ok(),
        _ => None,
    }
}

struct Reader<'a> {
    catalog: &'a StateCatalog,
    keys: [Option<MaskKeys>; 3],
    steam: Option<u64>,
    hero: Option<u64>,
    hero_pawn: Option<u64>,
    pawn: Option<u64>,
    words: Vec<u32>,
    cache: HashMap<Vec<u32>, Arc<DecodedStates>>,
}

impl<'a> Reader<'a> {
    fn new(ctx: &Context, catalog: &'a StateCatalog) -> Self {
        let controller = ctx.serializers().get("CCitadelPlayerController");
        let key = |path| controller.and_then(|s| s.resolve_field_key(path));
        let pawn = ctx.serializers().get("CCitadelPlayerPawn");
        Self {
            catalog,
            keys: [
                "m_bvEnabledPredictedStateMask",
                "m_bvEnabledStateMask",
                "m_bvDisabledStateMask",
            ]
            .map(|name| {
                pawn.and_then(|s| MaskKeys::resolve(s, &format!("m_pModifierProp.{name}")))
            }),
            steam: key("m_steamID"),
            hero: key("m_PlayerDataGlobal.m_nHeroID"),
            hero_pawn: key("m_hHeroPawn"),
            pawn: key("m_hPawn"),
            words: Vec::new(),
            cache: HashMap::new(),
        }
    }

    fn mask(&mut self, index: usize, pawn: Option<&Entity>) -> Option<Arc<DecodedStates>> {
        self.keys[index].as_ref()?.read(pawn?, &mut self.words)?;
        // Reuse decoded names across players, ticks and mask kinds. Do not create
        // strings or a key allocation for masks already seen in this query.
        if let Some(decoded) = self.cache.get(&self.words) {
            return Some(Arc::clone(decoded));
        }
        let decoded = Arc::new(self.catalog.decode(&self.words));
        self.cache.insert(self.words.clone(), Arc::clone(&decoded));
        Some(decoded)
    }

    fn collect(
        &mut self,
        ctx: &Context,
        query: &PlayerStateQuery,
        visit: &mut impl FnMut(PlayerStateRow),
    ) {
        for (index, controller) in ctx
            .entities()
            .iter()
            .filter(|(_, e)| e.class_name.as_ref() == "CCitadelPlayerController")
        {
            let Some(slot) = index.checked_sub(1).and_then(|i| u32::try_from(i).ok()) else {
                continue;
            };
            let Some(hero_id) = integer(controller, self.hero)
                .filter(|&v| v != 0)
                .and_then(|v| i64::try_from(v).ok())
            else {
                continue;
            };
            let steam_id = integer(controller, self.steam).filter(|&v| v != 0);
            if query
                .steam_ids
                .as_ref()
                .is_some_and(|ids| steam_id.is_none_or(|id| !ids.contains(&id)))
            {
                continue;
            }
            // A dead player's m_hPawn can reference a spectator pawn. Use the
            // recorded hero handle so seeking into a death needs no earlier state.
            let pawn = controller
                .get_handle(self.hero_pawn.or(self.pawn))
                .and_then(|handle| ctx.entities().get_by_handle(handle))
                .filter(|pawn| pawn.class_name.as_ref() == "CCitadelPlayerPawn");
            visit(PlayerStateRow {
                tick: ctx.tick(),
                steam_id,
                player_slot: slot,
                hero_id,
                states: self.mask(0, pawn),
                enabled_states: self.mask(1, pawn),
                disabled_states: self.mask(2, pawn),
            });
        }
    }
}

impl Parser {
    /// Collect named states at exact ticks, or at every recorded tick.
    ///
    /// # Errors
    /// Returns an error for invalid selections or a corrupt replay.
    pub fn player_states(
        &self,
        query: &PlayerStateQuery,
        catalog: &StateCatalog,
    ) -> Result<Vec<PlayerStateRow>> {
        let mut rows = Vec::new();
        self.visit_player_states(query, catalog, |row| rows.push(row))?;
        Ok(rows)
    }

    /// Stream rows to a consumer without retaining the full-match result in Rust.
    ///
    /// # Errors
    /// Returns an error for invalid selections or a corrupt replay.
    pub fn visit_player_states(
        &self,
        query: &PlayerStateQuery,
        catalog: &StateCatalog,
        mut visit: impl FnMut(PlayerStateRow),
    ) -> Result<()> {
        if query.steam_ids.as_ref().is_some_and(|ids| ids.contains(&0)) {
            return Err(StateError::Invalid(
                "steam_ids must contain nonzero Steam IDs".into(),
            ));
        }
        let mut ticks = query.ticks.clone();
        if let Some(ticks) = &mut ticks {
            ticks.sort_unstable();
            ticks.dedup();
            let available = self.distinct_ticks()?;
            if ticks
                .iter()
                .any(|&tick| tick < 0 || available.binary_search(&tick).is_err())
            {
                return Err(StateError::Invalid(
                    "requested tick is negative or absent from the demo".into(),
                ));
            }
            if ticks.is_empty() {
                return Ok(());
            }
        }
        if query.steam_ids.as_ref().is_some_and(HashSet::is_empty) {
            return Ok(());
        }
        let initial = self.parse_init()?;
        let mut reader = Reader::new(&initial, catalog);
        let classes = HashSet::from(["CCitadelPlayerController", "CCitadelPlayerPawn"]);
        let mut seen = HashSet::new();
        let mut on_tick = |ctx: &Context| {
            if let Some(ticks) = &ticks
                && (ticks.binary_search(&ctx.tick()).is_err() || !seen.insert(ctx.tick()))
            {
                return;
            }
            reader.collect(ctx, query, &mut visit);
        };
        if let Some(ticks) = &ticks {
            if let (Some(&first), Some(&last)) = (ticks.first(), ticks.last()) {
                let start = self
                    .full_packet_offsets()?
                    .into_iter()
                    .rfind(|(_, tick)| *tick < first)
                    .map(|(offset, _)| offset);
                let end = last
                    .checked_add(1)
                    .ok_or_else(|| StateError::Invalid("tick is too large".into()))?;
                self.decode_segment(start, end, &classes, &mut on_tick)?;
            }
        } else {
            self.run_to_end_filtered(&classes, on_tick)?;
        }
        if ticks
            .as_ref()
            .is_some_and(|ticks| ticks.len() != seen.len())
        {
            return Err(StateError::Invalid(
                "requested ticks were not decoded".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{BareCharEncoding, DecodeProfile, PreciseQAngleMode};
    use crate::entity::{
        FlattenedField, FlattenedSerializer, FlattenedSerializerDefinition, SerializerContainer,
    };
    use serde_json::json;

    fn catalog(mapping: serde_json::Value) -> Result<StateCatalog> {
        StateCatalog::from_bytes(
            &serde_json::to_vec(&json!({
                "catalog":"modifiers", "client_version":"test", "source_commit":"test",
                "modifier_states":mapping,
            }))
            .unwrap(),
        )
    }

    fn serializer() -> SerializerContainer {
        SerializerContainer::parse(
            FlattenedSerializer::new(
                vec![FlattenedSerializerDefinition::new(Some(0), vec![0])],
                vec!["Test".into(), "uint32[3]".into(), "mask".into()],
                vec![FlattenedField::new(Some(1), Some(2))],
            ),
            DecodeProfile::new(BareCharEncoding::UnsignedVarint, PreciseQAngleMode::Raw),
        )
        .unwrap()
    }

    #[test]
    fn names_use_catalog_indices_and_unknown_bits_are_preserved() {
        let a = catalog(json!({"2":"MODIFIER_STATE_Z", "35":"MODIFIER_STATE_A"})).unwrap();
        let decoded = a.decode(&[4, 0x8000_0008, 1]);
        assert_eq!(
            decoded
                .names
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<&str>>(),
            ["A", "Z"]
        );
        assert_eq!(decoded.unknown, [63, 64]);
        assert!(a.decode(&[0, 0, 0]).names.is_empty());
        let b = catalog(json!({"2":"MODIFIER_STATE_OTHER"})).unwrap();
        assert_eq!(&*b.decode(&[4]).names[0], "OTHER");
    }

    #[test]
    fn invalid_and_missing_catalog_mappings_fail_clearly() {
        assert!(
            catalog(json!({}))
                .unwrap_err()
                .to_string()
                .contains("boon versions")
        );
        for invalid in [
            json!({"1":"SPRINTING"}),
            json!({"1":"MODIFIER_STATE_"}),
            json!({"1":"MODIFIER_STATE_COUNT"}),
            json!({"1":"MODIFIER_STATE_A", "2":"MODIFIER_STATE_A"}),
        ] {
            assert!(catalog(invalid).is_err());
        }
    }

    #[test]
    fn fixed_array_keys_follow_the_schema_and_missing_words_are_not_zero() {
        let serializers = serializer();
        let s = serializers.get("Test").unwrap();
        let keys = MaskKeys::resolve(s, "mask").unwrap();
        assert_eq!(keys.0.len(), 3);
        for (i, &key) in keys.0.iter().enumerate() {
            let fp = FieldPath::unpack(key);
            assert_eq!(fp.last, 1);
            assert_eq!(usize::from(fp.data[1]), i);
        }
        let mut entity = Entity::from_fields(1, 1, 0, "Test", true, Default::default()).unwrap();
        let mut words = Vec::new();
        for &key in &keys.0 {
            entity.fields.insert(key, FieldValue::U64(0));
        }
        assert!(keys.read(&entity, &mut words).is_some());
        assert_eq!(words, [0, 0, 0]);
        entity
            .fields
            .insert(keys.0[1], FieldValue::U64(u64::from(u32::MAX)));
        assert!(keys.read(&entity, &mut words).is_some());
        assert_eq!(words[1], u32::MAX);
        entity.fields.remove(&keys.0[2]);
        assert!(keys.read(&entity, &mut words).is_none());
        entity.fields.insert(keys.0[2], FieldValue::U64(u64::MAX));
        assert!(keys.read(&entity, &mut words).is_none());
        assert!(MaskKeys::resolve(s, "missing").is_none());
    }

    #[test]
    fn decoding_reuses_masks_without_merging_enabled_and_disabled_states() {
        let catalog = catalog(json!({"1":"MODIFIER_STATE_TEST"})).unwrap();
        let mut reader = Reader::new(&Context::new(1.0 / 64.0).unwrap(), &catalog);
        reader.keys = [Some(MaskKeys(vec![1])), Some(MaskKeys(vec![2])), None];
        let entity = Entity::from_fields(
            1,
            1,
            0,
            "Test",
            true,
            HashMap::from([(1, FieldValue::U32(2)), (2, FieldValue::U32(0))])
                .into_iter()
                .collect(),
        )
        .unwrap();
        let first = reader.mask(0, Some(&entity)).unwrap();
        let second = reader.mask(0, Some(&entity)).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(&*first.names[0], "TEST");
        assert!(reader.mask(1, Some(&entity)).unwrap().names.is_empty());
        assert!(reader.mask(2, Some(&entity)).is_none());
        assert!(reader.mask(0, None).is_none());
    }
}
