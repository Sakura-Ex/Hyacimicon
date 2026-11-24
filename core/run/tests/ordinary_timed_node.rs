//! Settlement against completed and incomplete results from the existing engine.

use battle_core::{
    diagnostics::ScenarioFile,
    engine::simulate,
    model::{Effect, Outcome, Scale, Scenario, Target, Termination},
};
use run_core::{
    HpChange, OrdinaryTimedNodeHpRule, SquadHpLosses, TimedNodeHpInput, TimedNodeHpSettlement,
    settle_ordinary_timed_node_hp,
};

/// Use the existing duel configuration with one strike and five slow targets.
fn scenario(target: Target) -> Scenario {
    let fixture = include_bytes!("../../battle/examples/duel.json");
    let mut scenario = ScenarioFile::from_slice(fixture).unwrap().battle;
    scenario.max_av = 150.0;
    scenario.mechanics.cycles = vec![50.0, 100.0];
    let ally = &mut scenario.allies[0];
    ally.stats.speed = 100.0;
    ally.actions[0].effects = vec![Effect::Damage {
        scale: Scale::Attack,
        ratio: 0.0,
        flat: 1000.0,
        hits: 1,
        can_crit: false,
        target,
    }];
    let mut enemy = scenario.waves[0][0].clone();
    enemy.stats.speed = 1.0;
    scenario.waves[0] = (1..=5)
        .map(|index| {
            let mut enemy = enemy.clone();
            enemy.id = format!("enemy_{index}");
            enemy
        })
        .collect();
    scenario
}

/// Use the reference rule with explicitly supplied progress and recovery.
fn input(progress_percent: Option<u8>, post_battle_healing: Option<u32>) -> TimedNodeHpInput {
    TimedNodeHpInput {
        hp_before: 92,
        hp_cap: 100,
        rule: OrdinaryTimedNodeHpRule {
            base_loss: 5,
            progress_loss_coefficient: 10,
        },
        progress_percent,
        post_battle_healing,
    }
}

/// Construct a hand-calculated HP component for direct comparison.
fn change(requested: u32, applied: u32) -> HpChange {
    HpChange { requested, applied }
}

#[test]
fn timeout_preserves_av_and_applies_separate_squad_hp_changes() {
    let battle = simulate(&scenario(Target::EnemyLowestHp), 42).unwrap();
    assert_eq!(battle.outcome, Outcome::Defeat);
    assert_eq!(battle.termination, Termination::AvBudgetExhausted);
    assert_eq!(battle.elapsed_av, 150.0);
    assert_eq!(battle.remaining_av, 0.0);
    assert_eq!(
        battle.units.iter().filter(|u| !u.ally && !u.alive).count(),
        1
    );
    assert_eq!(
        settle_ordinary_timed_node_hp(&battle, input(Some(20), Some(2))),
        Ok(TimedNodeHpSettlement::Settled {
            losses: SquadHpLosses {
                base: change(5, 5),
                progress: change(8, 8),
                hp_after_losses: 79,
            },
            recovery: Some(change(2, 2)),
            hp_after: 81,
        })
    );
}

#[test]
fn victory_preserves_av_and_full_progress_uses_the_hp_cap() {
    let battle = simulate(&scenario(Target::AllEnemies), 42).unwrap();
    assert_eq!(battle.outcome, Outcome::Victory);
    assert_eq!(battle.termination, Termination::EnemiesDefeated);
    assert_eq!(battle.elapsed_av, 100.0);
    assert_eq!(battle.remaining_av, 50.0);
    assert_eq!(
        settle_ordinary_timed_node_hp(&battle, input(Some(100), Some(2))),
        Ok(TimedNodeHpSettlement::Settled {
            losses: SquadHpLosses {
                base: change(0, 0),
                progress: change(0, 0),
                hp_after_losses: 92,
            },
            recovery: Some(change(2, 2)),
            hp_after: 94,
        })
    );
    let capped = TimedNodeHpInput {
        hp_before: 99,
        ..input(Some(100), Some(2))
    };
    assert_eq!(
        settle_ordinary_timed_node_hp(&battle, capped),
        Ok(TimedNodeHpSettlement::Settled {
            losses: SquadHpLosses {
                base: change(0, 0),
                progress: change(0, 0),
                hp_after_losses: 99,
            },
            recovery: Some(change(2, 1)),
            hp_after: 100,
        })
    );
}

#[test]
fn progress_controls_the_loss_and_the_last_fraction_truncates() {
    let victory = simulate(&scenario(Target::AllEnemies), 42).unwrap();
    let defeat = simulate(&scenario(Target::EnemyLowestHp), 42).unwrap();
    for battle in [&victory, &defeat] {
        for (p, base, progress) in [
            (10, 5, 9),
            (11, 5, 8),
            (20, 5, 8),
            (21, 5, 7),
            (99, 5, 0),
            (100, 0, 0),
        ] {
            let after = 92 - base - progress;
            assert_eq!(
                settle_ordinary_timed_node_hp(battle, input(Some(p), Some(0))),
                Ok(TimedNodeHpSettlement::Settled {
                    losses: SquadHpLosses {
                        base: change(base, base),
                        progress: change(progress, progress),
                        hp_after_losses: after,
                    },
                    recovery: Some(change(0, 0)),
                    hp_after: after,
                })
            );
        }
    }
}

#[test]
fn missing_progress_or_recovery_and_incomplete_battles_are_explicit() {
    let battle = simulate(&scenario(Target::AllEnemies), 42).unwrap();
    assert_eq!(
        settle_ordinary_timed_node_hp(&battle, input(None, Some(2))),
        Ok(TimedNodeHpSettlement::ProgressUnknown)
    );
    assert_eq!(
        settle_ordinary_timed_node_hp(&battle, input(Some(20), None)),
        Ok(TimedNodeHpSettlement::RecoveryUnknown {
            losses: SquadHpLosses {
                base: change(5, 5),
                progress: change(8, 8),
                hp_after_losses: 79,
            },
        })
    );
    let mut limited = scenario(Target::EnemyLowestHp);
    limited.max_actions = 1;
    let incomplete = simulate(&limited, 42).unwrap();
    assert_eq!(incomplete.termination, Termination::ActionLimit);
    assert_eq!(
        settle_ordinary_timed_node_hp(&incomplete, input(Some(20), Some(2))),
        Ok(TimedNodeHpSettlement::Incomplete)
    );
}

#[test]
fn reaching_zero_stops_recovery_even_if_its_amount_is_unknown() {
    let battle = simulate(&scenario(Target::EnemyLowestHp), 42).unwrap();
    for healing in [Some(2), None] {
        let input = TimedNodeHpInput {
            hp_before: 8,
            ..input(Some(20), healing)
        };
        assert_eq!(
            settle_ordinary_timed_node_hp(&battle, input),
            Ok(TimedNodeHpSettlement::Settled {
                losses: SquadHpLosses {
                    base: change(5, 5),
                    progress: change(8, 3),
                    hp_after_losses: 0,
                },
                recovery: None,
                hp_after: 0,
            })
        );
    }
}

#[test]
fn rejects_invalid_effective_progress_and_squad_hp() {
    let battle = simulate(&scenario(Target::AllEnemies), 42).unwrap();
    assert_eq!(
        settle_ordinary_timed_node_hp(&battle, input(Some(101), Some(2))),
        Err("effective progress must be between 0 and 100 percent")
    );
    let over_cap = TimedNodeHpInput {
        hp_before: 101,
        ..input(Some(100), Some(2))
    };
    assert_eq!(
        settle_ordinary_timed_node_hp(&battle, over_cap),
        Err("squad HP must fit within a positive HP cap")
    );
}
