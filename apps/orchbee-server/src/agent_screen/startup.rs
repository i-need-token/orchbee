//! The startup window of one published Agent run. From the first sample that selects its process
//! until a screen rule or a program status first recognises its state, the run reads as opening;
//! a run that is still unrecognised when the settle time ends reads as unknown. Neither step can
//! publish blocked: only a rule, a program status or a report says an Agent needs its user.
use super::{Detection, PanelAgentState};
use std::time::Duration;

/// Evidence without a visible working, idle or blocker flag waits this long after first sight.
pub(crate) const STARTUP_GRACE: Duration = Duration::from_secs(3);
/// How long an unrecognised run reads as opening. Captured Agents that fetch plugins, install
/// skills or start a daemon before drawing take several seconds to show a first screen.
pub(crate) const OPENING_SETTLE: Duration = Duration::from_secs(15);

impl Detection {
    /// The published state of a run whose screen no rule has recognised yet.
    pub(crate) fn opening() -> Self {
        Self {
            state: PanelAgentState::Opening,
            ..Self::fallback(false)
        }
    }

    /// Whether this evidence names a state a rule or a program status recognised.
    pub(crate) const fn recognised(self) -> bool {
        !matches!(
            self.state,
            PanelAgentState::Unknown | PanelAgentState::Opening
        )
    }
}

/// The evidence one sample passes to screen stability, or `None` to leave the published fallback
/// as it is. `since_seen` runs from the run's first sight; `recognised` says whether a recognised
/// state was already published for this run.
pub(crate) fn admit(
    detection: Option<Detection>,
    since_seen: Duration,
    recognised: bool,
) -> Option<Detection> {
    let settled = since_seen >= OPENING_SETTLE;
    let detection = match detection {
        Some(detection) => detection,
        // A sample without a screen is no evidence, except that an unrecognised run's settle
        // time has ended.
        None if !recognised && settled => Detection::fallback(false),
        None => return None,
    };
    let visible = detection.visible_working || detection.visible_idle || detection.visible_blocker;
    if !visible && since_seen < STARTUP_GRACE {
        return None;
    }
    // An unreadable screen keeps an unrecognised run opening until the settle time ends.
    if !recognised && !settled && !detection.skip && detection.state == PanelAgentState::Unknown {
        return Some(Detection::opening());
    }
    Some(detection)
}

#[cfg(test)]
mod tests;
