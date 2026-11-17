//! Explicit property-modifier storage. No raw Config defaults are inferred.
use crate::{expression::Expression, model::BattleEvent};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Property {
    CritDamage,
    DamageBonus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyChange {
    pub property: Property,
    pub value: Expression,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StackMode {
    ReplaceBySource,
    AddBySource,
    ReplaceGlobal,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TickUnit {
    Owner,
    Source,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lifetime {
    pub event: BattleEvent,
    pub unit: TickUnit,
    pub ticks: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModifierDefinition {
    pub stack_mode: StackMode,
    pub max_stacks: u32,
    pub lifetime: Option<Lifetime>,
    pub properties: Vec<PropertyChange>,
}

impl ModifierDefinition {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_stacks == 0 || self.max_stacks > 10_000 {
            return Err("modifier max_stacks must be 1..10000".into());
        }
        if self.lifetime.as_ref().is_some_and(|l| l.ticks == 0) {
            return Err("modifier lifetime ticks must be positive".into());
        }
        if self.properties.len() > 32 {
            return Err("too many modifier properties".into());
        }
        for property in &self.properties {
            property.value.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ModifierState {
    pub id: String,
    pub source: String,
    pub stacks: u32,
    pub remaining_ticks: Option<u32>,
}

pub(crate) struct ModifierInstance {
    pub state: ModifierState,
    pub definition: ModifierDefinition,
    // Values are evaluated at application/stack time and replaced on refresh.
    pub values: Vec<(Property, f64)>,
}

#[derive(Default)]
pub(crate) struct ModifierBook {
    pub instances: Vec<ModifierInstance>,
}

impl ModifierBook {
    pub fn apply(
        &mut self,
        id: &str,
        source: &str,
        stacks: u32,
        definition: &ModifierDefinition,
        values: Vec<(Property, f64)>,
    ) -> u32 {
        let position = self.instances.iter().position(|m| {
            m.state.id == id
                && (definition.stack_mode == StackMode::ReplaceGlobal || m.state.source == source)
        });
        let stacks = position
            .filter(|_| definition.stack_mode == StackMode::AddBySource)
            .map_or(stacks, |i| {
                self.instances[i].state.stacks.saturating_add(stacks)
            })
            .min(definition.max_stacks);
        let instance = ModifierInstance {
            state: ModifierState {
                id: id.into(),
                source: source.into(),
                stacks,
                remaining_ticks: definition.lifetime.as_ref().map(|l| l.ticks),
            },
            definition: definition.clone(),
            values,
        };
        if let Some(i) = position {
            self.instances[i] = instance;
        } else {
            self.instances.push(instance);
        }
        stacks
    }

    pub fn remove(&mut self, id: &str, source: Option<&str>) -> Vec<ModifierState> {
        let mut removed = vec![];
        self.instances.retain(|m| {
            let matches = m.state.id == id && source.is_none_or(|s| m.state.source == s);
            if matches {
                removed.push(m.state.clone());
            }
            !matches
        });
        removed
    }

    pub fn tick(
        &mut self,
        event: BattleEvent,
        subject: Option<&str>,
        owner: &str,
    ) -> Vec<ModifierState> {
        let mut removed = vec![];
        self.instances.retain_mut(|m| {
            if let Some(lifetime) = &m.definition.lifetime {
                let unit = match lifetime.unit {
                    TickUnit::Owner => owner,
                    TickUnit::Source => &m.state.source,
                };
                if lifetime.event == event && (event.is_global() || subject == Some(unit)) {
                    let left = m
                        .state
                        .remaining_ticks
                        .as_mut()
                        .expect("validated finite lifetime");
                    *left -= 1;
                    if *left == 0 {
                        removed.push(m.state.clone());
                        return false;
                    }
                }
            }
            true
        });
        removed
    }

    pub fn delta(&self, property: Property) -> Result<f64, String> {
        let value: f64 = self
            .instances
            .iter()
            .flat_map(|m| {
                m.values.iter().filter_map(move |(p, v)| {
                    (*p == property).then_some(v * f64::from(m.state.stacks))
                })
            })
            .sum();
        if !value.is_finite() {
            return Err("nonfinite modifier property sum".into());
        }
        Ok(value)
    }

    pub fn states(&self) -> Vec<ModifierState> {
        self.instances.iter().map(|m| m.state.clone()).collect()
    }
}
