//! Prepares a single battle from an effective account and an explicit encounter.

mod equipment;

use std::collections::{BTreeMap, BTreeSet};

use account_core::{
    playable::{CharacterPanel, PLAYABLE_ACCOUNT_SCHEMA_VERSION, PLAYABLE_LEVEL, PlayableAccount},
    reference::Element as AccountElement,
};
use battle_core::{elements::Element, model::Scenario};

pub use equipment::OpeningPlanarCritDamage;

/// Binds an encounter's allied unit instance to its effective account character.
pub struct AllyBinding<'a> {
    /// Stable unit ID in the encounter.
    pub unit_id: &'a str,
    /// Character ID in the effective account configuration.
    pub character_id: &'a str,
}

/// Copies account-owned panel values and attaches one supported opening equipment rule.
///
/// The encounter supplies actions, starting resources, at least one enemy wave,
/// and fight rules.
/// If an ally has an element profile, its resistance stays encounter-owned and its
/// elemental damage bonus comes from the account. The opening rule uses only the
/// eligible wearer's resolved equipment and effective effect resistance.
pub fn assemble_battle(
    mut encounter: Scenario,
    account: &PlayableAccount,
    roster: &[AllyBinding<'_>],
    opening: &OpeningPlanarCritDamage<'_>,
) -> Result<Scenario, String> {
    if account.schema_version != PLAYABLE_ACCOUNT_SCHEMA_VERSION {
        return Err("unsupported playable account schema version".into());
    }
    if encounter.waves.is_empty() {
        return Err("encounter must include at least one enemy wave".into());
    }
    opening.validate()?;
    if roster.is_empty() || roster.len() != encounter.allies.len() {
        return Err("roster must bind every encounter ally".into());
    }
    let mut bindings = BTreeMap::new();
    let mut characters = BTreeSet::new();
    for entry in roster {
        if bindings.insert(entry.unit_id, entry.character_id).is_some()
            || !characters.insert(entry.character_id)
        {
            return Err("roster contains a duplicate unit or account character".into());
        }
    }
    let mut seen_units = BTreeSet::new();
    let mut wearer = None;
    for unit in &mut encounter.allies {
        let id = unit.id.as_str();
        if !seen_units.insert(id) || encounter.waves.iter().flatten().any(|enemy| enemy.id == id) {
            return Err(format!("ambiguous allied unit ID {id}"));
        }
        let character_id = bindings
            .get(id)
            .ok_or_else(|| format!("missing account binding for ally {id}"))?;
        let mut matches = account
            .characters
            .iter()
            .filter(|character| character.character_id == *character_id);
        let character = matches
            .next()
            .ok_or_else(|| format!("missing account character {character_id}"))?;
        if matches.next().is_some() {
            return Err(format!("ambiguous account character {character_id}"));
        }
        let panel = &character.panel;
        let stats = &mut unit.stats;
        let max_hp = panel.hp.total();
        let speed = panel.speed.total();
        if !(max_hp.is_finite()
            && max_hp > 0.0
            && speed.is_finite()
            && speed > 0.0
            && (0.0..=max_hp).contains(&unit.initial_hp))
        {
            return Err(format!("invalid HP or speed for ally {id}"));
        }

        stats.level = u16::from(PLAYABLE_LEVEL);
        stats.max_hp = max_hp;
        stats.attack = panel.attack.total();
        stats.defense = panel.defense.total();
        stats.speed = speed;
        stats.taunt = panel.aggro;
        stats.crit_rate = panel.crit_rate;
        stats.crit_damage = panel.crit_damage;
        stats.damage_bonus = panel.all_damage_boost;
        stats.healing_bonus = panel.outgoing_healing_boost;
        stats.incoming_healing_bonus = panel.incoming_healing_boost;
        if let Some(profile) = encounter.mechanics.element_profiles.get_mut(id) {
            profile.damage_bonus = element_bonuses(panel);
        }
        if equipment::effective_planar_set(character, &account.owned_relics)?
            == Some(opening.set_id)
        {
            if !panel.effect_resistance.is_finite() {
                return Err(format!("invalid effect resistance for ally {id}"));
            }
            if unit.initial_hp > 0.0
                && panel.effect_resistance >= opening.minimum_effect_resistance
                && wearer.replace(id).is_some()
            {
                return Err("multiple eligible opening equipment wearers".into());
            }
        }
    }
    opening.install(&mut encounter.mechanics, wearer)?;
    Ok(encounter)
}

/// Expands the sparse account bonuses into the battle profile's seven elements.
fn element_bonuses(panel: &CharacterPanel) -> BTreeMap<Element, f64> {
    use AccountElement::*;
    [
        (Element::Physical, Physical),
        (Element::Fire, Fire),
        (Element::Ice, Ice),
        (Element::Thunder, Lightning),
        (Element::Wind, Wind),
        (Element::Quantum, Quantum),
        (Element::Imaginary, Imaginary),
    ]
    .into_iter()
    .map(|(battle, account)| {
        let bonus = panel
            .elemental_damage_boost
            .get(&account)
            .copied()
            .unwrap_or(0.0);
        (battle, bonus)
    })
    .collect()
}
