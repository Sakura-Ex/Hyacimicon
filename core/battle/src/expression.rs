//! Runtime numeric expressions. Variable lookup is supplied by the owning state.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expression {
    Constant { value: f64 },
    Variable { key: String },
    Add { left: Box<Self>, right: Box<Self> },
    Subtract { left: Box<Self>, right: Box<Self> },
    Multiply { left: Box<Self>, right: Box<Self> },
    Divide { left: Box<Self>, right: Box<Self> },
    Maximum { left: Box<Self>, right: Box<Self> },
    Floor { value: Box<Self> },
    Negate { value: Box<Self> },
}

impl Expression {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_at(0)
    }
    fn validate_at(&self, depth: usize) -> Result<(), String> {
        if depth > 32 {
            return Err("expression nesting exceeds 32".into());
        }
        match self {
            Self::Constant { value } if !value.is_finite() => {
                Err("nonfinite expression constant".into())
            }
            Self::Variable { key } if key.is_empty() || key.len() > 200 => {
                Err("invalid dynamic key".into())
            }
            Self::Add { left, right }
            | Self::Subtract { left, right }
            | Self::Multiply { left, right }
            | Self::Divide { left, right }
            | Self::Maximum { left, right } => {
                left.validate_at(depth + 1)?;
                right.validate_at(depth + 1)
            }
            Self::Negate { value } | Self::Floor { value } => value.validate_at(depth + 1),
            _ => Ok(()),
        }
    }
    /// Missing bindings and invalid arithmetic fail; neither becomes a zero value.
    pub fn evaluate(&self, lookup: &impl Fn(&str) -> Option<f64>) -> Result<f64, String> {
        let value = match self {
            Self::Constant { value } => *value,
            Self::Variable { key } => {
                lookup(key).ok_or_else(|| format!("unresolved dynamic key {key}"))?
            }
            Self::Add { left, right } => left.evaluate(lookup)? + right.evaluate(lookup)?,
            Self::Subtract { left, right } => left.evaluate(lookup)? - right.evaluate(lookup)?,
            Self::Multiply { left, right } => left.evaluate(lookup)? * right.evaluate(lookup)?,
            Self::Divide { left, right } => {
                let denominator = right.evaluate(lookup)?;
                if denominator == 0.0 {
                    return Err("dynamic division by zero".into());
                }
                left.evaluate(lookup)? / denominator
            }
            Self::Negate { value } => -value.evaluate(lookup)?,
            Self::Floor { value } => value.evaluate(lookup)?.floor(),
            Self::Maximum { left, right } => left.evaluate(lookup)?.max(right.evaluate(lookup)?),
        };
        if !value.is_finite() {
            return Err("nonfinite dynamic expression result".into());
        }
        Ok(value)
    }
}
