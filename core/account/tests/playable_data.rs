#![cfg(feature = "private-data-tests")]

//! 核对静态计算资料、试用模板与映射模板的关联。

use std::collections::BTreeSet;

use account_core::{
    playable::{PlayableRelics, attributes::Attribute},
    reference::SkillKind,
};

#[test]
fn loads_all_documented_records_and_preserves_cross_references() {
    let data = support::playable_data();
    assert_eq!(data.manifest.game_version, "4.7");
    assert_eq!(data.manifest.season, "S2");
    assert_eq!(data.characters.len(), 61);
    assert_eq!(data.light_cones.len(), 172);
    assert_eq!(data.trials.len(), 61);
    assert_eq!(data.mappings.len(), 60);
    assert_eq!(data.default_light_cones.len(), 9);
    for (path, cone_id) in &data.default_light_cones {
        let cone = data
            .light_cones
            .iter()
            .find(|entry| entry.id == *cone_id)
            .unwrap();
        assert_eq!(cone.path, *path);
    }
    assert_eq!(data.minor_traces.len(), 60);
    for minor in &data.minor_traces {
        assert_eq!(
            minor.nodes.keys().copied().collect::<Vec<_>>(),
            (201..=210).collect::<Vec<_>>()
        );
    }
    let character_ids: BTreeSet<_> = data.characters.iter().map(|entry| &entry.id).collect();
    let trial_ids: BTreeSet<_> = data
        .trials
        .iter()
        .map(|entry| &entry.character_id)
        .collect();
    assert_eq!(character_ids, trial_ids);
    assert_eq!(character_ids.len(), 61);
    let cone_ids: BTreeSet<_> = data.light_cones.iter().map(|entry| &entry.id).collect();
    for trial in &data.trials {
        assert!(cone_ids.contains(&trial.light_cone_id));
        if let Some(traces) = &trial.traces {
            let minor = data
                .minor_traces
                .iter()
                .find(|entry| entry.character_id == trial.character_id)
                .unwrap();
            assert!(
                traces
                    .inactive_minor_nodes
                    .iter()
                    .all(|node| minor.nodes.contains_key(node))
            );
        }
    }
    for mapping in &data.mappings {
        assert!(character_ids.contains(&mapping.character_id));
    }
}

#[test]
fn binds_the_last_two_minor_nodes_and_keeps_fully_unlocked_trials() {
    let data = support::playable_data();
    let welt = data
        .minor_traces
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(welt.nodes[&201].attribute, Attribute::EffectHitRate);
    assert_eq!(welt.nodes[&201].value, 0.04);
    assert_eq!(welt.nodes[&209].attribute, Attribute::ImaginaryDamageBoost);
    assert_eq!(welt.nodes[&209].value, 0.064);
    assert_eq!(welt.nodes[&210].value, 0.08);
    let jingliu = data
        .minor_traces
        .iter()
        .find(|entry| entry.character_id == "1212")
        .unwrap();
    assert_eq!(jingliu.nodes[&209].attribute, Attribute::Speed);
    assert_eq!(jingliu.nodes[&209].value, 4.0);
    for trial in &data.trials {
        match trial.character_id.as_str() {
            "6042" => assert!(trial.traces.is_none()),
            "1408" | "1412" => assert!(
                trial
                    .traces
                    .as_ref()
                    .unwrap()
                    .inactive_minor_nodes
                    .is_empty()
            ),
            _ => assert_eq!(
                trial.traces.as_ref().unwrap().inactive_minor_nodes,
                [209, 210]
            ),
        }
    }
}

#[test]
fn keeps_trial_and_mapping_values_distinct() {
    let data = support::playable_data();
    let trial = data
        .trials
        .iter()
        .find(|entry| entry.character_id == "1408")
        .unwrap();
    assert_eq!(
        trial.traces.as_ref().unwrap().base_levels[&SkillKind::Basic],
        6
    );
    let welt = data
        .mappings
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert_eq!(welt.outer.set_id, "117");
    assert_eq!(welt.planar.set_id, "309");
    assert_eq!(welt.outer.stats[&Attribute::Speed], 29.632);
    assert!((welt.planar.stats[&Attribute::ImaginaryDamageBoost] - 0.388803).abs() < 1e-12);
}

#[test]
fn preserves_light_cone_base_speed_as_a_distinct_property_at_every_rank() {
    let data = support::playable_data();
    for id in ["23036", "23044", "23065"] {
        let cone = data
            .light_cones
            .iter()
            .find(|entry| entry.id == id)
            .unwrap();
        assert_eq!(
            cone.properties[&Attribute::BaseSpeed],
            [12.0, 14.0, 16.0, 18.0, 20.0]
        );
        assert!(!cone.properties.contains_key(&Attribute::Speed));
    }
}

#[test]
fn preserves_aggregate_trial_relics_and_special_characters() {
    let data = support::playable_data();
    let welt = data
        .trials
        .iter()
        .find(|entry| entry.character_id == "1004")
        .unwrap();
    assert!((welt.relics.stats[&Attribute::ImaginaryDamageBoost] - 0.3888).abs() < 1e-12);
    let relics = PlayableRelics::TrialAggregate {
        configuration: welt.relics.clone(),
    };
    let encoded = serde_json::to_value(relics).unwrap();
    assert_eq!(encoded["kind"], "trial_aggregate");
    assert!(encoded["configuration"]["stats"].is_object());
    assert!(encoded.get("instance_ids").is_none());
    let pam = data
        .characters
        .iter()
        .find(|entry| entry.id == "6042")
        .unwrap();
    assert_eq!(pam.base.hp, 917.28);
    assert_eq!(pam.base.crit_rate(), 1.0);
    assert_eq!(pam.base.aggro, 125);
    assert_eq!(pam.rarity, None);
    assert!(
        !data
            .mappings
            .iter()
            .any(|entry| entry.character_id == "6042")
    );
    let aha = data
        .trials
        .iter()
        .find(|entry| entry.character_id == "1511")
        .unwrap();
    assert_eq!(
        aha.traces.as_ref().unwrap().base_levels[&SkillKind::AhaBiography],
        1
    );
    let normal = data
        .characters
        .iter()
        .find(|entry| entry.id == "1004")
        .unwrap();
    assert_eq!(normal.base.crit_rate(), 0.05);
}

mod support;
