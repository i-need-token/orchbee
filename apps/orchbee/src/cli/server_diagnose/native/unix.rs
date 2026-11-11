//! Unix preserves descriptor-based metadata facts and refuses Windows private activation reads.
use super::{Entry, Unsupported};
use orchbee_local_runtime::{ActivationState, EntryObservation, ModeCheckedLocalSessionRendezvous};
use std::{
    io,
    path::{Path, PathBuf},
};

pub(in crate::cli::server_diagnose) const PRIVATE_RUNTIME: bool = false;

pub(in crate::cli::server_diagnose) fn unsupported_field(key: &str) -> bool {
    matches!(
        key,
        "pathUtf16" | "private" | "activation" | "stopRequestId"
    )
}

/// The selected runtime base, as activation names it.
pub(in crate::cli::server_diagnose) fn metadata_base() -> io::Result<PathBuf> {
    crate::local_activation::runtime_location().map(|(base, _)| base)
}

/// The selected Session scope, as activation names it.
pub(in crate::cli::server_diagnose) fn metadata_scope() -> io::Result<String> {
    crate::local_activation::runtime_location().map(|(_, scope)| scope)
}

pub(in crate::cli::server_diagnose) fn observe_entry(
    session: &ModeCheckedLocalSessionRendezvous,
    entry: Entry,
) -> io::Result<EntryObservation> {
    match entry {
        Entry::Marker => session.observe_activation_marker(),
        Entry::ActivationLock => session.observe_activation_lock(),
        Entry::ClientSocket => session.observe_client_socket(),
        Entry::WorkerSocket => session.observe_worker_socket(),
        Entry::ClientPendingSocket => session.observe_client_pending_socket(),
        Entry::AgentObservationSocket => session.observe_agent_observation_socket(),
        Entry::HostEndpoint => session.observe_host_endpoint(),
        Entry::HostDirectory => session.observe_host_directory(),
    }
}

pub(in crate::cli::server_diagnose) fn activation_state(
    _session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<ActivationState> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(in crate::cli::server_diagnose) fn activation_lock_exists(
    _session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<bool> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(in crate::cli::server_diagnose) fn endpoint_exists_before(
    _path: &std::path::Path,
    _deadline: std::time::Instant,
) -> io::Result<Option<bool>> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(in crate::cli::server_diagnose) fn observation_endpoint_path(
    _runtime: &Path,
) -> Result<PathBuf, Unsupported> {
    Err(Unsupported)
}
