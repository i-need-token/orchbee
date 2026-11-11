//! Provides shared native runtime admission, exact process ownership, device configuration and
//! plugin registry semantics, and storage capabilities to application owners. It does not choose
//! application startup policy or own Workbench state; the Server retains the meaning of opaque
//! Session checkpoint payloads.

mod admission_refusal;
pub use admission_refusal::{AdmissionRefusal, RefusalReason};
mod notification_policy;
pub use notification_policy::{AgentSoundSetting, NotificationDelivery, NotificationSettings};
mod user_preferences;
pub use user_preferences::{
    UserColorThemes, UserPreferencesScope, UserPreferencesStore, UserTerminalColors,
    UserTerminalLaunchIntent,
};
mod plugins;
pub use plugins::{
    PluginActionDefinition, PluginGithubSource, PluginManagedSource, PluginManifest,
    PluginPaneDefinition, PluginPanePlacement, PluginPlatform, PluginPopupSize,
    PluginProvisionCommand, PluginProvisionExecutor, PluginProvisionFailure, PluginProvisionOutput,
    PluginProvisionReport, PluginProvisionStage, PluginRegisteredCatalog, PluginRegistry,
    PluginRegistryEntry, PluginRegistryError, PluginRegistryMutation, PluginRegistryResult,
    plugin_git_reference, plugin_identifier,
};
#[cfg(windows)]
mod host_appearance;
mod host_system;
#[cfg(windows)]
pub use host_appearance::read_windows_apps_use_light_theme;
pub use host_system::WindowsVersion;
#[cfg(windows)]
pub use host_system::read_windows_version;
mod local_clock;
pub use local_clock::{LocalClock, local_clock};
mod private_cache;
pub use private_cache::PrivateCache;
mod user_path;
pub use user_path::{USER_PATH_LIMIT, UserPathError, UserPathOperand, normalize_user_path};
mod user_settings;
pub use user_settings::{
    AgentSidebarSettings, AgentSidebarToken, AgentSidebarTokenKind, CommandSize, CustomCommand,
    CustomCommandAction, CustomCommandEntry, DEFAULT_WINDOW_TITLE, DefaultActionBindings,
    DirectoryPolicy, KeyChord, KeyOverride, KeySettings, KeyTrigger,
    MAXIMUM_AGENT_SIDEBAR_PROFILE_BYTES, MAXIMUM_AGENT_SOUND_OVERRIDES,
    MAXIMUM_NOTIFICATION_DELAY_SECONDS, MAXIMUM_NOTIFICATION_PROFILE_BYTES,
    MAXIMUM_SETTINGS_DIAGNOSTICS, MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES, SettingsDiagnostic,
    SettingsDiagnosticDisposition, SettingsDiagnostics, SettingsError, SidebarTokenStyle,
    SpaceSidebarSettings, SpaceSidebarToken, SpaceSidebarTokenKind, USER_CONFIG_OPTIONS,
    UpdateChannel, UserConfigOption, UserSettings, UserSettingsLoad, WindowTitleTemplate,
    default_user_settings_toml, default_user_settings_toml_with_actions, read_user_settings,
    read_user_settings_at, trigger_label, user_settings_path,
};
pub use user_settings::{KeysResetOutcome, KeysResetReport, reset_keybindings, set_update_channel};
mod directory;
pub use directory::{windows_interactive_powershell_initialization, windows_reported_directory};
mod command_outcome;
pub use command_outcome::{CommandFailure, CommandOutput};
mod command;
pub use command::{windows_encoded_command, windows_raw_command_encoded};
mod activation_record;
mod cohort;
mod data_root;
pub use data_root::{
    DataRoot, DataRootError, HOME_VARIABLE, LAYOUT, LayoutRefusal, PRODUCT_VERSION, StateRecord,
    TUI_POSITION_RECORD, agent_integration_may_have_been_enabled, default_runtime_base,
    legacy_agent_observation_capsules,
};
mod record_migration;
#[cfg(windows)]
pub use record_migration::back_up_for_reset;
pub use record_migration::{
    OpenedRecord, RecordNotice, RecordProblem, VersionedFile, VersionedRecord, VersionedWriter,
    inspect_records, open_to_read, open_to_write, report_record_notice, reported_record_notices,
    take_unshown_record_notices,
};
mod environment;
pub use environment::{
    ENVIRONMENT_VARIABLE, EnvironmentKind, checkout_name, environment_kind, environment_name,
    environment_stem, is_release, session_route_environment, valid_environment_stem,
};
mod diagnostic_bundle;
pub use diagnostic_bundle::{
    ARGUMENT_PLACEHOLDER, COMMAND_PLACEHOLDER, CREDENTIAL_PLACEHOLDER, DiagnosticRedactor,
    ENVIRONMENT_PLACEHOLDER, HOME_PLACEHOLDER, QUERY_PLACEHOLDER, UNRECOGNIZED_PLACEHOLDER,
    diagnostic_archive, redact_preferences, redact_settings,
};
mod diagnostic_log;
pub use diagnostic_log::{
    DAILY_LOG_LIMIT_BYTES, LogField, LogLevel, LogRole, LogValue, RETAINED_LOG_DAYS,
    RecentDiagnosticLogs, install_diagnostic_log, log_event, recent_diagnostic_logs,
};
#[cfg(windows)]
mod destructive_log;
pub use data_root::{create_private_directory, prepare_default_runtime_base};
#[cfg(windows)]
pub use destructive_log::{DESTRUCTIVE_LOG_ROTATION_BYTES, append_destructive_request};
#[cfg(windows)]
mod editor_documents;
#[cfg(windows)]
pub use editor_documents::{EditorDocumentRemoval, remove_stale_editor_documents};
#[cfg(windows)]
mod process_observation;
#[cfg(windows)]
mod process_registration;
mod registered_child;
pub use activation_record::{ActivationState, RetainedActivationRecord};
pub use registered_child::{SessionChild, spawn_session_child};
mod durable_sessions;
pub use durable_sessions::RetainedSessionRecord;
#[cfg(windows)]
mod checkpoint_restore_notice;
#[cfg(windows)]
mod checkpoint_rotation;
#[cfg(windows)]
mod layout_history;
mod rendezvous;
mod rendezvous_types;
mod retained_child;
mod session_catalog;
mod session_name;
mod session_store;
#[cfg(windows)]
mod terminal_output;
#[cfg(windows)]
mod time_name;
pub use session_catalog::{
    NamedSessionDeleteFailure, NamedSessionDeleteOutcome, NamedSessionDeleteReason,
    NamedSessionEntry, SessionCatalog, UnrecognizedRecord, UnrecognizedRecordKind,
    UnrecognizedRecordReason,
};
pub use session_name::SessionName;
pub mod private_filesystem;
pub use session_store::{
    CheckpointLoad, CheckpointRestoreNotice, LayoutHistoryEntry, SessionStore,
};
#[cfg(all(test, windows))]
mod tests;

#[cfg(test)]
pub(crate) fn test_temporary_directory() -> std::path::PathBuf {
    #[cfg(target_os = "macos")]
    {
        std::path::PathBuf::from("/private/tmp")
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::env::temp_dir()
    }
}

pub use cohort::{Cohort, CohortArtifact, CohortError, ObservationAdaptersSidecar};
#[cfg(windows)]
pub use rendezvous::{CLIENT_PENDING_SOCKET_NAME, HOST_ENDPOINT_ID, HOST_ENDPOINT_NAME};
pub use rendezvous::{
    EntryKind, EntryObservation, HOST_SCOPE_NAME, LockAttempt, LockKind,
    ModeCheckedLocalSessionRendezvous, ModeCheckedSessionLock, RendezvousError,
    SessionLifetimeObservation, SessionScope,
};
pub use retained_child::RetainedChild;

/// Spawns an exact cohort executable with inherited environment, no inherited
/// stdio or handles, and a neutral working directory (the system directory on
/// Windows, `/` on Unix). The returned owner observes exit without killing on drop.
pub fn spawn_retained(
    executable: &std::path::Path,
    arguments: &[&std::ffi::OsStr],
) -> std::io::Result<RetainedChild> {
    RetainedChild::spawn(executable, arguments)
}

mod command_child;
pub use command_child::{CommandChild, CommandSpawnFailure};
#[cfg(windows)]
pub use command_child::{
    resolve_windows_program, run_windows_command, run_windows_command_exchange,
};
mod default_interactive_launch;
pub use default_interactive_launch::{
    DefaultInteractiveLaunch, DefaultInteractiveLaunchError, DefaultLaunchEnvironmentEntry,
    default_launch_environment, resolve_default_interactive_launch,
};
