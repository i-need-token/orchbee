//! Client admission adapter: only the managed Windows composition supplies a lifecycle owner, which
//! records Session stops and serves the `drain` and `exit` control requests.
use super::application_connection::{Effect, Refusal};
use crate::agent_service::AgentService;
use orchbee_connection_handshake::ProcessState;
use orchbee_local_client_protocol::{
    SessionStopRequest, SessionStopResult, SessionStopStatus, StopAction, StopId,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionStopOutcome, SessionStopRejection, WorkbenchSessionStopRequest,
    WorkbenchSessionStopResult,
};

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

    /// Answers an admitted Client's Session stop, an intent record whose lease holds the stop back
    /// until this answer is ready. The stop is recorded under the Client's identity as a
    /// `server stop` start records it, and only for the Session this Server holds.
    pub(super) fn stop_session(
        &self,
        request: &WorkbenchSessionStopRequest,
        session_matches: Option<bool>,
    ) -> WorkbenchSessionStopResult {
        let outcome = if session_matches == Some(true) {
            // The identity's text is 32 hexadecimal digits that are not all zero.
            let id = StopId::new(request.stop_id.bytes()).expect("a nonzero stop identity");
            match self
                .request(&SessionStopRequest {
                    id,
                    action: StopAction::Start,
                })
                .status
            {
                SessionStopStatus::Unmanaged => SessionStopOutcome::Unmanaged {},
                // A start records its identity whenever no stop exists, so a pending or failed
                // stop is this identity's own; `not-recorded` answers only a status or a retry.
                SessionStopStatus::Pending { .. } | SessionStopStatus::Failed { .. } => {
                    SessionStopOutcome::Recorded {}
                }
                SessionStopStatus::Busy { .. } | SessionStopStatus::NotRecorded => {
                    SessionStopOutcome::Busy {}
                }
            }
        } else {
            SessionStopOutcome::Rejected {
                reason: SessionStopRejection::SessionMismatch,
            }
        };
        WorkbenchSessionStopResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            outcome,
        }
    }

    /// Admits one application record. A `drain` refuses intents only; a Session stop or an `exit`
    /// refuses every record.
    pub(super) fn admit(&'a self, effect: Effect) -> Result<Lease<'a>, Refusal> {
        #[cfg(windows)]
        let record = match self.shutdown {
            Some(owner) => Some(match effect {
                Effect::Read => owner.enter_read().ok_or(Refusal::Stopping)?,
                // The course only advances, so a refusal observed as `draining` here was a drain.
                Effect::Intent => owner.enter().ok_or_else(|| match owner.process_state() {
                    ProcessState::Draining => Refusal::Draining,
                    _ => Refusal::Stopping,
                })?,
            }),
            None => None,
        };
        #[cfg(not(windows))]
        let _ = effect;
        Ok(Lease {
            #[cfg(windows)]
            _record: record,
            _control: self,
        })
    }

    /// Admits a new Client connection, which a `drain` refuses like an intent.
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
