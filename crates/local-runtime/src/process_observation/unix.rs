//! Linux registers no process identities: a Session's liveness is its lifetime lock and its Terminal
//! Host's lock, so a record written here leaves every role unregistered. No observation is ever
//! made and an identified registration stays unknown; a Host's absence is its free lock, which the
//! rendezvous observes.
use crate::process_registration::{ProcessObservation, ROLES, SessionProcesses};

pub(crate) struct ProcessSnapshot {
    observations: [ProcessObservation; ROLES.len()],
}
impl ProcessSnapshot {
    pub(crate) fn observe(_processes: &SessionProcesses) -> Self {
        Self {
            observations: [ProcessObservation::Unknown; ROLES.len()],
        }
    }
    /// What the snapshot observed of each role, in `ROLES` order.
    pub(crate) const fn observations(&self) -> [ProcessObservation; ROLES.len()] {
        self.observations
    }
}
