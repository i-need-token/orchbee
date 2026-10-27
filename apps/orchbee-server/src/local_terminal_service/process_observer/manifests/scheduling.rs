//! Automatic checks share the maintenance slot but never consume manual retry receipts.
use super::Operation;
use orchbee_local_runtime::UserSettings;
use orchbee_workbench_protocol::{AgentManifestAutomatic, AgentManifestCheckSource};
use std::time::{Duration, Instant};
const INTERVAL: Duration = Duration::from_secs(30 * 60);
pub(super) struct Schedule {
    check: bool,
    source: AgentManifestCheckSource,
    background: bool,
    release: bool,
    due: Option<Instant>,
    pub(super) last: Option<Operation>,
}
impl Schedule {
    pub(super) fn new(
        settings: &UserSettings,
        background: bool,
        release: bool,
        now: Instant,
    ) -> Self {
        let mut schedule = Self {
            check: false,
            source: AgentManifestCheckSource::Default,
            background,
            release,
            due: None,
            last: None,
        };
        schedule.apply(settings, now);
        schedule
    }
    pub(super) fn apply(&mut self, settings: &UserSettings, now: Instant) {
        let before = self.check;
        self.check = settings.manifest_check;
        self.source = if settings.manifest_check_source == "file" {
            AgentManifestCheckSource::File
        } else {
            AgentManifestCheckSource::Default
        };
        if !self.check || !self.background || !self.release {
            self.due = None;
        } else if !before {
            self.due = Some(now);
        }
    }
    pub(super) fn project(&self, available: bool) -> AgentManifestAutomatic {
        AgentManifestAutomatic {
            manifest_check: self.check,
            manifest_check_source: self.source,
            background_enabled: self.background,
            release_build: self.release,
            update_available: available,
            enabled: self.check && self.background && self.release && available,
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
