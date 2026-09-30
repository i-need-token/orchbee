//! Private native development runtime boundaries; applications own startup policy.
#[cfg(windows)]
mod windows_plugins;
#[cfg(windows)]
pub use windows_plugins::{
    PluginActionDefinition, PluginGithubSource, PluginManagedSource, PluginManifest,
    PluginPaneDefinition, PluginPanePlacement, PluginPlatform, PluginPopupSize,
    PluginProvisionCommand, PluginProvisionExecutor, PluginProvisionFailure, PluginProvisionOutput,
    PluginProvisionReport, PluginProvisionStage, PluginRegistry, PluginRegistryEntry,
    PluginRegistryError, PluginRegistryMutation, PluginRegistryResult, plugin_git_reference,
    plugin_identifier,
};
#[cfg(windows)]
mod private_cache_windows;
#[cfg(windows)]
pub use private_cache_windows::WindowsPrivateCache;
#[cfg(windows)]
mod windows_user_path;
#[cfg(windows)]
pub use windows_user_path::{
    WINDOWS_USER_PATH_LIMIT, WindowsUserPathError, normalize_windows_user_path,
};
#[cfg(windows)]
mod windows_user_settings;
#[cfg(windows)]
pub use windows_user_settings::{
    DEFAULT_WINDOW_TITLE, MAXIMUM_AGENT_SIDEBAR_PROFILE_BYTES, MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES,
    WindowsAgentSidebarSettings, WindowsAgentSidebarToken, WindowsAgentSidebarTokenKind,
    WindowsDirectoryPolicy, WindowsKeyChord, WindowsKeyOverride, WindowsKeySettings,
    WindowsKeyTrigger, WindowsKeysResetOutcome, WindowsKeysResetReport,
    WindowsNotificationDelivery, WindowsNotificationSettings, WindowsSettingsError,
    WindowsShellMode, WindowsShellSettings, WindowsSidebarTokenStyle, WindowsSpaceSidebarSettings,
    WindowsSpaceSidebarToken, WindowsSpaceSidebarTokenKind, WindowsUpdateChannel,
    WindowsUserSettings, WindowsWindowTitleTemplate, read_windows_user_settings,
    read_windows_user_settings_at, reset_windows_keybindings, set_windows_update_channel,
    windows_user_settings_path,
};
#[cfg(windows)]
mod windows_directory;
#[cfg(windows)]
pub use windows_directory::{WINDOWS_DIRECTORY_PROMPT, windows_reported_directory};
#[cfg(windows)]
mod windows_command;
#[cfg(windows)]
pub use windows_command::windows_encoded_command;
#[cfg(windows)]
mod activation_record_windows;
mod cohort;
#[cfg(windows)]
pub use activation_record_windows::{ActivationState, RetainedActivationRecord};
#[cfg(windows)]
mod durable_sessions_windows;
#[cfg(windows)]
pub use durable_sessions_windows::WindowsRetainedSessionRecord;
#[cfg(windows)]
mod named_site_catalog_windows;
#[cfg_attr(windows, path = "rendezvous_windows.rs")]
mod rendezvous;
mod rendezvous_types;
#[cfg_attr(windows, path = "retained_child_windows.rs")]
mod retained_child;
#[cfg(windows)]
mod session_store_windows;
#[cfg(windows)]
mod site_name_windows;
#[cfg(windows)]
pub use named_site_catalog_windows::{
    WindowsNamedSiteCatalog, WindowsNamedSiteDeleteFailure, WindowsNamedSiteDeleteOutcome,
    WindowsNamedSiteDeleteReason, WindowsNamedSiteEntry,
};
#[cfg(windows)]
pub use site_name_windows::WindowsSiteName;
#[cfg(windows)]
mod windows_permissions;
#[cfg(windows)]
pub use session_store_windows::WindowsSessionStore;
#[cfg(all(test, windows))]
mod windows_tests;

pub use cohort::{NativeArtifact, NativeCohort, ObservationAdaptersSidecar};
#[cfg(windows)]
mod windows_release_store;
pub use rendezvous::{
    CLIENT_PENDING_SOCKET_NAME, EntryKind, EntryObservation, HOST_ENDPOINT_ID, HOST_ENDPOINT_NAME,
    HOST_SCOPE_NAME, LockAttempt, LockKind, ModeCheckedLocalSiteRendezvous, ModeCheckedSiteLock,
    RendezvousError, SiteLifetimeObservation, SiteScope,
};
pub use retained_child::RetainedChild;
#[cfg(windows)]
pub use windows_release_store::{
    WindowsReleaseStage, WindowsReleaseStore, open_windows_release_file, valid_windows_release_path,
};

#[cfg(target_os = "linux")]
mod linux_permissions;

/// Spawns an exact cohort executable with inherited environment and no inherited
/// stdio or handles. The returned owner observes exit without killing on drop.
pub fn spawn_retained(
    executable: &std::path::Path,
    arguments: &[&std::ffi::OsStr],
) -> std::io::Result<RetainedChild> {
    RetainedChild::spawn(executable, arguments)
}

#[cfg(windows)]
mod windows_command_child;
#[cfg(windows)]
pub use windows_command_child::{
    WindowsCommandChild, WindowsCommandFailure, WindowsCommandOutput, WindowsCommandSpawnFailure,
    resolve_windows_program, run_windows_command, run_windows_command_exchange,
};
