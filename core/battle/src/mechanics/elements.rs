//! Explicit seven-element panels for attacks that select their damage element.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Element {
    Physical,
    Fire,
    Ice,
    Thunder,
    Wind,
    Quantum,
    Imaginary,
}

impl Element {
    pub const ALL: [Self; 7] = [
        Self::Physical,
        Self::Fire,
        Self::Ice,
        Self::Thunder,
        Self::Wind,
        Self::Quantum,
        Self::Imaginary,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Physical => "physical",
            Self::Fire => "fire",
            Self::Ice => "ice",
            Self::Thunder => "thunder",
            Self::Wind => "wind",
            Self::Quantum => "quantum",
            Self::Imaginary => "imaginary",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(tag = "selection", rename_all = "snake_case", deny_unknown_fields)]
pub enum DamageElement {
    Fixed { element: Element },
    RandomAll,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ElementProfile {
    /// Additional element-specific bonus; excludes Stats.damage_bonus and modifiers.
    pub damage_bonus: BTreeMap<Element, f64>,
    /// Complete resistance by element, replacing the legacy resolved scalar.
    pub resistance: BTreeMap<Element, f64>,
}

impl ElementProfile {
    pub fn validate(&self) -> Result<(), String> {
        for element in Element::ALL {
            let bonus = self
                .damage_bonus
                .get(&element)
                .ok_or("element damage bonus missing")?;
            let resistance = self
                .resistance
                .get(&element)
                .ok_or("element resistance missing")?;
            if !bonus.is_finite()
                || !resistance.is_finite()
                || *bonus < -1.0
                || *bonus > 1e6
                || *resistance < -1e6
                || *resistance > 1e6
            {
                return Err("invalid elemental panel".into());
            }
        }
        Ok(())
    }
}
