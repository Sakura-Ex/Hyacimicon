use account_core::{
    RawAccount, RelicInstance, RelicMainStat, RelicSlot, RelicSubStat,
    import::reliquary::import_reliquary_archive,
    playable::{
        CharacterPanel, PlayableAccount, PlayableRelicGroup, PlayableRelics,
        attributes::Attribute,
        conversion::{ConversionRequest, convert_playable_account},
    },
    reference::PlayableData,
};
use battle_core::{
    diagnostics::{ScenarioFile, simulate},
    engine::{BattleSession, BattleStep},
    model::{BattleResult, Outcome, Scenario, Termination},
};
use run_core::{AllyBinding, OpeningPlanarCritDamage, assemble_battle};

const SEED: u64 = 42;
const RULE: OpeningPlanarCritDamage<'_> = OpeningPlanarCritDamage {
    set_id: "310",
    minimum_effect_resistance: 0.30,
    crit_damage_bonus: 0.10,
};
const ROSTER: [AllyBinding<'_>; 2] = [
    AllyBinding {
        unit_id: "wearer",
        character_id: "1306",
    },
    AllyBinding {
        unit_id: "teammate",
        character_id: "1004",
    },
];

/// Import the repository's existing account fixture.
fn imported_account() -> RawAccount {
    let export = include_str!("../../account/tests/fixtures/reliquary_v4.json");
    import_reliquary_archive(export, "equipment-fixture")
        .unwrap()
        .account
}

/// Resolve the effective configuration across the raw and playable persistence boundaries.
fn account(raw: &RawAccount, data: &PlayableData) -> PlayableAccount {
    let raw = serde_json::from_slice(&serde_json::to_vec(raw).unwrap()).unwrap();
    let playable = convert_playable_account(
        &raw,
        data,
        &ConversionRequest {
            config_id: "opening-planar".into(),
            character_aliases: Default::default(),
        },
    )
    .unwrap();
    serde_json::from_slice(&serde_json::to_vec(&playable).unwrap()).unwrap()
}

/// Keep the existing synthetic basic action and make a two-wave damage oracle.
fn encounter() -> Scenario {
    let mut battle = ScenarioFile::from_slice(include_bytes!("../../battle/examples/duel.json"))
        .unwrap()
        .battle;
    battle.max_av = 1_200.0;
    battle.allies[0].id = "wearer".into();
    let mut teammate = battle.allies[0].clone();
    teammate.id = "teammate".into();
    battle.allies.push(teammate);
    let enemy = &mut battle.waves[0][0];
    enemy.id = "first-enemy".into();
    enemy.stats.attack = 0.0;
    enemy.stats.max_hp = 12_000.0;
    enemy.initial_hp = enemy.stats.max_hp;
    let mut second = enemy.clone();
    second.id = "second-enemy".into();
    battle.waves.push(vec![second]);
    battle
}

fn panel(account: &PlayableAccount) -> &CharacterPanel {
    &account
        .characters
        .iter()
        .find(|c| c.character_id == ROSTER[0].character_id)
        .unwrap()
        .panel
}

fn approximately(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-10,
        "expected {expected}, received {actual}"
    );
}

/// Drive the public step/submit interface with the same first-choice policy as diagnostics.
fn drive(mut session: BattleSession<'_>) -> BattleResult {
    loop {
        let (id, choice) = match session.advance().unwrap() {
            BattleStep::Decision(decision) => (decision.id, decision.legal.first().cloned()),
            BattleStep::Finished(result) => return result.clone(),
        };
        session.submit(id, choice).unwrap();
    }
}

#[test]
fn effective_panel_to_session_matches_diagnostics() {
    let mut data = PlayableData::bundled().unwrap();
    let trial = data
        .trials
        .iter_mut()
        .find(|t| t.character_id == ROSTER[0].character_id)
        .unwrap();
    assert_eq!(trial.relics.planar_set_id, RULE.set_id);
    // Guarantee a critical hit for a damage check while keeping resistance and
    // other static attributes from the bundled snapshot.
    trial.relics.stats.insert(Attribute::CritRate, 0.95);
    let account = account(&imported_account(), &data);
    let wearer = panel(&account);
    let prepared = assemble_battle(encounter(), &account, &ROSTER, &RULE).unwrap();
    let stats = &prepared.allies[0].stats;
    assert_eq!(stats.attack, wearer.attack.total());
    assert_eq!(stats.speed, wearer.speed.total());
    assert_eq!(stats.max_hp, wearer.hp.total());
    assert_eq!(stats.crit_damage, wearer.crit_damage);
    assert_eq!(stats.crit_rate, 1.0);
    approximately(wearer.effect_resistance, 0.416);

    let file = ScenarioFile::from_slice(&serde_json::to_vec(&prepared).unwrap()).unwrap();
    let in_memory = drive(BattleSession::new(&prepared, SEED).unwrap());
    let diagnostic = simulate(&file.battle, SEED, false).unwrap();
    assert_eq!(
        serde_json::to_value(&in_memory).unwrap(),
        serde_json::to_value(diagnostic).unwrap()
    );
    assert_eq!(in_memory.outcome, Outcome::Victory);
    assert_eq!(in_memory.termination, Termination::EnemiesDefeated);
    assert_eq!(in_memory.waves_cleared, 2);
    assert!(in_memory.remaining_av > 0.0);

    let mut traced = simulate(&file.battle, SEED, true).unwrap();
    let applications: Vec<_> = traced
        .events
        .iter()
        .filter(|e| e.kind == "modifier_applied")
        .map(|e| (e.av, e.actor.as_str(), e.target.as_deref()))
        .collect();
    assert_eq!(
        applications,
        [
            (0.0, "wearer", Some("wearer")),
            (0.0, "wearer", Some("teammate"))
        ]
    );
    let first_hit = traced
        .events
        .iter()
        .find(|e| e.kind == "damage" && e.actor == "wearer")
        .unwrap();
    assert_eq!(first_hit.critical, Some(true));
    approximately(
        first_hit.amount.unwrap(),
        wearer.attack.total()
            * (1.0 + wearer.all_damage_boost)
            * (1.0 + wearer.crit_damage + RULE.crit_damage_bonus),
    );
    traced.events.clear();
    assert_eq!(
        serde_json::to_value(&in_memory).unwrap(),
        serde_json::to_value(traced).unwrap()
    );
    assert_eq!(
        assemble_battle(prepared, &account, &ROSTER, &RULE).unwrap_err(),
        "reserved modifier already present: run:opening-planar-crit-damage"
    );
    println!(
        "seed {SEED}, two waves, {} actions, {:.6} remaining AV",
        in_memory.actions, in_memory.remaining_av
    );
}

#[test]
fn static_resistance_is_counted_once_at_the_inclusive_threshold() {
    let mut data = PlayableData::bundled().unwrap();
    let character_id = ROSTER[0].character_id;
    data.minor_traces
        .iter_mut()
        .find(|entry| entry.character_id == character_id)
        .unwrap()
        .nodes
        .retain(|_, bonus| bonus.attribute != Attribute::EffectResistance);
    // This adjacent value sums with 0.10 to exactly 0.30 in f64.
    let threshold_base = f64::from_bits(0.20_f64.to_bits() - 1);
    for (base, expected, triggers) in [(0.15, 0.25, 0), (threshold_base, 0.30, 1)] {
        data.trials
            .iter_mut()
            .find(|trial| trial.character_id == character_id)
            .unwrap()
            .relics
            .stats
            .insert(Attribute::EffectResistance, base);
        let account = account(&imported_account(), &data);
        assert_eq!(panel(&account).effect_resistance, expected);
        let prepared = assemble_battle(encounter(), &account, &ROSTER, &RULE).unwrap();
        assert_eq!(prepared.mechanics.triggers.len(), triggers);
        let traced = simulate(&prepared, SEED, true).unwrap();
        assert_eq!(
            traced
                .events
                .iter()
                .filter(|e| e.kind == "modifier_applied")
                .count(),
            2 * triggers
        );
    }
}

#[test]
fn uses_the_resolved_planar_group_after_mapping_or_keeping_owned_pieces() {
    let mut data = PlayableData::bundled().unwrap();
    let character_id = ROSTER[1].character_id;
    let mapped = &mut data
        .mappings
        .iter_mut()
        .find(|entry| entry.character_id == character_id)
        .unwrap()
        .planar;
    mapped.set_id = RULE.set_id.into();
    mapped.stats.insert(Attribute::EffectResistance, 0.25);
    let mut raw = imported_account();
    let mut encounter = encounter();
    encounter.allies.remove(0);
    let roster = &ROSTER[1..];
    let configurations = |account: PlayableAccount| {
        let character = account
            .characters
            .iter()
            .find(|c| c.character_id == character_id)
            .unwrap();
        let PlayableRelics::Partitioned { planar, .. } = &character.relics else {
            panic!("owned fixture character should have a partitioned configuration");
        };
        let prepared = assemble_battle(encounter.clone(), &account, roster, &RULE).unwrap();
        (planar.clone(), prepared.mechanics.triggers.len())
    };

    // With no owned planar pieces equipped, account conversion uses the mapped pair.
    let (planar, triggers) = configurations(account(&raw, &data));
    assert!(matches!(planar, PlayableRelicGroup::Mapped { .. }));
    assert_eq!(triggers, 1);

    let character = raw
        .characters
        .iter_mut()
        .find(|c| c.character_id == character_id)
        .unwrap();
    for (id, slot) in [
        ("fixture-sphere", RelicSlot::PlanarSphere),
        ("fixture-rope", RelicSlot::LinkRope),
    ] {
        character.relic_instance_ids.insert(slot, id.into());
        raw.relics.push(RelicInstance {
            instance_id: id.into(),
            set_id: RULE.set_id.into(),
            slot,
            rarity: 5,
            level: 15,
            mainstat: RelicMainStat::AtkPercent,
            substats: [(RelicSubStat::EffectRes, 0.125)].into(),
        });
    }
    let (planar, triggers) = configurations(account(&raw, &data));
    assert!(matches!(planar, PlayableRelicGroup::Owned { .. }));
    assert_eq!(triggers, 1);

    // Both pieces are retained, but a split set cannot trigger the two-piece effect.
    raw.relics.last_mut().unwrap().set_id = "301".into();
    let (planar, triggers) = configurations(account(&raw, &data));
    assert!(matches!(planar, PlayableRelicGroup::Owned { .. }));
    assert_eq!(triggers, 0);
}
