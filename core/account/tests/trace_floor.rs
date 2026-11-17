//! 验证试用行迹保底保留本体高等级、已激活节点和未知状态。

use std::collections::BTreeMap;

use account_core::{TraceConfig, playable::traces::raise_traces_to_trial};

/// 用四项技能基础等级构造瓦尔特普通形态的最小行迹。
fn traces(levels: [u8; 4]) -> TraceConfig {
    TraceConfig {
        skill_levels: ["1004001", "1004002", "1004003", "1004004"]
            .into_iter()
            .zip(levels)
            .map(|(node, level)| (node.to_owned(), level))
            .collect(),
        node_unlocks: BTreeMap::new(),
    }
}

#[test]
fn raises_each_skill_independently_and_preserves_owned_unlocks() {
    let mut owned = traces([4, 10, 7, 9]);
    owned.node_unlocks = BTreeMap::from([
        ("1004201".to_owned(), false),
        ("1004209".to_owned(), true),
        ("1004210".to_owned(), false),
    ]);
    let original = owned.clone();
    let mut trial = traces([5, 8, 8, 8]);
    trial.node_unlocks = BTreeMap::from([
        ("1004201".to_owned(), true),
        ("1004202".to_owned(), true),
        ("1004209".to_owned(), false),
        ("1004210".to_owned(), false),
    ]);
    let result = raise_traces_to_trial(&owned, &trial).unwrap();
    assert_eq!(result.skill_levels, traces([5, 10, 8, 9]).skill_levels);
    assert!(result.node_unlocks["1004201"]);
    assert!(result.node_unlocks["1004202"]);
    assert!(result.node_unlocks["1004209"]);
    assert!(!result.node_unlocks["1004210"]);
    assert_eq!(owned, original);
    assert_eq!(raise_traces_to_trial(&result, &trial).unwrap(), result);
}

#[test]
fn uses_the_actual_trial_floor_and_reports_unknown_skill_levels() {
    let mut owned = traces([5, 8, 8, 10]);
    let mut trial = traces([6, 10, 10, 10]);
    trial.node_unlocks.insert("1004209".to_owned(), false);
    let result = raise_traces_to_trial(&owned, &trial).unwrap();
    assert_eq!(result.skill_levels, traces([6, 10, 10, 10]).skill_levels);
    assert!(!result.node_unlocks.contains_key("1004209"));
    owned.skill_levels.remove("1004002");
    assert_eq!(
        raise_traces_to_trial(&owned, &trial).unwrap_err().node_id,
        "1004002"
    );
}
