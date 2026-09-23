//! Private native development runtime boundaries; applications own startup policy.
#[cfg(windows)]
mod activation_record_windows;
mod cohort;
#[cfg(windows)]
pub use activation_record_windows::{ActivationState, RetainedActivationRecord};
#[cfg_attr(windows, path = "rendezvous_windows.rs")]
mod rendezvous;
mod rendezvous_types;
#[cfg_attr(windows, path = "retained_child_windows.rs")]
mod retained_child;
#[cfg(windows)]
mod windows_permissions;
#[cfg(all(test, windows))]
mod windows_tests;

pub use cohort::{NativeArtifact, NativeCohort, ObservationAdaptersSidecar};
pub use rendezvous::{
    CLIENT_PENDING_SOCKET_NAME, EntryKind, EntryObservation, HOST_ENDPOINT_ID, HOST_ENDPOINT_NAME,
    HOST_SCOPE_NAME, LockAttempt, LockKind, ModeCheckedLocalSiteRendezvous, ModeCheckedSiteLock,
    RendezvousError, SiteLifetimeObservation, SiteScope,
};
pub use retained_child::RetainedChild;

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
