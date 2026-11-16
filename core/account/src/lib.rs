//! 玩家导入和程序生成共用的账号模型、参考数据与实际配置来源计划。
//!
//! 字段保留账号原始练度，百分比使用比例值。来源适配器负责字段映射，
//! 配置转换按持有情况选择本体或试用，应用玩法保底并计算静态面板；
//! 训练绑定通过本 crate 使用相同类型。

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod import;
pub mod playable;
pub mod raw;
pub mod reference;

pub use raw::{
    CharacterConfig, InvalidRelicSlot, LightConeInstance, RAW_ACCOUNT_SCHEMA_VERSION, RawAccount,
    RelicInstance, RelicMainStat, RelicSlot, RelicSubStat, TraceConfig,
};
