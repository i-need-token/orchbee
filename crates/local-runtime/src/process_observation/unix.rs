//! Linux registers no process identities: a Session's liveness is its lifetime lock and its Terminal
//! Host's lock, so a record written here leaves every role unregistered. No observation is ever
//! made and an identified registration stays unknown; a Host's absence is its free lock, which the
//! rendezvous observes.
use crate::{
    durable_sessions::RecordWriter,
    process_registration::{ProcessObservation, ROLES, RecoveryVerdict, SessionProcesses},
};

pub(crate) struct ProcessSnapshot {
    observations: [ProcessObservation; ROLES.len()],
}
impl ProcessSnapshot {
    pub(crate) fn observe(_processes: &SessionProcesses) -> Self {
        Self {
            observations: [ProcessObservation::Unknown; ROLES.len()],
        }
    }
    /// Cold recovery's verdict on the record `processes` was read from under `_writer`.
    pub(crate) fn verdict(
        &self,
        processes: &SessionProcesses,
        _writer: &RecordWriter,
    ) -> RecoveryVerdict {
        processes.status(self.observations)
    }
}
