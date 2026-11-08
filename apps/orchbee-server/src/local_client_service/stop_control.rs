//! Client admission adapter: only the managed Windows composition supplies a lifecycle owner, which
//! records Session stops and serves the `drain` and `exit` control requests.
use crate::agent_service::AgentService;
use orchbee_connection_handshake::ProcessState;
use orchbee_local_client_protocol::{SessionStopRequest, SessionStopResult, SessionStopStatus};

pub(super) struct StopControl<'a> {
    pub(super) agents: &'a AgentService,
    #[cfg(windows)]
    pub(super) shutdown: Option<&'a crate::session_shutdown::SessionShutdown>,
}

pub(super) struct Lease<'a> {
    #[cfg(windows)]
    _record: Option<crate::session_shutdown::RecordLease<'a>>,
    _control: &'a StopControl<'a>,
}

// Keep the same explicit admission-release boundary on adapters with no lifecycle owner.
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        #[cfg(windows)]
        self._record.take();
    }
}

impl<'a> StopControl<'a> {
    /// The state a `query` reports; a Server without a lifecycle owner is always `ready`.
    pub(super) fn process_state(&self) -> ProcessState {
        #[cfg(windows)]
        if let Some(owner) = self.shutdown {
            return owner.process_state();
        }
        ProcessState::Ready
    }

    /// The state the handshake weighs an offer against.
    pub(super) fn offer_state(&self) -> ProcessState {
        #[cfg(windows)]
        if let Some(owner) = self.shutdown {
            return owner.offer_state();
        }
        ProcessState::Ready
    }

    /// Serves `drain` and returns the state its result reports, or `None` without a lifecycle
    /// owner.
    pub(super) fn drain(&self) -> Option<ProcessState> {
        #[cfg(windows)]
        if let Some(owner) = self.shutdown {
            return Some(owner.drain());
        }
        None
    }

    /// Records `exit` and, once the lifecycle owner ends the process, sends `reply` before the
    /// process may end. No reply follows when the end does not arrive by `deadline`. Returns false
    /// without a lifecycle owner, having recorded nothing.
    pub(super) fn exit_before(&self, deadline: std::time::Instant, reply: impl FnOnce()) -> bool {
        #[cfg(windows)]
        if let Some(owner) = self.shutdown {
            let slot = owner.exit();
            if slot.ended_before(deadline) {
                reply();
            }
            return true;
        }
        let _ = (deadline, reply);
        false
    }

    pub(super) fn unmanaged(agents: &'a AgentService) -> Self {
        Self {
            agents,
            #[cfg(windows)]
            shutdown: None,
        }
    }

    pub(super) fn request(&self, request: &SessionStopRequest) -> SessionStopResult {
        #[cfg(windows)]
        if let Some(owner) = self.shutdown {
            return owner.request(request);
        }
        SessionStopResult {
            id: request.id,
            status: SessionStopStatus::Unmanaged,
        }
    }

    pub(super) fn enter(&'a self) -> Option<Lease<'a>> {
        #[cfg(windows)]
        let record = match self.shutdown {
            Some(owner) => Some(owner.enter()?),
            None => None,
        };
        Some(Lease {
            #[cfg(windows)]
            _record: record,
            _control: self,
        })
    }
}
