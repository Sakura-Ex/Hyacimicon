//! Timing and normal-action selection for the existing basic_v1 profile.
//!
//! Absolute AV and expression order are retained for replay compatibility.
//! Cycle rules, mutable speed, extra turns and pending requests are later work.

#[derive(Default)]
pub(crate) struct Timeline {
    now: f64,
    penalty: f64,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AdvanceResult {
    Ready,
    Deadline,
}

impl Timeline {
    pub(crate) fn now(&self) -> f64 {
        self.now
    }

    pub(crate) fn penalty(&self) -> f64 {
        self.penalty
    }

    pub(crate) fn add_penalty(&mut self, amount: f64) -> Result<(), String> {
        let penalty = self.penalty + amount;
        if !penalty.is_finite() {
            return Err("nonfinite revival penalty".into());
        }
        self.penalty = penalty;
        Ok(())
    }

    /// Already elapsed budget only; caller checks this before cancellation.
    pub(crate) fn exhausted(&self, max_av: f64) -> bool {
        self.now + self.penalty >= max_av
    }

    /// Called after immediate actions, cancellation and action-limit checks.
    /// An event exactly at the deadline is not executed under basic_v1.
    pub(crate) fn advance_to(&mut self, time: f64, max_av: f64) -> Result<AdvanceResult, String> {
        if time + self.penalty >= max_av {
            self.now = (max_av - self.penalty).max(self.now);
            return Ok(AdvanceResult::Deadline);
        }
        if time < self.now {
            return Err("action clock moved backwards".into());
        }
        self.now = time;
        Ok(AdvanceResult::Ready)
    }

    pub(crate) fn shifted(&self, before: f64, fraction: f64, interval: f64) -> f64 {
        (before - fraction * interval).max(self.now)
    }
}

/// Candidates retain their original unit indices after filtering dead units.
/// Equal times use the existing input-order tie rule, not iterator order.
pub(crate) fn next_normal(candidates: impl Iterator<Item = (usize, f64)>) -> Option<(usize, f64)> {
    candidates.min_by(|(ia, a), (ib, b)| a.total_cmp(b).then(ia.cmp(ib)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn penalty_consumes_budget_without_advancing_natural_time() {
        let mut t = Timeline::default();
        assert_eq!(t.advance_to(50.0, 100.0).unwrap(), AdvanceResult::Ready);
        t.add_penalty(49.0).unwrap();
        assert!(!t.exhausted(100.0));
        t.add_penalty(1.0).unwrap();
        assert!(t.exhausted(100.0));
        assert_eq!((t.now(), t.penalty()), (50.0, 50.0));
    }

    #[test]
    fn deadline_is_strict_and_uses_remaining_budget() {
        let mut t = Timeline::default();
        t.add_penalty(25.0).unwrap();
        assert_eq!(t.advance_to(74.0, 100.0).unwrap(), AdvanceResult::Ready);
        assert_eq!(t.advance_to(75.0, 100.0).unwrap(), AdvanceResult::Deadline);
        assert_eq!(t.now(), 75.0);
    }

    #[test]
    fn boundary_float_is_not_rounded_into_deadline() {
        let mut t = Timeline::default();
        assert_eq!(
            t.advance_to(81.00445524503841, 81.00445524503843).unwrap(),
            AdvanceResult::Ready
        );
        assert_eq!(t.now().to_bits(), 81.00445524503841_f64.to_bits());
    }

    #[test]
    fn normal_selection_retains_original_indices() {
        let candidates = [(9, 50.0), (3, 50.0), (1, 60.0)];
        assert_eq!(next_normal(candidates.into_iter()), Some((3, 50.0)));
        assert_eq!(next_normal(std::iter::empty()), None);
    }

    #[test]
    fn shifting_clamps_at_now_and_allows_delay() {
        let mut t = Timeline::default();
        t.advance_to(50.0, 200.0).unwrap();
        assert_eq!(t.shifted(100.0, 1.0, 100.0), 50.0);
        assert_eq!(t.shifted(100.0, -0.5, 100.0), 150.0);
    }

    #[test]
    fn timing_errors_leave_the_clock_unchanged() {
        let mut t = Timeline::default();
        t.advance_to(50.0, 100.0).unwrap();
        assert_eq!(
            t.advance_to(49.0, 100.0).unwrap_err(),
            "action clock moved backwards"
        );
        assert_eq!(
            t.add_penalty(f64::INFINITY).unwrap_err(),
            "nonfinite revival penalty"
        );
        assert_eq!((t.now(), t.penalty()), (50.0, 0.0));
    }
}
