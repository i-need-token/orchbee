//! A Client-owned publication watcher. Only the UI owner installs a validated key snapshot.
use crate::{
    client_view::{ClientView, Notice},
    local_client::{LocalClientOrigin, LocalOperationClock},
};
use orchbee_local_ipc::{AsLocalIo, LocalStream};
use orchbee_local_site_runtime::{
    WindowsAgentSidebarSettings, WindowsKeySettings, WindowsNotificationSettings,
    WindowsSpaceSidebarSettings,
};
use orchbee_workbench_protocol::{
    ConfigurationAction, ConfigurationOutcome, ConfigurationRequest, ConfigurationSnapshot,
    ProtocolVersion, SiteId, WorkbenchRequestId,
};
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Default)]
struct Handoff {
    stopped: bool,
    reload: bool,
    snapshot: Option<ConfigurationSnapshot>,
    notice: Option<PendingNotice>,
}
#[derive(Clone, Copy)]
struct PendingNotice {
    revision: Option<u64>,
    needs_application: bool,
    notice: Notice,
}
type Shared = Arc<(Mutex<Handoff>, Condvar)>;
pub(super) struct Watcher {
    shared: Shared,
    wake: LocalStream,
    task: Option<JoinHandle<()>>,
    revision: Option<u64>,
    applied_snapshot: Option<ConfigurationSnapshot>,
    notification_update: Option<(u64, WindowsNotificationSettings)>,
}
impl Watcher {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        site: SiteId,
    ) -> std::io::Result<Self> {
        let shared = Arc::new((Mutex::new(Handoff::default()), Condvar::new()));
        let (reader, wake) = LocalStream::pair()?;
        let state = Arc::clone(&shared);
        let task = thread::Builder::new()
            .name("orchbee-client-config".into())
            .spawn(move || run(state, origin, clock, site, reader))?;
        Ok(Self {
            shared,
            wake,
            task: Some(task),
            revision: None,
            applied_snapshot: None,
            notification_update: None,
        })
    }
    pub(super) fn reload(&self) -> bool {
        let Ok(mut state) = self.shared.0.lock() else {
            return false;
        };
        if state.stopped {
            return false;
        }
        state.reload = true;
        self.shared.1.notify_all();
        true
    }
    pub(super) fn tick(&mut self, view: &mut ClientView) -> bool {
        self.notification_update = None;
        let Ok(mut state) = self.shared.0.lock() else {
            return false;
        };
        let snapshot = state.snapshot.take();
        let notice = state.notice.take();
        drop(state);
        let mut changed = false;
        let mut applied_revision = None;
        let mut unchanged_revision = None;
        if let Some(snapshot) = snapshot {
            if self.applied_snapshot.as_ref() == Some(&snapshot) {
                unchanged_revision = Some(snapshot.revision.value());
            }
            if self
                .revision
                .is_none_or(|revision| snapshot.revision.value() > revision)
            {
                match (
                    WindowsKeySettings::from_profile(&snapshot.keys_profile),
                    WindowsNotificationSettings::from_profile(&snapshot.notification_profile),
                    WindowsAgentSidebarSettings::from_profile(&snapshot.agent_sidebar_profile),
                    WindowsSpaceSidebarSettings::from_profile(&snapshot.spaces_sidebar_profile),
                ) {
                    (Ok(keys), Ok(notifications), Ok(sidebar), Ok(spaces)) => {
                        view.machines.configure_space_sidebar(spaces);
                        view.machines.configure_agent_sidebar(sidebar);
                        view.notifications.configure(notifications.clone());
                        self.notification_update = Some((snapshot.revision.value(), notifications));
                        view.configure_keys(&keys);
                        view.configuration_warning(
                            !snapshot.diagnostics.items.is_empty()
                                || snapshot.diagnostics.omitted_count > 0,
                        );
                        self.revision = Some(snapshot.revision.value());
                        applied_revision = self.revision;
                        self.applied_snapshot = Some(snapshot);
                        changed = true;
                    }
                    _ => {
                        view.configuration_notice(Notice::ConfigurationFailed);
                        return true;
                    }
                }
            }
        }
        if let Some(notice) = notice
            && match notice.revision {
                None => true,
                Some(revision) if notice.needs_application => applied_revision == Some(revision),
                Some(revision) => {
                    applied_revision == Some(revision) || unchanged_revision == Some(revision)
                }
            }
        {
            view.configuration_notice(notice.notice);
            changed = true;
        }
        changed
    }
    pub(super) fn take_notification_update(
        &mut self,
    ) -> Option<(u64, WindowsNotificationSettings)> {
        self.notification_update.take()
    }
    pub(super) fn close(&mut self) {
        if self.task.is_none() {
            return;
        }
        if let Ok(mut state) = self.shared.0.lock() {
            state.stopped = true;
        }
        self.shared.1.notify_all();
        let _ = self.wake.shutdown(std::net::Shutdown::Both);
        if let Some(task) = self.task.take() {
            let _ = task.join();
        }
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        self.close();
    }
}

fn run(
    shared: Shared,
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    site: SiteId,
    wake: LocalStream,
) {
    let mut client = None;
    let mut serial = 0_u64;
    loop {
        let reload = {
            let Ok(mut state) = shared.0.lock() else {
                return;
            };
            if state.stopped {
                return;
            }
            std::mem::take(&mut state.reload)
        };
        if client.is_none() {
            client = origin
                .reconnect_before_cancellable(
                    clock.now() + Duration::from_secs(2),
                    clock.clone(),
                    Some(wake.as_local_io()),
                )
                .ok();
        }
        let Some(next) = serial.checked_add(1) else {
            return;
        };
        serial = next;
        let request = ConfigurationRequest {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{serial:032x}"))
                .expect("bounded request id"),
            site_id: site.clone(),
            action: if reload {
                ConfigurationAction::Reload
            } else {
                ConfigurationAction::Current
            },
        };
        let result = client.as_mut().map(|client| {
            client.configuration(
                request,
                clock.now() + Duration::from_secs(2),
                Some(wake.as_local_io()),
            )
        });
        let mut submitted = false;
        let outcome = match result {
            Some(Ok(reply)) => Some(reply.outcome),
            Some(Err(error)) => {
                submitted = error.submitted;
                client = None;
                None
            }
            None => None,
        };
        let Ok(mut state) = shared.0.lock() else {
            return;
        };
        if state.stopped {
            return;
        }
        match outcome {
            Some(ConfigurationOutcome::Current { snapshot }) => {
                state.snapshot = Some(snapshot);
            }
            Some(ConfigurationOutcome::Reloaded { snapshot, changed }) => {
                if reload {
                    let warning = !snapshot.diagnostics.items.is_empty()
                        || snapshot.diagnostics.omitted_count > 0;
                    let notice = if snapshot.restart_required.is_empty() {
                        (!warning).then_some(Notice::ConfigurationPublished)
                    } else {
                        Some(Notice::ConfigurationRestart)
                    };
                    state.notice = notice.map(|notice| PendingNotice {
                        revision: Some(snapshot.revision.value()),
                        needs_application: changed,
                        notice,
                    });
                }
                state.snapshot = Some(snapshot);
            }
            _ if reload => {
                state.notice = Some(PendingNotice {
                    revision: None,
                    needs_application: false,
                    notice: if submitted {
                        Notice::ConfigurationUnknown
                    } else {
                        Notice::ConfigurationFailed
                    },
                });
            }
            _ => {}
        }
        if !state.reload
            && let Ok((next, _)) = shared.1.wait_timeout(state, Duration::from_millis(500))
            && next.stopped
        {
            return;
        }
    }
}

#[cfg(test)]
mod tests;
