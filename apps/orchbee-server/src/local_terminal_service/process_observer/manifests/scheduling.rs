//! Automatic checks share the maintenance slot but never consume manual retry receipts.
use super::Operation;
use orchbee_workbench_protocol::AgentManifestAutomatic;
use std::time::{Duration, Instant};
const INTERVAL: Duration = Duration::from_secs(30 * 60);
pub(super) struct Schedule {
    background: bool,
    release: bool,
    due: Option<Instant>,
    pub(super) last: Option<Operation>,
}
impl Schedule {
    /// A Session that allows background work in a release build checks at once, then every
    /// interval; any other Session never schedules a check.
    pub(super) fn new(background: bool, release: bool, now: Instant) -> Self {
        Self {
            background,
            release,
            due: (background && release).then_some(now),
            last: None,
        }
    }
    pub(super) fn project(&self, available: bool) -> AgentManifestAutomatic {
        AgentManifestAutomatic {
            background_enabled: self.background,
            release_build: self.release,
            update_available: available,
            enabled: self.background && self.release && available,
            last_operation: self.last.clone().map(Box::new),
        }
    }
    pub(super) fn admit(&mut self, now: Instant, available: bool) -> bool {
        if !available || self.due.is_none_or(|due| due > now) {
            return false;
        }
        self.due = now.checked_add(INTERVAL);
        self.last = Some(Operation::Pending {});
        true
    }
}

#[cfg(test)]
mod tests;
