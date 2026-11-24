//! 使用显式操作序列驱动会话，并将每一步和诊断事件关联。
use super::{
    replay_checks::{provided_object, ResultExpectation, StateExpectation},
    BattleReport, Event, ScenarioFile,
};
use crate::{
    decision::{Decision, DecisionPoint, Observation, Operation},
    engine::{BattleSession, BattleStep, RecordedDraw},
};
use serde::{Deserialize, Serialize};

/// 按顺序提交到单场会话的诊断输入。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayScript {
    /// 本操作文件的格式版本。
    schema_version: u32,
    /// 每个决策点所需的一个显式选择。
    steps: Vec<ScriptStep>,
    /// Observed branch choices for a strict replay; omission uses the seed.
    #[serde(default, deserialize_with = "deserialize_random_outcomes")]
    random_outcomes: Option<Vec<RecordedDraw>>,
    /// State at the first decision or a battle that ended during opening.
    #[serde(default, deserialize_with = "provided_object")]
    expect_opening: Option<StateExpectation>,
    /// Final outcome and state, including battles with no decisions.
    #[serde(default, deserialize_with = "provided_object")]
    expect_result: Option<ResultExpectation>,
}

fn deserialize_random_outcomes<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<RecordedDraw>>, D::Error> {
    Vec::deserialize(deserializer).map(Some)
}

/// 一个决策点及对应的选择与观察检查。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScriptStep {
    /// 预期出现的决策窗口类型。
    point: DecisionPoint,
    /// 选择要执行的技能，或放弃插入机会。
    choice: Choice,
    /// 可选的事件身份与数值检查，偏移以本步开始为基准。
    #[serde(default)]
    expect: Vec<ExpectedEvent>,
    /// State after this submission advances to the next decision or finish.
    #[serde(default, deserialize_with = "provided_object")]
    expect_after: Option<StateExpectation>,
}

/// 用场景中的稳定标识指定技能，索引由当前合法列表确定。
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Choice {
    /// 放弃当前插入机会。空结构体变体使解析器检查多余字段。
    Skip {},
    /// 选择执行者和技能。
    Action {
        /// 场景的单位 ID。
        actor: String,
        /// 该单位的技能 ID。
        action: String,
        /// Optional stable ID of the selected primary enemy.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
}

/// 待核对事件的身份及可选的 `amount` 数值。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedEvent {
    /// 事件在本步中的零基偏移。
    offset: usize,
    /// 预期事件类型。
    kind: String,
    /// 事件执行者 ID。
    actor: String,
    /// 事件记录的目标 ID。
    target: Option<String>,
    /// 事件记录的技能 ID。
    action: Option<String>,
    /// 精确核对原始事件数值，单位与该事件的 `amount` 相同。
    #[serde(default, deserialize_with = "deserialize_amount")]
    amount: Option<f64>,
}

/// 字段缺省时跳过数值核对，提供字段时必须是数值。
fn deserialize_amount<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f64>, D::Error> {
    f64::deserialize(deserializer).map(Some)
}

impl ExpectedEvent {
    /// 对照已执行的事件，报告具体缺失或不一致的偏移。
    fn verify(&self, events: &[Event]) -> Result<(), String> {
        let actual = events
            .get(self.offset)
            .ok_or_else(|| format!("missing event at offset {}", self.offset))?;
        if actual.kind != self.kind
            || actual.actor != self.actor
            || actual.target != self.target
            || actual.action != self.action
        {
            return Err(format!(
                "event {} mismatch: expected {} actor {:?} action {:?} target {:?}, got {} actor {:?} action {:?} target {:?}",
                actual.index, self.kind, self.actor, self.action, self.target,
                actual.kind, actual.actor, actual.action, actual.target,
            ));
        }
        let Some(expected) = self.amount else {
            return Ok(());
        };
        if actual.amount != Some(expected) {
            let actual_amount = actual
                .amount
                .map_or_else(|| "missing".into(), |amount| amount.to_string());
            return Err(format!(
                "event {} ({}) amount mismatch: expected {expected}, got {actual_amount}",
                actual.index, actual.kind
            ));
        }
        Ok(())
    }
}

/// 可读的合法操作，独立于运行时的索引。
#[derive(Debug, Serialize)]
struct NamedOperation {
    /// 执行者 ID。
    actor: String,
    /// 技能 ID。
    action: String,
}

/// 单次决策的输入、提交前的观测与对应回放范围。
#[derive(Debug, Serialize)]
struct ReplayStep<'a> {
    /// 本会话内的决策编号。
    decision_id: u64,
    /// 实际出现的决策类型。
    point: DecisionPoint,
    /// 当前波次，首波为 1。
    wave: usize,
    /// 提交前的行动轴、预算和单位状态。
    observation: Observation,
    /// 当时可选的单位和技能。
    legal: Vec<NamedOperation>,
    /// 已提交的选择。
    choice: &'a Choice,
    /// 提交前的事件位置，包含于本步范围中。
    event_start: usize,
    /// 提交返回后的事件位置。此前是提交阶段，此后是推进阶段。
    event_after_submit: usize,
    /// 到下一决策或终局时的事件位置，不包含于本步范围中。
    event_end: usize,
}

/// 兼有普通战斗报告与逐步操作索引的诊断结果。
#[derive(Debug, Serialize)]
pub struct ReplayReport<'a> {
    /// 战斗结果与完整事件数组。
    #[serde(flatten)]
    battle: BattleReport,
    /// 按会话决策顺序排列的记录。
    steps: Vec<ReplayStep<'a>>,
    /// All recorded outcomes were consumed on a successful strict replay.
    #[serde(skip_serializing_if = "Option::is_none")]
    random_outcomes: Option<&'a [RecordedDraw]>,
}

/// 执行完整脚本。操作缺失、多余、非法、事件不符或回放截断都会返回错误。
pub fn replay<'a>(
    file: &ScenarioFile,
    seed: u64,
    script: &'a ReplayScript,
) -> Result<ReplayReport<'a>, String> {
    if script.schema_version != 1 {
        return Err(format!(
            "unsupported replay schema version {}",
            script.schema_version
        ));
    }
    let opening_error = |error| {
        if script.random_outcomes.is_some() {
            format!("opening: {error}")
        } else {
            error
        }
    };
    let mut session =
        BattleSession::with_recorded_random(&file.battle, seed, script.random_outcomes.clone())
            .map_err(opening_error)?;
    let mut next = advance(&mut session).map_err(opening_error)?;
    session
        .check_random_interval(next.is_none())
        .map_err(opening_error)?;
    if let Some(expected) = &script.expect_opening {
        verify_boundary(&session, next.as_ref(), expected)
            .map_err(|error| format!("opening state: {error}"))?;
    }
    let mut steps = Vec::new();
    for (index, entry) in script.steps.iter().enumerate() {
        let decision = next.ok_or_else(|| format!("battle finished before script step {index}"))?;
        let context = format!("script step {index}, decision {}", decision.id);
        if entry.point != decision.point {
            return Err(format!(
                "{context}: expected {:?}, got {:?}",
                entry.point, decision.point
            ));
        }
        let event_start = session.replay_trace()?.len();
        let wave = session.replay_wave();
        let legal = named_operations(&session, &decision);
        let choice = resolve_choice(&entry.choice, &decision.legal, &legal)
            .map_err(|error| format!("{context}: {error}"))?;
        session.set_random_step(index);
        let target = match &entry.choice {
            Choice::Action { target, .. } => target.as_deref(),
            Choice::Skip {} => None,
        };
        match (choice, target) {
            (Some(op), Some(target)) => session.submit_targeted(decision.id, op, target),
            (choice, _) => session.submit(decision.id, choice),
        }
        .map_err(|error| format!("{context}: {error}"))?;
        let event_after_submit = session.replay_trace()?.len();
        next = advance(&mut session).map_err(|error| format!("{context}: {error}"))?;
        session
            .check_random_interval(next.is_none())
            .map_err(|error| format!("{context}: {error}"))?;
        let trace = session.replay_trace()?;
        for check in &entry.expect {
            check
                .verify(&trace[event_start..])
                .map_err(|error| format!("{context}: {error}"))?;
        }
        if let Some(expected) = &entry.expect_after {
            verify_boundary(&session, next.as_ref(), expected)
                .map_err(|error| format!("{context}, state after: {error}"))?;
        }
        steps.push(ReplayStep {
            decision_id: decision.id,
            point: decision.point,
            wave,
            observation: decision.observation,
            legal,
            choice: &entry.choice,
            event_start,
            event_after_submit,
            event_end: trace.len(),
        });
    }
    if let Some(decision) = next {
        return Err(format!(
            "script ended before decision {} ({:?})",
            decision.id, decision.point
        ));
    }
    session.replay_trace()?;
    if let Some(expected) = &script.expect_result {
        let result = session
            .replay_result()
            .ok_or("battle has no terminal snapshot")?;
        expected
            .verify(session.replay_wave(), result)
            .map_err(|error| format!("result: {error}"))?;
    }
    Ok(ReplayReport {
        battle: file.report(session.into_result()?, seed),
        steps,
        random_outcomes: script.random_outcomes.as_deref(),
    })
}

/// Check a snapshot at the same boundary already used by the replay loop.
fn verify_boundary(
    session: &BattleSession<'_>,
    next: Option<&Decision>,
    expected: &StateExpectation,
) -> Result<(), String> {
    let wave = session.replay_wave();
    if let Some(decision) = next {
        expected.verify_observation(wave, &decision.observation)
    } else {
        let result = session
            .replay_result()
            .ok_or("battle has no terminal snapshot")?;
        expected.verify_result(wave, result)
    }
}

/// 推进到下一决策并取出快照，便于继续提交和读取事件。
fn advance(session: &mut BattleSession<'_>) -> Result<Option<Decision>, String> {
    match session.advance()? {
        BattleStep::Decision(decision) => Ok(Some(decision.clone())),
        BattleStep::Finished(_) => Ok(None),
    }
}

/// 用当前在场单位读取合法操作的名字，保留合法列表顺序。
fn named_operations(session: &BattleSession<'_>, decision: &Decision) -> Vec<NamedOperation> {
    decision
        .legal
        .iter()
        .map(|op| {
            let (actor, action) = session.operation_names(op);
            NamedOperation {
                actor: actor.into(),
                action: action.into(),
            }
        })
        .collect()
}

/// 从同一决策的合法列表解析显式选择，报告不存在或歧义的名字。
fn resolve_choice(
    choice: &Choice,
    operations: &[Operation],
    names: &[NamedOperation],
) -> Result<Option<Operation>, String> {
    let Choice::Action { actor, action, .. } = choice else {
        return Ok(None);
    };
    let mut matching = operations
        .iter()
        .zip(names)
        .filter(|(_, name)| &name.actor == actor && &name.action == action);
    let (selected, _) = matching
        .next()
        .ok_or_else(|| format!("illegal operation {actor:?}/{action:?}"))?;
    if matching.next().is_some() {
        return Err(format!("ambiguous operation {actor:?}/{action:?}"));
    }
    Ok(Some(selected.clone()))
}
