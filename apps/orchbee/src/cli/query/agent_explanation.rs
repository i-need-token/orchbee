//! The Explain reads `agent get --format json` adds after its own read, on the same admitted
//! Client and under the same deadline: a canonical session's derived explanation, or a terminal
//! Agent's detection-rule evaluation. Text output skips them. The read only adds to a get that
//! already answered, so a read that gives no explanation leaves its member null.
use super::{
    AgentExchangeError, AgentExplainOutcome, AgentReadExchange, AgentReadOperation,
    AgentReadRequest, AgentReadResult, AgentSessionId, BorrowedFd, Failure, FailureKind, Instant,
    LocalClient, ObapRequestId, ProtocolVersion, Snapshot, agent_protocol_invalid,
    agent_session_rejection, agent_unavailable_details, classify_agent_read, obap_request_id,
    request_id, terminal_agent,
};
use crate::local_client::MetadataExchangeReason;
use orchbee_agent_observation_protocol::{
    AgentGetOutcome, AgentLaunchBinding, AgentSessionExplanation,
};
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentFailure, PanelAgentOutcome, PanelAgentRequest,
    TerminalAgentExplanation,
};

/// The canonical `agent get` exchange: the get request, then, for a machine document whose get
/// read found the session, the Explain request of the same session.
pub(super) struct GetRead {
    pub(super) get: AgentReadRequest,
    pub(super) explain: Option<(ObapRequestId, AgentSessionId)>,
}

/// The Explain reply keeps its own transport error, classified as the Explain operation.
pub(super) type ExplainReply = (
    ObapRequestId,
    AgentSessionId,
    Result<AgentReadResult, AgentExchangeError>,
);

impl AgentReadExchange for GetRead {
    type Response = (AgentReadResult, Option<ExplainReply>);
    fn exchange(
        self,
        client: &mut LocalClient,
        total: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<Self::Response, AgentExchangeError> {
        let got = client.submit_agent_read_before_cancellable(self.get, total, wake)?;
        let found = matches!(&got, AgentReadResult::Get { result, .. }
            if matches!(result.outcome, AgentGetOutcome::Found { .. }));
        let explained = self
            .explain
            .filter(|_| found)
            .map(|(request_id, session_id)| {
                let reply = client.submit_agent_read_before_cancellable(
                    AgentReadRequest::Explain {
                        request_id: request_id.clone(),
                        session_id: session_id.clone(),
                    },
                    total,
                    wake,
                );
                (request_id, session_id, reply)
            });
        Ok((got, explained))
    }
}

/// Keeps a get that already answered when its Explain read gives no explanation for the target:
/// unavailable, not found, out of time or cut off leaves the member null. A denial, a refusal, a
/// protocol violation or a cancellation still fails the command.
fn auxiliary<T>(read: Result<T, Failure>) -> Result<Option<T>, Failure> {
    match read {
        Ok(explanation) => Ok(Some(explanation)),
        Err(failure)
            if matches!(
                failure.kind,
                FailureKind::ServiceUnavailable
                    | FailureKind::NotFound
                    | FailureKind::Timeout
                    | FailureKind::TransportLoss
            ) =>
        {
            Ok(None)
        }
        Err(failure) => Err(failure),
    }
}

/// The Explain read of one canonical session on an admitted Client.
pub(in crate::cli) fn session(
    client: &mut LocalClient,
    session_id: &AgentSessionId,
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<Option<AgentSessionExplanation>, Failure> {
    let request_id = obap_request_id()?;
    let reply = client.submit_agent_read_before_cancellable(
        AgentReadRequest::Explain {
            request_id: request_id.clone(),
            session_id: session_id.clone(),
        },
        total,
        wake,
    );
    session_answer((request_id, session_id.clone(), reply))
        .map_err(|failure| failure.failure_schema(crate::cli::schema::AGENT_FAILURE_ID))
}

/// Only a `found` explanation of the requested session answers; an unavailable or missing
/// session leaves the member null, while a refusal or another read's answer fails the command
/// with the Explain operation's failure.
pub(super) fn session_answer(
    reply: ExplainReply,
) -> Result<Option<AgentSessionExplanation>, Failure> {
    auxiliary(session_explanation(reply))
}

fn session_explanation(
    (request_id, session_id, reply): ExplainReply,
) -> Result<AgentSessionExplanation, Failure> {
    let operation = AgentReadOperation::Explain;
    let source = super::super::results::agent_source(&request_id);
    match reply.map_err(|error| classify_agent_read(error, operation))? {
        AgentReadResult::Explain { result, .. } => match result.outcome {
            AgentExplainOutcome::Found {
                session,
                explanation,
                ..
            } if session.snapshot.session_id == session_id => Ok(explanation),
            AgentExplainOutcome::Found { .. } => Err(agent_protocol_invalid(operation)),
            AgentExplainOutcome::Rejected { reason } => {
                Err(agent_session_rejection(operation, reason, &session_id).source(source))
            }
            AgentExplainOutcome::Unavailable { .. } => {
                Err(Failure::new(FailureKind::ServiceUnavailable)
                    .details(agent_unavailable_details(operation))
                    .source(source))
            }
        },
        AgentReadResult::List { .. } | AgentReadResult::Get { .. } => {
            Err(agent_protocol_invalid(operation))
        }
    }
}

/// The detection-rule evaluation of one terminal Agent, read under the snapshot's revision. An
/// Agent the Server has no detection rules for, such as one reported under a name outside the
/// provider registry with no detected process, has no evaluation and leaves the member null.
pub(in crate::cli) fn terminal(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    binding: AgentLaunchBinding,
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<Option<TerminalAgentExplanation>, Failure> {
    auxiliary(terminal_explanation(client, snapshot, binding, total, wake))
}

fn terminal_explanation(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    binding: AgentLaunchBinding,
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<TerminalAgentExplanation, Failure> {
    let id = request_id()?;
    let reply = client
        .panel_agent(
            PanelAgentRequest {
                protocol_version: ProtocolVersion,
                request_id: id,
                session_id: snapshot.session_id.clone(),
                action: PanelAgentAction::Explain {
                    binding,
                    input_area: false,
                },
            },
            total,
            wake,
        )
        .map_err(|error| {
            terminal_agent::failure(
                match error.reason {
                    MetadataExchangeReason::ConnectionUnavailable => {
                        FailureKind::ServiceUnavailable
                    }
                    MetadataExchangeReason::Encoding => FailureKind::InternalFailure,
                    MetadataExchangeReason::Deadline | MetadataExchangeReason::ResponseDeadline => {
                        FailureKind::Timeout
                    }
                    MetadataExchangeReason::Correlation | MetadataExchangeReason::Protocol => {
                        FailureKind::ProtocolInvalid
                    }
                    _ => FailureKind::TransportLoss,
                },
                "read-failed",
            )
        })?;
    match reply.outcome {
        PanelAgentOutcome::Explained {
            workbench_revision,
            explanation,
            ..
        } => {
            if workbench_revision != snapshot.revision {
                return Err(crate::cli::query::workbench_failure("workbench-changed"));
            }
            Ok(*explanation)
        }
        PanelAgentOutcome::Failed { reason } => Err(terminal_agent::failure(
            match reason {
                PanelAgentFailure::Denied => FailureKind::Denied,
                PanelAgentFailure::Unavailable | PanelAgentFailure::ResourceLimit => {
                    FailureKind::ServiceUnavailable
                }
                PanelAgentFailure::NotFound
                | PanelAgentFailure::Unbound
                | PanelAgentFailure::BindingChanged => FailureKind::NotFound,
                PanelAgentFailure::ResultTooLarge => FailureKind::OutputTooLarge,
                PanelAgentFailure::SessionMismatch
                | PanelAgentFailure::InvalidRequest
                | PanelAgentFailure::SourceLimit
                | PanelAgentFailure::HistoryLimit
                | PanelAgentFailure::RevisionExhausted
                | PanelAgentFailure::ClockOverflow => FailureKind::ProtocolInvalid,
            },
            match reason {
                PanelAgentFailure::NotFound
                | PanelAgentFailure::Unbound
                | PanelAgentFailure::BindingChanged => "binding-changed",
                PanelAgentFailure::Denied
                | PanelAgentFailure::Unavailable
                | PanelAgentFailure::SessionMismatch
                | PanelAgentFailure::InvalidRequest
                | PanelAgentFailure::SourceLimit
                | PanelAgentFailure::HistoryLimit
                | PanelAgentFailure::ResourceLimit
                | PanelAgentFailure::RevisionExhausted
                | PanelAgentFailure::ClockOverflow
                | PanelAgentFailure::ResultTooLarge => "read-failed",
            },
        )),
        PanelAgentOutcome::Titles { .. }
        | PanelAgentOutcome::PresentationTitles { .. }
        | PanelAgentOutcome::Identified { .. }
        | PanelAgentOutcome::PresentationsRead { .. }
        | PanelAgentOutcome::States { .. }
        | PanelAgentOutcome::Observed { .. }
        | PanelAgentOutcome::Read { .. }
        | PanelAgentOutcome::PresentationReported { .. }
        | PanelAgentOutcome::Reported { .. } => Err(terminal_agent::failure(
            FailureKind::ProtocolInvalid,
            "read-failed",
        )),
    }
}
