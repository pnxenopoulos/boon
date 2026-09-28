use crate::*;

/// Transfer primitive buffers into Polars without copying their values.
fn numeric_column<T>(name: &str, values: Vec<T>) -> Column
where
    Series: NamedFromOwned<Vec<T>>,
{
    Series::from_vec(name.into(), values).into_column()
}

// ─────────────────────────── Parallel player_ticks ───────────────────────────
//
// `player_ticks` is a per-tick full snapshot of player pawn + controller state.
// Both classes are re-keyframed at every `DEM_FullPacket`, so the demo can be
// split at those keyframes and each segment decoded on its own thread, then the
// per-segment rows concatenated in order — identical to a single serial pass.
// See `Parser::decode_segment` / `full_packet_offsets`.

/// Get the number of keyframe segments for parallel `player_ticks` decoding.
///
/// The default is the CPU count. `BOON_TICK_SEGMENTS` overrides the default.
/// A value of `1` disables parallel processing. Read the value for each call so
/// that tests can select serial processing.
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

// The vector is usable only when the serializer exposes its count and entry fields.
pub(super) fn stat_viewer_values_available(
    count: Option<u64>,
    keys: &[StatViewerKeys; STAT_VIEWER_SLOTS],
) -> bool {
    count.is_some() && keys[0].value_type.is_some() && keys[0].value.is_some()
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
}

impl PtKeys {
    pub(super) fn resolve(ctx: &boon_parser::Context) -> Self {
        let pawn = ctx.serializers().get("CCitadelPlayerPawn");
        let ctrl = ctx.serializers().get("CCitadelPlayerController");
        let p = |name: &str| pawn.and_then(|s| s.resolve_field_key(name));
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

/// Player positions used by analyses that do not need all player-tick columns.
#[derive(Default)]
pub(super) struct PlayerPositionCols {
    pub(super) tick: Vec<i32>,
    pub(super) hero_id: Vec<i64>,
    pub(super) x: Vec<f32>,
    pub(super) y: Vec<f32>,
}

impl PlayerPositionCols {
    pub(super) fn collect_tick(&mut self, ctx: &boon_parser::Context, keys: &PtKeys) {
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
            let [x, y, _] = pawn.world_position(
                [keys.cell_x, keys.cell_y, keys.cell_z],
                [keys.vec_x, keys.vec_y, keys.vec_z],
            );
            self.tick.push(ctx.tick());
            self.hero_id.push(hero_id);
            self.x.push(x);
            self.y.push(y);
        }
    }

    pub(super) fn into_dataframe(self) -> PyResult<DataFrame> {
        df_from_columns(vec![
            numeric_column("tick", self.tick),
            numeric_column("hero_id", self.hero_id),
            numeric_column("x", self.x),
            numeric_column("y", self.y),
        ])
        .map_err(|error| {
            InvalidDemoError::new_err(format!("Failed to create position DataFrame: {error}"))
        })
    }
}

/// Live barrier remaining, decoded from each pawn's persistent
/// `modifier_barrier_tracker` entry in the `ActiveModifiers` string table.
/// Deadlock stores barrier capacity in `float1` and the current amount in
/// `float2`; demos without that tracker naturally stay at zero.
pub(super) const BARRIER_TRACKER_MODIFIER_ID: u32 = 4_267_845_006; // modifier_barrier_tracker

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
        let mut modifiers = std::mem::take(&mut self.modifiers);
        modifiers.rebuild(ctx);
        self.remaining_by_pawn.clear();
        self.serial_to_pawn.clear();
        self.pawn_to_serial.clear();
        for (&serial, entry) in modifiers.entries() {
            self.apply_live_entry(serial, entry);
        }
        self.modifiers = modifiers;
    }

    pub(super) fn remaining(&self, pawn_handle: u32) -> f32 {
        boon_parser::protobuf_handle_index(Some(pawn_handle))
            .and_then(|idx| self.remaining_by_pawn.get(&idx).copied())
            .unwrap_or(0.0)
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
    pub(super) stat_modifiers: [Vec<f32>; boon_parser::StatModifierKind::COUNT],
    pub(super) stat_modifier_values_available: Vec<bool>,
    pub(super) unknown_stat_modifier_count: Vec<u32>,
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
            let stat_modifier_values_available =
                stat_viewer_values_available(k.stat_viewer_count, &k.stat_viewer);
            let stat_modifier_count = ctrl
                .get_i64(k.stat_viewer_count)
                .clamp(0, STAT_VIEWER_SLOTS as i64) as usize;
            let stat_modifier_totals = boon_parser::aggregate_stat_modifier_values(
                k.stat_viewer[..stat_modifier_count]
                    .iter()
                    .map(|keys| (ctrl.get_u32(keys.value_type), ctrl.get_f32(keys.value))),
            );
            for kind in boon_parser::StatModifierKind::ALL {
                self.stat_modifiers[kind.index()].push(stat_modifier_totals[kind]);
            }
            self.stat_modifier_values_available
                .push(stat_modifier_values_available);
            self.unknown_stat_modifier_count
                .push(stat_modifier_totals.unknown_count);
            let level = ctrl.get_i64(k.level);
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

    /// Build the `player_ticks` DataFrame. Column order/names must match `load()`.
    pub(super) fn into_dataframe(self) -> PyResult<DataFrame> {
        let [
            stat_modifier_health,
            stat_modifier_spirit_power,
            stat_modifier_fire_rate,
            stat_modifier_weapon_damage,
            stat_modifier_cooldown_reduction,
            stat_modifier_ammo,
            stat_modifier_bullet_resist,
            stat_modifier_spirit_resist,
        ] = self.stat_modifiers;
        df_from_columns(vec![
            numeric_column("tick", self.tick),
            numeric_column("hero_id", self.hero_id),
            numeric_column("x", self.x),
            numeric_column("y", self.y),
            numeric_column("z", self.z),
            numeric_column("pitch", self.pitch),
            numeric_column("yaw", self.yaw),
            numeric_column("roll", self.roll),
            Column::new("in_regen_zone".into(), self.in_regen_zone),
            Column::new("in_item_shop".into(), self.in_item_shop),
            numeric_column("death_time", self.death_time),
            numeric_column("last_spawn_time", self.last_spawn_time),
            numeric_column("respawn_time", self.respawn_time),
            numeric_column("health", self.health),
            numeric_column("max_health", self.max_health),
            numeric_column("barrier", self.barrier),
            numeric_column("stat_modifier_health", stat_modifier_health),
            numeric_column("stat_modifier_spirit_power", stat_modifier_spirit_power),
            numeric_column("stat_modifier_fire_rate", stat_modifier_fire_rate),
            numeric_column("stat_modifier_weapon_damage", stat_modifier_weapon_damage),
            numeric_column(
                "stat_modifier_cooldown_reduction",
                stat_modifier_cooldown_reduction,
            ),
            numeric_column("stat_modifier_ammo", stat_modifier_ammo),
            numeric_column("stat_modifier_bullet_resist", stat_modifier_bullet_resist),
            numeric_column("stat_modifier_spirit_resist", stat_modifier_spirit_resist),
            Column::new(
                "stat_modifier_values_available".into(),
                self.stat_modifier_values_available,
            ),
            numeric_column(
                "unknown_stat_modifier_count",
                self.unknown_stat_modifier_count,
            ),
            numeric_column("lifestate", self.lifestate),
            numeric_column("souls", self.souls),
            numeric_column("spent_souls", self.spent_souls),
            numeric_column("in_combat_end_time", self.combat_end),
            numeric_column("in_combat_last_damage_time", self.combat_last_dmg),
            numeric_column("in_combat_start_time", self.combat_start),
            numeric_column("player_damage_dealt_end_time", self.dmg_dealt_end),
            numeric_column("player_damage_dealt_last_damage_time", self.dmg_dealt_last),
            numeric_column("player_damage_dealt_start_time", self.dmg_dealt_start),
            numeric_column("player_damage_taken_end_time", self.dmg_taken_end),
            numeric_column("player_damage_taken_last_damage_time", self.dmg_taken_last),
            numeric_column("player_damage_taken_start_time", self.dmg_taken_start),
            numeric_column("time_revealed_by_npc", self.time_revealed),
            numeric_column("build_id", self.build_id),
            Column::new("is_alive".into(), self.is_alive),
            Column::new("has_rebirth".into(), self.has_rebirth),
            Column::new("has_rejuvenator".into(), self.has_rejuvenator),
            Column::new("has_ultimate_trained".into(), self.has_ultimate),
            numeric_column("health_regen", self.health_regen),
            numeric_column("ultimate_cooldown_start", self.ult_cd_start),
            numeric_column("ultimate_cooldown_end", self.ult_cd_end),
            numeric_column("ap_net_worth", self.ap_nw),
            numeric_column("gold_net_worth", self.gold_nw),
            numeric_column("denies", self.denies),
            numeric_column("hero_damage", self.hero_damage),
            numeric_column("hero_healing", self.hero_healing),
            numeric_column("objective_damage", self.obj_damage),
            numeric_column("self_healing", self.self_healing),
            numeric_column("kill_streak", self.kill_streak),
            numeric_column("last_hits", self.last_hits),
            numeric_column("level", self.level),
            numeric_column("kills", self.kills),
            numeric_column("deaths", self.deaths),
            numeric_column("assists", self.assists),
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

    pub(super) fn into_dataframe(self) -> PyResult<DataFrame> {
        df_from_columns(vec![
            numeric_column("tick", self.tick),
            Column::new("is_paused".into(), self.is_paused),
            numeric_column("next_midboss", self.next_midboss),
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
    pub(super) ttype: Vec<&'static str>,
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
            self.ttype.push(ttype);
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

    pub(super) fn into_dataframe(self) -> PyResult<DataFrame> {
        df_from_columns(vec![
            numeric_column("tick", self.tick),
            Column::new("trooper_type".into(), self.ttype),
            numeric_column("team_num", self.team_num),
            numeric_column("lane", self.lane),
            numeric_column("health", self.health),
            numeric_column("max_health", self.max_health),
            numeric_column("x", self.x),
            numeric_column("y", self.y),
            numeric_column("z", self.z),
            numeric_column("entity_id", self.entity_id),
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

pub(super) type SnapshotFrames = (Option<DataFrame>, Option<DataFrame>, Option<DataFrame>);

/// One segment's accumulated snapshot columns.
#[derive(Default)]
pub(super) struct SegSnap {
    pub(super) pt: PtCols,
    pub(super) wt: WtCols,
    pub(super) tr: TrCols,
    pub(super) barriers: BarrierState,
}

impl SegSnap {
    pub(super) fn into_frames(self, wants: SnapWants) -> PyResult<SnapshotFrames> {
        Ok((
            wants
                .player_ticks
                .then(|| self.pt.into_dataframe())
                .transpose()?,
            wants
                .world_ticks
                .then(|| self.wt.into_dataframe())
                .transpose()?,
            wants
                .troopers
                .then(|| self.tr.into_dataframe())
                .transpose()?,
        ))
    }

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
