//! Windows retains its private runtime observations and refuses POSIX metadata observations.
use super::Entry;
use orchbee_local_runtime::{ActivationState, EntryObservation, ModeCheckedLocalSessionRendezvous};
use std::{io, path::PathBuf};

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
