use crate::*;

// ─────────────────────────── Parallel player_ticks ───────────────────────────
//
// `player_ticks` is a per-tick full snapshot of player pawn + controller state.
// Both classes are re-keyframed at every `DEM_FullPacket`, so the demo can be
// split at those keyframes and each segment decoded on its own thread, then the
// per-segment rows concatenated in order — identical to a single serial pass.
// See `Parser::decode_segment` / `full_packet_offsets`.

/// Number of keyframe segments to split the parallel `player_ticks` decode
/// across: the CPU count, overridable via `BOON_TICK_SEGMENTS` (`1` disables
/// parallelism; read fresh so tests can force the serial path).
pub(super) fn parallel_segments() -> usize {
    std::env::var("BOON_TICK_SEGMENTS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        })
        .max(1)
}

pub(super) const STAT_VIEWER_SLOTS: usize = 20;
pub(super) const UPGRADE_SLOTS: usize = 16;
pub(super) const ABILITY_UPGRADE_SLOTS: usize = 8;

#[derive(Clone, Copy, Default)]
pub(super) struct AbilityUpgradeKeys {
    pub(super) ability_id: Option<u64>,
    pub(super) upgrade_info: Option<u64>,
}

// Spirit power feeds hero-specific m_mapScalingStats rules from heroes.vdata.
pub(super) const MODIFIER_VALUE_SPIRIT_POWER: u32 = 158;

#[derive(Clone, Copy, Default)]
pub(super) struct StatViewerKeys {
    pub(super) value_type: Option<u64>,
    pub(super) value: Option<u64>,
}

pub(super) fn resolve_stat_viewer_keys(
    serializer: Option<&boon_parser::Serializer>,
) -> [StatViewerKeys; STAT_VIEWER_SLOTS] {
    std::array::from_fn(|i| StatViewerKeys {
        value_type: serializer.and_then(|s| {
            s.resolve_field_key(&format!(
                "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_eValType"
            ))
        }),
        value: serializer.and_then(|s| {
            s.resolve_field_key(&format!(
                "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_flValue"
            ))
        }),
    })
}

/// Combine independent resistance sources using Deadlock's multiplicative
/// stacking rule.
pub(super) fn combine_resistance(current: f32, source: f32) -> f32 {
    100.0 - (100.0 - current) * (100.0 - source) / 100.0
}

/// Reconstruct the baseline resistance shown by the client from hero
/// progression, spirit-power scaling, and unconditional equipped-item stats.
///
/// Temporary buffs, barriers, auras, and enemy resistance reductions are not
/// included because the controller does not replicate a final resistance value.
pub(super) fn effective_resistances_from_values(
    hero_id: i64,
    level: i64,
    values: impl IntoIterator<Item = (u32, f32)>,
    upgrades: impl IntoIterator<Item = u32>,
) -> [f32; 2] {
    let stats = boon_parser::hero_resistance_stats(hero_id);
    let level_ups = level.saturating_sub(1) as f32;
    let mut spirit_power = stats.base_spirit_power + level_ups * stats.spirit_power_per_level;

    for (value_type, value) in values {
        if value_type == MODIFIER_VALUE_SPIRIT_POWER && value.is_finite() {
            spirit_power += value;
        }
    }

    let hero_bullet = stats.base_bullet_resist
        + level_ups * stats.bullet_resist_per_level
        + spirit_power * stats.bullet_resist_per_spirit_power;
    let hero_spirit = stats.base_spirit_resist
        + level_ups * stats.spirit_resist_per_level
        + spirit_power * stats.spirit_resist_per_spirit_power;
    let mut bullet = combine_resistance(0.0, hero_bullet);
    let mut spirit = combine_resistance(0.0, hero_spirit);

    for upgrade_id in upgrades {
        let item = boon_parser::item_resistance_stats(upgrade_id);
        bullet = combine_resistance(bullet, item.bullet_resist);
        spirit = combine_resistance(spirit, item.spirit_resist);
    }

    [bullet, spirit]
}

/// Split the full-packet offsets into `n` contiguous `(start_offset, end_tick)`
/// segments: segment 0 starts from the signon baseline (`None`), the rest
/// cold-restart at an evenly spaced full packet.
pub(super) fn segment_ranges(offsets: &[(usize, i32)], n: usize) -> Vec<(Option<usize>, i32)> {
    (0..n)
        .map(|i| {
            let start = (i != 0).then(|| offsets[i * offsets.len() / n].0);
            let end_tick = if i == n - 1 {
                i32::MAX
            } else {
                offsets[(i + 1) * offsets.len() / n].1
            };
            (start, end_tick)
        })
        .collect()
}

/// Field keys for the `player_ticks` snapshot, resolved once from the send-table
/// serializers. `p_*` fields live on `CCitadelPlayerPawn`, `c_*` on
/// `CCitadelPlayerController`.
#[derive(Clone, Copy, Default)]
pub(super) struct PtKeys {
    pub(super) hero_id: Option<u64>,
    pub(super) vec_x: Option<u64>,
    pub(super) vec_y: Option<u64>,
    pub(super) vec_z: Option<u64>,
    pub(super) cell_x: Option<u64>,
    pub(super) cell_y: Option<u64>,
    pub(super) cell_z: Option<u64>,
    pub(super) camera: Option<u64>,
    pub(super) in_regen: Option<u64>,
    pub(super) in_item_shop: Option<u64>,
    pub(super) death_time: Option<u64>,
    pub(super) last_spawn: Option<u64>,
    pub(super) respawn: Option<u64>,
    pub(super) health: Option<u64>,
    pub(super) max_health: Option<u64>,
    pub(super) lifestate: Option<u64>,
    pub(super) souls: Option<u64>,
    pub(super) spent_souls: Option<u64>,
    pub(super) combat_end: Option<u64>,
    pub(super) combat_last_dmg: Option<u64>,
    pub(super) combat_start: Option<u64>,
    pub(super) dmg_dealt_end: Option<u64>,
    pub(super) dmg_dealt_last: Option<u64>,
    pub(super) dmg_dealt_start: Option<u64>,
    pub(super) dmg_taken_end: Option<u64>,
    pub(super) dmg_taken_last: Option<u64>,
    pub(super) dmg_taken_start: Option<u64>,
    pub(super) time_revealed: Option<u64>,
    pub(super) build_id: Option<u64>,
    pub(super) pawn_handle: Option<u64>,
    pub(super) health_max: Option<u64>,
    pub(super) alive: Option<u64>,
    pub(super) rebirth: Option<u64>,
    pub(super) rejuvenator: Option<u64>,
    pub(super) ultimate: Option<u64>,
    pub(super) health_regen: Option<u64>,
    pub(super) ult_cd_end: Option<u64>,
    pub(super) ult_cd_start: Option<u64>,
    pub(super) ap_nw: Option<u64>,
    pub(super) gold_nw: Option<u64>,
    pub(super) denies: Option<u64>,
    pub(super) hero_damage: Option<u64>,
    pub(super) hero_healing: Option<u64>,
    pub(super) obj_damage: Option<u64>,
    pub(super) self_healing: Option<u64>,
    pub(super) kill_streak: Option<u64>,
    pub(super) last_hits: Option<u64>,
    pub(super) level: Option<u64>,
    pub(super) kills: Option<u64>,
    pub(super) deaths: Option<u64>,
    pub(super) assists: Option<u64>,
    pub(super) stat_viewer_count: Option<u64>,
    pub(super) stat_viewer: [StatViewerKeys; STAT_VIEWER_SLOTS],
    pub(super) upgrade_count: Option<u64>,
    pub(super) upgrades: [Option<u64>; UPGRADE_SLOTS],
    pub(super) ability_upgrades: [AbilityUpgradeKeys; ABILITY_UPGRADE_SLOTS],
}

impl PtKeys {
    pub(super) fn resolve(ctx: &boon_parser::Context) -> Self {
        let pawn = ctx.serializers().get("CCitadelPlayerPawn");
        let ctrl = ctx.serializers().get("CCitadelPlayerController");
        let p = |name: &str| pawn.and_then(|s| s.resolve_field_key(name));
        let upgrades = std::array::from_fn(|i| {
            ctrl.and_then(|s| s.resolve_field_key(&format!("m_PlayerDataGlobal.m_vecUpgrades.{i}")))
        });
        let ability_upgrades = std::array::from_fn(|i| AbilityUpgradeKeys {
            ability_id: ctrl.and_then(|s| {
                s.resolve_field_key(&format!(
                    "m_PlayerDataGlobal.m_vecAbilityUpgradeState.{i:04}.m_ItemID"
                ))
            }),
            upgrade_info: ctrl.and_then(|s| {
                s.resolve_field_key(&format!(
                    "m_PlayerDataGlobal.m_vecAbilityUpgradeState.{i:04}.m_nUpgradeInfo"
                ))
            }),
        });
        let c = |name: &str| ctrl.and_then(|s| s.resolve_field_key(name));
        let stat_viewer = resolve_stat_viewer_keys(ctrl);
        Self {
            hero_id: p("m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID"),
            vec_x: p("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX"),
            vec_y: p("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY"),
            vec_z: p("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ"),
            cell_x: p("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX"),
            cell_y: p("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY"),
            cell_z: p("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ"),
            camera: p("m_angClientCamera"),
            in_regen: p("m_bInRegenerationZone"),
            in_item_shop: p("m_bInItemShopZone"),
            death_time: p("m_flDeathTime"),
            last_spawn: p("m_flLastSpawnTime"),
            respawn: p("m_flRespawnTime"),
            health: p("m_iHealth"),
            max_health: p("m_iMaxHealth"),
            lifestate: p("m_lifeState"),
            souls: p("m_nCurrencies.m_nCurrencies"),
            spent_souls: p("m_nSpentCurrencies.m_nSpentCurrencies"),
            combat_end: p("m_sInCombat.m_flEndTime"),
            combat_last_dmg: p("m_sInCombat.m_flLastDamageTime"),
            combat_start: p("m_sInCombat.m_flStartTime"),
            dmg_dealt_end: p("m_sPlayerDamageDealt.m_flEndTime"),
            dmg_dealt_last: p("m_sPlayerDamageDealt.m_flLastDamageTime"),
            dmg_dealt_start: p("m_sPlayerDamageDealt.m_flStartTime"),
            dmg_taken_end: p("m_sPlayerDamageTaken.m_flEndTime"),
            dmg_taken_last: p("m_sPlayerDamageTaken.m_flLastDamageTime"),
            dmg_taken_start: p("m_sPlayerDamageTaken.m_flStartTime"),
            time_revealed: p("m_timeRevealedOnMinimapByNPC"),
            build_id: p("m_unHeroBuildID"),
            pawn_handle: c("m_hPawn"),
            health_max: c("m_PlayerDataGlobal.m_iHealthMax"),
            alive: c("m_PlayerDataGlobal.m_bAlive"),
            rebirth: c("m_PlayerDataGlobal.m_bHasRebirth"),
            rejuvenator: c("m_PlayerDataGlobal.m_bHasRejuvenator"),
            ultimate: c("m_PlayerDataGlobal.m_bUltimateTrained"),
            health_regen: c("m_PlayerDataGlobal.m_flHealthRegen"),
            ult_cd_end: c("m_PlayerDataGlobal.m_flUltimateCooldownEnd"),
            ult_cd_start: c("m_PlayerDataGlobal.m_flUltimateCooldownStart"),
            ap_nw: c("m_PlayerDataGlobal.m_iAPNetWorth"),
            gold_nw: c("m_PlayerDataGlobal.m_iGoldNetWorth"),
            denies: c("m_PlayerDataGlobal.m_iDenies"),
            hero_damage: c("m_PlayerDataGlobal.m_iHeroDamage"),
            hero_healing: c("m_PlayerDataGlobal.m_iHeroHealing"),
            obj_damage: c("m_PlayerDataGlobal.m_iObjectiveDamage"),
            self_healing: c("m_PlayerDataGlobal.m_iSelfHealing"),
            kill_streak: c("m_PlayerDataGlobal.m_iKillStreak"),
            upgrade_count: c("m_PlayerDataGlobal.m_vecUpgrades"),
            upgrades,
            ability_upgrades,
            last_hits: c("m_PlayerDataGlobal.m_iLastHits"),
            level: c("m_PlayerDataGlobal.m_iLevel"),
            kills: c("m_PlayerDataGlobal.m_iPlayerKills"),
            deaths: c("m_PlayerDataGlobal.m_iDeaths"),
            assists: c("m_PlayerDataGlobal.m_iPlayerAssists"),
            stat_viewer_count: c("m_PlayerDataGlobal.m_vecStatViewerModifierValues"),
            stat_viewer,
        }
    }
}

/// Live barrier remaining, decoded from each pawn's persistent
/// `modifier_barrier_tracker` entry in the `ActiveModifiers` string table.
/// Deadlock stores barrier capacity in `float1` and the current amount in
/// `float2`; demos without that tracker naturally stay at zero.
pub(super) const BARRIER_TRACKER_MODIFIER_ID: u32 = 4_267_845_006;

#[derive(Default)]
pub(super) struct BarrierState {
    pub(super) modifiers: boon_parser::ModifierState,
    pub(super) remaining_by_pawn: HashMap<i32, f32>,
    pub(super) serial_to_pawn: HashMap<u32, i32>,
    pub(super) pawn_to_serial: HashMap<i32, u32>,
}

impl BarrierState {
    pub(super) fn remove_serial(&mut self, serial: u32) {
        let Some(pawn) = self.serial_to_pawn.remove(&serial) else {
            return;
        };
        if self.pawn_to_serial.get(&pawn) == Some(&serial) {
            self.pawn_to_serial.remove(&pawn);
            self.remaining_by_pawn.remove(&pawn);
        }
    }

    pub(super) fn apply_live_entry(
        &mut self,
        serial: u32,
        entry: &boon_proto::proto::CModifierTableEntry,
    ) {
        if entry.modifier_subclass != Some(BARRIER_TRACKER_MODIFIER_ID) {
            self.remove_serial(serial);
            return;
        }
        let Some(pawn) = boon_parser::protobuf_handle_index(entry.parent) else {
            return;
        };
        let remaining = entry.float2.unwrap_or(0.0);
        let remaining = if remaining.is_finite() {
            remaining.max(0.0)
        } else {
            0.0
        };

        if let Some(old_pawn) = self.serial_to_pawn.insert(serial, pawn)
            && old_pawn != pawn
            && self.pawn_to_serial.get(&old_pawn) == Some(&serial)
        {
            self.pawn_to_serial.remove(&old_pawn);
            self.remaining_by_pawn.remove(&old_pawn);
        }
        if let Some(old_serial) = self.pawn_to_serial.insert(pawn, serial)
            && old_serial != serial
        {
            self.serial_to_pawn.remove(&old_serial);
        }
        self.remaining_by_pawn.insert(pawn, remaining);
    }

    pub(super) fn update(&mut self, ctx: &boon_parser::Context) {
        for change in self.modifiers.update(ctx) {
            match change.kind {
                boon_parser::ModifierChangeKind::Removed => {
                    self.remove_serial(change.serial);
                }
                boon_parser::ModifierChangeKind::Applied
                | boon_parser::ModifierChangeKind::Changed => {
                    self.apply_live_entry(change.serial, &change.entry);
                }
            }
        }
    }

    pub(super) fn rebuild(&mut self, ctx: &boon_parser::Context) {
        self.modifiers.rebuild(ctx);
        self.remaining_by_pawn.clear();
        self.serial_to_pawn.clear();
        self.pawn_to_serial.clear();
        let entries: Vec<_> = self
            .modifiers
            .entries()
            .iter()
            .map(|(&serial, entry)| (serial, entry.clone()))
            .collect();
        for (serial, entry) in entries {
            self.apply_live_entry(serial, &entry);
        }
    }

    pub(super) fn remaining(&self, pawn_handle: u32) -> f32 {
        boon_parser::protobuf_handle_index(Some(pawn_handle))
            .and_then(|idx| self.remaining_by_pawn.get(&idx).copied())
            .unwrap_or(0.0)
    }
}

pub(super) fn raw_u32(entity: &boon_parser::Entity, key: Option<u64>) -> u32 {
    key.and_then(|key| entity.fields.get(&key))
        .and_then(|value| match value {
            boon_parser::FieldValue::I32(value) => Some(*value as u32),
            boon_parser::FieldValue::I64(value) => Some(*value as u32),
            boon_parser::FieldValue::U32(value) => Some(*value),
            boon_parser::FieldValue::U64(value) => Some(*value as u32),
            _ => None,
        })
        .unwrap_or(0)
}

pub(super) fn stat_inputs(
    controller: &boon_parser::Entity,
    keys: &PtKeys,
) -> (Vec<u32>, HashMap<u32, u8>) {
    let upgrade_count = controller
        .get_i64(keys.upgrade_count)
        .clamp(0, UPGRADE_SLOTS as i64) as usize;
    let upgrades = keys.upgrades[..upgrade_count]
        .iter()
        .map(|key| controller.get_u32(*key))
        .filter(|id| *id != 0)
        .collect();

    let mut ability_tiers = HashMap::with_capacity(ABILITY_UPGRADE_SLOTS);
    for keys in keys.ability_upgrades {
        let ability_id = controller.get_u32(keys.ability_id);
        if ability_id == 0 {
            continue;
        }
        let upgrade_bits = raw_u32(controller, keys.upgrade_info) >> 17;
        ability_tiers.insert(ability_id, upgrade_bits.count_ones().min(3) as u8);
    }
    (upgrades, ability_tiers)
}

#[derive(Default)]
pub(super) struct StatValueCols {
    pub(super) native: Vec<f32>,
    pub(super) baseline: Vec<f32>,
    pub(super) effective: Vec<f32>,
    pub(super) complete: Vec<bool>,
}

pub(super) struct StatCols {
    pub(super) tick: Vec<i32>,
    pub(super) hero_id: Vec<i64>,
    pub(super) values: [StatValueCols; boon_parser::STAT_COUNT],
}

impl Default for StatCols {
    fn default() -> Self {
        Self {
            tick: Vec::new(),
            hero_id: Vec::new(),
            values: std::array::from_fn(|_| StatValueCols::default()),
        }
    }
}

impl StatCols {
    pub(super) fn collect_tick(
        &mut self,
        ctx: &boon_parser::Context,
        keys: &PtKeys,
        modifiers: &boon_parser::ModifierState,
        selected: boon_parser::StatMask,
    ) {
        for (_, controller) in ctx
            .entities()
            .iter()
            .filter(|(_, entity)| entity.class_name.as_ref() == "CCitadelPlayerController")
        {
            let Some(pawn_handle) = controller.get_handle(keys.pawn_handle) else {
                continue;
            };
            let Some(pawn) = ctx.entities().get_by_handle(pawn_handle) else {
                continue;
            };
            if pawn.class_name.as_ref() != "CCitadelPlayerPawn" {
                continue;
            }
            let hero_id = pawn.get_i64(keys.hero_id);
            if hero_id == 0 {
                continue;
            }
            let Some(pawn_index) = boon_parser::protobuf_handle_index(Some(pawn_handle)) else {
                continue;
            };
            let (upgrades, ability_tiers) = stat_inputs(controller, keys);
            let layers = boon_parser::evaluate_player_stats(
                hero_id,
                controller.get_i64(keys.level),
                &upgrades,
                &ability_tiers,
                modifiers.entries().values().filter(|entry| {
                    boon_parser::protobuf_handle_index(entry.parent) == Some(pawn_index)
                }),
            );

            self.tick.push(ctx.tick());
            self.hero_id.push(hero_id);
            for stat in selected.iter() {
                let columns = &mut self.values[stat as usize];
                columns.native.push(layers.native[stat]);
                columns.baseline.push(layers.baseline[stat]);
                columns.effective.push(layers.effective[stat]);
                columns.complete.push(layers.complete.contains(stat));
            }
        }
    }

    pub(super) fn append(&mut self, mut other: Self, selected: boon_parser::StatMask) {
        self.tick.append(&mut other.tick);
        self.hero_id.append(&mut other.hero_id);
        for stat in selected.iter() {
            let target = &mut self.values[stat as usize];
            let source = &mut other.values[stat as usize];
            target.native.append(&mut source.native);
            target.baseline.append(&mut source.baseline);
            target.effective.append(&mut source.effective);
            target.complete.append(&mut source.complete);
        }
    }

    pub(super) fn into_dataframe(self, selected: boon_parser::StatMask) -> PyResult<DataFrame> {
        let mut columns = vec![
            Column::new("tick".into(), self.tick),
            Column::new("hero_id".into(), self.hero_id),
        ];
        for stat in selected.iter() {
            let name = stat.name();
            let values = &self.values[stat as usize];
            columns.push(Column::new(
                format!("{name}_native").into(),
                values.native.clone(),
            ));
            columns.push(Column::new(
                format!("{name}_baseline").into(),
                values.baseline.clone(),
            ));
            columns.push(Column::new(
                format!("{name}_effective").into(),
                values.effective.clone(),
            ));
            columns.push(Column::new(
                format!("{name}_complete").into(),
                values.complete.clone(),
            ));
        }
        df_from_columns(columns).map_err(|error| {
            InvalidDemoError::new_err(format!("Failed to create DataFrame: {error}"))
        })
    }
}

#[derive(Default)]
pub(super) struct StatSegment {
    pub(super) columns: StatCols,
    pub(super) modifiers: boon_parser::ModifierState,
    pub(super) initialized: bool,
}

impl StatSegment {
    pub(super) fn update(&mut self, ctx: &boon_parser::Context) {
        if self.initialized {
            self.modifiers.update(ctx);
        } else {
            self.modifiers.rebuild(ctx);
            self.initialized = true;
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct PlayerStatInputs {
    pub(super) hero_id: i64,
    pub(super) level: i64,
    pub(super) upgrades: Vec<u32>,
    pub(super) ability_tiers: HashMap<u32, u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ModifierEffectSignature {
    pub(super) ability_id: u32,
    pub(super) modifier_id: u32,
    pub(super) stacks: i32,
    pub(super) in_aura_range: Option<bool>,
}

#[derive(Default)]
pub(super) struct StatEffectCols {
    pub(super) tick: Vec<i32>,
    pub(super) hero_id: Vec<i64>,
    pub(super) event: Vec<String>,
    pub(super) stat: Vec<String>,
    pub(super) operation: Vec<String>,
    pub(super) value: Vec<f32>,
    pub(super) source_type: Vec<String>,
    pub(super) layer: Vec<String>,
    pub(super) ability_id: Vec<u32>,
    pub(super) ability_name: Vec<String>,
    pub(super) modifier_id: Vec<u32>,
    pub(super) modifier_name: Vec<String>,
    pub(super) serial: Vec<u32>,
    pub(super) caster_hero_id: Vec<i64>,
    pub(super) provider_hero_id: Vec<i64>,
    pub(super) stacks: Vec<i32>,
    pub(super) duration: Vec<f32>,
    pub(super) active: Vec<bool>,
    pub(super) complete: Vec<bool>,
}

pub(super) struct ModifierEffectRow<'a> {
    pub(super) tick: i32,
    pub(super) hero_id: i64,
    pub(super) event: &'a str,
    pub(super) effect: boon_parser::StatEffect,
    pub(super) source_type: &'a str,
    pub(super) layer: &'a str,
    pub(super) ability_id: u32,
    pub(super) modifier_id: u32,
    pub(super) serial: u32,
    pub(super) caster_hero_id: i64,
    pub(super) provider_hero_id: i64,
    pub(super) stacks: i32,
    pub(super) duration: f32,
    pub(super) active: bool,
    pub(super) spirit_power: f32,
    pub(super) ability_tier: u8,
}

impl StatEffectCols {
    pub(super) fn push(&mut self, row: ModifierEffectRow<'_>) {
        let (value, expression_complete) = row.effect.resolve(row.spirit_power, row.ability_tier);
        self.tick.push(row.tick);
        self.hero_id.push(row.hero_id);
        self.event.push(row.event.to_string());
        self.stat.push(row.effect.stat.name().to_string());
        self.operation.push(row.effect.operation.name().to_string());
        self.value.push(value);
        self.source_type.push(row.source_type.to_string());
        self.layer.push(row.layer.to_string());
        self.ability_id.push(row.ability_id);
        self.ability_name
            .push(boon_parser::ability_name(row.ability_id).to_string());
        self.modifier_id.push(row.modifier_id);
        self.modifier_name
            .push(boon_parser::modifier_name(row.modifier_id).to_string());
        self.serial.push(row.serial);
        self.caster_hero_id.push(row.caster_hero_id);
        self.provider_hero_id.push(row.provider_hero_id);
        self.stacks.push(row.stacks);
        self.duration.push(row.duration);
        self.active.push(row.active);
        self.complete.push(expression_complete && row.stacks <= 1);
    }

    pub(super) fn into_dataframe(self) -> PyResult<DataFrame> {
        df_from_columns(vec![
            Column::new("tick".into(), self.tick),
            Column::new("hero_id".into(), self.hero_id),
            Column::new("event".into(), self.event),
            Column::new("stat".into(), self.stat),
            Column::new("operation".into(), self.operation),
            Column::new("value".into(), self.value),
            Column::new("source_type".into(), self.source_type),
            Column::new("layer".into(), self.layer),
            Column::new("ability_id".into(), self.ability_id),
            Column::new("ability_name".into(), self.ability_name),
            Column::new("modifier_id".into(), self.modifier_id),
            Column::new("modifier_name".into(), self.modifier_name),
            Column::new("serial".into(), self.serial),
            Column::new("caster_hero_id".into(), self.caster_hero_id),
            Column::new("provider_hero_id".into(), self.provider_hero_id),
            Column::new("stacks".into(), self.stacks),
            Column::new("duration".into(), self.duration),
            Column::new("active".into(), self.active),
            Column::new("complete".into(), self.complete),
        ])
        .map_err(|error| {
            InvalidDemoError::new_err(format!("Failed to create stat_effects DataFrame: {error}"))
        })
    }
}

/// Column vectors accumulated for `player_ticks`. One per output column; the
/// order and names in [`into_columns`](PtCols::into_columns) must match the
/// serial builder in `load()`.
#[derive(Default)]
pub(super) struct PtCols {
    pub(super) tick: Vec<i32>,
    pub(super) hero_id: Vec<i64>,
    pub(super) x: Vec<f32>,
    pub(super) y: Vec<f32>,
    pub(super) z: Vec<f32>,
    pub(super) pitch: Vec<f32>,
    pub(super) yaw: Vec<f32>,
    pub(super) roll: Vec<f32>,
    pub(super) in_regen_zone: Vec<bool>,
    pub(super) in_item_shop: Vec<bool>,
    pub(super) death_time: Vec<f32>,
    pub(super) last_spawn_time: Vec<f32>,
    pub(super) respawn_time: Vec<f32>,
    pub(super) health: Vec<i64>,
    pub(super) max_health: Vec<i64>,
    pub(super) barrier: Vec<f32>,
    pub(super) bullet_resist: Vec<f32>,
    pub(super) spirit_resist: Vec<f32>,
    pub(super) lifestate: Vec<i64>,
    pub(super) souls: Vec<i64>,
    pub(super) spent_souls: Vec<i64>,
    pub(super) combat_end: Vec<f32>,
    pub(super) combat_last_dmg: Vec<f32>,
    pub(super) combat_start: Vec<f32>,
    pub(super) dmg_dealt_end: Vec<f32>,
    pub(super) dmg_dealt_last: Vec<f32>,
    pub(super) dmg_dealt_start: Vec<f32>,
    pub(super) dmg_taken_end: Vec<f32>,
    pub(super) dmg_taken_last: Vec<f32>,
    pub(super) dmg_taken_start: Vec<f32>,
    pub(super) time_revealed: Vec<f32>,
    pub(super) build_id: Vec<i64>,
    pub(super) is_alive: Vec<bool>,
    pub(super) has_rebirth: Vec<bool>,
    pub(super) has_rejuvenator: Vec<bool>,
    pub(super) has_ultimate: Vec<bool>,
    pub(super) health_regen: Vec<f32>,
    pub(super) ult_cd_start: Vec<f32>,
    pub(super) ult_cd_end: Vec<f32>,
    pub(super) ap_nw: Vec<i64>,
    pub(super) gold_nw: Vec<i64>,
    pub(super) denies: Vec<i64>,
    pub(super) hero_damage: Vec<i64>,
    pub(super) hero_healing: Vec<i64>,
    pub(super) obj_damage: Vec<i64>,
    pub(super) self_healing: Vec<i64>,
    pub(super) kill_streak: Vec<i64>,
    pub(super) last_hits: Vec<i64>,
    pub(super) level: Vec<i64>,
    pub(super) kills: Vec<i64>,
    pub(super) deaths: Vec<i64>,
    pub(super) assists: Vec<i64>,
}

impl PtCols {
    /// Append one snapshot row per live player at `ctx.tick()` (mirrors the serial
    /// collector in `load()`; must stay in sync with it).
    pub(super) fn collect_tick(
        &mut self,
        ctx: &boon_parser::Context,
        k: &PtKeys,
        barriers: &BarrierState,
    ) {
        for (_, ctrl) in ctx
            .entities()
            .iter()
            .filter(|(_, e)| e.class_name.as_ref() == "CCitadelPlayerController")
        {
            let Some(pawn_handle) = ctrl.get_handle(k.pawn_handle) else {
                continue;
            };
            let pawn = match ctx.entities().get_by_handle(pawn_handle) {
                Some(p) if p.class_name.as_ref() == "CCitadelPlayerPawn" => p,
                _ => continue,
            };
            let hid = pawn.get_i64(k.hero_id);
            if hid == 0 {
                continue;
            }
            self.tick.push(ctx.tick());
            self.hero_id.push(hid);
            let [x, y, z] =
                pawn.world_position([k.cell_x, k.cell_y, k.cell_z], [k.vec_x, k.vec_y, k.vec_z]);
            self.x.push(x);
            self.y.push(y);
            self.z.push(z);
            let a = pawn.get_qangle(k.camera);
            self.pitch.push(a[0]);
            self.yaw.push(a[1]);
            self.roll.push(a[2]);
            self.in_regen_zone.push(pawn.get_bool(k.in_regen));
            self.in_item_shop.push(pawn.get_bool(k.in_item_shop));
            self.death_time.push(pawn.get_f32(k.death_time));
            self.last_spawn_time.push(pawn.get_f32(k.last_spawn));
            self.respawn_time.push(pawn.get_f32(k.respawn));
            self.health.push(pawn.get_i64(k.health));
            let eff = ctrl.get_i64(k.health_max);
            self.max_health.push(if eff > 0 {
                eff
            } else {
                pawn.get_i64(k.max_health)
            });
            self.barrier.push(barriers.remaining(pawn_handle));
            let level = ctrl.get_i64(k.level);
            let [bullet_resist, spirit_resist] = effective_resistances_from_values(
                hid,
                level,
                k.stat_viewer
                    .iter()
                    .take(
                        ctrl.get_i64(k.stat_viewer_count)
                            .clamp(0, STAT_VIEWER_SLOTS as i64) as usize,
                    )
                    .map(|keys| (ctrl.get_u32(keys.value_type), ctrl.get_f32(keys.value))),
                k.upgrades
                    .iter()
                    .take(ctrl.get_i64(k.upgrade_count).clamp(0, UPGRADE_SLOTS as i64) as usize)
                    .map(|key| ctrl.get_u32(*key)),
            );
            self.bullet_resist.push(bullet_resist);
            self.spirit_resist.push(spirit_resist);
            self.lifestate.push(pawn.get_i64(k.lifestate));
            self.souls.push(pawn.get_i64(k.souls));
            self.spent_souls.push(pawn.get_i64(k.spent_souls));
            self.combat_end.push(pawn.get_f32(k.combat_end));
            self.combat_last_dmg.push(pawn.get_f32(k.combat_last_dmg));
            self.combat_start.push(pawn.get_f32(k.combat_start));
            self.dmg_dealt_end.push(pawn.get_f32(k.dmg_dealt_end));
            self.dmg_dealt_last.push(pawn.get_f32(k.dmg_dealt_last));
            self.dmg_dealt_start.push(pawn.get_f32(k.dmg_dealt_start));
            self.dmg_taken_end.push(pawn.get_f32(k.dmg_taken_end));
            self.dmg_taken_last.push(pawn.get_f32(k.dmg_taken_last));
            self.dmg_taken_start.push(pawn.get_f32(k.dmg_taken_start));
            self.time_revealed.push(pawn.get_f32(k.time_revealed));
            self.build_id.push(pawn.get_i64(k.build_id));
            self.is_alive.push(ctrl.get_bool(k.alive));
            self.has_rebirth.push(ctrl.get_bool(k.rebirth));
            self.has_rejuvenator.push(ctrl.get_bool(k.rejuvenator));
            self.has_ultimate.push(ctrl.get_bool(k.ultimate));
            self.health_regen.push(ctrl.get_f32(k.health_regen));
            // Column start ← field CooldownEnd, column end ← field CooldownStart
            // (kept identical to the serial builder).
            self.ult_cd_start.push(ctrl.get_f32(k.ult_cd_end));
            self.ult_cd_end.push(ctrl.get_f32(k.ult_cd_start));
            self.ap_nw.push(ctrl.get_i64(k.ap_nw));
            self.gold_nw.push(ctrl.get_i64(k.gold_nw));
            self.denies.push(ctrl.get_i64(k.denies));
            self.hero_damage.push(ctrl.get_i64(k.hero_damage));
            self.hero_healing.push(ctrl.get_i64(k.hero_healing));
            self.obj_damage.push(ctrl.get_i64(k.obj_damage));
            self.self_healing.push(ctrl.get_i64(k.self_healing));
            self.kill_streak.push(ctrl.get_i64(k.kill_streak));
            self.last_hits.push(ctrl.get_i64(k.last_hits));
            self.level.push(level);
            self.kills.push(ctrl.get_i64(k.kills));
            self.deaths.push(ctrl.get_i64(k.deaths));
            self.assists.push(ctrl.get_i64(k.assists));
        }
    }

    /// Append another segment's rows onto this one (segments are joined in order).
    pub(super) fn append(&mut self, mut o: PtCols) {
        self.tick.append(&mut o.tick);
        self.hero_id.append(&mut o.hero_id);
        self.x.append(&mut o.x);
        self.y.append(&mut o.y);
        self.z.append(&mut o.z);
        self.pitch.append(&mut o.pitch);
        self.yaw.append(&mut o.yaw);
        self.roll.append(&mut o.roll);
        self.in_regen_zone.append(&mut o.in_regen_zone);
        self.in_item_shop.append(&mut o.in_item_shop);
        self.death_time.append(&mut o.death_time);
        self.last_spawn_time.append(&mut o.last_spawn_time);
        self.respawn_time.append(&mut o.respawn_time);
        self.health.append(&mut o.health);
        self.max_health.append(&mut o.max_health);
        self.barrier.append(&mut o.barrier);
        self.bullet_resist.append(&mut o.bullet_resist);
        self.spirit_resist.append(&mut o.spirit_resist);
        self.lifestate.append(&mut o.lifestate);
        self.souls.append(&mut o.souls);
        self.spent_souls.append(&mut o.spent_souls);
        self.combat_end.append(&mut o.combat_end);
        self.combat_last_dmg.append(&mut o.combat_last_dmg);
        self.combat_start.append(&mut o.combat_start);
        self.dmg_dealt_end.append(&mut o.dmg_dealt_end);
        self.dmg_dealt_last.append(&mut o.dmg_dealt_last);
        self.dmg_dealt_start.append(&mut o.dmg_dealt_start);
        self.dmg_taken_end.append(&mut o.dmg_taken_end);
        self.dmg_taken_last.append(&mut o.dmg_taken_last);
        self.dmg_taken_start.append(&mut o.dmg_taken_start);
        self.time_revealed.append(&mut o.time_revealed);
        self.build_id.append(&mut o.build_id);
        self.is_alive.append(&mut o.is_alive);
        self.has_rebirth.append(&mut o.has_rebirth);
        self.has_rejuvenator.append(&mut o.has_rejuvenator);
        self.has_ultimate.append(&mut o.has_ultimate);
        self.health_regen.append(&mut o.health_regen);
        self.ult_cd_start.append(&mut o.ult_cd_start);
        self.ult_cd_end.append(&mut o.ult_cd_end);
        self.ap_nw.append(&mut o.ap_nw);
        self.gold_nw.append(&mut o.gold_nw);
        self.denies.append(&mut o.denies);
        self.hero_damage.append(&mut o.hero_damage);
        self.hero_healing.append(&mut o.hero_healing);
        self.obj_damage.append(&mut o.obj_damage);
        self.self_healing.append(&mut o.self_healing);
        self.kill_streak.append(&mut o.kill_streak);
        self.last_hits.append(&mut o.last_hits);
        self.level.append(&mut o.level);
        self.kills.append(&mut o.kills);
        self.deaths.append(&mut o.deaths);
        self.assists.append(&mut o.assists);
    }

    /// Build the `player_ticks` DataFrame. Column order/names must match `load()`.
    pub(super) fn into_dataframe(self) -> PyResult<DataFrame> {
        df_from_columns(vec![
            Column::new("tick".into(), self.tick),
            Column::new("hero_id".into(), self.hero_id),
            Column::new("x".into(), self.x),
            Column::new("y".into(), self.y),
            Column::new("z".into(), self.z),
            Column::new("pitch".into(), self.pitch),
            Column::new("yaw".into(), self.yaw),
            Column::new("roll".into(), self.roll),
            Column::new("in_regen_zone".into(), self.in_regen_zone),
            Column::new("in_item_shop".into(), self.in_item_shop),
            Column::new("death_time".into(), self.death_time),
            Column::new("last_spawn_time".into(), self.last_spawn_time),
            Column::new("respawn_time".into(), self.respawn_time),
            Column::new("health".into(), self.health),
            Column::new("max_health".into(), self.max_health),
            Column::new("barrier".into(), self.barrier),
            Column::new("bullet_resist_baseline".into(), self.bullet_resist),
            Column::new("spirit_resist_baseline".into(), self.spirit_resist),
            Column::new("lifestate".into(), self.lifestate),
            Column::new("souls".into(), self.souls),
            Column::new("spent_souls".into(), self.spent_souls),
            Column::new("in_combat_end_time".into(), self.combat_end),
            Column::new("in_combat_last_damage_time".into(), self.combat_last_dmg),
            Column::new("in_combat_start_time".into(), self.combat_start),
            Column::new("player_damage_dealt_end_time".into(), self.dmg_dealt_end),
            Column::new(
                "player_damage_dealt_last_damage_time".into(),
                self.dmg_dealt_last,
            ),
            Column::new(
                "player_damage_dealt_start_time".into(),
                self.dmg_dealt_start,
            ),
            Column::new("player_damage_taken_end_time".into(), self.dmg_taken_end),
            Column::new(
                "player_damage_taken_last_damage_time".into(),
                self.dmg_taken_last,
            ),
            Column::new(
                "player_damage_taken_start_time".into(),
                self.dmg_taken_start,
            ),
            Column::new("time_revealed_by_npc".into(), self.time_revealed),
            Column::new("build_id".into(), self.build_id),
            Column::new("is_alive".into(), self.is_alive),
            Column::new("has_rebirth".into(), self.has_rebirth),
            Column::new("has_rejuvenator".into(), self.has_rejuvenator),
            Column::new("has_ultimate_trained".into(), self.has_ultimate),
            Column::new("health_regen".into(), self.health_regen),
            Column::new("ultimate_cooldown_start".into(), self.ult_cd_start),
            Column::new("ultimate_cooldown_end".into(), self.ult_cd_end),
            Column::new("ap_net_worth".into(), self.ap_nw),
            Column::new("gold_net_worth".into(), self.gold_nw),
            Column::new("denies".into(), self.denies),
            Column::new("hero_damage".into(), self.hero_damage),
            Column::new("hero_healing".into(), self.hero_healing),
            Column::new("objective_damage".into(), self.obj_damage),
            Column::new("self_healing".into(), self.self_healing),
            Column::new("kill_streak".into(), self.kill_streak),
            Column::new("last_hits".into(), self.last_hits),
            Column::new("level".into(), self.level),
            Column::new("kills".into(), self.kills),
            Column::new("deaths".into(), self.deaths),
            Column::new("assists".into(), self.assists),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))
    }
}

/// `world_ticks` field keys (on `CCitadelGameRulesProxy`).
#[derive(Clone, Copy, Default)]
pub(super) struct WkKeys {
    pub(super) is_paused: Option<u64>,
    pub(super) next_midboss: Option<u64>,
}

impl WkKeys {
    pub(super) fn resolve(ctx: &boon_parser::Context) -> Self {
        let s = ctx.serializers().get("CCitadelGameRulesProxy");
        Self {
            is_paused: s.and_then(|s| s.resolve_field_key("m_pGameRules.m_bGamePaused")),
            next_midboss: s
                .and_then(|s| s.resolve_field_key("m_pGameRules.m_tNextMidBossSpawnTime")),
        }
    }
}

/// `world_ticks` column vectors (one row per tick).
#[derive(Default)]
pub(super) struct WtCols {
    pub(super) tick: Vec<i32>,
    pub(super) is_paused: Vec<bool>,
    pub(super) next_midboss: Vec<f32>,
}

impl WtCols {
    pub(super) fn collect_tick(&mut self, ctx: &boon_parser::Context, k: &WkKeys) {
        if let Some((_, e)) = ctx
            .entities()
            .iter()
            .find(|(_, e)| e.class_name.as_ref() == "CCitadelGameRulesProxy")
        {
            self.tick.push(ctx.tick());
            self.is_paused.push(e.get_bool(k.is_paused));
            self.next_midboss.push(e.get_f32(k.next_midboss));
        }
    }

    pub(super) fn append(&mut self, mut o: WtCols) {
        self.tick.append(&mut o.tick);
        self.is_paused.append(&mut o.is_paused);
        self.next_midboss.append(&mut o.next_midboss);
    }

    pub(super) fn into_dataframe(self) -> PyResult<DataFrame> {
        df_from_columns(vec![
            Column::new("tick".into(), self.tick),
            Column::new("is_paused".into(), self.is_paused),
            Column::new("next_midboss".into(), self.next_midboss),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))
    }
}

/// `troopers` field keys (on `CNPC_Trooper` / `CNPC_TrooperBoss`).
#[derive(Clone, Copy, Default)]
pub(super) struct TkKeys {
    pub(super) health: Option<u64>,
    pub(super) max_health: Option<u64>,
    pub(super) team_num: Option<u64>,
    pub(super) lane: Option<u64>,
    pub(super) lifestate: Option<u64>,
    pub(super) vec_x: Option<u64>,
    pub(super) vec_y: Option<u64>,
    pub(super) vec_z: Option<u64>,
    pub(super) cell_x: Option<u64>,
    pub(super) cell_y: Option<u64>,
    pub(super) cell_z: Option<u64>,
}

impl TkKeys {
    pub(super) fn resolve(ctx: &boon_parser::Context) -> Self {
        let s = ctx
            .serializers()
            .get("CNPC_Trooper")
            .or_else(|| ctx.serializers().get("CNPC_TrooperBoss"));
        let f = |name: &str| s.and_then(|s| s.resolve_field_key(name));
        Self {
            health: f("m_iHealth"),
            max_health: f("m_iMaxHealth"),
            team_num: f("m_iTeamNum"),
            lane: f("m_iLane"),
            lifestate: f("m_lifeState"),
            vec_x: f("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX"),
            vec_y: f("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY"),
            vec_z: f("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ"),
            cell_x: f("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX"),
            cell_y: f("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY"),
            cell_z: f("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ"),
        }
    }
}

/// `troopers` column vectors (one row per alive lane trooper per tick).
#[derive(Default)]
pub(super) struct TrCols {
    pub(super) tick: Vec<i32>,
    pub(super) ttype: Vec<String>,
    pub(super) team_num: Vec<i64>,
    pub(super) lane: Vec<i64>,
    pub(super) health: Vec<i64>,
    pub(super) max_health: Vec<i64>,
    pub(super) x: Vec<f32>,
    pub(super) y: Vec<f32>,
    pub(super) z: Vec<f32>,
    pub(super) entity_id: Vec<i32>,
}

impl TrCols {
    pub(super) fn collect_tick(&mut self, ctx: &boon_parser::Context, k: &TkKeys) {
        for (idx, e) in ctx.entities().iter() {
            if !e.active {
                continue;
            }
            let ttype = match e.class_name.as_ref() {
                "CNPC_Trooper" => "trooper",
                "CNPC_TrooperBoss" => "trooper_boss",
                _ => continue,
            };
            let max_hp = e.get_i64(k.max_health);
            if max_hp == 0 {
                continue;
            }
            if e.get_i64(k.lifestate) != 0 {
                continue;
            }
            self.tick.push(ctx.tick());
            self.ttype.push(ttype.to_string());
            self.team_num.push(e.get_i64(k.team_num));
            self.lane.push(e.get_i64(k.lane));
            self.health.push(e.get_i64(k.health));
            self.max_health.push(max_hp);
            let [x, y, z] =
                e.world_position([k.cell_x, k.cell_y, k.cell_z], [k.vec_x, k.vec_y, k.vec_z]);
            self.x.push(x);
            self.y.push(y);
            self.z.push(z);
            self.entity_id.push(idx);
        }
    }

    pub(super) fn append(&mut self, mut o: TrCols) {
        self.tick.append(&mut o.tick);
        self.ttype.append(&mut o.ttype);
        self.team_num.append(&mut o.team_num);
        self.lane.append(&mut o.lane);
        self.health.append(&mut o.health);
        self.max_health.append(&mut o.max_health);
        self.x.append(&mut o.x);
        self.y.append(&mut o.y);
        self.z.append(&mut o.z);
        self.entity_id.append(&mut o.entity_id);
    }

    pub(super) fn into_dataframe(self) -> PyResult<DataFrame> {
        df_from_columns(vec![
            Column::new("tick".into(), self.tick),
            Column::new("trooper_type".into(), self.ttype),
            Column::new("team_num".into(), self.team_num),
            Column::new("lane".into(), self.lane),
            Column::new("health".into(), self.health),
            Column::new("max_health".into(), self.max_health),
            Column::new("x".into(), self.x),
            Column::new("y".into(), self.y),
            Column::new("z".into(), self.z),
            Column::new("entity_id".into(), self.entity_id),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))
    }
}

/// Which snapshot datasets a parallel pass should collect.
#[derive(Clone, Copy, Default)]
pub(super) struct SnapWants {
    pub(super) player_ticks: bool,
    pub(super) world_ticks: bool,
    pub(super) troopers: bool,
}

impl SnapWants {
    pub(super) fn any(self) -> bool {
        self.player_ticks || self.world_ticks || self.troopers
    }
}

/// All snapshot field keys, resolved once from the send tables.
pub(super) struct SnapKeys {
    pub(super) pt: PtKeys,
    pub(super) wk: WkKeys,
    pub(super) tk: TkKeys,
}

/// One segment's accumulated snapshot columns.
#[derive(Default)]
pub(super) struct SegSnap {
    pub(super) pt: PtCols,
    pub(super) wt: WtCols,
    pub(super) tr: TrCols,
    pub(super) barriers: BarrierState,
}

impl SegSnap {
    pub(super) fn update(&mut self, ctx: &boon_parser::Context, wants: SnapWants) {
        if wants.player_ticks {
            self.barriers.update(ctx);
        }
    }

    pub(super) fn collect_tick(
        &mut self,
        ctx: &boon_parser::Context,
        keys: &SnapKeys,
        wants: SnapWants,
    ) {
        if wants.player_ticks {
            self.pt.collect_tick(ctx, &keys.pt, &self.barriers);
        }
        if wants.world_ticks {
            self.wt.collect_tick(ctx, &keys.wk);
        }
        if wants.troopers {
            self.tr.collect_tick(ctx, &keys.tk);
        }
    }

    pub(super) fn append(&mut self, o: SegSnap) {
        self.pt.append(o.pt);
        self.wt.append(o.wt);
        self.tr.append(o.tr);
    }
}

/// Which ticks a snapshot pass collects rows at. Resolved up front so it is
/// independent of how the demo is split into parallel segments.
pub(super) enum TickPredicate {
    /// Every tick.
    All,
    /// Every tick within `[start, end]`.
    Window { start: i32, end: i32 },
    /// The explicit tick set, within `[start, end]`.
    Set {
        ticks: std::collections::HashSet<i32>,
        start: i32,
        end: i32,
    },
}

impl TickPredicate {
    #[inline]
    pub(super) fn matches(&self, t: i32) -> bool {
        match self {
            TickPredicate::All => true,
            TickPredicate::Window { start, end } => t >= *start && t <= *end,
            TickPredicate::Set { ticks, start, end } => {
                t >= *start && t <= *end && ticks.contains(&t)
            }
        }
    }
}
