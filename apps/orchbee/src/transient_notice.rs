//! One bounded coalesced notice lifetime; receipts and pending operations keep their own owners.
use std::time::{Duration, Instant};

const LIFETIME: Duration = Duration::from_secs(4);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NoticeTimer<T> {
    observed: Option<(T, Instant)>,
}
impl<T> Default for NoticeTimer<T> {
    fn default() -> Self {
        Self { observed: None }
    }
}
impl<T: Copy + Eq> NoticeTimer<T> {
    /// Identical notices coalesce until cleared; repaint never extends their lifetime.
    pub(crate) fn expired(&mut self, value: Option<T>, now: Instant) -> bool {
        let Some(value) = value else {
            self.observed = None;
            return false;
        };
        match self.observed {
            Some((prior, since)) if prior == value => {
                if now.saturating_duration_since(since) >= LIFETIME {
                    self.observed = None;
                    true
                } else {
                    false
                }
            }
            _ => {
                self.observed = Some((value, now));
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_paints_do_not_extend_notice_lifetime() {
        let now = Instant::now();
        let mut timer = NoticeTimer::default();
        assert!(!timer.expired(Some(1), now));
        assert!(!timer.expired(Some(1), now + Duration::from_secs(3)));
        assert!(timer.expired(Some(1), now + Duration::from_secs(4)));
        assert!(!timer.expired(Some(2), now + Duration::from_secs(4)));
        assert!(!timer.expired(None, now + Duration::from_secs(5)));
        assert!(!timer.expired(Some(2), now + Duration::from_secs(8)));
    }
}
