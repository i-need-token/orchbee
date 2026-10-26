//! Windows retains its private runtime observations and refuses POSIX metadata observations.
use super::{Entry, Unsupported};
use orchbee_local_runtime::{ActivationState, EntryObservation, ModeCheckedLocalSessionRendezvous};
use std::{
    io,
    path::{Path, PathBuf},
};

pub(in crate::cli::server_diagnose) const PRIVATE_RUNTIME: bool = true;

pub(in crate::cli::server_diagnose) fn unsupported_field(key: &str) -> bool {
    matches!(
        key,
        "pathBytesHex" | "kind" | "ownerMatch" | "mode" | "size" | "linkCount"
    )
}

pub(in crate::cli::server_diagnose) fn metadata_base() -> io::Result<PathBuf> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(in crate::cli::server_diagnose) fn metadata_scope() -> io::Result<String> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(in crate::cli::server_diagnose) fn observe_entry(
    _session: &ModeCheckedLocalSessionRendezvous,
    _entry: Entry,
) -> io::Result<EntryObservation> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(in crate::cli::server_diagnose) fn activation_state(
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<ActivationState> {
    session.activation_state()
}

pub(in crate::cli::server_diagnose) fn activation_lock_exists(
    session: &ModeCheckedLocalSessionRendezvous,
) -> io::Result<bool> {
    session.activation_lock_exists()
}

pub(in crate::cli::server_diagnose) fn endpoint_exists_before(
    path: &std::path::Path,
    deadline: std::time::Instant,
) -> io::Result<Option<bool>> {
    orchbee_local_ipc::endpoint_exists_before(path, deadline)
}

pub(in crate::cli::server_diagnose) fn observation_endpoint_path(
    runtime: &Path,
) -> Result<PathBuf, Unsupported> {
    Ok(ModeCheckedLocalSessionRendezvous::observation_endpoint_path(runtime))
}
