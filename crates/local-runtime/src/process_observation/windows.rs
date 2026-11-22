//! Read-only exact process observations. These handles never carry termination rights. A process
//! this account may not open, such as another account's or a protected one, is decided from the
//! system's process list, which needs no right to any process: a live process keeps its ID and the
//! program it started from until it ends, so an ID that no process holds, or that a process running
//! another program holds, names no live process that started from the expected program.
use crate::{
    BlockerRole, CohortArtifact,
    durable_sessions::RecordWriter,
    process_registration::{
        ProcessIdentity, ProcessObservation, ProcessRole, RecoveryVerdict, Registration,
        SessionProcesses,
    },
};
use std::io;
use winsafe::{HPROCESS, HPROCESSLIST, co, guard::CloseHandleGuard};

pub(crate) fn identity(handle: &HPROCESS) -> io::Result<ProcessIdentity> {
    let pid = handle.GetProcessId().map_err(io::Error::other)?;
    let (creation, _, _, _) = handle.GetProcessTimes().map_err(io::Error::other)?;
    ProcessIdentity::new(pid, u64::from(creation)).ok_or_else(|| io::ErrorKind::InvalidData.into())
}

pub(crate) fn current_identity() -> io::Result<ProcessIdentity> {
    identity(&HPROCESS::GetCurrentProcess())
}

pub(crate) struct ProcessSnapshot {
    observations: [ProcessObservation; 3],
    // Retaining the opened objects keeps each observation tied to the queried process.
    _handles: Vec<CloseHandleGuard<HPROCESS>>,
}
impl ProcessSnapshot {
    pub(crate) fn observe(processes: &SessionProcesses) -> Self {
        let mut snapshot = Self {
            observations: [ProcessObservation::Gone; 3],
            _handles: Vec::new(),
        };
        for (index, role) in [ProcessRole::Server, ProcessRole::Worker, ProcessRole::Host]
            .into_iter()
            .enumerate()
        {
            if let Some(expected) = processes.registration(role).identity() {
                let (observation, handle) = observe(expected, program(role));
                snapshot.observations[index] = observation;
                if let Some(handle) = handle {
                    snapshot._handles.push(handle);
                }
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

/// What the Host registration of `processes` names now. A live process with the registered ID but
/// another creation time is another process, so the registered Host is gone.
pub(crate) fn host_presence(processes: &SessionProcesses) -> crate::HostPresence {
    match processes.registration(ProcessRole::Host) {
        Registration::NotStarted => crate::HostPresence::NotStarted,
        Registration::Spawning => crate::HostPresence::Spawning,
        Registration::Identified { identity } => {
            match observe(identity, program(ProcessRole::Host)).0 {
                ProcessObservation::Gone => crate::HostPresence::Gone,
                ProcessObservation::Present(observed) if observed == identity => {
                    crate::HostPresence::Running(Some((identity.pid(), identity.creation_time())))
                }
                ProcessObservation::Present(_) => crate::HostPresence::Gone,
                ProcessObservation::Unknown => crate::HostPresence::Unknown,
            }
        }
    }
}

/// Whether `role`'s registration has no live process: never started, or exact identity gone.
/// A reserved spawn and an unknown observation are not absence.
pub(crate) fn role_absent(processes: &SessionProcesses, role: ProcessRole) -> bool {
    match processes.registration(role) {
        Registration::NotStarted => true,
        Registration::Spawning => false,
        Registration::Identified { identity } => {
            observe(identity, program(role)).0 == ProcessObservation::Gone
        }
    }
}

/// The program a registered `role` starts from.
fn program(role: ProcessRole) -> CohortArtifact {
    BlockerRole::of(role).program()
}

/// What the ID of `expected`, a process started from `program`, names now, opened with the rights
/// to query and wait alone. A live process returns the handle that keeps its ID from naming another
/// process while the caller decides about it.
pub(crate) fn observe(
    expected: ProcessIdentity,
    program: CohortArtifact,
) -> (ProcessObservation, Option<CloseHandleGuard<HPROCESS>>) {
    let handle = match HPROCESS::OpenProcess(
        co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::SYNCHRONIZE,
        false,
        expected.pid(),
    ) {
        Ok(handle) => handle,
        Err(error) if error == co::ERROR::INVALID_PARAMETER => {
            return (ProcessObservation::Gone, None);
        }
        Err(_) => return (unopened(expected.pid(), &program.file_name()), None),
    };
    let observation = match handle.WaitForSingleObject(Some(0)) {
        Ok(co::WAIT::OBJECT_0) => ProcessObservation::Gone,
        Ok(co::WAIT::TIMEOUT) => {
            identity(&handle).map_or(ProcessObservation::Unknown, ProcessObservation::Present)
        }
        _ => ProcessObservation::Unknown,
    };
    (observation, Some(handle))
}

/// A process ID this account could not open, for a process started from the program file
/// `program`: gone unless the process that holds it runs that program, which stays unknown, as
/// does a process list that cannot be read.
fn unopened(pid: u32, program: &str) -> ProcessObservation {
    let holder = || -> io::Result<Option<String>> {
        let mut processes = HPROCESSLIST::CreateToolhelp32Snapshot(co::TH32CS::SNAPPROCESS, None)
            .map_err(io::Error::other)?;
        for process in processes.iter_processes() {
            let process = process.map_err(io::Error::other)?;
            if process.th32ProcessID == pid {
                return Ok(Some(process.szExeFile()));
            }
        }
        Ok(None)
    };
    match holder() {
        Ok(Some(running)) if running.eq_ignore_ascii_case(program) => ProcessObservation::Unknown,
        Ok(_) => ProcessObservation::Gone,
        Err(_) => ProcessObservation::Unknown,
    }
}

#[cfg(test)]
mod tests;
