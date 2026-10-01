//! One Site-lifetime configuration owner. Fallible candidate preparation precedes publication.
use orchbee_local_site_runtime::{WindowsUserSettings, read_windows_user_settings_at};
use orchbee_workbench_protocol::{
    ConfigurationAction as Action, ConfigurationFailure as Failure,
    ConfigurationOutcome as Outcome, ConfigurationRequest, ConfigurationRestart as Restart,
    ConfigurationResult, ConfigurationShellMode, ConfigurationSnapshot, ProtocolVersion,
    WorkbenchRevision,
};
use std::sync::Mutex;

#[derive(Default)]
pub(super) struct Registry(Mutex<Option<State>>);
struct State {
    startup: WindowsUserSettings,
    current: WindowsUserSettings,
    snapshot: ConfigurationSnapshot,
}
impl Registry {
    pub(super) fn initialize(&self, settings: &WindowsUserSettings) -> std::io::Result<()> {
        let mut slot = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("Configuration owner poisoned"))?;
        if slot.is_some() {
            return Err(std::io::ErrorKind::AlreadyExists.into());
        }
        *slot = Some(State {
            startup: settings.clone(),
            current: settings.clone(),
            snapshot: snapshot(settings, settings, 0),
        });
        Ok(())
    }
    fn execute(
        &self,
        request: &ConfigurationRequest,
        apply: impl FnOnce(&WindowsUserSettings) -> bool,
    ) -> Outcome {
        let Ok(mut slot) = self.0.try_lock() else {
            return Outcome::failed(Failure::Busy);
        };
        let Some(state) = slot.as_mut() else {
            return Outcome::failed(Failure::Unavailable);
        };
        if request.action == Action::Current {
            return Outcome::Current {
                snapshot: state.snapshot.clone(),
            };
        }
        let candidate = match read_windows_user_settings_at(
            state.startup.path.as_deref(),
            state.startup.explicit,
            |name| std::env::var_os(name),
        ) {
            Ok(candidate) => candidate,
            Err(error) => {
                return Outcome::Failed {
                    reason: Failure::LoadFailed,
                    field: Some(error.field.into()),
                    detail: Some(error.reason.into()),
                };
            }
        };
        // The first explicit publication replaces Client startup-only keys, even if otherwise equal.
        let changed = state.snapshot.revision.value() == 0 || candidate != state.current;
        if !changed {
            return if apply(&candidate) {
                Outcome::Reloaded {
                    snapshot: state.snapshot.clone(),
                    changed: false,
                }
            } else {
                Outcome::failed(Failure::Unavailable)
            };
        }
        let Some(revision) = state.snapshot.revision.value().checked_add(1) else {
            return Outcome::failed(Failure::RevisionExhausted);
        };
        let next = snapshot(&state.startup, &candidate, revision);
        // Prepare the exact bounded outbound representation before changing the live scheduler.
        if orchbee_local_site_runtime::WindowsKeySettings::from_profile(&next.keys_profile).is_err()
            || next.keys_profile.len() > 64 * 1024
            || orchbee_local_site_runtime::WindowsSpaceSidebarSettings::from_profile(
                &next.spaces_sidebar_profile,
            )
            .is_err()
            || orchbee_local_site_runtime::WindowsAgentSidebarSettings::from_profile(
                &next.agent_sidebar_profile,
            )
            .is_err()
        {
            return Outcome::failed(Failure::LoadFailed);
        }
        let reply = Outcome::Reloaded {
            snapshot: next.clone(),
            changed: true,
        };
        let prepared = ConfigurationResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome: reply.clone(),
        };
        if orchbee_workbench_protocol::serialize_workbench_response(
            &orchbee_workbench_protocol::WorkbenchResponse::Configuration(prepared),
        )
        .is_err()
        {
            return Outcome::failed(Failure::LoadFailed);
        }
        if !apply(&candidate) {
            return Outcome::failed(Failure::Unavailable);
        }
        state.current = candidate;
        state.snapshot = next;
        reply
    }
}
fn snapshot(
    startup: &WindowsUserSettings,
    current: &WindowsUserSettings,
    revision: u64,
) -> ConfigurationSnapshot {
    let mut restart_required = Vec::new();
    if startup.resume_agents_on_restore != current.resume_agents_on_restore {
        restart_required.push(Restart::ResumeAgents);
    }
    ConfigurationSnapshot {
        revision: WorkbenchRevision::new(revision),
        keys_profile: current.keys.profile(),
        window_title: current.window_title.clone(),
        notification_profile: current.notifications.profile(),
        agent_sidebar_profile: current.agent_sidebar.profile(),
        spaces_sidebar_profile: current.space_sidebar.profile(),
        manifest_check: current.manifest_check,
        default_shell: current.shell.executable().into(),
        shell_mode: match current.shell.mode() {
            orchbee_local_site_runtime::WindowsShellMode::Auto => ConfigurationShellMode::Auto,
            orchbee_local_site_runtime::WindowsShellMode::Login => ConfigurationShellMode::Login,
            orchbee_local_site_runtime::WindowsShellMode::NonLogin => {
                ConfigurationShellMode::NonLogin
            }
        },
        restart_required,
    }
}
impl super::LocalTerminalService {
    pub(crate) fn configured_commands(
        &self,
    ) -> Result<
        (
            WorkbenchRevision,
            Vec<orchbee_local_site_runtime::WindowsCustomCommandEntry>,
        ),
        orchbee_workbench_protocol::ConfiguredCommandFailure,
    > {
        let slot = self
            .configuration
            .0
            .try_lock()
            .map_err(|_| orchbee_workbench_protocol::ConfiguredCommandFailure::Busy)?;
        let state = slot
            .as_ref()
            .ok_or(orchbee_workbench_protocol::ConfiguredCommandFailure::Unavailable)?;
        Ok((
            state.snapshot.revision,
            state.current.keys.command_catalog(),
        ))
    }
    pub(crate) fn notifications_disabled(&self) -> Result<bool, ()> {
        let slot = self.configuration.0.try_lock().map_err(|_| ())?;
        Ok(slot.as_ref().is_some_and(|state| {
            state.current.notifications.delivery
                == orchbee_local_site_runtime::WindowsNotificationDelivery::Off
        }))
    }
    pub(crate) fn window_title_configuration(&self) -> Result<String, ()> {
        let slot = self.configuration.0.try_lock().map_err(|_| ())?;
        Ok(slot
            .as_ref()
            .map_or_else(String::new, |state| state.current.window_title.clone()))
    }

    /// No fallible effect follows scheduler admission. Creation and reload share this lock.
    pub(super) fn apply_configuration(&self, settings: &WindowsUserSettings) -> bool {
        use orchbee_capability_node_protocol::{DefaultShellMode, DefaultShellSelection};
        use orchbee_local_site_runtime::WindowsShellMode;
        let mode = match settings.shell.mode() {
            WindowsShellMode::Auto => DefaultShellMode::Auto,
            WindowsShellMode::Login => DefaultShellMode::Login,
            WindowsShellMode::NonLogin => DefaultShellMode::NonLogin,
        };
        let Ok(shell) = DefaultShellSelection::new(settings.shell.executable().into(), mode) else {
            return false;
        };
        let Ok(mut coordinator) = self.coordinator.lock() else {
            return false;
        };
        if !self.process_observer.apply_configuration(settings) {
            return false;
        }
        coordinator.set_default_shell(shell);
        true
    }
    pub(crate) fn execute_configuration(
        &self,
        request: ConfigurationRequest,
    ) -> ConfigurationResult {
        let outcome = match self.workbench_site_matches(&request.site_id) {
            None => Outcome::failed(Failure::Unavailable),
            Some(false) => Outcome::failed(Failure::SiteMismatch),
            Some(true) => self
                .configuration
                .execute(&request, |settings| self.apply_configuration(settings)),
        };
        ConfigurationResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome,
        }
    }
}

#[cfg(test)]
mod tests;
