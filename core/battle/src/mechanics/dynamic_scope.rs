//! Explicit transient storage for sourced ability and task-template invocations.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DynamicScope {
    Ability,
    TaskTemplate,
}

impl DynamicScope {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ability => "ability",
            Self::TaskTemplate => "task_template",
        }
    }
}

struct Frame {
    owner: usize,
    scope: DynamicScope,
    values: BTreeMap<String, f64>,
}

#[derive(Default)]
pub(crate) struct ScopeFrames(Vec<Frame>);

impl ScopeFrames {
    pub fn push(&mut self, owner: usize, scope: DynamicScope) {
        self.0.push(Frame {
            owner,
            scope,
            values: BTreeMap::new(),
        });
    }
    pub fn pop(&mut self) {
        self.0.pop();
    }
    pub fn get(&self, owner: usize, scope: DynamicScope, key: &str) -> Option<f64> {
        // A matching inner frame shadows the whole outer frame, even when the
        // requested key is absent. Never fall through to a previous invocation.
        self.0
            .iter()
            .rev()
            .find(|f| f.owner == owner && f.scope == scope)
            .and_then(|f| f.values.get(key).copied())
    }
    pub fn set(
        &mut self,
        owner: usize,
        scope: DynamicScope,
        key: &str,
        value: f64,
    ) -> Result<(), String> {
        let frame = self
            .0
            .iter_mut()
            .rev()
            .find(|f| f.owner == owner && f.scope == scope)
            .ok_or_else(|| format!("no active {scope:?} dynamic scope for owner"))?;
        frame.values.insert(key.into(), value);
        Ok(())
    }
}
