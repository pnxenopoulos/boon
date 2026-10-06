//! Select baseline effects from catalog roles and replay evidence.
use super::*;

impl StatMode {
    pub(super) fn includes_effect(self, effect: &Value) -> bool {
        self == Self::Current
            || !effect["usage_flags"].as_str().is_some_and(|flags| {
                flags
                    .split('|')
                    .any(|flag| flag.trim() == "ConditionallyApplied")
            })
    }
}

impl PlayerInputs<'_> {
    /// Filter once so every stat and its dependencies use the same inputs.
    pub(super) fn select_mode(
        &mut self,
        catalog: &StatCatalog,
        mode: StatMode,
    ) -> BTreeSet<String> {
        let mut diagnostics = BTreeSet::new();
        if mode == StatMode::Current {
            return diagnostics;
        }
        self.active.retain(|entry| {
            // A finite lifetime proves this application is temporary. An absent
            // or nonpositive duration does not prove that an effect is passive.
            if entry.duration.is_some_and(|duration| duration.is_finite() && duration > 0.0) {
                return false;
            }
            let Some(id) = entry.modifier_subclass else {
                diagnostics.insert("baseline excludes a modifier with no ID; its role is unknown".into());
                return false;
            };
            // Preserve the existing stat resolver's exclusion of ping markers.
            if matches!(id, PLAYER_PINGED | ENTITY_PINGED) {
                return false;
            }
            let source = match catalog.modifier(id, entry.ability_subclass) {
                Ok(source) => source,
                Err(error) => {
                    diagnostics.insert(format!("baseline excludes {error}; its role is unknown"));
                    return false;
                }
            };
            if let Some(owner) = source.ability_id.and_then(|id| catalog.abilities.get(&id)) {
                // Nested proc buffs are not intrinsic effects. Foreign effects
                // do not become baseline merely because the recipient owns an
                // ability with the same ID as the caster.
                return source.is_intrinsic_modifier_of(owner)
                    && self.owned.iter().any(|owned| owned.record_key == owner.record_key)
                    && !matches!((entry.parent, entry.caster), (Some(parent), Some(caster)) if parent != caster);
            }
            // Powerups are runtime effects. Permanent pickup totals are already
            // supplied by the stat-viewer vector, including signed penalties.
            if source.misc_id.is_some() {
                return false;
            }
            diagnostics.insert(format!(
                "baseline excludes {}; its passive or active role is unknown",
                source.record_key
            ));
            false
        });
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> StatCatalog {
        let folder = super::super::super::catalog::tests::fixture();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
        hero["m_mapLevelInfo"] = json!({});
        hero["m_mapPurchaseBonuses"] = json!({});
        hero["m_mapStartingStats"] = json!({});
        let intrinsic = &mut catalog.modifiers[1];
        intrinsic.definition_path = "/test_gun/m_AutoIntrinsicModifiers/0".into();
        intrinsic.record_key = "abilities#/test_gun/m_AutoIntrinsicModifiers/0".into();
        intrinsic.stat_changes = vec![
            json!({"stat": SPIRIT, "value": 4, "definition_path":"/spirit"}),
            json!({"stat": PERCENT, "value": 20, "definition_path":"/ammo"}),
            json!({"stat": BULLET_LIFESTEAL, "value": 20, "definition_path":"/lifesteal"}),
        ];
        let proc_buff = &mut catalog.modifiers[2];
        proc_buff.ability_id = Some(123);
        proc_buff.definition_path = "/test_gun/m_AutoIntrinsicModifiers/0/m_BuffModifier".into();
        proc_buff.stat_changes = vec![
            json!({"stat": SPIRIT, "value": 8, "definition_path":"/spirit"}),
            json!({"stat": PERCENT, "value": 50, "definition_path":"/ammo"}),
            json!({"stat": BULLET_LIFESTEAL, "value": 30, "definition_path":"/lifesteal"}),
        ];
        catalog
    }

    fn inputs<'a>(
        catalog: &'a StatCatalog,
        entries: &'a [CModifierTableEntry],
    ) -> PlayerInputs<'a> {
        PlayerInputs {
            hero: &catalog.heroes[&999],
            level: Some(0.0),
            weapon: &catalog.abilities[&123],
            inventory: vec![],
            owned: vec![&catalog.abilities[&123]],
            active: entries.iter().collect(),
            permanent: vec![
                RecordedStat {
                    source_id: 10,
                    value_type: None,
                    value: 9.0,
                },
                RecordedStat {
                    source_id: 10,
                    value_type: None,
                    value: -5.0,
                },
            ],
        }
    }

    fn entry(id: u32, serial: u32) -> CModifierTableEntry {
        CModifierTableEntry {
            modifier_subclass: Some(id),
            serial_number: Some(serial),
            ..Default::default()
        }
    }

    #[test]
    fn modes_recompute_nonlinear_stats_and_spirit_dependencies() {
        let mut catalog = catalog();
        for (index, percent) in [(1, 15), (2, 20)] {
            catalog.modifiers[index].stat_changes.push(json!({
                "stat": "MODIFIER_VALUE_TECH_POWER_PERCENT", "value": percent,
                "definition_path": "/spirit-percent"
            }));
        }
        // Both rows are untimed: catalog roles, not duration, separate them.
        let entries = [entry(11, 1), entry(12, 2)];
        let ctx = Context::new(1.0 / 64.0).unwrap();
        let controller = Entity::from_fields(1, 1, 0, "test", true, Default::default()).unwrap();
        for (mode, ammo, lifesteal, serials) in [
            (StatMode::Baseline, 30, 20.0, vec![1]),
            (StatMode::Current, 57, 44.0, vec![1, 2]),
        ] {
            let mut inputs = inputs(&catalog, &entries);
            assert!(inputs.select_mode(&catalog, mode).is_empty());
            let mut trace = Vec::new();
            let mut resolver = Resolver {
                mode,
                ctx: &ctx,
                catalog: &catalog,
                controller: &controller,
                hero_id: 999,
                steam_id: None,
                stat: HeroStat::ClipSize,
                game_time: Some(0.0),
                game_start: Some(0.0),
                explain: true,
                contributions: &mut trace,
                ignored_modifiers: BTreeMap::new(),
                inferred_bindings: BTreeSet::new(),
                unmapped_inputs: BTreeSet::new(),
            };
            // Base 20 + 0.75 * spirit; additive ammo bonuses, then ceil.
            // Signed permanent inputs remain in both modes.
            assert_eq!(resolver.ammo(&inputs).unwrap(), ammo);
            assert!(
                (resolver
                    .lifesteal(&inputs, HeroStat::BulletLifesteal)
                    .unwrap()
                    - lifesteal)
                    .abs()
                    < 1e-10
            );
            assert!(resolver.unmapped_inputs.is_empty());
            assert!(trace.iter().all(|row| row.mode == mode));
            assert_eq!(
                trace
                    .iter()
                    .filter(|row| row.input == "spirit_power" && row.kind == "percent")
                    .map(|row| row.value)
                    .collect::<Vec<_>>(),
                if mode == StatMode::Baseline {
                    vec![15.0]
                } else {
                    vec![15.0, 20.0]
                }
            );
            assert_eq!(
                trace
                    .iter()
                    .filter(|row| row.input == "spirit_power" && row.kind == "flat")
                    .filter_map(|row| row.modifier_serial)
                    .collect::<Vec<_>>(),
                serials
            );
        }
    }

    #[test]
    fn baseline_requires_owned_passive_evidence_and_keeps_unknown_roles_visible() {
        let catalog = catalog();
        let mut timed = entry(11, 2);
        timed.duration = Some(5.0);
        let mut foreign = entry(11, 3);
        foreign.parent = Some(1);
        foreign.caster = Some(2);
        let entries = [entry(11, 1), timed, foreign, entry(12, 4), entry(987654, 5)];
        let mut inputs = inputs(&catalog, &entries);
        let diagnostics = inputs.select_mode(&catalog, StatMode::Baseline);
        assert_eq!(
            inputs
                .active
                .iter()
                .map(|entry| entry.serial_number)
                .collect::<Vec<_>>(),
            [Some(1)]
        );
        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("987654") && message.contains("role is unknown"))
        );
        // Sold or removed items cannot grant a baseline effect from a stale row.
        inputs.owned.clear();
        assert!(inputs.select_mode(&catalog, StatMode::Baseline).is_empty());
        assert!(inputs.active.is_empty());
    }

    #[test]
    fn baseline_skips_unbound_conditions_and_rejects_recorded_only_gravity() {
        let mut catalog = catalog();
        catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
            "stat":"MODIFIER_VALUE_FIRE_RATE", "property_name":"ArbitraryConditionalBonus",
            "value":37, "usage_flags":"ConditionallyApplied", "modifier_keys":[]
        })];
        let ctx = Context::new(1.0 / 64.0).unwrap();
        let controller = Entity::from_fields(1, 1, 0, "test", true, Default::default()).unwrap();
        let inputs = inputs(&catalog, &[]);
        let mut trace = Vec::new();
        let mut resolver = Resolver {
            mode: StatMode::Baseline,
            ctx: &ctx,
            catalog: &catalog,
            controller: &controller,
            hero_id: 999,
            steam_id: None,
            stat: HeroStat::ClipSize,
            game_time: None,
            game_start: None,
            explain: true,
            contributions: &mut trace,
            ignored_modifiers: BTreeMap::new(),
            inferred_bindings: BTreeSet::new(),
            unmapped_inputs: BTreeSet::new(),
        };
        assert_eq!(resolver.fire_rate(&inputs).unwrap(), 0.0);
        assert!(
            resolver
                .gravity_scale()
                .unwrap_err()
                .to_string()
                .contains("baseline gravity scale is unavailable")
        );
        resolver.mode = StatMode::Current;
        assert!(
            resolver
                .fire_rate(&inputs)
                .unwrap_err()
                .to_string()
                .contains("conditional property")
        );
    }
}
