//! Semantic readers pin current logical membership and the existing physical viewer admission.
use super::{ProcessObservationTarget, SessionCoordinator};
use crate::terminal_coordinator::WorkerPlacement;
use orchbee_workbench_protocol::{
    SemanticEventData as Data, SemanticEventKind as Kind, SemanticEventSource,
    SemanticEventsFailure as Failure, SemanticPane, SessionId, WorkbenchRevision,
};
use std::{collections::BTreeMap, time::Instant};

pub(super) struct Observation {
    pub(super) physical: orchbee_server::TerminalAgentBinding,
    started: Instant,
    frame: Option<(u64, u64)>,
    lifecycle: Option<(u64, bool)>,
}
pub(super) type Observations = BTreeMap<(String, String), Observation>;

impl SessionCoordinator {
    /// Independent correlated Host facts; process discovery refusal does not establish terminal exit.
    pub(crate) fn observe_terminal_projection(
        &mut self,
        target: crate::terminal_coordinator::BindingViewerTarget,
        frame: Option<(u64, u64)>,
        lifecycle: Option<(u64, bool)>,
        started: Instant,
        now: Instant,
    ) -> bool {
        if started > now || frame.is_some_and(|(generation, _)| generation == 0) {
            return false;
        }
        let Ok(facts) = self.terminal_agent_bindings() else {
            return false;
        };
        let Some(physical) = facts
            .iter()
            .find(|fact| {
                fact.identity == target.identity
                    && fact.worker_generation == target.worker.generation
                    && fact.binding.worker_id.as_str()
                        == format!(
                            "{:032x}",
                            u128::from_be_bytes(target.worker.worker_id.as_bytes())
                        )
            })
            .cloned()
        else {
            return false;
        };
        self.terminal_observations
            .retain(|_, observed| facts.contains(&observed.physical));
        let key = (
            physical.binding.worker_id.as_str().into(),
            physical.binding.terminal_id.as_str().into(),
        );
        let prior = self.terminal_observations.get(&key);
        if prior.is_some_and(|prior| prior.started >= started) {
            return false;
        }
        let frame = frame.filter(|frame| {
            prior
                .and_then(|prior| prior.frame)
                .is_none_or(|old| *frame >= old)
        });
        let lifecycle = lifecycle.filter(|(revision, settled)| {
            prior
                .and_then(|prior| prior.lifecycle)
                .is_none_or(|(old_revision, old_settled)| {
                    *revision >= old_revision && (!old_settled || *settled)
                })
        });
        if frame.is_none() && lifecycle.is_none() {
            return false;
        }
        let changed = frame.is_some_and(|frame| {
            prior
                .and_then(|prior| prior.frame)
                .is_some_and(|old| old != frame)
        });
        let exited = lifecycle.is_some_and(|(_, settled)| {
            settled
                && prior
                    .and_then(|prior| prior.lifecycle)
                    .is_none_or(|(_, old)| !old)
        });
        let Some(workbench) = self.workbench.as_ref() else {
            return false;
        };
        let journal = workbench.event_journal();
        let panes =
            orchbee_server::semantic_panes_for_binding(workbench.snapshot(), &physical.binding);
        for pane in panes {
            let workspace = journal.workspace_for_context(&pane.terminal_context_id);
            if changed && let Some((generation, revision)) = frame {
                journal.publish(
                    workbench.snapshot().revision,
                    workspace.clone(),
                    Kind::PaneOutputChanged,
                    Data::Output {
                        pane: pane.clone(),
                        content_generation: WorkbenchRevision::new(generation),
                        content_revision: WorkbenchRevision::new(revision),
                        matched_line: None,
                    },
                );
            }
            if exited && let Some((revision, _)) = lifecycle {
                journal.publish(
                    workbench.snapshot().revision,
                    workspace,
                    Kind::PaneExited,
                    Data::Exited {
                        pane,
                        lifecycle_revision: WorkbenchRevision::new(revision),
                    },
                );
            }
        }
        let retained_frame = frame.or_else(|| prior.and_then(|prior| prior.frame));
        let retained_lifecycle = lifecycle.or_else(|| prior.and_then(|prior| prior.lifecycle));
        self.terminal_observations.insert(
            key,
            Observation {
                physical,
                started,
                frame: retained_frame,
                lifecycle: retained_lifecycle,
            },
        );
        true
    }
    pub(crate) fn event_journal(&self) -> Option<orchbee_server::SessionEventJournal> {
        Some(self.workbench.as_ref()?.event_journal())
    }
    pub(crate) fn maintain_events(&mut self) -> Result<(), Failure> {
        self.workbench
            .as_mut()
            .ok_or(Failure::Unavailable)?
            .maintain_workspace_metadata(std::time::Instant::now())
            .map_err(|_| Failure::Unavailable)
    }
    /// Commit due metadata expiry and return the next exact owner deadline under the Session gate.
    pub(crate) fn maintain_event_deadline(
        &mut self,
        now: std::time::Instant,
    ) -> Result<Option<std::time::Instant>, Failure> {
        let Some(workbench) = self.workbench.as_mut() else {
            return Ok(None);
        };
        workbench
            .maintain_workspace_metadata(now)
            .map_err(|_| Failure::Unavailable)?;
        Ok(workbench.workspace_metadata_deadline())
    }
    pub(crate) fn event_source(
        &mut self,
        session: &SessionId,
        source: &SemanticEventSource,
        worker: Option<WorkerPlacement>,
    ) -> Result<(SemanticPane, WorkbenchRevision, ProcessObservationTarget), Failure> {
        let workbench = self.workbench.as_ref().ok_or(Failure::Unavailable)?;
        if workbench.session_id() != session {
            return Err(Failure::SessionMismatch);
        }
        let pane =
            orchbee_server::semantic_pane(workbench.snapshot(), source).ok_or(Failure::NotFound)?;
        let binding = pane.binding.as_ref().ok_or(Failure::SourceUnavailable)?;
        let revision = workbench.snapshot().revision;
        let prepared = self
            .prepare_binding_viewer(
                &orchbee_client_server_protocol::SessionId::parse(session.as_str())
                    .map_err(|_| Failure::InvalidIntent)?,
                &orchbee_client_server_protocol::WorkerId::parse(binding.worker_id.as_str())
                    .map_err(|_| Failure::InvalidIntent)?,
                &orchbee_client_server_protocol::TerminalId::parse(binding.terminal_id.as_str())
                    .map_err(|_| Failure::InvalidIntent)?,
                worker,
            )
            .map_err(|_| Failure::SourceUnavailable)?;
        let target = ProcessObservationTarget {
            session_id: session.clone(),
            binding: binding.clone(),
            target: prepared.target,
        };
        Ok((pane, revision, target))
    }
}
