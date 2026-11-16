//! 玩家导入和程序生成共用的账号模型、参考数据与实际配置来源计划。
//!
//! 字段保留账号原始练度，百分比使用比例值。来源适配器负责字段映射，
//! 来源计划按持有情况选择本体或试用，后续展开负责玩法映射和静态面板；
//! 训练绑定通过本 crate 使用相同类型。

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod attributes;
pub mod character;
pub mod equipment;
pub mod import;
pub mod planning;
pub mod playable;
pub mod raw;
pub mod reference;
pub mod stats;

pub use character::{CharacterConfig, TraceConfig};
pub use equipment::{InvalidRelicSlot, LightConeInstance, RelicInstance, RelicSlot};
pub use raw::{RAW_ACCOUNT_SCHEMA_VERSION, RawAccount};
pub use stats::{RelicMainStat, RelicSubStat};
