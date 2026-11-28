//! Provides shared native runtime admission, exact process ownership, device configuration and
//! plugin registry semantics, and storage capabilities to application owners. It does not choose
//! application startup policy or own Workbench state; the Server retains the meaning of opaque
//! Session checkpoint payloads.
#![deny(unsafe_code)]

mod admission_refusal;
pub use admission_refusal::{AdmissionRefusal, RefusalReason};
mod notification_policy;
pub use notification_policy::{AgentSoundSetting, NotificationDelivery, NotificationSettings};
mod plugins;
pub use plugins::{
    PluginActionDefinition, PluginGithubSource, PluginManagedSource, PluginManifest,
    PluginPaneDefinition, PluginPanePlacement, PluginPlatform, PluginPopupSize,
    PluginProvisionCommand, PluginProvisionExecutor, PluginProvisionFailure, PluginProvisionOutput,
    PluginProvisionReport, PluginProvisionStage, PluginRegisteredCatalog, PluginRegistry,
    PluginRegistryEntry, PluginRegistryError, PluginRegistryMutation, PluginRegistryResult,
    plugin_git_reference, plugin_identifier,
};
mod host_appearance;
mod host_system;
#[cfg(target_os = "macos")]
pub use host_appearance::read_macos_dark_appearance;
#[cfg(windows)]
pub use host_appearance::read_windows_apps_use_light_theme;
pub use host_system::WindowsVersion;
#[cfg(windows)]
pub use host_system::read_windows_version;
mod installed_shells;
pub use installed_shells::{InstalledShell, InstalledShellKind, installed_shells};
#[cfg(unix)]
mod host_name;
#[cfg(unix)]
pub use host_name::local_host_name;
mod civil_date;
mod digest_hex;
mod local_clock;
pub use local_clock::{LocalClock, local_clock, unix_milliseconds_at_local};
mod file_lock;
pub use file_lock::HeldLock;
mod private_cache;
pub use private_cache::PrivateCache;
mod release_store;
pub use release_store::{
    ActiveRelease, FallbackEvent, FallbackOutcome, FallbackRecorder, RELEASE_POINTER,
    RELEASE_POINTER_LIMIT, RELEASE_VERSIONS, Recorded, VersionHeld, activation_text,
    active_release, complete_move, held_versions, is_release_version_directory, record_event,
    release_store_root, window_of_cohort,
};
mod keeper;
#[cfg(windows)]
pub use keeper::worker_recorded;
pub use keeper::{
    HeldTerminal, Holding, HoldingState, KEEPER_REPLY_BOUND, KeeperClient, KeeperProcess,
    KeeperRefusal, KeeperStatus, ProgramHolding, start_keeper,
};
pub use keeper::{RecordedKeepers, end_recorded_keeper, recorded_keepers};
pub use keeper::{keeper_live, moves_live};
mod automatic_updates;
pub use automatic_updates::{
    AUTOMATIC_UPDATE_HELPER, CLIENT_FAILURE_HELPER, RELAUNCH_PROTOCOL_VARIABLE,
    WINDOW_INSTALLER_HELPER, automatic_updates_live, window_updates_live,
};
mod user_path;
pub use user_path::{
    USER_PATH_LIMIT, UserPathError, UserPathGrammar, UserPathOperand, normalize_posix_user_path,
    normalize_user_path,
};
mod user_settings;
pub use user_settings::set_update_channel;
pub use user_settings::{
    AGENT_SIDEBAR_PROFILE, CommandSize, CustomCommand, CustomCommandAction, CustomCommandEntry,
    DEFAULT_WINDOW_TITLE, DefaultActionBindings, DesktopTerminalSettings, DirectoryPolicy,
    KeyChord, KeyOverride, KeySettings, KeyTrigger, MAXIMUM_AGENT_SOUND_OVERRIDES,
    MAXIMUM_NOTIFICATION_DELAY_SECONDS, MAXIMUM_NOTIFICATION_PROFILE_BYTES,
    MAXIMUM_SETTING_CHANGES, MAXIMUM_SETTINGS_DIAGNOSTICS, MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES,
    MAXIMUM_STRUCTURED_ENTRIES, MAXIMUM_STRUCTURED_VALUE_BYTES, MAXIMUM_USER_SETTINGS_BYTES,
    MacosOptionAsAlt, SETTING_KEYS, SETTING_PLATFORMS, Scalar, SettingCategory, SettingChange,
    SettingChanges, SettingClients, SettingDefault, SettingEffect, SettingGroup, SettingKey,
    SettingScope, SettingValue, SettingsDiagnostic, SettingsDiagnosticDisposition,
    SettingsDiagnostics, SettingsEditError, SettingsError, SettingsSyntaxError, SidebarTokenStyle,
    SpaceSidebarSettings, SpaceSidebarToken, SpaceSidebarTokenKind, UnusableTerminalDefaults,
    UpdateChannel, UserColorThemes, UserSettings, UserSettingsField, UserSettingsLoad,
    UserTerminalColors, UserTerminalLaunchIntent, WindowTitleTemplate, configuration_diagnostics,
    default_user_settings_toml, default_user_settings_toml_with_actions, edit_settings,
    find_setting, read_startup_settings, read_startup_settings_at, read_user_settings,
    read_user_settings_at, restore_setting_default, set_setting, setting_changes, settings_refusal,
    settings_snapshot, settings_values, snapshot_values, trigger_label, user_settings_path,
    value_changes,
};
mod directory;
pub use directory::{
    AGENT_START_VARIABLE, long_form_of_short_name, posix_reported_directory, reported_directory,
    same_existing_folder, windows_agent_start_command, windows_cmd_agent_start_line,
    windows_interactive_powershell_initialization, windows_powershell_application,
    windows_reported_directory,
};
mod command_outcome;
pub use command_outcome::{CommandFailure, CommandOutput};
mod command;
pub use command::{
    posix_command_line, posix_shell_word, windows_encoded_command, windows_raw_command_encoded,
};
mod activation_record;
mod cohort;
mod data_root;
pub use data_root::{
    DataRoot, DataRootError, HOME_VARIABLE, LAYOUT, LayoutRefusal, PRODUCT_VERSION, StateRecord,
    TUI_POSITION_RECORD, agent_integration_may_have_been_enabled, compare_product_versions,
    default_runtime_base,
};
mod record_migration;
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
    diagnostic_archive, redact_settings,
};
mod diagnostic_log;
pub use diagnostic_log::{
    DAILY_LOG_LIMIT_BYTES, LogField, LogLevel, LogRole, LogValue, RETAINED_LOG_DAYS,
    RecentDiagnosticLogs, install_diagnostic_log, log_event, log_panicked, panic_thread_token,
    recent_diagnostic_logs, source_file_stem, utc_timestamp,
};
mod destructive_log;
pub use data_root::{create_private_directory, prepare_default_runtime_base};
pub use destructive_log::{DESTRUCTIVE_LOG_ROTATION_BYTES, append_destructive_request};
pub mod host_capacity;
#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "reviewed Mach host statistics and sysctl reads are isolated here"
)]
mod host_capacity_native;
#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "reviewed libproc reads of a process's start time, program and children are isolated here"
)]
mod process_identity_native;
#[cfg(target_os = "linux")]
mod process_identity_proc;
#[cfg(target_os = "linux")]
mod retained_spawn_linux;
#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "reviewed posix_spawn attributes, file actions and child waits are isolated here"
)]
mod retained_spawn_native;
#[cfg(windows)]
mod standard_rights;
#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "reviewed Win32 token restriction, attribute lists and process creation are isolated here"
)]
mod standard_rights_native;
#[cfg(windows)]
pub use standard_rights::{
    Administrators, IntegrityLevel, RelaunchFailure, TokenRights, relaunch_with_standard_rights,
    token_rights,
};
mod process_observation;
pub use process_observation::HostPresence;
#[cfg(unix)]
pub use process_observation::{process_ended, running_children, running_program};
mod process_registration;
mod recovery_blocker;
pub use recovery_blocker::{BlockerRole, RecoveryBlocker};
mod registered_child;
pub use activation_record::{ActivationState, RetainedActivationRecord};
pub use registered_child::{SessionChild, SessionChildStep, spawn_session_child};
pub use registered_child::{
    make_session_keeper_current, record_session_host_epoch, release_session_host,
    release_session_keeper,
};
mod durable_sessions;
pub use durable_sessions::{
    Fenced, HostSlotPresence, RetainedSessionRecord, Successor, session_moved_to,
};
mod exact_process;
pub use exact_process::ExactProcess;
mod successor_record;
pub use successor_record::{
    RestartCleared, RestartRefusal, RestartReservation, SuccessorRecord, clear_restart,
};
mod checkpoint_rotation;
mod layout_history;
mod machine_hub;
pub use machine_hub::machine_hub_endpoint;
mod rendezvous;
mod rendezvous_types;
mod retained_child;
mod session_catalog;
mod session_name;
mod session_store;
mod terminal_output;
mod time_name;
pub use session_catalog::{
    NamedSessionDeleteFailure, NamedSessionDeleteOutcome, NamedSessionDeleteReason,
    NamedSessionEntry, SessionCatalog, UnrecognizedRecord, UnrecognizedRecordKind,
    UnrecognizedRecordReason,
};
pub use session_name::SessionName;
pub mod private_filesystem;
pub use session_store::{
    CheckpointLoad, CheckpointRestoreNotice, SessionStore,
    bindings::{MAXIMUM_BINDING_RECORD_BYTES, read_session_bindings},
};
#[cfg(test)]
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

pub use cohort::{
    Cohort, CohortArtifact, CohortError, FilesExecutableError, ObservationAdaptersSidecar,
    current_executable,
};
#[cfg(unix)]
pub use rendezvous::host_lock_held;
#[cfg(windows)]
pub use rendezvous::{CLIENT_PENDING_SOCKET_NAME, HOST_ENDPOINT_ID, HOST_ENDPOINT_NAME};
pub use rendezvous::{
    EntryKind, EntryObservation, HOST_SCOPE_NAME, HostSlot, KeeperSlot, LockAttempt, LockKind,
    MAXIMUM_HOSTS, ModeCheckedLocalSessionRendezvous, ModeCheckedSessionLock, RendezvousError,
    SessionHolderObservation, SessionLifetimeObservation, SessionScope, acquire_past_probes,
};
#[cfg(windows)]
pub use retained_child::JobPlacement;
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
#[cfg(unix)]
mod session_path;

mod command_child;
#[cfg(unix)]
pub use command_child::DetachedChild;
pub use command_child::{CommandChild, CommandSpawnFailure};
#[cfg(unix)]
pub use command_child::{EndedGroups, GroupReport, ReportedGroup, end_reported_groups};
#[cfg(unix)]
pub use command_child::{
    SpawnEnvironment, run_unix_command, run_unix_command_exchange, run_unmasked_unix_command,
    run_unmasked_unix_command_following,
};
#[cfg(windows)]
pub use command_child::{
    resolve_windows_program, run_background_command_exchange, run_contained_command_exchange,
    run_windows_command, run_windows_command_exchange, windows_system_program,
};
mod default_interactive_launch;
pub use default_interactive_launch::{
    DefaultInteractiveLaunch, DefaultInteractiveLaunchError, DefaultLaunchEnvironmentEntry,
    default_launch_environment, resolve_default_interactive_launch,
};
#[cfg(unix)]
pub use default_interactive_launch::{character_locale_entry, login_shell_command};
