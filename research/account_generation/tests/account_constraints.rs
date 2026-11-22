//! 验证账号金数、装备唯一性、资源集中度和角色覆盖约束。

use std::collections::{BTreeMap, BTreeSet};

use account_core::{RelicMainStat, RelicSlot, RelicSubStat, TraceConfig};
use account_generator::{
    AccountReference, AccountRequest, ChaCha8Rng, CharacterSpec, GeneratedAccount, GenerationError,
    GoldCategory, LightConeSpec, RelicRequest, RelicSpec, SeedableRng, generate_account,
    generate_character,
};

/// 构造包含两个指定候选角色和一个核心角色的基础请求。
fn two_characters() -> AccountRequest {
    AccountRequest {
        character_pool: Some(vec!["1004".into(), "1005".into()]),
        core_character_ids: vec!["1005".into()],
        ..AccountRequest::new("two-characters", 2)
    }
}

/// 构造固定使用指定光锥的角色配置。
fn with_cone(id: &str) -> CharacterSpec {
    CharacterSpec {
        light_cone: LightConeSpec {
            light_cone_id: Some(id.into()),
            ..LightConeSpec::default()
        },
        ..CharacterSpec::default()
    }
}

/// 验证分类金数统计、装备引用完整性及预算不足时的失败结果。
#[test]
fn gold_counts_and_equipment_references_follow_the_budget_categories() {
    let reference = AccountReference::bundled().unwrap();
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let mut request = AccountRequest {
        gold_count: Some(6.0),
        five_star_build: Some((2, 1)),
        ..two_characters()
    };
    let result = generate_account(&request, &reference, &mut rng).unwrap();
    use GoldCategory::{Free, Limited, Standard};
    assert_eq!(
        result.copies_by_category(),
        BTreeMap::from([(Limited, 4), (Standard, 4), (Free, 0)])
    );
    assert_eq!(
        result.gold_by_category(),
        BTreeMap::from([(Limited, 4.0), (Standard, 2.0)])
    );
    assert_eq!(result.gold_count(), 6.0);
    let mut cones: BTreeSet<_> = result
        .account
        .light_cones
        .iter()
        .map(|cone| &cone.instance_id)
        .collect();
    let mut relics: BTreeMap<_, _> = result
        .account
        .relics
        .iter()
        .map(|piece| (&piece.instance_id, piece.slot))
        .collect();
    assert_eq!(cones.len(), 2);
    assert_eq!(relics.len(), 12);
    for character in &result.account.characters {
        assert!(cones.remove(character.light_cone_instance_id.as_ref().unwrap()));
        assert_eq!(character.relic_instance_ids.len(), 6);
        for (slot, instance) in &character.relic_instance_ids {
            assert_eq!(relics.remove(instance), Some(*slot));
        }
    }
    request.gold_count = Some(4.0);
    assert!(matches!(
        generate_account(&request, &reference, &mut rng),
        Err(GenerationError::NoSolution)
    ));
    request.gold_categories = vec![Limited];
    assert_eq!(
        generate_account(&request, &reference, &mut rng)
            .unwrap()
            .gold_count(),
        4.0
    );
}

/// 验证免费资源可排除在金数外，也可显式计入预算。
#[test]
fn free_resources_are_counted_and_can_be_included_in_the_budget() {
    let reference = AccountReference::bundled().unwrap();
    let mut spec = with_cone("24006");
    spec.eidolons = 2;
    spec.light_cone.superimposition = 5;
    let mut request = AccountRequest {
        gold_count: Some(0.0),
        character_pool: Some(vec!["8009".into()]),
        character_overrides: BTreeMap::from([("8009".into(), spec)]),
        ..AccountRequest::new("free", 1)
    };
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let result = generate_account(&request, &reference, &mut rng).unwrap();
    assert_eq!(result.copies_by_category()[&GoldCategory::Free], 8);
    assert_eq!(result.gold_count(), 0.0);
    request.gold_categories.push(GoldCategory::Free);
    request.gold_count = Some(8.0);
    assert_eq!(
        generate_account(&request, &reference, &mut rng)
            .unwrap()
            .gold_count(),
        8.0
    );
}

/// 验证半金精度的自动配装及唯一光锥实例约束。
#[test]
fn automatic_builds_respect_half_gold_and_unique_cones() {
    let reference = AccountReference::bundled().unwrap();
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let half_gold = AccountRequest {
        gold_count: Some(0.5),
        character_pool: Some(vec!["1004".into()]),
        light_cone_pool: Some(vec!["21006".into(), "21001".into()]),
        ..AccountRequest::new("half-gold", 1)
    };
    let result = generate_account(&half_gold, &reference, &mut rng).unwrap();
    assert_eq!(result.gold_count(), 0.5);
    assert_eq!(result.account.characters[0].eidolons, 0);
    assert_eq!(result.account.light_cones[0].light_cone_id, "21001");

    let overrides = ["1004", "1005"]
        .into_iter()
        .map(|id| (id.into(), CharacterSpec::default()))
        .collect();
    let mut request = AccountRequest {
        character_overrides: overrides,
        light_cone_pool: Some(vec!["24003".into(), "23004".into()]),
        ..two_characters()
    };
    for seed in 0..8 {
        let result =
            generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(seed)).unwrap();
        assert!([2.0, 2.5].contains(&result.gold_count()));
        assert!(
            result
                .account
                .light_cones
                .iter()
                .filter(|cone| cone.light_cone_id == "24003")
                .count()
                <= 1
        );
    }
    request.light_cone_pool = Some(vec!["21001".into()]);
    request.gold_count = Some(1.5);
    let result = generate_account(&request, &reference, &mut rng).unwrap();
    assert!(
        result
            .account
            .light_cones
            .iter()
            .all(|cone| cone.light_cone_id == "21001")
    );
    assert_ne!(
        result.account.light_cones[0].instance_id,
        result.account.light_cones[1].instance_id
    );
}

/// 验证覆盖角色仍可成为自动核心角色。
#[test]
fn ownership_overrides_still_allow_either_character_to_be_the_automatic_core() {
    let reference = AccountReference::bundled().unwrap();
    let request = AccountRequest {
        character_overrides: BTreeMap::from([("1004".into(), CharacterSpec::default())]),
        core_character_ids: Vec::new(),
        teammate_bias: 0.0,
        ..two_characters()
    };
    let cores: BTreeSet<_> = (0..12)
        .flat_map(|seed| {
            let result =
                generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(seed))
                    .unwrap();
            assert!(
                result
                    .account
                    .characters
                    .iter()
                    .any(|character| character.character_id == "1004")
            );
            assert_eq!(result.core_character_ids.len(), 1);
            result.core_character_ids
        })
        .collect();
    assert_eq!(
        cores,
        BTreeSet::from(["1004".to_owned(), "1005".to_owned()])
    );
}

/// 验证资源集中度提高核心角色获得的金数和遗器分数占比。
#[test]
fn concentrating_resources_changes_the_core_shares_under_the_same_constraints() {
    let reference = AccountReference::bundled().unwrap();
    let generate = |request: &AccountRequest| {
        generate_account(request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap()
    };
    let mut request = AccountRequest {
        gold_count: Some(8.0),
        ..two_characters()
    };
    let even = generate(&request);
    request.gold_concentration = Some(1.0);
    let focused = generate(&request);
    assert_eq!(even.gold_count(), focused.gold_count());
    assert!(focused.core_gold_share() > even.core_gold_share() + 0.2);

    let mut request = AccountRequest {
        gold_count: Some(6.0),
        five_star_build: Some((2, 1)),
        min_mean_effective_rolls: Some(3.0),
        max_mean_effective_rolls: Some(3.05),
        ..two_characters()
    };
    let even = generate(&request);
    request.relic_concentration = Some(1.0);
    let focused = generate(&request);
    assert!(
        [&even, &focused]
            .into_iter()
            .all(|result| (3.0..=3.05).contains(&result.mean_effective_rolls()))
    );
    assert!(focused.core_relic_share() > even.core_relic_share() + 0.3);
}

/// 验证账号品质档位及独立平均分上下限作用于生成结果。
#[test]
fn account_quality_and_independent_average_bounds_apply_to_the_result() {
    let reference = AccountReference::bundled().unwrap();
    let mut request = AccountRequest {
        relic_quality_tier: Some(5),
        ..two_characters()
    };
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let result = generate_account(&request, &reference, &mut rng).unwrap();
    let maximum: f64 = result
        .investments
        .values()
        .map(|value| value.relic_score_upper)
        .sum();
    let score = result.mean_effective_rolls() * 12.0;
    assert!((0.8 * maximum..=maximum + 1e-6).contains(&score));

    request.relic_quality_tier = None;
    request.min_mean_effective_rolls = Some(4.0);
    let result = generate_account(&request, &reference, &mut rng).unwrap();
    assert!(result.mean_effective_rolls() >= 4.0 - 1e-6);
    request.min_mean_effective_rolls = None;
    request.max_mean_effective_rolls = Some(1.0);
    let result = generate_account(&request, &reference, &mut rng).unwrap();
    assert!(result.mean_effective_rolls() <= 1.0 + 1e-6);
}

/// 验证负遗器评分在角色和账号汇总时保持符号与目标值。
#[test]
fn signed_relic_scores_survive_character_and_account_aggregation() {
    let reference = AccountReference::bundled().unwrap();
    let mut pieces = vec![RelicSpec {
        required_substats: vec![RelicSubStat::Spd],
        ..RelicSpec::new(RelicSlot::Head)
    }];
    pieces.extend((2..=6).map(|slot| RelicSpec {
        min_effective_rolls: Some(0.0),
        max_effective_rolls: Some(0.0),
        ..RelicSpec::new(RelicSlot::try_from(slot).unwrap())
    }));
    let request = AccountRequest {
        min_mean_effective_rolls: Some(-1.0 / 6.0),
        max_mean_effective_rolls: Some(-1.0 / 6.0),
        character_overrides: BTreeMap::from([(
            "1004".into(),
            CharacterSpec {
                relics: Some(RelicRequest {
                    pieces,
                    effective_stat_weights: BTreeMap::from([(RelicSubStat::Spd, -1.0)]),
                    tier_weights: [0.0, 1.0, 0.0],
                    ..RelicRequest::default()
                }),
                ..CharacterSpec::default()
            },
        )]),
        max_attempts: 7,
        ..AccountRequest::new("signed", 1)
    };
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let result = generate_account(&request, &reference, &mut rng).unwrap();
    assert_eq!(result.account.relics[0].substats[&RelicSubStat::Spd], 2.3);
    assert_eq!(result.mean_effective_rolls(), -1.0 / 6.0);
    assert_eq!(result.attempts, 7);
    let empty = AccountRequest {
        max_mean_effective_rolls: Some(-0.1),
        ..AccountRequest::new("empty", 0)
    };
    assert!(matches!(
        generate_account(&empty, &reference, &mut rng),
        Err(GenerationError::NoSolution)
    ));
}

/// 验证显式行迹与单件遗器覆盖保留其他推荐配置。
#[test]
fn explicit_character_traces_and_partial_relic_overrides_preserve_other_recommendations() {
    let reference = AccountReference::bundled().unwrap();
    let traces = TraceConfig {
        skill_levels: BTreeMap::from([("1310001".into(), 3)]),
        node_unlocks: BTreeMap::from([("1310101".into(), true), ("1310102".into(), false)]),
    };
    let spec = CharacterSpec {
        traces: Some(traces.clone()),
        relics: Some(RelicRequest {
            pieces: vec![RelicSpec {
                set_id: Some("306".into()),
                mainstat: vec![RelicMainStat::HpPercent, RelicMainStat::DefPercent],
                ..RelicSpec::new(RelicSlot::PlanarSphere)
            }],
            effective_stat_weights: BTreeMap::from([(RelicSubStat::Spd, 1.0)]),
            min_total_effective_rolls: Some(12.0),
            max_total_effective_rolls: Some(18.0),
            ..RelicRequest::default()
        }),
        ..CharacterSpec::default()
    };
    let generate = |id| -> GeneratedAccount {
        generate_character(
            id,
            "1310",
            &spec,
            &reference,
            10_000,
            &mut ChaCha8Rng::seed_from_u64(42),
        )
        .unwrap()
    };
    let result = generate("first");
    let other = generate("second");
    assert_eq!(result.account.characters[0].traces, traces);
    assert_eq!(result.account.relics[3].mainstat, RelicMainStat::Spd);
    assert_eq!(result.account.relics[4].set_id, "306");
    assert!(
        [RelicMainStat::HpPercent, RelicMainStat::DefPercent]
            .contains(&result.account.relics[4].mainstat)
    );
    assert_eq!(
        result
            .account
            .relics
            .iter()
            .map(|piece| u8::from(piece.slot))
            .collect::<Vec<_>>(),
        (1..=6).collect::<Vec<_>>()
    );
    assert!((12.0..=18.0).contains(&result.investments["1310"].piece_scores.iter().sum()));
    assert_ne!(
        result.account.light_cones[0].instance_id,
        other.account.light_cones[0].instance_id
    );
    assert!(
        result
            .account
            .relics
            .iter()
            .zip(other.account.relics)
            .all(|(first, second)| first.instance_id != second.instance_id)
    );
}
