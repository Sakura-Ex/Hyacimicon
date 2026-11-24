//! 普通虚数击破的数值、触发点和既有战斗结算回归。
mod common;

use battle_core::{
    elements::{Element, ElementProfile},
    engine::simulate,
    model::*,
};
use common::{action, close, scenario, state, unit};

const LEVEL_80: f64 = 3767.5533;
const REFERENCE_DAMAGE: f64 = 6593.218275;

fn imaginary(amount: f64, stance_unit: f64, target: Target) -> Effect {
    Effect::ImaginaryToughness {
        amount,
        break_damage: ImaginaryBreakDamage {
            level_multiplier: LEVEL_80,
            break_effect: 1.0,
            stance_unit,
        },
        target,
    }
}

fn battle(effects: Vec<Effect>, current_toughness: f64) -> Scenario {
    let mut hero = unit("hero", 200.0);
    hero.stats.level = 80;
    hero.actions = vec![action("attack", effects)];
    let mut enemy = unit("enemy", 100.0);
    enemy.initial_hp = 10_000.0;
    enemy.stats.max_hp = enemy.initial_hp;
    enemy.stats.defense = 1000.0;
    enemy.stats.toughness_broken = current_toughness == 0.0;
    let mut input = scenario(vec![hero], vec![vec![enemy]]);
    input.max_actions = 1;
    input
        .mechanics
        .toughness
        .insert("enemy".into(), current_toughness);
    input.mechanics.max_toughness.insert("enemy".into(), 120.0);
    input
}

#[test]
fn one_break_uses_full_toughness_after_partial_reduction() {
    let target = Target::EnemyLowestHp;
    let input = battle(
        vec![
            imaginary(4.0, 10.0, target),
            imaginary(6.0, 10.0, target),
            imaginary(1.0, 10.0, target),
        ],
        10.0,
    );
    let result = simulate(&input, 42).unwrap();
    let enemy = state(&result, "enemy");
    close(enemy.toughness, 0.0);
    close(enemy.hp, 10_000.0 - REFERENCE_DAMAGE);
}

#[test]
fn damage_requires_a_transition_and_explicit_opt_in() {
    let target = Target::EnemyLowestHp;
    for (current, effect, remaining) in [
        (11.0, imaginary(10.0, 10.0, target), 1.0),
        (0.0, imaginary(10.0, 10.0, target), 0.0),
        (
            10.0,
            Effect::Toughness {
                amount: 10.0,
                target,
            },
            0.0,
        ),
    ] {
        let mut input = battle(vec![effect], current);
        input.mechanics.max_toughness.clear();
        let result = simulate(&input, 42).unwrap();
        let enemy = state(&result, "enemy");
        close(enemy.toughness, remaining);
        close(enemy.hp, 10_000.0);
    }
}

#[test]
fn break_uses_resistance_and_vulnerability_with_an_explicit_stance_unit() {
    let mut input = battle(vec![imaginary(30.0, 30.0, Target::EnemyLowestHp)], 30.0);
    input.mechanics.max_toughness.insert("enemy".into(), 360.0);
    let source = &mut input.allies[0].stats;
    source.attack = 8000.0;
    source.crit_rate = 1.0;
    source.crit_damage = 5.0;
    source.damage_bonus = 10.0;
    source.independent_multiplier = 3.0;
    source.resistance_penetration = 0.1;
    input.mechanics.ally_damage_factor = 0.3;
    let enemy = &mut input.waves[0][0].stats;
    enemy.vulnerability = 0.25;
    enemy.damage_reduction = 0.7;
    input.mechanics.element_profiles.insert(
        "enemy".into(),
        ElementProfile {
            damage_bonus: Element::ALL
                .into_iter()
                .map(|element| (element, 0.0))
                .collect(),
            resistance: Element::ALL
                .into_iter()
                .map(|element| {
                    (
                        element,
                        if element == Element::Imaginary {
                            0.2
                        } else {
                            0.5
                        },
                    )
                })
                .collect(),
        },
    );
    let result = simulate(&input, 42).unwrap();
    close(state(&result, "enemy").hp, 10_000.0 - 7417.370559375);
}

#[test]
fn opening_break_settles_damage_and_shield_for_each_target() {
    let mut input = battle(vec![], 10.0);
    for id in ["other", "third"] {
        let mut enemy = input.waves[0][0].clone();
        enemy.id = id.into();
        input.waves[0].push(enemy);
        input.mechanics.toughness.insert(id.into(), 10.0);
        input.mechanics.max_toughness.insert(id.into(), 120.0);
    }
    input.mechanics.triggers = vec![
        Trigger {
            owner: "enemy".into(),
            event: BattleEvent::BattleStart,
            subject: None,
            action: None,
            uses: 1,
            expires_on: None,
            duration: 0,
            effects: vec![Effect::Shield {
                scale: Scale::Attack,
                flat: 2000.0,
                ratio: 0.0,
                max_total: 2000.0,
                duration_turns: 2,
                target: Target::SelfUnit,
            }],
        },
        Trigger {
            owner: "hero".into(),
            event: BattleEvent::BattleStart,
            subject: None,
            action: None,
            uses: 1,
            expires_on: None,
            duration: 0,
            effects: vec![imaginary(10.0, 10.0, Target::AllEnemies)],
        },
    ];
    let result = simulate(&input, 42).unwrap();
    close(state(&result, "enemy").hp, 12_000.0 - REFERENCE_DAMAGE);
    for id in ["other", "third"] {
        close(state(&result, id).hp, 10_000.0 - REFERENCE_DAMAGE);
    }
}

#[test]
fn break_damage_does_not_carry_over_after_a_listener_kills_and_revives_the_target() {
    let target = Target::EnemyLowestHp;
    let mut input = battle(vec![imaginary(10.0, 10.0, target)], 10.0);
    input.waves[0][0].revival = Some(Revival {
        charges: 1,
        hp_fraction: 1.0,
        av_penalty: 25.0,
    });
    input.mechanics.triggers.push(Trigger {
        owner: "hero".into(),
        event: BattleEvent::Break,
        subject: Some("enemy".into()),
        action: None,
        uses: 1,
        expires_on: None,
        duration: 0,
        effects: vec![common::damage(25_000.0, target)],
    });
    let result = simulate(&input, 42).unwrap();
    let enemy = state(&result, "enemy");
    assert_eq!(enemy.revivals_left, 0);
    close(enemy.hp, 10_000.0);
}

#[test]
fn missing_max_toughness_is_reported_for_an_explicit_break() {
    let mut input = battle(vec![imaginary(10.0, 10.0, Target::EnemyLowestHp)], 10.0);
    input.mechanics.max_toughness.clear();
    assert_eq!(
        simulate(&input, 42).unwrap_err(),
        "missing max_toughness for imaginary break target enemy"
    );
}

#[test]
fn imaginary_toughness_rejects_an_invalid_current_gauge_before_reduction() {
    let target = Target::EnemyLowestHp;
    let oversized = battle(vec![imaginary(1.0, 30.0, target)], 360.0);
    assert_eq!(
        simulate(&oversized, 42).unwrap_err(),
        "imaginary break target enemy current toughness 360 exceeds max_toughness 120"
    );
    for current in [-1.0, f64::INFINITY, f64::NAN] {
        let invalid = battle(vec![imaginary(10.0, 10.0, target)], current);
        assert_eq!(
            simulate(&invalid, 42).unwrap_err(),
            "imaginary break target enemy current toughness must be finite and non-negative"
        );
    }
}

#[test]
fn lethal_break_inside_a_hit_has_no_attack_type_and_restores_the_hits_type() {
    let target = Target::EnemyLowestHp;
    let grant_on_direct = |amount| Effect::Conditional {
        condition: Condition::HitAttackType {
            attack_type: AttackType::Direct,
        },
        then_effects: vec![Effect::Energy {
            amount,
            target: Target::SelfUnit,
        }],
        else_effects: vec![],
    };
    let mut input = battle(vec![common::damage(20.0, target)], 10.0);
    let enemy = &mut input.waves[0][0];
    enemy.initial_hp = 100.0;
    enemy.stats.max_hp = enemy.initial_hp;
    input.mechanics.triggers = vec![
        Trigger {
            owner: "hero".into(),
            event: BattleEvent::Hit,
            subject: Some("hero".into()),
            action: None,
            uses: 1,
            expires_on: None,
            duration: 0,
            effects: vec![imaginary(10.0, 10.0, target)],
        },
        Trigger {
            owner: "hero".into(),
            event: BattleEvent::Hit,
            subject: Some("hero".into()),
            action: None,
            uses: 1,
            expires_on: None,
            duration: 0,
            effects: vec![grant_on_direct(3.0)],
        },
        Trigger {
            owner: "hero".into(),
            event: BattleEvent::Death,
            subject: Some("enemy".into()),
            action: None,
            uses: 1,
            expires_on: None,
            duration: 0,
            effects: vec![grant_on_direct(7.0)],
        },
    ];
    let result = simulate(&input, 42).unwrap();
    assert!(!state(&result, "enemy").alive);
    close(state(&result, "hero").energy, 3.0);

    input.allies[0].actions[0].effects = vec![common::damage(300.0, target)];
    let result = simulate(&input, 42).unwrap();
    assert!(!state(&result, "enemy").alive);
    close(state(&result, "hero").energy, 10.0);
}
