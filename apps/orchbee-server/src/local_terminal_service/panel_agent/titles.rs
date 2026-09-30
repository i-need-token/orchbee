//! Physical title projection fenced against exact committed membership before and after I/O.
use super::super::{LocalTerminalService, placement};
use crate::worker_obcn_carrier::{
    WorkerOperationRequest, WorkerOperationResult, WorkerRequestHandle,
};
use orchbee_agent_observation_protocol::AgentLaunchBinding;
use orchbee_capability_node_protocol as node;
use orchbee_workbench_protocol::*;

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
        _ => Err(PanelAgentFailure::BindingChanged),
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
            site_id: request.site_id.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn binding() -> AgentLaunchBinding {
        serde_json::from_value(json!({"presentationId":"00000000000000000000000000000001","generation":"00000000000000000000000000000002","hostEpoch":"00000000000000000000000000000003","hostTerminalId":"00000000000000000000000000000004"})).unwrap()
    }
    fn result(binding: AgentLaunchBinding) -> PanelAgentResult {
        PanelAgentResult {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse("00000000000000000000000000000005").unwrap(),
            site_id: SiteId::parse("00000000000000000000000000000006").unwrap(),
            outcome: PanelAgentOutcome::States {
                workbench_revision: WorkbenchRevision::new(1),
                report_revision: WorkbenchRevision::new(2),
                presentations: vec![TerminalAgentStateRow {
                    presentation_id: TerminalPresentationId::parse(
                        binding.presentation_id.as_str(),
                    )
                    .unwrap(),
                    snapshot: Some(TerminalAgentStateSnapshot {
                        binding,
                        sequence: WorkbenchRevision::new(7),
                        agent: Some("codex".into()),
                        state: PanelAgentState::Working,
                    }),
                }],
            },
        }
    }
    #[test]
    fn exact_physical_title_context_rejects_changed_creation_and_partial_membership() {
        let exact = binding();
        assert!(state_context(result(exact.clone()), std::slice::from_ref(&exact)).is_ok());
        let mut changed = exact.clone();
        changed.generation = orchbee_agent_observation_protocol::AgentAttachmentGeneration::parse(
            "00000000000000000000000000000009",
        )
        .unwrap();
        assert_eq!(
            state_context(result(changed), std::slice::from_ref(&exact)),
            Err(PanelAgentFailure::BindingChanged)
        );
        let mut incomplete = result(exact.clone());
        if let PanelAgentOutcome::States { presentations, .. } = &mut incomplete.outcome {
            presentations.clear();
        }
        assert_eq!(
            state_context(incomplete, std::slice::from_ref(&exact)),
            Err(PanelAgentFailure::BindingChanged)
        );
        let physical = identities(&[exact]).unwrap()[0];
        assert_eq!(physical.creation_id().as_bytes(), 2_u128.to_be_bytes());
        assert_eq!(physical.host_epoch().as_bytes(), 3_u128.to_be_bytes());
        assert_eq!(physical.terminal_id().as_bytes(), 4_u128.to_be_bytes());
    }
    #[test]
    fn one_physical_title_observation_projects_to_distinct_exact_presentation_aliases() {
        let first = binding();
        let mut second = first.clone();
        second.presentation_id = orchbee_agent_observation_protocol::AgentPresentationId::parse(
            "00000000000000000000000000000007",
        )
        .unwrap();
        let bindings = vec![first, second];
        let physical = identities(&bindings).unwrap();
        assert_eq!(physical[0], physical[1]);
        let rows = vec![node::TerminalTitleRow {
            identity: physical[0],
            observation: node::TerminalTitleObservation::Observed {
                revision: std::num::NonZeroU64::MIN,
                title: Some("⠋ shared".into()),
            },
        }];
        let projected = project_titles(&bindings, &physical, &rows).unwrap();
        assert_eq!(projected.len(), 2);
        assert_ne!(
            projected[0].binding.presentation_id,
            projected[1].binding.presentation_id
        );
        assert_eq!(projected[0].observation, projected[1].observation);
        assert!(project_titles(&bindings, &physical, &[]).is_err());
    }
}
