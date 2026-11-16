//! 角色和光锥基础属性、局内星级参数及运行所需的版本标识。

use serde::{Deserialize, Serialize};

/// 常规命途键，沿用已有账号资料。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GamePath {
    /// 毁灭。
    Warrior,
    /// 巡猎。
    Rogue,
    /// 智识。
    Mage,
    /// 同谐。
    Shaman,
    /// 虚无。
    Warlock,
    /// 存护。
    Knight,
    /// 丰饶。
    Priest,
    /// 记忆。
    Memory,
    /// 欢愉。
    Elation,
}

/// 角色攻击属性。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Element {
    /// 物理。
    Physical,
    /// 火。
    Fire,
    /// 冰。
    Ice,
    /// 雷。
    Lightning,
    /// 风。
    Wind,
    /// 量子。
    Quantum,
    /// 虚数。
    Imaginary,
}

/// 常规角色与玩法专属角色的区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterKind {
    /// 对齐原始账号持有列表的常规角色。
    Standard,
    /// 由玩法提供的专属角色。
    GameplayExclusive,
}

/// 用于版本核对和结果追踪的数据快照标识。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataManifest {
    /// 快照 ID，关联其中全部文件。
    pub snapshot_id: String,
    /// 游戏版本。
    pub game_version: String,
    /// 玩法赛季。
    pub season: String,
}

/// 角色或光锥自身的基础生命、攻击和防御。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseCombatStats {
    /// 基础生命。
    pub hp: f64,
    /// 基础攻击。
    pub attack: f64,
    /// 基础防御。
    pub defense: f64,
}

/// 角色自身数值，保留文档未提供的嘲讽值为空。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterBaseStats {
    /// 基础生命。
    pub hp: f64,
    /// 基础攻击。
    pub attack: f64,
    /// 基础防御。
    pub defense: f64,
    /// 基础速度。
    pub speed: f64,
    /// 基础暴击率，使用比例。
    pub crit_rate: f64,
    /// 基础暴击伤害，使用比例。
    pub crit_damage: f64,
    /// 基础嘲讽值，未记录时为空。
    pub aggro: Option<u16>,
}

/// 文档所列能量或专属资源的容量。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnergyCapacity {
    /// 容量，文档为缺省符号时为空。
    pub amount: Option<f64>,
    /// 文档括号内的专属资源名。
    pub resource_name: Option<String>,
}

/// 文档记录的一种角色形态的 80 级基础资料。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterData {
    /// 文档提供的角色 ID。
    pub id: String,
    /// 常规或玩法专属角色。
    pub kind: CharacterKind,
    /// 常规命途，玩法专属角色可能未提供。
    pub path: Option<GamePath>,
    /// 常规四／五星稀有度，未提供时为空。
    pub rarity: Option<u8>,
    /// 攻击属性。
    pub element: Element,
    /// 本条基础数据适用等级。
    pub level: u8,
    /// 本条基础数据适用突破阶段。
    pub promotion: u8,
    /// 角色自身属性。
    pub base: CharacterBaseStats,
    /// 能量或专属资源容量。
    pub energy: EnergyCapacity,
}

/// 文档记录的一种光锥的 80 级基础资料。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightConeData {
    /// 光锥种类 ID。
    pub id: String,
    /// 命途。
    pub path: GamePath,
    /// 三／四／五星稀有度。
    pub rarity: u8,
    /// 数据适用等级。
    pub level: u8,
    /// 数据适用突破阶段。
    pub promotion: u8,
    /// 光锥自身属性。
    pub base: BaseCombatStats,
}

/// 玩法允许的站位类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    /// 前台。
    Front,
    /// 后台。
    Back,
    /// 前后台均可。
    Both,
}

/// 后台能量初始值与上限，区分未指定初始值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundEnergy {
    /// 初始值，未指定时为空。
    pub initial: Option<f64>,
    /// 上限。
    pub maximum: f64,
}

/// 一种玩法形态在指定局内星级下的参数，供后续战斗组装使用。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameplayFormData {
    /// 对应常规或玩法专属角色 ID。
    pub character_id: String,
    /// 玩法角色 ID，与常规 ID 分开保存。
    pub gameplay_id: String,
    /// 费用。
    pub cost: u8,
    /// 可用站位类别。
    pub position: Position,
    /// 局内星级。
    pub star: u8,
    /// 前台强度。
    pub front_strength: Option<f64>,
    /// 后台强度。
    pub back_strength: Option<f64>,
    /// 治疗强度。
    pub healing_strength: Option<f64>,
    /// 护盾强度。
    pub shield_strength: Option<f64>,
    /// 生命增幅，使用比例。
    pub hp_amplification: Option<f64>,
    /// 速度增幅，使用比例。
    pub speed_amplification: Option<f64>,
    /// 通用初始能量，未指定时为空。
    pub initial_energy: Option<f64>,
    /// 后台能量参数。
    pub background_energy: Option<BackgroundEnergy>,
    /// 基础伤害修正，使用比例。
    pub base_damage_modifier: Option<f64>,
    /// 每增加一魂的伤害修正，使用比例。
    pub damage_modifier_per_eidolon: Option<f64>,
    /// 基础幸运率。
    pub lucky_rate: f64,
    /// 基础幸运伤害。
    pub lucky_damage: f64,
}
