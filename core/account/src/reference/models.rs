//! 文档数值的共用元数据、角色和光锥基础属性，以及局内星级参数。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 每份数据或数值在来源文档中的核对状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verification {
    /// 文档已经给出数值。
    Documented,
    /// 文档明确说明已经完成游戏内对照。
    InGameVerified,
    /// 文档明确要求继续完成游戏内对照。
    PendingInGame,
}

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

/// 生成资料所用的文档和规范化为 LF 后的 SHA-256。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentSource {
    /// 相对于仓库根目录的文档路径。
    pub path: String,
    /// 文档内容摘要。
    pub sha256: String,
}

/// 版本化数据文件；`data` 使用对应业务类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataFile<T> {
    /// 数据结构版本。
    pub schema_version: u32,
    /// 游戏版本。
    pub game_version: String,
    /// 玩法赛季。
    pub season: String,
    /// 数据来源文档。
    pub source_documents: Vec<DocumentSource>,
    /// 该文件的数据内容。
    pub data: T,
}

/// 数据快照中一个文件的摘要和条目数量。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataFileSummary {
    /// 文件按 LF 保存时的 SHA-256。
    pub sha256: String,
    /// 记录数量；复合遗器文件统计主词条、副词条和套装条目之和。
    pub count: usize,
}

/// 文档尚未提供或尚未核对的数据项。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataGap {
    /// 缺项类别。
    pub key: String,
    /// 需要补充的具体内容。
    pub description: String,
}

/// 一组文档数据的版本、文件索引和缺项。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataManifest {
    /// 清单结构版本。
    pub schema_version: u32,
    /// 快照 ID，关联其中全部文件。
    pub snapshot_id: String,
    /// 游戏版本。
    pub game_version: String,
    /// 玩法赛季。
    pub season: String,
    /// 相对于快照目录的文件索引。
    pub files: BTreeMap<String, DataFileSummary>,
    /// 已记录的待补充内容。
    pub known_gaps: Vec<DataGap>,
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
    /// 图鉴显示名。
    pub name: String,
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
    /// 文档提供的图鉴条目 ID。
    pub encyclopedia_entry_id: Option<String>,
    /// 游戏内核对状态。
    pub verification: Verification,
    /// 来源文档中的行号。
    pub source_line: u32,
}

/// 文档记录的一种光锥的 80 级基础资料。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightConeData {
    /// 光锥种类 ID。
    pub id: String,
    /// 显示名。
    pub name: String,
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
    /// 核对状态。
    pub verification: Verification,
    /// 来源行号。
    pub source_line: u32,
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
    /// 来源核对状态。
    pub verification: Verification,
    /// 来源行号。
    pub source_line: u32,
}
