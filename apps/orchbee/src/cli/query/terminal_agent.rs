//! Exact physical semantic reads; transport correlation also proves contiguous cursor coverage.
use super::{
    BorrowedFd, Failure, FailureKind, Instant, LocalClient, ProtocolVersion, Snapshot, request_id,
};
use crate::local_client::workbench::workspace_metadata::MetadataExchangeReason;
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentFailure, PanelAgentOutcome, PanelAgentRequest,
    TerminalAgentObservation, WorkbenchRevision,
};

#[derive(Debug)]
pub(crate) struct TerminalAgentRead {
    pub source: serde_json::Value,
    pub report_revision: WorkbenchRevision,
    pub observation: Box<TerminalAgentObservation>,
}
impl TerminalAgentRead {
    pub(in crate::cli) fn document(&self) -> serde_json::Value {
        serde_json::json!({"source":self.source,"observation":self.observation})
    }
}
pub(in crate::cli) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    binding: &AgentLaunchBinding,
    after: Option<WorkbenchRevision>,
    deadline: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<TerminalAgentRead, Failure> {
    let id = request_id()?;
    let reply = client
        .panel_agent(
            PanelAgentRequest {
                protocol_version: ProtocolVersion,
                request_id: id.clone(),
                site_id: snapshot.site_id.clone(),
                action: PanelAgentAction::Observe {
                    binding: binding.clone(),
                    after_sequence: after,
                },
            },
            deadline,
            wake,
        )
        .map_err(|error| {
            failure(
                match error.reason {
                    MetadataExchangeReason::SessionUnavailable => FailureKind::ServiceUnavailable,
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
    if Instant::now() >= deadline {
        return Err(failure(FailureKind::Timeout, "read-deadline"));
    }
    match reply.outcome {
        PanelAgentOutcome::Observed {
            workbench_revision,
            report_revision,
            observation,
        } => {
            if workbench_revision != snapshot.revision {
                return Err(crate::cli::workbench_agents::failure("workbench-changed"));
            }
            Ok(TerminalAgentRead {
                report_revision,
                source: serde_json::json!({"siteId":snapshot.site_id,"workbenchRevision":workbench_revision,"reportRevision":report_revision,"requestId":id}),
                observation,
            })
        }
        PanelAgentOutcome::Failed { reason } => Err(failure(
            match reason {
                PanelAgentFailure::Denied => FailureKind::Denied,
                PanelAgentFailure::Unavailable => FailureKind::ServiceUnavailable,
                PanelAgentFailure::NotFound
                | PanelAgentFailure::Unbound
                | PanelAgentFailure::BindingChanged => FailureKind::NotFound,
                PanelAgentFailure::HistoryLimit => FailureKind::Rejected,
                PanelAgentFailure::ResultTooLarge => FailureKind::OutputTooLarge,
                _ => FailureKind::ProtocolInvalid,
            },
            match reason {
                PanelAgentFailure::HistoryLimit => "history-lost",
                PanelAgentFailure::InvalidRequest => "cursor-invalid",
                PanelAgentFailure::BindingChanged
                | PanelAgentFailure::NotFound
                | PanelAgentFailure::Unbound => "binding-changed",
                _ => "read-failed",
            },
        )),
        _ => Err(failure(FailureKind::ProtocolInvalid, "read-failed")),
    }
}

pub(in crate::cli) fn failure(kind: FailureKind, reason: &'static str) -> Failure {
    Failure::new(kind)
        .version(4)
        .failure_schema(crate::cli::schema::AGENT_FAILURE_ID)
        .details(crate::cli::stage_reason_details("agent-terminal", reason))
}
