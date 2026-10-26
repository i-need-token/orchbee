//! A Client-owned publication watcher. Only the UI owner installs a validated key snapshot.
use crate::{
    client_view::{ClientView, Notice},
    local_client::{LocalClientOrigin, LocalOperationClock},
};
use orchbee_local_runtime::{
    AgentSidebarSettings, KeySettings, NotificationSettings, SpaceSidebarSettings,
};
use orchbee_workbench_protocol::{
    ConfigurationAction, ConfigurationOutcome, ConfigurationRequest, ConfigurationSnapshot,
    ProtocolVersion, SessionId, WorkbenchRequestId,
};
use std::{
    sync::{Arc, Condvar, Mutex},
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
/// Complete profile admission shared by draft extraction and the sole live applier.
pub(super) struct Admitted {
    keys: KeySettings,
    agents: AgentSidebarSettings,
    spaces: SpaceSidebarSettings,
    pub(super) notifications: NotificationSettings,
}
pub(super) fn admit(snapshot: &ConfigurationSnapshot) -> Option<Admitted> {
    Some(Admitted {
        keys: KeySettings::from_profile(&snapshot.keys_profile).ok()?,
        notifications: NotificationSettings::from_profile(&snapshot.notification_profile).ok()?,
        agents: AgentSidebarSettings::from_profile(&snapshot.agent_sidebar_profile).ok()?,
        spaces: SpaceSidebarSettings::from_profile(&snapshot.spaces_sidebar_profile).ok()?,
    })
}
fn queue_snapshot(state: &mut Handoff, snapshot: ConfigurationSnapshot) {
    if state
        .snapshot
        .as_ref()
        .is_none_or(|pending| snapshot.revision > pending.revision)
    {
        state.snapshot = Some(snapshot);
    }
}
pub(super) struct Watcher {
    shared: Shared,
    connection: Mutex<Option<Connection>>,
    revision: Option<u64>,
    applied_snapshot: Option<ConfigurationSnapshot>,
    notification_update: Option<(u64, NotificationSettings)>,
}
struct Connection {
    origin: LocalClientOrigin,
    clock: LocalOperationClock,
    session: SessionId,
    current_id: WorkbenchRequestId,
    subscription: crate::local_client::multiplex::Subscription,
    serial: u64,
    pending: Option<(
        WorkbenchRequestId,
        crate::local_client::subscription::Request,
    )>,
}
impl Watcher {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        clock: LocalOperationClock,
        session: SessionId,
    ) -> std::io::Result<Self> {
        let current_id = WorkbenchRequestId::parse("00000000000000000000000000000001")
            .expect("nonzero identity");
        let subscription = origin.subscribe(
            &orchbee_workbench_protocol::WorkbenchRequest::Configuration(ConfigurationRequest {
                protocol_version: ProtocolVersion,
                request_id: current_id.clone(),
                session_id: session.clone(),
                action: ConfigurationAction::Current,
            }),
            orchbee_workbench_protocol::StreamMode::State,
        )?;
        Ok(Self {
            shared: Arc::new((Mutex::default(), Condvar::new())),
            connection: Mutex::new(Some(Connection {
                origin,
                clock,
                session,
                current_id,
                subscription,
                serial: 1,
                pending: None,
            })),
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
        drop(state);
        self.poll_connection();
        true
    }
    pub(super) fn tick(&mut self, view: &mut ClientView) -> bool {
        self.poll_connection();
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
                match admit(&snapshot) {
                    Some(Admitted {
                        keys,
                        notifications,
                        agents: sidebar,
                        spaces,
                    }) => {
                        view.configure_theme(match snapshot.theme_name {
                            orchbee_workbench_protocol::ConfigurationThemeName::TerminalNative => {
                                orchbee_local_runtime::ThemeName::TerminalNative
                            }
                            orchbee_workbench_protocol::ConfigurationThemeName::OneLight => {
                                orchbee_local_runtime::ThemeName::OneLight
                            }
                            orchbee_workbench_protocol::ConfigurationThemeName::OneDark => {
                                orchbee_local_runtime::ThemeName::OneDark
                            }
                        });
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
                    None => {
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
    pub(super) fn take_notification_update(&mut self) -> Option<(u64, NotificationSettings)> {
        self.notification_update.take()
    }
    pub(super) fn close(&mut self) {
        if let Ok(mut state) = self.shared.0.lock() {
            state.stopped = true;
        }
        self.connection
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
    }
    fn poll_connection(&self) {
        use crate::local_client::multiplex::StreamEvent;
        use orchbee_workbench_protocol::{
            WorkbenchRequest, WorkbenchResponse, read_workbench_response,
        };
        let Ok(mut connection) = self.connection.lock() else {
            return;
        };
        let Some(c) = connection.as_mut() else {
            return;
        };
        let reload = self.shared.0.lock().is_ok_and(|mut state| {
            !state.stopped && c.pending.is_none() && std::mem::take(&mut state.reload)
        });
        if reload {
            let Some(serial) = c.serial.checked_add(1) else {
                return;
            };
            c.serial = serial;
            let id = WorkbenchRequestId::parse(&format!("{serial:032x}")).expect("nonzero serial");
            let request = WorkbenchRequest::Configuration(ConfigurationRequest {
                protocol_version: ProtocolVersion,
                request_id: id.clone(),
                session_id: c.session.clone(),
                action: ConfigurationAction::Reload,
            });
            match c.origin.request(
                &request,
                c.clock.now() + Duration::from_secs(2),
                c.clock.clone(),
            ) {
                Ok(request) => c.pending = Some((id, request)),
                Err(_) => publish_outcome(&self.shared, None, true, false),
            }
        }
        if let Some((expected, pending)) = &c.pending
            && let Some(reply) = pending.poll()
        {
            let (outcome, submitted) = match reply {
                Ok(WorkbenchResponse::Configuration(reply))
                    if reply.request_id == *expected && reply.session_id == c.session =>
                {
                    (Some(reply.outcome), false)
                }
                Ok(_) => (None, true),
                Err(submitted) => (None, submitted),
            };
            publish_outcome(&self.shared, outcome, true, submitted);
            c.pending = None;
        }
        loop {
            match c.subscription.poll() {
                Ok(Some(StreamEvent::Record(bytes))) => {
                    if let Ok(WorkbenchResponse::Configuration(reply)) =
                        read_workbench_response(&mut std::io::Cursor::new(bytes))
                        && reply.request_id == c.current_id
                        && reply.session_id == c.session
                    {
                        publish_outcome(&self.shared, Some(reply.outcome), false, false);
                    }
                }
                Ok(Some(StreamEvent::Gap | StreamEvent::Disconnected)) => {}
                Ok(None) | Err(_) => break,
            }
        }
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        self.close();
    }
}

fn publish_outcome(
    shared: &Shared,
    outcome: Option<ConfigurationOutcome>,
    reload: bool,
    submitted: bool,
) {
    let Ok(mut state) = shared.0.lock() else {
        return;
    };
    if state.stopped {
        return;
    }
    match outcome {
        Some(ConfigurationOutcome::Current { snapshot }) => {
            queue_snapshot(&mut state, snapshot);
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
            queue_snapshot(&mut state, snapshot);
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
}

#[cfg(test)]
mod tests;
