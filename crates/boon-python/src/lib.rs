use std::collections::HashMap;
use std::path::PathBuf;

use boon_proto::proto::CitadelUserMessageIds as Msg;
use polars::prelude::*;
use prost::Message;
use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyFileNotFoundError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3_polars::PyDataFrame;

pyo3::create_exception!(_boon, InvalidDemoError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, DemoHeaderError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, DemoInfoError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, DemoMessageError, pyo3::exceptions::PyException);
pyo3::create_exception!(_boon, NotStreetBrawlError, pyo3::exceptions::PyException);

/// Build a `DataFrame` from columns, inferring row count from the first column.
fn df_from_columns(columns: Vec<Column>) -> PolarsResult<DataFrame> {
    let height = columns.first().map_or(0, |c| c.len());
    DataFrame::new(height, columns)
}

/// Helper to convert boon errors to Python exceptions.
fn to_py_err(e: boon_parser::Error) -> PyErr {
    match e {
        boon_parser::Error::Io(io_err) => PyErr::from(io_err),
        boon_parser::Error::InvalidMagic { got } => {
            InvalidDemoError::new_err(format!("Invalid demo file: bad magic bytes {got:?}"))
        }
        boon_parser::Error::Parse { context } => {
            InvalidDemoError::new_err(format!("Invalid demo file: {context}"))
        }
        other => InvalidDemoError::new_err(format!("{other}")),
    }
}

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
fn parallel_segments() -> usize {
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

/// Split the full-packet offsets into `n` contiguous `(start_offset, end_tick)`
/// segments: segment 0 starts from the signon baseline (`None`), the rest
/// cold-restart at an evenly spaced full packet.
fn segment_ranges(offsets: &[(usize, i32)], n: usize) -> Vec<(Option<usize>, i32)> {
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
struct PtKeys {
    hero_id: Option<u64>,
    vec_x: Option<u64>,
    vec_y: Option<u64>,
    vec_z: Option<u64>,
    cell_x: Option<u64>,
    cell_y: Option<u64>,
    cell_z: Option<u64>,
    camera: Option<u64>,
    in_regen: Option<u64>,
    in_item_shop: Option<u64>,
    death_time: Option<u64>,
    last_spawn: Option<u64>,
    respawn: Option<u64>,
    health: Option<u64>,
    max_health: Option<u64>,
    lifestate: Option<u64>,
    souls: Option<u64>,
    spent_souls: Option<u64>,
    combat_end: Option<u64>,
    combat_last_dmg: Option<u64>,
    combat_start: Option<u64>,
    dmg_dealt_end: Option<u64>,
    dmg_dealt_last: Option<u64>,
    dmg_dealt_start: Option<u64>,
    dmg_taken_end: Option<u64>,
    dmg_taken_last: Option<u64>,
    dmg_taken_start: Option<u64>,
    time_revealed: Option<u64>,
    build_id: Option<u64>,
    pawn_handle: Option<u64>,
    health_max: Option<u64>,
    alive: Option<u64>,
    rebirth: Option<u64>,
    rejuvenator: Option<u64>,
    ultimate: Option<u64>,
    health_regen: Option<u64>,
    ult_cd_end: Option<u64>,
    ult_cd_start: Option<u64>,
    ap_nw: Option<u64>,
    gold_nw: Option<u64>,
    denies: Option<u64>,
    hero_damage: Option<u64>,
    hero_healing: Option<u64>,
    obj_damage: Option<u64>,
    self_healing: Option<u64>,
    kill_streak: Option<u64>,
    last_hits: Option<u64>,
    level: Option<u64>,
    kills: Option<u64>,
    deaths: Option<u64>,
    assists: Option<u64>,
}

impl PtKeys {
    fn resolve(ctx: &boon_parser::Context) -> Self {
        let pawn = ctx.serializers().get("CCitadelPlayerPawn");
        let ctrl = ctx.serializers().get("CCitadelPlayerController");
        let p = |name: &str| pawn.and_then(|s| s.resolve_field_key(name));
        let c = |name: &str| ctrl.and_then(|s| s.resolve_field_key(name));
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
        }
    }
}

/// Column vectors accumulated for `player_ticks`. One per output column; the
/// order and names in [`into_columns`](PtCols::into_columns) must match the
/// serial builder in `load()`.
#[derive(Default)]
struct PtCols {
    tick: Vec<i32>,
    hero_id: Vec<i64>,
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
    pitch: Vec<f32>,
    yaw: Vec<f32>,
    roll: Vec<f32>,
    in_regen_zone: Vec<bool>,
    in_item_shop: Vec<bool>,
    death_time: Vec<f32>,
    last_spawn_time: Vec<f32>,
    respawn_time: Vec<f32>,
    health: Vec<i64>,
    max_health: Vec<i64>,
    lifestate: Vec<i64>,
    souls: Vec<i64>,
    spent_souls: Vec<i64>,
    combat_end: Vec<f32>,
    combat_last_dmg: Vec<f32>,
    combat_start: Vec<f32>,
    dmg_dealt_end: Vec<f32>,
    dmg_dealt_last: Vec<f32>,
    dmg_dealt_start: Vec<f32>,
    dmg_taken_end: Vec<f32>,
    dmg_taken_last: Vec<f32>,
    dmg_taken_start: Vec<f32>,
    time_revealed: Vec<f32>,
    build_id: Vec<i64>,
    is_alive: Vec<bool>,
    has_rebirth: Vec<bool>,
    has_rejuvenator: Vec<bool>,
    has_ultimate: Vec<bool>,
    health_regen: Vec<f32>,
    ult_cd_start: Vec<f32>,
    ult_cd_end: Vec<f32>,
    ap_nw: Vec<i64>,
    gold_nw: Vec<i64>,
    denies: Vec<i64>,
    hero_damage: Vec<i64>,
    hero_healing: Vec<i64>,
    obj_damage: Vec<i64>,
    self_healing: Vec<i64>,
    kill_streak: Vec<i64>,
    last_hits: Vec<i64>,
    level: Vec<i64>,
    kills: Vec<i64>,
    deaths: Vec<i64>,
    assists: Vec<i64>,
}

impl PtCols {
    /// Append one snapshot row per live player at `ctx.tick()` (mirrors the serial
    /// collector in `load()`; must stay in sync with it).
    fn collect_tick(&mut self, ctx: &boon_parser::Context, k: &PtKeys) {
        for (_, ctrl) in ctx
            .entities()
            .iter()
            .filter(|(_, e)| e.class_name.as_ref() == "CCitadelPlayerController")
        {
            let pawn = match ctrl
                .get_handle(k.pawn_handle)
                .and_then(|h| ctx.entities().get_by_handle(h))
            {
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
            self.level.push(ctrl.get_i64(k.level));
            self.kills.push(ctrl.get_i64(k.kills));
            self.deaths.push(ctrl.get_i64(k.deaths));
            self.assists.push(ctrl.get_i64(k.assists));
        }
    }

    /// Append another segment's rows onto this one (segments are joined in order).
    fn append(&mut self, mut o: PtCols) {
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
    fn into_dataframe(self) -> PyResult<DataFrame> {
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
struct WkKeys {
    is_paused: Option<u64>,
    next_midboss: Option<u64>,
}

impl WkKeys {
    fn resolve(ctx: &boon_parser::Context) -> Self {
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
struct WtCols {
    tick: Vec<i32>,
    is_paused: Vec<bool>,
    next_midboss: Vec<f32>,
}

impl WtCols {
    fn collect_tick(&mut self, ctx: &boon_parser::Context, k: &WkKeys) {
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

    fn append(&mut self, mut o: WtCols) {
        self.tick.append(&mut o.tick);
        self.is_paused.append(&mut o.is_paused);
        self.next_midboss.append(&mut o.next_midboss);
    }

    fn into_dataframe(self) -> PyResult<DataFrame> {
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
struct TkKeys {
    health: Option<u64>,
    max_health: Option<u64>,
    team_num: Option<u64>,
    lane: Option<u64>,
    lifestate: Option<u64>,
    vec_x: Option<u64>,
    vec_y: Option<u64>,
    vec_z: Option<u64>,
    cell_x: Option<u64>,
    cell_y: Option<u64>,
    cell_z: Option<u64>,
}

impl TkKeys {
    fn resolve(ctx: &boon_parser::Context) -> Self {
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
struct TrCols {
    tick: Vec<i32>,
    ttype: Vec<String>,
    team_num: Vec<i64>,
    lane: Vec<i64>,
    health: Vec<i64>,
    max_health: Vec<i64>,
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
    entity_id: Vec<i32>,
}

impl TrCols {
    fn collect_tick(&mut self, ctx: &boon_parser::Context, k: &TkKeys) {
        for (idx, e) in ctx.entities().iter() {
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

    fn append(&mut self, mut o: TrCols) {
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

    fn into_dataframe(self) -> PyResult<DataFrame> {
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
struct SnapWants {
    player_ticks: bool,
    world_ticks: bool,
    troopers: bool,
}

impl SnapWants {
    fn any(self) -> bool {
        self.player_ticks || self.world_ticks || self.troopers
    }
}

/// All snapshot field keys, resolved once from the send tables.
struct SnapKeys {
    pt: PtKeys,
    wk: WkKeys,
    tk: TkKeys,
}

/// One segment's accumulated snapshot columns.
#[derive(Default)]
struct SegSnap {
    pt: PtCols,
    wt: WtCols,
    tr: TrCols,
}

impl SegSnap {
    fn collect_tick(&mut self, ctx: &boon_parser::Context, keys: &SnapKeys, wants: SnapWants) {
        if wants.player_ticks {
            self.pt.collect_tick(ctx, &keys.pt);
        }
        if wants.world_ticks {
            self.wt.collect_tick(ctx, &keys.wk);
        }
        if wants.troopers {
            self.tr.collect_tick(ctx, &keys.tk);
        }
    }

    fn append(&mut self, o: SegSnap) {
        self.pt.append(o.pt);
        self.wt.append(o.wt);
        self.tr.append(o.tr);
    }
}

/// Which ticks a snapshot pass collects rows at. Resolved up front so it is
/// independent of how the demo is split into parallel segments.
enum TickPredicate {
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
    fn matches(&self, t: i32) -> bool {
        match self {
            TickPredicate::All => true,
            TickPredicate::Window { start, end } => t >= *start && t <= *end,
            TickPredicate::Set { ticks, start, end } => {
                t >= *start && t <= *end && ticks.contains(&t)
            }
        }
    }
}

/// A `str` or `list[str]` Python argument (e.g. `datasets=`, `events=`).
#[derive(FromPyObject)]
enum StrOrList {
    #[pyo3(transparent)]
    One(String),
    #[pyo3(transparent)]
    Many(Vec<String>),
}

impl StrOrList {
    fn into_vec(self) -> Vec<String> {
        match self {
            StrOrList::One(s) => vec![s],
            StrOrList::Many(v) => v,
        }
    }
}

/// An `int` or `list[int]` Python argument (e.g. `ticks=`).
#[derive(FromPyObject)]
enum IntOrList {
    #[pyo3(transparent)]
    One(i32),
    #[pyo3(transparent)]
    Many(Vec<i32>),
}

impl IntOrList {
    fn into_vec(self) -> Vec<i32> {
        match self {
            IntOrList::One(t) => vec![t],
            IntOrList::Many(v) => v,
        }
    }
}

const VALID_DATASETS: &[&str] = &[
    "abilities",
    "ability_upgrades",
    "ability_ticks",
    "chat",
    "mid_boss",
    "objectives",
    "player_ticks",
    "world_ticks",
    "kills",
    "damage",
    "flex_slots",
    "item_purchases",
    "troopers",
    "neutrals",
    "stat_modifier_events",
    "active_modifiers",
    "urn",
    "rift",
];

const VALID_STREET_BRAWL_DATASETS: &[&str] = &["street_brawl_ticks", "street_brawl_rounds"];

/// Known Rift ("Koth") cash-in sites, as `([x, y], lane)`.
///
/// The Rift entities carry no `m_iLane` field, so the lane has to come from the
/// cash-in location. Each site below was cross-checked against the lane of the
/// buffed trooper cohort that spawns for the winning team after a capture. Only
/// these two sites have been observed, so any other location resolves to lane
/// `0` rather than being guessed at.
const RIFT_LANE_SITES: &[([f32; 2], i64)] = &[([-7560.0, 0.0], 1), ([7612.0, 0.0], 6)];

/// Match radius, in Hammer units, for associating a location with a known Rift
/// site. The two known sites are ~15k units apart, so this is deliberately
/// loose enough to absorb per-match jitter without ever matching both.
const RIFT_LANE_TOLERANCE: f32 = 1024.0;

/// Upper bound, in Hammer units, on a plausible map coordinate.
///
/// The game clears `m_vKothCashInCurrentLocation` to `FLT_MAX` rather than to
/// zero once a Rift resolves. `FLT_MAX` is finite, so an `is_finite` check does
/// not reject it — this bound does.
const RIFT_COORD_SANITY: f32 = 1.0e6;

/// The lane for a Rift cash-in location, or `0` when the location is not a
/// known Rift site (see [`RIFT_LANE_SITES`]).
fn rift_lane_for(x: f32, y: f32) -> i64 {
    if !x.is_finite() || !y.is_finite() {
        return 0;
    }
    for ([sx, sy], lane) in RIFT_LANE_SITES {
        if (x - sx).abs() <= RIFT_LANE_TOLERANCE && (y - sy).abs() <= RIFT_LANE_TOLERANCE {
            return *lane;
        }
    }
    0
}

/// A Deadlock demo file.
///
/// Args:
///     path: Path to the demo file.
///
/// Raises:
///     FileNotFoundError: If the file does not exist.
///     InvalidDemoError: If the file is not a valid demo file.
#[pyclass]
struct Demo {
    parser: boon_parser::Parser,
    path: PathBuf,
    // Cached info from file_header
    build: i32,
    map_name: String,
    // Cached info from file_info
    total_ticks: i32,
    playback_time: f32,
    tick_rate: i32,
    // Cached info from first tick entities
    match_id: Option<u64>,
    game_mode: i64,
    // Sorted ticks where the game was paused (lazily built from world_ticks)
    paused_ticks: Option<Vec<i32>>,
    // Cached dataset DataFrames
    cached_player_ticks: Option<DataFrame>,
    cached_world_ticks: Option<DataFrame>,
    cached_kills: Option<DataFrame>,
    cached_damage: Option<DataFrame>,
    // Game over state: (winning_team_num, tick), None if no event found
    game_over: Option<(i32, i32)>,
    // Hero IDs from the one-shot `BannedHeroes` message. `Some(vec![])` means
    // the demo was scanned and carried no bans; `None` means not scanned yet.
    banned_hero_ids: Option<Vec<u32>>,
    always_events_scanned: bool,
    // Flex slot unlock events
    cached_flex_slots: Option<DataFrame>,
    cached_abilities: Option<DataFrame>,
    cached_ability_upgrades: Option<DataFrame>,
    cached_item_purchases: Option<DataFrame>,
    cached_chat: Option<DataFrame>,
    cached_objectives: Option<DataFrame>,
    cached_mid_boss: Option<DataFrame>,
    cached_troopers: Option<DataFrame>,
    cached_neutrals: Option<DataFrame>,
    cached_stat_modifier_events: Option<DataFrame>,
    cached_active_modifiers: Option<DataFrame>,
    cached_ability_ticks: Option<DataFrame>,
    cached_players: Option<DataFrame>,
    cached_street_brawl_ticks: Option<DataFrame>,
    cached_street_brawl_rounds: Option<DataFrame>,
    cached_urn: Option<DataFrame>,
    cached_rift: Option<DataFrame>,
}

/// The (gold, orbs) a player earned from a given source at a snapshot, or
/// ``(0, 0)`` when that source is absent.
fn gold_source_totals(
    stats: &boon_proto::proto::c_msg_match_meta_data_contents::PlayerStats,
    source: boon_proto::proto::c_msg_match_meta_data_contents::EGoldSource,
) -> (u32, u32) {
    stats
        .gold_sources
        .iter()
        .find(|g| g.source == Some(source as i32))
        .map(|g| (g.gold(), g.gold_orbs()))
        .unwrap_or((0, 0))
}

/// Build the long-form ``snapshots`` DataFrame: one row per (snapshot time,
/// player), with a ``snapshot_time_s`` column plus every per-player stat. The
/// per-source gold/orbs columns come from ``PlayerStats.gold_sources`` keyed by
/// ``EGoldSource``; ``unknown_*`` is the ``k_eItemGooseEgg`` source. (The
/// scoreboard last-hit total is not per-snapshot; see ``build_last_hits_frame``.)
fn build_snapshots_frame(
    match_info: &boon_proto::proto::c_msg_match_meta_data_contents::MatchInfo,
) -> PolarsResult<DataFrame> {
    use boon_proto::proto::c_msg_match_meta_data_contents::EGoldSource;
    use std::collections::BTreeSet;

    // Stats are stored per player; take the union of timestamps so players who
    // abandoned early (fewer snapshots) are still handled correctly.
    let mut times: BTreeSet<u32> = BTreeSet::new();
    for player in &match_info.players {
        for stats in &player.stats {
            times.insert(stats.time_stamp_s());
        }
    }

    let mut snapshot_time_s = Vec::new();
    let mut hero_id = Vec::new();
    let mut kills = Vec::new();
    let mut deaths = Vec::new();
    let mut assists = Vec::new();
    let mut net_worth = Vec::new();
    let mut denies = Vec::new();
    let mut level = Vec::new();
    let mut lane = Vec::new();
    let mut creep_kills = Vec::new();
    let mut neutral_kills = Vec::new();
    let mut player_damage = Vec::new();
    let mut player_gold = Vec::new();
    let mut player_orbs = Vec::new();
    let mut lane_creep_gold = Vec::new();
    let mut lane_creep_orbs = Vec::new();
    let mut neutral_creep = Vec::new();
    let mut neutral_creep_orbs = Vec::new();
    let mut boss_gold = Vec::new();
    let mut boss_orbs = Vec::new();
    let mut treasure_gold = Vec::new();
    let mut treasure_orbs = Vec::new();
    let mut denies_gold = Vec::new();
    let mut denies_orbs = Vec::new();
    let mut team_bonus_gold = Vec::new();
    let mut team_bonus_orbs = Vec::new();
    let mut breakable_gold = Vec::new();
    let mut breakable_orbs = Vec::new();
    let mut assassinate_gold = Vec::new();
    let mut assassinate_orbs = Vec::new();
    let mut trophy_collector_gold = Vec::new();
    let mut trophy_collector_orbs = Vec::new();
    let mut cultist_sacrifice_gold = Vec::new();
    let mut cultist_sacrifice_orbs = Vec::new();
    let mut unknown_gold = Vec::new();
    let mut unknown_orbs = Vec::new();
    let mut assists_gold = Vec::new();
    let mut assists_orbs = Vec::new();

    for &time in &times {
        for player in &match_info.players {
            let Some(stats) = player.stats.iter().find(|s| s.time_stamp_s() == time) else {
                continue;
            };
            snapshot_time_s.push(time);
            hero_id.push(player.hero_id());
            kills.push(stats.kills());
            deaths.push(stats.deaths());
            assists.push(stats.assists());
            net_worth.push(stats.net_worth());
            denies.push(stats.denies());
            level.push(stats.level());
            lane.push(player.assigned_lane());
            creep_kills.push(stats.creep_kills());
            neutral_kills.push(stats.neutral_kills());
            player_damage.push(stats.player_damage());

            // Per-source gold/orbs (see EGoldSource); `gold`/`orbs` rebind per source.
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KEPlayers);
            player_gold.push(gold);
            player_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KELaneCreeps);
            lane_creep_gold.push(gold);
            lane_creep_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KENeutrals);
            neutral_creep.push(gold);
            neutral_creep_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KEBosses);
            boss_gold.push(gold);
            boss_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KETreasure);
            treasure_gold.push(gold);
            treasure_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KEDenies);
            denies_gold.push(gold);
            denies_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KETeamBonus);
            team_bonus_gold.push(gold);
            team_bonus_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KEBreakable);
            breakable_gold.push(gold);
            breakable_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KEAbilityAssassinate);
            assassinate_gold.push(gold);
            assassinate_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KEItemTrophyCollector);
            trophy_collector_gold.push(gold);
            trophy_collector_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KEItemCultistSacrifice);
            cultist_sacrifice_gold.push(gold);
            cultist_sacrifice_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KEItemGooseEgg);
            unknown_gold.push(gold);
            unknown_orbs.push(orbs);
            let (gold, orbs) = gold_source_totals(stats, EGoldSource::KEAssists);
            assists_gold.push(gold);
            assists_orbs.push(orbs);
        }
    }

    df_from_columns(vec![
        Column::new("snapshot_time_s".into(), snapshot_time_s),
        Column::new("hero_id".into(), hero_id),
        Column::new("kills".into(), kills),
        Column::new("deaths".into(), deaths),
        Column::new("assists".into(), assists),
        Column::new("net_worth".into(), net_worth),
        Column::new("denies".into(), denies),
        Column::new("level".into(), level),
        Column::new("lane".into(), lane),
        Column::new("creep_kills".into(), creep_kills),
        Column::new("neutral_kills".into(), neutral_kills),
        Column::new("player_damage".into(), player_damage),
        Column::new("player_gold".into(), player_gold),
        Column::new("player_orbs".into(), player_orbs),
        Column::new("lane_creep_gold".into(), lane_creep_gold),
        Column::new("lane_creep_orbs".into(), lane_creep_orbs),
        Column::new("neutral_creep".into(), neutral_creep),
        Column::new("neutral_creep_orbs".into(), neutral_creep_orbs),
        Column::new("boss_gold".into(), boss_gold),
        Column::new("boss_orbs".into(), boss_orbs),
        Column::new("treasure_gold".into(), treasure_gold),
        Column::new("treasure_orbs".into(), treasure_orbs),
        Column::new("denies_gold".into(), denies_gold),
        Column::new("denies_orbs".into(), denies_orbs),
        Column::new("team_bonus_gold".into(), team_bonus_gold),
        Column::new("team_bonus_orbs".into(), team_bonus_orbs),
        Column::new("breakable_gold".into(), breakable_gold),
        Column::new("breakable_orbs".into(), breakable_orbs),
        Column::new("assassinate_gold".into(), assassinate_gold),
        Column::new("assassinate_orbs".into(), assassinate_orbs),
        Column::new("trophy_collector_gold".into(), trophy_collector_gold),
        Column::new("trophy_collector_orbs".into(), trophy_collector_orbs),
        Column::new("cultist_sacrifice_gold".into(), cultist_sacrifice_gold),
        Column::new("cultist_sacrifice_orbs".into(), cultist_sacrifice_orbs),
        Column::new("unknown_gold".into(), unknown_gold),
        Column::new("unknown_orbs".into(), unknown_orbs),
        Column::new("assists_gold".into(), assists_gold),
        Column::new("assists_orbs".into(), assists_orbs),
    ])
}

/// Build the per-player ``last_hits`` DataFrame (``hero_id``, ``last_hits``).
///
/// The scoreboard last-hit (souls secured) total is only recorded once per
/// match, so it is returned separately from the time-series snapshots.
fn build_last_hits_frame(
    match_info: &boon_proto::proto::c_msg_match_meta_data_contents::MatchInfo,
) -> PolarsResult<DataFrame> {
    let mut hero_id = Vec::new();
    let mut last_hits = Vec::new();
    for player in &match_info.players {
        hero_id.push(player.hero_id());
        last_hits.push(player.last_hits());
    }
    df_from_columns(vec![
        Column::new("hero_id".into(), hero_id),
        Column::new("last_hits".into(), last_hits),
    ])
}

/// Build the post-match ``objectives`` DataFrame from match metadata. One row
/// per objective; ``destroyed_time_s``/``first_damage_time_s`` are null when
/// the objective was never destroyed/damaged.
fn build_objectives_frame(
    match_info: &boon_proto::proto::c_msg_match_meta_data_contents::MatchInfo,
) -> PolarsResult<DataFrame> {
    let mut team_objective_id = Vec::new();
    let mut team = Vec::new();
    let mut destroyed_time_s: Vec<Option<u32>> = Vec::new();
    let mut first_damage_time_s: Vec<Option<u32>> = Vec::new();
    let mut creep_damage = Vec::new();
    let mut player_damage = Vec::new();
    let mut player_spirit_damage = Vec::new();

    for obj in &match_info.objectives {
        team_objective_id.push(obj.team_objective_id() as i32);
        team.push(obj.team() as i32);
        destroyed_time_s.push(obj.destroyed_time_s);
        first_damage_time_s.push(obj.first_damage_time_s);
        creep_damage.push(obj.creep_damage());
        player_damage.push(obj.player_damage());
        player_spirit_damage.push(obj.player_spirit_damage());
    }

    df_from_columns(vec![
        Column::new("team_objective_id".into(), team_objective_id),
        Column::new("team".into(), team),
        Column::new("destroyed_time_s".into(), destroyed_time_s),
        Column::new("first_damage_time_s".into(), first_damage_time_s),
        Column::new("creep_damage".into(), creep_damage),
        Column::new("player_damage".into(), player_damage),
        Column::new("player_spirit_damage".into(), player_spirit_damage),
    ])
}

/// Human-readable label for a damage-matrix ``EStatType`` value.
fn stat_type_label(stat_type: i32) -> String {
    match stat_type {
        0 => "damage".to_string(),
        1 => "healing".to_string(),
        2 => "heal_prevented".to_string(),
        3 => "mitigated".to_string(),
        4 => "lethal".to_string(),
        5 => "regen".to_string(),
        other => format!("unknown_{other}"),
    }
}

/// Whether a ``source_name`` is one of Valve's coarse damage-type buckets
/// (``Bullet``/``Ability``/``Melee``/``Misc``/``UnknownAbility``) rather than a
/// specific source. Coarse buckets use Capitalized display names; specific
/// sources use snake_case identifiers (e.g. ``citadel_weapon_astro_set``).
fn is_category_source(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase) && !name.contains('_')
}

/// Build the post-match ``damage`` DataFrame from the damage matrix: one row per
/// (``dealer_player_slot``, ``target_player_slot``, ``source_name``,
/// ``sample_time_s``). ``dealer_hero_id``/``target_hero_id`` resolve the slots
/// to heroes (null for non-player slots such as 0), so the frame joins to
/// ``snapshots``/``last_hits`` on ``hero_id``. ``damage`` is the per-interval
/// value of ``stat_type``
/// (``damage``/``healing``/``heal_prevented``/``mitigated``/``lethal``/``regen``)
/// dealt from dealer to target during the interval ending at that sample. It is
/// additive: ``sum`` for totals, ``cumsum`` over ``sample_time_s`` for the
/// running total.
///
/// Empty when the demo has no damage matrix. The underlying cumulative arrays
/// are shorter than the full sample list when the pair met mid-match, so they
/// are aligned to the end of ``sample_time_s`` (verified against the snapshot
/// ``player_damage``).
///
/// The matrix records the same hit twice: under a coarse category
/// (``is_category`` true) and under a specific source (``is_category`` false).
/// Categories only exist for ``damage`` (and some ``mitigated``); every other
/// stat type is specific-only. Summing all rows therefore double-counts
/// ``damage`` — filter to ``is_category == False`` for the complete, never
/// double-counted per-source breakdown across all stat types.
fn build_damage_frame(
    match_info: &boon_proto::proto::c_msg_match_meta_data_contents::MatchInfo,
) -> PolarsResult<DataFrame> {
    let mut dealer_player_slot = Vec::new();
    let mut dealer_hero_id: Vec<Option<u32>> = Vec::new();
    let mut target_player_slot = Vec::new();
    let mut target_hero_id: Vec<Option<u32>> = Vec::new();
    let mut source_name = Vec::new();
    let mut is_category = Vec::new();
    let mut stat_type = Vec::new();
    let mut sample_time_s = Vec::new();
    let mut damage = Vec::new();

    // player_slot -> hero_id, so dealer/target slots resolve to heroes (slot 0
    // and other non-player slots have no hero and stay null).
    let slot_to_hero: HashMap<u32, u32> = match_info
        .players
        .iter()
        .filter_map(|p| p.player_slot.map(|slot| (slot, p.hero_id())))
        .collect();

    if let Some(matrix) = match_info.damage_matrix.as_ref() {
        let times = &matrix.sample_time_s;
        let n = times.len();
        let (names, stats): (&[String], &[i32]) = match matrix.source_details.as_ref() {
            Some(sd) => (sd.source_name.as_slice(), sd.stat_type.as_slice()),
            None => (&[], &[]),
        };

        for dealer in &matrix.damage_dealers {
            let dslot = dealer.dealer_player_slot();
            let dhero = slot_to_hero.get(&dslot).copied();
            for source in &dealer.damage_sources {
                let idx = source.source_details_index() as usize;
                let name = names.get(idx).cloned().unwrap_or_default();
                let category = is_category_source(&name);
                let stat = stat_type_label(stats.get(idx).copied().unwrap_or(0));
                for dtp in &source.damage_to_players {
                    let tslot = dtp.target_player_slot();
                    let thero = slot_to_hero.get(&tslot).copied();
                    let arr = &dtp.damage;
                    // Cumulative arrays cover the last `arr.len()` samples. Emit
                    // per-interval deltas so the `damage` column is additive
                    // (sum for totals; cumsum over `sample_time_s` for the
                    // running total).
                    let start = n.saturating_sub(arr.len());
                    let mut prev = 0u32;
                    for (k, &cumulative) in arr.iter().enumerate() {
                        let time = times.get(start + k).copied().unwrap_or(0);
                        let delta = cumulative.saturating_sub(prev);
                        prev = cumulative;
                        dealer_player_slot.push(dslot);
                        dealer_hero_id.push(dhero);
                        target_player_slot.push(tslot);
                        target_hero_id.push(thero);
                        source_name.push(name.clone());
                        is_category.push(category);
                        stat_type.push(stat.clone());
                        sample_time_s.push(time);
                        damage.push(delta);
                    }
                }
            }
        }
    }

    df_from_columns(vec![
        Column::new("dealer_player_slot".into(), dealer_player_slot),
        Column::new("dealer_hero_id".into(), dealer_hero_id),
        Column::new("target_player_slot".into(), target_player_slot),
        Column::new("target_hero_id".into(), target_hero_id),
        Column::new("source_name".into(), source_name),
        Column::new("is_category".into(), is_category),
        Column::new("stat_type".into(), stat_type),
        Column::new("sample_time_s".into(), sample_time_s),
        Column::new("damage".into(), damage),
    ])
}

#[pymethods]
impl Demo {
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let path = PathBuf::from(path);

        // Check if file exists first for a clear FileNotFoundError
        if !path.exists() {
            return Err(PyFileNotFoundError::new_err(format!(
                "Demo file not found: {}",
                path.display()
            )));
        }

        let parser = boon_parser::Parser::from_file(&path).map_err(to_py_err)?;

        // Verify the file is a valid demo
        parser.verify().map_err(to_py_err)?;

        // Parse header info
        let header = parser.file_header().map_err(to_py_err)?;
        let build = header
            .build_num
            .ok_or_else(|| DemoHeaderError::new_err("missing build number in file header"))?;
        let map_name = header
            .map_name
            .ok_or_else(|| DemoHeaderError::new_err("missing map name in file header"))?;

        // Parse file info
        let info = parser.file_info().map_err(to_py_err)?;
        let total_ticks = info
            .playback_ticks
            .ok_or_else(|| DemoInfoError::new_err("missing playback ticks in file info"))?;
        let playback_time = info
            .playback_time
            .ok_or_else(|| DemoInfoError::new_err("missing playback time in file info"))?;

        // Parse the first tick and best-effort resolve match_id / game_mode from
        // CCitadelGameRulesProxy. The match ID is optional: some demos (partial
        // captures, sandbox / custom content) don't carry one, so we record it
        // when present and leave it `None` otherwise rather than refusing to
        // open the demo. game_mode defaults to 0 when unavailable.
        let ctx = parser.parse_to_tick(1).map_err(to_py_err)?;

        let game_rules = ctx
            .entities()
            .iter()
            .find(|(_, e)| e.class_name.as_ref() == "CCitadelGameRulesProxy");

        let match_id = game_rules.and_then(|(_, e)| {
            let serializer = ctx.serializers().get(&e.class_name)?;
            let mid_key = serializer.resolve_field_key("m_pGameRules.m_unMatchID")?;
            match e.fields.get(&mid_key)? {
                boon_parser::FieldValue::U64(id) => Some(*id),
                boon_parser::FieldValue::I64(id) => Some(*id as u64),
                _ => None,
            }
        });

        let game_mode = game_rules
            .and_then(|(_, e)| {
                let serializer = ctx.serializers().get(&e.class_name)?;
                Some(e.get_i64(serializer.resolve_field_key("m_pGameRules.m_eGameMode")))
            })
            .unwrap_or(0);

        let tick_rate = if playback_time > 0.0 {
            (total_ticks as f32 / playback_time).round() as i32
        } else {
            0
        };

        Ok(Demo {
            parser,
            path,
            build,
            map_name,
            total_ticks,
            playback_time,
            tick_rate,
            match_id,
            game_mode,
            paused_ticks: None,
            cached_player_ticks: None,
            cached_world_ticks: None,
            cached_kills: None,
            cached_damage: None,
            game_over: None,
            banned_hero_ids: None,
            always_events_scanned: false,
            cached_abilities: None,
            cached_flex_slots: None,
            cached_ability_upgrades: None,
            cached_item_purchases: None,
            cached_chat: None,
            cached_objectives: None,
            cached_mid_boss: None,
            cached_troopers: None,
            cached_neutrals: None,
            cached_stat_modifier_events: None,
            cached_active_modifiers: None,
            cached_ability_ticks: None,
            cached_players: None,
            cached_street_brawl_ticks: None,
            cached_street_brawl_rounds: None,
            cached_urn: None,
            cached_rift: None,
        })
    }

    /// Verify that the file is a valid demo file.
    ///
    /// Returns:
    ///     True if the file is valid.
    ///
    /// Note:
    ///     This is already called during construction, so it will always
    ///     return True for an existing Demo instance.
    fn verify(&self) -> PyResult<bool> {
        self.parser.verify().map_err(to_py_err)?;
        Ok(true)
    }

    /// The path to the demo file.
    #[getter]
    fn path(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let pathlib = py.import("pathlib")?;
        let path = pathlib
            .getattr("Path")?
            .call1((self.path.to_string_lossy().to_string(),))?;
        Ok(path.unbind())
    }

    /// The total number of ticks in the demo.
    #[getter]
    fn total_ticks(&self) -> i32 {
        self.total_ticks
    }

    /// The total duration of the demo in seconds.
    #[getter]
    fn total_seconds(&self) -> f32 {
        self.playback_time
    }

    /// The total duration of the demo as a formatted string (e.g., "12:34").
    #[getter]
    fn total_clock_time(&self) -> String {
        let total_seconds = self.playback_time as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{minutes}:{seconds:02}")
    }

    /// The build number of the game that recorded the demo.
    #[getter]
    fn build(&self) -> i32 {
        self.build
    }

    /// The name of the map the demo was recorded on.
    #[getter]
    fn map_name(&self) -> String {
        self.map_name.clone()
    }

    /// The match ID for this demo, or ``None`` if the demo does not carry one
    /// (e.g. a partial capture or sandbox / custom content).
    #[getter]
    fn match_id(&self) -> Option<u64> {
        self.match_id
    }

    /// The game mode ID for this demo.
    ///
    /// Use ``game_mode_names()`` to resolve IDs to names.
    #[getter]
    fn game_mode(&self) -> i64 {
        self.game_mode
    }

    /// The tick rate of the demo (ticks per second).
    #[getter]
    fn tick_rate(&self) -> i32 {
        self.tick_rate
    }

    /// Parse the post-match summary from the demo's ``PostMatchDetails`` event.
    ///
    /// Returns a dictionary with four top-level keys:
    ///
    /// - ``snapshots``: a Polars DataFrame with one row per (snapshot, player).
    ///   Snapshots are taken at intervals through the match (not every minute);
    ///   ``snapshot_time_s`` marks each one. Columns hold that player's running
    ///   totals at that time: ``hero_id``, ``kills``, ``deaths``, ``assists``,
    ///   ``net_worth``, ``denies``, ``level``, ``lane``, ``creep_kills``,
    ///   ``neutral_kills``, ``player_damage``, and the per-source gold/orbs
    ///   breakdown.
    /// - ``last_hits``: a Polars DataFrame of ``hero_id`` and ``last_hits`` (the
    ///   final scoreboard last-hit / souls-secured total, which is only recorded
    ///   per match, not per snapshot).
    /// - ``objectives``: a Polars DataFrame of post-match objective records
    ///   (lane/team objectives, destruction time, and damage taken).
    /// - ``damage``: a Polars DataFrame of the damage matrix — one row per
    ///   (dealer, target, source, sample). Dealer/target are given as both
    ///   ``*_player_slot`` and resolved ``*_hero_id`` (null for non-player slots
    ///   like 0), so it joins to the other frames on ``hero_id``. ``damage`` is
    ///   the per-interval (additive) amount for that ``stat_type`` (a string:
    ///   ``damage``, ``healing``, ``mitigated``, …) dealt during the interval
    ///   ending at ``sample_time_s``. Each hit is recorded under both a coarse
    ///   category (``is_category`` true) and a specific source, so filter to
    ///   ``is_category == False`` to avoid double-counting, then ``sum``.
    ///
    /// Raises ``DemoMessageError`` if the demo contains no post-match details
    /// (for example, an incomplete recording).
    fn summary(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        use boon_proto::proto::{CCitadelUserMsgPostMatchDetails, CMsgMatchMetaDataContents};

        let events = self.parser.events(None).map_err(to_py_err)?;
        let event = events
            .iter()
            .find(|e| e.msg_type == Msg::KEUserMsgPostMatchDetails as u32)
            .ok_or_else(|| DemoMessageError::new_err("no PostMatchDetails event found in demo"))?;

        let outer =
            CCitadelUserMsgPostMatchDetails::decode(event.payload.as_slice()).map_err(|e| {
                DemoMessageError::new_err(format!("failed to decode PostMatchDetails: {e}"))
            })?;
        let details_bytes = outer.match_details.as_ref().ok_or_else(|| {
            DemoMessageError::new_err("PostMatchDetails has no match_details bytes")
        })?;
        let contents =
            CMsgMatchMetaDataContents::decode(details_bytes.as_slice()).map_err(|e| {
                DemoMessageError::new_err(format!("failed to decode match metadata: {e}"))
            })?;
        let match_info = contents
            .match_info
            .ok_or_else(|| DemoMessageError::new_err("match metadata has no match_info"))?;

        let to_df_err =
            |e: PolarsError| DemoMessageError::new_err(format!("failed to build summary: {e}"));
        let snapshots = build_snapshots_frame(&match_info).map_err(to_df_err)?;
        let last_hits = build_last_hits_frame(&match_info).map_err(to_df_err)?;
        let objectives = build_objectives_frame(&match_info).map_err(to_df_err)?;
        let damage = build_damage_frame(&match_info).map_err(to_df_err)?;

        let dict = PyDict::new(py);
        dict.set_item("snapshots", PyDataFrame(snapshots))?;
        dict.set_item("last_hits", PyDataFrame(last_hits))?;
        dict.set_item("objectives", PyDataFrame(objectives))?;
        dict.set_item("damage", PyDataFrame(damage))?;
        Ok(dict.into_any().unbind())
    }

    /// Snapshot per-tick state at selected ticks in a single parallel pass.
    ///
    /// Decodes the demo once (across keyframe segments, in parallel) and collects
    /// rows only at the ticks you select, so sampling is far cheaper than pulling
    /// a full per-tick frame and filtering in Python.
    ///
    /// Args:
    ///     datasets: Which snapshot dataset(s) to return — ``"player_ticks"``
    ///         (default), ``"world_ticks"``, ``"troopers"``, or a list of them.
    ///     ticks: A specific tick or list of ticks.
    ///     every: Sample every ``N`` ticks (gap-robust stride).
    ///     seconds: Sample about once per ``seconds`` (converted via the tick rate).
    ///         Mutually exclusive with ``every``.
    ///     events: Sample at the ticks of these event datasets (e.g. ``"kills"``
    ///         or ``["kills", "damage"]``).
    ///     start_tick, end_tick: Restrict to a contiguous ``[start, end]`` window.
    ///
    /// With no stride/ticks/events but a window, every tick in the window is
    /// returned; specifying none of them is an error. Returns a single DataFrame
    /// when one dataset is requested, otherwise a dict keyed by dataset name.
    ///
    /// Example:
    ///     >>> demo.snapshots(every=64)                     # ~1 row/sec of ticks
    ///     >>> demo.snapshots(ticks=[29000, 30000])         # specific ticks
    ///     >>> demo.snapshots("troopers", events="kills")   # troopers at kill ticks
    ///     >>> demo.snapshots(["player_ticks", "world_ticks"], seconds=1.0)
    #[pyo3(signature = (datasets=None, *, ticks=None, every=None, seconds=None, events=None, start_tick=None, end_tick=None))]
    #[allow(clippy::too_many_arguments)]
    fn snapshots(
        &mut self,
        py: Python<'_>,
        datasets: Option<StrOrList>,
        ticks: Option<IntOrList>,
        every: Option<i32>,
        seconds: Option<f32>,
        events: Option<StrOrList>,
        start_tick: Option<i32>,
        end_tick: Option<i32>,
    ) -> PyResult<Py<PyAny>> {
        // Requested datasets -> SnapWants (default player_ticks).
        let names = datasets
            .map(StrOrList::into_vec)
            .unwrap_or_else(|| vec!["player_ticks".to_string()]);
        if names.is_empty() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "snapshots(): `datasets` must not be empty",
            ));
        }
        let mut wants = SnapWants::default();
        for n in &names {
            match n.as_str() {
                "player_ticks" => wants.player_ticks = true,
                "world_ticks" => wants.world_ticks = true,
                "troopers" => wants.troopers = true,
                other => {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "snapshots(): datasets must be player_ticks / world_ticks / troopers, got '{other}'"
                    )));
                }
            }
        }

        // Stride: `every` (ticks) or `seconds` (converted), mutually exclusive.
        if every.is_some() && seconds.is_some() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "snapshots(): pass either `every` or `seconds`, not both",
            ));
        }
        let stride: Option<i32> = match (every, seconds) {
            (Some(step), _) if step < 1 => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "snapshots(): `every` must be >= 1 tick",
                ));
            }
            (Some(step), _) => Some(step),
            (_, Some(secs)) if !secs.is_finite() || secs <= 0.0 => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "snapshots(): `seconds` must be a positive number",
                ));
            }
            (_, Some(secs)) => Some(((secs * self.tick_rate as f32).round() as i32).max(1)),
            (None, None) => None,
        };

        // Explicit + event ticks.
        let explicit = ticks.map(IntOrList::into_vec).unwrap_or_default();
        let mut tick_set: std::collections::HashSet<i32> = std::collections::HashSet::new();
        if let Some(events) = events {
            tick_set = self.event_ticks(&events.into_vec())?;
        }
        tick_set.extend(&explicit);

        let has_window = start_tick.is_some() || end_tick.is_some();
        if stride.is_none() && tick_set.is_empty() && !has_window {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "snapshots(): specify at least one of `ticks`, `every`, `seconds`, \
                 `events`, or `start_tick` / `end_tick`",
            ));
        }

        // Fast path: a single tick with no stride or window seeks directly
        // (`parse_to_tick`) instead of decoding the whole demo.
        let (pt, wt, tr) = if stride.is_none() && !has_window && tick_set.len() == 1 {
            let t = *tick_set.iter().next().expect("len == 1");
            self.snapshot_at_tick(t, wants)?
        } else {
            // The tick predicate: the union of the stride-sampled ticks and the
            // explicit/event ticks, restricted to the window. With no sampler,
            // every tick in the window.
            let start = start_tick.unwrap_or(i32::MIN);
            let end = end_tick.unwrap_or(i32::MAX);
            let pred = if stride.is_none() && tick_set.is_empty() {
                TickPredicate::Window { start, end }
            } else {
                let mut sampled = tick_set;
                if let Some(step) = stride {
                    let mut last: Option<i32> = None;
                    for t in self.parser.distinct_ticks().map_err(to_py_err)? {
                        if last.is_none_or(|l| t - l >= step) {
                            sampled.insert(t);
                            last = Some(t);
                        }
                    }
                }
                TickPredicate::Set {
                    ticks: sampled,
                    start,
                    end,
                }
            };
            self.build_snapshots_parallel(wants, &pred)?
        };
        let frame_for = |name: &str| -> Option<DataFrame> {
            match name {
                "player_ticks" => pt.clone(),
                "world_ticks" => wt.clone(),
                "troopers" => tr.clone(),
                _ => None,
            }
        };

        if names.len() == 1 {
            let df = frame_for(&names[0]).unwrap();
            PyDataFrame(df).into_py_any(py)
        } else {
            let dict = PyDict::new(py);
            for n in &names {
                dict.set_item(n, PyDataFrame(frame_for(n).unwrap()))?;
            }
            Ok(dict.into_any().unbind())
        }
    }

    /// Convert a tick number to seconds elapsed, excluding paused time.
    ///
    /// Automatically loads ``world_ticks`` on first call to determine pauses.
    fn tick_to_seconds(&mut self, tick: i32) -> PyResult<f64> {
        if self.tick_rate == 0 {
            return Ok(0.0);
        }
        self.ensure_paused_ticks_built()?;
        let active_ticks = self.count_active_ticks(tick);
        Ok(active_ticks as f64 / self.tick_rate as f64)
    }

    /// Convert a tick number to a clock time string (e.g., ``"03:14"``),
    /// excluding paused time.
    ///
    /// Automatically loads ``world_ticks`` on first call to determine pauses.
    fn tick_to_clock_time(&mut self, tick: i32) -> PyResult<String> {
        let secs = self.tick_to_seconds(tick)?;
        let total_seconds = secs as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        Ok(format!("{minutes}:{seconds:02}"))
    }

    /// Get player information as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - player_name: The player's display name
    /// - steam_id: The player's Steam ID
    /// - hero_id: The player's hero ID
    /// - team_num: The player's raw team number
    /// - start_lane: The player's original lane color
    ///   (1=yellow, 3=green, 4=blue, 6=purple, 0=none; from the `CMsgLaneColor` proto enum)
    #[getter]
    fn players(&mut self) -> PyResult<PyDataFrame> {
        if let Some(ref df) = self.cached_players {
            return Ok(PyDataFrame(df.clone()));
        }

        // The roster (name / steam id / hero / team / lane) is set once the
        // match is underway and never changes, so it can be snapshotted from a
        // single tick. Prefer the game-over tick: it is late enough that every
        // field is populated (heroes locked, lanes assigned) but before the
        // post-game teardown that despawns the player controllers. Snapshotting
        // there — rather than the final recorded tick — avoids both reading
        // pre-game placeholders and finding the controllers (partially or
        // fully) gone at the end. Fall back to the final tick only when a demo
        // has no game-over event (e.g. an incomplete recording).
        self.ensure_always_events_scanned()?;
        let snapshot_tick = self.game_over.map_or(self.total_ticks, |(_, tick)| tick);
        let mut df = self.collect_players_at(snapshot_tick)?;

        // Defensive: if that tick somehow had no controllers, try the other.
        if df.height() == 0 && snapshot_tick != self.total_ticks {
            df = self.collect_players_at(self.total_ticks)?;
        }

        self.cached_players = Some(df.clone());
        Ok(PyDataFrame(df))
    }

    /// Heroes banned from this match as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - hero_id: The banned hero's ID (joins to ``players.hero_id``)
    /// - hero_name: The resolved hero name, or ``"HERO_NOT_FOUND"`` for an ID
    ///   that predates the bundled hero table
    ///
    /// Read from the one-shot ``BannedHeroes`` user message, which the server
    /// sends early in the demo (before the match starts) only when the match
    /// has bans. The message carries nothing but the hero IDs — no team, no
    /// banning player, and no pick/ban ordering — so this cannot be used to
    /// reconstruct a draft.
    ///
    /// An empty DataFrame means no bans were recorded for this match. Demos
    /// from builds that never emit the message are indistinguishable from
    /// ban-free matches, so treat empty as "nothing recorded" rather than as
    /// positive proof that nothing was banned.
    #[getter]
    fn banned_heroes(&mut self) -> PyResult<PyDataFrame> {
        self.ensure_always_events_scanned()?;
        let ids: Vec<i64> = self
            .banned_hero_ids
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|&id| id as i64)
            .collect();
        let names: Vec<&'static str> = ids.iter().map(|&id| boon_parser::hero_name(id)).collect();
        let df = df_from_columns(vec![
            Column::new("hero_id".into(), ids),
            Column::new("hero_name".into(), names),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
        Ok(PyDataFrame(df))
    }

    /// Return the list of dataset names that can be passed to ``load()`` or accessed as properties.
    ///
    /// Returns:
    ///     A list of valid dataset name strings.
    #[staticmethod]
    fn available_datasets() -> Vec<&'static str> {
        let mut all = VALID_DATASETS.to_vec();
        all.extend_from_slice(VALID_STREET_BRAWL_DATASETS);
        all
    }

    /// Load one or more datasets from the demo file in a single pass.
    ///
    /// Valid dataset names: see ``available_datasets()``.
    /// Already-loaded datasets are skipped. Multiple datasets requested together
    /// share a single parse pass over the file for efficiency.
    ///
    /// Args:
    ///     *datasets: One or more dataset names to load.
    ///
    /// Raises:
    ///     ValueError: If an unknown dataset name is provided.
    ///     NotStreetBrawlError: If a street brawl dataset is requested on a non-street-brawl demo.
    #[pyo3(signature = (*datasets))]
    fn load(&mut self, datasets: Vec<String>) -> PyResult<()> {
        // Validate dataset names
        for name in &datasets {
            if !VALID_DATASETS.contains(&name.as_str())
                && !VALID_STREET_BRAWL_DATASETS.contains(&name.as_str())
            {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Unknown dataset: {name:?}. Valid datasets: {VALID_DATASETS:?}, street brawl: {VALID_STREET_BRAWL_DATASETS:?}"
                )));
            }
        }

        // Check game mode for street brawl datasets
        if datasets
            .iter()
            .any(|s| VALID_STREET_BRAWL_DATASETS.contains(&s.as_str()))
            && self.game_mode != 4
        {
            return Err(NotStreetBrawlError::new_err(
                "Street brawl datasets are only available for street brawl demos (game_mode=4)",
            ));
        }

        // Determine what to load (skip already cached)
        let load_abilities =
            datasets.iter().any(|s| s == "abilities") && self.cached_abilities.is_none();
        let load_player_ticks =
            datasets.iter().any(|s| s == "player_ticks") && self.cached_player_ticks.is_none();
        let load_world_ticks =
            datasets.iter().any(|s| s == "world_ticks") && self.cached_world_ticks.is_none();
        let load_kills = datasets.iter().any(|s| s == "kills") && self.cached_kills.is_none();
        let load_damage = datasets.iter().any(|s| s == "damage") && self.cached_damage.is_none();
        let load_flex_slots =
            datasets.iter().any(|s| s == "flex_slots") && self.cached_flex_slots.is_none();
        let load_ability_upgrades = datasets.iter().any(|s| s == "ability_upgrades")
            && self.cached_ability_upgrades.is_none();
        let load_item_purchases =
            datasets.iter().any(|s| s == "item_purchases") && self.cached_item_purchases.is_none();
        let load_chat = datasets.iter().any(|s| s == "chat") && self.cached_chat.is_none();
        let load_objectives =
            datasets.iter().any(|s| s == "objectives") && self.cached_objectives.is_none();
        let load_mid_boss =
            datasets.iter().any(|s| s == "mid_boss") && self.cached_mid_boss.is_none();
        let load_troopers =
            datasets.iter().any(|s| s == "troopers") && self.cached_troopers.is_none();
        let load_neutrals =
            datasets.iter().any(|s| s == "neutrals") && self.cached_neutrals.is_none();
        let load_stat_modifier_events = datasets.iter().any(|s| s == "stat_modifier_events")
            && self.cached_stat_modifier_events.is_none();
        let load_active_modifiers = datasets.iter().any(|s| s == "active_modifiers")
            && self.cached_active_modifiers.is_none();
        let load_ability_ticks =
            datasets.iter().any(|s| s == "ability_ticks") && self.cached_ability_ticks.is_none();
        let load_urn = datasets.iter().any(|s| s == "urn") && self.cached_urn.is_none();
        let load_street_brawl_ticks = datasets.iter().any(|s| s == "street_brawl_ticks")
            && self.cached_street_brawl_ticks.is_none();
        let load_street_brawl_rounds = datasets.iter().any(|s| s == "street_brawl_rounds")
            && self.cached_street_brawl_rounds.is_none();
        let load_rift = datasets.iter().any(|s| s == "rift") && self.cached_rift.is_none();

        if !load_abilities
            && !load_player_ticks
            && !load_world_ticks
            && !load_kills
            && !load_damage
            && !load_flex_slots
            && !load_ability_upgrades
            && !load_item_purchases
            && !load_chat
            && !load_objectives
            && !load_mid_boss
            && !load_troopers
            && !load_neutrals
            && !load_stat_modifier_events
            && !load_active_modifiers
            && !load_ability_ticks
            && !load_urn
            && !load_street_brawl_ticks
            && !load_street_brawl_rounds
            && !load_rift
        {
            return Ok(());
        }

        // One-pass fast path: if everything still to load is a parallel-safe
        // snapshot dataset (player_ticks / world_ticks / troopers), decode them
        // together in a single parallel keyframe-segmented pass and skip the
        // serial pass. (Each is re-keyframed at every full packet, so segmented
        // decoding is byte-for-byte identical — verified in tests.)
        let only_snapshots = !load_abilities
            && !load_kills
            && !load_damage
            && !load_flex_slots
            && !load_ability_upgrades
            && !load_item_purchases
            && !load_chat
            && !load_objectives
            && !load_mid_boss
            && !load_neutrals
            && !load_stat_modifier_events
            && !load_active_modifiers
            && !load_ability_ticks
            && !load_urn
            && !load_street_brawl_ticks
            && !load_street_brawl_rounds
            && !load_rift;
        if only_snapshots {
            return self.ensure_snapshots(SnapWants {
                player_ticks: load_player_ticks,
                world_ticks: load_world_ticks,
                troopers: load_troopers,
            });
        }

        let need_events = load_abilities
            || load_kills
            || load_damage
            || load_flex_slots
            || load_item_purchases
            || load_chat
            || load_mid_boss
            || load_street_brawl_rounds;

        // ability_ticks needs every ability *entity* class decoded. There are
        // hundreds (one per ability), so collect their names from the send tables
        // (any networked class whose name contains "Ability") into an owned Vec
        // that outlives the borrowed `&str` class filter below.
        let ability_class_names: Vec<String> = if load_ability_ticks {
            self.parser
                .parse_send_tables()
                .map(|sc| {
                    sc.iter()
                        .map(|(name, _)| name)
                        .filter(|name| name.contains("Ability"))
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Build union class filter
        let mut class_names: Vec<&str> = Vec::new();
        if load_player_ticks {
            class_names.push("CCitadelPlayerPawn");
            class_names.push("CCitadelPlayerController");
        }
        if load_world_ticks || load_street_brawl_ticks || load_rift {
            class_names.push("CCitadelGameRulesProxy");
        }
        if load_abilities
            || load_kills
            || load_damage
            || load_mid_boss
            || load_active_modifiers
            || load_urn
        {
            class_names.push("CCitadelPlayerPawn");
        }
        if load_ability_upgrades || load_item_purchases || load_chat || load_stat_modifier_events {
            class_names.push("CCitadelPlayerController");
        }
        if load_objectives {
            class_names.push("CNPC_Boss_Tier2");
            class_names.push("CNPC_Boss_Tier3");
            class_names.push("CNPC_BarrackBoss");
            class_names.push("CNPC_MidBoss");
            class_names.push("CCitadel_Destroyable_Building");
        }
        if load_troopers {
            class_names.push("CNPC_Trooper");
            class_names.push("CNPC_TrooperBoss");
        }
        if load_neutrals {
            class_names.push("CNPC_TrooperNeutral");
        }
        if load_urn {
            class_names.push("CCitadelIdolReturnTrigger");
        }
        if load_rift {
            // The spawner announces a Rift before it becomes contestable; the
            // rest of the lifecycle comes off the game rules entity.
            class_names.push("CCitadelItemKothSpawner");
        }
        if load_ability_ticks {
            // Pawns for the owner -> hero mapping, plus every ability class.
            class_names.push("CCitadelPlayerPawn");
            for n in &ability_class_names {
                class_names.push(n.as_str());
            }
        }
        let class_filter: std::collections::HashSet<&str> = class_names.into_iter().collect();

        // ── Column vectors for player_ticks ──
        let pt_capacity = if load_player_ticks {
            self.total_ticks as usize * 12
        } else {
            0
        };
        let mut pt_tick: Vec<i32> = Vec::with_capacity(pt_capacity);
        let mut pt_hero_id: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_x: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_y: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_z: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_pitch: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_yaw: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_roll: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_in_regen_zone: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_in_item_shop: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_death_time: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_last_spawn_time: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_respawn_time: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_health: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_max_health: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_lifestate: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_souls: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_spent_souls: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_combat_end: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_combat_last_dmg: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_combat_start: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_dealt_end: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_dealt_last: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_dealt_start: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_taken_end: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_taken_last: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_dmg_taken_start: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_time_revealed: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_build_id: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_is_alive: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_has_rebirth: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_has_rejuvenator: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_has_ultimate: Vec<bool> = Vec::with_capacity(pt_capacity);
        let mut pt_health_regen: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_ult_cd_start: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_ult_cd_end: Vec<f32> = Vec::with_capacity(pt_capacity);
        let mut pt_ap_nw: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_gold_nw: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_denies: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_hero_damage: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_hero_healing: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_obj_damage: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_self_healing: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_kill_streak: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_last_hits: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_level: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_kills: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_deaths: Vec<i64> = Vec::with_capacity(pt_capacity);
        let mut pt_assists: Vec<i64> = Vec::with_capacity(pt_capacity);

        // ── Column vectors for world_ticks ──
        let wt_capacity = if load_world_ticks {
            self.total_ticks as usize
        } else {
            0
        };
        let mut wt_tick: Vec<i32> = Vec::with_capacity(wt_capacity);
        let mut wt_is_paused: Vec<bool> = Vec::with_capacity(wt_capacity);
        let mut wt_next_midboss: Vec<f32> = Vec::with_capacity(wt_capacity);

        // ── Kill / damage event collection ──
        struct RawEvent {
            tick: i32,
            payload: Vec<u8>,
        }
        let mut raw_kill_events: Vec<RawEvent> = Vec::new();
        let mut raw_damage_events: Vec<RawEvent> = Vec::new();
        let mut entity_to_hero: HashMap<i32, i64> = HashMap::new();
        let mut entity_to_hero_built = false;
        let mut found_game_over: Option<(i32, i32)> = None;
        let mut found_banned_heroes: Vec<u32> = Vec::new();
        let mut flex_ticks: Vec<i32> = Vec::new();
        let mut flex_team_nums: Vec<i32> = Vec::new();
        let mut ability_ticks: Vec<i32> = Vec::new();
        let mut ability_hero_ids: Vec<i64> = Vec::new();
        let mut ability_names: Vec<String> = Vec::new();
        let mut slot_to_hero: HashMap<i32, i64> = HashMap::new();
        let mut slot_to_hero_built = false;

        // ── Column vectors for ability_upgrades ──
        let mut au_ticks: Vec<i32> = Vec::new();
        let mut au_hero_ids: Vec<i64> = Vec::new();
        let mut au_ability_ids: Vec<u32> = Vec::new();
        let mut au_tier: Vec<i32> = Vec::new();
        // Change detection: (controller_entity_index, slot_index) → previous upgrade_bits
        let mut au_prev_bits: HashMap<(i32, usize), i32> = HashMap::new();

        // ── Column vectors for chat ──
        let mut chat_ticks: Vec<i32> = Vec::new();
        let mut chat_hero_ids: Vec<i64> = Vec::new();
        let mut chat_texts: Vec<String> = Vec::new();
        let mut chat_types: Vec<String> = Vec::new();

        // ── Column vectors for objectives (change detection) ──
        let mut obj_tick: Vec<i32> = Vec::new();
        let mut obj_type: Vec<String> = Vec::new();
        let mut obj_team_num: Vec<i64> = Vec::new();
        let mut obj_lane: Vec<i64> = Vec::new();
        let mut obj_health: Vec<i64> = Vec::new();
        let mut obj_max_health: Vec<i64> = Vec::new();
        let mut obj_phase: Vec<i64> = Vec::new();
        let mut obj_x: Vec<f32> = Vec::new();
        let mut obj_y: Vec<f32> = Vec::new();
        let mut obj_z: Vec<f32> = Vec::new();
        let mut obj_entity_id: Vec<i32> = Vec::new();
        // Change detection: entity_index → (health, max_health, phase)
        let mut obj_prev: HashMap<i32, (i64, i64, i64)> = HashMap::new();
        // Patron phase key
        let mut patron_phase_key: Option<u64> = None;
        // ── Column vectors for mid_boss ──
        let mut mb_ticks: Vec<i32> = Vec::new();
        let mut mb_team_nums: Vec<i32> = Vec::new();
        let mut mb_events: Vec<String> = Vec::new();

        // ── Column vectors for item_purchases ──
        let mut ip_ticks: Vec<i32> = Vec::new();
        let mut ip_hero_ids: Vec<i64> = Vec::new();
        let mut ip_ability_ids: Vec<u32> = Vec::new();
        let mut ip_changes: Vec<String> = Vec::new();

        // ── Column vectors for troopers (lane only) ──
        let mut tr_tick: Vec<i32> = Vec::new();
        let mut tr_type: Vec<String> = Vec::new();
        let mut tr_team_num: Vec<i64> = Vec::new();
        let mut tr_lane: Vec<i64> = Vec::new();
        let mut tr_health: Vec<i64> = Vec::new();
        let mut tr_max_health: Vec<i64> = Vec::new();
        let mut tr_x: Vec<f32> = Vec::new();
        let mut tr_y: Vec<f32> = Vec::new();
        let mut tr_z: Vec<f32> = Vec::new();
        let mut tr_entity_id: Vec<i32> = Vec::new();

        // ── Column vectors for neutrals (change-detected) ──
        let mut nt_tick: Vec<i32> = Vec::new();
        let mut nt_team_num: Vec<i64> = Vec::new();
        let mut nt_health: Vec<i64> = Vec::new();
        let mut nt_max_health: Vec<i64> = Vec::new();
        let mut nt_x: Vec<f32> = Vec::new();
        let mut nt_y: Vec<f32> = Vec::new();
        let mut nt_z: Vec<f32> = Vec::new();
        let mut nt_entity_id: Vec<i32> = Vec::new();
        // Change detection: entity_index → (was_alive, health, max_health, x_bits, y_bits, z_bits)
        let mut nt_prev: HashMap<i32, (bool, i64, i64, u32, u32, u32)> = HashMap::new();

        // ── Column vectors for stat_modifiers (event-based change detection) ──
        let mut sm_tick: Vec<i32> = Vec::new();
        let mut sm_hero_id: Vec<i64> = Vec::new();
        let mut sm_stat_type: Vec<String> = Vec::new();
        let mut sm_amount: Vec<f32> = Vec::new();
        // Change detection: (controller_entity_index, eValType) → previous summed value
        let mut sm_prev: HashMap<(i32, u32), f32> = HashMap::new();

        // ── Column vectors for active_modifiers ──
        let mut am_tick: Vec<i32> = Vec::new();
        let mut am_hero_id: Vec<i64> = Vec::new();
        let mut am_event: Vec<String> = Vec::new();
        let mut am_modifier_id: Vec<u32> = Vec::new();
        let mut am_ability_id: Vec<u32> = Vec::new();
        let mut am_duration: Vec<f32> = Vec::new();
        let mut am_caster_hero_id: Vec<i64> = Vec::new();
        let mut am_stacks: Vec<i32> = Vec::new();

        // ── Column vectors for ability_ticks (entity change detection) ──
        let mut at_tick: Vec<i32> = Vec::new();
        let mut at_hero_id: Vec<i64> = Vec::new();
        let mut at_ability_id: Vec<u32> = Vec::new();
        let mut at_slot: Vec<i32> = Vec::new();
        let mut at_cooldown_start: Vec<f32> = Vec::new();
        let mut at_cooldown_end: Vec<f32> = Vec::new();
        let mut at_remaining_charges: Vec<i32> = Vec::new();
        let mut at_charge_recharge_start: Vec<f32> = Vec::new();
        let mut at_charge_recharge_end: Vec<f32> = Vec::new();
        // ── Column vectors for street_brawl_ticks ──
        let sbt_capacity = if load_street_brawl_ticks {
            self.total_ticks as usize
        } else {
            0
        };
        let mut sbt_tick: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_round: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_state: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_amber_score: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_sapphire_score: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_buy_countdown: Vec<i32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_next_state_time: Vec<f32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_state_start_time: Vec<f32> = Vec::with_capacity(sbt_capacity);
        let mut sbt_non_combat_time: Vec<f32> = Vec::with_capacity(sbt_capacity);

        // ── Column vectors for street_brawl_rounds ──
        let mut sbr_round: Vec<i32> = Vec::new();
        let mut sbr_tick: Vec<i32> = Vec::new();
        let mut sbr_scoring_team: Vec<i32> = Vec::new();
        let mut sbr_amber_score: Vec<i32> = Vec::new();
        let mut sbr_sapphire_score: Vec<i32> = Vec::new();
        let mut sbr_round_counter: i32 = 0;

        // ── Column vectors for urn ──
        let mut urn_tick: Vec<i32> = Vec::new();
        let mut urn_event: Vec<String> = Vec::new();
        let mut urn_hero_id: Vec<i64> = Vec::new();
        let mut urn_team_num: Vec<i64> = Vec::new();
        let mut urn_x: Vec<f32> = Vec::new();
        let mut urn_y: Vec<f32> = Vec::new();
        let mut urn_z: Vec<f32> = Vec::new();

        // ── Column vectors for rift ──
        let mut rift_num: Vec<i32> = Vec::new();
        let mut rift_announce_tick: Vec<Option<i32>> = Vec::new();
        let mut rift_active_tick: Vec<i32> = Vec::new();
        let mut rift_capture_tick: Vec<Option<i32>> = Vec::new();
        let mut rift_expire_tick: Vec<Option<i32>> = Vec::new();
        let mut rift_winning_team: Vec<Option<i32>> = Vec::new();
        let mut rift_lane: Vec<i64> = Vec::new();
        let mut rift_x: Vec<f32> = Vec::new();
        let mut rift_y: Vec<f32> = Vec::new();
        let mut rift_z: Vec<f32> = Vec::new();

        // Rift lifecycle state. One row is emitted per completed Rift, when the
        // game rules entity clears the cash-in.
        let mut rift_counter: i32 = 0;
        let mut rift_live = false;
        // Tick a spawner last appeared, consumed by the next Rift that opens.
        let mut rift_pending_announce: Option<i32> = None;
        let mut rift_cur_announce: Option<i32> = None;
        let mut rift_cur_active_tick: i32 = 0;
        let mut rift_cur_capture_tick: Option<i32> = None;
        let mut rift_cur_winning_team: Option<i32> = None;
        let mut rift_cur_loc: [f32; 3] = [0.0; 3];
        // m_nKothScoringTeam holds the *previous* Rift's winner until the next
        // one opens, so a positive value only counts as a capture once this Rift
        // has been observed contested (-1). Without this a stale winner would
        // register a capture on the same tick the Rift opened.
        let mut rift_seen_contested = false;
        let mut rift_spawners_prev: std::collections::HashSet<i32> =
            std::collections::HashSet::new();
        let mut rift_spawners_cur: std::collections::HashSet<i32> =
            std::collections::HashSet::new();

        // Track active modifiers by serial_number for change detection
        struct CachedMod {
            hero_id: i64,
            modifier_id: u32,
            ability_id: u32,
            duration: f32,
            caster_hero_id: i64,
            stacks: i32,
        }
        let mut am_prev: HashMap<u32, CachedMod> = HashMap::new();
        // ActiveModifiers entry index -> serial currently stored there. Lets us
        // detect a removal when a slot is reused by a new modifier without an
        // explicit `entry_type == 2` (replaces the old full-table rescan).
        let mut am_idx_serial: HashMap<usize, u32> = HashMap::new();

        // ability_ticks: per-ability-class resolved field keys (cached on first
        // sight of each class) and per-entity previous state for change detection.
        struct AbilityKeys {
            subclass_id: Option<u64>,
            slot: Option<u64>,
            cooldown_start: Option<u64>,
            cooldown_end: Option<u64>,
            remaining_charges: Option<u64>,
            recharge_start: Option<u64>,
            recharge_end: Option<u64>,
            owner: Option<u64>,
        }
        #[derive(PartialEq)]
        struct AbilState {
            cooldown_start: f32,
            cooldown_end: f32,
            remaining_charges: i32,
            recharge_start: f32,
            recharge_end: f32,
        }
        let mut ability_keys_cache: HashMap<String, AbilityKeys> = HashMap::new();
        let mut abil_prev: HashMap<i32, AbilState> = HashMap::new();

        // Track idol modifiers for urn lifecycle
        const GOLDEN_IDOL_ABILITY: u32 = 2521299219;
        const IDOL_RETURN: u32 = 3388847715;

        // serial -> hero_id for golden_idol modifiers (carrying state)
        let mut urn_idol_serials: HashMap<u32, i64> = HashMap::new();
        // ActiveModifiers entry index -> idol-relevant serial currently there
        // (golden_idol or idol_return). Mirrors `am_idx_serial` for the urn pass
        // so a slot being reused counts as the old idol modifier disappearing.
        let mut urn_idx_serial: HashMap<usize, u32> = HashMap::new();
        // hero_id -> number of active golden_idol modifiers
        let mut urn_hero_count: HashMap<i64, i32> = HashMap::new();
        // serials for idol_return modifiers already emitted
        let mut urn_return_seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        // hero_id -> last tick a "returned" event was emitted (dedup flicker)
        let mut urn_last_return_tick: HashMap<i64, i32> = HashMap::new();
        // entity_idx -> (disabled, team_num) for delivery trigger change detection
        let mut urn_trigger_prev: HashMap<i32, (bool, i64)> = HashMap::new();

        // ── Field keys ──
        let mut keys_resolved = false;

        // Pawn keys (needed for player_ticks and kills entity_to_hero)
        let mut pk_hero_id: Option<u64> = None;
        let mut pk_vec_x: Option<u64> = None;
        let mut pk_vec_y: Option<u64> = None;
        let mut pk_vec_z: Option<u64> = None;
        let mut pk_cell_x: Option<u64> = None;
        let mut pk_cell_y: Option<u64> = None;
        let mut pk_cell_z: Option<u64> = None;
        let mut pk_camera: Option<u64> = None;
        let mut pk_in_regen: Option<u64> = None;
        let mut pk_in_item_shop: Option<u64> = None;
        let mut pk_death_time: Option<u64> = None;
        let mut pk_last_spawn: Option<u64> = None;
        let mut pk_respawn: Option<u64> = None;
        let mut pk_health: Option<u64> = None;
        let mut pk_max_health: Option<u64> = None;
        let mut pk_lifestate: Option<u64> = None;
        let mut pk_souls: Option<u64> = None;
        let mut pk_spent_souls: Option<u64> = None;
        let mut pk_combat_end: Option<u64> = None;
        let mut pk_combat_last_dmg: Option<u64> = None;
        let mut pk_combat_start: Option<u64> = None;
        let mut pk_dmg_dealt_end: Option<u64> = None;
        let mut pk_dmg_dealt_last: Option<u64> = None;
        let mut pk_dmg_dealt_start: Option<u64> = None;
        let mut pk_dmg_taken_end: Option<u64> = None;
        let mut pk_dmg_taken_last: Option<u64> = None;
        let mut pk_dmg_taken_start: Option<u64> = None;
        let mut pk_time_revealed: Option<u64> = None;
        let mut pk_build_id: Option<u64> = None;

        // Controller keys
        let mut ck_pawn_handle: Option<u64> = None;
        let mut ck_alive: Option<u64> = None;
        let mut ck_rebirth: Option<u64> = None;
        let mut ck_rejuvenator: Option<u64> = None;
        let mut ck_ultimate: Option<u64> = None;
        let mut ck_health_regen: Option<u64> = None;
        let mut ck_health_max: Option<u64> = None;
        let mut ck_ult_cd_end: Option<u64> = None;
        let mut ck_ult_cd_start: Option<u64> = None;
        let mut ck_ap_nw: Option<u64> = None;
        let mut ck_gold_nw: Option<u64> = None;
        let mut ck_denies: Option<u64> = None;
        let mut ck_hero_damage: Option<u64> = None;
        let mut ck_hero_healing: Option<u64> = None;
        let mut ck_obj_damage: Option<u64> = None;
        let mut ck_self_healing: Option<u64> = None;
        let mut ck_kill_streak: Option<u64> = None;
        let mut ck_last_hits: Option<u64> = None;
        let mut ck_level: Option<u64> = None;
        let mut ck_kills: Option<u64> = None;
        let mut ck_deaths: Option<u64> = None;
        let mut ck_assists: Option<u64> = None;

        // Controller hero_id key (for purchases/shop_events slot→hero mapping)
        let mut ck_hero_id: Option<u64> = None;

        // Ability upgrade slot keys: (item_id_key, upgrade_bits_key) for indices 0..7
        let mut au_slot_keys: Vec<(Option<u64>, Option<u64>)> = Vec::new();

        // Objective NPC keys (shared across all NPC classes)
        let mut nk_health: Option<u64> = None;
        let mut nk_max_health: Option<u64> = None;
        let mut nk_team_num: Option<u64> = None;
        let mut nk_lane: Option<u64> = None;
        let mut nk_vec_x: Option<u64> = None;
        let mut nk_vec_y: Option<u64> = None;
        let mut nk_vec_z: Option<u64> = None;
        let mut nk_cell_x: Option<u64> = None;
        let mut nk_cell_y: Option<u64> = None;
        let mut nk_cell_z: Option<u64> = None;
        // Shrine (CCitadel_Destroyable_Building) has different field keys
        let mut shrine_health: Option<u64> = None;
        let mut shrine_max_health: Option<u64> = None;
        let mut shrine_vec_x: Option<u64> = None;
        let mut shrine_vec_y: Option<u64> = None;
        let mut shrine_vec_z: Option<u64> = None;
        let mut shrine_cell_x: Option<u64> = None;
        let mut shrine_cell_y: Option<u64> = None;
        let mut shrine_cell_z: Option<u64> = None;
        let mut shrine_team_num: Option<u64> = None;

        // Trooper NPC keys (lane troopers)
        let mut tk_health: Option<u64> = None;
        let mut tk_max_health: Option<u64> = None;
        let mut tk_team_num: Option<u64> = None;
        let mut tk_lane: Option<u64> = None;
        let mut tk_lifestate: Option<u64> = None;
        let mut tk_vec_x: Option<u64> = None;
        let mut tk_vec_y: Option<u64> = None;
        let mut tk_vec_z: Option<u64> = None;
        let mut tk_cell_x: Option<u64> = None;
        let mut tk_cell_y: Option<u64> = None;
        let mut tk_cell_z: Option<u64> = None;

        // Neutral NPC keys
        let mut ntk_health: Option<u64> = None;
        let mut ntk_max_health: Option<u64> = None;
        let mut ntk_team_num: Option<u64> = None;
        let mut ntk_lifestate: Option<u64> = None;
        let mut ntk_vec_x: Option<u64> = None;
        let mut ntk_vec_y: Option<u64> = None;
        let mut ntk_vec_z: Option<u64> = None;
        let mut ntk_cell_x: Option<u64> = None;
        let mut ntk_cell_y: Option<u64> = None;
        let mut ntk_cell_z: Option<u64> = None;

        // StatViewerModifierValues keys for indices 0..20: (modifier_id, val_type, value)
        let mut smk_keys: Vec<(Option<u64>, Option<u64>, Option<u64>)> = Vec::new();

        // World keys
        let mut wk_is_paused: Option<u64> = None;
        let mut wk_next_midboss: Option<u64> = None;

        // Urn delivery trigger keys (CCitadelIdolReturnTrigger)
        let mut urnk_disabled: Option<u64> = None;
        let mut urnk_team_num: Option<u64> = None;
        let mut urnk_vec_x: Option<u64> = None;
        let mut urnk_vec_y: Option<u64> = None;
        let mut urnk_vec_z: Option<u64> = None;
        let mut urnk_cell_x: Option<u64> = None;
        let mut urnk_cell_y: Option<u64> = None;
        let mut urnk_cell_z: Option<u64> = None;

        // Street brawl keys
        let mut sbk_round: Option<u64> = None;
        let mut sbk_state: Option<u64> = None;
        let mut sbk_amber_score: Option<u64> = None;
        let mut sbk_sapphire_score: Option<u64> = None;
        let mut sbk_buy_countdown: Option<u64> = None;
        let mut sbk_next_state_time: Option<u64> = None;
        let mut sbk_state_start_time: Option<u64> = None;
        let mut sbk_non_combat_time: Option<u64> = None;

        // Rift (Koth) keys, on the game rules entity
        let mut rk_cashin_started: Option<u64> = None;
        let mut rk_scoring_team: Option<u64> = None;
        let mut rk_location: Option<u64> = None;

        // ── Single-pass callback logic (shared between both code paths) ──
        //
        // We use a macro to avoid duplicating the entity extraction code across
        // the events-aware and entities-only branches.
        macro_rules! collect_entity_data {
            ($ctx:expr) => {
                if !keys_resolved {
                    if load_abilities || load_player_ticks || load_kills || load_damage || load_active_modifiers || load_urn || load_ability_ticks {
                        if let Some(s) = $ctx.serializers().get("CCitadelPlayerPawn") {
                            pk_hero_id = s.resolve_field_key(
                                "m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID",
                            );
                            if load_player_ticks || load_urn {
                                pk_vec_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                                );
                                pk_vec_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                                );
                                pk_vec_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                                );
                                pk_cell_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX",
                                );
                                pk_cell_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY",
                                );
                                pk_cell_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ",
                                );
                                pk_camera = s.resolve_field_key("m_angClientCamera");
                                pk_in_regen = s.resolve_field_key("m_bInRegenerationZone");
                                pk_in_item_shop = s.resolve_field_key("m_bInItemShopZone");
                                pk_death_time = s.resolve_field_key("m_flDeathTime");
                                pk_last_spawn = s.resolve_field_key("m_flLastSpawnTime");
                                pk_respawn = s.resolve_field_key("m_flRespawnTime");
                                pk_health = s.resolve_field_key("m_iHealth");
                                pk_max_health = s.resolve_field_key("m_iMaxHealth");
                                pk_lifestate = s.resolve_field_key("m_lifeState");
                                pk_souls = s.resolve_field_key("m_nCurrencies.m_nCurrencies");
                                pk_spent_souls =
                                    s.resolve_field_key("m_nSpentCurrencies.m_nSpentCurrencies");
                                pk_combat_end = s.resolve_field_key("m_sInCombat.m_flEndTime");
                                pk_combat_last_dmg =
                                    s.resolve_field_key("m_sInCombat.m_flLastDamageTime");
                                pk_combat_start = s.resolve_field_key("m_sInCombat.m_flStartTime");
                                pk_dmg_dealt_end =
                                    s.resolve_field_key("m_sPlayerDamageDealt.m_flEndTime");
                                pk_dmg_dealt_last =
                                    s.resolve_field_key("m_sPlayerDamageDealt.m_flLastDamageTime");
                                pk_dmg_dealt_start =
                                    s.resolve_field_key("m_sPlayerDamageDealt.m_flStartTime");
                                pk_dmg_taken_end =
                                    s.resolve_field_key("m_sPlayerDamageTaken.m_flEndTime");
                                pk_dmg_taken_last =
                                    s.resolve_field_key("m_sPlayerDamageTaken.m_flLastDamageTime");
                                pk_dmg_taken_start =
                                    s.resolve_field_key("m_sPlayerDamageTaken.m_flStartTime");
                                pk_time_revealed =
                                    s.resolve_field_key("m_timeRevealedOnMinimapByNPC");
                                pk_build_id = s.resolve_field_key("m_unHeroBuildID");
                            }
                        }
                    }
                    if load_player_ticks {
                        if let Some(s) = $ctx.serializers().get("CCitadelPlayerController") {
                            ck_pawn_handle = s.resolve_field_key("m_hPawn");
                            ck_alive = s.resolve_field_key("m_PlayerDataGlobal.m_bAlive");
                            ck_rebirth =
                                s.resolve_field_key("m_PlayerDataGlobal.m_bHasRebirth");
                            ck_rejuvenator =
                                s.resolve_field_key("m_PlayerDataGlobal.m_bHasRejuvenator");
                            ck_ultimate =
                                s.resolve_field_key("m_PlayerDataGlobal.m_bUltimateTrained");
                            ck_health_regen =
                                s.resolve_field_key("m_PlayerDataGlobal.m_flHealthRegen");
                            // Effective max health. The pawn's m_iMaxHealth is a
                            // base/stale value that current health routinely
                            // exceeds; the controller's m_iHealthMax is the live
                            // total (level + items + buffs).
                            ck_health_max =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iHealthMax");
                            ck_ult_cd_end = s
                                .resolve_field_key("m_PlayerDataGlobal.m_flUltimateCooldownEnd");
                            ck_ult_cd_start = s.resolve_field_key(
                                "m_PlayerDataGlobal.m_flUltimateCooldownStart",
                            );
                            ck_ap_nw =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iAPNetWorth");
                            ck_gold_nw =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iGoldNetWorth");
                            ck_denies = s.resolve_field_key("m_PlayerDataGlobal.m_iDenies");
                            ck_hero_damage =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iHeroDamage");
                            ck_hero_healing =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iHeroHealing");
                            ck_obj_damage =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iObjectiveDamage");
                            ck_self_healing =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iSelfHealing");
                            ck_kill_streak =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iKillStreak");
                            ck_last_hits =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iLastHits");
                            ck_level = s.resolve_field_key("m_PlayerDataGlobal.m_iLevel");
                            ck_kills =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iPlayerKills");
                            ck_deaths = s.resolve_field_key("m_PlayerDataGlobal.m_iDeaths");
                            ck_assists =
                                s.resolve_field_key("m_PlayerDataGlobal.m_iPlayerAssists");
                        }
                    }
                    if load_item_purchases || load_chat {
                        if let Some(s) = $ctx.serializers().get("CCitadelPlayerController") {
                            ck_hero_id =
                                s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                        }
                    }
                    if load_ability_upgrades {
                        if let Some(s) = $ctx.serializers().get("CCitadelPlayerController") {
                            if ck_hero_id.is_none() {
                                ck_hero_id =
                                    s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                            }
                            for i in 0..8usize {
                                let item_key = s.resolve_field_key(&format!(
                                    "m_PlayerDataGlobal.m_vecAbilityUpgradeState.{i:04}.m_ItemID"
                                ));
                                let bits_key = s.resolve_field_key(&format!(
                                    "m_PlayerDataGlobal.m_vecAbilityUpgradeState.{i:04}.m_nUpgradeInfo"
                                ));
                                au_slot_keys.push((item_key, bits_key));
                            }
                        }
                    }
                    if load_stat_modifier_events {
                        if let Some(s) = $ctx.serializers().get("CCitadelPlayerController") {
                            if ck_hero_id.is_none() {
                                ck_hero_id =
                                    s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                            }
                            for i in 0..20usize {
                                let mid = s.resolve_field_key(&format!(
                                    "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_SourceModifierID"
                                ));
                                let vt = s.resolve_field_key(&format!(
                                    "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_eValType"
                                ));
                                let val = s.resolve_field_key(&format!(
                                    "m_PlayerDataGlobal.m_vecStatViewerModifierValues.{i}.m_flValue"
                                ));
                                smk_keys.push((mid, vt, val));
                            }
                        }
                    }
                    if load_objectives {
                        // NPC objective classes share field names; resolve from first found
                        for obj_class in &["CNPC_Boss_Tier2", "CNPC_Boss_Tier3", "CNPC_BarrackBoss", "CNPC_MidBoss"] {
                            if let Some(s) = $ctx.serializers().get(*obj_class) {
                                nk_health = s.resolve_field_key("m_iHealth");
                                nk_max_health = s.resolve_field_key("m_iMaxHealth");
                                nk_team_num = s.resolve_field_key("m_iTeamNum");
                                nk_lane = s.resolve_field_key("m_iLane");
                                nk_vec_x = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX");
                                nk_vec_y = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY");
                                nk_vec_z = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ");
                                nk_cell_x = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX");
                                nk_cell_y = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY");
                                nk_cell_z = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ");
                                break;
                            }
                        }
                        // Patron phase key
                        if let Some(s) = $ctx.serializers().get("CNPC_Boss_Tier3") {
                            patron_phase_key = s.resolve_field_key("m_ePhase");
                        }
                        // Shrine has a different serializer with different field keys
                        if let Some(s) = $ctx.serializers().get("CCitadel_Destroyable_Building") {
                            shrine_health = s.resolve_field_key("m_iHealth");
                            shrine_max_health = s.resolve_field_key("m_iMaxHealth");
                            shrine_team_num = s.resolve_field_key("m_iTeamNum");
                            shrine_vec_x = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX");
                            shrine_vec_y = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY");
                            shrine_vec_z = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ");
                            shrine_cell_x = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX");
                            shrine_cell_y = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY");
                            shrine_cell_z = s.resolve_field_key("CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ");
                        }
                    }
                    if load_troopers {
                        for tr_class in &["CNPC_Trooper", "CNPC_TrooperBoss"] {
                            if let Some(s) = $ctx.serializers().get(*tr_class) {
                                tk_health = s.resolve_field_key("m_iHealth");
                                tk_max_health = s.resolve_field_key("m_iMaxHealth");
                                tk_team_num = s.resolve_field_key("m_iTeamNum");
                                tk_lane = s.resolve_field_key("m_iLane");
                                tk_lifestate = s.resolve_field_key("m_lifeState");
                                tk_vec_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                                );
                                tk_vec_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                                );
                                tk_vec_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                                );
                                tk_cell_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX",
                                );
                                tk_cell_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY",
                                );
                                tk_cell_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ",
                                );
                                break;
                            }
                        }
                    }
                    if load_neutrals {
                        for nt_class in &["CNPC_TrooperNeutral"] {
                            if let Some(s) = $ctx.serializers().get(*nt_class) {
                                ntk_health = s.resolve_field_key("m_iHealth");
                                ntk_max_health = s.resolve_field_key("m_iMaxHealth");
                                ntk_team_num = s.resolve_field_key("m_iTeamNum");
                                ntk_lifestate = s.resolve_field_key("m_lifeState");
                                ntk_vec_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                                );
                                ntk_vec_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                                );
                                ntk_vec_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                                );
                                ntk_cell_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX",
                                );
                                ntk_cell_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY",
                                );
                                ntk_cell_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ",
                                );
                                break;
                            }
                        }
                    }
                    if load_world_ticks {
                        if let Some(s) = $ctx.serializers().get("CCitadelGameRulesProxy") {
                            wk_is_paused =
                                s.resolve_field_key("m_pGameRules.m_bGamePaused");
                            wk_next_midboss =
                                s.resolve_field_key("m_pGameRules.m_tNextMidBossSpawnTime");
                        }
                    }
                    if load_urn {
                        if let Some(s) = $ctx.serializers().get("CCitadelIdolReturnTrigger") {
                            urnk_disabled = s.resolve_field_key("m_bDisabled");
                            urnk_team_num = s.resolve_field_key("m_iTeamNum");
                            urnk_vec_x = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                            );
                            urnk_vec_y = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                            );
                            urnk_vec_z = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                            );
                            urnk_cell_x = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX",
                            );
                            urnk_cell_y = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY",
                            );
                            urnk_cell_z = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ",
                            );
                        }
                    }
                    if load_street_brawl_ticks {
                        if let Some(s) = $ctx.serializers().get("CCitadelGameRulesProxy") {
                            sbk_round = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_iRound");
                            sbk_state = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_eStreetBrawlState");
                            sbk_amber_score = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_iTeamAmberScore");
                            sbk_sapphire_score = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_iTeamSapphireScore");
                            sbk_buy_countdown = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_iLastBuyCountDown");
                            sbk_next_state_time = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_flNextStateTime");
                            sbk_state_start_time = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_flStreetBrawlStateStartTime");
                            sbk_non_combat_time = s.resolve_field_key("m_pGameRules.m_tStreetBrawl.m_flStreetBrawlTotalNonCombatTime");
                        }
                    }
                    if load_rift {
                        if let Some(s) = $ctx.serializers().get("CCitadelGameRulesProxy") {
                            rk_cashin_started =
                                s.resolve_field_key("m_pGameRules.m_timeKothCashInStarted");
                            rk_scoring_team =
                                s.resolve_field_key("m_pGameRules.m_nKothScoringTeam");
                            rk_location =
                                s.resolve_field_key("m_pGameRules.m_vKothCashInCurrentLocation");
                        }
                    }
                    keys_resolved = true;
                }

                // ── Collect player_ticks ──
                if load_player_ticks {
                    let controllers: Vec<&boon_parser::Entity> = $ctx
                        .entities()
                        .iter()
                        .filter(|(_, e)| e.class_name.as_ref() == "CCitadelPlayerController")
                        .map(|(_, e)| e)
                        .collect();

                    for ctrl in &controllers {
                        let pawn = match ctrl.get_handle(ck_pawn_handle)
                            .and_then(|h| $ctx.entities().get_by_handle(h))
                        {
                            Some(p) if p.class_name.as_ref() == "CCitadelPlayerPawn" => p,
                            _ => continue,
                        };

                        let hid = pawn.get_i64(pk_hero_id);
                        if hid == 0 {
                            continue;
                        }

                        pt_tick.push($ctx.tick());
                        pt_hero_id.push(hid);
                        let [pawn_x, pawn_y, pawn_z] = pawn.world_position(
                            [pk_cell_x, pk_cell_y, pk_cell_z],
                            [pk_vec_x, pk_vec_y, pk_vec_z],
                        );
                        pt_x.push(pawn_x);
                        pt_y.push(pawn_y);
                        pt_z.push(pawn_z);
                        let angles = pawn.get_qangle(pk_camera);
                        pt_pitch.push(angles[0]);
                        pt_yaw.push(angles[1]);
                        pt_roll.push(angles[2]);
                        pt_in_regen_zone.push(pawn.get_bool(pk_in_regen));
                        pt_in_item_shop.push(pawn.get_bool(pk_in_item_shop));
                        pt_death_time.push(pawn.get_f32(pk_death_time));
                        pt_last_spawn_time.push(pawn.get_f32(pk_last_spawn));
                        pt_respawn_time.push(pawn.get_f32(pk_respawn));
                        pt_health.push(pawn.get_i64(pk_health));
                        // Prefer the controller's effective max; fall back to the
                        // pawn's base max when the controller isn't populated yet.
                        let eff_max_health = ctrl.get_i64(ck_health_max);
                        pt_max_health.push(if eff_max_health > 0 {
                            eff_max_health
                        } else {
                            pawn.get_i64(pk_max_health)
                        });
                        pt_lifestate.push(pawn.get_i64(pk_lifestate));
                        pt_souls.push(pawn.get_i64(pk_souls));
                        pt_spent_souls.push(pawn.get_i64(pk_spent_souls));
                        pt_combat_end.push(pawn.get_f32(pk_combat_end));
                        pt_combat_last_dmg.push(pawn.get_f32(pk_combat_last_dmg));
                        pt_combat_start.push(pawn.get_f32(pk_combat_start));
                        pt_dmg_dealt_end.push(pawn.get_f32(pk_dmg_dealt_end));
                        pt_dmg_dealt_last.push(pawn.get_f32(pk_dmg_dealt_last));
                        pt_dmg_dealt_start.push(pawn.get_f32(pk_dmg_dealt_start));
                        pt_dmg_taken_end.push(pawn.get_f32(pk_dmg_taken_end));
                        pt_dmg_taken_last.push(pawn.get_f32(pk_dmg_taken_last));
                        pt_dmg_taken_start.push(pawn.get_f32(pk_dmg_taken_start));
                        pt_time_revealed.push(pawn.get_f32(pk_time_revealed));
                        pt_build_id.push(pawn.get_i64(pk_build_id));
                        pt_is_alive.push(ctrl.get_bool(ck_alive));
                        pt_has_rebirth.push(ctrl.get_bool(ck_rebirth));
                        pt_has_rejuvenator.push(ctrl.get_bool(ck_rejuvenator));
                        pt_has_ultimate.push(ctrl.get_bool(ck_ultimate));
                        pt_health_regen.push(ctrl.get_f32(ck_health_regen));
                        // Note: column start → field CooldownEnd, column end → field CooldownStart
                        pt_ult_cd_start.push(ctrl.get_f32(ck_ult_cd_end));
                        pt_ult_cd_end.push(ctrl.get_f32(ck_ult_cd_start));
                        pt_ap_nw.push(ctrl.get_i64(ck_ap_nw));
                        pt_gold_nw.push(ctrl.get_i64(ck_gold_nw));
                        pt_denies.push(ctrl.get_i64(ck_denies));
                        pt_hero_damage.push(ctrl.get_i64(ck_hero_damage));
                        pt_hero_healing.push(ctrl.get_i64(ck_hero_healing));
                        pt_obj_damage.push(ctrl.get_i64(ck_obj_damage));
                        pt_self_healing.push(ctrl.get_i64(ck_self_healing));
                        pt_kill_streak.push(ctrl.get_i64(ck_kill_streak));
                        pt_last_hits.push(ctrl.get_i64(ck_last_hits));
                        pt_level.push(ctrl.get_i64(ck_level));
                        pt_kills.push(ctrl.get_i64(ck_kills));
                        pt_deaths.push(ctrl.get_i64(ck_deaths));
                        pt_assists.push(ctrl.get_i64(ck_assists));
                    }
                }

                // ── Collect world_ticks / street_brawl_ticks ──
                if load_world_ticks || load_street_brawl_ticks {
                    if let Some((_, entity)) = $ctx
                        .entities()
                        .iter()
                        .find(|(_, e)| e.class_name.as_ref() == "CCitadelGameRulesProxy")
                    {
                        if load_world_ticks {
                            wt_tick.push($ctx.tick());
                            wt_is_paused.push(entity.get_bool(wk_is_paused));
                            wt_next_midboss.push(entity.get_f32(wk_next_midboss));
                        }
                        if load_street_brawl_ticks {
                            sbt_tick.push($ctx.tick());
                            sbt_round.push(entity.get_i64(sbk_round) as i32);
                            sbt_state.push(entity.get_i64(sbk_state) as i32);
                            sbt_amber_score.push(entity.get_i64(sbk_amber_score) as i32);
                            sbt_sapphire_score.push(entity.get_i64(sbk_sapphire_score) as i32);
                            sbt_buy_countdown.push(entity.get_i64(sbk_buy_countdown) as i32);
                            sbt_next_state_time.push(entity.get_f32(sbk_next_state_time));
                            sbt_state_start_time.push(entity.get_f32(sbk_state_start_time));
                            sbt_non_combat_time.push(entity.get_f32(sbk_non_combat_time));
                        }
                    }
                }

                // ── Collect rift (Koth) lifecycle ──
                if load_rift {
                    // A spawner that was absent last tick announces the next
                    // Rift. Entity indices get recycled and m_flCreateTime is not
                    // transmitted, so presence-diffing is the only reliable way
                    // to spot the spawn.
                    for (idx, entity) in $ctx.entities().iter() {
                        if entity.class_name.as_ref() == "CCitadelItemKothSpawner" {
                            rift_spawners_cur.insert(idx);
                            if !rift_spawners_prev.contains(&idx) && !rift_live {
                                rift_pending_announce = Some($ctx.tick());
                            }
                        }
                    }
                    std::mem::swap(&mut rift_spawners_prev, &mut rift_spawners_cur);
                    rift_spawners_cur.clear();

                    if let Some((_, entity)) = $ctx
                        .entities()
                        .iter()
                        .find(|(_, e)| e.class_name.as_ref() == "CCitadelGameRulesProxy")
                    {
                        // m_timeKothCashInStarted holds a real GameTime_t while a
                        // Rift is contestable and 0 otherwise. It is also re-armed
                        // mid-Rift (resetting the give-up timer), so only the
                        // 0 -> non-zero edge marks the start.
                        let cashin_started = entity.get_f32(rk_cashin_started);
                        let live = cashin_started > 0.0 && cashin_started.is_finite();
                        let scoring_team = entity.get_i64(rk_scoring_team) as i32;

                        if live && !rift_live {
                            rift_live = true;
                            rift_cur_announce = rift_pending_announce.take();
                            rift_cur_active_tick = $ctx.tick();
                            rift_cur_capture_tick = None;
                            rift_cur_winning_team = None;
                            rift_cur_loc = [0.0; 3];
                            rift_seen_contested = scoring_team <= 0;
                        }

                        if rift_live {
                            // Only read the location while the cash-in is still
                            // live: it is cleared to FLT_MAX on the same tick the
                            // Rift resolves, which would otherwise overwrite the
                            // real position just before the row is emitted.
                            if live {
                                let loc = entity.get_vector3(rk_location);
                                if loc != [0.0; 3]
                                    && loc.iter().all(|c| c.abs() < RIFT_COORD_SANITY)
                                {
                                    rift_cur_loc = loc;
                                }
                            }
                            if scoring_team <= 0 {
                                rift_seen_contested = true;
                            } else if rift_seen_contested && rift_cur_capture_tick.is_none() {
                                rift_cur_capture_tick = Some($ctx.tick());
                                rift_cur_winning_team = Some(scoring_team);
                            }
                        }

                        if !live && rift_live {
                            rift_live = false;
                            rift_counter += 1;
                            rift_num.push(rift_counter);
                            rift_announce_tick.push(rift_cur_announce);
                            rift_active_tick.push(rift_cur_active_tick);
                            rift_capture_tick.push(rift_cur_capture_tick);
                            // No winner by the time the Rift clears => it timed
                            // out (see m_timeKothGiveUp).
                            rift_expire_tick.push(if rift_cur_capture_tick.is_none() {
                                Some($ctx.tick())
                            } else {
                                None
                            });
                            rift_winning_team.push(rift_cur_winning_team);
                            rift_lane.push(rift_lane_for(rift_cur_loc[0], rift_cur_loc[1]));
                            rift_x.push(rift_cur_loc[0]);
                            rift_y.push(rift_cur_loc[1]);
                            rift_z.push(rift_cur_loc[2]);
                        }
                    }
                }

                // ── Build entity_to_hero map (for kills/damage/mid_boss resolution) ──
                if (load_abilities || load_kills || load_damage || load_mid_boss || load_active_modifiers || load_urn || load_ability_ticks) && !entity_to_hero_built {
                    for (idx, entity) in $ctx.entities().iter() {
                        if entity.class_name.as_ref() == "CCitadelPlayerPawn" {
                            let hid = entity.get_i64(pk_hero_id);
                            if hid != 0 {
                                entity_to_hero.insert(idx, hid);
                            }
                        }
                    }
                    entity_to_hero_built = true;
                }

                // ── Build slot_to_hero map (for item_purchases/chat: userid → hero_id) ──
                if (load_item_purchases || load_chat) && !slot_to_hero_built {
                    for (idx, entity) in $ctx.entities().iter() {
                        if entity.class_name.as_ref() == "CCitadelPlayerController" {
                            let hid = entity.get_i64(ck_hero_id);
                            if hid != 0 {
                                // userid is 0-based, controller entity index is 1-based
                                slot_to_hero.insert(idx - 1, hid);
                            }
                        }
                    }
                    if !slot_to_hero.is_empty() {
                        slot_to_hero_built = true;
                    }
                }

                // ── Collect ability_upgrades (entity change detection) ──
                if load_ability_upgrades {
                    for (idx, entity) in $ctx.entities().iter() {
                        if entity.class_name.as_ref() != "CCitadelPlayerController" {
                            continue;
                        }
                        let hero_id = entity.get_i64(ck_hero_id);
                        if hero_id == 0 {
                            continue;
                        }
                        for (slot_idx, (item_key, bits_key)) in au_slot_keys.iter().enumerate() {
                            let ability_id = entity.get_u32(*item_key);
                            if ability_id == 0 {
                                continue;
                            }
                            // m_nUpgradeInfo packs upgrade bits in bits 17+
                            let upgrade_bits = bits_key
                                .and_then(|k| entity.fields.get(&k))
                                .and_then(|v| match v {
                                    boon_parser::FieldValue::I32(n) => Some((*n >> 17) as i32),
                                    boon_parser::FieldValue::I64(n) => Some((*n >> 17) as i32),
                                    boon_parser::FieldValue::U32(n) => Some((*n >> 17) as i32),
                                    boon_parser::FieldValue::U64(n) => Some((*n >> 17) as i32),
                                    _ => None,
                                })
                                .unwrap_or(0);
                            let key = (idx, slot_idx);
                            let prev = au_prev_bits.get(&key).copied().unwrap_or(0);
                            if upgrade_bits != prev {
                                au_prev_bits.insert(key, upgrade_bits);
                                if upgrade_bits > prev {
                                    au_ticks.push($ctx.tick());
                                    au_hero_ids.push(hero_id);
                                    au_ability_ids.push(ability_id);
                                    au_tier.push(upgrade_bits.count_ones() as i32 - 1);
                                }
                            }
                        }
                    }
                }

                // ── Collect objectives (change detection on health/max_health/phase) ──
                if load_objectives {
                    for &idx in $ctx.entities().updated_indices() {
                        let Some(entity) = $ctx.entities().get(idx) else {
                            continue;
                        };
                        let obj_class = entity.class_name.as_ref();
                        let is_patron = obj_class == "CNPC_Boss_Tier3";
                        let (otype, hp_key, max_hp_key, team_key, lane_key, cell_keys, offset_keys) = match obj_class {
                            "CNPC_Boss_Tier2" => ("walker", nk_health, nk_max_health, nk_team_num, nk_lane, [nk_cell_x, nk_cell_y, nk_cell_z], [nk_vec_x, nk_vec_y, nk_vec_z]),
                            "CNPC_Boss_Tier3" => ("patron", nk_health, nk_max_health, nk_team_num, nk_lane, [nk_cell_x, nk_cell_y, nk_cell_z], [nk_vec_x, nk_vec_y, nk_vec_z]),
                            "CNPC_BarrackBoss" => ("barracks", nk_health, nk_max_health, nk_team_num, nk_lane, [nk_cell_x, nk_cell_y, nk_cell_z], [nk_vec_x, nk_vec_y, nk_vec_z]),
                            "CNPC_MidBoss" => ("mid_boss", nk_health, nk_max_health, nk_team_num, nk_lane, [nk_cell_x, nk_cell_y, nk_cell_z], [nk_vec_x, nk_vec_y, nk_vec_z]),
                            "CCitadel_Destroyable_Building" => ("shrine", shrine_health, shrine_max_health, shrine_team_num, None, [shrine_cell_x, shrine_cell_y, shrine_cell_z], [shrine_vec_x, shrine_vec_y, shrine_vec_z]),
                            _ => continue,
                        };
                        let max_hp = entity.get_i64(max_hp_key);
                        if max_hp == 0 {
                            continue;
                        }
                        let hp = entity.get_i64(hp_key);
                        let phase = if is_patron { entity.get_i64(patron_phase_key) } else { 0 };
                        let cur = (hp, max_hp, phase);
                        let changed = match obj_prev.get(&idx) {
                            None => true,
                            Some(prev) => *prev != cur,
                        };
                        if changed {
                            obj_prev.insert(idx, cur);
                            obj_tick.push($ctx.tick());
                            obj_type.push(otype.to_string());
                            obj_team_num.push(entity.get_i64(team_key));
                            obj_lane.push(entity.get_i64(lane_key));
                            obj_health.push(hp);
                            obj_max_health.push(max_hp);
                            obj_phase.push(phase);
                            let [ox, oy, oz] = entity.world_position(cell_keys, offset_keys);
                            obj_x.push(ox);
                            obj_y.push(oy);
                            obj_z.push(oz);
                            obj_entity_id.push(idx);
                        }

                    }
                }

                // ── Collect troopers (lane troopers, per-tick alive only) ──
                if load_troopers {
                    for (idx, entity) in $ctx.entities().iter() {
                        let ttype = match entity.class_name.as_ref() {
                            "CNPC_Trooper" => "trooper",
                            "CNPC_TrooperBoss" => "trooper_boss",
                            _ => continue,
                        };
                        let max_hp = entity.get_i64(tk_max_health);
                        if max_hp == 0 {
                            continue;
                        }
                        let lifestate = entity.get_i64(tk_lifestate);
                        if lifestate != 0 {
                            continue;
                        }
                        tr_tick.push($ctx.tick());
                        tr_type.push(ttype.to_string());
                        tr_team_num.push(entity.get_i64(tk_team_num));
                        tr_lane.push(entity.get_i64(tk_lane));
                        tr_health.push(entity.get_i64(tk_health));
                        tr_max_health.push(max_hp);
                        let [trx, try_, trz] = entity.world_position(
                            [tk_cell_x, tk_cell_y, tk_cell_z],
                            [tk_vec_x, tk_vec_y, tk_vec_z],
                        );
                        tr_x.push(trx);
                        tr_y.push(try_);
                        tr_z.push(trz);
                        tr_entity_id.push(idx);
                    }
                }

                // ── Collect stat_modifiers (event-based change detection) ──
                if load_stat_modifier_events {
                    for (idx, entity) in $ctx.entities().iter() {
                        if entity.class_name.as_ref() != "CCitadelPlayerController" {
                            continue;
                        }
                        let hero_id = entity.get_i64(ck_hero_id);
                        if hero_id == 0 {
                            continue;
                        }

                        // Sum values by eValType
                        let mut by_type: HashMap<u32, f32> = HashMap::new();
                        for (_mid_key, vt_key, val_key) in &smk_keys {
                            let vt_val = vt_key
                                .and_then(|k| entity.fields.get(&k))
                                .and_then(|v| match v {
                                    boon_parser::FieldValue::U32(n) => Some(*n),
                                    boon_parser::FieldValue::I32(n) => Some(*n as u32),
                                    boon_parser::FieldValue::U64(n) => Some(*n as u32),
                                    boon_parser::FieldValue::I64(n) => Some(*n as u32),
                                    _ => None,
                                })
                                .unwrap_or(0);
                            if vt_val == 0 {
                                continue;
                            }
                            let fl_val = entity.get_f32(*val_key);
                            *by_type.entry(vt_val).or_insert(0.0) += fl_val;
                        }

                        // Emit events for changed stat types
                        for (vt_val, total) in &by_type {
                            let key = (idx, *vt_val);
                            let prev = sm_prev.get(&key).copied().unwrap_or(0.0);
                            if (*total - prev).abs() > f32::EPSILON {
                                sm_prev.insert(key, *total);
                                let stat_name = match *vt_val {
                                    31 => "health",
                                    51 => "spirit_power",
                                    79 => "fire_rate",
                                    18 => "weapon_damage",
                                    109 => "cooldown_reduction",
                                    172 => "ammo",
                                    _ => continue,
                                };
                                sm_tick.push($ctx.tick());
                                sm_hero_id.push(hero_id);
                                sm_stat_type.push(stat_name.to_string());
                                sm_amount.push(*total - prev);
                            }
                        }
                    }
                }

                // ── Collect active_modifiers (string table change detection) ──
                //
                // The ActiveModifiers string table grows to >1000 entries and is
                // delta-updated, so rescanning + re-decoding every entry each tick
                // is the dominant cost of this dataset. Instead we decode only the
                // entries the delta touched this tick (`dirty_indices`) and keep an
                // index -> serial map. A removal shows up either as an explicit
                // `entry_type == 2` or as a slot being reused by a new serial; both
                // are changes to that index, so both are caught here. This is exact
                // because the table never shrinks and indices are stable (a serial
                // only leaves the table when its slot is rewritten).
                if load_active_modifiers {
                    if let Some(table) = $ctx.string_tables().find_table("ActiveModifiers") {
                        for &idx in table.dirty_indices() {
                            let Some(entry) = table.entries().get(idx) else {
                                continue;
                            };
                            let data = match &entry.user_data {
                                Some(d) if !d.is_empty() => d,
                                _ => continue,
                            };

                            let Ok(modifier) =
                                boon_proto::proto::CModifierTableEntry::decode(data.as_slice())
                            else {
                                continue;
                            };

                            let Some(serial) = modifier.serial_number else { continue };

                            // Slot reused by a different serial => the old modifier
                            // was removed without an explicit entry_type == 2.
                            if let Some(old_serial) = am_idx_serial.get(&idx).copied()
                                && old_serial != serial
                                && let Some(cached) = am_prev.remove(&old_serial)
                            {
                                am_tick.push($ctx.tick());
                                am_hero_id.push(cached.hero_id);
                                am_event.push("removed".to_string());
                                am_modifier_id.push(cached.modifier_id);
                                am_ability_id.push(cached.ability_id);
                                am_duration.push(cached.duration);
                                am_caster_hero_id.push(cached.caster_hero_id);
                                am_stacks.push(cached.stacks);
                            }

                            let mod_entry_type = modifier.entry_type.unwrap_or(1);

                            if mod_entry_type == 2 {
                                am_idx_serial.remove(&idx);
                                if let Some(cached) = am_prev.remove(&serial) {
                                    am_tick.push($ctx.tick());
                                    am_hero_id.push(cached.hero_id);
                                    am_event.push("removed".to_string());
                                    am_modifier_id.push(cached.modifier_id);
                                    am_ability_id.push(cached.ability_id);
                                    am_duration.push(cached.duration);
                                    am_caster_hero_id.push(cached.caster_hero_id);
                                    am_stacks.push(cached.stacks);
                                }
                                continue;
                            }

                            am_idx_serial.insert(idx, serial);

                            let Some(parent_idx) =
                                boon_parser::protobuf_handle_index(modifier.parent)
                            else {
                                continue;
                            };

                            let Some(&hero_id) = entity_to_hero.get(&parent_idx) else {
                                continue;
                            };

                            match am_prev.entry(serial) {
                                std::collections::hash_map::Entry::Vacant(e) => {
                                    let mod_id = modifier.modifier_subclass.unwrap_or(0);
                                    let abil_id = modifier.ability_subclass.unwrap_or(0);
                                    let duration = modifier.duration.unwrap_or(-1.0);
                                    let caster_hero_id =
                                        boon_parser::protobuf_handle_index(modifier.caster)
                                            .and_then(|i| entity_to_hero.get(&i).copied())
                                            .unwrap_or(0);
                                    let stacks = modifier.stack_count.unwrap_or(0);

                                    am_tick.push($ctx.tick());
                                    am_hero_id.push(hero_id);
                                    am_event.push("applied".to_string());
                                    am_modifier_id.push(mod_id);
                                    am_ability_id.push(abil_id);
                                    am_duration.push(duration);
                                    am_caster_hero_id.push(caster_hero_id);
                                    am_stacks.push(stacks);

                                    e.insert(CachedMod {
                                        hero_id,
                                        modifier_id: mod_id,
                                        ability_id: abil_id,
                                        duration,
                                        caster_hero_id,
                                        stacks,
                                    });
                                }
                                // Modifier already tracked: its string-table entry was
                                // re-sent with updated fields. Emit a `changed` row when
                                // the stack count moved (e.g. a stacking debuff accruing
                                // headshots) and refresh the cache, so the live count is
                                // visible and the eventual `removed` row reports the final
                                // total rather than the value at first sighting.
                                std::collections::hash_map::Entry::Occupied(mut e) => {
                                    let stacks = modifier.stack_count.unwrap_or(0);
                                    let cached = e.get_mut();
                                    if stacks != cached.stacks {
                                        am_tick.push($ctx.tick());
                                        am_hero_id.push(cached.hero_id);
                                        am_event.push("changed".to_string());
                                        am_modifier_id.push(cached.modifier_id);
                                        am_ability_id.push(cached.ability_id);
                                        am_duration.push(cached.duration);
                                        am_caster_hero_id.push(cached.caster_hero_id);
                                        am_stacks.push(stacks);
                                        cached.stacks = stacks;
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Collect ability_ticks (entity change detection on cooldown/charges) ──
                //
                // Each ability is its own entity (one networked class per ability).
                // We walk the decoded ability entities each tick, read their
                // cooldown/charge fields, and emit a row only when an ability's
                // state changes (change-only, like active_modifiers). Field keys
                // differ per ability class, so they are resolved once per class and
                // cached. Owner -> hero comes from m_hOwnerEntity (the pawn).
                if load_ability_ticks {
                    // Only entities this tick changed: an ability's cooldown/charge
                    // state can only change on a tick it was updated.
                    for &idx in $ctx.entities().updated_indices() {
                        let Some(entity) = $ctx.entities().get(idx) else {
                            continue;
                        };
                        if !entity.class_name.contains("Ability") {
                            continue;
                        }
                        if !ability_keys_cache.contains_key(entity.class_name.as_ref()) {
                            let s = $ctx.serializers().get(&entity.class_name);
                            let r = |p: &str| s.and_then(|s| s.resolve_field_key(p));
                            let ak = AbilityKeys {
                                subclass_id: r("m_nSubclassID"),
                                slot: r("m_eAbilitySlot"),
                                cooldown_start: r("m_flCooldownStart"),
                                cooldown_end: r("m_flCooldownEnd"),
                                remaining_charges: r("m_iRemainingCharges"),
                                recharge_start: r("m_flChargeRechargeStart"),
                                recharge_end: r("m_flChargeRechargeEnd"),
                                owner: r("m_hOwnerEntity"),
                            };
                            ability_keys_cache.insert(entity.class_name.to_string(), ak);
                        }
                        let keys = &ability_keys_cache[entity.class_name.as_ref()];
                        // Capability gate: real abilities expose cooldown + charges.
                        if keys.cooldown_end.is_none() || keys.remaining_charges.is_none() {
                            continue;
                        }
                        let hero_id = entity
                            .get_handle(keys.owner)
                            .map(|h| (h & boon_parser::ENTITY_HANDLE_INDEX_MASK) as i32)
                            .and_then(|owner_idx| entity_to_hero.get(&owner_idx).copied())
                            .unwrap_or(0);
                        if hero_id == 0 {
                            continue;
                        }
                        let state = AbilState {
                            cooldown_start: entity.get_f32(keys.cooldown_start),
                            cooldown_end: entity.get_f32(keys.cooldown_end),
                            remaining_charges: entity.get_i64(keys.remaining_charges) as i32,
                            recharge_start: entity.get_f32(keys.recharge_start),
                            recharge_end: entity.get_f32(keys.recharge_end),
                        };
                        let changed = abil_prev.get(&idx).map(|p| *p != state).unwrap_or(true);
                        if changed {
                            at_tick.push($ctx.tick());
                            at_hero_id.push(hero_id);
                            at_ability_id.push(entity.get_u32(keys.subclass_id));
                            at_slot.push(entity.get_i64(keys.slot) as i32);
                            at_cooldown_start.push(state.cooldown_start);
                            at_cooldown_end.push(state.cooldown_end);
                            at_remaining_charges.push(state.remaining_charges);
                            at_charge_recharge_start.push(state.recharge_start);
                            at_charge_recharge_end.push(state.recharge_end);
                            abil_prev.insert(idx, state);
                        }
                    }
                }

                // ── Collect urn (idol lifecycle tracking) ──
                //
                // Same change-only strategy as active_modifiers: walk just the
                // entries the delta touched, keeping `urn_idx_serial` (entry index
                // -> idol serial there). A golden idol "drops" when its slot is
                // explicitly removed (entry_type == 2) or reused by another serial.
                // The pickup/drop counters are order-sensitive, so we process
                // touched indices in ascending index order — mirroring the previous
                // full scan — and defer slot-reuse drops to a post-pass, mirroring
                // the previous post-loop so per-tick ordering is unchanged.
                if load_urn {
                    if let Some(table) = $ctx.string_tables().find_table("ActiveModifiers") {
                        let mut dirty: Vec<usize> = table.dirty_indices().to_vec();
                        dirty.sort_unstable();
                        dirty.dedup();

                        // Golden serials whose slot was reused this tick; dropped
                        // after the main pass (matches the old post-loop ordering).
                        let mut urn_overwrite_gone: Vec<u32> = Vec::new();

                        for &idx in &dirty {
                            let Some(entry) = table.entries().get(idx) else {
                                continue;
                            };
                            let data = match &entry.user_data {
                                Some(d) if !d.is_empty() => d,
                                _ => continue,
                            };

                            let Ok(modifier) =
                                boon_proto::proto::CModifierTableEntry::decode(data.as_slice())
                            else {
                                continue;
                            };

                            let Some(serial) = modifier.serial_number else { continue };

                            let mod_entry_type = modifier.entry_type.unwrap_or(1);

                            // The idol serial previously stored at this slot leaving:
                            // explicit removal (handled inline below) or slot reuse
                            // (deferred to the post-pass).
                            if let Some(old_serial) = urn_idx_serial.get(&idx).copied() {
                                if old_serial != serial {
                                    urn_idx_serial.remove(&idx);
                                    urn_overwrite_gone.push(old_serial);
                                    urn_return_seen.remove(&old_serial);
                                } else if mod_entry_type == 2 {
                                    urn_idx_serial.remove(&idx);
                                }
                            }

                            // Handle explicit removal (entry_type == 2)
                            if mod_entry_type == 2 {
                                if let Some(hero_id) = urn_idol_serials.remove(&serial) {
                                    let count =
                                        urn_hero_count.entry(hero_id).or_insert(0);
                                    *count -= 1;
                                    if *count <= 0 {
                                        urn_hero_count.remove(&hero_id);
                                        let pawn = entity_to_hero.iter()
                                            .find(|(_, hid)| **hid == hero_id)
                                            .and_then(|(idx, _)| $ctx.entities().get(*idx));
                                        let [drop_x, drop_y, drop_z] = pawn.map_or(
                                            [0.0, 0.0, 0.0],
                                            |e| e.world_position(
                                                [pk_cell_x, pk_cell_y, pk_cell_z],
                                                [pk_vec_x, pk_vec_y, pk_vec_z],
                                            ),
                                        );
                                        urn_tick.push($ctx.tick());
                                        urn_event.push("dropped".to_string());
                                        urn_hero_id.push(hero_id);
                                        urn_team_num.push(0);
                                        urn_x.push(drop_x);
                                        urn_y.push(drop_y);
                                        urn_z.push(drop_z);
                                    }
                                }
                                urn_return_seen.remove(&serial);
                                continue;
                            }

                            let mod_id = modifier.modifier_subclass.unwrap_or(0);
                            let abil_id = modifier.ability_subclass.unwrap_or(0);
                            let is_golden_idol = abil_id == GOLDEN_IDOL_ABILITY;
                            let is_idol_return = mod_id == IDOL_RETURN;

                            if !is_golden_idol && !is_idol_return {
                                urn_idx_serial.remove(&idx);
                                continue;
                            }

                            let Some(parent_idx) =
                                boon_parser::protobuf_handle_index(modifier.parent)
                            else {
                                continue;
                            };

                            let Some(&hero_id) = entity_to_hero.get(&parent_idx) else {
                                continue;
                            };

                            // Look up pawn position for hero events
                            let pawn = $ctx.entities().get(parent_idx);
                            let [hero_x, hero_y, hero_z] = pawn.map_or(
                                [0.0, 0.0, 0.0],
                                |e| e.world_position(
                                    [pk_cell_x, pk_cell_y, pk_cell_z],
                                    [pk_vec_x, pk_vec_y, pk_vec_z],
                                ),
                            );

                            urn_idx_serial.insert(idx, serial);

                            if is_golden_idol
                                && !urn_idol_serials.contains_key(&serial)
                            {
                                let count =
                                    urn_hero_count.entry(hero_id).or_insert(0);
                                if *count == 0 {
                                    urn_tick.push($ctx.tick());
                                    urn_event.push("picked_up".to_string());
                                    urn_hero_id.push(hero_id);
                                    urn_team_num.push(0);
                                    urn_x.push(hero_x);
                                    urn_y.push(hero_y);
                                    urn_z.push(hero_z);
                                }
                                *count += 1;
                                urn_idol_serials.insert(serial, hero_id);
                            }

                            if is_idol_return && urn_return_seen.insert(serial) {
                                let last = urn_last_return_tick
                                    .get(&hero_id)
                                    .copied()
                                    .unwrap_or(-999);
                                if $ctx.tick() - last > 64 {
                                    urn_tick.push($ctx.tick());
                                    urn_event.push("returned".to_string());
                                    urn_hero_id.push(hero_id);
                                    urn_team_num.push(0);
                                    urn_x.push(hero_x);
                                    urn_y.push(hero_y);
                                    urn_z.push(hero_z);
                                    urn_last_return_tick.insert(hero_id, $ctx.tick());
                                }
                            }
                        }

                        // Slot-reuse drops (mirrors the previous post-loop).
                        for serial in urn_overwrite_gone {
                            if let Some(hero_id) = urn_idol_serials.remove(&serial) {
                                let count =
                                    urn_hero_count.entry(hero_id).or_insert(0);
                                *count -= 1;
                                if *count <= 0 {
                                    urn_hero_count.remove(&hero_id);
                                    let pawn = entity_to_hero.iter()
                                        .find(|(_, hid)| **hid == hero_id)
                                        .and_then(|(idx, _)| $ctx.entities().get(*idx));
                                    let [drop_x, drop_y, drop_z] = pawn.map_or(
                                        [0.0, 0.0, 0.0],
                                        |e| e.world_position(
                                            [pk_cell_x, pk_cell_y, pk_cell_z],
                                            [pk_vec_x, pk_vec_y, pk_vec_z],
                                        ),
                                    );
                                    urn_tick.push($ctx.tick());
                                    urn_event.push("dropped".to_string());
                                    urn_hero_id.push(hero_id);
                                    urn_team_num.push(0);
                                    urn_x.push(drop_x);
                                    urn_y.push(drop_y);
                                    urn_z.push(drop_z);
                                }
                            }
                        }
                    }
                }

                // ── Collect urn delivery triggers ──
                if load_urn {
                    for &idx in $ctx.entities().updated_indices() {
                        let Some(entity) = $ctx.entities().get(idx) else {
                            continue;
                        };
                        if entity.class_name.as_ref() != "CCitadelIdolReturnTrigger" {
                            continue;
                        }
                        let disabled = entity.get_bool(urnk_disabled);
                        let team = entity.get_i64(urnk_team_num);
                        let cur = (disabled, team);
                        let prev = urn_trigger_prev.get(&idx).copied();
                        let changed = match prev {
                            None => true,
                            Some(p) => p != cur,
                        };
                        if changed {
                            urn_trigger_prev.insert(idx, cur);
                            let [trig_x, trig_y, trig_z] = entity.world_position(
                                [urnk_cell_x, urnk_cell_y, urnk_cell_z],
                                [urnk_vec_x, urnk_vec_y, urnk_vec_z],
                            );
                            if !disabled && team != 0 {
                                urn_tick.push($ctx.tick());
                                urn_event.push("delivery_active".to_string());
                                urn_hero_id.push(0);
                                urn_team_num.push(team);
                                urn_x.push(trig_x);
                                urn_y.push(trig_y);
                                urn_z.push(trig_z);
                            } else if disabled {
                                // Only emit inactive when transitioning from active
                                if let Some((prev_disabled, _)) = prev {
                                    if !prev_disabled {
                                        urn_tick.push($ctx.tick());
                                        urn_event.push("delivery_inactive".to_string());
                                        urn_hero_id.push(0);
                                        urn_team_num.push(team);
                                        urn_x.push(trig_x);
                                        urn_y.push(trig_y);
                                        urn_z.push(trig_z);
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Collect neutrals (change-detected, only emit on state change) ──
                if load_neutrals {
                    for &idx in $ctx.entities().updated_indices() {
                        let Some(entity) = $ctx.entities().get(idx) else {
                            continue;
                        };
                        if entity.class_name.as_ref() != "CNPC_TrooperNeutral" {
                            continue;
                        }
                        let max_hp = entity.get_i64(ntk_max_health);
                        if max_hp == 0 {
                            continue;
                        }
                        let lifestate = entity.get_i64(ntk_lifestate);
                        let alive = lifestate == 0;
                        let [x, y, z] = entity.world_position(
                            [ntk_cell_x, ntk_cell_y, ntk_cell_z],
                            [ntk_vec_x, ntk_vec_y, ntk_vec_z],
                        );
                        let hp = entity.get_i64(ntk_health);

                        let cur = (alive, hp, max_hp, x.to_bits(), y.to_bits(), z.to_bits());
                        let changed = match nt_prev.get(&idx) {
                            None => true,
                            Some(prev) => {
                                alive != prev.0
                                    || (alive && (hp != prev.1 || max_hp != prev.2 || x.to_bits() != prev.3 || y.to_bits() != prev.4 || z.to_bits() != prev.5))
                            }
                        };
                        if changed {
                            nt_prev.insert(idx, cur);
                            if alive {
                                nt_tick.push($ctx.tick());
                                nt_team_num.push(entity.get_i64(ntk_team_num));
                                nt_health.push(hp);
                                nt_max_health.push(max_hp);
                                nt_x.push(x);
                                nt_y.push(y);
                                nt_z.push(z);
                                nt_entity_id.push(idx);
                            }
                        }
                    }
                }

            };
        }

        // ── Run the parse pass ──
        if need_events {
            self.parser
                .run_to_end_with_events_filtered(&class_filter, |ctx, events| {
                    collect_entity_data!(ctx);

                    for event in events {
                        if load_kills && event.msg_type == Msg::KEUserMsgHeroKilled as u32 {
                            raw_kill_events.push(RawEvent {
                                tick: event.tick,
                                payload: event.payload.clone(),
                            });
                        }
                        if load_damage && event.msg_type == Msg::KEUserMsgDamage as u32 {
                            raw_damage_events.push(RawEvent {
                                tick: event.tick,
                                payload: event.payload.clone(),
                            });
                        }
                        if found_game_over.is_none()
                            && event.msg_type == Msg::KEUserMsgGameOver as u32
                            && let Ok(msg) = boon_proto::proto::CCitadelUserMessageGameOver::decode(
                                event.payload.as_slice(),
                            )
                        {
                            found_game_over = Some((msg.winning_team.unwrap_or(0), event.tick));
                        }
                        if event.msg_type == Msg::KEUserMsgBannedHeroes as u32
                            && let Ok(msg) = boon_proto::proto::CCitadelUserMsgBannedHeroes::decode(
                                event.payload.as_slice(),
                            )
                        {
                            found_banned_heroes.extend(msg.banned_hero_ids);
                        }
                        // Collect FlexSlotUnlocked events (msg_type 356)
                        if load_flex_slots
                            && event.msg_type == Msg::KEUserMsgFlexSlotUnlocked as u32
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMsgFlexSlotUnlocked::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            flex_ticks.push(event.tick);
                            flex_team_nums.push(msg.team_number.unwrap_or(0));
                        }
                        // Collect ImportantAbilityUsed events (msg_type 365)
                        if load_abilities
                            && event.msg_type == Msg::KEUserMsgImportantAbilityUsed as u32
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMessageImportantAbilityUsed::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            let hero_id = boon_parser::protobuf_handle_index(msg.player)
                                .and_then(|i| entity_to_hero.get(&i).copied())
                                .unwrap_or(0);
                            ability_ticks.push(event.tick);
                            ability_hero_ids.push(hero_id);
                            ability_names.push(msg.ability_name.unwrap_or_default());
                        }
                        // Collect AbilitiesChanged events (msg_type 309) for item_purchases
                        if load_item_purchases
                            && event.msg_type == Msg::KEUserMsgAbilitiesChanged as u32
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMsgAbilitiesChanged::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            let player_slot = msg.purchaser_player_slot.unwrap_or(-1);
                            let hero_id = slot_to_hero.get(&player_slot).copied().unwrap_or(0);
                            let ability_id = msg.ability_id.unwrap_or(0);
                            let change = match msg.change.unwrap_or(-1) {
                                0 => "purchased",
                                1 => "upgraded",
                                2 => "sold",
                                3 => "swapped",
                                4 => "failure",
                                _ => "unknown",
                            };
                            ip_ticks.push(event.tick);
                            ip_hero_ids.push(hero_id);
                            ip_ability_ids.push(ability_id);
                            ip_changes.push(change.to_string());
                        }
                        // Collect ChatMsg events (msg_type 314)
                        if load_chat
                            && event.msg_type == Msg::KEUserMsgChatMsg as u32
                            && let Ok(msg) = boon_proto::proto::CCitadelUserMsgChatMsg::decode(
                                event.payload.as_slice(),
                            )
                        {
                            let player_slot = msg.player_slot.unwrap_or(-1);
                            let hero_id = slot_to_hero.get(&player_slot).copied().unwrap_or(0);
                            let chat_type = if msg.all_chat.unwrap_or(false) {
                                "all"
                            } else {
                                "team"
                            };
                            chat_ticks.push(event.tick);
                            chat_hero_ids.push(hero_id);
                            chat_texts.push(msg.text.unwrap_or_default());
                            chat_types.push(chat_type.to_string());
                        }
                        // Collect mid_boss lifecycle events
                        if load_mid_boss {
                            if event.msg_type == Msg::KEUserMsgMidBossSpawned as u32 {
                                mb_ticks.push(event.tick);
                                mb_team_nums.push(0);
                                mb_events.push("spawned".to_string());
                            }
                            if event.msg_type == Msg::KEUserMsgBossKilled as u32
                                && let Ok(msg) =
                                    boon_proto::proto::CCitadelUserMsgBossKilled::decode(
                                        event.payload.as_slice(),
                                    )
                                && msg.entity_killed_class.unwrap_or(0) == 8
                            // mid_boss entity class
                            {
                                mb_ticks.push(event.tick);
                                mb_team_nums.push(msg.objective_team.unwrap_or(0));
                                mb_events.push("killed".to_string());
                            }
                            if event.msg_type == Msg::KEUserMsgRejuvStatus as u32
                                && let Ok(msg) =
                                    boon_proto::proto::CCitadelUserMsgRejuvStatus::decode(
                                        event.payload.as_slice(),
                                    )
                            {
                                // RejuvStatus event_type enum from proto
                                let event_name = match msg.event_type.unwrap_or(0) {
                                    6 => "picked_up", // rejuv buff picked up
                                    7 => "used",      // rejuv buff consumed
                                    8 => "expired",   // rejuv buff expired
                                    _ => "unknown",
                                };
                                mb_ticks.push(event.tick);
                                mb_team_nums.push(msg.user_team.unwrap_or(0));
                                mb_events.push(event_name.to_string());
                            }
                        }
                        // Collect StreetBrawlScoring events (msg_type 362)
                        if load_street_brawl_rounds
                            && event.msg_type == Msg::KEUserMsgStreetBrawlScoring as u32
                            && let Ok(msg) =
                                boon_proto::proto::CCitadelUserMsgStreetBrawlScoring::decode(
                                    event.payload.as_slice(),
                                )
                        {
                            sbr_round_counter += 1;
                            sbr_round.push(sbr_round_counter);
                            sbr_tick.push(event.tick);
                            sbr_scoring_team.push(msg.scoring_team.unwrap_or(0));
                            sbr_amber_score.push(msg.amber_score.unwrap_or(0));
                            sbr_sapphire_score.push(msg.sapphire_score.unwrap_or(0));
                        }
                    }
                })
                .map_err(to_py_err)?;
        } else {
            self.parser
                .run_to_end_filtered(&class_filter, |ctx| {
                    collect_entity_data!(ctx);
                })
                .map_err(to_py_err)?;
        }

        // ── Store always-scanned events if found during events pass ──
        if need_events && !self.always_events_scanned {
            self.game_over = found_game_over;
            self.banned_hero_ids = Some(found_banned_heroes);
            self.always_events_scanned = true;
        }

        // ── Build and cache DataFrames ──

        if load_player_ticks {
            let df = df_from_columns(vec![
                Column::new("tick".into(), pt_tick),
                Column::new("hero_id".into(), pt_hero_id),
                Column::new("x".into(), pt_x),
                Column::new("y".into(), pt_y),
                Column::new("z".into(), pt_z),
                Column::new("pitch".into(), pt_pitch),
                Column::new("yaw".into(), pt_yaw),
                Column::new("roll".into(), pt_roll),
                Column::new("in_regen_zone".into(), pt_in_regen_zone),
                Column::new("in_item_shop".into(), pt_in_item_shop),
                Column::new("death_time".into(), pt_death_time),
                Column::new("last_spawn_time".into(), pt_last_spawn_time),
                Column::new("respawn_time".into(), pt_respawn_time),
                Column::new("health".into(), pt_health),
                Column::new("max_health".into(), pt_max_health),
                Column::new("lifestate".into(), pt_lifestate),
                Column::new("souls".into(), pt_souls),
                Column::new("spent_souls".into(), pt_spent_souls),
                Column::new("in_combat_end_time".into(), pt_combat_end),
                Column::new("in_combat_last_damage_time".into(), pt_combat_last_dmg),
                Column::new("in_combat_start_time".into(), pt_combat_start),
                Column::new("player_damage_dealt_end_time".into(), pt_dmg_dealt_end),
                Column::new(
                    "player_damage_dealt_last_damage_time".into(),
                    pt_dmg_dealt_last,
                ),
                Column::new("player_damage_dealt_start_time".into(), pt_dmg_dealt_start),
                Column::new("player_damage_taken_end_time".into(), pt_dmg_taken_end),
                Column::new(
                    "player_damage_taken_last_damage_time".into(),
                    pt_dmg_taken_last,
                ),
                Column::new("player_damage_taken_start_time".into(), pt_dmg_taken_start),
                Column::new("time_revealed_by_npc".into(), pt_time_revealed),
                Column::new("build_id".into(), pt_build_id),
                Column::new("is_alive".into(), pt_is_alive),
                Column::new("has_rebirth".into(), pt_has_rebirth),
                Column::new("has_rejuvenator".into(), pt_has_rejuvenator),
                Column::new("has_ultimate_trained".into(), pt_has_ultimate),
                Column::new("health_regen".into(), pt_health_regen),
                Column::new("ultimate_cooldown_start".into(), pt_ult_cd_start),
                Column::new("ultimate_cooldown_end".into(), pt_ult_cd_end),
                Column::new("ap_net_worth".into(), pt_ap_nw),
                Column::new("gold_net_worth".into(), pt_gold_nw),
                Column::new("denies".into(), pt_denies),
                Column::new("hero_damage".into(), pt_hero_damage),
                Column::new("hero_healing".into(), pt_hero_healing),
                Column::new("objective_damage".into(), pt_obj_damage),
                Column::new("self_healing".into(), pt_self_healing),
                Column::new("kill_streak".into(), pt_kill_streak),
                Column::new("last_hits".into(), pt_last_hits),
                Column::new("level".into(), pt_level),
                Column::new("kills".into(), pt_kills),
                Column::new("deaths".into(), pt_deaths),
                Column::new("assists".into(), pt_assists),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_player_ticks = Some(df);
        }

        if load_world_ticks {
            let df = df_from_columns(vec![
                Column::new("tick".into(), wt_tick),
                Column::new("is_paused".into(), wt_is_paused),
                Column::new("next_midboss".into(), wt_next_midboss),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_world_ticks = Some(df);
        }

        if load_kills {
            // Decode raw kill events and resolve entity indices to hero IDs
            let n = raw_kill_events.len();
            let mut kill_tick: Vec<i32> = Vec::with_capacity(n);
            let mut victim_hero_id: Vec<i64> = Vec::with_capacity(n);
            let mut attacker_hero_id: Vec<i64> = Vec::with_capacity(n);
            let mut assister_builder = ListPrimitiveChunkedBuilder::<Int64Type>::new(
                "assister_hero_ids".into(),
                n,
                4,
                DataType::Int64,
            );

            for raw in &raw_kill_events {
                let msg =
                    boon_proto::proto::CCitadelUserMsgHeroKilled::decode(raw.payload.as_slice())
                        .map_err(|e| {
                            DemoMessageError::new_err(format!(
                                "Failed to decode HeroKilled event: {e}"
                            ))
                        })?;

                kill_tick.push(raw.tick);
                victim_hero_id.push(
                    entity_to_hero
                        .get(&msg.entindex_victim.unwrap_or(-1))
                        .copied()
                        .unwrap_or(0),
                );
                attacker_hero_id.push(
                    entity_to_hero
                        .get(&msg.entindex_attacker.unwrap_or(-1))
                        .copied()
                        .unwrap_or(0),
                );

                let assister_ids: Vec<i64> = msg
                    .entindex_assisters
                    .iter()
                    .filter_map(|idx| entity_to_hero.get(idx).copied())
                    .collect();
                assister_builder.append_slice(&assister_ids);
            }

            let assister_series = assister_builder.finish().into_column();
            let df = df_from_columns(vec![
                Column::new("tick".into(), kill_tick),
                Column::new("victim_hero_id".into(), victim_hero_id),
                Column::new("attacker_hero_id".into(), attacker_hero_id),
                assister_series,
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_kills = Some(df);
        }

        if load_damage {
            // Decode raw damage events and resolve entity indices to hero IDs
            let n = raw_damage_events.len();
            let mut dmg_tick: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_damage: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_pre_damage: Vec<f32> = Vec::with_capacity(n);
            let mut dmg_victim_hero_id: Vec<i64> = Vec::with_capacity(n);
            let mut dmg_attacker_hero_id: Vec<i64> = Vec::with_capacity(n);
            let mut dmg_victim_health_new: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_hitgroup_id: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_crit_damage: Vec<f32> = Vec::with_capacity(n);
            let mut dmg_attacker_class: Vec<u32> = Vec::with_capacity(n);
            let mut dmg_victim_class: Vec<u32> = Vec::with_capacity(n);

            for raw in &raw_damage_events {
                let msg =
                    boon_proto::proto::CCitadelUserMessageDamage::decode(raw.payload.as_slice())
                        .map_err(|e| {
                            DemoMessageError::new_err(format!("Failed to decode Damage event: {e}"))
                        })?;

                dmg_tick.push(raw.tick);
                dmg_damage.push(msg.damage.unwrap_or(0));
                dmg_pre_damage.push(msg.pre_damage.unwrap_or(0.0));
                dmg_victim_hero_id.push(
                    entity_to_hero
                        .get(&msg.entindex_victim.unwrap_or(-1))
                        .copied()
                        .unwrap_or(0),
                );
                dmg_attacker_hero_id.push(
                    entity_to_hero
                        .get(&msg.entindex_attacker.unwrap_or(-1))
                        .copied()
                        .unwrap_or(0),
                );
                dmg_victim_health_new.push(msg.victim_health_new.unwrap_or(0));
                dmg_hitgroup_id.push(msg.hitgroup_id.unwrap_or(0));
                dmg_crit_damage.push(msg.crit_damage.unwrap_or(0.0));
                dmg_attacker_class.push(msg.attacker_class.unwrap_or(0));
                dmg_victim_class.push(msg.victim_class.unwrap_or(0));
            }

            let df = df_from_columns(vec![
                Column::new("tick".into(), dmg_tick),
                Column::new("damage".into(), dmg_damage),
                Column::new("pre_damage".into(), dmg_pre_damage),
                Column::new("victim_hero_id".into(), dmg_victim_hero_id),
                Column::new("attacker_hero_id".into(), dmg_attacker_hero_id),
                Column::new("victim_health_new".into(), dmg_victim_health_new),
                Column::new("hitgroup_id".into(), dmg_hitgroup_id),
                Column::new("crit_damage".into(), dmg_crit_damage),
                Column::new("attacker_class".into(), dmg_attacker_class),
                Column::new("victim_class".into(), dmg_victim_class),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_damage = Some(df);
        }

        if load_abilities {
            let df = df_from_columns(vec![
                Column::new("tick".into(), ability_ticks),
                Column::new("hero_id".into(), ability_hero_ids),
                Column::new("ability".into(), ability_names),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_abilities = Some(df);
        }

        if load_flex_slots {
            let df = df_from_columns(vec![
                Column::new("tick".into(), flex_ticks),
                Column::new("team_num".into(), flex_team_nums),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_flex_slots = Some(df);
        }

        if load_ability_upgrades {
            let df = df_from_columns(vec![
                Column::new("tick".into(), au_ticks),
                Column::new("hero_id".into(), au_hero_ids),
                Column::new("ability_id".into(), au_ability_ids),
                Column::new("tier".into(), au_tier),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_ability_upgrades = Some(df);
        }

        if load_item_purchases {
            let df = df_from_columns(vec![
                Column::new("tick".into(), ip_ticks),
                Column::new("hero_id".into(), ip_hero_ids),
                Column::new("ability_id".into(), ip_ability_ids),
                Column::new("change".into(), ip_changes),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_item_purchases = Some(df);
        }

        if load_chat {
            let df = df_from_columns(vec![
                Column::new("tick".into(), chat_ticks),
                Column::new("hero_id".into(), chat_hero_ids),
                Column::new("text".into(), chat_texts),
                Column::new("chat_type".into(), chat_types),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_chat = Some(df);
        }

        if load_objectives {
            let df = df_from_columns(vec![
                Column::new("tick".into(), obj_tick),
                Column::new("objective_type".into(), obj_type),
                Column::new("team_num".into(), obj_team_num),
                Column::new("lane".into(), obj_lane),
                Column::new("health".into(), obj_health),
                Column::new("max_health".into(), obj_max_health),
                Column::new("phase".into(), obj_phase),
                Column::new("x".into(), obj_x),
                Column::new("y".into(), obj_y),
                Column::new("z".into(), obj_z),
                Column::new("entity_id".into(), obj_entity_id),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_objectives = Some(df);
        }

        if load_mid_boss {
            let df = df_from_columns(vec![
                Column::new("tick".into(), mb_ticks),
                Column::new("team_num".into(), mb_team_nums),
                Column::new("event".into(), mb_events),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_mid_boss = Some(df);
        }

        if load_troopers {
            let df = df_from_columns(vec![
                Column::new("tick".into(), tr_tick),
                Column::new("trooper_type".into(), tr_type),
                Column::new("team_num".into(), tr_team_num),
                Column::new("lane".into(), tr_lane),
                Column::new("health".into(), tr_health),
                Column::new("max_health".into(), tr_max_health),
                Column::new("x".into(), tr_x),
                Column::new("y".into(), tr_y),
                Column::new("z".into(), tr_z),
                Column::new("entity_id".into(), tr_entity_id),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_troopers = Some(df);
        }

        if load_neutrals {
            let df = df_from_columns(vec![
                Column::new("tick".into(), nt_tick),
                Column::new("team_num".into(), nt_team_num),
                Column::new("health".into(), nt_health),
                Column::new("max_health".into(), nt_max_health),
                Column::new("x".into(), nt_x),
                Column::new("y".into(), nt_y),
                Column::new("z".into(), nt_z),
                Column::new("entity_id".into(), nt_entity_id),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_neutrals = Some(df);
        }

        if load_stat_modifier_events {
            let df = df_from_columns(vec![
                Column::new("tick".into(), sm_tick),
                Column::new("hero_id".into(), sm_hero_id),
                Column::new("stat_type".into(), sm_stat_type),
                Column::new("amount".into(), sm_amount),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_stat_modifier_events = Some(df);
        }

        if load_active_modifiers {
            let df = df_from_columns(vec![
                Column::new("tick".into(), am_tick),
                Column::new("hero_id".into(), am_hero_id),
                Column::new("event".into(), am_event),
                Column::new("modifier_id".into(), am_modifier_id),
                Column::new("ability_id".into(), am_ability_id),
                Column::new("duration".into(), am_duration),
                Column::new("caster_hero_id".into(), am_caster_hero_id),
                Column::new("stacks".into(), am_stacks),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_active_modifiers = Some(df);
        }

        if load_ability_ticks {
            let df = df_from_columns(vec![
                Column::new("tick".into(), at_tick),
                Column::new("hero_id".into(), at_hero_id),
                Column::new("ability_id".into(), at_ability_id),
                Column::new("slot".into(), at_slot),
                Column::new("cooldown_start".into(), at_cooldown_start),
                Column::new("cooldown_end".into(), at_cooldown_end),
                Column::new("remaining_charges".into(), at_remaining_charges),
                Column::new("charge_recharge_start".into(), at_charge_recharge_start),
                Column::new("charge_recharge_end".into(), at_charge_recharge_end),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_ability_ticks = Some(df);
        }

        if load_urn {
            let df = df_from_columns(vec![
                Column::new("tick".into(), urn_tick),
                Column::new("event".into(), urn_event),
                Column::new("hero_id".into(), urn_hero_id),
                Column::new("team_num".into(), urn_team_num),
                Column::new("x".into(), urn_x),
                Column::new("y".into(), urn_y),
                Column::new("z".into(), urn_z),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_urn = Some(df);
        }

        if load_street_brawl_ticks {
            let df = df_from_columns(vec![
                Column::new("tick".into(), sbt_tick),
                Column::new("round".into(), sbt_round),
                Column::new("state".into(), sbt_state),
                Column::new("amber_score".into(), sbt_amber_score),
                Column::new("sapphire_score".into(), sbt_sapphire_score),
                Column::new("buy_countdown".into(), sbt_buy_countdown),
                Column::new("next_state_time".into(), sbt_next_state_time),
                Column::new("state_start_time".into(), sbt_state_start_time),
                Column::new("non_combat_time".into(), sbt_non_combat_time),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_street_brawl_ticks = Some(df);
        }

        if load_street_brawl_rounds {
            let df = df_from_columns(vec![
                Column::new("round".into(), sbr_round),
                Column::new("tick".into(), sbr_tick),
                Column::new("scoring_team".into(), sbr_scoring_team),
                Column::new("amber_score".into(), sbr_amber_score),
                Column::new("sapphire_score".into(), sbr_sapphire_score),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_street_brawl_rounds = Some(df);
        }

        if load_rift {
            let df = df_from_columns(vec![
                Column::new("rift_num".into(), rift_num),
                Column::new("announce_tick".into(), rift_announce_tick),
                Column::new("active_tick".into(), rift_active_tick),
                Column::new("capture_tick".into(), rift_capture_tick),
                Column::new("expire_tick".into(), rift_expire_tick),
                Column::new("winning_team".into(), rift_winning_team),
                Column::new("lane".into(), rift_lane),
                Column::new("x".into(), rift_x),
                Column::new("y".into(), rift_y),
                Column::new("z".into(), rift_z),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_rift = Some(df);
        }

        Ok(())
    }

    /// Per-tick, per-player state as a Polars DataFrame.
    ///
    /// Returns a DataFrame with 48 columns covering position, health, combat
    /// timers, kills, deaths, net worth, and more for every player at every tick.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn player_ticks(&mut self) -> PyResult<PyDataFrame> {
        self.ensure_snapshots(SnapWants {
            player_ticks: true,
            ..Default::default()
        })?;
        Ok(PyDataFrame(self.cached_player_ticks.clone().unwrap()))
    }

    /// World state at every tick as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``is_paused``, ``next_midboss``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn world_ticks(&mut self) -> PyResult<PyDataFrame> {
        self.ensure_snapshots(SnapWants {
            world_ticks: true,
            ..Default::default()
        })?;
        Ok(PyDataFrame(self.cached_world_ticks.clone().unwrap()))
    }

    /// Hero kill events as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - tick: The game tick when the kill occurred
    /// - victim_hero_id: The hero ID of the killed player
    /// - attacker_hero_id: The hero ID of the attacker
    /// - assister_hero_ids: List of hero IDs of players who assisted
    ///
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn kills(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_kills.is_none() {
            self.load(vec!["kills".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_kills.clone().unwrap()))
    }

    /// Damage events as a Polars DataFrame.
    ///
    /// Returns a DataFrame with columns:
    /// - tick: The game tick when the damage occurred
    /// - damage: The damage dealt
    /// - pre_damage: The damage before mitigation
    /// - victim_hero_id: The hero ID of the victim (0 if not a hero)
    /// - attacker_hero_id: The hero ID of the attacker (0 if not a hero)
    /// - victim_health_new: The victim's health after damage
    /// - hitgroup_id: The hitgroup that was hit (use ``hitgroup_names()`` to resolve)
    /// - crit_damage: Critical damage amount
    /// - attacker_class: The attacker's entity class ID
    /// - victim_class: The victim's entity class ID
    ///
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn damage(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_damage.is_none() {
            self.load(vec!["damage".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_damage.clone().unwrap()))
    }

    /// Flex slot unlock events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``team_num``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn flex_slots(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_flex_slots.is_none() {
            self.load(vec!["flex_slots".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_flex_slots.clone().unwrap()))
    }

    /// Ability usage events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn abilities(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_abilities.is_none() {
            self.load(vec!["abilities".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_abilities.clone().unwrap()))
    }

    /// Hero ability upgrade events (skill point spending) as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id``, ``tier``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn ability_upgrades(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_ability_upgrades.is_none() {
            self.load(vec!["ability_upgrades".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_ability_upgrades.clone().unwrap()))
    }

    /// Item purchase/sell/upgrade events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id``, ``change``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn item_purchases(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_item_purchases.is_none() {
            self.load(vec!["item_purchases".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_item_purchases.clone().unwrap()))
    }

    /// Chat messages as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``text``, ``chat_type``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn chat(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_chat.is_none() {
            self.load(vec!["chat".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_chat.clone().unwrap()))
    }

    /// Objective health state changes as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``objective_type``, ``team_num``, ``lane``, ``health``, ``max_health``, ``phase``, ``x``, ``y``, ``z``, ``entity_id``.
    /// Emits a row when an objective's health or max_health changes.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn objectives(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_objectives.is_none() {
            self.load(vec!["objectives".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_objectives.clone().unwrap()))
    }

    /// Mid boss lifecycle events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``team_num``, ``event``.
    /// Events: ``"spawned"``, ``"killed"``, ``"picked_up"``, ``"used"``, ``"expired"``.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn mid_boss(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_mid_boss.is_none() {
            self.load(vec!["mid_boss".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_mid_boss.clone().unwrap()))
    }

    /// Rift lifecycle as a Polars DataFrame — one row per Rift.
    ///
    /// The Rift is a periodic king-of-the-hill objective (``Koth`` in the game
    /// files); the team that wins one gets buffed troopers in that lane.
    ///
    /// Columns: ``rift_num``, ``announce_tick``, ``active_tick``,
    /// ``capture_tick``, ``expire_tick``, ``winning_team``, ``lane``, ``x``,
    /// ``y``, ``z``. Exactly one of ``capture_tick`` / ``expire_tick`` is set
    /// per row; ``winning_team`` is null when the Rift expired uncaptured.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn rift(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_rift.is_none() {
            self.load(vec!["rift".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_rift.clone().unwrap()))
    }

    /// Per-tick alive lane trooper state as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``trooper_type``, ``team_num``, ``lane``,
    /// ``health``, ``max_health``, ``x``, ``y``, ``z``.
    ///
    /// Tracks ``CNPC_Trooper`` and ``CNPC_TrooperBoss`` only. Emits a row
    /// for every alive trooper at every tick.
    ///
    /// **Warning:** This dataset is large. It is not loaded by default.
    /// Access this property or call ``load("troopers")`` explicitly.
    #[getter]
    fn troopers(&mut self) -> PyResult<PyDataFrame> {
        self.ensure_snapshots(SnapWants {
            troopers: true,
            ..Default::default()
        })?;
        Ok(PyDataFrame(self.cached_troopers.clone().unwrap()))
    }

    /// Neutral creep state changes as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``team_num``,
    /// ``health``, ``max_health``, ``x``, ``y``, ``z``.
    ///
    /// Tracks ``CNPC_TrooperNeutral``.
    /// Only emits a row when an alive neutral's state changes (health,
    /// position), significantly reducing data volume.
    ///
    /// **Note:** Not loaded by default. Access this property or call
    /// ``load("neutrals")`` explicitly.
    #[getter]
    fn neutrals(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_neutrals.is_none() {
            self.load(vec!["neutrals".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_neutrals.clone().unwrap()))
    }

    /// Permanent stat bonus change events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``stat_type``, ``amount``.
    ///
    /// ``stat_type`` is one of: ``"health"``, ``"spirit_power"``, ``"fire_rate"``,
    /// ``"weapon_damage"``, ``"cooldown_reduction"``, ``"ammo"``.
    /// ``amount`` is the increase from this event.
    ///
    /// Emits a row whenever a stat total changes (idol/breakable pickups).
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn stat_modifier_events(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_stat_modifier_events.is_none() {
            self.load(vec!["stat_modifier_events".to_string()])?;
        }
        Ok(PyDataFrame(
            self.cached_stat_modifier_events.clone().unwrap(),
        ))
    }

    /// Active buff/debuff modifier events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``hero_id``, ``event``, ``modifier_id``, ``ability_id``,
    /// ``duration``, ``caster_hero_id``, ``stacks``.
    ///
    /// Events: ``"applied"`` when a modifier is first seen on a player,
    /// ``"changed"`` when its ``stacks`` count changes while active, and
    /// ``"removed"`` when it disappears. The ``removed`` row reports the final
    /// stack count.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn active_modifiers(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_active_modifiers.is_none() {
            self.load(vec!["active_modifiers".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_active_modifiers.clone().unwrap()))
    }

    /// Ability cooldown / charge state changes as a Polars DataFrame.
    ///
    /// Change-only: a row is emitted for an ability only on the tick its
    /// cooldown or charge state changes (not every tick), keeping the frame
    /// compact. One ability entity exists per ability the player owns, including
    /// innate movement abilities (jump, dash, slide, ...) which can be filtered
    /// out via ``slot``.
    ///
    /// Columns: ``tick``, ``hero_id``, ``ability_id`` (a ``CUtlStringToken``;
    /// resolve with ``ability_names()``), ``slot`` (``EAbilitySlots_t``),
    /// ``cooldown_start`` / ``cooldown_end`` (game time; available again at
    /// ``cooldown_end``), ``remaining_charges``, and ``charge_recharge_start`` /
    /// ``charge_recharge_end`` (recharge window of the charge currently
    /// regenerating).
    ///
    /// Not loaded by default. Access this property or call
    /// ``load("ability_ticks")`` explicitly.
    #[getter]
    fn ability_ticks(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_ability_ticks.is_none() {
            self.load(vec!["ability_ticks".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_ability_ticks.clone().unwrap()))
    }

    /// Urn (idol) lifecycle events as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``event``, ``hero_id``, ``team_num``, ``x``, ``y``, ``z``.
    ///
    /// Events: ``"picked_up"`` when a player grabs it, ``"dropped"`` when
    /// the carrier loses it, ``"returned"`` when the urn is delivered,
    /// ``"delivery_active"`` when a delivery point activates,
    /// ``"delivery_inactive"`` when a delivery point deactivates.
    ///
    /// For modifier events (``picked_up``, ``dropped``, ``returned``),
    /// ``team_num``/``x``/``y``/``z`` are 0. For delivery events, ``hero_id`` is 0.
    /// Auto-loads on first access if not already loaded via ``load()``.
    #[getter]
    fn urn(&mut self) -> PyResult<PyDataFrame> {
        if self.cached_urn.is_none() {
            self.load(vec!["urn".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_urn.clone().unwrap()))
    }

    /// Per-tick street brawl state as a Polars DataFrame.
    ///
    /// Columns: ``tick``, ``round``, ``state``, ``amber_score``,
    /// ``sapphire_score``, ``buy_countdown``, ``next_state_time``,
    /// ``state_start_time``, ``non_combat_time``.
    ///
    /// Only available for street brawl demos (game_mode=4).
    /// Auto-loads on first access if not already loaded via ``load()``.
    ///
    /// Raises:
    ///     NotStreetBrawlError: If the demo is not a street brawl game.
    #[getter]
    fn street_brawl_ticks(&mut self) -> PyResult<PyDataFrame> {
        if self.game_mode != 4 {
            return Err(NotStreetBrawlError::new_err(
                "Street brawl datasets are only available for street brawl demos (game_mode=4)",
            ));
        }
        if self.cached_street_brawl_ticks.is_none() {
            self.load(vec!["street_brawl_ticks".to_string()])?;
        }
        Ok(PyDataFrame(self.cached_street_brawl_ticks.clone().unwrap()))
    }

    /// Street brawl round scoring events as a Polars DataFrame.
    ///
    /// Columns: ``round``, ``tick``, ``scoring_team``, ``amber_score``,
    /// ``sapphire_score``.
    ///
    /// Only available for street brawl demos (game_mode=4).
    /// Auto-loads on first access if not already loaded via ``load()``.
    ///
    /// Raises:
    ///     NotStreetBrawlError: If the demo is not a street brawl game.
    #[getter]
    fn street_brawl_rounds(&mut self) -> PyResult<PyDataFrame> {
        if self.game_mode != 4 {
            return Err(NotStreetBrawlError::new_err(
                "Street brawl datasets are only available for street brawl demos (game_mode=4)",
            ));
        }
        if self.cached_street_brawl_rounds.is_none() {
            self.load(vec!["street_brawl_rounds".to_string()])?;
        }
        Ok(PyDataFrame(
            self.cached_street_brawl_rounds.clone().unwrap(),
        ))
    }

    /// The team number of the winning team.
    ///
    /// Scans for the ``k_EUserMsg_GameOver`` event on first access.
    /// Returns ``None`` if no game over event was found.
    #[getter]
    fn winning_team_num(&mut self) -> PyResult<Option<i32>> {
        self.ensure_always_events_scanned()?;
        Ok(self.game_over.map(|(team, _)| team))
    }

    /// The tick when the game ended.
    ///
    /// Scans for the ``k_EUserMsg_GameOver`` event on first access.
    /// Returns ``None`` if no game over event was found.
    #[getter]
    fn game_over_tick(&mut self) -> PyResult<Option<i32>> {
        self.ensure_always_events_scanned()?;
        Ok(self.game_over.map(|(_, tick)| tick))
    }

    /// The number of ticks of regulation play, excluding paused time.
    ///
    /// Counts active (non-paused) ticks from the start of the recording up to
    /// the ``k_EUserMsg_GameOver`` event, reflecting how much of the game was
    /// actually played rather than the full recording length (``total_ticks``,
    /// which also includes pre-game and post-match time). Scans for the
    /// game-over event and loads ``world_ticks`` on first access.
    ///
    /// Returns ``None`` if no game over event was found (e.g. an incomplete
    /// recording).
    #[getter]
    fn regulation_ticks(&mut self) -> PyResult<Option<i32>> {
        self.ensure_always_events_scanned()?;
        let Some((_, tick)) = self.game_over else {
            return Ok(None);
        };
        self.ensure_paused_ticks_built()?;
        Ok(Some(self.count_active_ticks(tick)))
    }

    /// The duration of regulation play in seconds, excluding paused time.
    ///
    /// Unlike ``total_seconds`` (the full recording length), this measures the
    /// active gameplay duration up to the ``k_EUserMsg_GameOver`` event. Equal
    /// to ``regulation_ticks / tick_rate``. Scans for the game-over event and
    /// loads ``world_ticks`` on first access.
    ///
    /// Returns ``None`` if no game over event was found.
    #[getter]
    fn regulation_seconds(&mut self) -> PyResult<Option<f32>> {
        if self.tick_rate == 0 {
            return Ok(None);
        }
        let Some(ticks) = self.regulation_ticks()? else {
            return Ok(None);
        };
        Ok(Some(ticks as f32 / self.tick_rate as f32))
    }

    /// The duration of regulation play as a formatted string (e.g. ``"32:45"``),
    /// excluding paused time.
    ///
    /// The regulation counterpart to ``total_clock_time``. Scans for the
    /// game-over event and loads ``world_ticks`` on first access.
    ///
    /// Returns ``None`` if no game over event was found.
    #[getter]
    fn regulation_clock_time(&mut self) -> PyResult<Option<String>> {
        let Some(secs) = self.regulation_seconds()? else {
            return Ok(None);
        };
        let total_seconds = secs as u32;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        Ok(Some(format!("{minutes}:{seconds:02}")))
    }

    fn __repr__(&self) -> String {
        let ticks = self.total_ticks;
        let abs_path = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        format!("Demo(path=\"{}\", ticks={ticks})", abs_path.display())
    }

    fn __str__(&self) -> String {
        let ticks = self.total_ticks;
        let abs_path = self
            .path
            .canonicalize()
            .unwrap_or_else(|_| self.path.clone());
        format!("Demo(path=\"{}\", ticks={ticks})", abs_path.display())
    }
}

impl Demo {
    /// Build the paused_ticks cache from world_ticks if not already done.
    fn ensure_paused_ticks_built(&mut self) -> PyResult<()> {
        if self.paused_ticks.is_some() {
            return Ok(());
        }
        // Ensure world_ticks is loaded
        if self.cached_world_ticks.is_none() {
            self.load(vec!["world_ticks".to_string()])?;
        }
        let wt = self.cached_world_ticks.as_ref().unwrap();
        let tick_col = wt.column("tick").unwrap();
        let paused_col = wt.column("is_paused").unwrap();
        let ticks = tick_col.i32().unwrap();
        let paused = paused_col.bool().unwrap();

        let mut paused_ticks = Vec::new();
        for i in 0..ticks.len() {
            if paused.get(i).unwrap_or(false) {
                paused_ticks.push(ticks.get(i).unwrap());
            }
        }
        self.paused_ticks = Some(paused_ticks);
        Ok(())
    }

    /// Count non-paused ticks up to the given tick.
    fn count_active_ticks(&self, tick: i32) -> i32 {
        let paused = self
            .paused_ticks
            .as_ref()
            .map(|pts| pts.partition_point(|&t| t < tick) as i32)
            .unwrap_or(0);
        (tick - paused).max(0)
    }

    /// Scan for the always-collected one-shot messages (`GameOver`,
    /// `BannedHeroes`) if not already done.
    /// Uses the lightweight events-only parser pass.
    fn ensure_always_events_scanned(&mut self) -> PyResult<()> {
        if self.always_events_scanned {
            return Ok(());
        }
        let events = self.parser.events(None).map_err(to_py_err)?;
        let mut banned: Vec<u32> = Vec::new();
        for event in &events {
            if event.msg_type == Msg::KEUserMsgGameOver as u32
                && let Ok(msg) =
                    boon_proto::proto::CCitadelUserMessageGameOver::decode(event.payload.as_slice())
            {
                self.game_over = Some((msg.winning_team.unwrap_or(0), event.tick));
            }
            if event.msg_type == Msg::KEUserMsgBannedHeroes as u32
                && let Ok(msg) =
                    boon_proto::proto::CCitadelUserMsgBannedHeroes::decode(event.payload.as_slice())
            {
                banned.extend(msg.banned_hero_ids);
            }
        }
        // Always `Some` after a full scan, so an empty list reads as "this match
        // had no bans" rather than "not looked at yet".
        self.banned_hero_ids = Some(banned);
        self.always_events_scanned = true;
        Ok(())
    }

    /// Collect the player roster from the `CCitadelPlayerController` entities
    /// present at `tick`. Players with no Steam ID (bots / empty slots) are
    /// skipped. Returns an empty frame when no controllers exist at `tick`.
    fn collect_players_at(&self, tick: i32) -> PyResult<DataFrame> {
        let ctx = self.parser.parse_to_tick(tick).map_err(to_py_err)?;

        let mut player_names: Vec<String> = Vec::new();
        let mut steam_ids: Vec<u64> = Vec::new();
        let mut hero_ids: Vec<i64> = Vec::new();
        let mut team_nums: Vec<i64> = Vec::new();
        let mut start_lanes: Vec<i64> = Vec::new();

        // Resolve field keys once for CCitadelPlayerController
        let player_serializer = ctx.serializers().get("CCitadelPlayerController");
        let key_player_name = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_iszPlayerName"));
        let key_steam_id = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_steamID"));
        let key_hero_id = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID"));
        let key_team_num = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_iTeamNum"));
        let key_start_lane = player_serializer
            .as_ref()
            .and_then(|s| s.resolve_field_key("m_nOriginalLaneAssignment"));

        // Find all CCitadelPlayerController entities
        for (_idx, entity) in ctx.entities().iter() {
            if entity.class_name.as_ref() == "CCitadelPlayerController" {
                let player_name = key_player_name
                    .and_then(|k| entity.fields.get(&k))
                    .and_then(|v| match v {
                        boon_parser::FieldValue::String(bytes) => {
                            Some(String::from_utf8_lossy(bytes).to_string())
                        }
                        _ => None,
                    })
                    .unwrap_or_default();

                let steam_id = key_steam_id
                    .and_then(|k| entity.fields.get(&k))
                    .and_then(|v| match v {
                        boon_parser::FieldValue::U64(id) => Some(*id),
                        _ => None,
                    })
                    .unwrap_or(0);

                // Skip players with no steam ID
                if steam_id == 0 {
                    continue;
                }

                let hero_id = entity.get_i64(key_hero_id);
                let team_num = entity.get_i64(key_team_num);
                // Original lane assignment (CMsgLaneColor IDs: 1=yellow, 3=green,
                // 4=blue, 6=purple, 0=none).
                let start_lane = entity.get_i64(key_start_lane);

                player_names.push(player_name);
                steam_ids.push(steam_id);
                hero_ids.push(hero_id);
                team_nums.push(team_num);
                start_lanes.push(start_lane);
            }
        }

        df_from_columns(vec![
            Column::new("player_name".into(), player_names),
            Column::new("steam_id".into(), steam_ids),
            Column::new("hero_id".into(), hero_ids),
            Column::new("team_num".into(), team_nums),
            Column::new("start_lane".into(), start_lanes),
        ])
        .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))
    }

    /// Decode the requested snapshot datasets in a SINGLE parallel pass across
    /// full-packet keyframe segments, returning `(player_ticks, world_ticks,
    /// troopers)` for whichever were requested. Player pawn/controller,
    /// game-rules, and alive-trooper state are all re-keyframed at every full
    /// packet, so the per-segment cold restarts stitch back into byte-for-byte
    /// the same frames as a serial pass. Falls back to one serial decode when
    /// parallelism is disabled (`BOON_TICK_SEGMENTS=1`) or the demo has ≤1
    /// keyframe.
    fn build_snapshots_parallel(
        &self,
        wants: SnapWants,
        pred: &TickPredicate,
    ) -> PyResult<(Option<DataFrame>, Option<DataFrame>, Option<DataFrame>)> {
        let mut classes: Vec<&str> = Vec::new();
        if wants.player_ticks {
            classes.push("CCitadelPlayerPawn");
            classes.push("CCitadelPlayerController");
        }
        if wants.world_ticks {
            classes.push("CCitadelGameRulesProxy");
        }
        if wants.troopers {
            classes.push("CNPC_Trooper");
            classes.push("CNPC_TrooperBoss");
        }
        let filter: std::collections::HashSet<&str> = classes.into_iter().collect();

        // Resolve all field keys once from the send-table serializers.
        let init = self.parser.parse_init().map_err(to_py_err)?;
        let keys = SnapKeys {
            pt: PtKeys::resolve(&init),
            wk: WkKeys::resolve(&init),
            tk: TkKeys::resolve(&init),
        };
        drop(init);

        let offsets = self.parser.full_packet_offsets().map_err(to_py_err)?;
        let n = parallel_segments().min(offsets.len().max(1));

        let merged = if n <= 1 {
            let mut cols = SegSnap::default();
            self.parser
                .decode_segment(None, i32::MAX, &filter, |ctx| {
                    if pred.matches(ctx.tick()) {
                        cols.collect_tick(ctx, &keys, wants);
                    }
                })
                .map_err(to_py_err)?;
            cols
        } else {
            let segments = segment_ranges(&offsets, n);
            let parser = &self.parser;
            let filter = &filter;
            let keys = &keys;
            let parts: std::result::Result<Vec<SegSnap>, String> = std::thread::scope(|s| {
                let handles: Vec<_> = segments
                    .iter()
                    .map(|&(start, end_tick)| {
                        s.spawn(move || -> std::result::Result<SegSnap, String> {
                            let mut cols = SegSnap::default();
                            parser
                                .decode_segment(start, end_tick, filter, |ctx| {
                                    if pred.matches(ctx.tick()) {
                                        cols.collect_tick(ctx, keys, wants);
                                    }
                                })
                                .map_err(|e| e.to_string())?;
                            Ok(cols)
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .map(|h| h.join().expect("snapshot segment thread panicked"))
                    .collect()
            });
            let mut merged = SegSnap::default();
            for part in parts.map_err(InvalidDemoError::new_err)? {
                merged.append(part);
            }
            merged
        };

        let SegSnap { pt, wt, tr } = merged;
        Ok((
            if wants.player_ticks {
                Some(pt.into_dataframe()?)
            } else {
                None
            },
            if wants.world_ticks {
                Some(wt.into_dataframe()?)
            } else {
                None
            },
            if wants.troopers {
                Some(tr.into_dataframe()?)
            } else {
                None
            },
        ))
    }

    /// Snapshot the requested datasets at a single tick via a direct
    /// `parse_to_tick` seek instead of a full-demo decode. `tick` must be a real
    /// emitted tick; otherwise empty frames are returned, matching the predicate
    /// path (which emits nothing for a tick with no data). Exact for the same
    /// reason the segmented decode is: these entities are re-keyframed at every
    /// full packet, so the seek reconstructs the identical state at `tick`.
    fn snapshot_at_tick(
        &self,
        tick: i32,
        wants: SnapWants,
    ) -> PyResult<(Option<DataFrame>, Option<DataFrame>, Option<DataFrame>)> {
        let ctx = self.parser.parse_to_tick(tick).map_err(to_py_err)?;
        let mut cols = SegSnap::default();
        if ctx.tick() == tick {
            let keys = SnapKeys {
                pt: PtKeys::resolve(&ctx),
                wk: WkKeys::resolve(&ctx),
                tk: TkKeys::resolve(&ctx),
            };
            cols.collect_tick(&ctx, &keys, wants);
        }
        let SegSnap { pt, wt, tr } = cols;
        Ok((
            if wants.player_ticks {
                Some(pt.into_dataframe()?)
            } else {
                None
            },
            if wants.world_ticks {
                Some(wt.into_dataframe()?)
            } else {
                None
            },
            if wants.troopers {
                Some(tr.into_dataframe()?)
            } else {
                None
            },
        ))
    }

    /// Populate the caches for the requested snapshot datasets that aren't
    /// already loaded, using a single parallel decode pass over the demo.
    fn ensure_snapshots(&mut self, mut wants: SnapWants) -> PyResult<()> {
        if self.cached_player_ticks.is_some() {
            wants.player_ticks = false;
        }
        if self.cached_world_ticks.is_some() {
            wants.world_ticks = false;
        }
        if self.cached_troopers.is_some() {
            wants.troopers = false;
        }
        if !wants.any() {
            return Ok(());
        }
        let (pt, wt, tr) = self.build_snapshots_parallel(wants, &TickPredicate::All)?;
        if let Some(df) = pt {
            self.cached_player_ticks = Some(df);
        }
        if let Some(df) = wt {
            self.cached_world_ticks = Some(df);
        }
        if let Some(df) = tr {
            self.cached_troopers = Some(df);
        }
        Ok(())
    }

    /// The cached DataFrame for a loaded dataset, by name (for `snapshots(events=)`).
    fn cached_frame(&self, name: &str) -> Option<&DataFrame> {
        match name {
            "abilities" => self.cached_abilities.as_ref(),
            "ability_upgrades" => self.cached_ability_upgrades.as_ref(),
            "ability_ticks" => self.cached_ability_ticks.as_ref(),
            "chat" => self.cached_chat.as_ref(),
            "mid_boss" => self.cached_mid_boss.as_ref(),
            "objectives" => self.cached_objectives.as_ref(),
            "player_ticks" => self.cached_player_ticks.as_ref(),
            "world_ticks" => self.cached_world_ticks.as_ref(),
            "kills" => self.cached_kills.as_ref(),
            "damage" => self.cached_damage.as_ref(),
            "flex_slots" => self.cached_flex_slots.as_ref(),
            "item_purchases" => self.cached_item_purchases.as_ref(),
            "troopers" => self.cached_troopers.as_ref(),
            "neutrals" => self.cached_neutrals.as_ref(),
            "stat_modifier_events" => self.cached_stat_modifier_events.as_ref(),
            "active_modifiers" => self.cached_active_modifiers.as_ref(),
            "urn" => self.cached_urn.as_ref(),
            "street_brawl_ticks" => self.cached_street_brawl_ticks.as_ref(),
            "street_brawl_rounds" => self.cached_street_brawl_rounds.as_ref(),
            "rift" => self.cached_rift.as_ref(),
            _ => None,
        }
    }

    /// Union of the `tick` columns of the given event datasets (loading each if
    /// needed), for `snapshots(events=)`.
    fn event_ticks(&mut self, names: &[String]) -> PyResult<std::collections::HashSet<i32>> {
        let mut set = std::collections::HashSet::new();
        for name in names {
            self.load(vec![name.clone()])?;
            let df = self.cached_frame(name).ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "snapshots(events=): '{name}' is not an event dataset with a tick column"
                ))
            })?;
            let tick = df.column("tick").and_then(|c| c.i32()).map_err(|_| {
                pyo3::exceptions::PyValueError::new_err(format!(
                    "snapshots(events=): '{name}' has no i32 `tick` column"
                ))
            })?;
            for t in tick.into_iter().flatten() {
                set.insert(t);
            }
        }
        Ok(set)
    }
}

/// Return a mapping of hero ID to hero name.
///
/// Returns:
///     A dict mapping hero IDs (int) to hero names (str).
#[pyfunction]
fn hero_names() -> HashMap<i64, &'static str> {
    boon_parser::all_heroes()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of team number to team name.
///
/// Returns:
///     A dict mapping team numbers (int) to team names (str).
#[pyfunction]
fn team_names() -> HashMap<i64, &'static str> {
    boon_parser::all_teams()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of ability hash ID to ability name.
///
/// Returns:
///     A dict mapping MurmurHash2 ability IDs (int) to ability names (str).
#[pyfunction]
fn ability_names() -> HashMap<u32, &'static str> {
    boon_parser::all_abilities()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of game mode ID to game mode name.
///
/// Returns:
///     A dict mapping game mode IDs (int) to game mode names (str).
#[pyfunction]
fn game_mode_names() -> HashMap<i64, &'static str> {
    boon_parser::all_game_modes()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of modifier hash ID to modifier name.
///
/// Returns:
///     A dict mapping MurmurHash2 modifier IDs (int) to modifier names (str).
#[pyfunction]
fn modifier_names() -> HashMap<u32, &'static str> {
    boon_parser::all_modifiers()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of patron phase ID to phase name.
///
/// Phases are the values of the ``CNPC_Boss_Tier3.m_ePhase`` netvar:
/// ``0=normal`` (shielded), ``1=final`` (killable), ``2=transforming``
/// (vulnerable). Non-patron objectives report ``0`` by default.
///
/// Returns:
///     A dict mapping patron phase IDs (int) to phase names (str).
#[pyfunction]
fn patron_phase_names() -> HashMap<i64, &'static str> {
    boon_parser::all_patron_phases()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of hit group ID to hit group name.
///
/// Hit group IDs are the values of Source 2's ``HitGroup_t`` enum, used by the
/// ``hitgroup_id`` column on the ``damage`` frame: ``0=generic``, ``1=head``,
/// ``2=chest``, ``3=stomach``, the limbs (``4``–``7``), ``8=neck``,
/// ``10=gear``, ``11=special``, the tier-2 / drone boss weakpoints
/// (``12``–``18``), ``19=head_no_resist``, and ``-1=invalid``.
///
/// Returns:
///     A dict mapping hit group IDs (int) to hit group names (str).
#[pyfunction]
fn hitgroup_names() -> HashMap<i64, &'static str> {
    boon_parser::all_hitgroups()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Return a mapping of life state ID to life state name.
///
/// Life state IDs are the values of Source 2's ``LifeState_t`` enum, used by
/// the ``lifestate`` column on ``player_ticks``: ``0=alive``, ``1=dying``,
/// ``2=dead``, ``3=respawnable``, ``4=respawning``.
///
/// Returns:
///     A dict mapping life state IDs (int) to life state names (str).
#[pyfunction]
fn lifestate_names() -> HashMap<i64, &'static str> {
    boon_parser::all_lifestates()
        .iter()
        .map(|&(id, name)| (id, name))
        .collect()
}

/// Python bindings for the Boon Deadlock demo parser.
#[pymodule]
fn _boon(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Demo>()?;
    m.add_function(wrap_pyfunction!(hero_names, m)?)?;
    m.add_function(wrap_pyfunction!(team_names, m)?)?;
    m.add_function(wrap_pyfunction!(ability_names, m)?)?;
    m.add_function(wrap_pyfunction!(modifier_names, m)?)?;
    m.add_function(wrap_pyfunction!(game_mode_names, m)?)?;
    m.add_function(wrap_pyfunction!(patron_phase_names, m)?)?;
    m.add_function(wrap_pyfunction!(hitgroup_names, m)?)?;
    m.add_function(wrap_pyfunction!(lifestate_names, m)?)?;
    m.add("InvalidDemoError", m.py().get_type::<InvalidDemoError>())?;
    m.add("DemoHeaderError", m.py().get_type::<DemoHeaderError>())?;
    m.add("DemoInfoError", m.py().get_type::<DemoInfoError>())?;
    m.add("DemoMessageError", m.py().get_type::<DemoMessageError>())?;
    m.add(
        "NotStreetBrawlError",
        m.py().get_type::<NotStreetBrawlError>(),
    )?;
    Ok(())
}
