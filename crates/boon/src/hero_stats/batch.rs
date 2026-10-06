//! Independent stat queries sharing one replay decode.
use super::{
    CalculationError, HeroStatQuery, Ruleset, StatCatalog, StatResult,
    ability_stats::{AbilityRuleset, AbilityStatQuery, AbilityStatResult, ImbueQuery, ImbueResult},
    inputs,
};
use crate::Parser;
use serde::Serialize;
use std::collections::BTreeSet;

/// Optional queries to run in one pass. Each retains its own ticks and filters.
#[derive(Clone, Debug, Default)]
pub struct StatBatch<'a> {
    hero_stats: Option<(&'a HeroStatQuery, &'a Ruleset)>,
    ability_stats: Option<(&'a AbilityStatQuery, &'a AbilityRuleset)>,
    imbues: Option<&'a ImbueQuery>,
}

impl<'a> StatBatch<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn hero_stats(mut self, query: &'a HeroStatQuery, rules: &'a Ruleset) -> Self {
        self.hero_stats = Some((query, rules));
        self
    }

    #[must_use]
    pub fn ability_stats(mut self, query: &'a AbilityStatQuery, rules: &'a AbilityRuleset) -> Self {
        self.ability_stats = Some((query, rules));
        self
    }

    #[must_use]
    pub fn imbues(mut self, query: &'a ImbueQuery) -> Self {
        self.imbues = Some(query);
        self
    }
}

/// Ordinary query results. Unrequested datasets are absent.
#[derive(Clone, Debug, Default, Serialize)]
pub struct StatBatchResult {
    pub hero_stats: Option<StatResult>,
    pub ability_stats: Option<AbilityStatResult>,
    pub imbues: Option<ImbueResult>,
}

impl Parser {
    /// Calculate hero stats, ability stats and/or imbues in one replay pass.
    ///
    /// Queries may use different ticks, players, modes and explanation settings.
    /// Each dataset has the same semantics as its standalone query.
    ///
    /// # Errors
    /// Returns an error for an empty batch or any invalid/unresolved query,
    /// according to that query's strict setting.
    pub fn calculate_stats(
        &self,
        batch: &StatBatch<'_>,
        catalog: &StatCatalog,
    ) -> Result<StatBatchResult, CalculationError> {
        if batch.hero_stats.is_none() && batch.ability_stats.is_none() && batch.imbues.is_none() {
            return Err(CalculationError::Invalid(
                "provide at least one stat query".into(),
            ));
        }
        let mut result = StatBatchResult {
            hero_stats: batch
                .hero_stats
                .map(|(query, rules)| StatResult::prepare(query, catalog, rules))
                .transpose()?,
            ability_stats: batch
                .ability_stats
                .map(|(query, rules)| AbilityStatResult::prepare(query, catalog, rules))
                .transpose()?,
            imbues: batch
                .imbues
                .map(|query| {
                    query.validate()?;
                    Ok::<_, CalculationError>(ImbueResult::new(catalog))
                })
                .transpose()?,
        };
        let ticks = |values: &[i32]| values.iter().copied().collect::<BTreeSet<_>>();
        let hero_ticks = ticks(batch.hero_stats.map_or(&[], |(query, _)| &query.ticks));
        let ability_ticks = ticks(
            batch
                .ability_stats
                .map_or(&[], |(query, _)| &query.selection.ticks),
        );
        let imbue_ticks = ticks(batch.imbues.map_or(&[], |query| &query.ticks));
        let requested: Vec<_> = hero_ticks
            .iter()
            .chain(&ability_ticks)
            .chain(&imbue_ticks)
            .copied()
            .collect();
        self.visit_stat_ticks(&requested, catalog, |ctx, modifiers| {
            if hero_ticks.contains(&ctx.tick())
                && let (Some((query, _)), Some(result)) = (batch.hero_stats, &mut result.hero_stats)
            {
                inputs::collect(ctx, query, catalog, modifiers, result)?;
            }
            if ability_ticks.contains(&ctx.tick())
                && let (Some((query, _)), Some(result)) =
                    (batch.ability_stats, &mut result.ability_stats)
            {
                query.collect(ctx, catalog, modifiers, result)?;
            }
            if imbue_ticks.contains(&ctx.tick())
                && let (Some(query), Some(result)) = (batch.imbues, &mut result.imbues)
            {
                query.collect(ctx, catalog, result)?;
            }
            Ok(())
        })?;
        if let Some(result) = &mut result.hero_stats {
            result.finish();
        }
        if let (Some((query, _)), Some(result)) = (batch.ability_stats, &result.ability_stats) {
            result.finish(query)?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{HeroStat, ability_stats::AbilityStat};
    use super::*;

    fn invalid(batch: &StatBatch<'_>, message: &str) {
        let folder = super::super::catalog::tests::fixture();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let error = Parser::from_bytes(Vec::new())
            .calculate_stats(batch, &catalog)
            .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }

    #[test]
    fn batch_validates_queries_before_decoding() {
        invalid(&StatBatch::new(), "at least one stat query");
        let hero = HeroStatQuery::new([1], [HeroStat::ClipSize]);
        let hero_rules = Ruleset::new().with(HeroStat::ClipSize.rule());
        let ability = AbilityStatQuery::new([1], [AbilityStat::RangeBonus]);
        let ability_rules = AbilityRuleset::new().with(AbilityStat::RangeBonus.rule());
        invalid(
            &StatBatch::new().hero_stats(&hero, &Ruleset::new()),
            "select clip_size",
        );
        invalid(
            &StatBatch::new().ability_stats(&ability, &AbilityRuleset::new()),
            "select range_bonus",
        );
        for ticks in [vec![], vec![-1], vec![1, -1]] {
            let imbues = ImbueQuery::new(ticks.clone());
            invalid(
                &StatBatch::new()
                    .hero_stats(&hero, &hero_rules)
                    .imbues(&imbues),
                "nonnegative ticks",
            );
            let hero = HeroStatQuery::new(ticks.clone(), [HeroStat::ClipSize]);
            invalid(
                &StatBatch::new()
                    .hero_stats(&hero, &hero_rules)
                    .ability_stats(&ability, &ability_rules),
                "nonnegative ticks",
            );
            let ability = AbilityStatQuery::new(ticks, [AbilityStat::RangeBonus]);
            invalid(
                &StatBatch::new().ability_stats(&ability, &ability_rules),
                "nonnegative ticks",
            );
        }
        invalid(
            &StatBatch::new().hero_stats(&HeroStatQuery::new([1], []), &hero_rules),
            "at least one stat",
        );
        invalid(
            &StatBatch::new().ability_stats(&AbilityStatQuery::new([1], []), &ability_rules),
            "at least one ability stat",
        );
    }

    #[test]
    fn batch_runs_optional_queries_and_propagates_replay_errors() {
        use boon_proto::proto::{CDemoFullPacket, CDemoSendTables, EDemoCommands};
        use prost::Message;
        let mut bytes = b"PBDEMS2\0".to_vec();
        bytes.extend([0; 8]);
        for (command, tick, body) in [
            (
                EDemoCommands::DemSendTables,
                0,
                CDemoSendTables {
                    data: Some(vec![0]),
                }
                .encode_to_vec(),
            ),
            (EDemoCommands::DemClassInfo, 0, vec![]),
            (EDemoCommands::DemSyncTick, 0, vec![]),
            (
                EDemoCommands::DemFullPacket,
                10,
                CDemoFullPacket::default().encode_to_vec(),
            ),
            (
                EDemoCommands::DemFullPacket,
                20,
                CDemoFullPacket::default().encode_to_vec(),
            ),
            (EDemoCommands::DemStop, 21, vec![]),
        ] {
            for value in [command as u64, tick, body.len() as u64] {
                prost::encoding::encode_varint(value, &mut bytes);
            }
            bytes.extend(body);
        }
        let parser = Parser::from_bytes(bytes);
        let folder = super::super::catalog::tests::fixture();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let hero = HeroStatQuery::new([20, 10, 20], [HeroStat::ClipSize]);
        let hero_rules = Ruleset::new().with(HeroStat::ClipSize.rule());
        let ability = AbilityStatQuery::new([20], [AbilityStat::RangeBonus])
            .mode(super::super::StatMode::Baseline);
        let ability_rules = AbilityRuleset::new().with(AbilityStat::RangeBonus.rule());
        let imbues = ImbueQuery::new([10]);
        let batch = StatBatch::new()
            .hero_stats(&hero, &hero_rules)
            .ability_stats(&ability, &ability_rules)
            .imbues(&imbues);
        let result = parser.calculate_stats(&batch, &catalog).unwrap();
        assert!(result.hero_stats.unwrap().values.is_empty());
        assert_eq!(
            result.ability_stats.unwrap().metadata.mode,
            Some(super::super::StatMode::Baseline)
        );
        assert!(result.imbues.unwrap().bindings.is_empty());
        let result = parser
            .calculate_stats(&StatBatch::new().imbues(&imbues), &catalog)
            .unwrap();
        assert!(result.hero_stats.is_none() && result.ability_stats.is_none());
        assert!(result.imbues.is_some());
        let error = parser
            .calculate_stats(&StatBatch::new().imbues(&ImbueQuery::new([19])), &catalog)
            .unwrap_err();
        assert!(error.to_string().contains("demo has no tick 19"));
        let missing = AbilityStatQuery::new([20], [AbilityStat::RangeBonus]).abilities([123]);
        let error = parser
            .calculate_stats(
                &StatBatch::new().ability_stats(&missing, &ability_rules),
                &catalog,
            )
            .unwrap_err();
        assert!(error.to_string().contains("not owned"));
        let missing = HeroStatQuery::new([20], [HeroStat::ClipSize]).steam_ids([1]);
        let error = parser
            .calculate_stats(
                &StatBatch::new().hero_stats(&missing, &hero_rules),
                &catalog,
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Steam ID 1 has no selected hero at tick 20")
        );
    }

    #[test]
    #[ignore = "requires 111155916.dem and installed boon-data 6746"]
    fn batch_matches_standalone_queries_on_demo() {
        let parser = Parser::from_file(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../111155916.dem"),
        )
        .unwrap();
        let catalog = StatCatalog::load("6746").unwrap();
        let hero_stats = [
            HeroStat::ClipSize,
            HeroStat::WeaponDamage,
            HeroStat::FireRate,
        ];
        let hero = HeroStatQuery::new([73952, 51208, 73952], hero_stats)
            .explain(true)
            .strict(false);
        let hero_rules = hero_stats
            .iter()
            .fold(Ruleset::new(), |rules, stat| rules.with(stat.rule()));
        let ability = AbilityStatQuery::new([51848], AbilityStat::ALL)
            .mode(super::super::StatMode::Baseline)
            .explain(true)
            .strict(false);
        let ability_rules = AbilityStat::ALL
            .iter()
            .fold(AbilityRuleset::new(), |rules, stat| rules.with(stat.rule()));
        let imbue = ImbueQuery::new([51208, 73952]);
        let result = parser
            .calculate_stats(
                &StatBatch::new()
                    .hero_stats(&hero, &hero_rules)
                    .ability_stats(&ability, &ability_rules)
                    .imbues(&imbue),
                &catalog,
            )
            .unwrap();
        let serialized = |result| serde_json::to_value(result).unwrap();
        assert_eq!(
            serialized(result.hero_stats.as_ref().unwrap()),
            serialized(
                &parser
                    .calculate_hero_stats(&hero, &catalog, &hero_rules)
                    .unwrap()
            )
        );
        assert_eq!(
            serde_json::to_value(result.ability_stats.as_ref().unwrap()).unwrap(),
            serde_json::to_value(
                parser
                    .calculate_ability_stats(&ability, &catalog, &ability_rules)
                    .unwrap()
            )
            .unwrap()
        );
        assert_eq!(
            serde_json::to_value(result.imbues.as_ref().unwrap()).unwrap(),
            serde_json::to_value(parser.imbues(&imbue, &catalog).unwrap()).unwrap()
        );
        let stats = result.hero_stats.unwrap();
        assert!(!stats.contributions.is_empty());
        assert!(
            stats
                .contributions
                .iter()
                .all(|row| hero_stats.contains(&row.stat))
        );
        assert_eq!(
            stats
                .contributions
                .iter()
                .map(|row| row.stat)
                .collect::<BTreeSet<_>>(),
            hero_stats.into_iter().collect()
        );
        for row in &stats.contributions {
            if let Ok(stat) = row.input.parse::<HeroStat>() {
                assert_eq!(row.stat, stat);
            }
        }
        let filtered = HeroStatQuery::new([73952], [HeroStat::ClipSize])
            .steam_ids([])
            .strict(false);
        let result = parser
            .calculate_stats(
                &StatBatch::new().hero_stats(&filtered, &hero_rules),
                &catalog,
            )
            .unwrap();
        assert!(result.hero_stats.unwrap().values.is_empty());
        assert!(result.ability_stats.is_none() && result.imbues.is_none());
    }
}
