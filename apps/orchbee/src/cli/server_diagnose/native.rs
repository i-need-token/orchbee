//! Native observation ports return host facts; the report workflows classify them.

pub(super) enum Entry {
    Marker,
    ActivationLock,
    ClientSocket,
    WorkerSocket,
    ClientPendingSocket,
    AgentObservationSocket,
    HostEndpoint,
    HostDirectory,
}

/// This host has no private Agent-observation endpoint spelling.
pub(super) struct Unsupported;

#[cfg(windows)]
#[path = "native/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "native/unix.rs"]
mod platform;

pub(super) use platform::{
    PRIVATE_RUNTIME, activation_lock_exists, activation_state, endpoint_exists_before,
    metadata_base, metadata_scope, observation_endpoint_path, observe_entry, unsupported_field,
};
