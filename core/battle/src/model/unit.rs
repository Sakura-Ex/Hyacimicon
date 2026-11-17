//! 单位的静态面板、初始资源与复活配置。
use super::*;
use serde::{Deserialize, Serialize};

/// Effective values, already composed by the caller. No account data is guessed.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Stats {
    pub level: u16,
    pub max_hp: f64,
    pub attack: f64,
    pub defense: f64,
    pub speed: f64,
    pub taunt: f64,
    pub crit_rate: f64,
    pub crit_damage: f64,
    pub damage_bonus: f64,
    pub defense_reduction: f64,
    pub defense_ignore: f64,
    pub resistance: f64,
    pub resistance_penetration: f64,
    pub vulnerability: f64,
    pub damage_reduction: f64,
    pub independent_multiplier: f64,
    pub toughness_broken: bool,
    pub healing_bonus: f64,
    pub incoming_healing_bonus: f64,
    pub shield_bonus: f64,
}

/// Optional explicit model of immediate revival. It is not inferred from game data.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Revival {
    pub charges: u8,
    pub hp_fraction: f64,
    pub av_penalty: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Unit {
    pub id: String,
    pub stats: Stats,
    pub initial_hp: f64,
    pub initial_energy: f64,
    pub max_energy: f64,
    pub revival: Option<Revival>,
    /// First affordable action wins, with ultimates considered at action boundaries.
    pub actions: Vec<Action>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SkillPoints {
    pub initial: u8,
    pub maximum: u8,
}
