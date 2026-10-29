//! One creation's admission inputs, read under the Worker registry guard: the reserved request
//! identity and lease, the Worker generation the creation targets, and the launch capture that
//! carries the System shell that same generation reported. Production takes the generation and its
//! shell from one registry request handle, so a creation cannot pair them across generations.

use orchbee_capability_node_protocol::LeaseExpiresAtUnixMilliseconds;
use orchbee_client_server_protocol::RequestId;

use super::{LaunchCapture, WorkerPlacement};
use crate::worker_obcn_carrier::WorkerRequestHandle;

pub(crate) struct CreateInputs {
    candidate: Option<RequestId>,
    expiry: Option<LeaseExpiresAtUnixMilliseconds>,
    worker: Option<WorkerPlacement>,
    capture: LaunchCapture,
}

impl CreateInputs {
    /// Targets the generation of `handle` and names its default launches with the System shell
    /// that generation reported. `preferences` is the capture read before any lock.
    pub(crate) fn admitted_by(
        candidate: Option<RequestId>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        handle: Option<&WorkerRequestHandle>,
        preferences: LaunchCapture,
    ) -> Self {
        Self {
            candidate,
            expiry,
            worker: handle.and_then(WorkerPlacement::of),
            capture: preferences
                .with_system_shell(handle.and_then(|handle| handle.system_shell().cloned())),
        }
    }

    /// Inputs for a coordinator driven without a Worker registry.
    #[cfg(test)]
    pub(crate) const fn new(
        candidate: Option<RequestId>,
        expiry: Option<LeaseExpiresAtUnixMilliseconds>,
        worker: Option<WorkerPlacement>,
        capture: LaunchCapture,
    ) -> Self {
        Self {
            candidate,
            expiry,
            worker,
            capture,
        }
    }

    /// Stands in for the System shell a registered generation reports.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_system_shell(
        mut self,
        system_shell: Option<orchbee_workbench_protocol::PresentationLaunchProgram>,
    ) -> Self {
        self.capture = self.capture.with_system_shell(system_shell);
        self
    }

    pub(crate) const fn worker(&self) -> Option<WorkerPlacement> {
        self.worker
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Option<RequestId>,
        Option<LeaseExpiresAtUnixMilliseconds>,
        Option<WorkerPlacement>,
        LaunchCapture,
    ) {
        (self.candidate, self.expiry, self.worker, self.capture)
    }
}
