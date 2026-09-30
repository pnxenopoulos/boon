use super::*;
use serde_json::json;

#[test]
fn recorded_types_separate_multistat_sources_and_unrelated_penalties() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapScalingStats"] = json!({});
    catalog.modifiers[0].stat_changes = vec![
        json!({"stat": PERCENT}),
        json!({"stat": "MODIFIER_VALUE_TECH_RANGE_PERCENT"}),
    ];
    // Deliberately invented enum ordinals: use the selected catalog, not numbers in code.
    catalog.modifier_value_types.extend([
        (900, PERCENT.into()),
        (901, "MODIFIER_VALUE_STAMINA".into()),
        (902, "MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE".into()),
    ]);
    let ctx = Context::new(1.0 / 64.0).unwrap();
    let controller = Entity::from_fields(1, 1, 0, "test", true, Default::default()).unwrap();
    let mut trace = Vec::new();
    let mut resolver = Resolver {
        mode: StatMode::Current,
        ctx: &ctx,
        catalog: &catalog,
        controller: &controller,
        hero_id: 999,
        steam_id: None,
        game_time: None,
        game_start: None,
        explain: true,
        contributions: &mut trace,
        ignored_modifiers: BTreeMap::new(),
        inferred_bindings: BTreeSet::new(),
        unmapped_inputs: BTreeSet::new(),
    };
    let hero = &catalog.heroes[&999];
    let mut inputs = PlayerInputs {
        hero,
        level: None,
        weapon: catalog.weapon(hero).unwrap(),
        inventory: vec![],
        owned: vec![],
        active: vec![],
        permanent: vec![
            RecordedStat {
                source_id: 10,
                value_type: Some(900),
                value: 13.0,
            },
            RecordedStat {
                source_id: 12,
                value_type: Some(901),
                value: -1.0,
            },
            RecordedStat {
                source_id: 12,
                value_type: Some(902),
                value: -17.0,
            },
        ],
    };
    assert_eq!(resolver.ammo(&inputs).unwrap(), 23);
    assert_eq!(
        resolver
            .total("MODIFIER_VALUE_STAMINA", "stamina", "flat", &inputs)
            .unwrap(),
        -1.0
    );
    assert_eq!(
        resolver.recorded_stat(&inputs.permanent[2]).unwrap(),
        Some("MODIFIER_VALUE_COOLDOWN_REDUCTION_PERCENTAGE")
    );
    assert_eq!(resolver.contributions.len(), 3); // Base ammo, ammo bonus, stamina penalty.
    // A known type is still usable without a source catalog record.
    inputs.permanent[0].source_id = 987654;
    assert_eq!(resolver.ammo(&inputs).unwrap(), 23);
    assert_eq!(
        resolver.contributions.last().unwrap().source,
        "modifier:987654"
    );
    // An unknown type cannot be silently inferred from a conflicting source binding.
    inputs.permanent[0].value_type = Some(999);
    assert!(
        resolver
            .ammo(&inputs)
            .unwrap_err()
            .to_string()
            .contains("modifier value type 999")
    );
    // Missing type evidence still permits the existing unique-source fallback.
    inputs.permanent[0].value_type = None;
    inputs.permanent[0].source_id = 10;
    assert!(
        resolver
            .ammo(&inputs)
            .unwrap_err()
            .to_string()
            .contains("ambiguous permanent stat source")
    );
}

#[test]
fn ability_scoped_bonuses_do_not_enter_global_hero_stats() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapScalingStats"] = json!({});
    let baseline = stat_result(&catalog, true, HeroStat::ClipSize).0.unwrap();
    for filter in ["EApplyFilter_OnlyIfImbued", "EApplyFilter_OnlyIfHasCharges"] {
        let mut effect = json!({"stat": PERCENT, "property_name": "ScopedAmmo"});
        effect["apply_filter"] = json!(filter);
        effect["definition_path"] = json!("/test/scoped-ammo");
        effect["value"] = json!(900);
        catalog.modifiers[2].stat_changes.push(effect);
        assert_eq!(
            stat_result(&catalog, true, HeroStat::ClipSize).0.unwrap(),
            baseline
        );
        catalog.modifiers[2].stat_changes.pop();
    }
}

#[test]
fn spirit_inputs_keep_scope_and_do_not_cancel_unsupported_percentages() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapLevelInfo"] = json!({});
    catalog.modifiers[2].stat_changes = vec![
        json!({"stat": SPIRIT, "value": 8, "definition_path": "/flat"}),
        json!({"stat": SPIRIT, "value": 90, "definition_path": "/imbued",
            "apply_filter": "EApplyFilter_OnlyIfImbued"}),
        json!({"stat": "MODIFIER_VALUE_TECH_POWER_PERCENT", "value": 40,
            "definition_path": "/imbued-percent", "apply_filter": "EApplyFilter_OnlyIfImbued"}),
    ];
    let (result, trace, _) = stat_result(&catalog, true, HeroStat::ClipSize);
    assert_eq!(result.unwrap(), 26.0); // 20 base + 8 global spirit * 0.75.
    assert!(!trace.iter().any(|c| c.definition_path.contains("imbued")));
    catalog.modifiers[2].stat_changes.extend([
        json!({"stat": "MODIFIER_VALUE_TECH_POWER_PERCENT", "value": 15,
            "definition_path": "/positive"}),
        json!({"stat": "MODIFIER_VALUE_TECH_POWER_PERCENT", "value": -15,
            "definition_path": "/negative"}),
    ]);
    let (result, trace, _) = stat_result(&catalog, true, HeroStat::ClipSize);
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("percentage spirit-power")
    );
    assert_eq!(
        trace
            .iter()
            .filter(|c| c.input == "spirit_power" && c.kind == "percent")
            .map(|c| c.value)
            .collect::<Vec<_>>(),
        [15.0, -15.0]
    );
    // A flat-value failure still leaves the separate percentage inputs visible.
    catalog.modifiers[2].stat_changes[0]["value"] = json!("unknown");
    let (result, trace, _) = stat_result(&catalog, true, HeroStat::ClipSize);
    assert!(result.is_err());
    assert_eq!(
        trace
            .iter()
            .filter(|c| c.input == "spirit_power" && c.kind == "percent")
            .count(),
        2
    );
}

#[test]
fn absent_catalog_states_exclude_old_modifiers_but_missing_evidence_does_not() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("modifiers.json");
    let mut file: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    file["modifier_states"] = json!({"35":"MODIFIER_STATE_TEST", "70":"MODIFIER_STATE_OTHER"});
    file["records"][2]["definition"]["m_nEnabledStateMask"] =
        json!("MODIFIER_STATE_TEST | MODIFIER_STATE_OTHER");
    std::fs::write(path, serde_json::to_vec(&file).unwrap()).unwrap();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let source = catalog.modifier(12, None).unwrap();
    assert!(modifier_states_absent(source, &catalog, &[0, 0, 0]));
    for evidence in [&[0, 8, 0][..], &[0, 0, 64], &[0], &[]] {
        assert!(!modifier_states_absent(source, &catalog, evidence));
    }
    // A new or unavailable enum mapping cannot establish inactivity.
    catalog.modifier_states.remove("MODIFIER_STATE_OTHER");
    assert!(!modifier_states_absent(
        catalog.modifier(12, None).unwrap(),
        &catalog,
        &[0, 0, 0]
    ));
    // Ordinary modifiers without declared states keep their normal lifetime.
    assert!(!modifier_states_absent(
        catalog.modifier(10, None).unwrap(),
        &catalog,
        &[0, 0, 0]
    ));
}

fn stat_result(
    catalog: &StatCatalog,
    active: bool,
    stat: HeroStat,
) -> (Result<f64>, Vec<Contribution>, BTreeMap<u32, String>) {
    stat_result_with_permanent(catalog, active, stat, vec![])
}

fn stat_result_with_permanent(
    catalog: &StatCatalog,
    active: bool,
    stat: HeroStat,
    permanent: Vec<(u32, f64)>,
) -> (Result<f64>, Vec<Contribution>, BTreeMap<u32, String>) {
    let mut inferred = BTreeSet::new();
    let modifier = CModifierTableEntry {
        modifier_subclass: Some(12),
        serial_number: Some(42),
        last_applied_time: Some(10.0),
        duration: Some(5.0),
        ..Default::default()
    };
    stat_result_with_modifier(
        catalog,
        stat,
        permanent,
        active.then_some(&modifier),
        true,
        &mut inferred,
    )
}

fn stat_result_with_modifier(
    catalog: &StatCatalog,
    stat: HeroStat,
    permanent: Vec<(u32, f64)>,
    modifier: Option<&CModifierTableEntry>,
    owned: bool,
    inferred: &mut BTreeSet<String>,
) -> (Result<f64>, Vec<Contribution>, BTreeMap<u32, String>) {
    let ctx = Context::new(1.0 / 64.0).unwrap();
    let controller = Entity::from_fields(1, 1, 0, "test", true, Default::default()).unwrap();
    let mut contributions = Vec::new();
    let mut resolver = Resolver {
        mode: StatMode::Current,
        ctx: &ctx,
        catalog,
        controller: &controller,
        hero_id: 999,
        steam_id: None,
        game_time: Some(12.0),
        game_start: Some(0.0),
        explain: true,
        contributions: &mut contributions,
        ignored_modifiers: BTreeMap::new(),
        inferred_bindings: BTreeSet::new(),
        unmapped_inputs: BTreeSet::new(),
    };
    let hero = &catalog.heroes[&999];
    let weapon = catalog.weapon(hero).unwrap();
    let inputs = PlayerInputs {
        hero,
        level: Some(5.0),
        weapon,
        inventory: vec![weapon],
        owned: if owned { vec![weapon] } else { vec![] },
        active: modifier.into_iter().collect(),
        permanent: permanent
            .into_iter()
            .map(|(source_id, value)| RecordedStat {
                source_id,
                value_type: None,
                value,
            })
            .collect(),
    };
    let result = match stat {
        HeroStat::ClipSize => resolver.ammo(&inputs).map(f64::from),
        HeroStat::BulletVelocity => resolver.bullet_velocity(&inputs),
        HeroStat::WeaponDamage => resolver.weapon_damage(&inputs),
        HeroStat::MeleeDistance => resolver.melee_distance(&inputs),
        HeroStat::LightMeleeDamage | HeroStat::HeavyMeleeDamage => {
            resolver.melee_damage(&inputs, stat)
        }
        HeroStat::ReloadTime => resolver.reload_time(&inputs),
        HeroStat::FireRate => resolver.fire_rate(&inputs),
        HeroStat::SlideDistance | HeroStat::BulletEvasion => {
            resolver.movement_percent(&inputs, stat)
        }
        HeroStat::GravityScale => resolver.gravity_scale(),
        HeroStat::Stamina => resolver.stamina(&inputs),
        HeroStat::DebuffResist => resolver.debuff_resist(&inputs),
        HeroStat::BulletResist | HeroStat::SpiritResist | HeroStat::MeleeResist => {
            resolver.resistance(&inputs, stat)
        }
        HeroStat::BulletLifesteal | HeroStat::SpiritLifesteal => resolver.lifesteal(&inputs, stat),
        HeroStat::MeleeLifesteal => resolver.melee_lifesteal(&inputs),
        HeroStat::MoveSpeed | HeroStat::SprintSpeed => resolver.speed(&inputs, stat),
        HeroStat::StaminaCooldown => resolver.stamina_cooldown(&inputs),
        HeroStat::DashSpeed
        | HeroStat::DashDuration
        | HeroStat::AirDashSpeed
        | HeroStat::AirDashDuration => resolver.dash(&inputs, stat),
        HeroStat::FalloffStart | HeroStat::FalloffEnd => resolver.falloff_range(&inputs, stat),
    };
    *inferred = resolver.inferred_bindings;
    inferred.extend(resolver.unmapped_inputs);
    let ignored = resolver.ignored_modifiers;
    (result, contributions, ignored)
}

#[test]
fn resistance_uses_catalog_growth_and_separately_combines_recipient_shred() {
    let folder = super::super::catalog::tests::fixture();
    let modifier_path = folder.path().join("modifiers.json");
    let mut records: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    for (stat, key, symbol, reduction) in [
        (
            HeroStat::BulletResist,
            "EBulletArmorDamageReduction",
            "MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST",
            "MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION",
        ),
        (
            HeroStat::SpiritResist,
            "ETechArmorDamageReduction",
            "MODIFIER_VALUE_TECH_RESIST",
            "MODIFIER_VALUE_TECH_RESIST_REDUCTION",
        ),
        (
            HeroStat::MeleeResist,
            "EMeleeResist",
            "MODIFIER_VALUE_MELEE_RESIST",
            "MODIFIER_VALUE_MELEE_RESIST_REDUCTION",
        ),
    ] {
        let resist = json!({"stat":symbol,"value":20,"definition_path":"/resist"});
        records["records"][2]["stat_changes"] = json!([
            resist, resist,
            {"stat":reduction,"value":-25,"definition_path":"/shred1"},
            {"stat":reduction,"value":-20,"definition_path":"/shred2"},
            {"stat":"MODIFIER_VALUE_INCOMING_DAMAGE_PERCENTAGE","value":-99,"definition_path":"/damage"},
            {"stat":"MODIFIER_VALUE_BULLET_RESIST_NON_HERO","value":99,"definition_path":"/npc"}
        ]);
        std::fs::write(&modifier_path, serde_json::to_vec(&records).unwrap()).unwrap();
        let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
        hero["m_mapStartingStats"] = json!({key:10,"ETechPower":20});
        hero["m_mapScalingStats"] = json!({key:{"eScalingStat":"ETechPower","flScale":0.25}});
        hero["m_mapStandardLevelUpUpgrades"] = json!({symbol:2});
        hero["m_mapLevelInfo"] = json!({"1":{"m_bUseStandardUpgrade":true},"3":{"m_bUseStandardUpgrade":true},"5":{"m_bUseStandardUpgrade":true},"6":{"m_bUseStandardUpgrade":true}});
        // Owning an offensive ability does not apply its shred to the owner.
        catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
            "stat":reduction,"value":-90,"usage_flags":"IntrinsicallyProvidedInAbility"
        })];
        let (inactive, _, ignored) = stat_result(&catalog, false, stat);
        assert_eq!(inactive.unwrap(), 21.0); // 10 base + 5 spirit + 6 boon.
        assert!(ignored.is_empty());
        let (active, trace, ignored) = stat_result(&catalog, true, stat);
        assert!((active.unwrap() + 3.2).abs() < 1e-12); // (21 + 20 * .79) - 40.
        assert!(ignored.is_empty());
        assert_eq!(
            trace
                .iter()
                .filter(|row| row.kind == "percent" && row.input == stat.as_str())
                .count(),
            1
        );
        assert_eq!(
            trace.iter().filter(|row| row.kind == "reduction").count(),
            2
        );
        // A missing zero base must not suppress explicit hero scaling.
        catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert_eq!(stat_result(&catalog, false, stat).0.unwrap(), 11.0);
        let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
        hero["m_mapStartingStats"] = json!({key:-15});
        hero["m_mapScalingStats"] = json!({});
        hero["m_mapStandardLevelUpUpgrades"] = json!({});
        assert_eq!(stat_result(&catalog, false, stat).0.unwrap(), -15.0);
        catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"][key] = json!("bad");
        assert!(stat_result(&catalog, false, stat).0.is_err());
    }
}

#[test]
fn melee_component_does_not_include_shared_weapon_resistance_or_shred() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("modifiers.json");
    let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    records["records"][2]["stat_changes"] = json!([
        {"stat":"MODIFIER_VALUE_BULLET_ARMOR_DAMAGE_RESIST","value":40,"definition_path":"/bullet"},
        {"stat":"MODIFIER_VALUE_BULLET_AND_MELEE_RESIST_REDUCTION","value":-25,"definition_path":"/shared_shred"},
        {"stat":"MODIFIER_VALUE_MELEE_RESIST","value":20,"definition_path":"/melee"},
        {"stat":"MODIFIER_VALUE_MELEE_RESIST_REDUCTION","value":-10,"definition_path":"/melee_shred"}
    ]);
    std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
    hero["m_mapStartingStats"] = json!({});
    hero["m_mapScalingStats"] = json!({});
    assert_eq!(
        stat_result(&catalog, true, HeroStat::BulletResist)
            .0
            .unwrap(),
        15.0
    );
    assert_eq!(
        stat_result(&catalog, true, HeroStat::SpiritResist)
            .0
            .unwrap(),
        0.0
    );
    assert_eq!(
        stat_result(&catalog, true, HeroStat::MeleeResist)
            .0
            .unwrap(),
        10.0
    );
}

#[test]
fn resistance_reports_unbound_bonuses_and_rejects_unknown_reduction_signs() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("modifiers.json");
    let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    records["records"][2]["stat_changes"] = json!([
        {"stat":"MODIFIER_VALUE_TECH_RESIST_REDUCTION","value":20,"definition_path":"/positive"}
    ]);
    std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] = json!({});
    catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
        "stat":"MODIFIER_VALUE_TECH_RESIST","value":27,"property_name":"ConditionalArmor","modifier_keys":[]
    })];
    let mut diagnostics = BTreeSet::new();
    let (value, trace, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::SpiritResist,
        vec![],
        None,
        true,
        &mut diagnostics,
    );
    assert_eq!(value.unwrap(), 0.0);
    assert!(trace.is_empty());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.contains("ConditionalArmor") && d.contains("no modifier binding"))
    );
    assert!(
        stat_result(&catalog, true, HeroStat::SpiritResist)
            .0
            .unwrap_err()
            .to_string()
            .contains("positive resistance-reduction")
    );
}

#[test]
fn empty_unbound_resistance_declarations_are_not_effects() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] = json!({});
    catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
        "kind":"ability_property","stat":"MODIFIER_VALUE_TECH_RESIST",
        "property_name":"EmptyResist","value":null,"raw_value":null,
        "modifier_keys":[],"usage_flags":"","scaling":null
    })];
    let mut diagnostics = BTreeSet::new();
    let (value, trace, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::SpiritResist,
        vec![],
        None,
        true,
        &mut diagnostics,
    );
    assert_eq!(value.unwrap(), 0.0);
    assert!(trace.is_empty() && diagnostics.is_empty());
    // An explicit invalid value is not the same as an empty declaration.
    catalog.abilities.get_mut(&123).unwrap().stat_changes[0]["raw_value"] = json!("bad");
    assert!(
        stat_result(&catalog, false, HeroStat::SpiritResist)
            .0
            .is_err()
    );
    catalog.abilities.get_mut(&123).unwrap().stat_changes[0]["raw_value"] = Value::Null;
    // An upgrade turns this into a potential input with a missing base.
    catalog.abilities.get_mut(&123).unwrap().definition["m_vecAbilityUpgrades"] = json!([
        {"m_vecPropertyUpgrades":[{"m_strPropertyName":"EmptyResist","m_strBonus":"10"}]}
    ]);
    assert!(
        stat_result(&catalog, false, HeroStat::SpiritResist)
            .0
            .is_err()
    );
}

#[test]
fn upgraded_resistance_reduction_requires_its_caster() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("modifiers.json");
    let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    records["records"][2]["ability_id"] = json!(123);
    records["records"][2]["stat_changes"] = json!([
        {"stat":"MODIFIER_VALUE_TECH_RESIST_REDUCTION","value":-10,"property_name":"Shred","definition_path":"/shred"}
    ]);
    std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] = json!({});
    catalog.abilities.get_mut(&123).unwrap().definition["m_vecAbilityUpgrades"] = json!([
        {"m_vecPropertyUpgrades":[{"m_strPropertyName":"Shred","m_strBonus":"-5"}]}
    ]);
    assert!(
        stat_result(&catalog, true, HeroStat::SpiritResist)
            .0
            .unwrap_err()
            .to_string()
            .contains("caster upgrades")
    );
}

#[test]
fn lifesteal_combines_catalog_sources_and_keeps_damage_types_separate() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("modifiers.json");
    let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let scaling = json!({"$type":"subclass","$value":{
        "_class":"scale_function_single_stat","m_eSpecificStatScaleType":"EHealingOutput"
    }});
    let bullet =
        json!({"stat":BULLET_LIFESTEAL,"value":22,"definition_path":"/bullet","scaling":scaling});
    let spirit =
        json!({"stat":SPIRIT_LIFESTEAL,"value":30,"definition_path":"/spirit","scaling":scaling});
    records["records"][2]["stat_changes"] = json!([bullet, bullet, spirit]);
    std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] =
        json!({"EBulletLifesteal":30,"ETechLifesteal":10});
    for (stat, innate, combined) in [
        (HeroStat::BulletLifesteal, 30.0, 45.4),
        (HeroStat::SpiritLifesteal, 10.0, 37.0),
    ] {
        assert_eq!(stat_result(&catalog, false, stat).0.unwrap(), innate);
        let (value, trace, ignored) = stat_result(&catalog, true, stat);
        assert!((value.unwrap() - combined).abs() < 1e-12);
        assert_eq!(trace.len(), 2); // Innate plus one deduplicated active source.
        assert!(trace.iter().all(|row| row.input == stat.as_str()));
        assert!(ignored.is_empty());
    }
    let stats = &mut catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"];
    *stats = json!({});
    assert_eq!(
        stat_result(&catalog, false, HeroStat::BulletLifesteal)
            .0
            .unwrap(),
        0.0
    );
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"]["EBulletLifesteal"] =
        json!("bad");
    assert!(
        stat_result(&catalog, false, HeroStat::BulletLifesteal)
            .0
            .is_err()
    );

    records["records"][2]["stat_changes"][0]["scaling"]["$value"]["m_eSpecificStatScaleType"] =
        json!("ETechPower");
    std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] = json!({});
    assert!(
        stat_result(&catalog, true, HeroStat::BulletLifesteal)
            .0
            .unwrap_err()
            .to_string()
            .contains("unsupported property scaling")
    );
}

#[test]
fn unbound_property_is_diagnosed_without_assuming_activation() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapStartingStats"] = json!({});
    for flags in [
        Value::Null,
        json!(""),
        json!("ConditionallyApplied"),
        json!("IntrinsicallyProvidedInAbility"),
    ] {
        catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
            "stat":BULLET_LIFESTEAL,"value":29,"property_name":"ArbitraryLifesteal",
            "usage_flags":flags,"modifier_keys":[],"definition_path":"/passive"
        })];
        let mut diagnostics = BTreeSet::new();
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::BulletLifesteal,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        let intrinsic = flags == "IntrinsicallyProvidedInAbility";
        assert_eq!(value.unwrap(), if intrinsic { 29.0 } else { 0.0 });
        assert_eq!(trace.len(), usize::from(intrinsic));
        assert_eq!(diagnostics.is_empty(), intrinsic);
        if !intrinsic {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.contains("ArbitraryLifesteal") && d.contains("no modifier binding"))
            );
        }
    }
}

#[test]
fn melee_lifesteal_uses_the_exact_passive_property_and_excludes_healing_procs() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for amount in [18, 31] {
        catalog.abilities.get_mut(&123).unwrap().definition = json!({
            "m_WeaponInfo":{"m_iClipSize":20},
            "m_eAbilityActivation":"CITADEL_ABILITY_ACTIVATION_PASSIVE",
            "m_mapAbilityProperties":{
                "MeleeLifesteal":{"m_strValue":amount,"m_subclassScaleFunction":{"$value":{
                    "_class":"scale_function_single_stat","m_eSpecificStatScaleType":"EHealingOutput"
                }}},
                "LifestealHealPercent":{"m_strValue":95},
                "LifestealHeal":{"m_strValue":900},
                "LifestrikeHealPercent":{"m_strValue":80},
                "NotMeleeLifesteal":{"m_strValue":99}
            }
        });
        let mut diagnostics = BTreeSet::new();
        let (value, trace, ignored) = stat_result_with_modifier(
            &catalog,
            HeroStat::MeleeLifesteal,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), f64::from(amount));
        assert_eq!(trace.len(), 1);
        assert!(trace[0].definition_path.ends_with("/MeleeLifesteal"));
        assert!(ignored.is_empty());
        assert!(diagnostics.iter().any(|d| d.contains("property rule")));
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::MeleeLifesteal,
            vec![],
            None,
            false,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 0.0);
        assert!(trace.is_empty() && diagnostics.is_empty());
    }
    catalog.abilities.get_mut(&123).unwrap().definition["m_eAbilityActivation"] =
        json!("CITADEL_ABILITY_ACTIVATION_INSTANT_CAST");
    let mut diagnostics = BTreeSet::new();
    let (value, trace, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::MeleeLifesteal,
        vec![],
        None,
        true,
        &mut diagnostics,
    );
    assert_eq!(value.unwrap(), 0.0);
    assert!(trace.is_empty());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.contains("no passive activation binding"))
    );
    catalog.abilities.get_mut(&123).unwrap().definition["m_mapAbilityProperties"]["TargetLifesteal"] =
        json!({"m_strValue":75});
    assert!(
        stat_result(&catalog, false, HeroStat::MeleeLifesteal)
            .0
            .unwrap_err()
            .to_string()
            .contains("requires target and damage-type context")
    );
}

#[test]
fn debuff_resist_uses_catalog_innate_bound_and_recorded_values_once() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    let symbol = "MODIFIER_VALUE_STATUS_RESISTANCE";
    let effect = json!({"stat":symbol,"value":37,"property_name":"ArbitraryResistance",
        "definition_path":"/test_gun/resistance","modifier_keys":["abilities#/second"]});
    abilities["records"][0]["stat_changes"] = json!([effect]);
    modifiers["records"][2]["stat_changes"] = json!([effect]);
    modifiers["records"][0]["stat_changes"] = json!([{"stat":symbol,"value":1}]);
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    for (stats, innate) in [(json!({}), 0.0), (json!({"EDebuffResist":-13}), -13.0)] {
        heroes["records"][0]["definition"]["m_mapStartingStats"] = stats;
        std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let (inactive, _, _) = stat_result(&catalog, false, HeroStat::DebuffResist);
        assert_eq!(inactive.unwrap(), innate);
        let (active, trace, _) =
            stat_result_with_permanent(&catalog, true, HeroStat::DebuffResist, vec![(10, 10.0)]);
        let expected = 100.0 * (1.0 - (1.0 - innate / 100.0) * 0.9 * 0.63);
        assert!((active.unwrap() - expected).abs() < 1e-12);
        assert_eq!(
            trace
                .iter()
                .filter(|r| r.modifier_serial == Some(42))
                .count(),
            1
        );
        assert!(
            trace
                .iter()
                .any(|r| r.value == 10.0 && r.definition_path.ends_with("m_flValue"))
        );
        if innate != 0.0 {
            assert!(
                trace
                    .iter()
                    .any(|r| r.value == innate && r.definition_path.ends_with("/EDebuffResist"))
            );
        }
    }
    heroes["records"][0]["definition"]["m_mapStartingStats"]["EDebuffResist"] = json!("invalid");
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, false, HeroStat::DebuffResist)
            .0
            .unwrap_err()
            .to_string()
            .contains("invalid base")
    );
}

#[test]
fn unbound_debuff_resist_condition_stays_partial() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let ability_path = folder.path().join("abilities.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({});
    abilities["records"][0]["stat_changes"] = json!([
        {"stat":"MODIFIER_VALUE_STATUS_RESISTANCE","value":20,"property_name":"ConditionalResistance",
         "modifier_keys":[],"usage_flags":"ConditionallyApplied"}
    ]);
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let mut diagnostics = BTreeSet::new();
    let (value, _, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::DebuffResist,
        vec![],
        None,
        true,
        &mut diagnostics,
    );
    assert_eq!(value.unwrap(), 0.0);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.contains("ConditionalResistance") && d.contains("no modifier binding"))
    );
}

#[test]
fn movement_uses_each_bound_bonus_and_normalizes_units() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] =
        json!({"EMaxMoveSpeed":6.4,"ESprintSpeed":1.6});
    modifiers["records"][2]["stat_changes"] = json!([
        {"stat":MOVE_SPEED,"raw_value":"2m","definition_path":"/first"},
        {"stat":MOVE_SPEED,"raw_value":"2m","definition_path":"/first"},
        {"stat":MOVE_SPEED,"value":3.0/rulesets::METERS_PER_SOURCE_UNIT,"definition_path":"/second"},
        {"stat":SPRINT_SPEED,"raw_value":"2m","definition_path":"/sprint_one"},
        {"stat":SPRINT_SPEED,"raw_value":"1.5m","definition_path":"/sprint_two"},
        {"stat":"MODIFIER_VALUE_MOVEMENT_SPEED_SLOW_PERCENT","value":90},
        {"stat":"MODIFIER_VALUE_MOVE_SPEED_LIMIT","value":1},
        {"stat":"MODIFIER_VALUE_SPRINT_ACCELERATION","value":800}
    ]);
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for (stat, base, expected) in [
        (HeroStat::MoveSpeed, 6.4, 10.9),
        (HeroStat::SprintSpeed, 1.6, 5.1),
    ] {
        assert_eq!(stat_result(&catalog, false, stat).0.unwrap(), base);
        let (value, trace, ignored) = stat_result(&catalog, true, stat);
        assert!((value.unwrap() - expected).abs() < 1e-12);
        assert!(ignored.is_empty());
        let flat: Vec<_> = trace.iter().filter(|row| row.kind == "flat").collect();
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].modifier_serial, Some(42));
        assert_eq!(flat[0].value, 2.0);
    }
    modifiers["records"][2]["stat_changes"].as_array_mut().unwrap().push(json!({
        "stat":"MODIFIER_VALUE_MOVEMENT_SPEED_MAX_PERCENT","value":70,"definition_path":"/percentage"
    }));
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!((stat_result(&catalog, true, HeroStat::MoveSpeed).0.unwrap() - 18.53).abs() < 1e-12);
    assert!(
        (stat_result(&catalog, true, HeroStat::SprintSpeed)
            .0
            .unwrap()
            - 5.1)
            .abs()
            < 1e-12
    );
    modifiers["records"][2]["stat_changes"].as_array_mut().unwrap().push(json!({
        "stat":"MODIFIER_VALUE_MOVEMENT_SPEED_MAX_PERCENT","value":20,"definition_path":"/another_percentage"
    }));
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, true, HeroStat::MoveSpeed)
            .0
            .unwrap_err()
            .to_string()
            .contains("combining movement-speed percentages")
    );
}

#[test]
fn movement_and_sprint_spirit_scaling_are_data_driven() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("heroes.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] =
        json!({"EMaxMoveSpeed":6,"ESprintSpeed":1,"ETechPower":20});
    heroes["records"][0]["definition"]["m_mapLevelInfo"] = json!({});
    heroes["records"][0]["definition"]["m_mapScalingStats"] = json!({
        "EMaxMoveSpeed":{"eScalingStat":"ETechPower","flScale":0.1},
        "ESprintSpeed":{"eScalingStat":"ETechPower","flScale":0.2}
    });
    std::fs::write(&path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for (stat, expected, scale) in [
        (HeroStat::MoveSpeed, 8.0, 2.0),
        (HeroStat::SprintSpeed, 5.0, 4.0),
    ] {
        let (value, trace, _) = stat_result(&catalog, false, stat);
        assert_eq!(value.unwrap(), expected);
        let row = trace.iter().find(|r| r.kind == "spirit_flat").unwrap();
        assert_eq!(row.value, scale);
        assert!(row.definition_path.contains("m_mapScalingStats"));
    }
}

#[test]
fn sprint_powerups_and_recorded_totals_use_source_units() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"ESprintSpeed":1.6});
    modifiers["records"][0]["stat_changes"] = json!([{"stat":SPRINT_SPEED,"value":1}]);
    modifiers["records"][2]["definition"] = json!({"m_flTimeMin":0,"m_flTimeMax":1});
    modifiers["records"][2]["stat_changes"] = json!([
        {"stat":SPRINT_SPEED,"value_min":2.0/rulesets::METERS_PER_SOURCE_UNIT,"value_max":7.0/rulesets::METERS_PER_SOURCE_UNIT,"definition_path":"/powerup"}
    ]);
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let entry = CModifierTableEntry {
        modifier_subclass: Some(12),
        ability_subclass: Some(456),
        serial_number: Some(42),
        last_applied_time: Some(12.0),
        duration: Some(60.0),
        ..Default::default()
    };
    let (value, trace, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::SprintSpeed,
        vec![],
        Some(&entry),
        true,
        &mut BTreeSet::new(),
    );
    assert!((value.unwrap() - 4.6).abs() < 1e-12);
    assert!((trace.last().unwrap().value - 3.0).abs() < 1e-12);
    let (value, trace, _) =
        stat_result_with_permanent(&catalog, false, HeroStat::SprintSpeed, vec![(10, 100.0)]);
    assert!((value.unwrap() - 4.14).abs() < 1e-12);
    assert!((trace.last().unwrap().value - 2.54).abs() < 1e-12);
}

#[test]
fn nested_proc_buffs_are_not_required_intrinsic_effects() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let ability_path = folder.path().join("abilities.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"EMaxMoveSpeed":6.4});
    abilities["records"][0]["stat_changes"] = json!([
        {"stat":MOVE_SPEED,"raw_value":"2m","modifier_keys":["abilities#/test_gun/m_AutoIntrinsicModifiers/0/m_ProcBuff"]}
    ]);
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let mut diagnostics = BTreeSet::new();
    let (value, _, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::MoveSpeed,
        vec![],
        None,
        true,
        &mut diagnostics,
    );
    assert_eq!(value.unwrap(), 6.4);
    assert!(diagnostics.is_empty());
}

#[test]
fn runtime_property_requires_an_active_modifier_and_its_ability_entity() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"ESprintSpeed":1.6});
    modifiers["records"][2]["stat_changes"] = json!([
        {"stat":SPRINT_SPEED,"raw_value":"0.27m","runtime_count":"m_iArbitraryCounter"}
    ]);
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let (inactive, _, _) = stat_result(&catalog, false, HeroStat::SprintSpeed);
    assert_eq!(inactive.unwrap(), 1.6);
    let (active, _, _) = stat_result(&catalog, true, HeroStat::SprintSpeed);
    assert!(
        active
            .unwrap_err()
            .to_string()
            .contains("counter has no owning ability entity")
    );
}

#[test]
fn unused_passive_speed_declaration_is_ignored_but_conditions_are_reported() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"ESprintSpeed":1.6});
    modifiers["records"][2]["ability_id"] = json!(123);
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    for flags in ["", "ConditionallyApplied"] {
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":SPRINT_SPEED,"raw_value":"1m","modifier_keys":[],
             "property_name":"OldSprintBonus","usage_flags":flags}
        ]);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for active in [false, true] {
            let entry = CModifierTableEntry {
                modifier_subclass: Some(12),
                ..Default::default()
            };
            let mut diagnostics = BTreeSet::new();
            let (value, _, _) = stat_result_with_modifier(
                &catalog,
                HeroStat::SprintSpeed,
                vec![],
                active.then_some(&entry),
                true,
                &mut diagnostics,
            );
            assert_eq!(value.unwrap(), 1.6);
            assert_eq!(diagnostics.is_empty(), flags.is_empty());
        }
    }
}

#[test]
fn unbound_display_alias_warns_without_assuming_a_stacking_rule() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let ability_path = folder.path().join("abilities.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"ESprintSpeed":1.6});
    abilities["records"][0]["definition"]["m_mapAbilityProperties"] = json!({
        "Declared":{"m_eProvidedPropertyType":SPRINT_SPEED},
        "ArbitraryEarnedValue":{"m_strLocTokenOverride":"Declared","m_strValue":"0.15m"}
    });
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let mut diagnostics = BTreeSet::new();
    let (value, _, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::SprintSpeed,
        vec![],
        None,
        true,
        &mut diagnostics,
    );
    assert_eq!(value.unwrap(), 1.6);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.contains("ArbitraryEarnedValue") && d.contains("no stat binding"))
    );
    diagnostics.clear();
    stat_result_with_modifier(
        &catalog,
        HeroStat::SprintSpeed,
        vec![],
        None,
        false,
        &mut diagnostics,
    )
    .0
    .unwrap();
    assert!(diagnostics.is_empty());
    assert!(stat_result(&catalog, false, HeroStat::MoveSpeed).0.is_err());
}

#[test]
fn metric_property_and_upgrade_values_require_a_speed_symbol() {
    for stat in [MOVE_SPEED, SPRINT_SPEED] {
        let base = speed_property_number(&json!("2.5m"), stat).unwrap();
        let upgrade = speed_property_number(&json!("-0.75m"), stat).unwrap();
        assert!((modifier_units(stat, base + upgrade) - 1.75).abs() < 1e-12);
        assert_eq!(
            modifier_units(stat, speed_property_number(&json!(100), stat).unwrap()),
            2.54
        );
        for raw in ["2.5 m/s", "1m + 2m", "bad"] {
            assert!(speed_property_number(&json!(raw), stat).is_none());
        }
    }
    assert!(speed_property_number(&json!("2m"), FLAT).is_none());
    assert_eq!(modifier_units(FLAT, 100.0), 100.0);
}

#[test]
fn stamina_and_dashes_use_catalog_values_and_active_bindings() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    // Invented values ensure these rules do not depend on live hero buckets.
    heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({
        "EStamina":6.25, "EStaminaRegenPerSecond":0.4,
        "EGroundDashDistanceInMeters":12, "EGroundDashDuration":0.8,
        "EAirDashDistanceInMeters":9, "EAirDashDuration":0.3
    });
    modifiers["records"][2]["stat_changes"] = json!([
        {"stat":"MODIFIER_VALUE_STAMINA","value":2.5,"definition_path":"/capacity"},
        {"stat":"MODIFIER_VALUE_STAMINA","value":2.5,"definition_path":"/capacity"},
        {"stat":"MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_PERCENTAGE","value":25,"definition_path":"/recovery"},
        {"stat":"MODIFIER_VALUE_MOVEMENT_GROUND_DASH_REDUCTION_PERCENT","value":-20,"definition_path":"/ground"},
        {"stat":"MODIFIER_VALUE_AIR_MOVE_DISTANCE_INCREASE_PERCENT","value":50,"definition_path":"/air"},
        {"stat":"MODIFIER_VALUE_AIR_CONTROL_PERCENT","value":900,"definition_path":"/control"}
    ]);
    // Unlimited air dashes do not grant unlimited stamina capacity.
    modifiers["records"][2]["definition"]["m_nEnabledStateMask"] =
        json!("MODIFIER_STATE_UNLIMITED_AIR_DASHES");
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for (stat, inactive, active) in [
        (HeroStat::Stamina, 6.25, 8.75),
        (HeroStat::StaminaCooldown, 2.5, 2.0),
        (HeroStat::DashSpeed, 15.0, 12.0),
        (HeroStat::DashDuration, 0.8, 0.8),
        (HeroStat::AirDashSpeed, 30.0, 45.0),
        (HeroStat::AirDashDuration, 0.3, 0.3),
    ] {
        assert!((stat_result(&catalog, false, stat).0.unwrap() - inactive).abs() < 1e-12);
        let (value, trace, ignored) = stat_result(&catalog, true, stat);
        assert!((value.unwrap() - active).abs() < 1e-12);
        assert!(ignored.is_empty());
        let effects: Vec<_> = trace
            .iter()
            .filter(|r| r.modifier_serial.is_some())
            .collect();
        if !matches!(stat, HeroStat::DashDuration | HeroStat::AirDashDuration) {
            assert_eq!(effects.len(), 1);
            assert_eq!(effects[0].modifier_serial, Some(42));
        }
    }
    modifiers["records"][2]["definition"]["m_nEnabledStateMask"] =
        json!("OTHER_STATE | MODIFIER_STATE_STAMINA_REGEN_PAUSED");
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, true, HeroStat::StaminaCooldown)
            .0
            .unwrap_err()
            .to_string()
            .contains("recovery is paused")
    );
    assert_eq!(
        stat_result(&catalog, false, HeroStat::StaminaCooldown)
            .0
            .unwrap(),
        2.5
    );
    assert_eq!(
        stat_result(&catalog, true, HeroStat::Stamina).0.unwrap(),
        8.75
    );
}

#[test]
fn movement_percentages_do_not_guess_stacking_or_recipient_bonuses() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({
        "EStamina":5, "EStaminaRegenPerSecond":0.5,
        "EGroundDashDistanceInMeters":12, "EGroundDashDuration":0.8,
        "EAirDashDistanceInMeters":9, "EAirDashDuration":0.3
    });
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    for (stat, symbol, base) in [
        (
            HeroStat::StaminaCooldown,
            "MODIFIER_VALUE_STAMINA_REGEN_PER_SECOND_PERCENTAGE",
            2.0,
        ),
        (
            HeroStat::DashSpeed,
            "MODIFIER_VALUE_MOVEMENT_GROUND_DASH_INCREASE_PERCENT",
            15.0,
        ),
        (
            HeroStat::AirDashSpeed,
            "MODIFIER_VALUE_AIR_MOVE_DISTANCE_INCREASE_PERCENT",
            30.0,
        ),
    ] {
        modifiers["records"][2]["stat_changes"] = json!([
            {"stat":symbol,"value":20,"definition_path":"/one"},
            {"stat":symbol,"value":30,"definition_path":"/two"}
        ]);
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, true, stat)
                .0
                .unwrap_err()
                .to_string()
                .contains("combining")
        );
        assert!((stat_result(&catalog, false, stat).0.unwrap() - base).abs() < 1e-12);
        modifiers["records"][2]["stat_changes"] = json!([]);
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":symbol,"value":70,"property_name":"Unbound","usage_flags":"ConditionallyApplied"}
        ]);
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let mut diagnostic = BTreeSet::new();
        let (value, _, _) =
            stat_result_with_modifier(&catalog, stat, vec![], None, true, &mut diagnostic);
        assert!((value.unwrap() - base).abs() < 1e-12);
        assert!(diagnostic.iter().any(|d| d.contains("no modifier binding")));
        abilities["records"][0]["stat_changes"] = json!([]);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    }
}

#[test]
fn missing_intrinsic_capacity_is_reported_without_inventing_a_modifier() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"EStamina":5});
    abilities["records"][0]["stat_changes"] = json!([
        {"stat":"MODIFIER_VALUE_STAMINA","value":2,"modifier_keys":["abilities#/test_gun/m_AutoIntrinsicModifiers/0"]}
    ]);
    modifiers["records"][2]["record_key"] = json!("abilities#/test_gun/m_AutoIntrinsicModifiers/0");
    modifiers["records"][2]["stat_changes"] = json!([
        {"stat":"MODIFIER_VALUE_STAMINA","value":2}
    ]);
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let mut diagnostic = BTreeSet::new();
    let (value, _, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::Stamina,
        vec![],
        None,
        true,
        &mut diagnostic,
    );
    assert_eq!(value.unwrap(), 5.0);
    assert!(
        diagnostic
            .iter()
            .any(|d| d.contains("no effective intrinsic modifier"))
    );
    let entry = CModifierTableEntry {
        modifier_subclass: Some(12),
        ability_subclass: Some(456),
        serial_number: Some(42),
        ..Default::default()
    };
    let (value, _, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::Stamina,
        vec![],
        Some(&entry),
        true,
        &mut diagnostic,
    );
    assert_eq!(value.unwrap(), 7.0);
    assert!(diagnostic.is_empty());
}

#[test]
fn stamina_uses_time_ranged_powerup_and_requires_base_data() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&hero_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapStartingStats"] = json!({"EStamina":5});
    modifiers["records"][2]["stat_changes"] = json!([
        {"stat":"MODIFIER_VALUE_STAMINA","value_min":2,"value_max":7,"definition_path":"/powerup"}
    ]);
    modifiers["records"][2]["definition"] = json!({"m_flTimeMin":0,"m_flTimeMax":1});
    std::fs::write(&hero_path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let entry = CModifierTableEntry {
        modifier_subclass: Some(12),
        ability_subclass: Some(456),
        serial_number: Some(42),
        last_applied_time: Some(12.0),
        duration: Some(60.0),
        ..Default::default()
    };
    let (value, trace, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::Stamina,
        vec![],
        Some(&entry),
        true,
        &mut BTreeSet::new(),
    );
    assert_eq!(value.unwrap(), 8.0);
    assert_eq!(trace.last().unwrap().value, 3.0);
    for stat in [
        HeroStat::StaminaCooldown,
        HeroStat::DashSpeed,
        HeroStat::AirDashDuration,
    ] {
        assert!(
            stat_result(&catalog, false, stat)
                .0
                .unwrap_err()
                .to_string()
                .contains("hero has no valid base")
        );
    }
}

#[test]
fn movement_stats_use_bound_modifiers_without_inheriting_owner_bonuses() {
    let folder = super::super::catalog::tests::fixture();
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    for (stat, symbol, value) in [
        (
            HeroStat::SlideDistance,
            "MODIFIER_VALUE_MOVEMENT_SLIDE_DISTANCE_SCALE",
            37.5,
        ),
        (
            HeroStat::BulletEvasion,
            "MODIFIER_VALUE_BULLET_EVASION",
            32.5,
        ),
    ] {
        let bound = json!({"stat":symbol,"value":value,"definition_path":"/test_gun/bound"});
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":symbol,"value":value,"definition_path":"/test_gun/bound","modifier_keys":["abilities#/second"]},
            {"stat":"MODIFIER_VALUE_MOVEMENT_SLIDE_TURN_SCALE","value":900}
        ]);
        modifiers["records"][2]["stat_changes"] = json!([bound, bound]);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert_eq!(stat_result(&catalog, false, stat).0.unwrap(), 0.0);
        let (active, trace, ignored) = stat_result(&catalog, true, stat);
        assert_eq!(active.unwrap(), value);
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].modifier_serial, Some(42));
        assert!(ignored.is_empty());

        // The same record now marks someone else. The owner's conditional
        // property has no recipient binding and must not transfer to them.
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":symbol,"property_name":"ConditionalBonus","value":99,"usage_flags":"ConditionallyApplied"}
        ]);
        modifiers["records"][2]["ability_id"] = json!(123);
        modifiers["records"][2]["definition_path"] = json!("/test_gun/m_TargetModifier");
        modifiers["records"][2]["stat_changes"] = json!([]);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let entry = CModifierTableEntry {
            modifier_subclass: Some(12),
            ability_subclass: Some(123),
            serial_number: Some(42),
            ..Default::default()
        };
        let mut diagnostics = BTreeSet::new();
        let (result, trace, _) = stat_result_with_modifier(
            &catalog,
            stat,
            vec![],
            Some(&entry),
            false,
            &mut diagnostics,
        );
        assert_eq!(result.unwrap(), 0.0);
        assert!(trace.is_empty());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.contains("no binding to recipient"))
        );
    }
}

#[test]
fn slide_multiplies_distinct_bonuses_but_evasion_does_not_guess_stacking() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("modifiers.json");
    let mut modifiers: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for (stat, symbol) in [
        (
            HeroStat::SlideDistance,
            "MODIFIER_VALUE_MOVEMENT_SLIDE_DISTANCE_SCALE",
        ),
        (HeroStat::BulletEvasion, "MODIFIER_VALUE_BULLET_EVASION"),
    ] {
        modifiers["records"][2]["stat_changes"] = json!([
            {"stat":symbol,"value":35,"definition_path":"/first"},
            {"stat":symbol,"value":50,"definition_path":"/second"}
        ]);
        std::fs::write(&path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let result = stat_result(&catalog, true, stat).0;
        if stat == HeroStat::SlideDistance {
            assert!((result.unwrap() - 102.5).abs() < 1e-12);
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("combining bullet-evasion")
            );
        }
    }
}

#[test]
fn property_names_do_not_establish_stat_bindings_or_diagnostics() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("abilities.json");
    let mut abilities: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    abilities["records"][0]["definition"]["m_mapAbilityProperties"] = json!({
        "AnyEvasionChance":{"m_strValue":"42"}
    });
    std::fs::write(path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let mut diagnostics = BTreeSet::new();
    let (result, trace, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::BulletEvasion,
        vec![],
        None,
        true,
        &mut diagnostics,
    );
    assert_eq!(result.unwrap(), 0.0);
    assert!(trace.is_empty());
    assert!(diagnostics.is_empty());
}

#[test]
fn exact_evasion_property_uses_unique_effect_and_preserves_explicit_bindings() {
    let folder = super::super::catalog::tests::fixture();
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    abilities["records"][0]["definition"]["m_mapAbilityProperties"] = json!({
        "EvasionPercent": {"m_strValue": "27.5"},
        "ExtraEvasionPercent": {"m_strValue": "99"}
    });
    modifiers["records"][1]["definition_path"] = json!("/test_gun/m_AutoCastDelayModifier");
    modifiers["records"][2]["definition_path"] = json!("/test_gun/m_WhateverEffect");
    modifiers["records"][2]["ability_id"] = json!(123);
    let save = |abilities: &Value, modifiers: &Value| {
        std::fs::write(&ability_path, serde_json::to_vec(abilities).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(modifiers).unwrap()).unwrap();
        StatCatalog::from_directory(folder.path()).unwrap()
    };
    let catalog = save(&abilities, &modifiers);
    let mut diagnostics = BTreeSet::new();
    let entry = CModifierTableEntry {
        modifier_subclass: Some(12),
        ability_subclass: Some(123),
        serial_number: Some(42),
        duration: Some(-1.0),
        ..Default::default()
    };
    for active in [None, Some(&entry)] {
        let (result, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::BulletEvasion,
            vec![],
            active,
            true,
            &mut diagnostics,
        );
        assert_eq!(result.unwrap(), if active.is_some() { 27.5 } else { 0.0 });
        assert_eq!(trace.len(), usize::from(active.is_some()));
        assert_eq!(diagnostics.len(), usize::from(active.is_some()));
    }
    let windup = CModifierTableEntry {
        modifier_subclass: Some(11),
        ..entry.clone()
    };
    let (value, trace, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::BulletEvasion,
        vec![],
        Some(&windup),
        true,
        &mut diagnostics,
    );
    assert_eq!(value.unwrap(), 0.0);
    assert!(trace.is_empty() && diagnostics.is_empty());

    // A second non-intrinsic, non-cast candidate makes activation ambiguous.
    modifiers["records"][1]["definition_path"] = json!("/test_gun/m_OtherEffect");
    let catalog = save(&abilities, &modifiers);
    let (value, trace, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::BulletEvasion,
        vec![],
        Some(&entry),
        true,
        &mut diagnostics,
    );
    assert_eq!(value.unwrap(), 0.0);
    assert!(trace.is_empty());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.contains("no modifier binding"))
    );

    // Intrinsic modifiers do not activate the fallback or create ambiguity.
    modifiers["records"][1]["definition_path"] = json!("/test_gun/m_AutoIntrinsicModifiers/0");
    let catalog = save(&abilities, &modifiers);
    assert_eq!(
        stat_result(&catalog, true, HeroStat::BulletEvasion)
            .0
            .unwrap(),
        27.5
    );

    // An explicit binding wins even if the raw property has a different value.
    modifiers["records"][2]["stat_changes"] = json!([{
        "stat":"MODIFIER_VALUE_BULLET_EVASION", "property_name":"EvasionPercent",
        "value":12.5, "definition_path":"/explicit"
    }]);
    let catalog = save(&abilities, &modifiers);
    let (value, trace, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::BulletEvasion,
        vec![],
        Some(&entry),
        true,
        &mut diagnostics,
    );
    assert_eq!(value.unwrap(), 12.5);
    assert_eq!(trace.len(), 1);
    assert!(diagnostics.is_empty());
}

#[test]
fn melee_uses_catalog_boons_investment_modifiers_and_heavy_spirit_scaling() {
    let folder = super::super::catalog::tests::fixture();
    let hero_path = folder.path().join("heroes.json");
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let read = |path: &std::path::Path| -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    };
    let write = |path: &std::path::Path, value: &Value| {
        std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    };
    let mut heroes = read(&hero_path);
    let hero = &mut heroes["records"][0]["definition"];
    hero["m_mapStartingStats"] =
        json!({"ELightMeleeDamage":40,"EHeavyMeleeDamage":100,"ETechPower":2});
    hero["m_mapLevelInfo"] = json!({
        "1":{},"2":{"m_bUseStandardUpgrade":true},"3":{},
        "4":{"m_bUseStandardUpgrade":true},"5":{"m_bUseStandardUpgrade":true},
        "6":{"m_bUseStandardUpgrade":true}
    });
    hero["m_mapStandardLevelUpUpgrades"] = json!({
        "MODIFIER_VALUE_BASE_MELEE_DAMAGE_FROM_LEVEL":2,"MODIFIER_VALUE_TECH_POWER":1
    });
    hero["m_mapPurchaseBonuses"] = json!({"EItemSlotType_WeaponMod":[
        {"m_nTier":2,"m_ValueType":"MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE","m_strValue":"8"},
        {"m_nTier":2,"m_ValueType":"MODIFIER_VALUE_TECH_POWER","m_strValue":"7"},
        {"m_nTier":3,"m_ValueType":"MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE","m_strValue":"99"}
    ]});
    hero["m_mapScalingStats"] =
        json!({"EHeavyMeleeDamage":{"eScalingStat":"ETechPower","flScale":0.6}});
    write(&hero_path, &heroes);
    let mut abilities = read(&ability_path);
    abilities["records"][0]["definition"]["m_eItemSlotType"] = json!("EItemSlotType_WeaponMod");
    abilities["records"][0]["definition"]["m_iItemTier"] = json!("EModTier_2");
    let melee = "MODIFIER_VALUE_MELEE_DAMAGE_INCREASE";
    let bound = json!({"stat":melee,"value":20,"definition_path":"/test_gun/buff"});
    abilities["records"][0]["stat_changes"] = json!([
        {"stat":"MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE","value":12,"definition_path":"/weapon","modifier_keys":["abilities#/second"]},
        {"stat":"MODIFIER_VALUE_TECH_POWER","value":4,"usage_flags":"IntrinsicallyProvidedInAbility"},
        {"stat":melee,"value":10,"definition_path":"/melee","modifier_keys":["abilities#/second"]},
        {"stat":melee,"value":20,"definition_path":"/test_gun/buff","modifier_keys":["abilities#/second"]},
        {"stat":"MODIFIER_VALUE_WEAPON_DAMAGE_TO_NPC_INCREASE","value":200}
    ]);
    write(&ability_path, &abilities);
    let mut modifiers = read(&modifier_path);
    modifiers["records"][0]["stat_changes"] = json!([
        {"stat":"MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE","value":1}
    ]);
    modifiers["records"][2]["stat_changes"] = json!([
        bound, bound,
        {"stat":"MODIFIER_VALUE_WEAPON_DAMAGE_INCREASE","value":12,"definition_path":"/weapon"},
        {"stat":melee,"value":10,"definition_path":"/melee"}
    ]);
    write(&modifier_path, &modifiers);
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for (stat, base) in [
        (HeroStat::LightMeleeDamage, 46.0),
        (HeroStat::HeavyMeleeDamage, 124.6),
    ] {
        for active in [false, true] {
            let (value, trace, ignored) =
                stat_result_with_permanent(&catalog, active, stat, vec![(10, 4.0)]);
            let factor = if active { 1.42 } else { 1.06 };
            assert!((value.unwrap() - base * factor).abs() < 1e-10);
            assert!(ignored.is_empty());
            assert_eq!(
                trace
                    .iter()
                    .filter(|c| c.modifier_serial == Some(42))
                    .count(),
                3 * usize::from(active)
            );
            assert_eq!(
                trace.iter().find(|c| c.kind == "boon_flat").unwrap().value,
                6.0
            );
            assert_eq!(
                trace
                    .iter()
                    .filter(|c| c.input == stat.as_str() && c.kind == "weapon_percent")
                    .map(|c| c.value)
                    .sum::<f64>(),
                if active { 24.0 } else { 12.0 }
            );
            if stat == HeroStat::HeavyMeleeDamage {
                assert_eq!(
                    trace
                        .iter()
                        .find(|c| c.input == stat.as_str() && c.kind == "flat")
                        .unwrap()
                        .value,
                    9.6
                );
            } else {
                assert!(!trace.iter().any(|c| c.input == "spirit_power"));
            }
        }
    }
    // Changing the catalog coefficient changes the result for an arbitrary hero ID.
    heroes["records"][0]["definition"]["m_mapScalingStats"]["EHeavyMeleeDamage"]["flScale"] =
        json!(1.2);
    write(&hero_path, &heroes);
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let value =
        stat_result_with_permanent(&catalog, false, HeroStat::HeavyMeleeDamage, vec![(10, 4.0)])
            .0
            .unwrap();
    assert!((value - 142.252).abs() < 1e-10);

    for unsupported in [
        "MODIFIER_VALUE_MELEE_DAMAGE_MULTIPLIER",
        "MODIFIER_VALUE_ALL_DAMAGE_MULTIPLIER",
    ] {
        modifiers["records"][2]["stat_changes"] = json!([{ "stat":unsupported,"value":10}]);
        write(&modifier_path, &modifiers);
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        assert!(
            stat_result(&catalog, true, HeroStat::LightMeleeDamage)
                .0
                .unwrap_err()
                .to_string()
                .contains(unsupported)
        );
    }
    abilities["records"][0]["stat_changes"] = json!([]);
    abilities["records"][0]["definition"]["m_mapAbilityProperties"] = json!({
        "BaseAttackDamagePercent":{"m_strValue":"18","m_eStatsUsageFlags":"ConditionallyApplied"}
    });
    write(&ability_path, &abilities);
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let mut diagnostics = BTreeSet::new();
    let (value, _, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::LightMeleeDamage,
        vec![],
        None,
        true,
        &mut diagnostics,
    );
    assert!((value.unwrap() - 47.84).abs() < 1e-10);
    assert!(
        diagnostics
            .iter()
            .any(|message| message.contains("BaseAttackDamagePercent")
                && message.contains("no stat mapping"))
    );
    abilities["records"][0]["definition"]["m_mapAbilityProperties"] = json!({});
    write(&ability_path, &abilities);
    heroes["records"][0]["definition"]["m_mapStartingStats"]
        .as_object_mut()
        .unwrap()
        .remove("ELightMeleeDamage");
    write(&hero_path, &heroes);
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, false, HeroStat::HeavyMeleeDamage)
            .0
            .unwrap_err()
            .to_string()
            .contains("no base light melee")
    );
}

#[test]
fn melee_ignores_unbound_target_damage_and_reports_missing_global_bonuses() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
    hero["m_mapStartingStats"] = json!({"ELightMeleeDamage":40,"EHeavyMeleeDamage":100});
    hero["m_mapLevelInfo"] = json!({});
    let owner = catalog.abilities.get_mut(&123).unwrap();
    owner.stat_changes = vec![
        json!({"stat":WEAPON_DAMAGE,"value":25,"property_name":"EnemyBonus","usage_flags":"ConditionallyApplied"}),
        json!({"stat":"MODIFIER_VALUE_ALL_DAMAGE_MULTIPLIER","value":15,"property_name":"TargetAmp","usage_flags":"ConditionallyApplied"}),
    ];
    owner.definition["m_mapAbilityProperties"] = json!({
        "EnemyBonus":{"m_strConditionalLocTokenOverride":"#EnemyAboveHealthThreshold_conditional"}
    });
    for (stat, base) in [
        (HeroStat::LightMeleeDamage, 40.0),
        (HeroStat::HeavyMeleeDamage, 100.0),
    ] {
        let mut diagnostics = BTreeSet::new();
        let (value, _, _) =
            stat_result_with_modifier(&catalog, stat, vec![], None, true, &mut diagnostics);
        assert_eq!(value.unwrap(), base);
        assert!(diagnostics.is_empty());
    }
    catalog.abilities.get_mut(&123).unwrap().stat_changes.push(json!({
        "stat":WEAPON_DAMAGE,"value":18,"property_name":"UnknownGlobalBonus","usage_flags":"ConditionallyApplied"
    }));
    for stat in [HeroStat::LightMeleeDamage, HeroStat::HeavyMeleeDamage] {
        let mut diagnostics = BTreeSet::new();
        assert!(
            stat_result_with_modifier(&catalog, stat, vec![], None, true, &mut diagnostics)
                .0
                .is_ok()
        );
        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("UnknownGlobalBonus"))
        );
    }
}

#[test]
fn weapon_stats_support_both_catalog_layouts_and_report_the_source_path() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapScalingStats"] = json!({});
    let info = json!({
        "m_iClipSize":31, "m_flBulletSpeed":1000, "m_reloadDuration":2.75,
        "m_flDamageFalloffStartRange":400, "m_flDamageFalloffEndRange":1000
    });
    for path in ["m_WeaponInfo", "m_mapWeaponInfos/primary"] {
        catalog.abilities.get_mut(&123).unwrap().definition = if path == "m_WeaponInfo" {
            json!({"m_WeaponInfo":info})
        } else {
            // Prefer the new primary block, not a stale legacy or alternate weapon.
            json!({"m_WeaponInfo":{"m_iClipSize":99},
                "m_mapWeaponInfos":{"primary":info,"secondary":{"m_iClipSize":7}}})
        };
        for (stat, expected, field) in [
            (HeroStat::ClipSize, 31.0, "m_iClipSize"),
            (HeroStat::BulletVelocity, 25.4, "m_flBulletSpeed"),
            (HeroStat::ReloadTime, 2.75, "m_reloadDuration"),
            (HeroStat::FalloffStart, 10.16, "m_flDamageFalloffStartRange"),
            (HeroStat::FalloffEnd, 25.4, "m_flDamageFalloffEndRange"),
        ] {
            let (value, trace, _) = stat_result(&catalog, false, stat);
            assert!((value.unwrap() - expected).abs() < 1e-10);
            assert_eq!(
                trace[0].definition_path,
                format!("/test_gun/{path}/{field}")
            );
        }
    }
    catalog.abilities.get_mut(&123).unwrap().definition = json!({
        "m_WeaponInfo":{"m_iClipSize":99},
        "m_mapWeaponInfos":{"secondary":{"m_iClipSize":7}}
    });
    assert!(stat_result(&catalog, false, HeroStat::ClipSize).0.is_err());
}

#[test]
fn falloff_endpoints_use_catalog_values_and_only_active_range_bonuses() {
    let folder = super::super::catalog::tests::fixture();
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    let stat = "MODIFIER_VALUE_BONUS_ATTACK_RANGE_PERCENT";
    let endpoints = [HeroStat::FalloffStart, HeroStat::FalloffEnd];
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, false, endpoints[0])
            .0
            .unwrap_err()
            .to_string()
            .contains("no base falloff")
    );
    for (start, end, bonus) in [(1000.0, 2500.0, 20.0), (600.0, 900.0, 37.5)] {
        abilities["records"][0]["definition"]["m_WeaponInfo"] = json!({
            "m_flDamageFalloffStartRange":start, "m_flDamageFalloffEndRange":end,
            "m_flRange":500, "m_flDamageFalloffBias":0.3,
            "m_flDamageFalloffStartScale":1, "m_flDamageFalloffEndScale":0.1
        });
        let effect = json!({"stat":stat,"value":bonus,"definition_path":"/test_gun/range"});
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":stat,"value":bonus,"definition_path":"/test_gun/range","modifier_keys":["abilities#/second"]},
            {"stat":"MODIFIER_VALUE_BONUS_BULLET_DAMAGE_LONG_RANGE_MIN_RANGE","value":15},
            {"stat":"MODIFIER_VALUE_TECH_RANGE","value":50}
        ]);
        modifiers["records"][2]["stat_changes"] = json!([effect, effect]);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        for (endpoint, base) in endpoints.into_iter().zip([start, end]) {
            let expected = base * rulesets::METERS_PER_SOURCE_UNIT;
            assert!((stat_result(&catalog, false, endpoint).0.unwrap() - expected).abs() < 1e-10);
            let (value, trace, ignored) = stat_result(&catalog, true, endpoint);
            assert!((value.unwrap() - expected * (1.0 + bonus / 100.0)).abs() < 1e-10);
            assert!(ignored.is_empty());
            assert_eq!(trace.len(), 2); // No duplicate count or unrelated range effects.
            assert_eq!(trace[0].kind, "base");
            assert_eq!(trace[0].value, expected);
            assert_eq!(trace[1].modifier_serial, Some(42));
            assert_eq!(trace[1].value, bonus);
            assert!(trace.iter().all(|c| c.input == endpoint.as_str()));
        }
    }
    abilities["records"][0]["stat_changes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"stat":stat,"value":8,"usage_flags":"IntrinsicallyProvidedInAbility"}));
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for endpoint in endpoints {
        assert!(stat_result(&catalog, false, endpoint).0.is_ok());
        assert!(
            stat_result(&catalog, true, endpoint)
                .0
                .unwrap_err()
                .to_string()
                .contains("combining falloff-range bonuses")
        );
    }
    abilities["records"][0]["definition"]["m_WeaponInfo"]["m_flDamageFalloffStartRange"] =
        json!(-1);
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for endpoint in endpoints {
        assert!(
            stat_result(&catalog, false, endpoint)
                .0
                .unwrap_err()
                .to_string()
                .contains("unsupported falloff-range inputs")
        );
    }
}

#[test]
fn velocity_uses_catalog_values_and_counts_bound_effects_only_when_active() {
    let folder = super::super::catalog::tests::fixture();
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    let stat = "MODIFIER_VALUE_BONUS_BULLET_SPEED_PERCENT";
    abilities["records"][0]["stat_changes"] = json!([
        {"stat":stat, "value":10, "definition_path":"/test_gun/passive", "usage_flags":"IntrinsicallyProvidedInAbility"},
        {"stat":stat, "value":30, "definition_path":"/test_gun/buff", "modifier_keys":["abilities#/second"], "usage_flags":"ConditionallyApplied"}
    ]);
    modifiers["records"][2]["stat_changes"] = json!([
        {"stat":stat, "value":30, "definition_path":"/test_gun/buff"}
    ]);
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    for base in [5000.0, 12000.0] {
        abilities["records"][0]["definition"]["m_WeaponInfo"]["m_flBulletSpeed"] = json!(base);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let (inactive, _, _) = stat_result(&catalog, false, HeroStat::BulletVelocity);
        let (active, trace, ignored) = stat_result(&catalog, true, HeroStat::BulletVelocity);
        assert!((inactive.unwrap() - base * 0.0254 * 1.1).abs() < 1e-10);
        assert!((active.unwrap() - base * 0.0254 * 1.4).abs() < 1e-10);
        assert_eq!(trace.iter().filter(|c| c.kind == "percent").count(), 2);
        assert!(ignored.is_empty());
        assert_eq!(
            trace
                .iter()
                .find(|c| c.modifier_serial == Some(42))
                .unwrap()
                .value,
            30.0
        );
    }

    // Ownership alone cannot establish an unbound conditional bonus.
    abilities["records"][0]["stat_changes"][0]["usage_flags"] = json!("ConditionallyApplied");
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, true, HeroStat::BulletVelocity)
            .0
            .unwrap_err()
            .to_string()
            .contains("conditional")
    );

    abilities["records"][0]["stat_changes"] = json!([]);
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    modifiers["records"][2]["stat_changes"][0]["stat"] =
        json!("MODIFIER_VALUE_BASE_BULLET_SPEED_OVERRIDE");
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, true, HeroStat::BulletVelocity)
            .0
            .unwrap_err()
            .to_string()
            .contains("overrides")
    );
}

#[test]
fn conditional_properties_use_the_unique_owner_modifier_with_diagnostics() {
    let folder = super::super::catalog::tests::fixture();
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    let effect = json!({
        "stat":"MODIFIER_VALUE_FIRE_RATE", "property_name":"AnyBonus", "value":27.5,
        "definition_path":"/test_gun/m_mapAbilityProperties/AnyBonus",
        "usage_flags":"ConditionallyApplied", "modifier_keys":[]
    });
    abilities["records"][0]["stat_changes"] = json!([effect]);
    modifiers["records"][1]["ability_id"] = json!(456);
    modifiers["records"][2]["ability_id"] = json!(123);
    modifiers["records"][2]["definition_path"] = json!("/test_gun/m_BuffModifier");
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let entry = CModifierTableEntry {
        modifier_subclass: Some(12),
        ability_subclass: Some(123),
        serial_number: Some(42),
        last_applied_time: Some(10.0),
        duration: Some(5.0),
        ..Default::default()
    };
    let mut inferred = BTreeSet::new();
    for bonus in [27.5, 83.0] {
        abilities["records"][0]["stat_changes"][0]["value"] = json!(bonus);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let (value, trace, ignored) = stat_result_with_modifier(
            &catalog,
            HeroStat::FireRate,
            vec![],
            None,
            true,
            &mut inferred,
        );
        assert_eq!(value.unwrap(), 0.0);
        assert!(trace.is_empty() && ignored.is_empty());
        assert_eq!(inferred.len(), 1);
        // Buff recipients need not own the source ability themselves.
        for owned in [false, true] {
            let (value, trace, ignored) = stat_result_with_modifier(
                &catalog,
                HeroStat::FireRate,
                vec![],
                Some(&entry),
                owned,
                &mut inferred,
            );
            assert_eq!(value.unwrap(), bonus);
            assert!(ignored.is_empty());
            assert_eq!(inferred.len(), 1);
            assert!(
                inferred
                    .first()
                    .unwrap()
                    .contains("AnyBonus in abilities#/test_gun -> abilities#/second")
            );
            assert_eq!(trace.len(), 1);
            assert_eq!(trace[0].modifier_serial, Some(42));
        }
    }
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for duration in [None, Some(-1.0), Some(0.0), Some(f32::INFINITY)] {
        let untimed = CModifierTableEntry {
            duration,
            ..entry.clone()
        };
        let error = stat_result_with_modifier(
            &catalog,
            HeroStat::FireRate,
            vec![],
            Some(&untimed),
            true,
            &mut inferred,
        )
        .0
        .unwrap_err();
        assert!(error.to_string().contains("untimed modifier"));
    }
    let missing_time = CModifierTableEntry {
        last_applied_time: None,
        ..entry.clone()
    };
    assert!(
        stat_result_with_modifier(
            &catalog,
            HeroStat::FireRate,
            vec![],
            Some(&missing_time),
            true,
            &mut inferred
        )
        .0
        .unwrap_err()
        .to_string()
        .contains("expiry")
    );
    let stacked = CModifierTableEntry {
        stack_count: Some(2),
        ..entry.clone()
    };
    assert!(
        stat_result_with_modifier(
            &catalog,
            HeroStat::FireRate,
            vec![],
            Some(&stacked),
            true,
            &mut inferred
        )
        .0
        .unwrap_err()
        .to_string()
        .contains("stacking")
    );

    // A permanent intrinsic modifier is not an activation signal.
    modifiers["records"][2]["definition_path"] = json!("/test_gun/m_AutoIntrinsicModifiers/0");
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for active in [false, true] {
        assert!(
            stat_result(&catalog, active, HeroStat::FireRate)
                .0
                .unwrap_err()
                .to_string()
                .contains("no activation binding")
        );
    }
    // Even one active instance cannot disambiguate multiple catalog candidates.
    modifiers["records"][2]["definition_path"] = json!("/test_gun/m_BuffModifier");
    modifiers["records"][1]["ability_id"] = json!(123);
    modifiers["records"][1]["definition_path"] = json!("/test_gun/m_OtherModifier");
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for active in [false, true] {
        assert!(
            stat_result(&catalog, active, HeroStat::FireRate)
                .0
                .unwrap_err()
                .to_string()
                .contains("no activation binding")
        );
    }

    assert!(
        stat_result_with_modifier(
            &catalog,
            HeroStat::FireRate,
            vec![],
            Some(&entry),
            false,
            &mut inferred
        )
        .0
        .unwrap_err()
        .to_string()
        .contains("no unique conditional modifier")
    );

    // Explicit catalog bindings take priority and do not need inference.
    abilities["records"][0]["stat_changes"][0]["modifier_keys"] = json!(["abilities#/second"]);
    modifiers["records"][2]["stat_changes"] = json!([effect, effect]);
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert_eq!(
        stat_result(&catalog, false, HeroStat::FireRate).0.unwrap(),
        0.0
    );
    let (value, trace, ignored) = stat_result_with_modifier(
        &catalog,
        HeroStat::FireRate,
        vec![],
        Some(&entry),
        true,
        &mut inferred,
    );
    assert_eq!(value.unwrap(), 27.5);
    assert!(ignored.is_empty() && inferred.is_empty());
    assert_eq!(trace.len(), 1);
    assert_eq!(trace[0].modifier_serial, Some(42));
}

#[test]
fn a_removed_ability_ends_only_its_intrinsic_modifiers() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.modifiers[2].ability_id = Some(123);
    catalog.modifiers[2].definition_path = "/test_gun/m_AutoIntrinsicModifiers/0".into();
    let ctx = Context::new(1.0 / 64.0).unwrap();
    let mut entry = CModifierTableEntry {
        modifier_subclass: Some(12),
        ability_subclass: Some(123),
        ability: Some(1234),
        ..Default::default()
    };
    assert!(!intrinsic_ability_present(&ctx, &catalog, &entry));
    // A timed cast buff may survive destruction of the source ability.
    catalog.modifiers[2].definition_path = "/test_gun/m_BuffModifier".into();
    assert!(intrinsic_ability_present(&ctx, &catalog, &entry));
    catalog.modifiers[2].definition_path = "/test_gun/m_AutoIntrinsicModifiers/0".into();
    entry.ability = None;
    assert!(intrinsic_ability_present(&ctx, &catalog, &entry));
}

#[test]
fn purchases_use_catalog_prices_and_one_threshold_per_category() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.heroes.get_mut(&999).unwrap().definition["m_mapPurchaseBonuses"] = json!({
        "test_slot":[{"m_ValueType":WEAPON_DAMAGE,"m_nTier":2,"m_strValue":"99"}]
    });
    catalog.heroes.get_mut(&999).unwrap().definition["m_MapModCostBonuses"] = json!({
        "test_slot":[
            {"nGoldThreshold":3100,"flBonus":41},
            {"nGoldThreshold":700,"flBonus":8},
            {"nGoldThreshold":1400,"flBonus":13}
        ]
    });
    let item = catalog.abilities.get_mut(&123).unwrap();
    item.definition["m_eItemSlotType"] = json!("test_slot");
    item.definition["m_iItemTier"] = json!("EModTier_2");
    catalog.generic_data = json!({"m_nItemPricePerTier":[0,700,1400]});
    let (value, trace, _) = stat_result(&catalog, false, HeroStat::WeaponDamage);
    assert_eq!(value.unwrap(), 13.0); // Neither the legacy 99 nor 8 + 13.
    assert_eq!(
        trace
            .iter()
            .find(|c| c.kind == "purchase_cost")
            .unwrap()
            .value,
        1400.0
    );
    assert_eq!(
        trace
            .iter()
            .filter(|c| c.kind == "purchase_percent")
            .count(),
        1
    );
    catalog.generic_data["m_nItemPricePerTier"][2] = json!(3100);
    assert_eq!(
        stat_result(&catalog, false, HeroStat::WeaponDamage)
            .0
            .unwrap(),
        41.0
    );
    let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
    hero.as_object_mut().unwrap().remove("m_mapPurchaseBonuses");
    let table = hero["m_MapModCostBonuses"]
        .as_object_mut()
        .unwrap()
        .remove("test_slot")
        .unwrap();
    hero["m_MapModCostBonuses"] = json!({"EItemSlotType_WeaponMod": table});
    catalog.abilities.get_mut(&123).unwrap().definition["m_eItemSlotType"] =
        json!("EItemSlotType_WeaponMod");
    assert_eq!(
        stat_result(&catalog, false, HeroStat::WeaponDamage)
            .0
            .unwrap(),
        41.0
    );
    catalog.generic_data = Value::Null;
    assert!(
        stat_result(&catalog, false, HeroStat::WeaponDamage)
            .0
            .unwrap_err()
            .to_string()
            .contains("missing catalog item prices")
    );
}

#[test]
fn purchase_thresholds_do_not_depend_on_catalog_order() {
    let table = json!([
        {"nGoldThreshold":2300,"flBonus":31},
        {"nGoldThreshold":500,"flBonus":7},
        {"nGoldThreshold":1200,"flBonus":19}
    ]);
    for (invested, bonus) in [
        (0.0, 0.0),
        (499.0, 0.0),
        (500.0, 7.0),
        (1200.0, 19.0),
        (1800.0, 19.0),
        (2500.0, 31.0),
    ] {
        assert_eq!(purchase_threshold_bonus(&table, invested).unwrap(), bonus);
    }
    assert!(purchase_threshold_bonus(&json!([{"nGoldThreshold":-1,"flBonus":5}]), 10.0).is_err());
}

#[test]
fn fire_rate_does_not_apply_unbound_ability_values_from_ownership() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let effect = json!({"stat":"MODIFIER_VALUE_FIRE_RATE","value":17,
        "property_name":"ArbitraryBuff","modifier_keys":[]});
    catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![effect.clone()];
    let mut diagnostics = BTreeSet::new();
    let (value, trace, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::FireRate,
        vec![],
        None,
        true,
        &mut diagnostics,
    );
    assert_eq!(value.unwrap(), 0.0);
    assert!(trace.is_empty());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.contains("ArbitraryBuff") && d.contains("no modifier binding"))
    );
    catalog.abilities.get_mut(&123).unwrap().stat_changes[0]["modifier_keys"] =
        json!(["abilities#/second"]);
    catalog.modifiers[2].stat_changes = vec![effect];
    assert_eq!(
        stat_result(&catalog, false, HeroStat::FireRate).0.unwrap(),
        0.0
    );
    assert_eq!(
        stat_result(&catalog, true, HeroStat::FireRate).0.unwrap(),
        17.0
    );
}

#[test]
fn weapon_damage_uses_catalog_purchases_boons_and_recorded_totals_once() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("modifiers.json");
    let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let bound = json!({"stat":WEAPON_DAMAGE,"value":12.5,"definition_path":"/bound"});
    records["records"][2]["stat_changes"] = json!([
        bound, bound,
        {"stat":"MODIFIER_VALUE_FLAT_BULLET_DAMAGE_POST_SCALE","value":90,"definition_path":"/flat"},
        {"stat":"MODIFIER_VALUE_CLOSE_RANGE_WEAPON_DAMAGE_INCREASE","value":80,"definition_path":"/close"},
        {"stat":"MODIFIER_VALUE_WEAPON_DAMAGE_TO_NPC_INCREASE","value":70,"definition_path":"/npc"}
    ]);
    records["records"][0]["stat_changes"] = json!([{"stat":WEAPON_DAMAGE,"value":3}]);
    records["records"][0]["misc_id"] = json!(90);
    std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let hero = &mut catalog.heroes.get_mut(&999).unwrap().definition;
    hero["m_mapPurchaseBonuses"] = json!({"arbitrary_slot":[
        {"m_ValueType":WEAPON_DAMAGE,"m_nTier":2,"m_strValue":"7.25"}
    ]});
    hero["m_mapLevelInfo"] =
        json!({"1":{"m_bUseStandardUpgrade":true},"5":{"m_bUseStandardUpgrade":true}});
    hero["m_mapStandardLevelUpUpgrades"] = json!({WEAPON_DAMAGE:1.5});
    // Base bullet damage and its spirit scaling are not percentage bonuses.
    hero["m_mapStartingStats"] =
        json!({"EBaseWeaponDamage":999,"EWeaponPower":0,"EWeaponPowerScale":1});
    hero["m_mapScalingStats"] =
        json!({"EBulletDamage":{"eScalingStat":"ETechPower","flScale":999}});
    let weapon = catalog.abilities.get_mut(&123).unwrap();
    weapon.definition["m_eItemSlotType"] = json!("arbitrary_slot");
    weapon.definition["m_iItemTier"] = json!("EModTier_2");
    weapon.stat_changes =
        vec![json!({"stat":WEAPON_DAMAGE,"value":12.5,"modifier_keys":["abilities#/second"]})];
    let (value, trace, ignored) =
        stat_result_with_permanent(&catalog, true, HeroStat::WeaponDamage, vec![(10, 17.0)]);
    assert_eq!(value.unwrap(), 39.75); // 7.25 purchase + 3 boon + 12.5 bound + 17 recorded.
    assert!(ignored.is_empty());
    assert_eq!(trace.len(), 4);
    assert!(
        trace
            .iter()
            .any(|row| row.value == 17.0 && row.definition_path.ends_with(".m_flValue"))
    );
    // A live permanent pickup is already included in its accumulated total.
    let pickup: Record = serde_json::from_value(json!({
        "record_key":"misc#/permanent","definition_path":"/permanent","misc_id":90,
        "definition":{"m_bIsPermanentPickup":true},"stat_changes":[]
    }))
    .unwrap();
    catalog.misc.insert(90, pickup);
    let entry = CModifierTableEntry {
        modifier_subclass: Some(10),
        serial_number: Some(99),
        ..Default::default()
    };
    let (value, _, _) = stat_result_with_modifier(
        &catalog,
        HeroStat::WeaponDamage,
        vec![(10, 17.0)],
        Some(&entry),
        true,
        &mut BTreeSet::new(),
    );
    assert_eq!(value.unwrap(), 27.25); // No extra catalog pickup amount.
}

#[test]
fn weapon_damage_does_not_apply_unbound_rewards_from_ownership() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for flags in ["", "ConditionallyApplied", "IntrinsicallyProvidedInAbility"] {
        catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
            "stat":WEAPON_DAMAGE,"property_name":"ArbitraryReward","value":6,
            "usage_flags":flags,"modifier_keys":[],"definition_path":"/reward",
            "scaling":{"unsupported_count":true}
        })];
        let mut diagnostics = BTreeSet::new();
        let (value, trace, _) = stat_result_with_modifier(
            &catalog,
            HeroStat::WeaponDamage,
            vec![],
            None,
            true,
            &mut diagnostics,
        );
        assert_eq!(value.unwrap(), 0.0);
        assert!(trace.is_empty());
        assert!(
            diagnostics
                .iter()
                .any(|message| message.contains("ArbitraryReward")
                    && message.contains("no modifier binding"))
        );
    }
    let owner = catalog.abilities.get_mut(&123).unwrap();
    owner.stat_changes.clear();
    owner.definition["m_mapAbilityProperties"] =
        json!({"BaseAttackDamagePercent":{"m_strValue":"18"}});
    let mut diagnostics = BTreeSet::new();
    assert_eq!(
        stat_result_with_modifier(
            &catalog,
            HeroStat::WeaponDamage,
            vec![],
            None,
            true,
            &mut diagnostics
        )
        .0
        .unwrap(),
        0.0
    );
    assert!(
        diagnostics
            .iter()
            .any(|message| message.contains("BaseAttackDamagePercent")
                && message.contains("no stat mapping"))
    );
}

#[test]
fn weapon_damage_ignores_empty_declarations_but_reports_missing_intrinsics() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    for empty in [Value::Null, json!("")] {
        catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
            "stat":WEAPON_DAMAGE,"kind":"ability_property","property_name":"Empty",
            "value":empty,"raw_value":empty,"usage_flags":"","modifier_keys":[]
        })];
        let mut diagnostics = BTreeSet::new();
        assert_eq!(
            stat_result_with_modifier(
                &catalog,
                HeroStat::WeaponDamage,
                vec![],
                None,
                true,
                &mut diagnostics
            )
            .0
            .unwrap(),
            0.0
        );
        assert!(diagnostics.is_empty());
    }
    catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
        "stat":WEAPON_DAMAGE,"value":22,"property_name":"Bonus",
        "modifier_keys":["abilities#/test_gun/m_AutoIntrinsicModifiers/0"]
    })];
    let mut diagnostics = BTreeSet::new();
    assert_eq!(
        stat_result_with_modifier(
            &catalog,
            HeroStat::WeaponDamage,
            vec![],
            None,
            true,
            &mut diagnostics
        )
        .0
        .unwrap(),
        0.0
    );
    assert!(
        diagnostics
            .iter()
            .any(|message| message.contains("no effective intrinsic modifier"))
    );
}

#[test]
fn weapon_damage_excludes_explicit_enemy_conditions_without_item_names() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let owner = catalog.abilities.get_mut(&123).unwrap();
    owner.stat_changes = vec![json!({
        "stat":WEAPON_DAMAGE,"value":25,"property_name":"ArbitraryEnemyBonus",
        "usage_flags":"ConditionallyApplied","modifier_keys":[]
    })];
    owner.definition["m_mapAbilityProperties"] = json!({
        "ArbitraryEnemyBonus":{"m_strConditionalLocTokenOverride":"#EnemyAboveHealthThreshold_conditional"}
    });
    let mut diagnostics = BTreeSet::new();
    assert_eq!(
        stat_result_with_modifier(
            &catalog,
            HeroStat::WeaponDamage,
            vec![],
            None,
            true,
            &mut diagnostics
        )
        .0
        .unwrap(),
        0.0
    );
    assert!(diagnostics.is_empty());
    catalog.abilities.get_mut(&123).unwrap().definition["m_mapAbilityProperties"]["ArbitraryEnemyBonus"] =
        json!({});
    stat_result_with_modifier(
        &catalog,
        HeroStat::WeaponDamage,
        vec![],
        None,
        true,
        &mut diagnostics,
    )
    .0
    .unwrap();
    assert!(!diagnostics.is_empty());
}

#[test]
fn foreign_marker_does_not_transfer_owner_reward_or_its_diagnostic() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("modifiers.json");
    let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    records["records"][2]["ability_id"] = json!(123);
    // A real recipient effect must still apply even if its source ability
    // has an unrelated unbound reward for its owner.
    records["records"][2]["stat_changes"] = json!([{"stat":WEAPON_DAMAGE,"value":11}]);
    std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
        "stat":WEAPON_DAMAGE,"value":3,"property_name":"OwnerReward",
        "usage_flags":"ConditionallyApplied","modifier_keys":[]
    })];
    let entry = CModifierTableEntry {
        modifier_subclass: Some(12),
        serial_number: Some(42),
        ..Default::default()
    };
    let mut diagnostics = BTreeSet::new();
    assert_eq!(
        stat_result_with_modifier(
            &catalog,
            HeroStat::WeaponDamage,
            vec![],
            Some(&entry),
            false,
            &mut diagnostics
        )
        .0
        .unwrap(),
        11.0
    );
    assert!(diagnostics.is_empty());
    stat_result_with_modifier(
        &catalog,
        HeroStat::WeaponDamage,
        vec![],
        Some(&entry),
        true,
        &mut diagnostics,
    )
    .0
    .unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|message| message.contains("OwnerReward"))
    );
}

#[test]
fn owned_counter_binding_requires_replay_state_and_valid_counts() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    catalog.abilities.get_mut(&123).unwrap().stat_changes = vec![json!({
        "stat":WEAPON_DAMAGE,"value":7.5,"property_name":"Reward",
        "runtime_counts":[{"field":"m_ArbitraryEarnedCount"}],"modifier_keys":[]
    })];
    assert!(
        stat_result(&catalog, false, HeroStat::WeaponDamage)
            .0
            .unwrap_err()
            .to_string()
            .contains("owning pawn")
    );
    for invalid in [-1.0, 0.5, f64::NAN, f64::INFINITY] {
        assert!(checked_runtime_count(invalid, "count").is_err());
    }
    assert_eq!(checked_runtime_count(0.0, "count").unwrap(), 0.0);
    assert_eq!(checked_runtime_count(12.0, "count").unwrap(), 12.0);
}

#[test]
fn weapon_damage_does_not_guess_legacy_power_or_modifier_stacks() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("modifiers.json");
    let mut records: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    records["records"][2]["stat_changes"] = json!([
        {"stat":WEAPON_DAMAGE,"value":10,"definition_path":"/bonus"},
        {"stat":"MODIFIER_VALUE_WEAPON_POWER","value":7,"definition_path":"/legacy"}
    ]);
    std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let mut diagnostics = BTreeSet::new();
    let mut entry = CModifierTableEntry {
        modifier_subclass: Some(12),
        serial_number: Some(42),
        ..Default::default()
    };
    assert_eq!(
        stat_result_with_modifier(
            &catalog,
            HeroStat::WeaponDamage,
            vec![],
            Some(&entry),
            true,
            &mut diagnostics
        )
        .0
        .unwrap(),
        10.0
    );
    assert!(
        diagnostics
            .iter()
            .any(|message| message.contains("weapon power"))
    );
    entry.stack_count = Some(2);
    assert!(
        stat_result_with_modifier(
            &catalog,
            HeroStat::WeaponDamage,
            vec![],
            Some(&entry),
            true,
            &mut diagnostics
        )
        .0
        .unwrap_err()
        .to_string()
        .contains("stacking rule")
    );
}

#[test]
fn fire_rate_preserves_individual_slows_and_pickup_totals() {
    let folder = super::super::catalog::tests::fixture();
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    let bonus = "MODIFIER_VALUE_FIRE_RATE";
    let slow = "MODIFIER_VALUE_FIRE_RATE_SLOW";
    abilities["records"][0]["stat_changes"] = json!([
        {"stat":bonus, "value":20, "definition_path":"/passive", "usage_flags":"IntrinsicallyProvidedInAbility"},
        {"stat":bonus, "value":10, "definition_path":"/bound", "modifier_keys":["abilities#/second"]}
    ]);
    let bound = json!({"stat":bonus, "value":10, "definition_path":"/bound"});
    modifiers["records"][2]["stat_changes"] = json!([
        bound, bound,
        {"stat":slow, "value":20, "definition_path":"/slow_a"},
        {"stat":slow, "value":30, "definition_path":"/slow_b"}
    ]);
    modifiers["records"][0]["stat_changes"] = json!([{"stat":bonus,"value":1.5}]);
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert_eq!(
        stat_result(&catalog, false, HeroStat::FireRate).0.unwrap(),
        20.0
    );
    let (value, trace, ignored) =
        stat_result_with_permanent(&catalog, true, HeroStat::FireRate, vec![(10, 4.0)]);
    // 20 + 10 + 4 - (1 - 0.8 * 0.7) * 100 = -10%.
    assert!((value.unwrap() + 10.0).abs() < 1e-12);
    assert!(ignored.is_empty());
    assert_eq!(trace.len(), 5);
    assert_eq!(trace.iter().filter(|r| r.kind == "slow").count(), 2);
    assert_eq!(trace[0].value, 4.0);

    // A signed negative FIRE_RATE input is also a slow under this rule.
    modifiers["records"][2]["stat_changes"][2]["stat"] = json!(bonus);
    modifiers["records"][2]["stat_changes"][2]["value"] = json!(-20);
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!((stat_result(&catalog, true, HeroStat::FireRate).0.unwrap() + 14.0).abs() < 1e-12);
}

#[test]
fn zero_hero_scaling_needs_no_spirit_and_unknown_scaling_fails() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("heroes.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapScalingStats"]["EFireRate"] =
        json!({"eScalingStat":"ETechPower", "flScale":0});
    std::fs::write(&path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert_eq!(
        stat_result(&catalog, false, HeroStat::FireRate).0.unwrap(),
        0.0
    );
    heroes["records"][0]["definition"]["m_mapScalingStats"]["EFireRate"]["eScalingStat"] =
        json!("unknown");
    std::fs::write(&path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, false, HeroStat::FireRate)
            .0
            .unwrap_err()
            .to_string()
            .contains("scaling stat")
    );
}

#[test]
fn reload_time_uses_catalog_duration_and_only_effective_adjustments() {
    let folder = super::super::catalog::tests::fixture();
    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    let stat = "MODIFIER_VALUE_RELOAD_SPEED";
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, false, HeroStat::ReloadTime)
            .0
            .unwrap_err()
            .to_string()
            .contains("base reload duration")
    );

    let effect = json!({"stat":stat, "value":-20, "definition_path":"/test_gun/reload"});
    // The bound adjustment counts once, even if its binding is duplicated.
    abilities["records"][0]["stat_changes"] = json!([
        {"stat":stat, "value":-20, "definition_path":"/test_gun/reload", "modifier_keys":["abilities#/second"]},
        {"stat":"MODIFIER_VALUE_MELEE_TRAVEL_DISTANCE_PERCENTAGE", "value":50}
    ]);
    modifiers["records"][2]["stat_changes"] = json!([effect, effect]);
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    for (base, single) in [(2.5, false), (0.4, true)] {
        abilities["records"][0]["definition"]["m_WeaponInfo"] = json!({
            "m_reloadDuration":base, "m_bReloadSingleBullets":single,
            "m_flReloadSingleBulletsInitialDelay":0.7, "m_iClipSize":30
        });
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let (inactive, _, _) = stat_result(&catalog, false, HeroStat::ReloadTime);
        let (active, trace, ignored) = stat_result(&catalog, true, HeroStat::ReloadTime);
        assert_eq!(inactive.unwrap(), base);
        assert!((active.unwrap() - base * 0.8).abs() < 1e-12);
        assert!(ignored.is_empty());
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].value, base);
        assert_eq!(trace[1].value, -20.0);
        assert_eq!(trace[1].modifier_serial, Some(42));
        assert!(trace.iter().all(|c| c.input == "reload_time"));
    }

    // An intrinsic property also applies without a modifier binding.
    abilities["records"][0]["stat_changes"][0] = json!({
        "stat":stat, "value":-10, "definition_path":"/test_gun/passive",
        "usage_flags":"IntrinsicallyProvidedInAbility"
    });
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        (stat_result(&catalog, false, HeroStat::ReloadTime)
            .0
            .unwrap()
            - 0.36)
            .abs()
            < 1e-12
    );
    // Do not assume additive or multiplicative stacking from VData names.
    assert!(
        stat_result(&catalog, true, HeroStat::ReloadTime)
            .0
            .unwrap_err()
            .to_string()
            .contains("combining")
    );

    abilities["records"][0]["stat_changes"][0]["usage_flags"] = json!("ConditionallyApplied");
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, false, HeroStat::ReloadTime)
            .0
            .unwrap_err()
            .to_string()
            .contains("conditional")
    );

    abilities["records"][0]["stat_changes"] = json!([]);
    abilities["records"][0]["definition"]["m_WeaponInfo"]["m_bReloadUseActiveWeaponInfoDuration"] =
        json!(true);
    std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert!(
        stat_result(&catalog, false, HeroStat::ReloadTime)
            .0
            .unwrap_err()
            .to_string()
            .contains("dynamic weapon")
    );
}

#[test]
fn melee_distance_resolves_catalog_bonuses_without_double_counting() {
    let folder = super::super::catalog::tests::fixture();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    assert_eq!(
        stat_result(&catalog, true, HeroStat::MeleeDistance)
            .0
            .unwrap(),
        0.0
    );

    let ability_path = folder.path().join("abilities.json");
    let modifier_path = folder.path().join("modifiers.json");
    let mut abilities: Value =
        serde_json::from_slice(&std::fs::read(&ability_path).unwrap()).unwrap();
    let mut modifiers: Value =
        serde_json::from_slice(&std::fs::read(&modifier_path).unwrap()).unwrap();
    let stat = "MODIFIER_VALUE_MELEE_TRAVEL_DISTANCE_PERCENTAGE";
    for bonus in [27.5, 83.0] {
        let effect = json!({"stat":stat, "value":bonus, "definition_path":"/test_gun/buff"});
        abilities["records"][0]["stat_changes"] = json!([
            {"stat":stat, "value":12.5, "definition_path":"/test_gun/passive", "usage_flags":"IntrinsicallyProvidedInAbility"},
            {"stat":stat, "value":bonus, "definition_path":"/test_gun/buff", "modifier_keys":["abilities#/second"]},
            {"stat":"MODIFIER_VALUE_BONUS_BULLET_SPEED_PERCENT", "value":99}
        ]);
        // Duplicate property bindings in a modifier must count only once.
        modifiers["records"][2]["stat_changes"] = json!([effect, effect]);
        std::fs::write(&ability_path, serde_json::to_vec(&abilities).unwrap()).unwrap();
        std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
        let catalog = StatCatalog::from_directory(folder.path()).unwrap();
        let (inactive, _, _) = stat_result(&catalog, false, HeroStat::MeleeDistance);
        let (active, trace, ignored) = stat_result(&catalog, true, HeroStat::MeleeDistance);
        assert_eq!(inactive.unwrap(), 12.5);
        assert_eq!(active.unwrap(), 12.5 + bonus);
        assert_eq!(trace.len(), 2);
        assert!(ignored.is_empty());
        assert!(
            trace
                .iter()
                .all(|c| c.input == "melee_distance" && c.kind == "percent")
        );
        assert_eq!(trace[1].modifier_serial, Some(42));
        assert_eq!(trace[1].value, bonus);
    }

    // Skip a missing modifier, retain known bonuses, and report its ID.
    modifiers["records"].as_array_mut().unwrap().pop();
    std::fs::write(&modifier_path, serde_json::to_vec(&modifiers).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let (value, trace, ignored) = stat_result(&catalog, true, HeroStat::MeleeDistance);
    assert_eq!(value.unwrap(), 12.5);
    assert_eq!(trace.len(), 1);
    assert_eq!(ignored[&12], "unresolved modifier ID 12");
}

#[test]
fn ping_markers_do_not_change_stats_or_report_missing_inputs() {
    let folder = super::super::catalog::tests::fixture();
    let mut catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let definition = &mut catalog.heroes.get_mut(&999).unwrap().definition;
    definition["m_mapScalingStats"] = json!({});
    definition["m_mapStartingStats"] = json!({});
    for stat in [
        HeroStat::ClipSize,
        HeroStat::FireRate,
        HeroStat::DebuffResist,
    ] {
        let (baseline, baseline_trace, _) = stat_result(&catalog, false, stat);
        let baseline = baseline.unwrap();
        for id in [PLAYER_PINGED, ENTITY_PINGED, 123456] {
            let modifier = CModifierTableEntry {
                modifier_subclass: Some(id),
                serial_number: Some(42),
                ..Default::default()
            };
            let mut inferred = BTreeSet::new();
            let (value, trace, ignored) = stat_result_with_modifier(
                &catalog,
                stat,
                vec![(id, 10.0)],
                Some(&modifier),
                true,
                &mut inferred,
            );
            assert_eq!(value.unwrap(), baseline);
            assert_eq!(trace.len(), baseline_trace.len());
            assert!(trace.iter().all(|c| c.modifier_serial != Some(42)));
            assert!(inferred.is_empty());
            if id == 123456 {
                assert_eq!(ignored.len(), 1);
                assert_eq!(ignored[&id], "unresolved modifier ID 123456");
            } else {
                assert!(ignored.is_empty());
            }
        }
    }
}

#[test]
fn missing_and_ambiguous_sources_are_reported_once_without_losing_known_totals() {
    let folder = super::super::catalog::tests::fixture();
    let path = folder.path().join("heroes.json");
    let mut heroes: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    heroes["records"][0]["definition"]["m_mapScalingStats"] = json!({});
    std::fs::write(&path, serde_json::to_vec(&heroes).unwrap()).unwrap();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let (value, _, ignored) = stat_result_with_permanent(
        &catalog,
        true,
        HeroStat::ClipSize,
        vec![(10, 4.0), (123456, 10.0), (123456, 2.0), (6, 1.0)],
    );
    assert_eq!(value.unwrap(), 21.0); // ceil(20 * 1.04)
    assert_eq!(ignored.len(), 2); // Repeated input passes also deduplicate.
    assert_eq!(ignored[&123456], "unresolved modifier ID 123456");
    assert!(ignored[&6].contains("ambiguous modifier ID 6"));
}

#[test]
fn powerup_uses_catalog_range_and_application_time() {
    let folder = super::super::catalog::tests::fixture();
    let catalog = StatCatalog::from_directory(folder.path()).unwrap();
    let ctx = Context::new(1.0 / 64.0).unwrap();
    let controller = Entity::from_fields(1, 1, 0, "test", true, Default::default()).unwrap();
    let mut contributions = Vec::new();
    let resolver = Resolver {
        mode: StatMode::Current,
        ctx: &ctx,
        catalog: &catalog,
        controller: &controller,
        hero_id: 999,
        steam_id: None,
        game_time: Some(800.0),
        game_start: Some(100.0),
        explain: true,
        contributions: &mut contributions,
        ignored_modifiers: BTreeMap::new(),
        inferred_bindings: BTreeSet::new(),
        unmapped_inputs: BTreeSet::new(),
    };
    let source: Record = serde_json::from_value(json!({"record_key":"misc#/any_powerup", "definition_path":"/any_powerup", "definition":{"m_flTimeMin":5,"m_flTimeMax":15}, "stat_changes":[]})).unwrap();
    let effect = json!({"value_min":10, "value_max":30});
    let mut entry = CModifierTableEntry {
        last_applied_time: Some(700.0),
        ..Default::default()
    };
    assert_eq!(
        resolver.effect(&effect, &source, Some(&entry)).unwrap(),
        20.0
    );
    entry.last_applied_time = Some(100.0);
    assert_eq!(
        resolver.effect(&effect, &source, Some(&entry)).unwrap(),
        10.0
    );
    entry.last_applied_time = Some(2000.0);
    assert_eq!(
        resolver.effect(&effect, &source, Some(&entry)).unwrap(),
        30.0
    );
    assert!(resolver.effect(&effect, &source, None).is_err());
    let zero_scale = json!({"value":10,"scaling":{"$value":{"_class":"scale_function_tech_damage", "m_flStatScale":0}}});
    assert_eq!(resolver.effect(&zero_scale, &source, None).unwrap(), 10.0);
    let nonzero_scale = json!({"value":10,"scaling":{"$value":{"_class":"scale_function_tech_damage", "m_flStatScale":0.2}}});
    assert!(resolver.effect(&nonzero_scale, &source, None).is_err());
    let unsupported = json!({"value":10,"scaling":{"$value":{"_class":"unknown"}}});
    assert!(resolver.effect(&unsupported, &source, None).is_err());
}
