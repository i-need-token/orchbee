//! Exact physical semantic reads; transport correlation also proves contiguous cursor coverage.
use super::{
    BorrowedFd, Failure, FailureKind, Instant, LocalClient, ProtocolVersion, Snapshot, request_id,
};
use crate::cli::results::terminal_agent::TerminalAgentRead;
use crate::local_client::workbench::metadata_exchange::MetadataExchangeReason;
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentFailure, PanelAgentOutcome, PanelAgentRequest, WorkbenchRevision,
};

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
                session_id: snapshot.session_id.clone(),
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
                return Err(crate::cli::query::workbench_failure("workbench-changed"));
            }
            Ok(TerminalAgentRead {
                report_revision,
                source: serde_json::json!({"sessionId":snapshot.session_id,"workbenchRevision":workbench_revision,"reportRevision":report_revision,"requestId":id}),
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
                PanelAgentFailure::SessionMismatch
                | PanelAgentFailure::InvalidRequest
                | PanelAgentFailure::SourceLimit
                | PanelAgentFailure::ResourceLimit
                | PanelAgentFailure::RevisionExhausted
                | PanelAgentFailure::ClockOverflow => FailureKind::ProtocolInvalid,
            },
            match reason {
                PanelAgentFailure::HistoryLimit => "history-lost",
                PanelAgentFailure::InvalidRequest => "cursor-invalid",
                PanelAgentFailure::BindingChanged
                | PanelAgentFailure::NotFound
                | PanelAgentFailure::Unbound => "binding-changed",
                PanelAgentFailure::Denied
                | PanelAgentFailure::Unavailable
                | PanelAgentFailure::SessionMismatch
                | PanelAgentFailure::SourceLimit
                | PanelAgentFailure::ResourceLimit
                | PanelAgentFailure::RevisionExhausted
                | PanelAgentFailure::ClockOverflow
                | PanelAgentFailure::ResultTooLarge => "read-failed",
            },
        )),
        PanelAgentOutcome::Titles { .. }
        | PanelAgentOutcome::PresentationsRead { .. }
        | PanelAgentOutcome::Explained { .. }
        | PanelAgentOutcome::Identified { .. }
        | PanelAgentOutcome::States { .. }
        | PanelAgentOutcome::Read { .. }
        | PanelAgentOutcome::PresentationReported { .. }
        | PanelAgentOutcome::Reported { .. } => {
            Err(failure(FailureKind::ProtocolInvalid, "read-failed"))
        }
    }
}

pub(in crate::cli) fn failure(kind: FailureKind, reason: &'static str) -> Failure {
    Failure::new(kind)
        .failure_schema(crate::cli::schema::AGENT_FAILURE_ID)
        .details(crate::cli::stage_reason_details("agent-terminal", reason))
}
