//! 原始账号的统一库存、配置标识和资料版本。

use serde::{Deserialize, Serialize};

use crate::{CharacterConfig, LightConeInstance, RelicInstance};

/// 本 crate 当前定义的原始账号序列化格式版本。
pub const RAW_ACCOUNT_SCHEMA_VERSION: u32 = 1;

/// 账号实际持有的角色与完整装备库存。
///
/// 角色形态按常规角色 ID 分别保存。装备关系由角色引用库存实例，未被引用的
/// 实例属于闲置库存。列表保留来源顺序，也让接入层可以检查重复 ID。
///
/// 本类型承载字段；引用完整性、数值合法性和版本兼容性由后续接入接口检查。
/// 创建数据时将 `schema_version` 设为 [`RAW_ACCOUNT_SCHEMA_VERSION`]。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawAccount {
    /// 内部格式版本，与来源导出格式版本分别记录。
    pub schema_version: u32,
    /// 调用方分配的配置标识，用于关联原始配置与后续生效配置。
    pub config_id: String,
    /// 已确认的游戏版本；未知时为 `None`。
    pub game_version: Option<String>,
    /// 已关联的游戏数据快照 ID；尚未关联时为 `None`。
    pub data_snapshot: Option<String>,
    /// 实际持有的角色及各自的原始配装。
    pub characters: Vec<CharacterConfig>,
    /// 完整光锥库存，包含闲置及同种类的多个实例。
    pub light_cones: Vec<LightConeInstance>,
    /// 完整遗器库存，包含闲置、未强化及未满强化实例。
    pub relics: Vec<RelicInstance>,
}
