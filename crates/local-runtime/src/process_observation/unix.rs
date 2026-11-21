//! POSIX registers no process identities: a Session's liveness is its lifetime lock and its Terminal
//! Host's lock, so a record written here leaves every role unregistered. No observation is ever
//! made and an identified registration stays unknown; a Host's absence is its free lock, which the
//! rendezvous observes.
use crate::{
    durable_sessions::RecordWriter,
    process_registration::{ProcessObservation, RecoveryVerdict, SessionProcesses},
};

pub(crate) struct ProcessSnapshot {
    observations: [ProcessObservation; 3],
}
impl ProcessSnapshot {
    pub(crate) fn observe(_processes: &SessionProcesses) -> Self {
        Self {
            observations: [ProcessObservation::Unknown; 3],
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
