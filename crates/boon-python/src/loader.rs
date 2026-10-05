use boon_parser::rift::{RIFT_COORD_SANITY, rift_lane_for};
use boon_parser::world_position;

use crate::*;

#[pymethods]
impl Demo {
    /// Load and cache one or more datasets using compatible parser passes.
    ///
    /// Already-loaded datasets are skipped. Event/entity datasets requested
    /// together share one filtered pass; snapshot datasets share one parallel
    /// keyframe-segmented pass, including when both groups are requested.
    #[pyo3(signature = (*datasets))]
    pub(crate) fn load(&mut self, py: Python<'_>, datasets: Vec<String>) -> PyResult<()> {
        let datasets: Vec<Dataset> = datasets
            .iter()
            .map(|name| name.parse())
            .collect::<PyResult<_>>()?;
        self.load_datasets(py, &datasets)
    }
}

impl Demo {
    pub(super) fn load_datasets(&mut self, py: Python<'_>, datasets: &[Dataset]) -> PyResult<()> {
        // Check game mode for street brawl datasets
        if datasets.iter().any(|dataset| dataset.is_street_brawl()) && self.game_mode != 4 {
            return Err(NotStreetBrawlError::new_err(
                "Street brawl datasets are only available for street brawl demos (game_mode=4)",
            ));
        }

        // Load small datasets together when they use the same entity class or
        // event stream. A later property access can then use the cache instead
        // of scanning the demo again.
        let requested = |dataset| datasets.contains(&dataset);
        let kill_cohort = requested(Dataset::Kills) || requested(Dataset::Abilities);
        let controller_cohort = requested(Dataset::AbilityUpgrades)
            || requested(Dataset::ItemPurchases)
            || requested(Dataset::Chat);

        let needs = |dataset| requested(dataset) && self.cached_datasets[dataset].is_none();
        let load_abilities = kill_cohort && self.cached_datasets[Dataset::Abilities].is_none();
        let load_player_ticks = needs(Dataset::PlayerTicks);
        let load_world_ticks = needs(Dataset::WorldTicks);
        let load_kills = kill_cohort && self.cached_datasets[Dataset::Kills].is_none();
        let load_damage = needs(Dataset::Damage);
        let load_flex_slots = needs(Dataset::FlexSlots);
        let load_ability_upgrades =
            controller_cohort && self.cached_datasets[Dataset::AbilityUpgrades].is_none();
        let load_item_purchases =
            controller_cohort && self.cached_datasets[Dataset::ItemPurchases].is_none();
        let load_chat = controller_cohort && self.cached_datasets[Dataset::Chat].is_none();
        let load_objectives = needs(Dataset::Objectives);
        let load_mid_boss = needs(Dataset::MidBoss);
        let load_troopers = needs(Dataset::Troopers);
        let load_neutrals = needs(Dataset::Neutrals);
        let load_breakables = needs(Dataset::Breakables);
        let load_sinners_sacrifice = needs(Dataset::SinnersSacrifice);
        let load_stat_modifier_events = needs(Dataset::StatModifierEvents);
        let load_active_modifiers = needs(Dataset::ActiveModifiers);
        let load_ability_ticks = needs(Dataset::AbilityTicks);
        let load_urn = needs(Dataset::Urn);
        let load_street_brawl_ticks = needs(Dataset::StreetBrawlTicks);
        let load_street_brawl_rounds = needs(Dataset::StreetBrawlRounds);
        let load_rift = needs(Dataset::Rift);

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
            && !load_breakables
            && !load_sinners_sacrifice
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

        // Resolve once, before advancing the parser, so acquisition failures are retryable.
        let breakable_names: HashMap<u32, String> = if load_breakables {
            py.import("boon")?
                .getattr("breakable_names")?
                .call0()?
                .extract()?
        } else {
            HashMap::new()
        };

        let modifier_types = if load_stat_modifier_events {
            let directory: PathBuf = py
                .import("boon.data")?
                .getattr("update")?
                .call0()?
                .extract()?;
            Some(
                boon_parser::StatModifierTypes::from_directory(&directory)
                    .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?,
            )
        } else {
            None
        };

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
            && !load_breakables
            && !load_sinners_sacrifice
            && !load_stat_modifier_events
            && !load_active_modifiers
            && !load_ability_ticks
            && !load_urn
            && !load_street_brawl_ticks
            && !load_street_brawl_rounds
            && !load_rift;
        if only_snapshots {
            return py.detach(|| {
                self.ensure_snapshots(SnapWants {
                    player_ticks: load_player_ticks,
                    world_ticks: load_world_ticks,
                    troopers: load_troopers,
                })
            });
        }
        // A mixed request used to collect snapshots in the serial entity/event
        // pass, forfeiting the keyframe-segmented speedup. Keep snapshots on
        // their dedicated parallel path, then let the remaining datasets share
        // the filtered serial pass below.
        if load_player_ticks || load_world_ticks || load_troopers {
            py.detach(|| {
                self.ensure_snapshots(SnapWants {
                    player_ticks: load_player_ticks,
                    world_ticks: load_world_ticks,
                    troopers: load_troopers,
                })
            })?;
        }

        let game_directory = if load_item_purchases {
            self.parser.file_header().map_err(to_py_err)?.game_directory
        } else {
            None
        };

        let need_events = load_abilities
            || load_kills
            || load_damage
            || load_sinners_sacrifice
            || load_flex_slots
            || load_item_purchases
            || load_chat
            || load_mid_boss
            || load_street_brawl_rounds;

        // Decode only the event types consumed by the requested datasets.
        // Deadlock demos can contain hundreds of thousands of particle, sound,
        // and combat events that would otherwise be allocated and immediately
        // discarded by this pass.
        let mut event_types = HashSet::new();
        if !self.always_events_scanned {
            event_types.insert(Msg::KEUserMsgGameOver as u32);
            event_types.insert(Msg::KEUserMsgBannedHeroes as u32);
        }
        if load_kills {
            event_types.insert(Msg::KEUserMsgHeroKilled as u32);
        }
        if load_damage || load_sinners_sacrifice {
            event_types.insert(Msg::KEUserMsgDamage as u32);
        }
        if load_flex_slots {
            event_types.insert(Msg::KEUserMsgFlexSlotUnlocked as u32);
        }
        if load_abilities {
            event_types.insert(Msg::KEUserMsgImportantAbilityUsed as u32);
        }
        if load_item_purchases {
            event_types.insert(Msg::KEUserMsgAbilitiesChanged as u32);
        }
        if load_chat {
            event_types.insert(Msg::KEUserMsgChatMsg as u32);
        }
        if load_mid_boss {
            event_types.insert(Msg::KEUserMsgMidBossSpawned as u32);
            event_types.insert(Msg::KEUserMsgBossKilled as u32);
            event_types.insert(Msg::KEUserMsgRejuvStatus as u32);
        }
        if load_street_brawl_rounds {
            event_types.insert(Msg::KEUserMsgStreetBrawlScoring as u32);
        }

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
        if load_street_brawl_ticks || load_rift || load_active_modifiers {
            class_names.push("CCitadelGameRulesProxy");
        }
        if load_abilities
            || load_kills
            || load_damage
            || load_sinners_sacrifice
            || load_mid_boss
            || load_active_modifiers
            || load_urn
        {
            class_names.push("CCitadelPlayerPawn");
        }
        if load_ability_upgrades
            || load_item_purchases
            || load_chat
            || load_stat_modifier_events
            || load_active_modifiers
        {
            class_names.push("CCitadelPlayerController");
        }
        if load_objectives {
            class_names.push("CNPC_Boss_Tier2");
            class_names.push("CNPC_Boss_Tier3");
            class_names.push("CNPC_BarrackBoss");
            class_names.push("CNPC_MidBoss");
            class_names.push("CCitadel_Destroyable_Building");
        }
        if load_neutrals {
            class_names.push("CNPC_TrooperNeutral");
        }
        if load_breakables {
            class_names.push("CCitadel_BreakableProp");
        }
        if load_sinners_sacrifice {
            class_names.push("CNPC_Neutral_SinnersSacrifice");
            class_names.push("CNPC_Neutral_SinnersSacrifice_Hideout");
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

        // ── Kill / damage event collection ──
        struct RawEvent<T> {
            tick: i32,
            message: Result<T, prost::DecodeError>,
        }
        let mut raw_kill_events: Vec<RawEvent<boon_proto::proto::CCitadelUserMsgHeroKilled>> =
            Vec::new();
        struct RawDamageEvent {
            event: RawEvent<boon_proto::proto::CCitadelUserMessageDamage>,
            victim_hero_id: i64,
            attacker_hero_id: i64,
        }
        let mut raw_damage_events = Vec::new();
        let mut entity_to_hero: HashMap<i32, i64> = HashMap::new();
        let mut entity_to_hero_built = false;
        let mut found_game_over: Option<(i32, i32)> = None;
        let mut found_banned_heroes: Vec<u32> = Vec::new();
        let mut flex_ticks: Vec<i32> = Vec::new();
        let mut flex_team_nums: Vec<i32> = Vec::new();
        let mut ability_ticks: Vec<i32> = Vec::new();
        let mut ability_hero_ids: Vec<i64> = Vec::new();
        let mut ability_names: Vec<String> = Vec::new();

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
        let mut ip_steam_ids: Vec<Option<u64>> = Vec::new();
        let mut ip_ability_ids: Vec<u32> = Vec::new();
        let mut ip_changes: Vec<String> = Vec::new();

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

        // ── Breakable prop terminal-leave events ──
        let mut bk_tick: Vec<i32> = Vec::new();
        let mut bk_event: Vec<&'static str> = Vec::new();
        let mut bk_entity_id: Vec<i32> = Vec::new();
        let mut bk_entity_serial: Vec<u32> = Vec::new();
        let mut bk_subclass_id: Vec<u32> = Vec::new();
        let mut bk_subclass_name: Vec<String> = Vec::new();
        let mut bk_team_num: Vec<i64> = Vec::new();
        let mut bk_x: Vec<f32> = Vec::new();
        let mut bk_y: Vec<f32> = Vec::new();
        let mut bk_z: Vec<f32> = Vec::new();
        let mut bk_live: HashMap<boon_parser::entity::EntityId, BreakableState> = HashMap::new();
        let mut bk_pending: HashMap<boon_parser::entity::EntityId, (i32, BreakableState)> =
            HashMap::new();

        // ── Sinner's Sacrifice machine lifecycle and hit events ──
        let mut sn_tick: Vec<i32> = Vec::new();
        let mut sn_event: Vec<&'static str> = Vec::new();
        let mut sn_entity_id: Vec<i32> = Vec::new();
        let mut sn_entity_serial: Vec<u32> = Vec::new();
        let mut sn_attacker_hero_id: Vec<i64> = Vec::new();
        let mut sn_damage: Vec<i32> = Vec::new();
        let mut sn_health: Vec<i64> = Vec::new();
        let mut sn_max_health: Vec<i64> = Vec::new();
        let mut sn_team_num: Vec<i64> = Vec::new();
        let mut sn_x: Vec<f32> = Vec::new();
        let mut sn_y: Vec<f32> = Vec::new();
        let mut sn_z: Vec<f32> = Vec::new();
        let mut sn_live: HashMap<boon_parser::entity::EntityId, SinnersSacrificeState> =
            HashMap::new();
        // Health changes are staged until all Damage messages for that tick
        // have been seen. Any unmatched decrease becomes an unattributed
        // fallback hit rather than disappearing from the dataset.
        let mut sn_pending_tick: Option<i32> = None;
        let mut sn_pending_hits: HashMap<
            boon_parser::entity::EntityId,
            (i32, SinnersSacrificeState),
        > = HashMap::new();
        let mut sn_damage_decode_error: Option<String> = None;

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
        let mut am_serial: Vec<u32> = Vec::new();
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
        // m_nKothScoringTeam keeps the previous winner until the next Rift opens.
        // Count a positive value only after the current Rift is contested (-1).
        // This check prevents a stale winner at the Rift open tick.
        let mut rift_seen_contested = false;
        let mut rift_spawners_prev: std::collections::HashSet<i32> =
            std::collections::HashSet::new();
        let mut rift_spawners_cur: std::collections::HashSet<i32> =
            std::collections::HashSet::new();

        // Effective (duration-aware) modifier state supplies this lifecycle frame.
        //
        // The engine re-stamps every live modifier on one tick about once a minute
        // (bookkeeping, not gameplay), bumping their last_applied_time. Effective state has
        // already expired the finite ones, so on that re-stamp it resurrects them and
        // reports each as a fresh `applied`, a phantom. On a re-stamp tick, an `applied` for
        // a modifier the hero already has (by identity, see `am_logical_seen`) is one of those
        // re-applications and is dropped, along with the rest of that entry's lifecycle
        // (`emitted` records whether a serial was actually reported, so its later
        // changed/removed stay silent too). A modifier the hero did not have is a real apply
        // and is always kept.
        //
        // Limitation: a suppressed entry stays silent until it leaves `am_prev`, so a genuine
        // re-cast of the same modifier inside a re-stamp window is masked. This trades a rare,
        // bounded miss for removing the re-stamp phantoms, and it never emits an orphan
        // changed/removed.
        struct CachedMod {
            hero_id: i64,
            modifier_id: u32,
            ability_id: u32,
            last_applied_time: f32,
            duration: f32,
            caster_hero_id: i64,
            stacks: i32,
            emitted: bool,
        }
        let mut am_prev: HashMap<u32, CachedMod> = HashMap::new();
        let mut am_state = boon_parser::EffectiveModifierState::default();
        // (hero, modifier_id) reported as `applied` this match. A re-stamp re-applies a
        // modifier a hero already has, so this identity -- not the serial -- is what marks a
        // re-application: the engine reuses a serial for some re-stamped modifiers and mints
        // a fresh one for others, and a serial-only test misses the fresh ones (letting the
        // finite-modifier-past-window applies through as phantoms).
        let mut am_logical_seen: std::collections::HashSet<(i64, u32)> =
            std::collections::HashSet::new();
        // A re-stamp re-applies modifiers the heroes already have across most of the roster
        // on one tick; real play re-applies a known modifier for at most a hero or two at a
        // time (an aura re-entering range), and a fresh cast is a modifier the hero did not
        // have. So the number of distinct heroes getting a re-application on a single tick is
        // the discriminator, and this bound sits far above normal play. It is not an applies
        // count: an early re-stamp re-applies fewer modifiers (fewer are live) yet still spans
        // the roster. On the available fixtures the most any non-re-stamp tick reaches is a
        // handful, and every re-stamp reaches most of the roster.
        const RESTAMP_MIN_REAPPLY_HEROES: usize = 6;

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
        const GOLDEN_IDOL_ABILITY: u32 = 2_521_299_219; // ability_golden_idol
        const IDOL_RETURN: u32 = 3_388_847_715; // modifier_citadel_idol_return

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

        // Pawn keys for hero resolution and urn positions
        let mut pk_hero_id: Option<u64> = None;
        let mut modifier_clock = boon_parser::ModifierClock::default();
        let mut pk_vec_x: Option<u64> = None;
        let mut pk_vec_y: Option<u64> = None;
        let mut pk_vec_z: Option<u64> = None;
        let mut pk_cell_x: Option<u64> = None;
        let mut pk_cell_y: Option<u64> = None;
        let mut pk_cell_z: Option<u64> = None;
        // Controller hero_id key (for purchases/shop_events slot→hero mapping)
        let mut ck_hero_id: Option<u64> = None;
        let mut ck_steam_id: Option<u64> = None;

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

        // Breakable prop keys
        let mut bkk_subclass_id: Option<u64> = None;
        let mut bkk_team_num: Option<u64> = None;
        let mut bkk_vec_x: Option<u64> = None;
        let mut bkk_vec_y: Option<u64> = None;
        let mut bkk_vec_z: Option<u64> = None;
        let mut bkk_cell_x: Option<u64> = None;
        let mut bkk_cell_y: Option<u64> = None;
        let mut bkk_cell_z: Option<u64> = None;

        // Sinner's Sacrifice machine keys
        let mut snk_health: Option<u64> = None;
        let mut snk_max_health: Option<u64> = None;
        let mut snk_team_num: Option<u64> = None;
        let mut snk_vec_x: Option<u64> = None;
        let mut snk_vec_y: Option<u64> = None;
        let mut snk_vec_z: Option<u64> = None;
        let mut snk_cell_x: Option<u64> = None;
        let mut snk_cell_y: Option<u64> = None;
        let mut snk_cell_z: Option<u64> = None;

        // StatViewerModifierValues keys for indices 0..20.
        let mut smk_count: Option<u64> = None;
        let mut smk_keys = [StatViewerKeys::default(); STAT_VIEWER_SLOTS];

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

        macro_rules! push_sinner_event {
            ($tick:expr, $event:expr, $id:expr, $attacker:expr, $damage:expr, $state:expr) => {{
                let id = $id;
                let state = $state;
                sn_tick.push($tick);
                sn_event.push($event);
                sn_entity_id.push(id.index);
                sn_entity_serial.push(id.serial);
                sn_attacker_hero_id.push($attacker);
                sn_damage.push($damage);
                sn_health.push(state.health);
                sn_max_health.push(state.max_health);
                sn_team_num.push(state.team_num);
                sn_x.push(state.x);
                sn_y.push(state.y);
                sn_z.push(state.z);
            }};
        }

        macro_rules! flush_pending_sinner_hits {
            () => {{
                if let Some(tick) = sn_pending_tick.take() {
                    let mut pending: Vec<_> = sn_pending_hits.drain().collect();
                    pending.sort_unstable_by_key(|(id, _)| (id.index, id.serial));
                    for (id, (damage, state)) in pending {
                        push_sinner_event!(tick, "hit", id, 0, damage, state);
                    }
                }
            }};
        }

        macro_rules! collect_entity_data {
            ($ctx:expr) => {
                if load_sinners_sacrifice
                    && sn_pending_tick.is_some()
                    && sn_pending_tick != Some($ctx.tick())
                {
                    flush_pending_sinner_hits!();
                }

                if !keys_resolved {
                    if load_abilities || load_kills || load_damage || load_sinners_sacrifice || load_active_modifiers || load_urn || load_ability_ticks {
                        if let Some(s) = $ctx.serializers().get("CCitadelPlayerPawn") {
                            pk_hero_id = s.resolve_field_key(
                                "m_CCitadelHeroComponent.m_spawnedHero.m_nHeroID",
                            );
                            if load_urn {
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
                            }
                        }
                    }
                    if load_item_purchases || load_chat {
                        if let Some(s) = $ctx.serializers().get("CCitadelPlayerController") {
                            ck_hero_id =
                                s.resolve_field_key("m_PlayerDataGlobal.m_nHeroID");
                            ck_steam_id = s.resolve_field_key("m_steamID");
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
                            smk_count = s.resolve_field_key(
                                "m_PlayerDataGlobal.m_vecStatViewerModifierValues",
                            );
                            smk_keys = resolve_stat_viewer_keys(Some(s));
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
                    if load_breakables {
                        if let Some(s) = $ctx.serializers().get("CCitadel_BreakableProp") {
                            bkk_subclass_id = s.resolve_field_key("m_nSubclassID");
                            bkk_team_num = s.resolve_field_key("m_iTeamNum");
                            bkk_vec_x = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                            );
                            bkk_vec_y = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                            );
                            bkk_vec_z = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                            );
                            bkk_cell_x = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX",
                            );
                            bkk_cell_y = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY",
                            );
                            bkk_cell_z = s.resolve_field_key(
                                "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ",
                            );
                        }
                    }
                    if load_sinners_sacrifice {
                        for sn_class in &[
                            "CNPC_Neutral_SinnersSacrifice",
                            "CNPC_Neutral_SinnersSacrifice_Hideout",
                        ] {
                            if let Some(s) = $ctx.serializers().get(*sn_class) {
                                snk_health = s.resolve_field_key("m_iHealth");
                                snk_max_health = s.resolve_field_key("m_iMaxHealth");
                                snk_team_num = s.resolve_field_key("m_iTeamNum");
                                snk_vec_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecX",
                                );
                                snk_vec_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecY",
                                );
                                snk_vec_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_vecZ",
                                );
                                snk_cell_x = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellX",
                                );
                                snk_cell_y = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellY",
                                );
                                snk_cell_z = s.resolve_field_key(
                                    "CBodyComponent.m_skeletonInstance.m_vecOrigin.m_cellZ",
                                );
                                break;
                            }
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
                    if load_active_modifiers {
                        modifier_clock = boon_parser::ModifierClock::resolve($ctx);
                    }
                    keys_resolved = true;
                }

                // ── Collect street_brawl_ticks ──
                if load_street_brawl_ticks {
                    if let Some((_, entity)) = $ctx
                        .entities()
                        .iter()
                        .find(|(_, e)| e.class_name.as_ref() == "CCitadelGameRulesProxy")
                    {
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
                //
                // A pawn's m_spawnedHero.m_nHeroID reads a pre-lock placeholder during the
                // intro (on some demos Bebop shows as hero 66, Billy as 27) and only settles
                // to the real hero once the match starts. Building this map once from that
                // early frame froze the placeholder, so every later modifier / kill on those
                // pawns attributed to the wrong hero and never appeared under the real one.
                // Do the full scan once, then refresh the pawns touched each tick so the
                // settled hero id wins.
                if load_abilities || load_kills || load_damage || load_sinners_sacrifice || load_mid_boss || load_active_modifiers || load_urn || load_ability_ticks {
                    if !entity_to_hero_built {
                        for (idx, entity) in $ctx.entities().iter() {
                            if entity.class_name.as_ref() == "CCitadelPlayerPawn" {
                                let hid = entity.get_i64(pk_hero_id);
                                if hid != 0 {
                                    entity_to_hero.insert(idx, hid);
                                }
                            }
                        }
                        entity_to_hero_built = true;
                    } else {
                        for &idx in $ctx.entities().updated_indices() {
                            let Some(entity) = $ctx.entities().get(idx) else {
                                continue;
                            };
                            if entity.class_name.as_ref() == "CCitadelPlayerPawn" {
                                let hid = entity.get_i64(pk_hero_id);
                                if hid != 0 {
                                    entity_to_hero.insert(idx, hid);
                                }
                            }
                        }
                    }
                }

                // ── Collect ability_upgrades (entity change detection) ──
                if load_ability_upgrades {
                    for &idx in $ctx.entities().updated_indices() {
                        let Some(entity) = $ctx.entities().get(idx) else {
                            continue;
                        };
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
                            let [ox, oy, oz] = world_position(entity, cell_keys, offset_keys);
                            obj_x.push(ox);
                            obj_y.push(oy);
                            obj_z.push(oz);
                            obj_entity_id.push(idx);
                        }

                    }
                }

                // ── Collect stat_modifiers (event-based change detection) ──
                if load_stat_modifier_events {
                    for &idx in $ctx.entities().updated_indices() {
                        let Some(entity) = $ctx.entities().get(idx) else {
                            continue;
                        };
                        if entity.class_name.as_ref() != "CCitadelPlayerController" {
                            continue;
                        }
                        let hero_id = entity.get_i64(ck_hero_id);
                        if hero_id == 0 {
                            continue;
                        }

                        // Sum values by eValType
                        let mut by_type: HashMap<u32, f32> = HashMap::new();
                        let count = entity.get_i64(smk_count).clamp(0, STAT_VIEWER_SLOTS as i64)
                            as usize;
                        for keys in smk_keys.iter().take(count) {
                            let vt_val = entity.get_u32(keys.value_type);
                            if vt_val == 0 {
                                continue;
                            }
                            let fl_val = entity.get_f32(keys.value);
                            *by_type.entry(vt_val).or_insert(0.0) += fl_val;
                        }

                        // Emit events for changed stat types
                        for (vt_val, total) in &by_type {
                            let Some(decoded) =
                                modifier_types.as_ref().expect("stat types loaded").decode(*vt_val)
                            else {
                                continue;
                            };
                            let key = (idx, *vt_val);
                            let prev = sm_prev.get(&key).copied().unwrap_or(0.0);
                            if (*total - prev).abs() > f32::EPSILON {
                                sm_prev.insert(key, *total);
                                sm_tick.push($ctx.tick());
                                sm_hero_id.push(hero_id);
                                sm_stat_type.push(decoded.kind.name().to_string());
                                sm_amount.push((*total - prev) * decoded.value_scale);
                            }
                        }
                    }
                }

                // ── Collect active_modifiers (effective, duration-aware state) ──
                if load_active_modifiers {
                    let game_time = modifier_clock.game_time($ctx);
                    let changes = am_state.update($ctx, game_time);

                    // Flag a re-stamp tick: many heroes get a modifier they already have
                    // re-applied at once. `am_logical_seen` still holds the state from before
                    // this tick, so an `applied` whose (hero, modifier_id) is in it is a
                    // re-application, whatever serial the re-stamp gave it.
                    let mut restamp_reapplied_heroes: std::collections::HashSet<i64> =
                        std::collections::HashSet::new();
                    for change in &changes {
                        if change.kind == boon_parser::ModifierChangeKind::Applied {
                            let hero = boon_parser::protobuf_handle_index(change.entry.parent)
                                .and_then(|index| entity_to_hero.get(&index).copied())
                                .unwrap_or(0);
                            let modifier_id = change.entry.modifier_subclass.unwrap_or(0);
                            if hero != 0 && am_logical_seen.contains(&(hero, modifier_id)) {
                                restamp_reapplied_heroes.insert(hero);
                            }
                        }
                    }
                    let is_restamp =
                        restamp_reapplied_heroes.len() >= RESTAMP_MIN_REAPPLY_HEROES;

                    for change in changes {
                        let serial = change.serial;
                        if change.kind == boon_parser::ModifierChangeKind::Removed {
                            if let Some(cached) = am_prev.remove(&serial) {
                                if cached.emitted {
                                    am_tick.push($ctx.tick());
                                    am_hero_id.push(cached.hero_id);
                                    am_event.push("removed".to_string());
                                    am_serial.push(serial);
                                    am_modifier_id.push(cached.modifier_id);
                                    am_ability_id.push(cached.ability_id);
                                    am_duration.push(cached.duration);
                                    am_caster_hero_id.push(cached.caster_hero_id);
                                    am_stacks.push(cached.stacks);
                                }
                            }
                            continue;
                        }

                        let modifier = change.entry;
                        let hero_id = boon_parser::protobuf_handle_index(modifier.parent)
                            .and_then(|index| entity_to_hero.get(&index).copied())
                            .or_else(|| am_prev.get(&serial).map(|cached| cached.hero_id))
                            .unwrap_or(0);
                        if hero_id == 0 {
                            continue;
                        }

                        let modifier_id = modifier.modifier_subclass.unwrap_or(0);
                        let ability_id = modifier.ability_subclass.unwrap_or(0);
                        let duration = modifier.duration.unwrap_or(-1.0);
                        let last_applied_time =
                            modifier.last_applied_time.unwrap_or(-1.0);
                        let caster_hero_id =
                            boon_parser::protobuf_handle_index(modifier.caster)
                                .and_then(|index| entity_to_hero.get(&index).copied())
                                .unwrap_or(0);
                        let stacks = modifier.stack_count.unwrap_or(0);

                        match am_prev.entry(serial) {
                            std::collections::hash_map::Entry::Vacant(entry) => {
                                // On a re-stamp tick, an `applied` for a modifier the hero
                                // already has is a re-application (whatever its serial), so
                                // drop it; `emitted` keeps its changed/removed silent too. A
                                // modifier the hero did not have is real and is kept.
                                let logical = (hero_id, modifier_id);
                                let emitted = !(is_restamp && am_logical_seen.contains(&logical));
                                if emitted {
                                    am_logical_seen.insert(logical);
                                    am_tick.push($ctx.tick());
                                    am_hero_id.push(hero_id);
                                    am_event.push("applied".to_string());
                                    am_serial.push(serial);
                                    am_modifier_id.push(modifier_id);
                                    am_ability_id.push(ability_id);
                                    am_duration.push(duration);
                                    am_caster_hero_id.push(caster_hero_id);
                                    am_stacks.push(stacks);
                                }
                                entry.insert(CachedMod {
                                    hero_id,
                                    modifier_id,
                                    ability_id,
                                    last_applied_time,
                                    duration,
                                    caster_hero_id,
                                    stacks,
                                    emitted,
                                });
                            }
                            std::collections::hash_map::Entry::Occupied(mut entry) => {
                                // A serial whose apply was dropped as a phantom stays silent
                                // for the rest of its lifecycle.
                                if !entry.get().emitted {
                                    continue;
                                }
                                let cached = entry.get_mut();
                                let caster_hero_id = if caster_hero_id == 0 {
                                    cached.caster_hero_id
                                } else {
                                    caster_hero_id
                                };
                                let hero_changed = hero_id != cached.hero_id;
                                let changed = hero_changed
                                    || modifier_id != cached.modifier_id
                                    || ability_id != cached.ability_id
                                    || stacks != cached.stacks
                                    || caster_hero_id != cached.caster_hero_id
                                    || duration.to_bits() != cached.duration.to_bits()
                                    || last_applied_time.to_bits()
                                        != cached.last_applied_time.to_bits();
                                if changed {
                                    // A retained serial can follow a hero swap. Close the
                                    // old hero's lifetime before opening the new one.
                                    if hero_changed {
                                        am_tick.push($ctx.tick());
                                        am_hero_id.push(cached.hero_id);
                                        am_event.push("removed".to_string());
                                        am_serial.push(serial);
                                        am_modifier_id.push(cached.modifier_id);
                                        am_ability_id.push(cached.ability_id);
                                        am_duration.push(cached.duration);
                                        am_caster_hero_id.push(cached.caster_hero_id);
                                        am_stacks.push(cached.stacks);
                                        am_logical_seen.insert((hero_id, modifier_id));
                                    }
                                    am_tick.push($ctx.tick());
                                    am_hero_id.push(hero_id);
                                    am_event.push(if hero_changed { "applied" } else { "changed" }.to_string());
                                    am_serial.push(serial);
                                    am_modifier_id.push(modifier_id);
                                    am_ability_id.push(ability_id);
                                    am_duration.push(duration);
                                    am_caster_hero_id.push(caster_hero_id);
                                    am_stacks.push(stacks);
                                    *cached = CachedMod {
                                        hero_id,
                                        modifier_id,
                                        ability_id,
                                        last_applied_time,
                                        duration,
                                        caster_hero_id,
                                        stacks,
                                        emitted: true,
                                    };
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
                                            |e| world_position(
                                                e,
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
                                |e| world_position(
                                    e,
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
                                        |e| world_position(
                                            e,
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
                            let [trig_x, trig_y, trig_z] = world_position(
                                entity,
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
                        let [x, y, z] = world_position(
                            entity,
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

                // ── Collect breakable map-prop destruction events ──
                if load_breakables {
                    let changes = $ctx.entities().entity_changes();

                    // A genuine reactivation cancels an earlier PVS-leave candidate.
                    for change in changes {
                        if change.class_name.as_ref() == "CCitadel_BreakableProp"
                            && change.kind
                                == boon_parser::entity::EntityChangeKind::Reactivated
                        {
                            bk_pending.remove(&change.id());
                        }
                    }

                    // updated_indices also contains the sign-on baseline in the
                    // first callback, while entity_changes intentionally does not.
                    // This seeds last-known state without a full entity scan.
                    for &idx in $ctx.entities().updated_indices() {
                        let Some(entity) = $ctx.entities().get(idx) else {
                            continue;
                        };
                        if entity.class_name.as_ref() != "CCitadel_BreakableProp" {
                            continue;
                        }
                        let id = boon_parser::entity::EntityId::new(idx, entity.serial);
                        if bk_pending.contains_key(&id) {
                            continue;
                        }
                        let [x, y, z] = world_position(
                            entity,
                            [bkk_cell_x, bkk_cell_y, bkk_cell_z],
                            [bkk_vec_x, bkk_vec_y, bkk_vec_z],
                        );
                        bk_live.insert(
                            id,
                            BreakableState {
                                subclass_id: entity.get_u32(bkk_subclass_id),
                                team_num: entity.get_i64(bkk_team_num),
                                x,
                                y,
                                z,
                            },
                        );
                    }

                    // pbdems2 reports slot replacement (including full-packet
                    // rebuilds) as Deleted(old) followed by Created(new).
                    for change in changes {
                        if change.class_name.as_ref() != "CCitadel_BreakableProp" {
                            continue;
                        }
                        match change.kind {
                            boon_parser::entity::EntityChangeKind::LeftPvs => {
                                if let Some(state) = bk_live.remove(&change.id()) {
                                    bk_pending
                                        .entry(change.id())
                                        .or_insert(($ctx.tick(), state));
                                }
                            }
                            boon_parser::entity::EntityChangeKind::Deleted => {
                                if let Some(current) = $ctx.entities().get(change.index) {
                                    let current_id =
                                        boon_parser::entity::EntityId::new(
                                            current.index,
                                            current.serial,
                                        );
                                    if current_id != change.id() {
                                        bk_live.remove(&change.id());
                                    }
                                    continue;
                                }
                                if let Some(state) = bk_live.remove(&change.id()) {
                                    bk_pending
                                        .entry(change.id())
                                        .or_insert(($ctx.tick(), state));
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // ── Collect Sinner's Sacrifice lifecycle and health state ──
                if load_sinners_sacrifice {
                    // Drop stale identities on genuine deletion or slot reuse,
                    // but retain the same identity across a full-packet rebuild.
                    // Left-PVS state is retained so a later reactivation can be
                    // recognized as the same machine rather than a new spawn.
                    for change in $ctx.entities().entity_changes() {
                        if !is_sinners_sacrifice(change.class_name.as_ref())
                            || change.kind != boon_parser::entity::EntityChangeKind::Deleted
                        {
                            continue;
                        }
                        let current_id = $ctx.entities().get(change.index).map(|entity| {
                            boon_parser::entity::EntityId::new(entity.index, entity.serial)
                        });
                        if current_id != Some(change.id()) {
                            sn_live.remove(&change.id());
                            sn_pending_hits.remove(&change.id());
                        }
                    }

                    for &idx in $ctx.entities().updated_indices() {
                        let Some(entity) = $ctx.entities().get(idx) else {
                            continue;
                        };
                        if !is_sinners_sacrifice(entity.class_name.as_ref()) {
                            continue;
                        }

                        let health = entity.get_i64(snk_health);
                        let max_health = entity.get_i64(snk_max_health);
                        // Inactive machines can briefly omit these fields. A
                        // real machine bottoms out at one health, so zero here
                        // is missing state rather than a hit or destruction.
                        if health <= 0 || max_health <= 0 {
                            continue;
                        }

                        let id = boon_parser::entity::EntityId::new(idx, entity.serial);
                        let [x, y, z] = world_position(
                            entity,
                            [snk_cell_x, snk_cell_y, snk_cell_z],
                            [snk_vec_x, snk_vec_y, snk_vec_z],
                        );
                        let state = SinnersSacrificeState {
                            health,
                            max_health,
                            team_num: entity.get_i64(snk_team_num),
                            x,
                            y,
                            z,
                        };

                        match sn_live.insert(id, state) {
                            None => {
                                push_sinner_event!($ctx.tick(), "spawned", id, 0, 0, state);
                            }
                            Some(previous) if health > previous.health => {
                                push_sinner_event!($ctx.tick(), "reset", id, 0, 0, state);
                            }
                            Some(previous) if health < previous.health => {
                                sn_pending_tick = Some($ctx.tick());
                                let health_lost = i32::try_from(previous.health - health)
                                    .unwrap_or(i32::MAX);
                                sn_pending_hits
                                    .entry(id)
                                    .and_modify(|(damage, pending_state)| {
                                        *damage = damage.saturating_add(health_lost);
                                        *pending_state = state;
                                    })
                                    .or_insert((health_lost, state));
                            }
                            _ => {}
                        }
                    }
                }

            };
        }

        // ── Run the parse pass ──
        if need_events {
            py.detach(|| {
                self.parser.run_to_end_with_event_types_filtered(
                    &class_filter,
                    &event_types,
                    |ctx, events| {
                        collect_entity_data!(ctx);

                        for event in events {
                            if load_kills && event.msg_type == Msg::KEUserMsgHeroKilled as u32 {
                                raw_kill_events.push(RawEvent {
                                    tick: event.tick,
                                    message: boon_proto::proto::CCitadelUserMsgHeroKilled::decode(
                                        event.payload.as_slice(),
                                    ),
                                });
                            }
                            if (load_damage || load_sinners_sacrifice)
                                && event.msg_type == Msg::KEUserMsgDamage as u32
                            {
                                // Decode once even when both the generic damage
                                // and Sinner's Sacrifice datasets are requested.
                                let message = boon_proto::proto::CCitadelUserMessageDamage::decode(
                                    event.payload.as_slice(),
                                );

                                if load_sinners_sacrifice {
                                    match &message {
                                        Ok(msg) => {
                                            let victim_index = msg.entindex_victim.unwrap_or(-1);
                                            let current_id = ctx
                                                .entities()
                                                .get(victim_index)
                                                .filter(|entity| {
                                                    is_sinners_sacrifice(entity.class_name.as_ref())
                                                })
                                                .map(|entity| {
                                                    boon_parser::entity::EntityId::new(
                                                        entity.index,
                                                        entity.serial,
                                                    )
                                                });
                                            let victim_id = current_id.or_else(|| {
                                                sn_live
                                                    .keys()
                                                    .find(|id| id.index == victim_index)
                                                    .copied()
                                            });

                                            if let Some(id) = victim_id
                                                && let Some(state) = sn_live.get(&id).copied()
                                            {
                                                let attacker_hero_id = entity_to_hero
                                                    .get(&msg.entindex_attacker.unwrap_or(-1))
                                                    .copied()
                                                    .unwrap_or(0);
                                                push_sinner_event!(
                                                    event.tick,
                                                    "hit",
                                                    id,
                                                    attacker_hero_id,
                                                    msg.damage.unwrap_or(0),
                                                    state
                                                );
                                                if sn_pending_tick == Some(event.tick) {
                                                    sn_pending_hits.remove(&id);
                                                }
                                            }
                                        }
                                        Err(error) => {
                                            if sn_damage_decode_error.is_none() {
                                                sn_damage_decode_error = Some(error.to_string());
                                            }
                                        }
                                    }
                                }

                                if load_damage {
                                    // Capture hero identity at the event tick, before a
                                    // later hero swap or entity-index reuse can change it.
                                    let hero_id = |index: Option<i32>| {
                                        index
                                            .and_then(|index| ctx.entities().get(index))
                                            .filter(|entity| {
                                                entity.class_name.as_ref() == "CCitadelPlayerPawn"
                                            })
                                            .map_or(0, |entity| entity.get_i64(pk_hero_id))
                                    };
                                    let (victim_hero_id, attacker_hero_id) =
                                        message.as_ref().map_or((0, 0), |msg| {
                                            (
                                                hero_id(msg.entindex_victim),
                                                hero_id(msg.entindex_attacker),
                                            )
                                        });
                                    raw_damage_events.push(RawDamageEvent {
                                        event: RawEvent {
                                            tick: event.tick,
                                            message,
                                        },
                                        victim_hero_id,
                                        attacker_hero_id,
                                    });
                                }
                            }
                            if found_game_over.is_none()
                                && event.msg_type == Msg::KEUserMsgGameOver as u32
                                && let Ok(msg) =
                                    boon_proto::proto::CCitadelUserMessageGameOver::decode(
                                        event.payload.as_slice(),
                                    )
                            {
                                found_game_over = Some((msg.winning_team.unwrap_or(0), event.tick));
                            }
                            if event.msg_type == Msg::KEUserMsgBannedHeroes as u32
                                && let Ok(msg) =
                                    boon_proto::proto::CCitadelUserMsgBannedHeroes::decode(
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
                                let hero_id = boon_parser::hero_id_for_player_slot(
                                    ctx.entities(),
                                    msg.purchaser_player_slot,
                                    ck_hero_id,
                                );
                                let ability_id = msg.ability_id.unwrap_or(0);
                                let change = boon_parser::demo::ability_change_name(
                                    msg.change,
                                    game_directory.as_deref(),
                                );
                                let steam_id = msg
                                    .purchaser_player_slot
                                    .filter(|&slot| slot >= 0)
                                    .and_then(|slot| slot.checked_add(1))
                                    .and_then(|index| ctx.entities().get(index))
                                    .filter(|entity| {
                                        entity.class_name.as_ref() == "CCitadelPlayerController"
                                    })
                                    .and_then(|entity| {
                                        ck_steam_id.and_then(|key| entity.fields.get(&key))
                                    })
                                    .and_then(|value| match value {
                                        boon_parser::FieldValue::U64(id) => Some(*id),
                                        _ => None,
                                    })
                                    .filter(|&id| id != 0);
                                ip_ticks.push(event.tick);
                                ip_hero_ids.push(hero_id);
                                ip_steam_ids.push(steam_id);
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
                                let hero_id = boon_parser::hero_id_for_player_slot(
                                    ctx.entities(),
                                    msg.player_slot,
                                    ck_hero_id,
                                );
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
                    },
                )
            })
            .map_err(to_py_err)?;
        } else {
            py.detach(|| {
                self.parser.run_to_end_filtered(&class_filter, |ctx| {
                    collect_entity_data!(ctx);
                })
            })
            .map_err(to_py_err)?;
        }

        if load_sinners_sacrifice {
            flush_pending_sinner_hits!();
            if let Some(error) = sn_damage_decode_error {
                return Err(DemoMessageError::new_err(format!(
                    "Failed to decode Damage event: {error}"
                )));
            }
        }

        // ── Store always-scanned events if found during events pass ──
        if need_events && !self.always_events_scanned {
            self.game_over = found_game_over;
            self.banned_hero_ids = Some(found_banned_heroes);
            self.always_events_scanned = true;
        }

        if load_breakables {
            // Only terminal leaves remain pending after the full demo has been
            // parsed. Sort by event time so HashMap iteration cannot affect the
            // DataFrame's deterministic row order.
            let mut breaks: Vec<_> = bk_pending.into_iter().collect();
            breaks.sort_unstable_by_key(|(id, (tick, _))| (*tick, id.index, id.serial));
            for (id, (tick, state)) in breaks {
                bk_tick.push(tick);
                bk_event.push("broken");
                bk_entity_id.push(id.index);
                bk_entity_serial.push(id.serial);
                bk_subclass_id.push(state.subclass_id);
                bk_subclass_name.push(
                    breakable_names
                        .get(&state.subclass_id)
                        .map_or("BREAKABLE_NOT_FOUND", String::as_str)
                        .to_owned(),
                );
                bk_team_num.push(state.team_num);
                bk_x.push(state.x);
                bk_y.push(state.y);
                bk_z.push(state.z);
            }
        }

        // ── Build and cache DataFrames ──

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
                let msg = raw.message.as_ref().map_err(|e| {
                    DemoMessageError::new_err(format!("Failed to decode HeroKilled event: {e}"))
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
            self.cached_datasets[Dataset::Kills] = Some(df);
        }

        if load_damage {
            // Build damage rows with the hero identities captured at each event.
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
            let mut dmg_victim_entity_id: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_ability_id: Vec<u32> = Vec::with_capacity(n);
            let mut dmg_type: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_citadel_type: Vec<i32> = Vec::with_capacity(n);
            let mut dmg_flags: Vec<u64> = Vec::with_capacity(n);
            let mut dmg_is_melee: Vec<bool> = Vec::with_capacity(n);
            let mut dmg_melee_type: Vec<Option<&'static str>> = Vec::with_capacity(n);
            let mut dmg_absorbed: Vec<Option<f32>> = Vec::with_capacity(n);
            let mut dmg_shield_new: Vec<Option<i32>> = Vec::with_capacity(n);
            let mut dmg_shield_max: Vec<Option<i32>> = Vec::with_capacity(n);
            let mut dmg_server_tick: Vec<Option<i32>> = Vec::with_capacity(n);

            for raw in &raw_damage_events {
                let msg = raw.event.message.as_ref().map_err(|e| {
                    DemoMessageError::new_err(format!("Failed to decode Damage event: {e}"))
                })?;

                dmg_tick.push(raw.event.tick);
                dmg_damage.push(msg.damage.unwrap_or(0));
                dmg_pre_damage.push(msg.pre_damage.unwrap_or(0.0));
                dmg_victim_entity_id.push(msg.entindex_victim.unwrap_or(-1));
                dmg_victim_hero_id.push(raw.victim_hero_id);
                dmg_attacker_hero_id.push(raw.attacker_hero_id);
                dmg_victim_health_new.push(msg.victim_health_new.unwrap_or(0));
                dmg_hitgroup_id.push(msg.hitgroup_id.unwrap_or(0));
                dmg_crit_damage.push(msg.crit_damage.unwrap_or(0.0));
                dmg_attacker_class.push(msg.attacker_class.unwrap_or(0));
                dmg_victim_class.push(msg.victim_class.unwrap_or(0));
                let ability_id = msg.ability_id.unwrap_or(0);
                let citadel_type = msg.citadel_type.unwrap_or(0);
                let damage_flags = msg.flags.unwrap_or(0);
                let (is_melee, melee_type) = classify_melee_damage(citadel_type, damage_flags);
                dmg_ability_id.push(ability_id);
                dmg_type.push(msg.r#type.unwrap_or(0));
                dmg_citadel_type.push(citadel_type);
                dmg_flags.push(damage_flags);
                dmg_is_melee.push(is_melee);
                dmg_melee_type.push(melee_type);
                dmg_absorbed.push(msg.damage_absorbed);
                dmg_shield_new.push(msg.victim_shield_new);
                dmg_shield_max.push(msg.victim_shield_max);
                dmg_server_tick.push(msg.server_tick);
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
                Column::new("victim_entity_id".into(), dmg_victim_entity_id),
                Column::new("ability_id".into(), dmg_ability_id),
                Column::new("damage_type".into(), dmg_type),
                Column::new("citadel_type".into(), dmg_citadel_type),
                Column::new("damage_flags".into(), dmg_flags),
                Column::new("is_melee".into(), dmg_is_melee),
                Column::new("melee_type".into(), dmg_melee_type),
                Column::new("damage_absorbed".into(), dmg_absorbed),
                Column::new("victim_shield_new".into(), dmg_shield_new),
                Column::new("victim_shield_max".into(), dmg_shield_max),
                Column::new("server_tick".into(), dmg_server_tick),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::Damage] = Some(df);
        }

        if load_abilities {
            let df = df_from_columns(vec![
                Column::new("tick".into(), ability_ticks),
                Column::new("hero_id".into(), ability_hero_ids),
                Column::new("ability".into(), ability_names),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::Abilities] = Some(df);
        }

        if load_flex_slots {
            let df = df_from_columns(vec![
                Column::new("tick".into(), flex_ticks),
                Column::new("team_num".into(), flex_team_nums),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::FlexSlots] = Some(df);
        }

        if load_ability_upgrades {
            let df = df_from_columns(vec![
                Column::new("tick".into(), au_ticks),
                Column::new("hero_id".into(), au_hero_ids),
                Column::new("ability_id".into(), au_ability_ids),
                Column::new("tier".into(), au_tier),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::AbilityUpgrades] = Some(df);
        }

        if load_item_purchases {
            let df = df_from_columns(vec![
                Column::new("tick".into(), ip_ticks),
                Column::new("steam_id".into(), ip_steam_ids),
                Column::new("hero_id".into(), ip_hero_ids),
                Column::new("ability_id".into(), ip_ability_ids),
                Column::new("change".into(), ip_changes),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::ItemPurchases] = Some(df);
        }

        if load_chat {
            let df = df_from_columns(vec![
                Column::new("tick".into(), chat_ticks),
                Column::new("hero_id".into(), chat_hero_ids),
                Column::new("text".into(), chat_texts),
                Column::new("chat_type".into(), chat_types),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::Chat] = Some(df);
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
            self.cached_datasets[Dataset::Objectives] = Some(df);
        }

        if load_mid_boss {
            let df = df_from_columns(vec![
                Column::new("tick".into(), mb_ticks),
                Column::new("team_num".into(), mb_team_nums),
                Column::new("event".into(), mb_events),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::MidBoss] = Some(df);
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
            self.cached_datasets[Dataset::Neutrals] = Some(df);
        }

        if load_breakables {
            let df = df_from_columns(vec![
                Column::new("tick".into(), bk_tick),
                Column::new("event".into(), bk_event),
                Column::new("entity_id".into(), bk_entity_id),
                Column::new("entity_serial".into(), bk_entity_serial),
                Column::new("subclass_id".into(), bk_subclass_id),
                Column::new("subclass_name".into(), bk_subclass_name),
                Column::new("team_num".into(), bk_team_num),
                Column::new("x".into(), bk_x),
                Column::new("y".into(), bk_y),
                Column::new("z".into(), bk_z),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::Breakables] = Some(df);
        }

        if load_sinners_sacrifice {
            let df = df_from_columns(vec![
                Column::new("tick".into(), sn_tick),
                Column::new("event".into(), sn_event),
                Column::new("entity_id".into(), sn_entity_id),
                Column::new("entity_serial".into(), sn_entity_serial),
                Column::new("attacker_hero_id".into(), sn_attacker_hero_id),
                Column::new("damage".into(), sn_damage),
                Column::new("health".into(), sn_health),
                Column::new("max_health".into(), sn_max_health),
                Column::new("team_num".into(), sn_team_num),
                Column::new("x".into(), sn_x),
                Column::new("y".into(), sn_y),
                Column::new("z".into(), sn_z),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::SinnersSacrifice] = Some(df);
        }

        if load_stat_modifier_events {
            let df = df_from_columns(vec![
                Column::new("tick".into(), sm_tick),
                Column::new("hero_id".into(), sm_hero_id),
                Column::new("stat_type".into(), sm_stat_type),
                Column::new("amount".into(), sm_amount),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::StatModifierEvents] = Some(df);
        }

        if load_active_modifiers {
            let df = df_from_columns(vec![
                Column::new("tick".into(), am_tick),
                Column::new("hero_id".into(), am_hero_id),
                Column::new("event".into(), am_event),
                Column::new("serial".into(), am_serial),
                Column::new("modifier_id".into(), am_modifier_id),
                Column::new("ability_id".into(), am_ability_id),
                Column::new("duration".into(), am_duration),
                Column::new("caster_hero_id".into(), am_caster_hero_id),
                Column::new("stacks".into(), am_stacks),
            ])
            .map_err(|e| InvalidDemoError::new_err(format!("Failed to create DataFrame: {e}")))?;
            self.cached_datasets[Dataset::ActiveModifiers] = Some(df);
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
            self.cached_datasets[Dataset::AbilityTicks] = Some(df);
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
            self.cached_datasets[Dataset::Urn] = Some(df);
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
            self.cached_datasets[Dataset::StreetBrawlTicks] = Some(df);
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
            self.cached_datasets[Dataset::StreetBrawlRounds] = Some(df);
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
            self.cached_datasets[Dataset::Rift] = Some(df);
        }

        Ok(())
    }
}
