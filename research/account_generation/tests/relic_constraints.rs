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
