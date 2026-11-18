use std::collections::{BTreeMap, BTreeSet};

use account_core::{RelicInstance, RelicMainStat as Main, RelicSlot as Slot, RelicSubStat as Sub};
use account_generator::{
    AccountReference, ChaCha8Rng, CharacterSpec, GenerationError, RelicRequest, RelicSpec,
    SeedableRng, generate_character, generate_character_relics, generate_relic, score_relic,
};

const MID: [f64; 3] = [0.0, 1.0, 0.0];
const ALL: [f64; 3] = [1.0; 3];

// 4.7 的单次低、中、高档数值。独立校验输出的单位和强化次数。
const VALUES: [(Sub, [f64; 3]); 12] = [
    (Sub::Hp, [33.87004, 38.103798, 42.33755]),
    (Sub::Atk, [16.935019, 19.051895, 21.168774]),
    (Sub::Def, [16.935019, 19.051895, 21.168774]),
    (Sub::HpPercent, [0.034560003, 0.038880002, 0.0432]),
    (Sub::AtkPercent, [0.034560003, 0.038880002, 0.0432]),
    (Sub::DefPercent, [0.0432, 0.0486, 0.054]),
    (Sub::Spd, [2.0, 2.3, 2.6]),
    (Sub::CritRate, [0.02592, 0.029160001, 0.0324]),
    (Sub::CritDmg, [0.05184, 0.058320003, 0.0648]),
    (Sub::EffectHitRate, [0.034560003, 0.038880002, 0.0432]),
    (Sub::EffectRes, [0.034560003, 0.038880002, 0.0432]),
    (Sub::BreakEffect, [0.05184, 0.058320003, 0.0648]),
];

fn all_stats() -> BTreeMap<Sub, f64> {
    VALUES.into_iter().map(|(stat, _)| (stat, 1.0)).collect()
}

#[test]
fn scoring_speed_alone_does_not_change_its_unconstrained_drop_rate() {
    let weights = BTreeMap::from([(Sub::Spd, 1.0)]);
    let spec = RelicSpec::new(Slot::Head);
    let mut with_speed = 0;
    for seed in 0..1024 {
        let (piece, _) = generate_relic(
            &spec,
            &weights,
            ALL,
            1,
            &mut ChaCha8Rng::seed_from_u64(seed),
        )
        .unwrap();
        with_speed += usize::from(piece.substats.contains_key(&Sub::Spd));
    }
    // 头部有 11 种合法副词条，每件取 4 种。期望约 372 件，标准差约 15 件。
    assert!((295..=450).contains(&with_speed), "{with_speed}");
}

#[test]
fn equal_scoring_substats_receive_independent_upgrades() {
    let stats = [Sub::AtkPercent, Sub::Spd, Sub::CritRate, Sub::CritDmg];
    let spec = RelicSpec {
        required_substats: stats.to_vec(),
        initial_substat_count: Some(4),
        ..RelicSpec::new(Slot::Head)
    };
    let weights = stats.map(|stat| (stat, 1.0)).into();
    let mut totals = [0; 4];
    let mut all_on_one = 0;
    for seed in 0..2048 {
        let (piece, _) = generate_relic(
            &spec,
            &weights,
            MID,
            1,
            &mut ChaCha8Rng::seed_from_u64(seed),
        )
        .unwrap();
        let hits = stats.map(|stat| {
            let (_, rolls) = VALUES.iter().find(|(id, _)| *id == stat).unwrap();
            (piece.substats[&stat] / rolls[1]).round() as usize - 1
        });
        assert_eq!(hits.iter().sum::<usize>(), 5);
        for (total, n) in totals.iter_mut().zip(hits) {
            *total += n;
        }
        all_on_one += usize::from(hits.contains(&5));
    }
    // 每条平均占 1/4。五次全中任一条的概率为 1/256，2048 件中期望 8 件。
    assert!(
        totals.iter().all(|n| (2310..=2810).contains(n)),
        "{totals:?}"
    );
    assert!(all_on_one <= 25, "{all_on_one}");
}

#[test]
fn exact_crit_score_samples_the_five_upgrades_by_roll_probability() {
    let spec = RelicSpec {
        required_substats: vec![Sub::AtkPercent, Sub::Spd, Sub::CritRate, Sub::CritDmg],
        initial_substat_count: Some(4),
        min_effective_rolls: Some(7.0),
        max_effective_rolls: Some(7.0),
        ..RelicSpec::new(Slot::Head)
    };
    let weights = BTreeMap::from([(Sub::CritRate, 1.0), (Sub::CritDmg, 1.0)]);
    let mut on_one = 0;
    for seed in 0..1024 {
        let (piece, score) = generate_relic(
            &spec,
            &weights,
            MID,
            1,
            &mut ChaCha8Rng::seed_from_u64(seed),
        )
        .unwrap();
        assert!((score - 7.0).abs() < 1e-9);
        let hits = (piece.substats[&Sub::CritRate] / 0.029160001).round() as usize - 1;
        on_one += usize::from(matches!(hits, 0 | 5));
    }
    // 目标要求五次全部落在双暴上，其中全部落在同一条的条件概率是 2 / 2^5。
    assert!((27..=103).contains(&on_one), "{on_one}");
}

#[test]
fn a_set_score_range_samples_compensating_low_and_high_rolls() {
    // 头部的攻击百分比与效果抵抗共得约两分，其余五件得零分。
    // 两条各有一次数值，允许低+高、中+中、高+低三种等概率搭配。
    let request = RelicRequest {
        pieces: (1..=6)
            .map(|slot| RelicSpec {
                required_substats: if slot == 1 {
                    vec![Sub::AtkPercent, Sub::EffectRes, Sub::Spd, Sub::Def]
                } else {
                    vec![]
                },
                max_effective_rolls: (slot != 1).then_some(0.0),
                ..RelicSpec::new(Slot::try_from(slot).unwrap())
            })
            .collect(),
        effective_stat_weights: BTreeMap::from([(Sub::AtkPercent, 1.0), (Sub::EffectRes, 1.0)]),
        min_total_effective_rolls: Some(1.999),
        max_total_effective_rolls: Some(2.001),
        ..Default::default()
    };
    let mut mid = 0;
    for seed in 0..384 {
        let pieces =
            generate_character_relics(&request, 6, &mut ChaCha8Rng::seed_from_u64(seed)).unwrap();
        let score: f64 = pieces.iter().map(|(_, value)| value).sum();
        assert!((1.999..=2.001).contains(&score));
        mid += usize::from((pieces[0].0.substats[&Sub::AtkPercent] - 0.038880002).abs() < 1e-10);
    }
    assert!((82..=175).contains(&mid), "{mid} middle rolls");
}

#[test]
fn a_set_score_target_keeps_substats_that_need_a_low_roll() {
    let mut pieces: Vec<_> = (1..=6)
        .map(|slot| RelicSpec {
            required_substats: vec![Sub::Def, Sub::EffectRes, Sub::BreakEffect],
            initial_substat_count: Some(4),
            ..RelicSpec::new(Slot::try_from(slot).unwrap())
        })
        .collect();
    pieces[0].min_effective_rolls = Some(0.8);
    pieces[0].max_effective_rolls = Some(0.8);
    for spec in &mut pieces[2..] {
        spec.required_substats.push(Sub::AtkPercent);
    }
    let request = RelicRequest {
        pieces,
        effective_stat_weights: BTreeMap::from([(Sub::Spd, 0.92), (Sub::CritRate, 0.8)]),
        min_total_effective_rolls: Some(1.6),
        max_total_effective_rolls: Some(1.6),
        ..Default::default()
    };
    let mut with_speed = 0;
    for seed in 0..512 {
        let pieces =
            generate_character_relics(&request, 6, &mut ChaCha8Rng::seed_from_u64(seed)).unwrap();
        assert!((pieces.iter().map(|(_, s)| s).sum::<f64>() - 1.6).abs() < 1e-9);
        assert!((score_relic(&pieces[0].0, &request.effective_stat_weights) - 0.8).abs() < 1e-9);
        with_speed += usize::from(pieces[0].0.substats.contains_key(&Sub::Spd));
    }
    // 只有一次低档速度或一次中档暴击率恰好得 0.8 分，两种出现概率相同。
    assert!(
        (197..=315).contains(&with_speed),
        "{with_speed} heads with speed"
    );
}

fn valid_roll_counts(piece: &RelicInstance, tiers: [f64; 3]) -> BTreeSet<u8> {
    assert_eq!(
        (piece.rarity, piece.level, piece.substats.len()),
        (5, 15, 4)
    );
    let mut totals = BTreeSet::from([0]);
    for (&stat, &actual) in &piece.substats {
        let (_, rolls) = VALUES.iter().find(|(key, _)| *key == stat).unwrap();
        let mut counts = BTreeSet::new();
        for low in 0..=6 {
            for mid in 0..=6 - low {
                for high in 0..=6 - low - mid {
                    let rolls_count = [low, mid, high];
                    let count: u8 = rolls_count.iter().sum();
                    if count == 0
                        || rolls_count
                            .iter()
                            .zip(tiers)
                            .any(|(&n, weight)| n > 0 && weight == 0.0)
                    {
                        continue;
                    }
                    let possible: f64 = rolls
                        .iter()
                        .zip(rolls_count)
                        .map(|(roll, n)| roll * f64::from(n))
                        .sum();
                    if (possible - actual).abs() < 1e-12 {
                        counts.insert(count);
                    }
                }
            }
        }
        totals = totals
            .iter()
            .flat_map(|sum| counts.iter().map(move |count| sum + count))
            .collect();
    }
    totals
}

#[test]
fn generated_relics_have_legal_main_stats_and_roll_decompositions() {
    let cases = [
        (Slot::Head, Main::Hp, Some(Sub::Hp), Sub::HpPercent),
        (Slot::Hands, Main::Atk, Some(Sub::Atk), Sub::AtkPercent),
        (Slot::Body, Main::CritRate, Some(Sub::CritRate), Sub::Hp),
        (Slot::Feet, Main::Spd, Some(Sub::Spd), Sub::AtkPercent),
        (
            Slot::PlanarSphere,
            Main::FireDmgBoost,
            None,
            Sub::AtkPercent,
        ),
        (
            Slot::LinkRope,
            Main::BreakEffect,
            Some(Sub::BreakEffect),
            Sub::Spd,
        ),
    ];
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    for (slot, main, excluded, required) in cases {
        for (initial, tiers) in [(3, ALL), (4, [1.0, 0.0, 1.0])] {
            let spec = RelicSpec {
                mainstat: vec![main],
                required_substats: vec![required],
                initial_substat_count: Some(initial),
                ..RelicSpec::new(slot)
            };
            let (piece, _) = generate_relic(&spec, &BTreeMap::new(), tiers, 1, &mut rng).unwrap();
            assert_eq!(piece.mainstat, main);
            assert!(piece.substats.contains_key(&required));
            assert!(excluded.is_none_or(|stat| !piece.substats.contains_key(&stat)));
            assert!(valid_roll_counts(&piece, tiers).contains(&(initial + 5)));
            let json = serde_json::to_value(&piece).unwrap();
            assert_eq!(json["slot"].as_u64(), Some(u64::from(u8::from(slot))));
        }
    }
}

#[test]
fn initial_three_and_four_rolls_are_both_sampled_and_score_targets_can_force_either() {
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let mut three = 0;
    let spec = RelicSpec::new(Slot::Head);
    for _ in 0..140 {
        let (piece, _) = generate_relic(&spec, &BTreeMap::new(), MID, 1, &mut rng).unwrap();
        let counts = valid_roll_counts(&piece, MID);
        assert!(matches!(counts.first(), Some(8 | 9)));
        three += usize::from(counts.contains(&8));
    }
    assert!((83..=117).contains(&three), "initial three: {three}");

    for target in [8.0, 9.0] {
        let spec = RelicSpec {
            min_effective_rolls: Some(target),
            max_effective_rolls: Some(target),
            ..spec.clone()
        };
        let (piece, score) = generate_relic(&spec, &all_stats(), MID, 1, &mut rng).unwrap();
        assert_eq!(score, target);
        assert_eq!(
            valid_roll_counts(&piece, MID),
            BTreeSet::from([target as u8])
        );
    }
}

#[test]
fn provided_six_piece_order_and_piece_and_total_bounds_are_all_preserved() {
    let request = RelicRequest {
        pieces: vec![
            RelicSpec {
                set_id: Some("306".into()),
                mainstat: vec![Main::AtkPercent],
                initial_substat_count: Some(3),
                max_effective_rolls: Some(8.0),
                ..RelicSpec::new(Slot::LinkRope)
            },
            RelicSpec {
                set_id: Some("115".into()),
                initial_substat_count: Some(4),
                min_effective_rolls: Some(9.0),
                ..RelicSpec::new(Slot::Head)
            },
        ],
        effective_stat_weights: all_stats(),
        tier_weights: MID,
        min_total_effective_rolls: Some(49.0),
        max_total_effective_rolls: Some(49.0),
    };
    let generate =
        || generate_character_relics(&request, 6, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    let result = generate();
    assert_eq!(result, generate());
    assert_eq!(
        result
            .iter()
            .map(|(piece, _)| u8::from(piece.slot))
            .collect::<Vec<_>>(),
        [6, 1, 2, 3, 4, 5]
    );
    assert_eq!((result[0].0.set_id.as_str(), result[0].1), ("306", 8.0));
    assert_eq!((result[1].0.set_id.as_str(), result[1].1), ("115", 9.0));
    assert_eq!(result.iter().map(|(_, score)| score).sum::<f64>(), 49.0);
    for (piece, score) in result {
        assert!((score_relic(&piece, &request.effective_stat_weights) - score).abs() < 1e-12);
    }
}

#[test]
fn main_stat_candidates_are_filtered_by_required_stats_and_score_reachability() {
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let mut spec = RelicSpec {
        set_id: Some("306".into()),
        mainstat: vec![Main::HpPercent, Main::DefPercent],
        required_substats: vec![Sub::HpPercent],
        ..RelicSpec::new(Slot::PlanarSphere)
    };
    let weights = BTreeMap::from([(Sub::HpPercent, 1.0)]);
    let (piece, _) = generate_relic(&spec, &weights, MID, 1, &mut rng).unwrap();
    assert_eq!(piece.mainstat, Main::DefPercent);
    assert!(piece.substats.contains_key(&Sub::HpPercent));
    spec.required_substats.clear();
    spec.min_effective_rolls = Some(5.0);
    spec.max_effective_rolls = Some(5.0);
    let (piece, score) = generate_relic(&spec, &weights, MID, 1, &mut rng).unwrap();
    assert_eq!(piece.mainstat, Main::DefPercent);
    assert_eq!(score, 5.0);
}

#[test]
fn scoring_weights_use_a_signed_hundred_point_scale() {
    let spec = RelicSpec {
        required_substats: vec![Sub::Spd],
        ..RelicSpec::new(Slot::Head)
    };
    for weight in [-100.0, 100.0, -100.01, 100.01, 1e16, f64::NAN] {
        let result = generate_relic(
            &spec,
            &BTreeMap::from([(Sub::Spd, weight)]),
            MID,
            1,
            &mut ChaCha8Rng::seed_from_u64(42),
        );
        if (-100.0..=100.0).contains(&weight) {
            assert!(result.is_ok(), "{weight}: {result:?}");
        } else {
            assert!(
                matches!(result, Err(GenerationError::InvalidRequest(_))),
                "{weight}: {result:?}"
            );
        }
    }
}

#[test]
fn a_half_strength_set_is_assigned_to_just_one_quality_tier() {
    // 三次中档速度约为 3 分，三次中档暴击恰好为 3 分。六件理论上限为 36。
    let speed = 1.0 - 2e-8;
    let request = RelicRequest {
        pieces: (1..=6)
            .map(|n| RelicSpec {
                required_substats: vec![Sub::Def, Sub::EffectRes, Sub::BreakEffect],
                initial_substat_count: Some(4),
                min_effective_rolls: Some(3.0 * speed),
                max_effective_rolls: Some(3.0 * speed),
                ..RelicSpec::new(Slot::try_from(n).unwrap())
            })
            .collect(),
        effective_stat_weights: BTreeMap::from([(Sub::Spd, speed), (Sub::CritRate, 1.0)]),
        tier_weights: MID,
        ..Default::default()
    };
    let reference = AccountReference::bundled().unwrap();
    for tier in [2, 3] {
        let spec = CharacterSpec {
            relic_quality_tier: Some(tier),
            relics: Some(request.clone()),
            ..Default::default()
        };
        for seed in [0, 36] {
            let account = generate_character(
                "boundary",
                "1310",
                &spec,
                &reference,
                1000,
                &mut ChaCha8Rng::seed_from_u64(seed),
            )
            .unwrap();
            let investment = &account.investments["1310"];
            assert_eq!(investment.relic_score_upper, 36.0);
            let total: f64 = investment.piece_scores.iter().sum();
            assert_eq!(
                total < 18.0,
                tier == 2,
                "tier {tier}, seed {seed}, score {total}"
            );
        }
    }
}

#[test]
fn signed_stat_weights_preserve_feasible_negative_and_positive_score_extremes() {
    let weights = BTreeMap::from([
        (Sub::HpPercent, 4.0),
        (Sub::Spd, -1.0),
        (Sub::AtkPercent, 3.0),
        (Sub::DefPercent, 100.0),
    ]);
    for target in [-2.0, 26.0] {
        let spec = RelicSpec {
            mainstat: vec![Main::HpPercent, Main::DefPercent],
            required_substats: vec![Sub::HpPercent, Sub::Spd],
            min_effective_rolls: Some(target),
            max_effective_rolls: Some(target),
            ..RelicSpec::new(Slot::Body)
        };
        let (piece, score) =
            generate_relic(&spec, &weights, MID, 1, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
        assert_eq!(piece.mainstat, Main::DefPercent);
        assert!((score - target).abs() < 1e-12);
        assert!((score_relic(&piece, &weights) - target).abs() < 1e-12);
    }
    for (tier, value) in [([1.0, 0.0, 0.0], 2.0), ([0.0, 0.0, 1.0], 2.6)] {
        let target = -(value / 2.3);
        let spec = RelicSpec {
            required_substats: vec![Sub::Spd],
            min_effective_rolls: Some(target),
            max_effective_rolls: Some(target),
            ..RelicSpec::new(Slot::Head)
        };
        let (piece, _) = generate_relic(
            &spec,
            &BTreeMap::from([(Sub::Spd, -1.0)]),
            tier,
            1,
            &mut ChaCha8Rng::seed_from_u64(7),
        )
        .unwrap();
        assert_eq!(piece.substats[&Sub::Spd], value);
    }
}

#[test]
fn invalid_requests_and_discrete_unreachable_scores_report_distinct_errors() {
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let spec = RelicSpec::new(Slot::Head);
    for weights in [
        [0.0; 3],
        [f64::NAN, 1.0, 1.0],
        [1.0, f64::INFINITY, 1.0],
        [1.0, -1.0, 0.0],
    ] {
        assert!(matches!(
            generate_relic(&spec, &BTreeMap::new(), weights, 0, &mut rng),
            Err(GenerationError::InvalidRequest(_))
        ));
    }
    for invalid in [
        RelicSpec {
            set_id: Some("301".into()),
            ..spec.clone()
        },
        RelicSpec {
            mainstat: vec![Main::Spd],
            ..spec.clone()
        },
        RelicSpec {
            initial_substat_count: Some(5),
            ..spec.clone()
        },
        RelicSpec {
            required_substats: vec![Sub::Spd, Sub::Spd],
            ..spec.clone()
        },
        RelicSpec {
            min_effective_rolls: Some(2.0),
            max_effective_rolls: Some(1.0),
            ..spec.clone()
        },
    ] {
        assert!(matches!(
            generate_relic(&invalid, &BTreeMap::new(), MID, 0, &mut rng),
            Err(GenerationError::InvalidRequest(_))
        ));
    }
    let impossible = RelicSpec {
        min_effective_rolls: Some(0.1),
        max_effective_rolls: Some(0.1),
        ..spec
    };
    assert!(matches!(
        generate_relic(
            &impossible,
            &BTreeMap::from([(Sub::Spd, 1.0)]),
            ALL,
            100,
            &mut rng
        ),
        Err(GenerationError::NoSolution)
    ));
}

#[test]
fn any_positive_tier_weight_is_still_eligible_for_exact_search() {
    let target = 2.0 / 2.3;
    let spec = RelicSpec {
        required_substats: vec![Sub::Spd],
        min_effective_rolls: Some(target),
        max_effective_rolls: Some(target),
        ..RelicSpec::new(Slot::Head)
    };
    let (piece, score) = generate_relic(
        &spec,
        &BTreeMap::from([(Sub::Spd, 1.0)]),
        [1e-300, 1e308, 1e308],
        1,
        &mut ChaCha8Rng::seed_from_u64(42),
    )
    .unwrap();
    assert_eq!(piece.substats[&Sub::Spd], 2.0);
    assert_eq!(score, target);
}

#[test]
fn all_character_quality_tiers_use_the_same_legal_upper_and_apply_score_overrides() {
    let reference = AccountReference::bundled().unwrap();
    let ratios = [0.0, 0.35, 0.5, 0.65, 0.8, 1.0];
    let mut spec = CharacterSpec {
        relics: Some(RelicRequest {
            pieces: (1..=6)
                .map(|id| {
                    let slot = Slot::try_from(id).unwrap();
                    RelicSpec {
                        mainstat: vec![match slot {
                            Slot::Head => Main::Hp,
                            Slot::Hands => Main::Atk,
                            _ => Main::HpPercent,
                        }],
                        ..RelicSpec::new(slot)
                    }
                })
                .collect(),
            effective_stat_weights: BTreeMap::from([(Sub::CritRate, 1.0), (Sub::CritDmg, 1.0)]),
            tier_weights: MID,
            ..RelicRequest::default()
        }),
        ..CharacterSpec::default()
    };
    let generate = |spec: &CharacterSpec| {
        generate_character(
            "quality",
            "1310",
            spec,
            &reference,
            10_000,
            &mut ChaCha8Rng::seed_from_u64(42),
        )
    };
    for tier in 1..=5 {
        spec.relic_quality_tier = Some(tier);
        let result = generate(&spec).unwrap();
        let stats = &result.investments["1310"];
        assert_eq!(stats.relic_score_upper, 42.0);
        let score: f64 = stats.piece_scores.iter().sum();
        let tier = usize::from(tier);
        assert!(score >= ratios[tier - 1] * 42.0 - 1e-6);
        assert!(score < ratios[tier] * 42.0 || (tier == 5 && score == 42.0));
    }
    let relics = spec.relics.as_mut().unwrap();
    relics.min_total_effective_rolls = Some(42.0);
    relics.max_total_effective_rolls = Some(42.0);
    assert_eq!(generate(&spec).unwrap().mean_effective_rolls(), 7.0);
    let relics = spec.relics.as_mut().unwrap();
    relics.min_total_effective_rolls = None;
    relics.max_total_effective_rolls = Some(0.0);
    assert!(matches!(generate(&spec), Err(GenerationError::NoSolution)));
}

#[test]
fn unscored_training_relics_can_spread_upgrades_across_multiple_substats() {
    let spec = RelicSpec {
        initial_substat_count: Some(4),
        required_substats: vec![Sub::Atk, Sub::Def, Sub::Spd, Sub::BreakEffect],
        ..RelicSpec::new(Slot::Head)
    };
    let mut split = false;
    for seed in 0..32 {
        let (piece, _) = generate_relic(
            &spec,
            &BTreeMap::new(),
            MID,
            1,
            &mut ChaCha8Rng::seed_from_u64(seed),
        )
        .unwrap();
        let counts: Vec<_> = VALUES
            .iter()
            .filter_map(|(stat, rolls)| {
                piece
                    .substats
                    .get(stat)
                    .map(|value| (value / rolls[1]).round() as u8)
            })
            .collect();
        assert_eq!(counts.iter().sum::<u8>(), 9);
        split |= counts.iter().filter(|&&n| n > 1).count() >= 2;
    }
    assert!(split, "all five upgrades always landed on a single substat");
}

#[test]
fn rounded_piece_limits_still_allow_the_requested_set_total() {
    let request = RelicRequest {
        pieces: (1..=6)
            .map(|slot| RelicSpec {
                initial_substat_count: Some(3),
                max_effective_rolls: Some(8.0 - 0.75e-6),
                ..RelicSpec::new(Slot::try_from(slot).unwrap())
            })
            .collect(),
        effective_stat_weights: all_stats(),
        tier_weights: MID,
        min_total_effective_rolls: Some(48.0),
        max_total_effective_rolls: Some(48.0),
    };
    let pieces =
        generate_character_relics(&request, 6, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    assert!(pieces.iter().all(|(_, score)| *score == 8.0));
}

#[test]
fn a_conflicting_rope_is_rejected_before_spending_the_remaining_budget() {
    let spec = CharacterSpec {
        relics: Some(RelicRequest {
            pieces: vec![RelicSpec {
                mainstat: vec![Main::BreakEffect],
                required_substats: vec![Sub::BreakEffect],
                ..RelicSpec::new(Slot::LinkRope)
            }],
            effective_stat_weights: BTreeMap::from([(Sub::Spd, 1.0)]),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(matches!(
        generate_character(
            "rope",
            "1310",
            &spec,
            &AccountReference::bundled().unwrap(),
            3,
            &mut ChaCha8Rng::seed_from_u64(42)
        ),
        Err(GenerationError::NoSolution)
    ));
}

#[test]
fn half_of_the_high_roll_maximum_belongs_to_the_third_tier() {
    let per_piece = 3.0 * 0.0648 / 0.058320003;
    let request = RelicRequest {
        pieces: (1..=6)
            .map(|n| RelicSpec {
                initial_substat_count: Some(4),
                required_substats: vec![Sub::Def, Sub::EffectRes, Sub::BreakEffect, Sub::CritDmg],
                min_effective_rolls: Some(per_piece),
                max_effective_rolls: Some(per_piece),
                ..RelicSpec::new(Slot::try_from(n).unwrap())
            })
            .collect(),
        effective_stat_weights: BTreeMap::from([(Sub::CritDmg, 1.0)]),
        tier_weights: [0.0, 0.0, 1.0],
        ..Default::default()
    };
    let reference = AccountReference::bundled().unwrap();
    let generate = |tier| {
        generate_character(
            "half",
            "1310",
            &CharacterSpec {
                relic_quality_tier: Some(tier),
                relics: Some(request.clone()),
                ..Default::default()
            },
            &reference,
            1000,
            &mut ChaCha8Rng::seed_from_u64(42),
        )
    };
    assert!(matches!(generate(2), Err(GenerationError::NoSolution)));
    let account = generate(3).unwrap();
    let investment = &account.investments["1310"];
    assert_eq!(
        investment.piece_scores.iter().sum::<f64>(),
        investment.relic_score_upper * 0.5
    );
}

#[test]
fn graduation_tier_can_reach_the_high_roll_maximum() {
    let reference = AccountReference::bundled().unwrap();
    for stat in [Sub::CritRate, Sub::Spd] {
        let mut request = RelicRequest {
            pieces: (1..=6)
                .map(|slot| RelicSpec {
                    required_substats: vec![Sub::Def, Sub::EffectRes, Sub::BreakEffect, stat],
                    initial_substat_count: Some(4),
                    ..RelicSpec::new(Slot::try_from(slot).unwrap())
                })
                .collect(),
            effective_stat_weights: BTreeMap::from([(stat, 1.0)]),
            tier_weights: [0.0, 0.0, 1.0],
            ..Default::default()
        };
        let upper = account_generator::maximum_character_relic_score(&request).unwrap();
        request.min_total_effective_rolls = Some(upper);
        let spec = CharacterSpec {
            relic_quality_tier: Some(5),
            relics: Some(request),
            ..Default::default()
        };
        let account = generate_character(
            "max",
            "1310",
            &spec,
            &reference,
            1000,
            &mut ChaCha8Rng::seed_from_u64(42),
        )
        .unwrap();
        let score: f64 = account.investments["1310"].piece_scores.iter().sum();
        assert!((score - upper).abs() < 1e-10, "{stat:?}: {score} / {upper}");
    }
}

#[test]
fn fifth_tier_accepts_a_positive_upper_in_normalized_score_units() {
    let spec = CharacterSpec {
        relics: Some(RelicRequest {
            effective_stat_weights: VALUES.iter().map(|(stat, _)| (*stat, 1e-8)).collect(),
            tier_weights: MID,
            ..Default::default()
        }),
        relic_quality_tier: Some(5),
        ..Default::default()
    };
    let result = generate_character(
        "normalized",
        "1310",
        &spec,
        &AccountReference::bundled().unwrap(),
        500,
        &mut ChaCha8Rng::seed_from_u64(42),
    )
    .unwrap();
    let investment = &result.investments["1310"];
    assert!(investment.piece_scores.iter().sum::<f64>() >= 0.8 * investment.relic_score_upper);
}

#[test]
fn equally_weighted_crit_stats_can_share_the_upgrades_on_a_finished_head_piece() {
    let spec = RelicSpec {
        required_substats: vec![Sub::CritRate, Sub::CritDmg, Sub::AtkPercent, Sub::Spd],
        initial_substat_count: Some(4),
        min_effective_rolls: Some(7.8),
        max_effective_rolls: Some(7.8),
        ..RelicSpec::new(Slot::Head)
    };
    let weights = BTreeMap::from([
        (Sub::CritRate, 1.0),
        (Sub::CritDmg, 1.0),
        (Sub::AtkPercent, 0.5),
        (Sub::Spd, 0.3),
    ]);
    let mut splits = BTreeSet::new();
    for seed in 0..32 {
        let (piece, _) = generate_relic(
            &spec,
            &weights,
            MID,
            1,
            &mut ChaCha8Rng::seed_from_u64(seed),
        )
        .unwrap();
        assert!((score_relic(&piece, &weights) - 7.8).abs() < 1e-9);
        let rate = (piece.substats[&Sub::CritRate] / 0.029160001).round() as u8;
        let dmg = (piece.substats[&Sub::CritDmg] / 0.058320003).round() as u8;
        assert_eq!(rate + dmg, 7);
        splits.insert((rate, dmg));
    }
    assert!(splits.iter().any(|&(rate, dmg)| rate > 1 && dmg > 1));
    assert!(splits.len() > 1);
}

#[test]
fn an_exact_speed_piece_can_roll_its_three_other_substats_at_different_tiers() {
    let spec = RelicSpec {
        initial_substat_count: Some(3),
        required_substats: vec![Sub::Spd, Sub::HpPercent, Sub::DefPercent, Sub::EffectRes],
        min_effective_rolls: Some(11.8 / 2.3),
        max_effective_rolls: Some(11.8 / 2.3),
        ..RelicSpec::new(Slot::Head)
    };
    let mut mixed = false;
    for seed in 0..32 {
        let (piece, _) = generate_relic(
            &spec,
            &BTreeMap::from([(Sub::Spd, 1.0)]),
            ALL,
            100,
            &mut ChaCha8Rng::seed_from_u64(seed),
        )
        .unwrap();
        assert!((piece.substats[&Sub::Spd] - 11.8).abs() < 1e-9);
        let tiers: BTreeSet<_> = VALUES
            .iter()
            .filter(|(stat, _)| *stat != Sub::Spd)
            .filter_map(|(stat, rolls)| {
                piece
                    .substats
                    .get(stat)
                    .map(|v| rolls.iter().position(|x| (x - v).abs() < 1e-12).unwrap())
            })
            .collect();
        mixed |= tiers.len() > 1;
    }
    assert!(mixed);
}

#[test]
fn welt_crit_upgrades_vary_independently_of_speed_and_hit_rate_upgrades() {
    let spec = RelicSpec {
        required_substats: vec![Sub::CritRate, Sub::CritDmg, Sub::Spd, Sub::EffectHitRate],
        initial_substat_count: Some(4),
        min_effective_rolls: Some(5.5),
        max_effective_rolls: Some(5.5),
        ..RelicSpec::new(Slot::Head)
    };
    let weights = BTreeMap::from([
        (Sub::CritRate, 1.0),
        (Sub::CritDmg, 1.0),
        (Sub::Spd, 0.3),
        (Sub::EffectHitRate, 0.3),
    ]);
    // 四条各出现一次。余下五次中双暴分两次，速度和命中分三次。
    let expected: BTreeSet<_> = (0..=2)
        .flat_map(|crit| (0..=3).map(move |spd| (crit, spd)))
        .collect();
    let mut observed = BTreeSet::new();
    for seed in 0..256 {
        let (piece, _) = generate_relic(
            &spec,
            &weights,
            MID,
            1,
            &mut ChaCha8Rng::seed_from_u64(seed),
        )
        .unwrap();
        assert!((score_relic(&piece, &weights) - 5.5).abs() < 1e-9);
        let crit = (piece.substats[&Sub::CritRate] / 0.029160001).round() as u8 - 1;
        let speed = (piece.substats[&Sub::Spd] / 2.3).round() as u8 - 1;
        observed.insert((crit, speed));
    }
    assert_eq!(observed, expected);
}
