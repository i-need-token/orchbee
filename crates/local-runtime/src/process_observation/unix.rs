//! POSIX registers no process identities: a Session's liveness is its lifetime lock and its Terminal
//! Host's lock, so a record written here leaves every role unregistered. No observation is ever
//! made and an identified registration stays unknown; a Host's absence is its free lock, which the
//! rendezvous observes.
use crate::process_registration::{ProcessObservation, RecoveryVerdict, SessionProcesses};

pub(crate) struct ProcessSnapshot {
    observations: [ProcessObservation; 3],
}
impl ProcessSnapshot {
    pub(crate) fn observe(_processes: &SessionProcesses) -> Self {
        Self {
            observations: [ProcessObservation::Unknown; 3],
        }
    }
    pub(crate) fn verdict(&self, processes: &SessionProcesses) -> RecoveryVerdict {
        processes.status(self.observations)
    }
}
