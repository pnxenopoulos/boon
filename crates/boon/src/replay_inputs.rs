//! Recorded controller upgrades and team minimap observations.
//!
//! Create readers from the replay's serializers. Recreate them if the schema
//! changes. Missing fields stay unknown; minimap coordinates use native bytes.

use std::collections::HashMap;

use crate::{
    Context, Entity, EntityContainer, EntityId, FieldValue, Serializer, SerializerContainer,
};

fn integer(value: Option<&FieldValue>) -> Option<i64> {
    match value? {
        FieldValue::I32(n) => Some(i64::from(*n)),
        FieldValue::U32(n) => Some(i64::from(*n)),
        FieldValue::I64(n) => Some(*n),
        FieldValue::U64(n) => i64::try_from(*n).ok(),
        _ => None,
    }
}

/// Read the current upgrade tokens with cached field keys and a reused buffer.
#[derive(Default)]
pub struct UpgradeReader {
    serializer: Option<Serializer>,
    count: Option<u64>,
    keys: Vec<u64>,
    values: Vec<u32>,
}

impl UpgradeReader {
    const PATH: &str = "m_PlayerDataGlobal.m_vecUpgrades";

    /// Use the player controller serializer, or `None` if it is absent.
    pub fn new(serializer: Option<&Serializer>) -> Self {
        Self {
            serializer: serializer.cloned(),
            count: serializer.and_then(|s| s.resolve_field_key(Self::PATH)),
            ..Self::default()
        }
    }

    /// Return the recorded tokens. Empty is distinct from missing or invalid.
    /// Same-length replacements are read again; values are not cached by count.
    pub fn read(&mut self, controller: &Entity) -> Option<&[u32]> {
        self.values.clear();
        let count = usize::try_from(integer(controller.field_value(self.count))?).ok()?;
        if count > self.keys.len() {
            let serializer = self.serializer.as_ref()?;
            // Check the last index before allocating. The serializer enforces
            // the packed field-path bounds, including indices above 255.
            serializer.resolve_field_key(&format!("{}.{last}", Self::PATH, last = count - 1))?;
            for index in self.keys.len()..count {
                self.keys
                    .push(serializer.resolve_field_key(&format!("{}.{index}", Self::PATH))?);
            }
        }
        for &key in &self.keys[..count] {
            self.values
                .push(u32::try_from(integer(controller.field_value(Some(key)))?).ok()?);
        }
        Some(&self.values)
    }
}

#[derive(Clone, Copy, Default)]
struct ControllerKeys {
    steam_id: Option<u64>,
    pawn: Option<u64>,
    hero_pawn: Option<u64>,
}

impl ControllerKeys {
    fn resolve(serializer: Option<&Serializer>) -> Self {
        let key = |name| serializer.and_then(|s| s.resolve_field_key(name));
        Self {
            steam_id: key("m_steamID"),
            pawn: key("m_hPawn"),
            hero_pawn: key("m_hHeroPawn"),
        }
    }
}

fn controller_steam_ids(
    entities: &EntityContainer,
    keys: ControllerKeys,
) -> HashMap<EntityId, Option<u64>> {
    let mut by_pawn = HashMap::new();
    for (_, controller) in entities
        .iter()
        .filter(|(_, e)| e.active && e.class_name.as_ref() == "CCitadelPlayerController")
    {
        let pawn = |key| {
            controller
                .get_handle(key)
                .and_then(|h| entities.get_by_handle(h))
                .filter(|e| e.active && e.class_name.as_ref() == "CCitadelPlayerPawn")
                .map(Entity::id)
        };
        let id = match (pawn(keys.pawn), pawn(keys.hero_pawn)) {
            (Some(a), Some(b)) if a != b => continue,
            (Some(id), _) | (_, Some(id)) => id,
            _ => continue,
        };
        let steam_id = match controller.field_value(keys.steam_id) {
            Some(FieldValue::U64(id)) if *id != 0 => Some(*id),
            _ => None,
        };
        by_pawn
            .entry(id)
            .and_modify(|value| *value = None)
            .or_insert(steam_id);
    }
    by_pawn
}

#[derive(Clone, Copy)]
struct EntryKeys {
    index: Option<u64>,
    team: Option<u64>,
    class: Option<u64>,
    visible: Option<u64>,
    x: Option<u64>,
    y: Option<u64>,
    hidden: Option<u64>,
}

impl EntryKeys {
    fn resolve(serializer: &Serializer, index: usize) -> Self {
        let key = |name| serializer.resolve_field_key(&format!("m_vecFOWEntities.{index}.{name}"));
        Self {
            index: key("m_nEntIndex"),
            team: key("m_nTeam"),
            class: key("m_eClass"),
            visible: key("m_bVisibleOnMap"),
            x: key("m_nPositionX"),
            y: key("m_nPositionY"),
            hidden: key("m_nTickHidden"),
        }
    }
}

/// One team's recorded observation of an enemy hero icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinimapObservation {
    /// Completed replay tick.
    pub tick: i32,
    /// Team that receives this minimap channel.
    pub observer_team: i64,
    /// Enemy team.
    pub target_team: i64,
    /// Current pawn identity, if resolved.
    pub target_entity: Option<EntityId>,
    /// Recorded hero ID, if resolved.
    pub target_hero_id: Option<i64>,
    /// Steam account from a unique current controller binding.
    pub target_steam_id: Option<u64>,
    /// Decoded FOW entity index, including unresolved values.
    pub fow_entity_index_raw: i64,
    /// Recorded icon visibility, or unknown if absent.
    pub visible_on_map: Option<bool>,
    /// Native 0–255 coordinates, only while shown.
    pub position: Option<[i64; 2]>,
    /// Recorded hidden-tick field; no time conversion is applied.
    pub tick_hidden_raw: Option<i64>,
    /// Last shown tick and coordinates for this team and pawn lifetime.
    pub last_seen: Option<(i32, [i64; 2])>,
}

/// Track enemy minimap observations without using hidden world positions.
pub struct EnemyMinimap {
    serializer: Option<Serializer>,
    team: Option<u64>,
    count: Option<u64>,
    hero: Option<u64>,
    pawn_team: Option<u64>,
    controller: ControllerKeys,
    entries: Vec<EntryKeys>,
    last_seen: HashMap<(i64, EntityId), (i32, [i64; 2])>,
    rows: Vec<MinimapObservation>,
}

impl EnemyMinimap {
    /// Resolve the replay's team, pawn, and controller field keys once.
    pub fn new(serializers: &SerializerContainer) -> Self {
        let team = serializers.get("CCitadelTeam");
        let pawn = serializers.get("CCitadelPlayerPawn");
        Self {
            serializer: team.cloned(),
            team: team.and_then(|s| s.resolve_field_key("m_iTeamNum")),
            count: team.and_then(|s| s.resolve_field_key("m_vecFOWEntities")),
            hero: pawn.and_then(|s| {
                s.resolve_field_key("m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID")
            }),
            pawn_team: pawn.and_then(|s| s.resolve_field_key("m_iTeamNum")),
            controller: ControllerKeys::resolve(serializers.get("CCitadelPlayerController")),
            entries: Vec::new(),
            last_seen: HashMap::new(),
            rows: Vec::new(),
        }
    }

    /// Read every completed tick to preserve last-seen history. Set `emit` to
    /// false to update history without producing rows for an unsampled tick.
    pub fn update(&mut self, ctx: &Context, emit: bool) -> &[MinimapObservation] {
        self.update_entities(ctx.tick(), ctx.entities(), emit)
    }

    fn update_entities(
        &mut self,
        tick: i32,
        entities: &EntityContainer,
        emit: bool,
    ) -> &[MinimapObservation] {
        self.rows.clear();
        let Some(serializer) = &self.serializer else {
            return &self.rows;
        };
        let steam_ids = emit.then(|| controller_steam_ids(entities, self.controller));
        for (_, team) in entities
            .iter()
            .filter(|(_, e)| e.active && e.class_name.as_ref() == "CCitadelTeam")
        {
            let Some(observer_team @ (2 | 3)) = integer(team.field_value(self.team)) else {
                continue;
            };
            let Some(count) =
                integer(team.field_value(self.count)).and_then(|n| usize::try_from(n).ok())
            else {
                continue;
            };
            if count > self.entries.len() {
                if serializer
                    .resolve_field_key(&format!("m_vecFOWEntities.{}.m_eClass", count - 1))
                    .is_none()
                {
                    continue;
                }
                self.entries
                    .extend((self.entries.len()..count).map(|i| EntryKeys::resolve(serializer, i)));
            }
            for key in &self.entries[..count] {
                if integer(team.field_value(key.class)) != Some(1) {
                    continue;
                }
                let Some(target_team @ (2 | 3)) = integer(team.field_value(key.team)) else {
                    continue;
                };
                if target_team == observer_team {
                    continue;
                }
                let Some(raw_index) = integer(team.field_value(key.index)) else {
                    continue;
                };
                // CEntityIndex is decoded by the game adapter. Never try other
                // indices based on which current pawns happen to exist.
                let pawn = i32::try_from(raw_index)
                    .ok()
                    .and_then(|i| entities.get(i))
                    .filter(|p| {
                        p.active
                            && p.class_name.as_ref() == "CCitadelPlayerPawn"
                            && integer(p.field_value(self.pawn_team)) == Some(target_team)
                    });
                let identity = pawn.map(Entity::id);
                let shown = match team.field_value(key.visible) {
                    Some(FieldValue::Bool(v)) => Some(*v),
                    _ => None,
                };
                let position = (shown == Some(true))
                    .then(|| {
                        let x = integer(team.field_value(key.x))?;
                        let y = integer(team.field_value(key.y))?;
                        ((0..=255).contains(&x) && (0..=255).contains(&y)).then_some([x, y])
                    })
                    .flatten();
                if let (Some(id), Some(position)) = (identity, position) {
                    self.last_seen.insert((observer_team, id), (tick, position));
                }
                if emit {
                    self.rows.push(MinimapObservation {
                        tick,
                        observer_team,
                        target_team,
                        target_entity: identity,
                        target_hero_id: pawn
                            .and_then(|p| integer(p.field_value(self.hero)))
                            .filter(|&id| id > 0),
                        target_steam_id: identity
                            .and_then(|id| steam_ids.as_ref()?.get(&id).copied().flatten()),
                        fow_entity_index_raw: raw_index,
                        visible_on_map: shown,
                        position,
                        tick_hidden_raw: integer(team.field_value(key.hidden)),
                        last_seen: identity
                            .and_then(|id| self.last_seen.get(&(observer_team, id)).copied()),
                    });
                }
            }
        }
        &self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{
        BareCharEncoding, DecodeProfile, FlattenedField, FlattenedSerializer,
        FlattenedSerializerDefinition, PreciseQAngleMode,
    };

    fn serializers() -> SerializerContainer {
        type Field = (&'static str, &'static str, Option<&'static str>);
        let definitions: &[(&str, &[Field])] = &[
            (
                "CCitadelPlayerController",
                &[
                    ("m_steamID", "uint64", None),
                    ("m_hPawn", "CHandle", None),
                    ("m_hHeroPawn", "CHandle", None),
                    ("m_PlayerDataGlobal", "PlayerData_t", Some("PlayerData")),
                ],
            ),
            (
                "PlayerData",
                &[(
                    "m_vecUpgrades",
                    "CNetworkUtlVectorBase< CUtlStringToken >",
                    None,
                )],
            ),
            (
                "CCitadelPlayerPawn",
                &[
                    ("m_iTeamNum", "int32", None),
                    (
                        "m_CCitadelHeroComponent",
                        "HeroComponent_t",
                        Some("HeroComponent"),
                    ),
                ],
            ),
            (
                "HeroComponent",
                &[("m_spawnedHero", "SpawnedHero_t", Some("SpawnedHero"))],
            ),
            ("SpawnedHero", &[("m_nHeroID", "int32", None)]),
            (
                "CCitadelTeam",
                &[
                    ("m_iTeamNum", "int32", None),
                    (
                        "m_vecFOWEntities",
                        "CNetworkUtlVectorBase< FOWEntry >",
                        Some("FOWEntry"),
                    ),
                ],
            ),
            (
                "FOWEntry",
                &[
                    ("m_nEntIndex", "int32", None),
                    ("m_nTeam", "int32", None),
                    ("m_eClass", "uint32", None),
                    ("m_bVisibleOnMap", "bool", None),
                    ("m_nPositionX", "uint8", None),
                    ("m_nPositionY", "uint8", None),
                    ("m_nTickHidden", "uint32", None),
                ],
            ),
        ];
        let mut symbols = Vec::<String>::new();
        let mut symbol = |name: &str| {
            if let Some(index) = symbols.iter().position(|s| s == name) {
                index as i32
            } else {
                symbols.push(name.into());
                symbols.len() as i32 - 1
            }
        };
        let mut fields = Vec::new();
        let mut serializers = Vec::new();
        for &(name, entries) in definitions.iter().rev() {
            let name = symbol(name);
            let mut indices = Vec::new();
            for &(name, ty, child) in entries {
                indices.push(fields.len() as i32);
                fields.push(
                    FlattenedField::new(Some(symbol(ty)), Some(symbol(name)))
                        .with_serializer_name_sym(child.map(&mut symbol)),
                );
            }
            serializers.push(FlattenedSerializerDefinition::new(Some(name), indices));
        }
        SerializerContainer::parse(
            FlattenedSerializer::new(serializers, symbols, fields),
            DecodeProfile::new(BareCharEncoding::UnsignedVarint, PreciseQAngleMode::Raw),
        )
        .unwrap()
    }

    fn entity(
        s: &SerializerContainer,
        class: &str,
        index: i32,
        serial: u32,
        values: Vec<(String, FieldValue)>,
    ) -> Entity {
        let serializer = s.get(class).unwrap();
        Entity::from_fields(
            index,
            serial,
            0,
            class,
            true,
            values
                .into_iter()
                .map(|(name, value)| (serializer.resolve_field_key(&name).unwrap(), value))
                .collect(),
        )
        .unwrap()
    }

    fn inventory(s: &SerializerContainer, ids: &[u32]) -> Entity {
        let mut values = vec![(
            UpgradeReader::PATH.into(),
            FieldValue::U64(ids.len() as u64),
        )];
        values.extend(
            ids.iter()
                .enumerate()
                .map(|(i, &id)| (format!("{}.{i}", UpgradeReader::PATH), FieldValue::U32(id))),
        );
        entity(s, "CCitadelPlayerController", 1, 1, values)
    }

    #[test]
    fn upgrades_distinguish_empty_missing_and_invalid_values() {
        let s = serializers();
        let mut reader = UpgradeReader::new(s.get("CCitadelPlayerController"));
        let mut controller = inventory(&s, &[]);
        assert_eq!(reader.read(&controller), Some([].as_slice()));
        controller.fields.clear();
        assert_eq!(reader.read(&controller), None);
        controller = inventory(&s, &[u32::MAX]);
        assert_eq!(reader.read(&controller), Some([u32::MAX].as_slice()));
        let key = s
            .get("CCitadelPlayerController")
            .unwrap()
            .resolve_field_key(&format!("{}.0", UpgradeReader::PATH))
            .unwrap();
        for invalid in [
            None,
            Some(FieldValue::I64(-1)),
            Some(FieldValue::U64(u64::MAX)),
        ] {
            controller.fields.remove(&key);
            if let Some(value) = invalid {
                controller.fields.insert(key, value);
            }
            assert_eq!(reader.read(&controller), None);
        }
    }

    #[test]
    fn upgrades_handle_replacements_shrinks_and_wide_arrays() {
        let s = serializers();
        let mut reader = UpgradeReader::new(s.get("CCitadelPlayerController"));
        for ids in [vec![7], vec![8], vec![], (0..257).collect()] {
            assert_eq!(reader.read(&inventory(&s, &ids)), Some(ids.as_slice()));
        }
        let mut controller = inventory(&s, &[1]);
        controller
            .fields
            .insert(reader.count.unwrap(), FieldValue::U64(4097));
        assert_eq!(reader.read(&controller), None);
    }

    fn pawn(s: &SerializerContainer, index: i32, serial: u32) -> Entity {
        entity(
            s,
            "CCitadelPlayerPawn",
            index,
            serial,
            vec![
                ("m_iTeamNum".into(), FieldValue::I64(3)),
                (
                    "m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID".into(),
                    FieldValue::I64(16),
                ),
            ],
        )
    }

    fn controller(s: &SerializerContainer, index: i32, pawn: &Entity) -> Entity {
        let handle = (pawn.serial << 14) | pawn.index as u32;
        entity(
            s,
            "CCitadelPlayerController",
            index,
            1,
            vec![
                ("m_steamID".into(), FieldValue::U64(123)),
                ("m_hPawn".into(), FieldValue::U32(handle)),
                ("m_hHeroPawn".into(), FieldValue::U32(handle)),
            ],
        )
    }

    fn team(s: &SerializerContainer, entry: usize, visible: bool) -> Entity {
        let prefix = format!("m_vecFOWEntities.{entry}");
        entity(
            s,
            "CCitadelTeam",
            2,
            1,
            vec![
                ("m_iTeamNum".into(), FieldValue::I64(2)),
                ("m_vecFOWEntities".into(), FieldValue::U64(entry as u64 + 1)),
                (format!("{prefix}.m_eClass"), FieldValue::U64(1)),
                (format!("{prefix}.m_nEntIndex"), FieldValue::I64(148)),
                (format!("{prefix}.m_nTeam"), FieldValue::I64(3)),
                (
                    format!("{prefix}.m_bVisibleOnMap"),
                    FieldValue::Bool(visible),
                ),
                (format!("{prefix}.m_nPositionX"), FieldValue::U64(100)),
                (format!("{prefix}.m_nPositionY"), FieldValue::U64(200)),
            ],
        )
    }

    #[test]
    fn minimap_uses_one_decoded_index_and_handles_wide_arrays() {
        let s = serializers();
        let mut reader = EnemyMinimap::new(&s);
        let mut entities = EntityContainer::new();
        let direct = pawn(&s, 148, 9);
        entities.insert(controller(&s, 1, &direct)).unwrap();
        entities.insert(direct).unwrap();
        entities.insert(pawn(&s, 74, 7)).unwrap();
        for entry in [0, 255, 256] {
            entities.insert(team(&s, entry, true)).unwrap();
            let rows = reader.update_entities(10, &entities, true);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].target_entity, Some(EntityId::new(148, 9)));
            assert_eq!(rows[0].target_steam_id, Some(123));
        }
        let mut missing = EntityContainer::new();
        missing.insert(team(&s, 0, true)).unwrap();
        missing.insert(pawn(&s, 74, 7)).unwrap();
        let row = &reader.update_entities(11, &missing, true)[0];
        assert_eq!(row.target_entity, None);
        assert_eq!(row.target_steam_id, None);
        assert_eq!(row.last_seen, None);
    }

    #[test]
    fn minimap_keeps_history_across_sampling_but_not_slot_reuse() {
        let s = serializers();
        let mut reader = EnemyMinimap::new(&s);
        let mut entities = EntityContainer::new();
        let old = pawn(&s, 148, 9);
        entities.insert(controller(&s, 1, &old)).unwrap();
        entities.insert(old).unwrap();
        entities.insert(team(&s, 0, true)).unwrap();
        assert!(reader.update_entities(10, &entities, false).is_empty());
        entities.insert(team(&s, 0, false)).unwrap();
        let row = &reader.update_entities(11, &entities, true)[0];
        assert_eq!(row.position, None);
        assert_eq!(row.last_seen, Some((10, [100, 200])));
        entities.insert(pawn(&s, 148, 10)).unwrap();
        let row = &reader.update_entities(12, &entities, true)[0];
        assert_eq!(row.target_entity, Some(EntityId::new(148, 10)));
        assert_eq!(row.target_steam_id, None); // The old full handle is stale.
        assert_eq!(row.last_seen, None);
        let current = entities.get(148).unwrap();
        entities.insert(controller(&s, 1, current)).unwrap();
        assert_eq!(
            reader.update_entities(13, &entities, true)[0].target_steam_id,
            Some(123)
        );
        entities
            .insert(controller(&s, 3, entities.get(148).unwrap()))
            .unwrap();
        assert_eq!(
            reader.update_entities(14, &entities, true)[0].target_steam_id,
            None
        );
    }
}
