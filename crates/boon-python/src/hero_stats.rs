use crate::*;
use boon_parser::hero_stats::{HeroStat, HeroStatQuery, Ruleset, StatCatalog};

#[pymethods]
impl Demo {
    // Keyword arguments mirror the public Python query.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (directory, ticks, *, stats, steam_ids=None, heroes=None, explain=false, strict=true))]
    fn _calculate_hero_stats(
        &self,
        py: Python<'_>,
        directory: PathBuf,
        ticks: Vec<i32>,
        stats: Vec<String>,
        steam_ids: Option<Vec<u64>>,
        heroes: Option<Vec<i64>>,
        explain: bool,
        strict: bool,
    ) -> PyResult<String> {
        py.detach(|| {
            let stats: Vec<HeroStat> = stats
                .iter()
                .map(|stat| stat.parse())
                .collect::<Result<_, _>>()?;
            let rules = stats
                .iter()
                .fold(Ruleset::new(), |rules, stat| rules.with(stat.rule()));
            let catalog = StatCatalog::from_directory(&directory)?;
            let mut query = HeroStatQuery::new(ticks, stats)
                .explain(explain)
                .strict(strict);
            if let Some(steam_ids) = steam_ids {
                query = query.steam_ids(steam_ids);
            }
            if let Some(heroes) = heroes {
                query = query.heroes(heroes);
            }
            let result = self.parser.calculate_hero_stats(&query, &catalog, &rules)?;
            serde_json::to_string(&result)
                .map_err(|e| boon_parser::hero_stats::CalculationError::Invalid(e.to_string()))
        })
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }
}

#[pymethods]
impl Demo {
    #[pyo3(signature = (directory, ticks, *, steam_ids=None))]
    fn _imbues(
        &self,
        py: Python<'_>,
        directory: PathBuf,
        ticks: Vec<i32>,
        steam_ids: Option<Vec<u64>>,
    ) -> PyResult<String> {
        py.detach(|| {
            let catalog = StatCatalog::from_directory(&directory)?;
            let mut query = boon_parser::ability_stats::ImbueQuery::new(ticks);
            if let Some(steam_ids) = steam_ids {
                query = query.steam_ids(steam_ids);
            }
            let result = self.parser.imbues(&query, &catalog)?;
            serde_json::to_string(&result)
                .map_err(|e| boon_parser::hero_stats::CalculationError::Invalid(e.to_string()))
        })
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (directory, ticks, *, stats, steam_ids=None, abilities=None, include_items=false, explain=false, strict=true))]
    fn _calculate_ability_stats(
        &self,
        py: Python<'_>,
        directory: PathBuf,
        ticks: Vec<i32>,
        stats: Vec<String>,
        steam_ids: Option<Vec<u64>>,
        abilities: Option<Vec<u32>>,
        include_items: bool,
        explain: bool,
        strict: bool,
    ) -> PyResult<String> {
        py.detach(|| {
            use boon_parser::ability_stats::{AbilityRuleset, AbilityStat, AbilityStatQuery};
            let stats: Vec<AbilityStat> =
                stats.iter().map(|s| s.parse()).collect::<Result<_, _>>()?;
            let rules = stats
                .iter()
                .fold(AbilityRuleset::new(), |rules, stat| rules.with(stat.rule()));
            let catalog = StatCatalog::from_directory(&directory)?;
            let mut query = AbilityStatQuery::new(ticks, stats)
                .include_items(include_items)
                .explain(explain)
                .strict(strict);
            if let Some(steam_ids) = steam_ids {
                query = query.steam_ids(steam_ids);
            }
            if let Some(abilities) = abilities {
                query = query.abilities(abilities);
            }
            let result = self
                .parser
                .calculate_ability_stats(&query, &catalog, &rules)?;
            serde_json::to_string(&result)
                .map_err(|e| boon_parser::hero_stats::CalculationError::Invalid(e.to_string()))
        })
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }
}
