//! Exact physical semantic reads; transport correlation also proves contiguous cursor coverage.
use super::*;
use crate::local_client::workbench::workspace_metadata::MetadataExchangeReason;
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentFailure, PanelAgentOutcome, PanelAgentRequest,
    TerminalAgentObservation, WorkbenchRevision,
};

#[derive(Debug)]
pub(crate) struct TerminalAgentRead {
    pub source: serde_json::Value,
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
            Failure::new(match error.reason {
                MetadataExchangeReason::SessionUnavailable => FailureKind::ServiceUnavailable,
                MetadataExchangeReason::Encoding => FailureKind::InternalFailure,
                MetadataExchangeReason::Deadline | MetadataExchangeReason::ResponseDeadline => {
                    FailureKind::Timeout
                }
                MetadataExchangeReason::Correlation | MetadataExchangeReason::Protocol => {
                    FailureKind::ProtocolInvalid
                }
                _ => FailureKind::TransportLoss,
            })
        })?;
    if Instant::now() >= deadline {
        return Err(Failure::new(FailureKind::Timeout));
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
                source: serde_json::json!({"siteId":snapshot.site_id,"workbenchRevision":workbench_revision,"reportRevision":report_revision,"requestId":id}),
                observation,
            })
        }
        PanelAgentOutcome::Failed { reason } => Err(Failure::new(match reason {
            PanelAgentFailure::Denied => FailureKind::Denied,
            PanelAgentFailure::Unavailable => FailureKind::ServiceUnavailable,
            PanelAgentFailure::NotFound
            | PanelAgentFailure::Unbound
            | PanelAgentFailure::BindingChanged => FailureKind::NotFound,
            PanelAgentFailure::HistoryLimit => FailureKind::Rejected,
            PanelAgentFailure::ResultTooLarge => FailureKind::OutputTooLarge,
            _ => FailureKind::ProtocolInvalid,
        })),
        _ => Err(Failure::new(FailureKind::ProtocolInvalid)),
    }
}
