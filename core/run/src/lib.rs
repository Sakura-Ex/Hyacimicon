//! A run advances from one actual sampled battle outcome, with explicit settlement.
mod env;
mod model;
mod resolution;
mod settlement;

pub use env::{derive_node_seed, RunEnv};
pub use model::*;
pub use resolution::{diagnose, resolve_once};
pub use settlement::validate_rules;
