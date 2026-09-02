use crate::*;

/// A `str` or `list[str]` Python argument (e.g. `datasets=`, `events=`).
#[derive(FromPyObject)]
pub(super) enum StrOrList {
    #[pyo3(transparent)]
    One(String),
    #[pyo3(transparent)]
    Many(Vec<String>),
}

impl StrOrList {
    pub(super) fn into_vec(self) -> Vec<String> {
        match self {
            StrOrList::One(s) => vec![s],
            StrOrList::Many(v) => v,
        }
    }
}

/// An `int` or `list[int]` Python argument (e.g. `ticks=`).
#[derive(FromPyObject)]
pub(super) enum IntOrList {
    #[pyo3(transparent)]
    One(i32),
    #[pyo3(transparent)]
    Many(Vec<i32>),
}

impl IntOrList {
    pub(super) fn into_vec(self) -> Vec<i32> {
        match self {
            IntOrList::One(t) => vec![t],
            IntOrList::Many(v) => v,
        }
    }
}

pub(super) const VALID_DATASETS: &[&str] = &[
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
    "breakables",
    "sinners_sacrifice",
    "stat_modifier_events",
    "active_modifiers",
    "urn",
    "rift",
];

pub(super) const VALID_STREET_BRAWL_DATASETS: &[&str] =
    &["street_brawl_ticks", "street_brawl_rounds"];

/// Known Rift ("Koth") cash-in sites, as `([x, y], lane)`.
///
/// The Rift entities carry no `m_iLane` field, so the lane has to come from the
/// cash-in location. Each site below was cross-checked against the lane of the
/// buffed trooper cohort that spawns for the winning team after a capture. Only
/// these two sites have been observed, so any other location resolves to lane
/// `0` rather than being guessed at.
pub(super) const RIFT_LANE_SITES: &[([f32; 2], i64)] = &[([-7560.0, 0.0], 1), ([7612.0, 0.0], 6)];

/// Match radius, in Hammer units, for associating a location with a known Rift
/// site. The two known sites are ~15k units apart, so this is deliberately
/// loose enough to absorb per-match jitter without ever matching both.
pub(super) const RIFT_LANE_TOLERANCE: f32 = 1024.0;

/// Upper bound, in Hammer units, on a plausible map coordinate.
///
/// The game clears `m_vKothCashInCurrentLocation` to `FLT_MAX` rather than to
/// zero once a Rift resolves. `FLT_MAX` is finite, so an `is_finite` check does
/// not reject it — this bound does.
pub(super) const RIFT_COORD_SANITY: f32 = 1.0e6;

/// The lane for a Rift cash-in location, or `0` when the location is not a
/// known Rift site (see [`RIFT_LANE_SITES`]).
pub(super) fn rift_lane_for(x: f32, y: f32) -> i64 {
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

pub(super) fn is_sinners_sacrifice(class_name: &str) -> bool {
    matches!(
        class_name,
        "CNPC_Neutral_SinnersSacrifice" | "CNPC_Neutral_SinnersSacrifice_Hideout"
    )
}

/// ``citadel_type`` value used for melee-typed damage.
pub(super) const MELEE_CITADEL_TYPE: i32 = 3;

/// Valve damage flags that distinguish a light or heavy melee hit.
/// These are the network values of ``DFLAG_LIGHT_MELEE`` and
/// ``DFLAG_HEAVY_MELEE`` respectively.
pub(super) const DAMAGE_FLAG_LIGHT_MELEE: u64 = 1 << 33;
pub(super) const DAMAGE_FLAG_HEAVY_MELEE: u64 = 1 << 34;

/// Return the melee flag and nullable melee subtype for a damage event.
/// Non-melee damage has no subtype. Valve's explicit flags identify light and
/// heavy hits; every other ``citadel_type == 3`` source is retained as
/// ``other`` rather than inferred from an ability name or damage amount.
pub(super) fn classify_melee_damage(
    citadel_type: i32,
    damage_flags: u64,
) -> (bool, Option<&'static str>) {
    if citadel_type != MELEE_CITADEL_TYPE {
        return (false, None);
    }

    let is_light = damage_flags & DAMAGE_FLAG_LIGHT_MELEE != 0;
    let is_heavy = damage_flags & DAMAGE_FLAG_HEAVY_MELEE != 0;
    let melee_type = match (is_light, is_heavy) {
        (true, false) => "light",
        (false, true) => "heavy",
        _ => "other",
    };
    (true, Some(melee_type))
}

#[cfg(test)]
mod damage_classification_tests {
    use super::*;

    #[test]
    pub(super) fn separates_light_heavy_other_and_non_melee() {
        assert_eq!(
            classify_melee_damage(MELEE_CITADEL_TYPE, DAMAGE_FLAG_LIGHT_MELEE),
            (true, Some("light"))
        );
        assert_eq!(
            classify_melee_damage(MELEE_CITADEL_TYPE, DAMAGE_FLAG_HEAVY_MELEE),
            (true, Some("heavy"))
        );
        assert_eq!(
            classify_melee_damage(
                MELEE_CITADEL_TYPE,
                DAMAGE_FLAG_LIGHT_MELEE | DAMAGE_FLAG_HEAVY_MELEE
            ),
            (true, Some("other"))
        );
        assert_eq!(
            classify_melee_damage(MELEE_CITADEL_TYPE, 0),
            (true, Some("other"))
        );
        assert_eq!(
            classify_melee_damage(1, DAMAGE_FLAG_LIGHT_MELEE),
            (false, None)
        );
    }
}

pub(super) struct SummaryFrames {
    pub(super) snapshots: DataFrame,
    pub(super) last_hits: DataFrame,
    pub(super) objectives: DataFrame,
    pub(super) damage: DataFrame,
}

#[derive(Clone, Copy)]
pub(super) struct BreakableState {
    pub(super) subclass_id: u32,
    pub(super) team_num: i64,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) z: f32,
}

#[derive(Clone, Copy)]
pub(super) struct SinnersSacrificeState {
    pub(super) health: i64,
    pub(super) max_health: i64,
    pub(super) team_num: i64,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) z: f32,
}

/// The (gold, orbs) a player earned from a given source at a snapshot, or
/// ``(0, 0)`` when that source is absent.
pub(super) fn gold_source_totals(
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
pub(super) fn build_snapshots_frame(
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
pub(super) fn build_last_hits_frame(
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
pub(super) fn build_objectives_frame(
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
pub(super) fn stat_type_label(stat_type: i32) -> String {
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
pub(super) fn is_category_source(name: &str) -> bool {
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
pub(super) fn build_damage_frame(
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
