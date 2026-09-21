//! Private native development runtime boundaries; applications own startup policy.
#[cfg(unix)]
mod cohort;
#[cfg(unix)]
mod rendezvous;
#[cfg(unix)]
mod retained_child;

#[cfg(unix)]
pub use cohort::{NativeArtifact, NativeCohort, ObservationAdaptersSidecar};
#[cfg(unix)]
pub use rendezvous::{
    CLIENT_PENDING_SOCKET_NAME, EntryKind, EntryObservation, HOST_ENDPOINT_ID, HOST_ENDPOINT_NAME,
    HOST_SCOPE_NAME, LockAttempt, LockKind, ModeCheckedLocalSiteRendezvous, ModeCheckedSiteLock,
    RendezvousError, SiteLifetimeObservation, SiteScope,
};
#[cfg(unix)]
pub use retained_child::RetainedChild;

#[cfg(target_os = "linux")]
mod linux_permissions;
