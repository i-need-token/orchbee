//! One Session-lifetime owner of `config.toml`: the last valid settings, the configuration and
//! settings snapshots, and the one revision counter both snapshots take their revisions from.
//! Fallible candidate preparation precedes publication.
use orchbee_local_runtime::{
    DirectoryPolicy, SettingChanges, SettingsDiagnostics, SettingsEditError, SettingsError,
    UserSettings, UserSettingsLoad, configuration_diagnostics, read_user_settings_at,
    setting_changes, settings_refusal, settings_snapshot,
};
use orchbee_workbench_protocol::{
    ConfigurationAction as Action, ConfigurationDiagnostics, ConfigurationFailure as Failure,
    ConfigurationOutcome as Outcome, ConfigurationRequest, ConfigurationResult,
    ConfigurationSnapshot, ContractError, MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED,
    ProtocolVersion, SessionId, SettingChange, SettingsFile, SettingsOutcome, SettingsRefusal,
    SettingsResult, SettingsSnapshot, UserPreferencesValues, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchRevision,
};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant, SystemTime},
};

/// The single sampler checks the frozen file at most this often.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
/// An edit waits this long for another reload or edit to finish before it is refused as busy.
const EDIT_ADMISSION_WAIT: Duration = Duration::from_millis(500);
const ADMISSION_RETRY: Duration = Duration::from_millis(10);

#[derive(Default)]
pub(super) struct Registry {
    state: Mutex<Option<State>>,
    /// Sampled and edited publications have no requesting connection; this wakes every
    /// subscriber.
    changes: orchbee_server::SessionChangeSource,
}
pub(super) struct PreparedInitial(State);
/// `current` is the last valid settings and also carries the frozen startup location: every read
/// at that location reports it.
struct State {
    current: UserSettings,
    snapshot: ConfigurationSnapshot,
    settings: SettingsSnapshot,
    /// Whether a process observer applies committed settings to live policy.
    applies: bool,
    reloading: bool,
}
impl State {
    /// The counter both snapshots take their revisions from: the latest revision either took.
    fn revision(&self) -> u64 {
        self.snapshot
            .revision
            .value()
            .max(self.settings.revision.value())
    }
}
struct FrozenState {
    current: UserSettings,
    snapshot: ConfigurationSnapshot,
    settings: SettingsSnapshot,
    revision: u64,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Publication {
    /// The first explicit publication replaces Client startup-only keys, even if otherwise equal.
    /// A file that cannot be read or parsed fails it.
    Explicit,
    /// A changed file publishes only a changed candidate and never reapplies an equal one. A file
    /// that cannot be read or parsed publishes its state with the last valid values.
    Sampled,
    /// The file an accepted edit wrote, published as a sampled one is.
    Edited,
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
/// Why a reload committed nothing.
struct Rejection {
    reason: Failure,
    field: Option<&'static str>,
    detail: Option<&'static str>,
}
impl From<Failure> for Rejection {
    fn from(reason: Failure) -> Self {
        Self {
            reason,
            field: None,
            detail: None,
        }
    }
}
impl Rejection {
    const fn load_failed(error: &SettingsError) -> Self {
        Self {
            reason: Failure::LoadFailed,
            field: Some(error.field),
            detail: Some(error.reason),
        }
    }
    fn outcome(self) -> Outcome {
        Outcome::Failed {
            reason: self.reason,
            field: self.field.map(Into::into),
            detail: self.detail.map(Into::into),
        }
    }
}
/// The snapshots a reload left current.
struct Committed {
    configuration: ConfigurationSnapshot,
    settings: SettingsSnapshot,
    configuration_changed: bool,
    /// Either snapshot changed, so subscribers must read again.
    published: bool,
    /// The committed settings turned `terminal.restore_output` from on to off.
    output_off: bool,
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
        self.install_initial(Self::prepare_initial(current, true)?)
    }
    /// `applies` says whether a process observer applies committed settings to live policy.
    pub(super) fn prepare_initial(
        current: &UserSettingsLoad,
        applies: bool,
    ) -> std::io::Result<PreparedInitial> {
        let diagnostics = protocol_diagnostics(&current.diagnostics).ok_or_else(|| {
            std::io::Error::other("Configuration diagnostics exceed protocol bounds")
        })?;
        let initial = snapshot(&current.settings, diagnostics, 0);
        let settings = settings_snapshot(current, 0);
        if preflight_publication(&Outcome::Current {
            snapshot: initial.clone(),
        })
        .and_then(|()| preflight_settings(&settings))
        .is_err()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Configuration startup publication exceeds protocol bounds",
            ));
        }
        Ok(PreparedInitial(State {
            current: current.settings.clone(),
            snapshot: initial,
            settings,
            applies,
            reloading: false,
        }))
    }
    pub(super) fn install_initial(&self, prepared: PreparedInitial) -> std::io::Result<()> {
        let mut slot = self
            .state
            .lock()
            .map_err(|_| std::io::Error::other("Configuration owner poisoned"))?;
        if slot.is_some() {
            return Err(std::io::ErrorKind::AlreadyExists.into());
        }
        *slot = Some(prepared.0);
        Ok(())
    }
    #[cfg(all(test, windows))]
    pub(super) fn initialized_for_test(&self) -> bool {
        self.state.lock().unwrap().is_some()
    }
    /// Reads the last valid preference values, or `None` before startup installs configuration.
    pub(super) fn with_preferences<T>(
        &self,
        read: impl FnOnce(&UserPreferencesValues) -> T,
    ) -> Option<T> {
        let slot = self.state.lock().ok()?;
        slot.as_ref().map(|state| read(&state.current.preferences))
    }
    #[cfg(all(test, windows))]
    fn execute(
        &self,
        request: &ConfigurationRequest,
        apply: impl FnOnce(&UserSettings) -> bool,
    ) -> Outcome {
        self.respond(request, read_frozen, apply).0
    }

    #[cfg(all(test, windows))]
    fn execute_with(
        &self,
        request: &ConfigurationRequest,
        load: impl FnOnce(Option<&Path>, bool) -> Result<UserSettingsLoad, SettingsError>,
        apply: impl FnOnce(&UserSettings) -> bool,
    ) -> Outcome {
        self.respond(request, load, apply).0
    }

    /// Answers one configuration request, and whether its commit turned kept output off.
    fn respond(
        &self,
        request: &ConfigurationRequest,
        load: impl FnOnce(Option<&Path>, bool) -> Result<UserSettingsLoad, SettingsError>,
        apply: impl FnOnce(&UserSettings) -> bool,
    ) -> (Outcome, bool) {
        if request.action == Action::Current {
            // Reloads read the file outside the lock, so a current read only waits for a commit.
            let Ok(slot) = self.state.lock() else {
                return (Outcome::failed(Failure::Unavailable), false);
            };
            let outcome = slot
                .as_ref()
                .map_or(Outcome::failed(Failure::Unavailable), |state| {
                    Outcome::Current {
                        snapshot: state.snapshot.clone(),
                    }
                });
            return (outcome, false);
        }
        match self.reload_with(Publication::Explicit, load, apply) {
            Ok(committed) => (
                Outcome::Reloaded {
                    snapshot: committed.configuration,
                    changed: committed.configuration_changed,
                },
                committed.output_off,
            ),
            Err(rejection) => (rejection.outcome(), false),
        }
    }

    /// The current settings snapshot.
    fn settings_current(&self) -> SettingsOutcome {
        let Ok(slot) = self.state.lock() else {
            return SettingsOutcome::refused(SettingsRefusal::Unavailable);
        };
        slot.as_ref().map_or(
            SettingsOutcome::refused(SettingsRefusal::Unavailable),
            |state| SettingsOutcome::Current {
                snapshot: Box::new(state.settings.clone()),
            },
        )
    }

    fn admit(&self) -> Result<(FrozenState, ReloadAdmission<'_>), Failure> {
        let Ok(mut slot) = self.state.try_lock() else {
            return Err(Failure::Busy);
        };
        let Some(state) = slot.as_mut() else {
            return Err(Failure::Unavailable);
        };
        if state.reloading {
            return Err(Failure::Busy);
        }
        state.reloading = true;
        let frozen = FrozenState {
            current: state.current.clone(),
            snapshot: state.snapshot.clone(),
            settings: state.settings.clone(),
            revision: state.revision(),
        };
        drop(slot);
        Ok((
            frozen,
            ReloadAdmission {
                registry: self,
                active: true,
            },
        ))
    }

    /// Admission for an edit: a person is waiting, so another reload or edit gets a moment to
    /// finish before the edit is refused as busy.
    fn admit_waiting(&self) -> Result<(FrozenState, ReloadAdmission<'_>), Failure> {
        let deadline = Instant::now() + EDIT_ADMISSION_WAIT;
        loop {
            match self.admit() {
                Err(Failure::Busy) if Instant::now() < deadline => {
                    std::thread::sleep(ADMISSION_RETRY);
                }
                admitted => return admitted,
            }
        }
    }

    fn reload_with(
        &self,
        publication: Publication,
        load: impl FnOnce(Option<&Path>, bool) -> Result<UserSettingsLoad, SettingsError>,
        apply: impl FnOnce(&UserSettings) -> bool,
    ) -> Result<Committed, Rejection> {
        let (frozen, admission) = self.admit()?;
        // File I/O and TOML preparation run without the committed-state mutex.
        let loaded = load(frozen.current.path.as_deref(), frozen.current.explicit);
        self.commit(admission, frozen, publication, loaded, apply)
    }

    /// Prepares `loaded` against `frozen` and commits it under `admission`. A file that is not
    /// valid TOML or cannot be read keeps the committed settings until the file changes: an
    /// explicit reload fails, while a sampled or edited file publishes its state with the last
    /// valid values.
    fn commit(
        &self,
        admission: ReloadAdmission<'_>,
        frozen: FrozenState,
        publication: Publication,
        loaded: Result<UserSettingsLoad, SettingsError>,
        apply: impl FnOnce(&UserSettings) -> bool,
    ) -> Result<Committed, Rejection> {
        let (valid, mut settings) = match loaded {
            Ok(load) => match load.syntax {
                None => {
                    let settings = settings_snapshot(&load, frozen.settings.revision.value());
                    (Some(load), settings)
                }
                Some(syntax) if publication == Publication::Explicit => {
                    return Err(Rejection::load_failed(&SettingsError::new(
                        "file",
                        syntax.reason,
                    )));
                }
                Some(_) => {
                    let kept = load.or_last_valid(&frozen.current);
                    (
                        None,
                        settings_snapshot(&kept, frozen.settings.revision.value()),
                    )
                }
            },
            Err(error) if publication == Publication::Explicit => {
                return Err(Rejection::load_failed(&error));
            }
            Err(error) => (
                None,
                SettingsSnapshot {
                    file: SettingsFile::Unreadable {
                        reason: error.reason.into(),
                    },
                    ..frozen.settings.clone()
                },
            ),
        };
        let settings_changed = settings != frozen.settings;
        let (configuration_changed, diagnostics) = match &valid {
            Some(load) => {
                let diagnostics =
                    protocol_diagnostics(&load.diagnostics).ok_or(Failure::LoadFailed)?;
                let changed = (publication == Publication::Explicit
                    && frozen.snapshot.revision.value() == 0)
                    || load.settings != frozen.current
                    || diagnostics != frozen.snapshot.diagnostics;
                (changed, Some(diagnostics))
            }
            None => (false, None),
        };
        let changed = configuration_changed || settings_changed;
        if !changed && publication != Publication::Explicit {
            return Ok(Committed {
                configuration: frozen.snapshot,
                settings: frozen.settings,
                configuration_changed: false,
                published: false,
                output_off: false,
            });
        }
        let revision = if changed {
            frozen
                .revision
                .checked_add(1)
                .ok_or(Failure::RevisionExhausted)?
        } else {
            frozen.revision
        };
        let configuration = match (&valid, diagnostics) {
            (Some(load), Some(diagnostics)) if configuration_changed => {
                snapshot(&load.settings, diagnostics, revision)
            }
            _ => frozen.snapshot.clone(),
        };
        if settings_changed {
            settings.revision = WorkbenchRevision::new(revision);
        }
        // Prepare the exact bounded outbound representation before changing the live scheduler.
        if orchbee_local_runtime::KeySettings::from_profile(&configuration.keys_profile).is_err()
            || configuration.keys_profile.len() > 64 * 1024
            || orchbee_local_runtime::SpaceSidebarSettings::from_profile(
                &configuration.spaces_sidebar_profile,
            )
            .is_err()
            || orchbee_local_runtime::AgentSidebarSettings::from_profile(
                &configuration.agent_sidebar_profile,
            )
            .is_err()
            || orchbee_local_runtime::NotificationSettings::from_profile(
                &configuration.notification_profile,
            )
            .is_err()
        {
            return Err(Failure::LoadFailed.into());
        }
        if preflight_publication(&Outcome::Reloaded {
            snapshot: configuration.clone(),
            changed: configuration_changed,
        })
        .and_then(|()| preflight_settings(&settings))
        .is_err()
        {
            return Err(Failure::LoadFailed.into());
        }
        let Ok(mut slot) = self.state.lock() else {
            return Err(Failure::Unavailable.into());
        };
        let Some(state) = slot.as_mut() else {
            return Err(Failure::Unavailable.into());
        };
        if !state.reloading || state.revision() != frozen.revision {
            return Err(Failure::Busy.into());
        }
        if let Some(load) = &valid
            && state.applies
            && !apply(&load.settings)
        {
            return Err(Failure::Unavailable.into());
        }
        let output_off = valid.as_ref().is_some_and(|load| {
            state.current.preferences.restore_terminal_output
                && !load.settings.preferences.restore_terminal_output
        });
        if configuration_changed && let Some(load) = valid {
            state.current = load.settings;
            state.snapshot = configuration.clone();
        }
        if settings_changed {
            state.settings = settings.clone();
        }
        state.reloading = false;
        admission.complete();
        Ok(Committed {
            configuration,
            settings,
            configuration_changed,
            published: changed,
            output_off,
        })
    }

    /// Writes `changes` to the frozen file and publishes the file it leaves: one batch, every
    /// change or none. A refused edit writes and publishes nothing. Returns whether the commit
    /// turned kept output off.
    fn edit_with(
        &self,
        changes: &[SettingChange],
        edit: impl FnOnce(Option<&Path>, &SettingChanges) -> Result<bool, SettingsEditError>,
        load: impl FnOnce(Option<&Path>, bool) -> Result<UserSettingsLoad, SettingsError>,
        apply: impl FnOnce(&UserSettings) -> bool,
    ) -> (SettingsOutcome, bool) {
        let (Some(first), Some(batch)) = (changes.first(), setting_changes(changes)) else {
            return (
                SettingsOutcome::refused(SettingsRefusal::Unavailable),
                false,
            );
        };
        let (frozen, admission) = match self.admit_waiting() {
            Ok(admitted) => admitted,
            Err(Failure::Busy) => return (SettingsOutcome::refused(SettingsRefusal::Busy), false),
            Err(_) => {
                return (
                    SettingsOutcome::refused(SettingsRefusal::Unavailable),
                    false,
                );
            }
        };
        let changed = match edit(frozen.current.path.as_deref(), &batch) {
            Ok(changed) => changed,
            Err(error) => return (settings_refusal(&error, first.key()), false),
        };
        let previous = frozen.settings.clone();
        let loaded = load(frozen.current.path.as_deref(), frozen.current.explicit);
        match self.commit(admission, frozen, Publication::Edited, loaded, apply) {
            Ok(committed) => {
                if committed.published {
                    self.changes.notify();
                }
                (
                    SettingsOutcome::Edited {
                        snapshot: Box::new(committed.settings),
                        changed,
                    },
                    committed.output_off,
                )
            }
            // The file is written; the sampler publishes it once live policy admits it.
            Err(_) => (
                SettingsOutcome::Edited {
                    snapshot: Box::new(previous),
                    changed,
                },
                false,
            ),
        }
    }

    /// The frozen startup location, once startup has installed configuration.
    fn location(&self) -> Option<PathBuf> {
        self.state
            .try_lock()
            .ok()?
            .as_ref()
            .and_then(|state| state.current.path.clone())
    }

    /// Returns whether a published commit turned kept output off.
    fn sample(
        &self,
        sampler: &mut Sampler,
        now: Instant,
        load: impl FnOnce(Option<&Path>, bool) -> Result<UserSettingsLoad, SettingsError>,
        apply: impl FnOnce(&UserSettings) -> bool,
    ) -> bool {
        if sampler.due.is_some_and(|due| now < due) {
            return false;
        }
        sampler.due = now.checked_add(SAMPLE_INTERVAL);
        let Some(path) = self.location() else {
            return false;
        };
        let observed = Observation::of(&path);
        if sampler.observed == Some(observed) {
            return false;
        }
        // The first observation also reloads: an edit between the startup read and this sample
        // would otherwise be missed. An unchanged candidate publishes nothing.
        match self.reload_with(Publication::Sampled, load, apply) {
            // Another reload or busy manifest maintenance; observe the same file again next time.
            Err(Rejection {
                reason: Failure::Busy | Failure::Unavailable,
                ..
            }) => false,
            Err(_) => {
                sampler.observed = Some(observed);
                false
            }
            Ok(committed) => {
                sampler.observed = Some(observed);
                if committed.published {
                    self.changes.notify();
                }
                committed.output_off
            }
        }
    }
}

fn read_frozen(path: Option<&Path>, explicit: bool) -> Result<UserSettingsLoad, SettingsError> {
    read_user_settings_at(path, explicit, |name| std::env::var_os(name))
}

/// The editor's environment, naming the frozen location so an edit writes the file the Session
/// reads.
fn edit_frozen(path: Option<&Path>, changes: &SettingChanges) -> Result<bool, SettingsEditError> {
    let path = path.map(|path| path.as_os_str().to_owned());
    orchbee_local_runtime::edit_settings(changes, |name| match (name, &path) {
        ("ORCHBEE_CONFIG_FILE", Some(path)) => Some(path.clone()),
        _ => std::env::var_os(name),
    })
}

fn fixed_identity() -> (WorkbenchRequestId, SessionId) {
    (
        WorkbenchRequestId::parse("00000000000000000000000000000001")
            .expect("fixed configuration preflight request id"),
        SessionId::parse("00000000000000000000000000000002")
            .expect("fixed configuration preflight Session id"),
    )
}

fn publication_result(outcome: Outcome) -> ConfigurationResult {
    let (request_id, session_id) = fixed_identity();
    ConfigurationResult {
        protocol_version: ProtocolVersion,
        request_id,
        session_id,
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

/// The settings snapshot as the bounded response that serves it.
fn preflight_settings(snapshot: &SettingsSnapshot) -> Result<(), ContractError> {
    let (request_id, session_id) = fixed_identity();
    orchbee_workbench_protocol::serialize_workbench_response(&WorkbenchResponse::Settings(
        SettingsResult {
            protocol_version: ProtocolVersion,
            request_id,
            session_id,
            outcome: SettingsOutcome::Current {
                snapshot: Box::new(snapshot.clone()),
            },
        },
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

/// The configuration snapshot refuses diagnostics whose omitted count exceeds its bound.
fn protocol_diagnostics(diagnostics: &SettingsDiagnostics) -> Option<ConfigurationDiagnostics> {
    u32::try_from(diagnostics.omitted_count())
        .is_ok_and(|omitted| omitted <= MAXIMUM_CONFIGURATION_DIAGNOSTICS_OMITTED)
        .then(|| configuration_diagnostics(diagnostics))
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
    /// Installs the startup configuration where no process observer runs: the Session serves and
    /// edits its settings, and committed settings have no live policy to apply.
    pub(crate) fn install_configuration(
        &self,
        configuration: &UserSettingsLoad,
    ) -> std::io::Result<()> {
        self.configuration
            .install_initial(Registry::prepare_initial(configuration, false)?)
    }
    /// Samples the frozen `config.toml` location and publishes a changed file: a file that cannot
    /// be read or parsed publishes its state and keeps the last valid settings until it changes.
    pub(super) fn sample_configuration(&self, sampler: &mut Sampler, now: Instant) {
        let output_off = self
            .configuration
            .sample(sampler, now, read_frozen, |settings| {
                self.apply_configuration(settings)
            });
        self.after_configuration_commit(output_off);
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
            Some(true) => {
                let (outcome, output_off) =
                    self.configuration
                        .respond(&request, read_frozen, |settings| {
                            self.apply_configuration(settings)
                        });
                self.after_configuration_commit(output_off);
                outcome
            }
        };
        ConfigurationResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id,
            session_id: request.session_id,
            outcome,
        }
    }
    /// Reads the settings snapshot, or writes one batch of changes to the frozen file.
    pub(super) fn settings_outcome(
        &self,
        operation: &orchbee_workbench_protocol::SettingsOperation,
    ) -> SettingsOutcome {
        match operation {
            orchbee_workbench_protocol::SettingsOperation::Read {} => {
                self.configuration.settings_current()
            }
            orchbee_workbench_protocol::SettingsOperation::Edit { changes } => {
                let (outcome, output_off) =
                    self.configuration
                        .edit_with(changes, edit_frozen, read_frozen, |settings| {
                            self.apply_configuration(settings)
                        });
                self.after_configuration_commit(output_off);
                outcome
            }
        }
    }
    /// Kept output exists only in the Windows checkpoint store; turning the setting off removes
    /// it at once, whether an edit or another editor turned it off.
    fn after_configuration_commit(&self, output_off: bool) {
        #[cfg(windows)]
        if output_off {
            self.remove_terminal_output();
        }
        #[cfg(not(windows))]
        let _ = output_off;
    }
}

#[cfg(all(test, windows))]
pub(super) mod tests;
