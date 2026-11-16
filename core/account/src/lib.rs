//! 玩家导入和程序生成共用的原始账号数据模型。
//!
//! 字段保留账号原始练度，百分比使用比例值。来源适配器负责字段映射，
//! 后续配置转换负责试用补齐与玩法保底；训练绑定通过本 crate 使用相同类型。

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod character;
pub mod equipment;
pub mod import;
pub mod raw;
pub mod stats;

pub use character::{CharacterConfig, TraceConfig};
pub use equipment::{InvalidRelicSlot, LightConeInstance, RelicInstance, RelicSlot};
pub use raw::{RAW_ACCOUNT_SCHEMA_VERSION, RawAccount};
pub use stats::{RelicMainStat, RelicSubStat};
