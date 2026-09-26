use crate::*;

/// A `str` or `list[str]` Python argument, such as `datasets=` or `events=`.
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

/// An `int` or `list[int]` Python argument, such as `ticks=`.
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

/// A dataset name validated at the Python API boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Dataset {
    Abilities,
    AbilityUpgrades,
    AbilityTicks,
    Chat,
    MidBoss,
    Objectives,
    PlayerTicks,
    WorldTicks,
    Kills,
    Damage,
    FlexSlots,
    ItemPurchases,
    Troopers,
    Neutrals,
    Breakables,
    SinnersSacrifice,
    StatModifierEvents,
    ActiveModifiers,
    Urn,
    Rift,
    StreetBrawlTicks,
    StreetBrawlRounds,
}

impl Dataset {
    pub(super) const ALL: [Self; 22] = [
        Self::Abilities,
        Self::AbilityUpgrades,
        Self::AbilityTicks,
        Self::Chat,
        Self::MidBoss,
        Self::Objectives,
        Self::PlayerTicks,
        Self::WorldTicks,
        Self::Kills,
        Self::Damage,
        Self::FlexSlots,
        Self::ItemPurchases,
        Self::Troopers,
        Self::Neutrals,
        Self::Breakables,
        Self::SinnersSacrifice,
        Self::StatModifierEvents,
        Self::ActiveModifiers,
        Self::Urn,
        Self::Rift,
        Self::StreetBrawlTicks,
        Self::StreetBrawlRounds,
    ];

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Abilities => "abilities",
            Self::AbilityUpgrades => "ability_upgrades",
            Self::AbilityTicks => "ability_ticks",
            Self::Chat => "chat",
            Self::MidBoss => "mid_boss",
            Self::Objectives => "objectives",
            Self::PlayerTicks => "player_ticks",
            Self::WorldTicks => "world_ticks",
            Self::Kills => "kills",
            Self::Damage => "damage",
            Self::FlexSlots => "flex_slots",
            Self::ItemPurchases => "item_purchases",
            Self::Troopers => "troopers",
            Self::Neutrals => "neutrals",
            Self::Breakables => "breakables",
            Self::SinnersSacrifice => "sinners_sacrifice",
            Self::StatModifierEvents => "stat_modifier_events",
            Self::ActiveModifiers => "active_modifiers",
            Self::Urn => "urn",
            Self::Rift => "rift",
            Self::StreetBrawlTicks => "street_brawl_ticks",
            Self::StreetBrawlRounds => "street_brawl_rounds",
        }
    }

    pub(super) const fn is_street_brawl(self) -> bool {
        matches!(self, Self::StreetBrawlTicks | Self::StreetBrawlRounds)
    }
}

impl std::str::FromStr for Dataset {
    type Err = PyErr;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|dataset| dataset.as_str() == name).ok_or_else(|| {
            let mut standard = Vec::new();
            let mut street_brawl = Vec::new();
            for dataset in Self::ALL {
                if dataset.is_street_brawl() {
                    street_brawl.push(dataset.as_str());
                } else {
                    standard.push(dataset.as_str());
                }
            }
            pyo3::exceptions::PyValueError::new_err(format!(
                "Unknown dataset: {name:?}. Valid datasets: {standard:?}, street brawl: {street_brawl:?}"
            ))
        })
    }
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
    pub(super) healing: DataFrame,
    pub(super) gold_sources: DataFrame,
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
    let mut player_slot = Vec::new();
    let mut creep_damage = Vec::new();
    let mut neutral_damage = Vec::new();
    let mut boss_damage = Vec::new();
    let mut self_damage = Vec::new();
    let mut player_damage_taken = Vec::new();
    let mut player_healing = Vec::new();
    let mut teammate_healing = Vec::new();
    let mut self_healing = Vec::new();
    let mut damage_mitigated = Vec::new();
    let mut damage_absorbed = Vec::new();
    let mut absorption_provided = Vec::new();
    let mut heal_prevented = Vec::new();
    let mut heal_lost = Vec::new();

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
            player_slot.push(player.player_slot);
            creep_damage.push(stats.creep_damage);
            neutral_damage.push(stats.neutral_damage);
            boss_damage.push(stats.boss_damage);
            self_damage.push(stats.self_damage);
            player_damage_taken.push(stats.player_damage_taken);
            player_healing.push(stats.player_healing);
            teammate_healing.push(stats.teammate_healing);
            self_healing.push(stats.self_healing);
            damage_mitigated.push(stats.damage_mitigated);
            damage_absorbed.push(stats.damage_absorbed);
            absorption_provided.push(stats.absorption_provided);
            heal_prevented.push(stats.heal_prevented);
            heal_lost.push(stats.heal_lost);

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
        Column::new("player_slot".into(), player_slot),
        Column::new("creep_damage".into(), creep_damage),
        Column::new("neutral_damage".into(), neutral_damage),
        Column::new("boss_damage".into(), boss_damage),
        Column::new("self_damage".into(), self_damage),
        Column::new("player_damage_taken".into(), player_damage_taken),
        Column::new("player_healing".into(), player_healing),
        Column::new("teammate_healing".into(), teammate_healing),
        Column::new("self_healing".into(), self_healing),
        Column::new("damage_mitigated".into(), damage_mitigated),
        Column::new("damage_absorbed".into(), damage_absorbed),
        Column::new("absorption_provided".into(), absorption_provided),
        Column::new("heal_prevented".into(), heal_prevented),
        Column::new("heal_lost".into(), heal_lost),
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

/// Return true for a Valve damage-category ``source_name``.
/// Categories are ``Bullet``, ``Ability``, ``Melee``, ``Misc``, and
/// ``UnknownAbility``. Categories use capitalized names. Specific sources use
/// snake_case names, such as ``citadel_weapon_astro_set``.
pub(super) fn is_category_source(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase) && !name.contains('_')
}

/// Build the post-match ``damage`` DataFrame from the damage matrix.
///
/// Each row identifies a dealer slot, target slot, source, and sample time.
/// The hero ID columns map player slots to heroes. Non-player slots are null.
/// The ``damage`` column contains the ``stat_type`` value for the interval
/// that ends at the sample. The ``total`` column preserves the recorded cumulative
/// value at ``sample_time_s``. Do not sum these totals across sample times.
///
/// Return an empty frame when the demo has no damage matrix. Some cumulative
/// arrays are shorter than the full sample list. This occurs when a pair meets
/// during the match. Align these arrays to the end of ``sample_time_s``. We
/// verified this alignment against snapshot ``player_damage``.
///
/// The matrix records each hit under a category and under a specific source.
/// Category rows set ``is_category`` to true. Most other rows set it to false.
/// Do not sum category rows and specific-source rows together. Filter to
/// ``is_category == False`` to get the per-source values without duplicates.
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
    let mut total = Vec::new();
    let mut interval_start_s = Vec::new();

    // Map player slots to hero IDs.
    // Keep slot 0 and other non-player slots null.
    let slot_to_hero: HashMap<u32, u32> = match_info
        .players
        .iter()
        .filter_map(|p| p.player_slot.map(|slot| (slot, p.hero_id())))
        .collect();

    if let Some(matrix) = match_info.damage_matrix.as_ref() {
        let times = &matrix.sample_time_s;
        let n = times.len();
        polars_ensure!(times.windows(2).all(|pair| pair[0] < pair[1]), ComputeError:
            "damage matrix sample times are not strictly increasing");
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
                    let start = n.checked_sub(arr.len()).ok_or_else(|| {
                        PolarsError::ComputeError(
                            "damage matrix has more values than sample times".into(),
                        )
                    })?;
                    let mut prev = 0u32;
                    for (k, &cumulative) in arr.iter().enumerate() {
                        let index = start + k;
                        let time = times[index];
                        interval_start_s.push(if index == 0 { 0 } else { times[index - 1] });
                        total.push(cumulative);
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
        Column::new("total".into(), total),
        Column::new("interval_start_s".into(), interval_start_s),
    ])
}

/// Healing and regeneration amounts and recorded totals at each sample.
/// Category rows duplicate specific sources and are excluded. Keep zero changes
/// so a source remains visible at later reporting periods.
pub(super) fn build_healing_frame(damage: &DataFrame) -> PolarsResult<DataFrame> {
    let stat_type = damage.column("stat_type")?.str()?;
    let category = damage.column("is_category")?.bool()?;
    let mask = (stat_type.equal("healing") | stat_type.equal("regen")) & !category;
    let mut healing = damage.filter(&mask)?;
    for (old, new) in [
        ("sample_time_s", "interval_end_s"),
        ("dealer_player_slot", "healer_player_slot"),
        ("dealer_hero_id", "healer_hero_id"),
        ("damage", "amount"),
    ] {
        healing.rename(old, new.into())?;
    }
    healing
        .select([
            "interval_start_s",
            "interval_end_s",
            "healer_player_slot",
            "healer_hero_id",
            "target_player_slot",
            "target_hero_id",
            "source_name",
            "stat_type",
            "amount",
            "total",
        ])?
        .sort(
            [
                "interval_end_s",
                "healer_player_slot",
                "target_player_slot",
                "source_name",
                "stat_type",
            ],
            SortMultipleOptions::default(),
        )
}

/// Preserve each recorded soul source at each player snapshot.
/// Keep unknown source IDs and absent counters instead of inventing values.
pub(super) fn build_gold_sources_frame(
    match_info: &boon_proto::proto::c_msg_match_meta_data_contents::MatchInfo,
) -> PolarsResult<DataFrame> {
    use boon_proto::proto::c_msg_match_meta_data_contents::EGoldSource;

    let mut snapshot_time_s = Vec::new();
    let mut player_slot = Vec::new();
    let mut hero_id = Vec::new();
    let mut source_id = Vec::new();
    let mut source_name = Vec::new();
    let mut gold = Vec::new();
    let mut gold_orbs = Vec::new();
    let mut kills = Vec::new();
    let mut damage = Vec::new();
    for player in &match_info.players {
        for stats in &player.stats {
            for source in &stats.gold_sources {
                snapshot_time_s.push(stats.time_stamp_s());
                player_slot.push(player.player_slot);
                hero_id.push(player.hero_id());
                source_id.push(source.source);
                source_name.push(source.source.map(|id| {
                    EGoldSource::try_from(id).map_or_else(
                        |_| format!("unknown_{id}"),
                        |value| value.as_str_name().to_owned(),
                    )
                }));
                gold.push(source.gold);
                gold_orbs.push(source.gold_orbs);
                kills.push(source.kills);
                damage.push(source.damage);
            }
        }
    }
    df_from_columns(vec![
        Column::new("snapshot_time_s".into(), snapshot_time_s),
        Column::new("player_slot".into(), player_slot),
        Column::new("hero_id".into(), hero_id),
        Column::new("source_id".into(), source_id),
        Column::new("source_name".into(), source_name),
        Column::new("gold".into(), gold),
        Column::new("gold_orbs".into(), gold_orbs),
        Column::new("kills".into(), kills),
        Column::new("damage".into(), damage),
    ])?
    .sort(
        ["snapshot_time_s", "player_slot", "source_id"],
        SortMultipleOptions::default(),
    )
}

#[cfg(test)]
mod summary_tests {
    use super::*;
    use boon_proto::proto::{
        CMsgMatchPlayerDamageMatrix,
        c_msg_match_meta_data_contents::{GoldSource, MatchInfo, PlayerStats, Players},
        c_msg_match_player_damage_matrix::{
            DamageDealer, DamageSource, DamageToPlayer, SourceDetails,
        },
    };

    fn matrix_info() -> MatchInfo {
        MatchInfo {
            damage_matrix: Some(CMsgMatchPlayerDamageMatrix {
                sample_time_s: vec![180, 360, 540],
                source_details: Some(SourceDetails {
                    source_name: vec!["ability_heal".into(), "base_stat_regen".into()],
                    stat_type: vec![1, 5],
                }),
                damage_dealers: vec![DamageDealer {
                    dealer_player_slot: Some(2),
                    damage_sources: vec![
                        DamageSource {
                            source_details_index: Some(0),
                            damage_to_players: vec![DamageToPlayer {
                                target_player_slot: Some(3),
                                damage: vec![12, 12],
                            }],
                        },
                        DamageSource {
                            source_details_index: Some(1),
                            damage_to_players: vec![DamageToPlayer {
                                target_player_slot: Some(2),
                                damage: vec![3, 9, 9],
                            }],
                        },
                    ],
                }],
            }),
            ..Default::default()
        }
    }

    #[test]
    fn cumulative_samples_keep_sparse_alignment_and_zero_changes() -> PolarsResult<()> {
        let damage = build_damage_frame(&matrix_info())?;
        assert_eq!(
            damage
                .column("sample_time_s")?
                .u32()?
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [360, 540, 180, 360, 540]
        );
        assert_eq!(
            damage
                .column("interval_start_s")?
                .u32()?
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [180, 360, 0, 180, 360]
        );
        assert_eq!(
            damage
                .column("damage")?
                .u32()?
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [12, 0, 3, 6, 0]
        );
        assert_eq!(
            damage
                .column("total")?
                .u32()?
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [12, 12, 3, 9, 9]
        );
        let healing = build_healing_frame(&damage)?;
        assert_eq!(healing.height(), 5);
        assert_eq!(healing.column("amount")?.u32()?.sum(), Some(21));
        Ok(())
    }

    #[test]
    fn missing_matrix_has_typed_empty_frames() -> PolarsResult<()> {
        let damage = build_damage_frame(&MatchInfo::default())?;
        let healing = build_healing_frame(&damage)?;
        assert_eq!(damage.height(), 0);
        assert_eq!(healing.height(), 0);
        assert_eq!(damage.column("total")?.dtype(), &DataType::UInt32);
        assert_eq!(healing.column("total")?.dtype(), &DataType::UInt32);
        assert_eq!(build_gold_sources_frame(&MatchInfo::default())?.height(), 0);
        Ok(())
    }

    #[test]
    fn invalid_matrix_times_are_errors() {
        let mut info = matrix_info();
        let matrix = info.damage_matrix.as_mut().expect("test matrix");
        matrix.sample_time_s = vec![180, 180, 540];
        assert!(build_damage_frame(&info).is_err());
        info.damage_matrix
            .as_mut()
            .expect("test matrix")
            .sample_time_s = vec![180];
        assert!(build_damage_frame(&info).is_err());
    }

    #[test]
    fn unknown_soul_sources_and_absent_counters_survive() -> PolarsResult<()> {
        let info = MatchInfo {
            players: vec![Players {
                player_slot: Some(2),
                hero_id: Some(66),
                stats: vec![PlayerStats {
                    time_stamp_s: Some(180),
                    player_healing: Some(0),
                    gold_sources: vec![GoldSource {
                        source: Some(2048),
                        gold: Some(17),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let sources = build_gold_sources_frame(&info)?;
        assert_eq!(sources.column("source_id")?.i32()?.get(0), Some(2048));
        assert_eq!(
            sources.column("source_name")?.str()?.get(0),
            Some("unknown_2048")
        );
        assert_eq!(sources.column("gold")?.u32()?.get(0), Some(17));
        assert_eq!(sources.column("gold_orbs")?.u32()?.get(0), None);
        let snapshots = build_snapshots_frame(&info)?;
        assert_eq!(snapshots.column("player_healing")?.u32()?.get(0), Some(0));
        assert_eq!(snapshots.column("self_healing")?.u32()?.get(0), None);
        Ok(())
    }
}
