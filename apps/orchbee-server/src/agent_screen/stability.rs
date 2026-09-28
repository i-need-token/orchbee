//! Immutable screen publication preparation; caller commits timing state only with the actual report update.
use super::{Detection, PanelAgentState};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Stability {
    published: Detection,
    pending_idle: Option<(u64, u8)>,
    last_refresh: Option<u64>,
    last_sample: Option<u64>,
}
pub(super) struct Prepared {
    pub(super) next: Stability,
    pub(super) publish: Option<Detection>,
}
impl Stability {
    pub(super) fn new(initial: Detection) -> Self {
        Self {
            published: initial,
            pending_idle: None,
            last_refresh: None,
            last_sample: None,
        }
    }
    /// Time is supplied as monotonic milliseconds. A stale sample has no effect.
    pub(super) fn prepare(
        &self,
        detected: Detection,
        now: u64,
        identity_changed_or_exited: bool,
    ) -> Result<Prepared, &'static str> {
        if self.last_sample.is_some_and(|last| now < last) {
            return Err("Stale screen observation");
        }
        let mut next = self.clone();
        next.last_sample = Some(now);
        if detected.skip {
            next.pending_idle = None;
            return Ok(Prepared {
                next,
                publish: None,
            });
        }
        let plain_idle = self.published.state == PanelAgentState::Working
            && detected.state == PanelAgentState::Idle
            && !detected.visible_idle
            && !detected.visible_blocker
            && !identity_changed_or_exited;
        if plain_idle {
            if let Some((started, count)) = self.pending_idle {
                if now - started < 700 && count < 2 {
                    next.pending_idle = Some((started, count + 1));
                    return Ok(Prepared {
                        next,
                        publish: None,
                    });
                }
                next.pending_idle = None;
            } else {
                next.pending_idle = Some((now, 0));
                return Ok(Prepared {
                    next,
                    publish: None,
                });
            }
        } else {
            next.pending_idle = None;
        }
        let refresh = detected.visible_blocker
            && self.published.visible_blocker
            && self.last_refresh.is_none_or(|last| now - last >= 800);
        let publish = (self.published != detected || identity_changed_or_exited || refresh)
            .then_some(detected);
        if publish.is_some() {
            next.published = detected;
            next.last_refresh = Some(now);
        }
        Ok(Prepared { next, publish })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn signal(state: PanelAgentState, visible: bool) -> Detection {
        Detection {
            state,
            skip: false,
            visible_idle: visible && state == PanelAgentState::Idle,
            visible_blocker: visible && state == PanelAgentState::Blocked,
            visible_working: visible && state == PanelAgentState::Working,
        }
    }
    #[test]
    fn plain_idle_requires_three_more_confirmations_or_elapsed_cap() {
        let original = Stability::new(signal(PanelAgentState::Working, true));
        let idle = signal(PanelAgentState::Idle, false);
        let mut state = original.clone();
        for t in [10, 110, 210] {
            let p = state.prepare(idle, t, false).unwrap();
            assert_eq!(p.publish, None);
            state = p.next;
        }
        assert_eq!(state.prepare(idle, 310, false).unwrap().publish, Some(idle));
        let p = original.prepare(idle, 10, false).unwrap();
        assert_eq!(p.next.prepare(idle, 709, false).unwrap().publish, None);
        assert_eq!(
            p.next.prepare(idle, 710, false).unwrap().publish,
            Some(idle)
        );
        assert_eq!(
            original.pending_idle, None,
            "preparation must not mutate owner"
        );
    }
    #[test]
    fn visible_idle_blocker_identity_change_and_exit_bypass_idle_hold() {
        let initial = Stability::new(signal(PanelAgentState::Working, true));
        let held = initial
            .prepare(signal(PanelAgentState::Idle, false), 1, false)
            .unwrap()
            .next;
        for (d, reset) in [
            (signal(PanelAgentState::Idle, true), false),
            (signal(PanelAgentState::Blocked, true), false),
            (signal(PanelAgentState::Idle, false), true),
        ] {
            let p = held.prepare(d, 2, reset).unwrap();
            assert_eq!(p.publish, Some(d));
            assert_eq!(p.next.pending_idle, None);
        }
    }
    #[test]
    fn stable_blocker_refresh_is_bounded_and_skip_or_stale_cannot_publish() {
        let block = signal(PanelAgentState::Blocked, true);
        let initial = Stability::new(signal(PanelAgentState::Idle, true));
        let state = initial.prepare(block, 100, false).unwrap().next;
        assert_eq!(state.prepare(block, 899, false).unwrap().publish, None);
        assert_eq!(
            state.prepare(block, 900, false).unwrap().publish,
            Some(block)
        );
        let before = state.clone();
        assert!(state.prepare(block, 99, false).is_err());
        assert_eq!(state, before);
        let skip = Detection {
            skip: true,
            ..signal(PanelAgentState::Unknown, false)
        };
        let p = state.prepare(skip, 1000, false).unwrap();
        assert_eq!(p.publish, None);
        assert_eq!(p.next.published, block);
    }
    #[test]
    fn transcript_viewer_clears_idle_confirmation_without_publishing() {
        let working = signal(PanelAgentState::Working, true);
        let idle = signal(PanelAgentState::Idle, false);
        let held = Stability::new(working)
            .prepare(idle, 1, false)
            .unwrap()
            .next;
        let skipped = held
            .prepare(
                Detection {
                    skip: true,
                    ..signal(PanelAgentState::Unknown, false)
                },
                900,
                false,
            )
            .unwrap();
        assert_eq!(skipped.publish, None);
        assert_eq!(skipped.next.published, working);
        assert_eq!(skipped.next.pending_idle, None);
        let resumed = skipped.next.prepare(idle, 1000, false).unwrap();
        assert_eq!(resumed.publish, None);
        assert_eq!(resumed.next.pending_idle, Some((1000, 0)));
    }
}
