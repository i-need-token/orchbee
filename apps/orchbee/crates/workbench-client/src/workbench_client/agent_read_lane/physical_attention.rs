//! One complete all-Session physical checkpoint, fenced by Workbench and report revisions.
use super::{LaneFailure, LocalClient, LocalClientOrigin, LocalOperationClock, mint_request_id};
use crate::workbench_client::agent_attention::{
    MAXIMUM_PHYSICAL_BYTES, MAXIMUM_PHYSICAL_ENTRIES, PhysicalRound,
};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    MAXIMUM_TERMINAL_AGENT_STATE_ROWS, PanelAgentAction, PanelAgentOutcome, PanelAgentRequest,
    PanelNode, ProtocolVersion, SnapshotReadIntent, TerminalAgentStateRow, TerminalPresentationId,
    WorkbenchRequestId, WorkbenchRevision, WorkbenchSnapshotOutcome,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

pub(super) fn read_round(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    slot: &mut Option<LocalClient>,
    deadline: Instant,
    wake: LocalIo<'_>,
    shutdown: &Arc<AtomicBool>,
) -> Result<PhysicalRound, LaneFailure> {
    if slot.is_none() {
        *slot = Some(
            origin
                .lane_before_cancellable(deadline, clock.clone(), Some(wake))
                .map_err(|_| LaneFailure::Transport)?,
        );
    }
    let result = Reader {
        client: slot.as_mut().ok_or(LaneFailure::Transport)?,
        clock,
        deadline,
        wake,
        shutdown,
    }
    .read();
    if result.is_err() {
        *slot = None;
    }
    result
}

struct Reader<'a> {
    client: &'a mut LocalClient,
    clock: &'a LocalOperationClock,
    deadline: Instant,
    wake: LocalIo<'a>,
    shutdown: &'a Arc<AtomicBool>,
}
impl Reader<'_> {
    fn id(&self) -> Result<WorkbenchRequestId, LaneFailure> {
        if self.clock.now() >= self.deadline || self.shutdown.load(Ordering::SeqCst) {
            return Err(LaneFailure::Transport);
        }
        WorkbenchRequestId::parse(mint_request_id()?.as_str()).map_err(|_| LaneFailure::Transport)
    }
    fn read(&mut self) -> Result<PhysicalRound, LaneFailure> {
        let request = SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: self.id()?,
        };
        let snapshot = match self
            .client
            .submit_workbench_snapshot_read_before_cancellable(
                request,
                self.deadline,
                Some(self.wake),
            )
            .map_err(|_| LaneFailure::Transport)?
            .outcome
        {
            WorkbenchSnapshotOutcome::Available { snapshot } => snapshot,
            WorkbenchSnapshotOutcome::Denied {} => return Err(LaneFailure::Denied),
            WorkbenchSnapshotOutcome::Unavailable { .. } => return Err(LaneFailure::Unavailable),
        };
        let mut ids = Vec::new();
        for context in &snapshot.terminal_contexts {
            for tab in &context.tabs {
                presentations(&tab.root, &mut ids)?;
            }
        }
        let mut round = RoundBuilder::default();
        // The empty final request fences even an empty Session and rejects a revision splice.
        for chunk in ids
            .chunks(MAXIMUM_TERMINAL_AGENT_STATE_ROWS)
            .chain(std::iter::once([].as_slice()))
        {
            let request = PanelAgentRequest {
                protocol_version: ProtocolVersion,
                request_id: self.id()?,
                session_id: snapshot.session_id.clone(),
                action: PanelAgentAction::States {
                    presentation_ids: chunk.to_vec(),
                },
            };
            match self
                .client
                .panel_agent(request, self.deadline, Some(self.wake))
                .map_err(|_| LaneFailure::Transport)?
                .outcome
            {
                PanelAgentOutcome::States {
                    workbench_revision,
                    report_revision,
                    presentations,
                } if workbench_revision == snapshot.revision => {
                    round.accept(report_revision, presentations)?;
                }
                PanelAgentOutcome::Failed {
                    reason: orchbee_workbench_protocol::PanelAgentFailure::Denied,
                } => return Err(LaneFailure::Denied),
                // This query consumes only its requested metadata/report payload; other receipt classes cannot answer that query.
                _ => return Err(LaneFailure::Unavailable),
            }
        }
        self.id()?; // Deadline and shutdown are checked again before publication.
        round.finish()
    }
}
fn presentations(
    node: &PanelNode,
    ids: &mut Vec<TerminalPresentationId>,
) -> Result<(), LaneFailure> {
    match node {
        PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } => {
            if ids.len() + 1 + child_presentations.len() > MAXIMUM_PHYSICAL_ENTRIES {
                return Err(LaneFailure::Unavailable);
            }
            ids.push(primary_presentation.presentation_id.clone());
            ids.extend(
                child_presentations
                    .iter()
                    .map(|p| p.presentation_id.clone()),
            );
        }
        PanelNode::Split { children, .. } => {
            for child in children.iter() {
                presentations(&child.node, ids)?;
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct RoundBuilder {
    revision: Option<WorkbenchRevision>,
    ids: std::collections::BTreeSet<TerminalPresentationId>,
    states: Vec<orchbee_workbench_protocol::TerminalAgentStateSnapshot>,
    bytes: usize,
}
impl RoundBuilder {
    fn accept(
        &mut self,
        revision: WorkbenchRevision,
        rows: Vec<TerminalAgentStateRow>,
    ) -> Result<(), LaneFailure> {
        if self.revision.is_some_and(|head| head != revision)
            || self.ids.len() + rows.len() > MAXIMUM_PHYSICAL_ENTRIES
        {
            return Err(LaneFailure::Unavailable);
        }
        for row in rows {
            if !self.ids.insert(row.presentation_id) {
                return Err(LaneFailure::Transport);
            }
            if let Some(state) = row.snapshot {
                self.bytes = self
                    .bytes
                    .checked_add(
                        serde_json::to_vec(&state)
                            .map_err(|_| LaneFailure::Transport)?
                            .len(),
                    )
                    .ok_or(LaneFailure::Unavailable)?;
                if self.bytes > MAXIMUM_PHYSICAL_BYTES {
                    return Err(LaneFailure::Unavailable);
                }
                self.states.push(state);
            }
        }
        self.revision = Some(revision);
        Ok(())
    }
    fn finish(self) -> Result<PhysicalRound, LaneFailure> {
        Ok(PhysicalRound {
            report_revision: self.revision.ok_or(LaneFailure::Transport)?,
            states: self.states,
        })
    }
}

#[cfg(test)]
mod tests;
