//! 验证账号、单角色与遗器生成结果及其约束和转换行为。

use std::collections::{BTreeMap, BTreeSet};

use account_core::{
    RawAccount, RelicMainStat, RelicSlot, RelicSubStat,
    playable::{
        CharacterOrigin,
        conversion::{ConversionRequest, convert_playable_account},
    },
    reference::PlayableData,
};
use account_generator::{
    AccountReference, AccountRequest, ChaCha8Rng, CharacterSpec, GenerationError, LightConeSpec,
    RelicRequest, RelicSpec, SeedableRng, generate_account, generate_character, generate_relic,
    score_relic,
};

/// 省略账号 ID 时，用同一种子可重放账号，所有光锥和遗器引用均使用生成的 ID。
#[test]
fn omitted_account_id_is_replayable_and_names_the_equipment_instances() {
    let request: AccountRequest = serde_json::from_str(r#"{"character_count": 1}"#).unwrap();
    let reference = AccountReference::bundled().unwrap();
    let draw = |rng: &mut ChaCha8Rng| generate_account(&request, &reference, rng).unwrap();
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let first = draw(&mut rng);
    let id = &first.account.config_id;
    let suffix = id.strip_prefix("account-").unwrap();
    assert_eq!(suffix.len(), 32);
    assert!(suffix.bytes().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(id, &draw(&mut rng).account.config_id);
    assert_eq!(first, draw(&mut ChaCha8Rng::seed_from_u64(42)));

    let character = &first.account.characters[0];
    let prefix = format!("{id}:{}", character.character_id);
    let cone_id = format!("{prefix}:light-cone:1");
    assert_eq!(first.account.light_cones[0].instance_id, cone_id);
    assert_eq!(character.light_cone_instance_id.as_ref(), Some(&cone_id));
    assert_eq!(first.account.relics.len(), 6);
    for relic in &first.account.relics {
        let relic_id = format!("{prefix}:relic:{}", u8::from(relic.slot));
        assert_eq!(relic.instance_id, relic_id);
        assert_eq!(
            character.relic_instance_ids.get(&relic.slot),
            Some(&relic_id)
        );
    }

    let named = AccountRequest::new(id, 1);
    let mut named_rng = ChaCha8Rng::seed_from_u64(42);
    let replay = generate_account(&named, &reference, &mut named_rng).unwrap();
    assert_eq!(replay, first);
}

/// 同一随机源连续生成空账号也会分别命名，显式传入空白名称仍属于无效请求。
#[test]
fn empty_accounts_get_individual_ids_and_explicit_blank_ids_are_invalid() {
    let reference = AccountReference::bundled().unwrap();
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let request: AccountRequest = serde_json::from_str(r#"{"character_count": 0}"#).unwrap();
    let first = generate_account(&request, &reference, &mut rng).unwrap();
    let second = generate_account(&request, &reference, &mut rng).unwrap();
    assert_ne!(first.account.config_id, second.account.config_id);
    let named: AccountRequest =
        serde_json::from_str(r#"{"account_id":"training","character_count":0}"#).unwrap();
    let explicit = generate_account(&named, &reference, &mut rng).unwrap();
    assert_eq!(explicit.account.config_id, "training");
    assert!(matches!(
        generate_account(&AccountRequest::new("  ", 0), &reference, &mut rng),
        Err(GenerationError::InvalidRequest(_))
    ));
}

/// 验证固定种子生成精确金数账号，并可序列化后转换为可玩配置。
#[test]
fn sample_has_exact_gold_and_converts_with_shared_account_core() {
    let reference = AccountReference::bundled().unwrap();
    let request = AccountRequest {
        gold_count: Some(20.0),
        core_character_ids: vec!["1310".to_owned(), "1225".to_owned()],
        resource_concentration: 0.7,
        min_mean_effective_rolls: Some(2.99),
        max_mean_effective_rolls: Some(3.01),
        ..AccountRequest::new("sample", 12)
    };
    let generated =
        generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    assert_eq!(generated.gold_count(), 20.0);
    assert!((2.99..=3.01).contains(&generated.mean_effective_rolls()));
    assert_eq!(generated.account.characters.len(), 12);
    assert_eq!(generated.account.light_cones.len(), 12);
    assert_eq!(generated.account.relics.len(), 72);
    assert!(
        generated
            .account
            .relics
            .iter()
            .all(|relic| relic.substats.len() == 4)
    );
    assert_eq!(
        generated,
        generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap()
    );
    let serialized = serde_json::to_string(&generated.account).unwrap();
    let account: RawAccount = serde_json::from_str(&serialized).unwrap();
    let playable = convert_playable_account(
        &account,
        &PlayableData::bundled().unwrap(),
        &ConversionRequest {
            config_id: "playable".to_owned(),
            character_aliases: BTreeMap::new(),
        },
    )
    .unwrap();
    assert!(
        playable
            .characters
            .iter()
            .any(|character| character.character_id == "1310"
                && matches!(character.origin, CharacterOrigin::Owned { .. }))
    );
    assert!(
        playable
            .characters
            .iter()
            .any(|character| character.origin == CharacterOrigin::Trial)
    );
}

/// 验证固定唯一光锥冲突返回无解，搜索预算为零时返回预算耗尽。
#[test]
fn fixed_light_cones_conflict_and_budget_exhaustion_are_reported() {
    let reference = AccountReference::bundled().unwrap();
    let spec = CharacterSpec {
        light_cone: LightConeSpec {
            light_cone_id: Some("24000".to_owned()),
            ..LightConeSpec::default()
        },
        ..CharacterSpec::default()
    };
    let request = AccountRequest {
        character_overrides: BTreeMap::from([
            ("1310".to_owned(), spec.clone()),
            ("1225".to_owned(), spec),
        ]),
        core_character_ids: [
            "1501", "1502", "1503", "1505", "1506", "1511", "1310", "1225",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        light_cone_pool: Some(vec!["23053".into()]),
        max_attempts: 1_000,
        ..AccountRequest::new("unique", 8)
    };
    let mut rng = ChaCha8Rng::seed_from_u64(2);
    assert!(matches!(
        generate_account(&request, &reference, &mut rng),
        Err(GenerationError::NoSolution)
    ));
    let request = AccountRequest {
        max_attempts: 0,
        ..AccountRequest::new("budget", 1)
    };
    assert!(matches!(
        generate_account(&request, &reference, &mut rng),
        Err(GenerationError::BudgetExhausted { attempts: 0 })
    ));
}

/// 验证单件遗器遵守指定强化档位、必需词条和精确有效分数。
#[test]
fn single_piece_respects_explicit_roll_tiers_and_required_stats() {
    let spec = RelicSpec {
        required_substats: vec![RelicSubStat::CritDmg],
        initial_substat_count: Some(4),
        min_effective_rolls: Some(6.6),
        ..RelicSpec::new(RelicSlot::Head)
    };
    let weights = BTreeMap::from([(RelicSubStat::CritDmg, 1.0)]);
    let (piece, score) = generate_relic(
        &spec,
        &weights,
        [0.0, 0.0, 1.0],
        10_000,
        &mut ChaCha8Rng::seed_from_u64(7),
    )
    .unwrap();
    assert_eq!(piece.mainstat, RelicMainStat::Hp);
    assert_eq!(piece.substats.len(), 4);
    assert!(!piece.substats.contains_key(&RelicSubStat::Hp));
    assert!((piece.substats[&RelicSubStat::CritDmg] - 6.0 * 0.0648).abs() < 1e-12);
    assert!((score - score_relic(&piece, &weights)).abs() < 1e-12);
}

/// 验证遗器搜索可在最后一次尝试成功，且整套生成按每件计入预算。
#[test]
fn full_tier_search_can_use_the_last_attempt_to_finish() {
    use account_generator::generate_character_relics;

    let spec = RelicSpec {
        initial_substat_count: Some(4),
        required_substats: vec![RelicSubStat::Spd],
        ..RelicSpec::new(RelicSlot::Head)
    };
    let weights = BTreeMap::from([(RelicSubStat::Spd, 1.0)]);
    let tiers = [1.0, 0.0, 1.0];
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    assert!(matches!(
        generate_relic(&spec, &weights, tiers, 0, &mut rng),
        Err(GenerationError::BudgetExhausted { attempts: 0 })
    ));
    let (piece, score) = generate_relic(&spec, &weights, tiers, 1, &mut rng).unwrap();
    assert!(piece.substats.contains_key(&RelicSubStat::Spd));
    assert!((score - score_relic(&piece, &weights)).abs() <= 1e-12);

    let request = RelicRequest {
        pieces: vec![spec],
        effective_stat_weights: weights,
        tier_weights: tiers,
        ..RelicRequest::default()
    };
    assert!(matches!(
        generate_character_relics(&request, 5, &mut rng),
        Err(GenerationError::BudgetExhausted { attempts: 5 })
    ));
    assert_eq!(
        generate_character_relics(&request, 6, &mut rng)
            .unwrap()
            .len(),
        6
    );
}

/// 验证中档目标不可达时，高档回退仍能在预算内满足套装总分。
#[test]
fn mixed_tier_fallback_keeps_the_budget_after_an_unreachable_middle_score() {
    let target = 5.0 + 2.6 / 2.3;
    let request = RelicRequest {
        pieces: (1..=6)
            .map(|slot| RelicSpec {
                initial_substat_count: Some(3),
                required_substats: vec![
                    RelicSubStat::Spd,
                    RelicSubStat::Def,
                    RelicSubStat::EffectRes,
                    RelicSubStat::EffectHitRate,
                ],
                min_effective_rolls: (slot < 6).then_some(1.0),
                max_effective_rolls: (slot < 6).then_some(1.0),
                ..RelicSpec::new(RelicSlot::try_from(slot).unwrap())
            })
            .collect(),
        effective_stat_weights: BTreeMap::from([(RelicSubStat::Spd, 1.0)]),
        tier_weights: [0.0, 1.0, 1.0],
        min_total_effective_rolls: Some(target),
        max_total_effective_rolls: Some(target),
    };
    let result = account_generator::generate_character_relics(
        &request,
        6,
        &mut ChaCha8Rng::seed_from_u64(42),
    )
    .unwrap();
    assert_eq!(result.len(), 6);
    assert_eq!(result[5].0.substats[&RelicSubStat::Spd], 2.6);
    assert!((result.iter().map(|(_, score)| score).sum::<f64>() - target).abs() <= 1e-12);
}

/// 验证中档网格按原始容差核验实际分数并拒绝偏差超限的目标。
#[test]
fn middle_grid_checks_the_actual_score_against_the_original_tolerance() {
    let target = 1.0 - 1e-6;
    let request = RelicRequest {
        pieces: (2..=6)
            .map(|slot| RelicSpec {
                max_effective_rolls: Some(0.0),
                ..RelicSpec::new(RelicSlot::try_from(slot).unwrap())
            })
            .collect(),
        effective_stat_weights: BTreeMap::from([(RelicSubStat::Spd, 1.00000000001)]),
        tier_weights: [0.0, 1.0, 0.0],
        min_total_effective_rolls: Some(target),
        max_total_effective_rolls: Some(target),
    };
    assert!(matches!(
        account_generator::generate_character_relics(
            &request,
            6,
            &mut ChaCha8Rng::seed_from_u64(42)
        ),
        Err(GenerationError::NoSolution)
    ));
}

/// 验证中档网格排除超过总分目标的候选并返回精确总分。
#[test]
fn middle_grid_discards_scores_above_the_requested_total() {
    let request = RelicRequest {
        effective_stat_weights: BTreeMap::from([
            (RelicSubStat::Spd, 100.0),
            (RelicSubStat::CritRate, 1.0),
        ]),
        tier_weights: [0.0, 1.0, 0.0],
        min_total_effective_rolls: Some(1.0),
        max_total_effective_rolls: Some(1.0),
        ..RelicRequest::default()
    };
    let result = account_generator::generate_character_relics(
        &request,
        6,
        &mut ChaCha8Rng::seed_from_u64(42),
    )
    .unwrap();
    assert_eq!(result.iter().map(|(_, score)| score).sum::<f64>(), 1.0);
}

/// 验证理论遗器分数上限依据合法强化数值计算，不受目标分数截断。
#[test]
fn theoretical_relic_upper_bound_uses_legal_rolls_before_score_targets() {
    let request = RelicRequest {
        pieces: vec![RelicSpec {
            initial_substat_count: Some(3),
            max_effective_rolls: Some(0.5),
            ..RelicSpec::new(RelicSlot::Head)
        }],
        effective_stat_weights: BTreeMap::from([(RelicSubStat::Spd, 1.0)]),
        tier_weights: [0.0, 1.0, 0.0],
        max_total_effective_rolls: Some(1.0),
        ..RelicRequest::default()
    };
    assert_eq!(
        account_generator::maximum_character_relic_score(&request).unwrap(),
        35.0
    );
}

/// 为流萤禁用中档强化，验证 60 人账号仍能在 400 金预算下生成 360 件遗器。
#[test]
fn full_roster_can_use_fallback_when_a_character_disables_middle_rolls() {
    let reference = AccountReference::bundled().unwrap();
    let request = AccountRequest {
        gold_count: Some(400.0),
        character_overrides: BTreeMap::from([(
            "1310".to_owned(),
            CharacterSpec {
                relics: Some(RelicRequest {
                    tier_weights: [1.0, 0.0, 1.0],
                    ..RelicRequest::default()
                }),
                ..CharacterSpec::default()
            },
        )]),
        ..AccountRequest::new("full-fallback", 60)
    };
    let result =
        generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    assert_eq!(result.account.relics.len(), 360);
    assert_eq!(result.gold_count(), 400.0);
}

/// 验证全角色名单满足精确遗器均分、金数预算并转换为可玩账号。
#[test]
fn full_roster_meets_an_exact_score_and_converts() {
    let reference = AccountReference::bundled().unwrap();
    let request = AccountRequest {
        gold_count: Some(400.0),
        min_mean_effective_rolls: Some(6.5),
        max_mean_effective_rolls: Some(6.5),
        max_attempts: 3_000,
        ..AccountRequest::new("full", 60)
    };
    let result =
        generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    assert_eq!(result.gold_count(), 400.0);
    assert_eq!(result.account.relics.len(), 360);
    assert!((result.mean_effective_rolls() - 6.5).abs() <= 1e-9);
    assert!(result.attempts <= request.max_attempts);
    let playable = convert_playable_account(
        &result.account,
        &PlayableData::bundled().unwrap(),
        &ConversionRequest {
            config_id: "full-playable".to_owned(),
            character_aliases: BTreeMap::new(),
        },
    )
    .unwrap();
    let owned: BTreeSet<_> = playable
        .characters
        .iter()
        .filter_map(|character| match &character.origin {
            CharacterOrigin::Owned { raw_character_id } => Some(raw_character_id),
            CharacterOrigin::Trial => None,
        })
        .collect();
    assert_eq!(
        owned,
        result
            .account
            .characters
            .iter()
            .map(|c| &c.character_id)
            .collect()
    );
}

/// 验证完整角色候选范围在搜索预算内达到精确及窄区间低分目标。
#[test]
fn full_catalog_can_meet_exact_and_narrow_lower_score_targets_within_the_budget() {
    let reference = AccountReference::bundled().unwrap();
    for (min, max, attempts) in [(3.0, 3.0, 3_000), (2.99, 3.01, 10_000)] {
        let request = AccountRequest {
            gold_count: Some(120.0),
            min_mean_effective_rolls: Some(min),
            max_mean_effective_rolls: Some(max),
            max_attempts: attempts,
            ..AccountRequest::new("full-low", 60)
        };
        let result =
            generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
        assert!((min - 1e-6..=max + 1e-6).contains(&result.mean_effective_rolls()));
        assert_eq!(result.account.relics.len(), 360);
        assert_eq!(result.gold_count(), 120.0);
        assert!(result.attempts <= attempts);
    }
}

/// 验证小额预算下可组合低档和高档速度强化达到账号精确分数。
#[test]
fn exact_score_can_combine_low_and_high_tiers_with_a_small_budget() {
    let mut pieces: Vec<_> = (1..=4)
        .map(|slot| RelicSpec {
            min_effective_rolls: Some(0.0),
            max_effective_rolls: Some(0.0),
            ..RelicSpec::new(RelicSlot::try_from(slot).unwrap())
        })
        .collect();
    for (slot, speed) in [(RelicSlot::PlanarSphere, 2.0), (RelicSlot::LinkRope, 2.6)] {
        pieces.push(RelicSpec {
            required_substats: vec![RelicSubStat::Spd],
            min_effective_rolls: Some(speed / 2.3 - 0.01),
            max_effective_rolls: Some(speed / 2.3 + 0.01),
            ..RelicSpec::new(slot)
        });
    }
    let request = AccountRequest {
        gold_count: Some(0.5),
        character_pool: Some(vec!["1004".to_owned()]),
        core_character_ids: vec!["1004".to_owned()],
        character_overrides: BTreeMap::from([(
            "1004".to_owned(),
            CharacterSpec {
                relics: Some(RelicRequest {
                    pieces,
                    effective_stat_weights: BTreeMap::from([(RelicSubStat::Spd, 1.0)]),
                    ..RelicRequest::default()
                }),
                ..CharacterSpec::default()
            },
        )]),
        min_mean_effective_rolls: Some(2.0 / 6.0),
        max_mean_effective_rolls: Some(2.0 / 6.0),
        max_attempts: 100,
        ..AccountRequest::new("mixed-tiers", 1)
    };
    let reference = AccountReference::bundled().unwrap();
    let result =
        generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    assert_eq!(result.account.relics[4].substats[&RelicSubStat::Spd], 2.0);
    assert_eq!(result.account.relics[5].substats[&RelicSubStat::Spd], 2.6);
}

/// 验证较宽账号分数区间会采样三种强化档位且结果保持在上下限内。
#[test]
fn wide_account_targets_sample_all_three_tiers_within_the_score_bounds() {
    let reference = AccountReference::bundled().unwrap();
    let mut pieces = vec![RelicSpec {
        required_substats: vec![RelicSubStat::Spd],
        max_effective_rolls: Some(1.5),
        ..RelicSpec::new(RelicSlot::Head)
    }];
    pieces.extend((2..=6).map(|slot| RelicSpec {
        max_effective_rolls: Some(0.0),
        ..RelicSpec::new(RelicSlot::try_from(slot).unwrap())
    }));
    let weights = BTreeMap::from([(RelicSubStat::Spd, 1.0)]);
    let request = AccountRequest {
        gold_count: Some(0.5),
        character_pool: Some(vec!["1004".to_owned()]),
        character_overrides: BTreeMap::from([(
            "1004".to_owned(),
            CharacterSpec {
                relics: Some(RelicRequest {
                    pieces,
                    effective_stat_weights: weights.clone(),
                    min_total_effective_rolls: Some(0.8),
                    max_total_effective_rolls: Some(1.2),
                    ..RelicRequest::default()
                }),
                ..CharacterSpec::default()
            },
        )]),
        min_mean_effective_rolls: Some(0.1),
        max_mean_effective_rolls: Some(0.25),
        max_attempts: 7,
        ..AccountRequest::new("wide", 1)
    };
    let speeds: Vec<_> = (0..24)
        .map(|seed| {
            let result =
                generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(seed))
                    .unwrap();
            let score: f64 = result
                .account
                .relics
                .iter()
                .map(|piece| score_relic(piece, &weights))
                .sum();
            assert!((0.8..=1.2).contains(&score));
            assert!((0.1..=0.25).contains(&(score / 6.0)));
            assert!((result.mean_effective_rolls() - score / 6.0).abs() <= 1e-12);
            assert_eq!(result.attempts, request.max_attempts);
            result.account.relics[0].substats[&RelicSubStat::Spd]
        })
        .collect();
    assert!([2.0, 2.3, 2.6].iter().all(|value| speeds.contains(value)));
}

/// 验证精确评分遗器的未计分速度词条仍可出现不同数值档位。
#[test]
fn exact_scored_piece_can_keep_numeric_variation_in_unscored_stats() {
    let spec = RelicSpec {
        required_substats: vec![RelicSubStat::CritDmg, RelicSubStat::Spd],
        min_effective_rolls: Some(1.0),
        max_effective_rolls: Some(1.0),
        ..RelicSpec::new(RelicSlot::Head)
    };
    let weights = BTreeMap::from([(RelicSubStat::CritDmg, 1.0)]);
    let mut has_non_middle_value = false;
    for seed in 0..12 {
        let (piece, score) = generate_relic(
            &spec,
            &weights,
            [1.0; 3],
            1,
            &mut ChaCha8Rng::seed_from_u64(seed),
        )
        .unwrap();
        assert!((score - 1.0).abs() <= 1e-9);
        assert!((score_relic(&piece, &weights) - 1.0).abs() <= 1e-9);
        let middle_rolls = piece.substats[&RelicSubStat::Spd] / 2.3;
        has_non_middle_value |= (middle_rolls - middle_rolls.round()).abs() > 1e-9;
    }
    assert!(has_non_middle_value);
}

/// 验证空账号可生成，固定统一星魂和光锥叠影时资源条件仍生效。
#[test]
fn empty_and_fixed_build_requests_preserve_resource_constraints() {
    let reference = AccountReference::bundled().unwrap();
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let empty = generate_account(&AccountRequest::new("empty", 0), &reference, &mut rng).unwrap();
    assert!(empty.account.characters.is_empty());
    assert_eq!(empty.gold_count(), 0.0);
    for eidolons in [0, 2] {
        let request = AccountRequest {
            character_pool: Some(vec!["1004".to_owned(), "1310".to_owned()]),
            five_star_build: Some((eidolons, 1)),
            relic_quality_tier: Some(3),
            ..AccountRequest::new("fixed", 2)
        };
        let result = generate_account(&request, &reference, &mut rng).unwrap();
        assert!(
            result
                .account
                .characters
                .iter()
                .all(|character| character.eidolons == eidolons)
        );
        assert!(
            result
                .account
                .light_cones
                .iter()
                .all(|cone| cone.superimposition == 1)
        );
        let score: f64 = result
            .investments
            .values()
            .flat_map(|i| i.piece_scores)
            .sum();
        let upper: f64 = result
            .investments
            .values()
            .map(|i| i.relic_score_upper)
            .sum();
        assert!((0.5..0.65).contains(&(score / upper)));
    }
}

/// 验证账号品质档按角色 ID 顺序汇总报告分数，并拒绝不可达档位。
#[test]
fn account_tier_uses_the_scores_reported_in_character_id_order() {
    use RelicSubStat::{AtkPercent, BreakEffect, CritDmg, CritRate, Def, EffectRes, Spd};
    let mut request = AccountRequest::new("mixed-high-rolls", 5);
    let profiles = [
        ("1004", CritRate, 0.0324, 0.029160001),
        ("1005", AtkPercent, 0.0432, 0.038880002),
        ("1103", CritDmg, 0.0648, 0.058320003),
        ("1105", Spd, 2.6, 2.3),
        ("1310", AtkPercent, 0.0432, 0.038880002),
    ];
    request.character_pool = Some(profiles.iter().map(|(id, ..)| id.to_string()).collect());
    for (id, stat, high, middle) in profiles {
        let score = 3.0 * high / middle;
        let pieces = (1..=6)
            .map(|n| RelicSpec {
                mainstat: vec![match n {
                    1 => RelicMainStat::Hp,
                    2 => RelicMainStat::Atk,
                    _ => RelicMainStat::HpPercent,
                }],
                initial_substat_count: Some(4),
                required_substats: vec![Def, EffectRes, BreakEffect, stat],
                min_effective_rolls: Some(score),
                max_effective_rolls: Some(score),
                ..RelicSpec::new(RelicSlot::try_from(n).unwrap())
            })
            .collect();
        request.character_overrides.insert(
            id.into(),
            CharacterSpec {
                relics: Some(RelicRequest {
                    pieces,
                    effective_stat_weights: BTreeMap::from([(stat, 1.0)]),
                    tier_weights: [0.0, 0.0, 1.0],
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
    }
    let reference = AccountReference::bundled().unwrap();
    request.relic_quality_tier = Some(2);
    let result =
        generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    let total: f64 = result
        .investments
        .values()
        .map(|i| i.piece_scores.iter().sum::<f64>())
        .sum();
    let upper: f64 = result
        .investments
        .values()
        .map(|i| i.relic_score_upper)
        .sum();
    assert!(total < 0.5 * upper);
    assert_eq!(result.mean_effective_rolls(), total / 30.0);
    request.relic_quality_tier = Some(3);
    assert!(matches!(
        generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)),
        Err(GenerationError::NoSolution)
    ));
}

/// 验证单角色生成保留指定等级、光锥配置及早期行迹状态。
#[test]
fn individual_character_keeps_explicit_equipment_and_early_traces() {
    let reference = AccountReference::bundled().unwrap();
    let spec = CharacterSpec {
        level: 20,
        promotion: 0,
        light_cone: LightConeSpec {
            light_cone_id: Some("24000".to_owned()),
            level: 20,
            promotion: 0,
            superimposition: 3,
        },
        ..CharacterSpec::default()
    };
    let generated = generate_character(
        "early",
        "1310",
        &spec,
        &reference,
        10_000,
        &mut ChaCha8Rng::seed_from_u64(42),
    )
    .unwrap();
    let character = &generated.account.characters[0];
    assert_eq!(character.ability_version, Some(0));
    assert_eq!(character.traces.skill_levels["1310001"], 1);
    assert!(!character.traces.node_unlocks["1310101"]);
    assert_eq!(generated.account.light_cones[0].light_cone_id, "24000");
    assert_eq!(generated.account.light_cones[0].superimposition, 3);
    convert_playable_account(
        &generated.account,
        &PlayableData::bundled().unwrap(),
        &ConversionRequest {
            config_id: "early-playable".to_owned(),
            character_aliases: BTreeMap::new(),
        },
    )
    .unwrap();
}

/// 验证零权重遗器可在每件一次的搜索预算内完成生成。
#[test]
fn zero_weight_relics_fit_exactly_one_attempt_per_piece() {
    let reference = AccountReference::bundled().unwrap();
    let request = AccountRequest {
        gold_count: Some(0.5),
        character_pool: Some(vec!["1004".to_owned()]),
        core_character_ids: vec!["1004".to_owned()],
        character_overrides: BTreeMap::from([(
            "1004".to_owned(),
            CharacterSpec {
                relics: Some(RelicRequest::default()),
                ..CharacterSpec::default()
            },
        )]),
        max_attempts: 7,
        ..AccountRequest::new("zero", 1)
    };
    let generated =
        generate_account(&request, &reference, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    assert_eq!(generated.mean_effective_rolls(), 0.0);
    assert_eq!(generated.attempts, 7);
}
