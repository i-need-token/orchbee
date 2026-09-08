//! Private native development runtime boundaries; applications own startup policy.
#[cfg(unix)]
mod cohort;
#[cfg(unix)]
mod rendezvous;
#[cfg(unix)]
mod retained_child;

#[cfg(unix)]
pub use cohort::{NativeArtifact, NativeCohort};
#[cfg(unix)]
pub use rendezvous::{
    LockAttempt, LockKind, ModeCheckedLocalSiteRendezvous, ModeCheckedSiteLock, RendezvousError,
    SiteScope,
};
#[cfg(unix)]
pub use retained_child::RetainedChild;

#[cfg(target_os = "linux")]
mod linux_permissions;
