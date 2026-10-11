//! Immutable screen publication preparation; caller commits timing state only with the actual report update.
use super::{Detection, PanelAgentState};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Stability {
    published: Detection,
    pending_idle: Option<(u64, u8)>,
    last_refresh: Option<u64>,
    last_sample: Option<u64>,
}
pub(crate) struct Prepared {
    pub(crate) next: Stability,
    pub(crate) publish: Option<Detection>,
}
impl Stability {
    pub(crate) fn new(initial: Detection) -> Self {
        Self {
            published: initial,
            pending_idle: None,
            last_refresh: None,
            last_sample: None,
        }
    }
    pub(crate) fn published(&self) -> Detection {
        self.published
    }
    /// Time is supplied as monotonic milliseconds. A stale sample has no effect.
    pub(crate) fn prepare(
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
mod tests;
