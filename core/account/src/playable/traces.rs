//! 在同一角色和能力版本下，按试用配置补齐基础行迹。

use std::{error::Error, fmt};

use crate::TraceConfig;

/// 原始账号缺少某项基础技能等级，无法判断其与试用等级的大小。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingTraceLevel {
    /// 信息缺失的真实行迹节点 ID。
    pub node_id: String,
}

impl fmt::Display for MissingTraceLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "缺少行迹节点 {} 的原始基础等级", self.node_id)
    }
}

impl Error for MissingTraceLevel {}

/// 按试用基础行迹补齐，保留本体更高等级和已经解锁的节点。
///
/// 调用方先将本体和试用绑定到同一角色、能力版本的真实节点 ID。技能等级
/// 逐项取较大值；试用已激活的一次性节点补为已激活，其余保留本体已知状态。
/// 星魂附加等级在本函数之后计算。缺少原始技能等级时返回具体节点，原始配置保持原值。
pub fn raise_traces_to_trial(
    owned: &TraceConfig,
    trial: &TraceConfig,
) -> Result<TraceConfig, MissingTraceLevel> {
    let mut result = owned.clone();
    for (node_id, trial_level) in &trial.skill_levels {
        let owned_level = owned
            .skill_levels
            .get(node_id)
            .ok_or_else(|| MissingTraceLevel {
                node_id: node_id.clone(),
            })?;
        result
            .skill_levels
            .insert(node_id.clone(), (*owned_level).max(*trial_level));
    }
    for (node_id, unlocked) in &trial.node_unlocks {
        if *unlocked {
            result.node_unlocks.insert(node_id.clone(), true);
        }
    }
    Ok(result)
}
