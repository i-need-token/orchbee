//! Automatic checks share the maintenance slot but never consume manual retry receipts.
use super::*;
use orchbee_local_site_runtime::WindowsUserSettings;
use orchbee_workbench_protocol::{AgentManifestAutomatic, AgentManifestCheckSource};
use std::time::{Duration, Instant};
const INTERVAL: Duration = Duration::from_secs(30 * 60);
pub(super) struct Schedule {
    pub(super) path: Option<PathBuf>,
    pub(super) explicit: bool,
    check: bool,
    source: AgentManifestCheckSource,
    background: bool,
    release: bool,
    due: Option<Instant>,
    pub(super) last: Option<Operation>,
}
impl Schedule {
    pub(super) fn new(
        settings: &WindowsUserSettings,
        background: bool,
        release: bool,
        now: Instant,
    ) -> Self {
        let mut schedule = Self {
            path: settings.path.clone(),
            explicit: settings.explicit,
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
    pub(super) fn apply(&mut self, settings: &WindowsUserSettings, now: Instant) {
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
mod tests {
    use super::*;
    #[test]
    fn release_background_and_settings_gates_use_monotonic_intervals() {
        let now = Instant::now();
        for background in [false, true] {
            for release in [false, true] {
                for check in [false, true] {
                    let settings = WindowsUserSettings {
                        manifest_check: check,
                        manifest_check_source: "file",
                        ..WindowsUserSettings::default()
                    };
                    let mut schedule = Schedule::new(&settings, background, release, now);
                    assert!(!schedule.admit(now, false));
                    assert_eq!(schedule.admit(now, true), background && release && check);
                    assert!(!schedule.admit(now + INTERVAL - Duration::from_secs(1), true));
                    assert_eq!(
                        schedule.admit(now + INTERVAL, true),
                        background && release && check
                    );
                    let off = WindowsUserSettings {
                        manifest_check: false,
                        manifest_check_source: "file",
                        ..WindowsUserSettings::default()
                    };
                    schedule.apply(&off, now);
                    assert!(!schedule.admit(now + INTERVAL * 2, true));
                    let on = WindowsUserSettings::default();
                    schedule.apply(&on, now + INTERVAL * 2);
                    assert_eq!(
                        schedule.admit(now + INTERVAL * 2, true),
                        background && release
                    );
                    schedule.project(true).validate().unwrap();
                }
            }
        }
    }
}
