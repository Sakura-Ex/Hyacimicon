//! 显式输入、固定随机源的战斗模拟与批次评估。
pub mod batch;
pub mod decision;
pub mod engine;
pub mod mechanics;
pub mod model;
pub mod settlement;

// 保留现有调用方使用的公开路径；实现统一放在 mechanics。
pub use mechanics::{dynamic_scope, elements, expression, math, modifiers, weakness};
