//! 分步会话与同步运行入口，共用同一套战斗推进。
use crate::{
    decision::*,
    elements::{DamageElement, Element},
    engine::timeline::{self as scheduler, Timeline, DISTANCE},
    math,
    model::*,
    modifiers::{ModifierBook, Property},
};
use rand_chacha::ChaCha8Rng;

mod damage;
mod effects;
mod events;
mod operations;
mod result;
mod rng;
mod session;
mod state;
mod targets;
mod timeline;
mod values;

use effects::finite;
use rng::{stream, uniform};
use state::{CombatUnit, Pending, State};

pub use session::{BattleSession, BattleStep};

pub const RNG_ALGORITHM: &str =
    "ChaCha8Rng/rand_chacha=0.9.0; SHA256-separated target/critical/element streams v1";
const TRACE_LIMIT: usize = 100_000;

/// Run a single scenario without cancellation; the input remains unchanged.
pub fn simulate(input: &ValidatedScenario, seed: u64, trace: bool) -> Result<BattleResult, String> {
    simulate_cancellable(input, seed, trace, &mut || false)
}
/// Check cancellation before each action, including immediate ultimate chains.
/// Checks occur at action boundaries before selecting an action or advancing AV,
/// never partway through an action's effects. A callback returning `true` yields
/// `Ok(BattleResult)` with [`Outcome::Incomplete`] and [`Termination::Cancelled`].
/// 清场/全灭优先结束；AV归零后，已进入的单位Turn先完成收尾，
/// 最终倒计时回合结束后才按额度耗尽结算，再处理新的取消或行动保护。
pub fn simulate_cancellable(
    input: &ValidatedScenario,
    seed: u64,
    trace: bool,
    cancel: &mut impl FnMut() -> bool,
) -> Result<BattleResult, String> {
    simulate_with_policy(input, seed, trace, cancel, &mut Automatic)
}

/// Execute through legal operations; policy cannot mutate battle state.
pub fn simulate_with_policy(
    input: &ValidatedScenario,
    seed: u64,
    trace: bool,
    cancel: &mut impl FnMut() -> bool,
    policy: &mut impl Policy,
) -> Result<BattleResult, String> {
    let mut session = BattleSession::new(input, seed, trace)?;
    loop {
        let (id, choice) = match session.advance_cancellable(cancel)? {
            BattleStep::Decision(decision) => (
                decision.id,
                policy.decide(decision.point, &decision.observation, &decision.legal),
            ),
            BattleStep::Finished(_) => return session.into_result(),
        };
        session.submit(id, choice)?;
    }
}
