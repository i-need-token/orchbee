//! The discovery phases over the shared machine retry policy; timing is independent of threads,
//! transport and wall-clock sleeps.
use crate::machine_retry::Backoff;
use std::time::Instant;

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
    backoff: Backoff,
}
impl Policy {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            phase: Phase::Waiting,
            retry_at: now,
            backoff: Backoff::new(),
        }
    }
    pub(super) fn due(&self, now: Instant) -> bool {
        matches!(self.phase, Phase::Waiting | Phase::Reconnecting) && now >= self.retry_at
    }
    pub(super) fn begin(&mut self) {
        self.phase = Phase::Connecting;
    }
    pub(super) fn healthy(&mut self, now: Instant) {
        self.backoff.online(now);
        self.phase = Phase::Online;
    }
    /// Carrier recovery retains the existing worker; only terminal failures schedule a new one.
    pub(super) fn recovering(&mut self) {
        self.backoff.interrupted();
        self.phase = Phase::Reconnecting;
    }
    pub(super) fn fail(&mut self, permanent: bool, now: Instant) {
        self.phase = if permanent {
            Phase::Attention
        } else {
            Phase::Reconnecting
        };
        self.retry_at = now + self.backoff.fail(now);
    }
    pub(super) fn retry(&mut self, now: Instant) {
        if matches!(self.phase, Phase::Attention | Phase::Reconnecting) {
            self.phase = Phase::Waiting;
            self.retry_at = now;
        }
    }
}

#[cfg(test)]
mod tests;
