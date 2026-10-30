//! Physical title projection fenced against exact committed membership before and after I/O.
use super::super::{LocalTerminalService, placement};
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestHandle,
};
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_capability_node_protocol as node;
use orchbee_workbench_protocol::{
    PanelAgentAction, PanelAgentFailure, PanelAgentOutcome, PanelAgentRequest, PanelAgentResult,
    ProtocolVersion, TerminalAgentStateRow, TerminalPresentationId, TerminalTitleObservation,
    TerminalTitleRow, WorkbenchResponse, WorkbenchRevision, serialize_workbench_response,
};

fn identities(
    bindings: &[AgentLaunchBinding],
) -> Result<Vec<node::HostTerminalIdentity>, PanelAgentFailure> {
    let bytes = |s: &str| {
        u128::from_str_radix(s, 16)
            .map(u128::to_be_bytes)
            .map_err(|_| PanelAgentFailure::InvalidRequest)
    };
    bindings
        .iter()
        .map(|b| {
            Ok(node::HostTerminalIdentity::new(
                node::HostCreationId::from_bytes(bytes(b.generation.as_str())?)
                    .map_err(|_| PanelAgentFailure::InvalidRequest)?,
                node::HostEpoch::from_bytes(bytes(b.host_epoch.as_str())?)
                    .map_err(|_| PanelAgentFailure::InvalidRequest)?,
                node::HostTerminalId::from_bytes(bytes(b.host_terminal_id.as_str())?)
                    .map_err(|_| PanelAgentFailure::InvalidRequest)?,
            ))
        })
        .collect()
}
fn state_context(
    result: PanelAgentResult,
    bindings: &[AgentLaunchBinding],
) -> Result<
    (
        WorkbenchRevision,
        WorkbenchRevision,
        Vec<TerminalAgentStateRow>,
    ),
    PanelAgentFailure,
> {
    match result.outcome {
        PanelAgentOutcome::States {
            workbench_revision,
            report_revision,
            presentations,
        } if presentations.len() == bindings.len()
            && presentations.iter().zip(bindings).all(|(row, b)| {
                row.presentation_id.as_str() == b.presentation_id.as_str()
                    && row.snapshot.as_ref().is_some_and(|s| &s.binding == b)
            }) =>
        {
            Ok((workbench_revision, report_revision, presentations))
        }
        PanelAgentOutcome::Failed { reason } => Err(reason),
        PanelAgentOutcome::Titles { .. }
        | PanelAgentOutcome::PresentationsRead { .. }
        | PanelAgentOutcome::Explained { .. }
        | PanelAgentOutcome::Identified { .. }
        | PanelAgentOutcome::States { .. }
        | PanelAgentOutcome::Observed { .. }
        | PanelAgentOutcome::Read { .. }
        | PanelAgentOutcome::PresentationReported { .. }
        | PanelAgentOutcome::Reported { .. } => Err(PanelAgentFailure::BindingChanged),
    }
}
fn project_titles(
    bindings: &[AgentLaunchBinding],
    physical: &[node::HostTerminalIdentity],
    rows: &[node::TerminalTitleRow],
) -> Result<Vec<TerminalTitleRow>, PanelAgentFailure> {
    bindings
        .iter()
        .zip(physical)
        .map(|(binding, id)| {
            let row = rows
                .iter()
                .find(|row| &row.identity == id)
                .ok_or(PanelAgentFailure::Unavailable)?;
            Ok(TerminalTitleRow {
                binding: binding.clone(),
                observation: match &row.observation {
                    node::TerminalTitleObservation::Unavailable => {
                        TerminalTitleObservation::Unavailable {}
                    }
                    node::TerminalTitleObservation::Observed { revision, title } => {
                        TerminalTitleObservation::Observed {
                            revision: WorkbenchRevision::new(revision.get()),
                            title: title.clone(),
                        }
                    }
                },
            })
        })
        .collect()
}
impl LocalTerminalService {
    fn title_worker(&self) -> Result<WorkerRequestHandle, PanelAgentFailure> {
        self.workers
            .lock()
            .map_err(|_| PanelAgentFailure::Unavailable)?
            .request_handle()
            .filter(|h| h.is_active())
            .ok_or(PanelAgentFailure::Unavailable)
    }
    pub(super) fn read_panel_titles(&self, request: &PanelAgentRequest) -> PanelAgentResult {
        let read = || -> Result<PanelAgentOutcome, PanelAgentFailure> {
            request
                .validate()
                .map_err(|_| PanelAgentFailure::InvalidRequest)?;
            let PanelAgentAction::Titles { bindings } = &request.action else {
                return Err(PanelAgentFailure::InvalidRequest);
            };
            let handle = self.title_worker()?;
            let mut probe = request.clone();
            probe.action = PanelAgentAction::States {
                presentation_ids: bindings
                    .iter()
                    .map(|b| {
                        TerminalPresentationId::parse(b.presentation_id.as_str())
                            .map_err(|_| PanelAgentFailure::InvalidRequest)
                    })
                    .collect::<Result<_, _>>()?,
            };
            let before = state_context(self.execute_panel_agent(probe.clone()), bindings)?;
            let operation = self
                .next_viewer_operation()
                .ok_or(PanelAgentFailure::Unavailable)?;
            let physical = identities(bindings)?;
            let mut unique = Vec::new();
            for id in &physical {
                if !unique.contains(id) {
                    unique.push(*id);
                }
            }
            let query = node::TerminalTitlesRequest {
                request_id: operation,
                worker_id: node::WorkerId::from_bytes(handle.worker_id().as_bytes())
                    .map_err(|_| PanelAgentFailure::Unavailable)?,
                worker_generation: node::WorkerGeneration::new(handle.generation()),
                identities: unique,
            };
            let result = match handle
                .request(WorkerOperationRequest::Titles(query.clone()))
                .map_err(|_| PanelAgentFailure::Unavailable)?
            {
                WorkerOperationResult::Titles(result) if result.answers(&query) => result,
                _ => return Err(PanelAgentFailure::Unavailable),
            };
            let after = state_context(self.execute_panel_agent(probe), bindings)?;
            if before != after
                || !handle.is_active()
                || placement(Some(&handle)) != placement(Some(&self.title_worker()?))
            {
                return Err(PanelAgentFailure::BindingChanged);
            }
            let presentations = project_titles(bindings, &physical, &result.rows)?;
            Ok(PanelAgentOutcome::Titles {
                workbench_revision: before.0,
                presentations,
            })
        };
        let mut result = PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: request.request_id.clone(),
            session_id: request.session_id.clone(),
            outcome: read().unwrap_or_else(|reason| PanelAgentOutcome::Failed { reason }),
        };
        if serialize_workbench_response(&WorkbenchResponse::PanelAgent(result.clone())).is_err() {
            result.outcome = PanelAgentOutcome::Failed {
                reason: PanelAgentFailure::ResultTooLarge,
            };
        }
        result
    }
}

#[cfg(all(test, windows))]
mod tests;
