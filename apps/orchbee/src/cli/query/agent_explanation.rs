//! One armed read owns target resolution and either canonical or physical explanation.
use super::{
    AgentExchangeError, AgentExplainOutcome, AgentQueryError, AgentReadExchange,
    AgentReadOperation, AgentReadRequest, AgentReadResult, BorrowedFd, Failure, FailureKind,
    Instant, LocalClient, ObapRequestId, Projection, ProtocolVersion, Snapshot, agent_armed_run,
    agent_protocol_invalid, agent_session_rejection, agent_target, agent_unavailable_details,
    classify_attempt, request_id, terminal_agent,
};
use crate::local_client::workbench::metadata_exchange::MetadataExchangeReason;
use orchbee_agent_observation_protocol::AgentNameTarget;
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentFailure, PanelAgentOutcome, PanelAgentRequest,
};

struct ExplainRead {
    target: AgentNameTarget,
    snapshot: Option<Snapshot>,
    request_id: ObapRequestId,
}

pub(crate) fn agent_explain(
    target: &crate::invocation::AgentTarget,
) -> Result<(serde_json::Value, Projection), AgentQueryError> {
    let operation = AgentReadOperation::Explain;
    let (_, attempt) = agent_armed_run(operation, |client, request_id, total, wake| {
        let target = agent_target::resolve_named(client, target, None, total, wake)?.target;
        let snapshot = if matches!(target, AgentNameTarget::Terminal { .. }) {
            Some(agent_target::read_snapshot(client, total, wake)?)
        } else {
            None
        };
        Ok(ExplainRead {
            target,
            snapshot,
            request_id,
        })
    })?;
    classify_attempt(attempt, operation)?.map_err(AgentQueryError::Read)
}

impl AgentReadExchange for ExplainRead {
    type Response = Result<(serde_json::Value, Projection), Failure>;
    fn exchange(
        self,
        client: &mut LocalClient,
        total: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<Self::Response, AgentExchangeError> {
        match self.target {
            AgentNameTarget::Session { session_id } => {
                let reply = client.submit_agent_read_before_cancellable(
                    AgentReadRequest::Explain {
                        request_id: self.request_id.clone(),
                        session_id: session_id.clone(),
                    },
                    total,
                    wake,
                )?;
                let operation = AgentReadOperation::Explain;
                let source = super::super::results::agent_source(&self.request_id);
                Ok(match reply {
                    AgentReadResult::Explain { result, .. } => match result.outcome {
                        AgentExplainOutcome::Found { ref session, .. }
                            if session.snapshot.session_id == session_id =>
                        {
                            Ok((source, Projection::AgentExplain(result.outcome)))
                        }
                        AgentExplainOutcome::Rejected { reason } => {
                            Err(agent_session_rejection(operation, reason, &session_id)
                                .source(source))
                        }
                        AgentExplainOutcome::Unavailable { .. } => {
                            Err(Failure::new(FailureKind::ServiceUnavailable)
                                .details(agent_unavailable_details(operation))
                                .source(source))
                        }
                        _ => Err(agent_protocol_invalid(operation)),
                    },
                    _ => Err(agent_protocol_invalid(operation)),
                })
            }
            AgentNameTarget::Terminal { binding } => Ok((|| {
                let snapshot = self.snapshot.ok_or_else(|| {
                    terminal_agent::failure(FailureKind::InternalFailure, "read-failed")
                })?;
                let id = request_id()?;
                let reply = client
                    .panel_agent(
                        PanelAgentRequest {
                            protocol_version: ProtocolVersion,
                            request_id: id.clone(),
                            site_id: snapshot.site_id.clone(),
                            action: PanelAgentAction::Explain { binding },
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
                                MetadataExchangeReason::Deadline
                                | MetadataExchangeReason::ResponseDeadline => FailureKind::Timeout,
                                MetadataExchangeReason::Correlation
                                | MetadataExchangeReason::Protocol => FailureKind::ProtocolInvalid,
                                _ => FailureKind::TransportLoss,
                            },
                            "read-failed",
                        )
                    })?;
                match reply.outcome {
                    PanelAgentOutcome::Explained {
                        workbench_revision,
                        report_revision,
                        explanation,
                    } => {
                        if workbench_revision != snapshot.revision {
                            return Err(crate::cli::workbench_agents::failure("workbench-changed"));
                        }
                        Ok((
                            serde_json::json!({"kind":"panel-agent-explain","sessionId":snapshot.site_id,"workbenchRevision":workbench_revision,"reportRevision":report_revision,"requestId":id}),
                            Projection::AgentTerminalExplanation(explanation),
                        ))
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
                            _ => FailureKind::ProtocolInvalid,
                        },
                        match reason {
                            PanelAgentFailure::NotFound
                            | PanelAgentFailure::Unbound
                            | PanelAgentFailure::BindingChanged => "binding-changed",
                            _ => "read-failed",
                        },
                    )),
                    _ => Err(terminal_agent::failure(
                        FailureKind::ProtocolInvalid,
                        "read-failed",
                    )),
                }
            })()),
        }
    }
}
