//! Client admission adapter: only the managed Session composition supplies a lifecycle owner, which
//! records Session stops and serves the `drain` and `exit` control requests, and the update job,
//! which answers a Client's check for updates.
use super::application_connection::{Effect, Refusal};
use crate::agent_service::AgentService;
use orchbee_connection_handshake::ProcessState;
use orchbee_local_client_protocol::{
    SessionStopRequest, SessionStopResult, SessionStopStatus, StopAction, StopId,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionStopOutcome, SessionStopRejection, UpdateCheckOutcome,
    UpdateCheckRejection, WorkbenchSessionStopRequest, WorkbenchSessionStopResult,
    WorkbenchUpdateCheckRequest, WorkbenchUpdateCheckResult,
};

pub(super) struct StopControl<'a> {
    pub(super) agents: &'a AgentService,
    pub(super) shutdown: Option<&'a crate::session_shutdown::SessionShutdown>,
    /// The update job a check for updates reaches; none where the Server runs no such job.
    pub(super) updates: Option<&'a crate::update_job::UpdateJob>,
}

pub(super) struct Lease<'a> {
    _record: Option<crate::session_shutdown::RecordLease<'a>>,
    _control: &'a StopControl<'a>,
}

// Keep the same explicit admission-release boundary on adapters with no lifecycle owner.
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self._record.take();
    }
}

impl<'a> StopControl<'a> {
    /// The state a `query` reports; a Server without a lifecycle owner is always `ready`.
    pub(super) fn process_state(&self) -> ProcessState {
        if let Some(owner) = self.shutdown {
            return owner.process_state();
        }
        ProcessState::Ready
    }

    /// Whether this Server serves `handover`: a managed Windows Server, whose restart keeps its
    /// terminals.
    pub(super) fn serves_handover(&self) -> bool {
        cfg!(any(windows, target_os = "macos")) && self.shutdown.is_some()
    }

    /// The state the handshake weighs an offer against.
    pub(super) fn offer_state(&self) -> ProcessState {
        if let Some(owner) = self.shutdown {
            return owner.offer_state();
        }
        ProcessState::Ready
    }

    /// Serves `drain` and returns the state its result reports, or `None` without a lifecycle
    /// owner.
    pub(super) fn drain(&self) -> Option<ProcessState> {
        if let Some(owner) = self.shutdown {
            return Some(owner.drain());
        }
        None
    }

    /// Records `exit` and, once the lifecycle owner ends the process, sends `reply` before the
    /// process may end. No reply follows when the end does not arrive by `deadline`. Returns false
    /// without a lifecycle owner, having recorded nothing.
    pub(super) fn exit_before(&self, deadline: std::time::Instant, reply: impl FnOnce()) -> bool {
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

    /// Delivers one `handover` step to the lifecycle owner and returns its answer: `Some(None)` when
    /// none came by `deadline`, and `None` without a lifecycle owner, having delivered nothing.
    #[cfg(any(windows, target_os = "macos"))]
    pub(super) fn handover(
        &self,
        step: orchbee_connection_handshake::HandoverStep,
        ticket: orchbee_connection_handshake::HandoverTicket,
        deadline: std::time::Instant,
    ) -> Option<Option<orchbee_connection_handshake::HandoverOutcome>> {
        self.shutdown
            .map(|owner| owner.handover(step, ticket, deadline))
    }

    pub(super) fn unmanaged(agents: &'a AgentService) -> Self {
        Self {
            agents,
            shutdown: None,
            updates: None,
        }
    }

    pub(super) fn request(&self, request: &SessionStopRequest) -> SessionStopResult {
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

    /// Answers an admitted Client's check for updates for the Session this Server holds. The job
    /// answers at once under its own lock; a Server without one answers `unavailable`.
    pub(super) fn update_check(
        &self,
        request: &WorkbenchUpdateCheckRequest,
        session_matches: Option<bool>,
    ) -> WorkbenchUpdateCheckResult {
        let outcome = if session_matches == Some(true) {
            self.updates
                .map_or(UpdateCheckOutcome::Unavailable {}, |job| {
                    job.answer(request.action)
                })
        } else {
            UpdateCheckOutcome::Rejected {
                reason: UpdateCheckRejection::SessionMismatch,
            }
        };
        WorkbenchUpdateCheckResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            outcome,
        }
    }

    /// Admits one application record. A `drain` refuses intents only; a Session stop or an `exit`
    /// refuses every record. A restart's hold refuses intents other than terminal records as a
    /// `drain` does.
    pub(super) fn admit(&'a self, effect: Effect) -> Result<Lease<'a>, Refusal> {
        // The course only advances, so a refusal observed as `draining` here was a drain, and one
        // observed as `ready` was a restart's hold.
        let refusal = |owner: &crate::session_shutdown::SessionShutdown| match owner.process_state()
        {
            ProcessState::Draining | ProcessState::Ready => Refusal::Draining,
            _ => Refusal::Stopping,
        };
        let record = match self.shutdown {
            Some(owner) => Some(match effect {
                Effect::Read => owner.enter_read().ok_or(Refusal::Stopping)?,
                Effect::RuntimeIntent => owner.enter().ok_or_else(|| refusal(owner))?,
                Effect::Intent => owner.enter_intent().ok_or_else(|| refusal(owner))?,
            }),
            None => None,
        };
        Ok(Lease {
            _record: record,
            _control: self,
        })
    }

    /// Admits a new Client connection, which a `drain` refuses like an intent, and a restart's hold
    /// when the Client did not offer `reconnect`.
    pub(super) fn enter(&'a self, reconnects: bool) -> Option<Lease<'a>> {
        let record = match self.shutdown {
            Some(owner) => Some(owner.enter_client(reconnects)?),
            None => None,
        };
        Some(Lease {
            _record: record,
            _control: self,
        })
    }
}
