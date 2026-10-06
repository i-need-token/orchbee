//! Settings shares the configuration admission; disk and live outcomes remain separate.
use super::{FrozenState, Registry, ReloadAdmission, protocol_diagnostics, snapshot};
use orchbee_local_site_runtime::{
    WindowsAgentSidebarSettings, WindowsKeySettings, WindowsNativeSettingsChanges,
    WindowsNativeSettingsFailure, WindowsNotificationSettings, WindowsSettingsSource,
    WindowsSpaceSidebarSettings, WindowsThemeName, WindowsUserSettings, WindowsUserSettingsLoad,
    prepare_windows_native_settings, read_windows_native_settings,
};
use orchbee_workbench_protocol::{
    ConfigurationSnapshot, ConfigurationThemeName, NativeSettingsFailure as Failure,
    NativeSettingsOperation as Operation, NativeSettingsOutcome as Outcome, NativeSettingsRequest,
    NativeSettingsResult, NativeSettingsSource, ProtocolVersion, WorkbenchResponse,
};
#[cfg(test)]
mod tests;

fn refused(reason: Failure) -> Outcome {
    Outcome::Refused { reason }
}
fn storage_failure(reason: WindowsNativeSettingsFailure) -> Outcome {
    refused(match reason {
        WindowsNativeSettingsFailure::SourceChanged => Failure::SourceChanged,
        WindowsNativeSettingsFailure::InvalidSource => Failure::InvalidSource,
        WindowsNativeSettingsFailure::InvalidChanges => Failure::InvalidChanges,
        WindowsNativeSettingsFailure::WriteFailed => Failure::WriteFailed,
    })
}
fn baseline_matches(frozen: &FrozenState, load: &WindowsUserSettingsLoad) -> bool {
    load.settings == frozen.current
        && protocol_diagnostics(&load.diagnostics).as_ref() == Some(&frozen.snapshot.diagnostics)
}
fn profiles_admitted(next: &ConfigurationSnapshot) -> bool {
    WindowsKeySettings::from_profile(&next.keys_profile).is_ok()
        && WindowsNotificationSettings::from_profile(&next.notification_profile).is_ok()
        && WindowsAgentSidebarSettings::from_profile(&next.agent_sidebar_profile).is_ok()
        && WindowsSpaceSidebarSettings::from_profile(&next.spaces_sidebar_profile).is_ok()
}
fn preflight(request: &NativeSettingsRequest, outcome: &Outcome) -> bool {
    orchbee_workbench_protocol::serialize_workbench_response(&WorkbenchResponse::Settings(
        NativeSettingsResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            site_id: request.site_id.clone(),
            outcome: outcome.clone(),
        },
    ))
    .is_ok()
}

impl Registry {
    fn execute_settings(
        &self,
        request: &NativeSettingsRequest,
        apply: impl FnOnce(&WindowsUserSettings) -> bool,
    ) -> Outcome {
        let Ok(mut slot) = self.0.try_lock() else {
            return refused(Failure::Busy);
        };
        let Some(state) = slot.as_mut() else {
            return refused(Failure::Unavailable);
        };
        if state.reloading {
            return refused(Failure::Busy);
        }
        let saving = match &request.operation {
            Operation::Open {} => false,
            Operation::Save {
                expected_revision,
                changes,
                ..
            } => {
                if changes.is_empty() {
                    return refused(Failure::InvalidChanges);
                }
                if *expected_revision != state.snapshot.revision {
                    return refused(Failure::ConfigurationChanged);
                }
                state.reloading = true;
                true
            }
        };
        let frozen = FrozenState {
            startup: state.startup.clone(),
            current: state.current.clone(),
            snapshot: state.snapshot.clone(),
        };
        drop(slot);
        let admission = ReloadAdmission {
            registry: self,
            active: saving,
        };
        let Some(path) = frozen.startup.path.as_deref() else {
            return refused(Failure::Unavailable);
        };
        if matches!(request.operation, Operation::Open {}) {
            let observed =
                match read_windows_native_settings(path, frozen.startup.explicit, |name| {
                    std::env::var_os(name)
                }) {
                    Ok(observed) => observed,
                    Err(error) => return storage_failure(error),
                };
            if !baseline_matches(&frozen, &observed.load) {
                return refused(Failure::ConfigurationChanged);
            }
            let Ok(slot) = self.0.lock() else {
                return refused(Failure::Unavailable);
            };
            if slot.as_ref().is_none_or(|state| {
                state.reloading || state.snapshot.revision != frozen.snapshot.revision
            }) {
                return refused(Failure::ConfigurationChanged);
            }
            let Ok(source) = NativeSettingsSource::parse(observed.source.as_str()) else {
                return refused(Failure::InvalidSource);
            };
            let reply = Outcome::Opened {
                source,
                snapshot: frozen.snapshot,
            };
            return if preflight(request, &reply) {
                reply
            } else {
                refused(Failure::ResponseTooLarge)
            };
        }
        let Operation::Save {
            source, changes, ..
        } = &request.operation
        else {
            return refused(Failure::InvalidChanges);
        };
        let Some(source) = WindowsSettingsSource::parse(source.as_str()) else {
            return refused(Failure::InvalidSource);
        };
        let prepared = match prepare_windows_native_settings(
            path,
            frozen.startup.explicit,
            &source,
            WindowsNativeSettingsChanges {
                theme: changes.theme_name.map(|theme| match theme {
                    ConfigurationThemeName::TerminalNative => WindowsThemeName::TerminalNative,
                    ConfigurationThemeName::OneLight => WindowsThemeName::OneLight,
                    ConfigurationThemeName::OneDark => WindowsThemeName::OneDark,
                }),
                background_reminders: changes.background_reminders,
                sound_enabled: changes.sound_enabled,
            },
            |name| std::env::var_os(name),
        ) {
            Ok(prepared) => prepared,
            Err(error) => return storage_failure(error),
        };
        if !baseline_matches(&frozen, prepared.previous()) {
            return refused(Failure::ConfigurationChanged);
        }
        let no_change = Outcome::NoChange {
            snapshot: frozen.snapshot.clone(),
        };
        if !prepared.changed() {
            if !preflight(request, &no_change) {
                return refused(Failure::ResponseTooLarge);
            }
            return match prepared.commit() {
                Ok(false) => no_change,
                Ok(true) => refused(Failure::WriteFailed),
                Err(error) => storage_failure(error),
            };
        }
        let Some(revision) = frozen.snapshot.revision.value().checked_add(1) else {
            return refused(Failure::RevisionExhausted);
        };
        let Some(diagnostics) = protocol_diagnostics(&prepared.candidate().diagnostics) else {
            return refused(Failure::InvalidChanges);
        };
        let candidate = prepared.candidate().settings.clone();
        let next = snapshot(&frozen.startup, &candidate, diagnostics, revision);
        if !profiles_admitted(&next) {
            return refused(Failure::InvalidChanges);
        }
        let success = Outcome::SavedApplied {
            snapshot: next.clone(),
        };
        let not_applied = Outcome::SavedNotApplied {
            snapshot: frozen.snapshot.clone(),
        };
        if !preflight(request, &success)
            || !preflight(request, &not_applied)
            || !preflight(request, &no_change)
        {
            return refused(Failure::ResponseTooLarge);
        }
        // The admission still excludes Reload while I/O runs without the committed-state mutex.
        {
            let Ok(slot) = self.0.lock() else {
                return refused(Failure::Unavailable);
            };
            if slot.as_ref().is_none_or(|state| {
                !state.reloading || state.snapshot.revision != frozen.snapshot.revision
            }) {
                return refused(Failure::ConfigurationChanged);
            }
        }
        match prepared.commit() {
            Ok(true) => {}
            Ok(false) => return no_change,
            Err(error) => return storage_failure(error),
        }
        // Every outcome below reports the completed disk save, including poisoned/live admission.
        let Ok(mut slot) = self.0.lock() else {
            return not_applied;
        };
        let Some(state) = slot.as_mut() else {
            return not_applied;
        };
        if !state.reloading
            || state.snapshot.revision != frozen.snapshot.revision
            || !apply(&candidate)
        {
            return not_applied;
        }
        state.current = candidate;
        state.snapshot = next;
        state.reloading = false;
        admission.complete();
        success
    }
}

impl super::super::LocalTerminalService {
    /// Native settings uses the matched Site and the existing sole live-configuration owner.
    pub(crate) fn execute_settings(&self, request: NativeSettingsRequest) -> NativeSettingsResult {
        let outcome = match self.workbench_site_matches(&request.site_id) {
            None => refused(Failure::Unavailable),
            Some(false) => refused(Failure::SiteMismatch),
            Some(true) => self
                .configuration
                .execute_settings(&request, |settings| self.apply_configuration(settings)),
        };
        NativeSettingsResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            site_id: request.site_id,
            outcome,
        }
    }
}
