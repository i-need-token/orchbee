//! POSIX registers no process identities: a Session's liveness is its lifetime lock alone, so a
//! record written here leaves every role unregistered. No observation is ever made, an identified
//! registration stays unknown, and no role is reported absent, since an unregistered role proves
//! nothing about a process that may run.
use crate::process_registration::{
    ProcessObservation, ProcessRole, RecoveryVerdict, SessionProcesses,
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
    pub(crate) fn verdict(&self, processes: &SessionProcesses) -> RecoveryVerdict {
        processes.status(self.observations)
    }
}

/// Never absent: without registration nothing proves the role's process gone.
pub(crate) fn role_absent(_processes: &SessionProcesses, _role: ProcessRole) -> bool {
    false
}
