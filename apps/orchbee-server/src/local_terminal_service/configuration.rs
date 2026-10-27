//! One Session-lifetime configuration owner. Fallible candidate preparation precedes publication.
use orchbee_local_runtime::{
    SettingsDiagnosticDisposition, SettingsDiagnostics, UserSettings, UserSettingsLoad,
    read_user_settings_at,
};
use orchbee_workbench_protocol::{
    ConfigurationAction as Action, ConfigurationDiagnostic, ConfigurationDiagnosticDisposition,
    ConfigurationDiagnostics, ConfigurationFailure as Failure, ConfigurationOutcome as Outcome,
    ConfigurationRequest, ConfigurationResult, ConfigurationSnapshot, ContractError,
    MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED, ProtocolVersion, SessionId, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision,
};
use std::sync::Mutex;

#[derive(Default)]
pub(super) struct Registry(Mutex<Option<State>>);
pub(super) struct PreparedInitial(State);
struct State {
    startup: UserSettings,
    current: UserSettings,
    snapshot: ConfigurationSnapshot,
    reloading: bool,
}
struct FrozenState {
    startup: UserSettings,
    current: UserSettings,
    snapshot: ConfigurationSnapshot,
}
struct ReloadAdmission<'a> {
    registry: &'a Registry,
    active: bool,
}
impl ReloadAdmission<'_> {
    fn complete(mut self) {
        self.active = false;
    }
}
impl Drop for ReloadAdmission<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Ok(mut slot) = self.registry.0.lock()
            && let Some(state) = slot.as_mut()
        {
            state.reloading = false;
        }
    }
}
impl Registry {
    #[cfg(test)]
    pub(super) fn initialize(&self, settings: &UserSettings) -> std::io::Result<()> {
        self.initialize_load(
            settings,
            &UserSettingsLoad {
                settings: settings.clone(),
                diagnostics: SettingsDiagnostics::default(),
            },
        )
    }
    #[cfg(test)]
    pub(super) fn initialize_load(
        &self,
        startup: &UserSettings,
        current: &UserSettingsLoad,
    ) -> std::io::Result<()> {
        self.install_initial(Self::prepare_initial(startup, current)?)
    }
    pub(super) fn prepare_initial(
        startup: &UserSettings,
        current: &UserSettingsLoad,
    ) -> std::io::Result<PreparedInitial> {
        let diagnostics = protocol_diagnostics(&current.diagnostics).ok_or_else(|| {
            std::io::Error::other("Configuration diagnostics exceed protocol bounds")
        })?;
        let initial = snapshot(&current.settings, diagnostics, 0);
        preflight_publication(&Outcome::Current {
            snapshot: initial.clone(),
        })
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Configuration startup publication exceeds protocol bounds",
            )
        })?;
        Ok(PreparedInitial(State {
            startup: startup.clone(),
            current: current.settings.clone(),
            snapshot: initial,
            reloading: false,
        }))
    }
    pub(super) fn install_initial(&self, prepared: PreparedInitial) -> std::io::Result<()> {
        let mut slot = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("Configuration owner poisoned"))?;
        if slot.is_some() {
            return Err(std::io::ErrorKind::AlreadyExists.into());
        }
        *slot = Some(prepared.0);
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn initialized_for_test(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
    fn execute(
        &self,
        request: &ConfigurationRequest,
        apply: impl FnOnce(&UserSettings) -> bool,
    ) -> Outcome {
        self.execute_with(
            request,
            |path, explicit| read_user_settings_at(path, explicit, |name| std::env::var_os(name)),
            apply,
        )
    }

    fn execute_with(
        &self,
        request: &ConfigurationRequest,
        load: impl FnOnce(
            Option<&std::path::Path>,
            bool,
        ) -> Result<
            orchbee_local_runtime::UserSettingsLoad,
            orchbee_local_runtime::SettingsError,
        >,
        apply: impl FnOnce(&UserSettings) -> bool,
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
        if state.reloading {
            return Outcome::failed(Failure::Busy);
        }
        state.reloading = true;
        let frozen = FrozenState {
            startup: state.startup.clone(),
            current: state.current.clone(),
            snapshot: state.snapshot.clone(),
        };
        drop(slot);
        let admission = ReloadAdmission {
            registry: self,
            active: true,
        };
        // File I/O and TOML preparation run without the committed-state mutex.
        let candidate = match load(frozen.startup.path.as_deref(), frozen.startup.explicit) {
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
        let Some(diagnostics) = protocol_diagnostics(&candidate.diagnostics) else {
            return Outcome::failed(Failure::LoadFailed);
        };
        let changed = frozen.snapshot.revision.value() == 0
            || candidate.settings != frozen.current
            || diagnostics != frozen.snapshot.diagnostics;
        let next = if changed {
            let Some(revision) = frozen.snapshot.revision.value().checked_add(1) else {
                return Outcome::failed(Failure::RevisionExhausted);
            };
            snapshot(&candidate.settings, diagnostics, revision)
        } else {
            frozen.snapshot.clone()
        };
        // Prepare the exact bounded outbound representation before changing the live scheduler.
        if orchbee_local_runtime::KeySettings::from_profile(&next.keys_profile).is_err()
            || next.keys_profile.len() > 64 * 1024
            || orchbee_local_runtime::SpaceSidebarSettings::from_profile(
                &next.spaces_sidebar_profile,
            )
            .is_err()
            || orchbee_local_runtime::AgentSidebarSettings::from_profile(
                &next.agent_sidebar_profile,
            )
            .is_err()
            || orchbee_local_runtime::NotificationSettings::from_profile(&next.notification_profile)
                .is_err()
        {
            return Outcome::failed(Failure::LoadFailed);
        }
        let reply = Outcome::Reloaded {
            snapshot: next.clone(),
            changed,
        };
        if preflight_publication(&reply).is_err() {
            return Outcome::failed(Failure::LoadFailed);
        }
        let Ok(mut slot) = self.0.lock() else {
            return Outcome::failed(Failure::Unavailable);
        };
        let Some(state) = slot.as_mut() else {
            return Outcome::failed(Failure::Unavailable);
        };
        if !state.reloading || state.snapshot.revision != frozen.snapshot.revision {
            return Outcome::failed(Failure::Busy);
        }
        if !apply(&candidate.settings) {
            return Outcome::failed(Failure::Unavailable);
        }
        if changed {
            state.current = candidate.settings;
            state.snapshot = next;
        }
        state.reloading = false;
        admission.complete();
        reply
    }
}

fn publication_result(outcome: Outcome) -> ConfigurationResult {
    ConfigurationResult {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse("00000000000000000000000000000001")
            .expect("fixed configuration preflight request id"),
        session_id: SessionId::parse("00000000000000000000000000000002")
            .expect("fixed configuration preflight Session id"),
        outcome,
    }
}

/// Serialize the same bounded response shape used by Current and Reloaded before publication.
/// Correlation identifiers have a fixed legal width, so their values do not change the wire size.
fn preflight_publication(outcome: &Outcome) -> Result<(), ContractError> {
    orchbee_workbench_protocol::serialize_workbench_response(&WorkbenchResponse::Configuration(
        publication_result(outcome.clone()),
    ))
    .map(drop)
}

fn snapshot(
    current: &UserSettings,
    diagnostics: ConfigurationDiagnostics,
    revision: u64,
) -> ConfigurationSnapshot {
    ConfigurationSnapshot {
        revision: WorkbenchRevision::new(revision),
        diagnostics,
        keys_profile: current.keys.profile(),
        window_title: current.window_title.clone(),
        notification_profile: current.notifications.profile(),
        agent_sidebar_profile: current.agent_sidebar.profile(),
        spaces_sidebar_profile: current.space_sidebar.profile(),
        manifest_check: current.manifest_check,
    }
}

fn protocol_diagnostics(diagnostics: &SettingsDiagnostics) -> Option<ConfigurationDiagnostics> {
    let omitted_count = u32::try_from(diagnostics.omitted_count()).ok()?;
    if omitted_count > MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED {
        return None;
    }
    Some(ConfigurationDiagnostics {
        items: diagnostics
            .items()
            .iter()
            .map(|item| ConfigurationDiagnostic {
                field: item.field.clone(),
                reason: item.reason.into(),
                disposition: match item.disposition {
                    SettingsDiagnosticDisposition::Defaulted => {
                        ConfigurationDiagnosticDisposition::Defaulted
                    }
                    SettingsDiagnosticDisposition::Disabled => {
                        ConfigurationDiagnosticDisposition::Disabled
                    }
                    SettingsDiagnosticDisposition::Ignored => {
                        ConfigurationDiagnosticDisposition::Ignored
                    }
                },
            })
            .collect(),
        omitted_count,
    })
}
impl super::LocalTerminalService {
    pub(crate) fn configured_commands(
        &self,
    ) -> Result<
        (
            WorkbenchRevision,
            Vec<orchbee_local_runtime::CustomCommandEntry>,
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
            state.current.notifications.delivery == orchbee_local_runtime::NotificationDelivery::Off
        }))
    }
    pub(crate) fn window_title_configuration(&self) -> Result<String, ()> {
        let slot = self.configuration.0.try_lock().map_err(|_| ())?;
        Ok(slot
            .as_ref()
            .map_or_else(String::new, |state| state.current.window_title.clone()))
    }

    /// No fallible effect follows scheduler admission.
    pub(super) fn apply_configuration(&self, settings: &UserSettings) -> bool {
        self.process_observer.apply_configuration(settings)
    }
    pub(crate) fn execute_configuration(
        &self,
        request: ConfigurationRequest,
    ) -> ConfigurationResult {
        let outcome = match self.workbench_session_matches(&request.session_id) {
            None => Outcome::failed(Failure::Unavailable),
            Some(false) => Outcome::failed(Failure::SessionMismatch),
            Some(true) => self
                .configuration
                .execute(&request, |settings| self.apply_configuration(settings)),
        };
        ConfigurationResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
