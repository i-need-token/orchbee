//! One Session-lifetime configuration owner. Fallible candidate preparation precedes publication.
use orchbee_local_runtime::{
    DirectoryPolicy, SettingsDiagnosticDisposition, SettingsDiagnostics, UserSettings,
    UserSettingsLoad, read_user_settings_at,
};
use orchbee_workbench_protocol::{
    ConfigurationAction as Action, ConfigurationDiagnostic, ConfigurationDiagnosticDisposition,
    ConfigurationDiagnostics, ConfigurationFailure as Failure, ConfigurationOutcome as Outcome,
    ConfigurationRequest, ConfigurationResult, ConfigurationSnapshot, ContractError,
    MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED, ProtocolVersion, SessionId, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision,
};
use std::{
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant, SystemTime},
};

/// The single sampler checks the frozen file at most this often.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(super) struct Registry {
    state: Mutex<Option<State>>,
    /// Sampled publications have no requesting connection; this wakes every subscriber.
    changes: orchbee_server::SessionChangeSource,
    preferences_unreadable: std::sync::atomic::AtomicBool,
}
pub(super) struct PreparedInitial(State);
/// `current` also carries the frozen startup location: every read at that location reports it.
struct State {
    current: UserSettings,
    snapshot: ConfigurationSnapshot,
    reloading: bool,
}
struct FrozenState {
    current: UserSettings,
    snapshot: ConfigurationSnapshot,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Publication {
    /// The first explicit publication replaces Client startup-only keys, even if otherwise equal.
    Explicit,
    /// A changed file publishes only a changed candidate and never reapplies an equal one.
    Sampled,
}
/// Metadata that changes whenever an editor replaces or rewrites the file.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Observation {
    Missing,
    File {
        length: u64,
        modified: Option<SystemTime>,
    },
    Unreadable,
}
impl Observation {
    fn of(path: &std::path::Path) -> Self {
        match std::fs::metadata(path) {
            Ok(metadata) => Self::File {
                length: metadata.len(),
                modified: metadata.modified().ok(),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::Missing,
            Err(_) => Self::Unreadable,
        }
    }
}
/// Owned by the process observer thread, so sampling needs no lock of its own.
#[derive(Default)]
pub(super) struct Sampler {
    observed: Option<Observation>,
    due: Option<Instant>,
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
        if let Ok(mut slot) = self.registry.state.lock()
            && let Some(state) = slot.as_mut()
        {
            state.reloading = false;
        }
    }
}
impl Registry {
    #[cfg(all(test, windows))]
    pub(super) fn initialize(&self, settings: &UserSettings) -> std::io::Result<()> {
        self.initialize_load(&UserSettingsLoad {
            settings: settings.clone(),
            diagnostics: SettingsDiagnostics::default(),
            syntax: None,
        })
    }
    #[cfg(all(test, windows))]
    pub(super) fn initialize_load(&self, current: &UserSettingsLoad) -> std::io::Result<()> {
        self.install_initial(Self::prepare_initial(current)?)
    }
    pub(super) fn prepare_initial(current: &UserSettingsLoad) -> std::io::Result<PreparedInitial> {
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
            current: current.settings.clone(),
            snapshot: initial,
            reloading: false,
        }))
    }
    pub(super) fn install_initial(&self, mut prepared: PreparedInitial) -> std::io::Result<()> {
        let mut slot = self
            .state
            .lock()
            .map_err(|_| std::io::Error::other("Configuration owner poisoned"))?;
        if slot.is_some() {
            return Err(std::io::ErrorKind::AlreadyExists.into());
        }
        if self
            .preferences_unreadable
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            retain_preferences_notice(&mut prepared.0.snapshot.diagnostics);
        }
        *slot = Some(prepared.0);
        Ok(())
    }
    /// One historical Session fact, not one warning per terminal creation. Readable Settings
    /// and an explicit reset remain authoritative for current preference values.
    pub(super) fn note_unreadable_preferences(&self) {
        let Ok(mut slot) = self.state.lock() else {
            return;
        };
        if self
            .preferences_unreadable
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        if let Some(state) = slot.as_mut() {
            let Some(revision) = state.snapshot.revision.value().checked_add(1) else {
                return;
            };
            let mut next = state.snapshot.clone();
            retain_preferences_notice(&mut next.diagnostics);
            next.revision = WorkbenchRevision::new(revision);
            if preflight_publication(&Outcome::Current {
                snapshot: next.clone(),
            })
            .is_err()
            {
                return;
            }
            state.snapshot = next;
            self.changes.notify();
        }
    }
    #[cfg(all(test, windows))]
    pub(super) fn initialized_for_test(&self) -> bool {
        self.state.lock().unwrap().is_some()
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
        if request.action == Action::Current {
            // Reloads read the file outside the lock, so a current read only waits for a commit.
            let Ok(slot) = self.state.lock() else {
                return Outcome::failed(Failure::Unavailable);
            };
            return slot
                .as_ref()
                .map_or(Outcome::failed(Failure::Unavailable), |state| {
                    Outcome::Current {
                        snapshot: state.snapshot.clone(),
                    }
                });
        }
        self.reload_with(Publication::Explicit, load, apply)
    }

    fn reload_with(
        &self,
        publication: Publication,
        load: impl FnOnce(
            Option<&std::path::Path>,
            bool,
        ) -> Result<
            orchbee_local_runtime::UserSettingsLoad,
            orchbee_local_runtime::SettingsError,
        >,
        apply: impl FnOnce(&UserSettings) -> bool,
    ) -> Outcome {
        let Ok(mut slot) = self.state.try_lock() else {
            return Outcome::failed(Failure::Busy);
        };
        let Some(state) = slot.as_mut() else {
            return Outcome::failed(Failure::Unavailable);
        };
        if state.reloading {
            return Outcome::failed(Failure::Busy);
        }
        state.reloading = true;
        let frozen = FrozenState {
            current: state.current.clone(),
            snapshot: state.snapshot.clone(),
        };
        drop(slot);
        let admission = ReloadAdmission {
            registry: self,
            active: true,
        };
        // File I/O and TOML preparation run without the committed-state mutex. A file that is not
        // valid TOML fails the reload, so the committed settings stay until the file changes.
        let candidate = match load(frozen.current.path.as_deref(), frozen.current.explicit)
            .and_then(UserSettingsLoad::require_valid_syntax)
        {
            Ok(candidate) => candidate,
            Err(error) => {
                return Outcome::Failed {
                    reason: Failure::LoadFailed,
                    field: Some(error.field.into()),
                    detail: Some(error.reason.into()),
                };
            }
        };
        let Some(mut diagnostics) = protocol_diagnostics(&candidate.diagnostics) else {
            return Outcome::failed(Failure::LoadFailed);
        };
        if self
            .preferences_unreadable
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            retain_preferences_notice(&mut diagnostics);
        }
        let changed = (publication == Publication::Explicit
            && frozen.snapshot.revision.value() == 0)
            || candidate.settings != frozen.current
            || diagnostics != frozen.snapshot.diagnostics;
        if !changed && publication == Publication::Sampled {
            return Outcome::Reloaded {
                snapshot: frozen.snapshot,
                changed,
            };
        }
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
        let Ok(mut slot) = self.state.lock() else {
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

    /// The frozen startup location, once startup has installed configuration.
    fn location(&self) -> Option<PathBuf> {
        self.state
            .try_lock()
            .ok()?
            .as_ref()
            .and_then(|state| state.current.path.clone())
    }

    fn sample(
        &self,
        sampler: &mut Sampler,
        now: Instant,
        load: impl FnOnce(
            Option<&std::path::Path>,
            bool,
        ) -> Result<
            orchbee_local_runtime::UserSettingsLoad,
            orchbee_local_runtime::SettingsError,
        >,
        apply: impl FnOnce(&UserSettings) -> bool,
    ) {
        if sampler.due.is_some_and(|due| now < due) {
            return;
        }
        sampler.due = now.checked_add(SAMPLE_INTERVAL);
        let Some(path) = self.location() else {
            return;
        };
        let observed = Observation::of(&path);
        if sampler.observed == Some(observed) {
            return;
        }
        // The first observation also reloads: an edit between the startup read and this sample
        // would otherwise be missed. An unchanged candidate publishes nothing.
        match self.reload_with(Publication::Sampled, load, apply) {
            // Another reload or busy manifest maintenance; observe the same file again next time.
            Outcome::Failed {
                reason: Failure::Busy | Failure::Unavailable,
                ..
            } => {}
            outcome => {
                sampler.observed = Some(observed);
                if matches!(outcome, Outcome::Reloaded { changed: true, .. }) {
                    self.changes.notify();
                }
            }
        }
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
        focus_new_panel: current.focus_new_panel,
        worktrees_directory: current.worktree_root.clone(),
        new_cwd: match &current.cwd {
            Some((DirectoryPolicy::Explicit, Some(path))) => path.clone(),
            Some((DirectoryPolicy::Home, _)) => "home".into(),
            Some((DirectoryPolicy::Current, _)) => "current".into(),
            Some((DirectoryPolicy::Workspace, _)) => "workspace".into(),
            None | Some((DirectoryPolicy::Follow | DirectoryPolicy::Explicit, _)) => {
                "follow".into()
            }
        },
    }
}

/// Keep the preference fact inside the existing bounded Session configuration channel.
fn retain_preferences_notice(diagnostics: &mut ConfigurationDiagnostics) {
    const FIELD: &str = "sharedPreferences";
    if diagnostics.items.iter().any(|item| item.field == FIELD) {
        return;
    }
    if diagnostics.items.len() == orchbee_workbench_protocol::MAXIMUM_CONFIGURATION_DIAGNOSTICS {
        diagnostics.items.pop();
        diagnostics.omitted_count = diagnostics
            .omitted_count
            .saturating_add(1)
            .min(MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED);
    }
    diagnostics.items.push(ConfigurationDiagnostic {
        field: FIELD.into(),
        reason: "unreadable".into(),
        disposition: ConfigurationDiagnosticDisposition::Defaulted,
    });
    diagnostics
        .items
        .sort_by(|a, b| (&a.field, &a.reason).cmp(&(&b.field, &b.reason)));
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
            .state
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
        let slot = self.configuration.state.try_lock().map_err(|_| ())?;
        Ok(slot.as_ref().is_some_and(|state| {
            state.current.notifications.delivery == orchbee_local_runtime::NotificationDelivery::Off
        }))
    }
    pub(crate) fn window_title_configuration(&self) -> Result<String, ()> {
        let slot = self.configuration.state.try_lock().map_err(|_| ())?;
        Ok(slot
            .as_ref()
            .map_or_else(String::new, |state| state.current.window_title.clone()))
    }

    /// No fallible effect follows scheduler admission.
    pub(super) fn apply_configuration(&self, settings: &UserSettings) -> bool {
        self.process_observer.apply_configuration(settings)
    }
    /// Samples the frozen `config.toml` location and publishes a changed file with explicit-reload
    /// semantics: a fatal file error keeps the previous snapshot until the file changes again.
    pub(super) fn sample_configuration(&self, sampler: &mut Sampler, now: Instant) {
        self.configuration.sample(
            sampler,
            now,
            |path, explicit| read_user_settings_at(path, explicit, |name| std::env::var_os(name)),
            |settings| self.apply_configuration(settings),
        );
    }
    pub(crate) fn watch_configuration(
        &self,
        signal: &std::sync::Arc<orchbee_server::SessionChangeSignal>,
    ) {
        self.configuration.changes.watch(signal);
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

#[cfg(all(test, windows))]
pub(super) mod tests;
