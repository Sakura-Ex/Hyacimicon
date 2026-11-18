//! Resolves one selected equipped set to a battle-start modifier.

use account_core::{
    RelicInstance, RelicSlot,
    playable::{PlayableCharacter, PlayableRelicGroup, PlayableRelics, RELIC_MAPPING_LEVEL},
};
use battle_core::{
    expression::Expression,
    model::{BattleEvent, Effect, Mechanics, Target, Trigger},
    modifiers::{ModifierDefinition, Property, PropertyChange, StackMode},
};

const MODIFIER_ID: &str = "run:opening-planar-crit-damage";

/// An explicit two-piece planar rule with one eligible wearer per encounter.
pub struct OpeningPlanarCritDamage<'a> {
    /// Effective planar set ID from the account snapshot.
    pub set_id: &'a str,
    /// Inclusive threshold on the effective account panel, expressed as a fraction.
    pub minimum_effect_resistance: f64,
    /// Additive critical damage applied at battle start, expressed as a fraction.
    pub crit_damage_bonus: f64,
}

impl OpeningPlanarCritDamage<'_> {
    pub(super) fn validate(&self) -> Result<(), String> {
        if !(!self.set_id.is_empty()
            && (0.0..=1.0).contains(&self.minimum_effect_resistance)
            && self.crit_damage_bonus.is_finite()
            && self.crit_damage_bonus > 0.0)
        {
            return Err("invalid opening planar critical damage rule".into());
        }
        Ok(())
    }

    pub(super) fn install(
        &self,
        mechanics: &mut Mechanics,
        wearer: Option<&str>,
    ) -> Result<(), String> {
        if mechanics.modifier_definitions.contains_key(MODIFIER_ID) {
            return Err(format!("reserved modifier already present: {MODIFIER_ID}"));
        }
        if let Some(wearer) = wearer {
            mechanics.modifier_definitions.insert(
                MODIFIER_ID.into(),
                ModifierDefinition {
                    stack_mode: StackMode::ReplaceBySource,
                    max_stacks: 1,
                    lifetime: None,
                    properties: vec![PropertyChange {
                        property: Property::CritDamage,
                        value: Expression::Constant {
                            value: self.crit_damage_bonus,
                        },
                    }],
                },
            );
            mechanics.triggers.push(Trigger {
                owner: wearer.into(),
                event: BattleEvent::BattleStart,
                subject: None,
                action: None,
                uses: 1,
                effects: vec![Effect::AddModifier {
                    id: MODIFIER_ID.into(),
                    stacks: Expression::Constant { value: 1.0 },
                    target: Target::AllAllies,
                }],
                expires_on: None,
                duration: 0,
            });
        }
        Ok(())
    }
}

/// Reads the effective two pieces after account-side trial selection and mapping.
pub(super) fn effective_planar_set<'a>(
    character: &'a PlayableCharacter,
    inventory: &'a [RelicInstance],
) -> Result<Option<&'a str>, String> {
    match &character.relics {
        PlayableRelics::TrialAggregate { configuration } => Ok(Some(&configuration.planar_set_id)),
        PlayableRelics::Partitioned { planar, .. } => match planar {
            PlayableRelicGroup::Mapped { configuration } => Ok(Some(&configuration.set_id)),
            PlayableRelicGroup::Owned { instance_ids } => {
                let [left, right] = instance_ids.as_slice() else {
                    return Err("owned planar group needs exactly two instances".into());
                };
                let left = planar_piece(left, inventory)?;
                let right = planar_piece(right, inventory)?;
                if !matches!(
                    (left.slot, right.slot),
                    (RelicSlot::PlanarSphere, RelicSlot::LinkRope)
                        | (RelicSlot::LinkRope, RelicSlot::PlanarSphere)
                ) {
                    return Err("owned planar group needs a sphere and a rope".into());
                }
                Ok((left.set_id == right.set_id).then_some(left.set_id.as_str()))
            }
        },
    }
}

/// Looks up the already selected owned instance without falling back to inventory defaults.
fn planar_piece<'a>(id: &str, inventory: &'a [RelicInstance]) -> Result<&'a RelicInstance, String> {
    let mut matches = inventory.iter().filter(|piece| piece.instance_id == id);
    let piece = matches
        .next()
        .ok_or_else(|| format!("missing equipped planar instance {id}"))?;
    if matches.next().is_some() || piece.level != RELIC_MAPPING_LEVEL {
        return Err(format!(
            "invalid or ambiguous equipped planar instance {id}"
        ));
    }
    Ok(piece)
}
