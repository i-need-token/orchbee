//! Read-only exact process observations. These handles never carry termination rights.
use crate::process_registration::{
    ProcessIdentity, ProcessObservation, ProcessRole, RecoveryVerdict, SiteProcesses,
};
use std::io;
use winsafe::{HPROCESS, co, guard::CloseHandleGuard};

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
    pub(crate) fn observe(processes: &SiteProcesses) -> Self {
        let mut snapshot = Self {
            observations: [ProcessObservation::Gone; 3],
            _handles: Vec::new(),
        };
        for (index, role) in [ProcessRole::Server, ProcessRole::Worker, ProcessRole::Host]
            .into_iter()
            .enumerate()
        {
            if let Some(expected) = processes.registration(role).identity() {
                let (observation, handle) = observe(expected);
                snapshot.observations[index] = observation;
                if let Some(handle) = handle {
                    snapshot._handles.push(handle);
                }
            }
        }
        snapshot
    }
    pub(crate) fn verdict(&self, processes: &SiteProcesses) -> RecoveryVerdict {
        processes.status(self.observations)
    }
}

fn observe(expected: ProcessIdentity) -> (ProcessObservation, Option<CloseHandleGuard<HPROCESS>>) {
    // Windows' protected system IDs must never be classified absent from OpenProcess failure.
    if expected.pid() <= 4 {
        return (ProcessObservation::Unknown, None);
    }
    let handle = match HPROCESS::OpenProcess(
        co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::SYNCHRONIZE,
        false,
        expected.pid(),
    ) {
        Ok(handle) => handle,
        Err(error) if error == co::ERROR::INVALID_PARAMETER => {
            return (ProcessObservation::Gone, None);
        }
        Err(_) => return (ProcessObservation::Unknown, None),
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

#[cfg(test)]
mod tests;
