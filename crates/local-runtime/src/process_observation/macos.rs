//! Exact process observations on macOS, by process ID and start time. They hold nothing open: no
//! handle keeps a POSIX process ID from naming a later process, so a caller that acts on a process
//! compares its start time again right before it does. A process of another account is decided from
//! the program it runs, which needs no right to that process: a live process keeps its ID and the
//! program it started from until it ends, so an ID that a process running another program holds
//! names no live process that started from the expected program.
use crate::{
    BlockerRole, CohortArtifact,
    durable_sessions::RecordWriter,
    process_identity_native::{self as native, Started},
    process_registration::{
        ProcessIdentity, ProcessObservation, ProcessRole, ROLES, RecoveryVerdict, Registration,
        SessionProcesses,
    },
};
use std::io;

/// The live process `pid` names: its ID and start time.
pub(crate) fn identity(pid: u32) -> io::Result<ProcessIdentity> {
    match native::started(pid)? {
        Started::At(start) => {
            ProcessIdentity::new(pid, start).ok_or_else(|| io::ErrorKind::InvalidData.into())
        }
        Started::Gone => Err(io::ErrorKind::NotFound.into()),
        Started::Denied => Err(io::ErrorKind::PermissionDenied.into()),
    }
}

pub(crate) fn current_identity() -> io::Result<ProcessIdentity> {
    identity(std::process::id())
}

pub(crate) struct ProcessSnapshot {
    observations: [ProcessObservation; ROLES.len()],
}
impl ProcessSnapshot {
    pub(crate) fn observe(processes: &SessionProcesses) -> Self {
        let mut snapshot = Self {
            observations: [ProcessObservation::Gone; ROLES.len()],
        };
        for (index, role) in ROLES.into_iter().enumerate() {
            if let Some(expected) = processes.registration(role).identity() {
                snapshot.observations[index] = observe(expected, program(role));
            }
        }
        snapshot
    }
    /// Cold recovery's verdict on the record `processes` was read from under `_writer`, the lock a
    /// registered child's creator holds across its whole attempt.
    pub(crate) fn verdict(
        &self,
        processes: &SessionProcesses,
        _writer: &RecordWriter,
    ) -> RecoveryVerdict {
        processes.status(self.observations)
    }
}

impl ProcessSnapshot {
    /// The identified Hosts of `processes` this snapshot found running with their exact identities.
    pub(crate) fn present_hosts(
        &self,
        processes: &SessionProcesses,
    ) -> Vec<crate::process_registration::KeptHost> {
        processes
            .identified_hosts()
            .into_iter()
            .filter(|kept| {
                ROLES
                    .into_iter()
                    .position(|role| role == ProcessRole::Host(kept.slot))
                    .and_then(|index| self.observations.get(index))
                    == Some(&ProcessObservation::Present(kept.identity))
            })
            .collect()
    }
}

/// What the registration of `role`, the Host or the Keeper, names now, in the Host's terms.
pub(crate) fn presence(processes: &SessionProcesses, role: ProcessRole) -> crate::HostPresence {
    match processes.registration(role) {
        Registration::NotStarted => crate::HostPresence::NotStarted,
        Registration::Spawning => crate::HostPresence::Spawning,
        Registration::Identified { identity, .. } => match observe(identity, program(role)) {
            ProcessObservation::Gone => crate::HostPresence::Gone,
            ProcessObservation::Present(observed) if observed == identity => {
                crate::HostPresence::Running(Some((identity.pid(), identity.creation_time())))
            }
            ProcessObservation::Present(_) => crate::HostPresence::Gone,
            ProcessObservation::Unknown => crate::HostPresence::Unknown,
        },
    }
}

/// Whether `role`'s registration has no live process: never started, or exact identity gone.
/// A reserved spawn and an unknown observation are not absence.
pub(crate) fn role_absent(processes: &SessionProcesses, role: ProcessRole) -> bool {
    match processes.registration(role) {
        Registration::NotStarted => true,
        Registration::Spawning => false,
        Registration::Identified { identity, .. } => {
            observe(identity, program(role)) == ProcessObservation::Gone
        }
    }
}

/// The full path of the program `process`, an ID and start time, runs: none once it ended, when its
/// ID names a later process, or when its program cannot be read. The start time is compared before
/// and after the read, so the path is that process's.
pub fn running_program(process: (u32, u64)) -> Option<std::path::PathBuf> {
    let same =
        || matches!(native::started(process.0), Ok(Started::At(start)) if start == process.1);
    if !same() {
        return None;
    }
    let path = native::program_path(process.0)?;
    same().then_some(path)
}

/// The child processes of `parent` that still run, each with its start time in microseconds, or
/// `None` when that cannot be read; a child that ended and waits to be reaped is left out.
///
/// # Errors
///
/// Returns an error when the children cannot be listed.
pub fn running_children(parent: u32) -> io::Result<Vec<(u32, Option<u64>)>> {
    let mut running = Vec::new();
    for pid in native::children(parent)? {
        match native::started(pid) {
            Ok(Started::Gone) => {}
            Ok(Started::At(start)) => running.push((pid, Some(start))),
            Ok(Started::Denied) | Err(_) => running.push((pid, None)),
        }
    }
    Ok(running)
}

/// The program a registered `role` starts from.
fn program(role: ProcessRole) -> CohortArtifact {
    BlockerRole::of(role).program()
}

/// What the ID of `expected`, a process started from `program`, names now. A process that ended
/// and waits to be reaped is gone.
pub(crate) fn observe(expected: ProcessIdentity, program: CohortArtifact) -> ProcessObservation {
    match native::started(expected.pid()) {
        Ok(Started::Gone) => ProcessObservation::Gone,
        Ok(Started::At(start)) => ProcessIdentity::new(expected.pid(), start)
            .map_or(ProcessObservation::Unknown, ProcessObservation::Present),
        Ok(Started::Denied) => denied(expected.pid(), &program.file_name()),
        Err(_) => ProcessObservation::Unknown,
    }
}

/// A process ID that another account's process holds, for a process started from the program file
/// `program`: gone unless that process runs a program of that name, which stays unknown, as does a
/// program that cannot be read.
fn denied(pid: u32, program: &str) -> ProcessObservation {
    match native::program_file_name(pid) {
        Some(running) if running.eq_ignore_ascii_case(program) => ProcessObservation::Unknown,
        Some(_) => ProcessObservation::Gone,
        None => ProcessObservation::Unknown,
    }
}
