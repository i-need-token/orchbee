//! Retry timing is independent of threads, transport and wall-clock sleeps.
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Waiting,
    Connecting,
    Online,
    Reconnecting,
    Attention,
}
pub(super) struct Policy {
    pub(super) phase: Phase,
    retry_at: Instant,
    failures: u32,
    online_since: Option<Instant>,
}
impl Policy {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            phase: Phase::Waiting,
            retry_at: now,
            failures: 0,
            online_since: None,
        }
    }
    pub(super) fn due(&self, now: Instant) -> bool {
        matches!(self.phase, Phase::Waiting | Phase::Reconnecting) && now >= self.retry_at
    }
    pub(super) fn begin(&mut self) {
        self.phase = Phase::Connecting;
    }
    pub(super) fn healthy(&mut self, now: Instant) {
        self.online_since.get_or_insert(now);
        self.phase = Phase::Online;
    }
    pub(super) fn fail(&mut self, permanent: bool, now: Instant) {
        if self
            .online_since
            .take()
            .is_some_and(|since| now.saturating_duration_since(since) >= Duration::from_secs(60))
        {
            self.failures = 0;
        }
        self.phase = if permanent {
            Phase::Attention
        } else {
            Phase::Reconnecting
        };
        let delay = Duration::from_millis((500_u64 << self.failures.min(8)).min(120_000));
        self.retry_at = now + delay;
        self.failures = self.failures.saturating_add(1);
    }
    pub(super) fn retry(&mut self, now: Instant) {
        if matches!(self.phase, Phase::Attention | Phase::Reconnecting) {
            self.phase = Phase::Waiting;
            self.retry_at = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retry_is_bounded_and_only_sustained_online_resets_history() {
        let mut now = Instant::now();
        let mut p = Policy::new(now);
        for millis in [
            500, 1000, 2000, 4000, 8000, 16000, 32000, 64000, 120000, 120000,
        ] {
            assert!(p.due(now));
            p.begin();
            assert!(!p.due(now));
            p.healthy(now);
            p.fail(false, now + Duration::from_secs(1));
            now += Duration::from_secs(1) + Duration::from_millis(millis);
            assert!(!p.due(now - Duration::from_millis(1)));
            assert!(p.due(now));
        }
        p.begin();
        p.healthy(now);
        p.healthy(now + Duration::from_secs(30));
        p.fail(false, now + Duration::from_secs(60));
        assert!(p.due(now + Duration::from_millis(60_500)));
    }
    #[test]
    fn permanent_failure_waits_for_explicit_retry() {
        let now = Instant::now();
        let mut p = Policy::new(now);
        p.begin();
        p.fail(true, now);
        assert!(!p.due(now + Duration::from_secs(3600)));
        p.retry(now);
        assert!(p.due(now));
    }
}
