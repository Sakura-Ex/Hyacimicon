use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Battle,
    Reward,
    Encounter,
    Boss,
}

impl NodeType {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "battle" => Ok(Self::Battle),
            "reward" => Ok(Self::Reward),
            "encounter" => Ok(Self::Encounter),
            "boss" => Ok(Self::Boss),
            other => Err(format!("未知节点类型：{other}")),
        }
    }

    pub fn loss_index(self) -> usize {
        match self {
            Self::Battle => 0,
            Self::Reward => 1,
            Self::Encounter => 2,
            Self::Boss => 3,
        }
    }

    pub fn strength_multiplier(self) -> f64 {
        match self {
            Self::Battle => 1.0,
            Self::Reward => 0.8,
            Self::Encounter => 1.2,
            Self::Boss => 1.5,
        }
    }
}

impl fmt::Display for NodeType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Battle => "battle",
            Self::Reward => "reward",
            Self::Encounter => "encounter",
            Self::Boss => "boss",
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    Front,
    Back,
    Flex,
}

impl Position {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "前台" | "front" => Ok(Self::Front),
            "后台" | "back" => Ok(Self::Back),
            "前后台" | "flex" => Ok(Self::Flex),
            other => Err(format!("未知角色站位：{other}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Rarity {
    Silver,
    Gold,
    Prismatic,
}

impl Rarity {
    pub fn enemy_rating_delta(self) -> i32 {
        match self {
            Self::Silver => 0,
            Self::Gold => 3,
            Self::Prismatic => 7,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct UnitDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cost: u8,
    pub costs: Vec<u8>,
    pub position: Position,
    pub roles: Vec<String>,
    pub traits: Vec<String>,
    pub trait_ids: Vec<String>,
    pub portrait_path: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TraitTier {
    pub level: i32,
    pub text: String,
    pub quality: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TraitDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub group: String,
    pub icon_path: Option<String>,
    pub tiers: Vec<TraitTier>,
    pub members: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AffixDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub effect: AffixEffect,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AffixEffect {
    pub initial_health_delta: i32,
    pub enemy_rating_multiplier: f64,
    pub plane_enemy_rating_multipliers: Vec<f64>,
    pub team_power_multiplier: f64,
    pub cost_power_multipliers: Vec<f64>,
    pub star_power_multipliers: Vec<f64>,
    pub xp_extra_cost_after_level: i32,
    pub xp_extra_cost: i32,
    pub reward_multiplier: f64,
}

impl AffixEffect {
    pub fn normalized(mut self) -> Self {
        if self.enemy_rating_multiplier == 0.0 {
            self.enemy_rating_multiplier = 1.0;
        }
        if self.plane_enemy_rating_multipliers.is_empty() {
            self.plane_enemy_rating_multipliers = vec![1.0; 3];
        }
        if self.team_power_multiplier == 0.0 {
            self.team_power_multiplier = 1.0;
        }
        if self.cost_power_multipliers.is_empty() {
            self.cost_power_multipliers = vec![1.0; 5];
        }
        if self.star_power_multipliers.is_empty() {
            self.star_power_multipliers = vec![1.0; 3];
        }
        if self.reward_multiplier == 0.0 {
            self.reward_multiplier = 1.0;
        }
        self
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct InvestmentEffect {
    pub gold: i32,
    pub xp: i32,
    pub free_rerolls: i32,
    pub node_free_rerolls: i32,
    pub plane_free_rerolls: i32,
    pub plane_xp: i32,
    pub interest_cap: Option<i32>,
    pub idle_node_free_rerolls: i32,
    pub idle_node_gold_from_interest: bool,
    pub node_gold: i32,
    pub node_gold_steps: i32,
    pub boss_gold: i32,
    pub last_supply_gold: i32,
    pub set_streak: Option<i32>,
    pub level_goal: Option<i32>,
    pub level_goal_gold: i32,
    pub lock_purchase_nodes: i32,
    pub deferred_gold: i32,
    pub free_reroll_chance: f64,
    pub xp_cost_delta: i32,
    pub team_power_multiplier: f64,
    pub trait_power_multiplier: f64,
    pub enemy_rating_delta: i32,
    pub swap_health_gold: bool,
}

impl InvestmentEffect {
    pub fn normalized(mut self) -> Self {
        if self.team_power_multiplier == 0.0 {
            self.team_power_multiplier = 1.0;
        }
        if self.trait_power_multiplier == 0.0 {
            self.trait_power_multiplier = 1.0;
        }
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InvestmentDefinition {
    pub id: String,
    pub name: String,
    pub rarity: Rarity,
    pub planes: Vec<u8>,
    pub description: String,
    #[serde(default)]
    pub effect: InvestmentEffect,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct EnvironmentEffect {
    pub initial_gold: i32,
    pub starting_unit_cost: Option<u8>,
    pub starting_unit_count: i32,
    pub plane_gold: Vec<i32>,
    pub strategy_rarity_mode: Option<Rarity>,
    pub first_strategy_rarity: Option<Rarity>,
    pub reward_multiplier: f64,
    pub remove_random_affix: bool,
    pub team_power_per_node: f64,
}

impl EnvironmentEffect {
    pub fn normalized(mut self) -> Self {
        if self.plane_gold.is_empty() {
            self.plane_gold = vec![0; 3];
        }
        if self.reward_multiplier == 0.0 {
            self.reward_multiplier = 1.0;
        }
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EnvironmentDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub effect: EnvironmentEffect,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DifficultyDefinition {
    pub id: String,
    pub name: String,
    pub enemy_difficulty: i32,
    pub affix_count: i32,
}

#[derive(Clone, Debug, Serialize)]
pub struct OperationRules {
    pub game_version: String,
    pub starting_health: i32,
    pub starting_gold: i32,
    pub starting_level: u8,
    pub starting_unit_max_cost: u8,
    pub enemy_difficulty_max: i32,
    pub affix_count_max: usize,
    pub shop_size: usize,
    pub bench_capacity: usize,
    pub front_capacity: usize,
    pub back_capacity: usize,
    pub refresh_cost: i32,
    pub xp_purchase_cost: i32,
    pub xp_purchase_amount: i32,
    pub interest_step_gold: i32,
    pub interest_cap: i32,
    pub base_battle_gold: i32,
    pub shop_odds: Vec<[f64; 5]>,
    pub xp_to_next_level: Vec<i32>,
    pub card_copies_by_cost: [i32; 5],
    pub investment_nodes: Vec<usize>,
    pub loss_damage: [i32; 4],
}

impl Default for OperationRules {
    fn default() -> Self {
        Self {
            game_version: "4.6.54".into(),
            starting_health: 100,
            starting_gold: 3,
            starting_level: 3,
            starting_unit_max_cost: 3,
            enemy_difficulty_max: 211,
            affix_count_max: 53,
            shop_size: 5,
            bench_capacity: 9,
            front_capacity: 4,
            back_capacity: 6,
            refresh_cost: 2,
            xp_purchase_cost: 1,
            xp_purchase_amount: 4,
            interest_step_gold: 10,
            interest_cap: 5,
            base_battle_gold: 5,
            shop_odds: vec![
                [1.0, 0.0, 0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0, 0.0],
                [0.65, 0.25, 0.10, 0.0, 0.0],
                [0.45, 0.33, 0.20, 0.02, 0.0],
                [0.30, 0.40, 0.25, 0.05, 0.0],
                [0.19, 0.30, 0.40, 0.10, 0.01],
                [0.18, 0.25, 0.32, 0.22, 0.03],
                [0.15, 0.20, 0.25, 0.30, 0.10],
                [0.05, 0.10, 0.20, 0.40, 0.25],
            ],
            xp_to_next_level: vec![0, 0, 4, 6, 20, 40, 52, 72, 84],
            card_copies_by_cost: [30, 25, 18, 10, 9],
            investment_nodes: vec![2, 7, 13],
            loss_damage: [8, 4, 10, 15],
        }
    }
}

#[derive(Clone, Debug)]
pub struct NodeDefinition {
    pub id: String,
    pub plane: u8,
    pub node_type: NodeType,
}

#[derive(Clone, Debug)]
pub struct RulePack {
    pub game_version: String,
    pub snapshot_id: String,
    pub content_sha256: String,
    pub validation_status: String,
    pub assumptions: Vec<String>,
    pub rules: OperationRules,
    pub units: HashMap<String, UnitDefinition>,
    pub traits: HashMap<String, TraitDefinition>,
    pub affixes: Vec<AffixDefinition>,
    pub difficulties: Vec<DifficultyDefinition>,
    pub environments: Vec<EnvironmentDefinition>,
    pub investments: Vec<InvestmentDefinition>,
    pub nodes: Vec<NodeDefinition>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash, Serialize)]
pub struct UnitInstance {
    pub unit_id: String,
    pub star: i32,
}
