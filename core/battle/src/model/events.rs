//! 战斗监听事件、插入请求与触发器声明。
use super::*;
use serde::{Deserialize, Serialize};

/// Runtime events are dispatched even when replay recording is disabled.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BattleEvent {
    BattleStart,
    WaveStart,
    TurnStart,
    TurnEnd,
    ActionStart,
    BeforeSkillUse,
    AfterAttack,
    ActionEnd,
    Hit,
    Break,
    Death,
    CycleEnd,
}

impl BattleEvent {
    /// Global events have no unit subject and tick every present owner's lifetime.
    pub fn is_global(self) -> bool {
        matches!(self, Self::BattleStart | Self::WaveStart | Self::CycleEnd)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    Ultimate,
    ExtraTurn,
    FollowUp,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub owner: String,
    pub event: BattleEvent,
    /// Unit-event subject filter; global events require None.
    pub subject: Option<String>,
    pub action: Option<String>,
    /// Zero means unlimited; positive counts are consumed before dispatch.
    pub uses: u32,
    pub effects: Vec<Effect>,
    /// Expire before any listeners of this event. Global ticks apply to present
    /// owners; unit ticks apply only when the owner is the event subject.
    pub expires_on: Option<BattleEvent>,
    pub duration: u32,
}
