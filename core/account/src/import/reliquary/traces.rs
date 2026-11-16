//! 用已固定版本的行迹数据把来源标签绑定到真实节点。

use std::collections::BTreeMap;

use super::{ImportError, ImportReport, ImportWarning, bounded, invalid, source};
use crate::TraceConfig;

pub(super) use crate::reference::TraceCatalog as Catalog;

impl Catalog {
    /// 按角色与能力版本转换已知技能和解锁状态。
    pub(super) fn convert(
        &self,
        character: &source::Character,
        path: &str,
        report: &mut ImportReport,
    ) -> Result<TraceConfig, ImportError> {
        let binding = self
            .characters
            .iter()
            .find(|binding| {
                binding.character_id == character.id
                    && binding.ability_version == character.ability_version
            })
            .ok_or_else(|| {
                invalid(
                    format!("{path}.ability_version"),
                    format!(
                        "行迹快照未覆盖角色 {} 的能力版本 {}",
                        character.id, character.ability_version
                    ),
                )
            })?;
        let mut skill_levels: BTreeMap<_, _> = binding
            .skills
            .values()
            .filter(|node| node.max_level == 1)
            .map(|node| (node.node_id.clone(), 1))
            .collect();
        report.default_techniques += usize::from(binding.skills.contains_key("technique"));
        let mut source_skills = character.skills.clone();
        for name in character.skills.keys() {
            if !["basic", "skill", "ult", "talent", "elation"].contains(&name.as_str()) {
                return Err(invalid(format!("{path}.skills.{name}"), "未知技能标签"));
            }
        }
        if let Some(memosprite) = &character.memosprite {
            for (name, level) in memosprite {
                if !["skill", "talent"].contains(&name.as_str()) {
                    return Err(invalid(
                        format!("{path}.memosprite.{name}"),
                        "未知忆灵技能标签",
                    ));
                }
                source_skills.insert(format!("memosprite.{name}"), *level);
            }
        }
        for (name, level) in &source_skills {
            let field = skill_path(path, name);
            let node = binding
                .skills
                .get(name)
                .ok_or_else(|| invalid(&field, "当前角色能力版本没有对应技能节点"))?;
            bounded(*level, 1, node.max_level, &field)?;
            skill_levels.insert(node.node_id.clone(), *level);
        }
        for (name, node) in &binding.skills {
            if node.max_level > 1 && !source_skills.contains_key(name) {
                report.warnings.push(ImportWarning {
                    path: skill_path(path, name),
                    message: "来源缺少技能等级，统一配置保留为未知".to_owned(),
                });
            }
        }
        let mut node_unlocks = BTreeMap::new();
        for (name, unlocked) in &character.traces {
            if let Some(node) = binding.nodes.get(name) {
                node_unlocks.insert(node.node_id.clone(), *unlocked);
            } else if name != "special" || *unlocked {
                return Err(invalid(
                    format!("{path}.traces.{name}"),
                    "当前角色能力版本没有对应行迹节点",
                ));
            }
            // 导出器为无 special 节点的角色也写出 false，该值为格式占位。
        }
        for name in binding.nodes.keys() {
            if !character.traces.contains_key(name) {
                report.warnings.push(ImportWarning {
                    path: format!("{path}.traces.{name}"),
                    message: "来源缺少行迹解锁状态，统一配置保留为未知".to_owned(),
                });
            }
        }
        Ok(TraceConfig {
            skill_levels,
            node_unlocks,
        })
    }
}

/// 将共用技能标签还原为对应来源对象中的字段路径。
fn skill_path(character_path: &str, name: &str) -> String {
    if name.starts_with("memosprite.") {
        format!("{character_path}.{name}")
    } else {
        format!("{character_path}.skills.{name}")
    }
}
