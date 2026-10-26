//! Two independently revisioned report owners are joined only across overlapping stable reads.
use super::{
    Arc, AtomicBool, Instant, LaneFailure, LocalClient, LocalClientOrigin, LocalOperationClock,
    Ordering, mint_request_id,
};
use crate::workbench_client::panel_reports::{MAXIMUM_BYTES, PanelReport, Round, Scope};
use orchbee_workbench_protocol::{
    MAXIMUM_PANEL_AGENT_READ_ROWS, MAXIMUM_PANEL_METADATA_READ_ROWS, PanelAgentAction,
    PanelAgentOutcome, PanelAgentRequest, PanelMetadataAction, PanelMetadataOutcome,
    PanelMetadataRequest, PresentationAgentRow, PresentationMetadataRow, ProtocolVersion,
    TerminalPresentationId, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
};
use std::collections::BTreeMap;

pub(super) fn read_round(
    origin: &LocalClientOrigin,
    clock: &LocalOperationClock,
    client_slot: &mut Option<LocalClient>,
    scope: Scope,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'_>,
    shutdown: &Arc<AtomicBool>,
) -> Result<Round, LaneFailure> {
    if client_slot.is_none() {
        *client_slot = Some(
            origin
                .reconnect_before_cancellable(deadline, clock.clone(), Some(wake))
                .map_err(|_| LaneFailure::Transport)?,
        );
    }
    let client = client_slot.as_mut().ok_or(LaneFailure::Transport)?;
    let result = Reader {
        client,
        clock,
        scope: &scope,
        deadline,
        wake,
        shutdown,
    }
    .read();
    if result.is_err() {
        *client_slot = None;
    }
    result.map(|rows| Round { scope, rows })
}
struct Reader<'a> {
    client: &'a mut LocalClient,
    clock: &'a LocalOperationClock,
    scope: &'a Scope,
    deadline: Instant,
    wake: orchbee_local_ipc::LocalIo<'a>,
    shutdown: &'a Arc<AtomicBool>,
}
impl Reader<'_> {
    fn id(&self) -> Result<WorkbenchRequestId, LaneFailure> {
        if self.shutdown.load(Ordering::SeqCst) || self.clock.now() >= self.deadline {
            return Err(LaneFailure::Transport);
        }
        WorkbenchRequestId::parse(mint_request_id()?.as_str()).map_err(|_| LaneFailure::Transport)
    }
    fn metadata(
        &mut self,
        ids: &[TerminalPresentationId],
    ) -> Result<(WorkbenchRevision, Vec<PresentationMetadataRow>), LaneFailure> {
        let request = PanelMetadataRequest {
            protocol_version: ProtocolVersion,
            request_id: self.id()?,
            site_id: self.scope.site.clone(),
            action: PanelMetadataAction::ReadPresentations {
                presentation_ids: ids.to_vec(),
            },
        };
        let outcome = self
            .client
            .panel_metadata(request, self.deadline, Some(self.wake))
            .map_err(|_| LaneFailure::Transport)?
            .outcome;
        match outcome {
            PanelMetadataOutcome::PresentationsRead {
                workbench_revision,
                metadata_revision,
                presentations,
            } if workbench_revision == self.scope.revision => {
                for row in &presentations {
                    self.binding(
                        &row.presentation_id,
                        &row.panel.panel_id,
                        row.panel.binding.as_ref(),
                    )?;
                }
                Ok((metadata_revision, presentations))
            }
            PanelMetadataOutcome::Failed {
                reason: orchbee_workbench_protocol::PanelMetadataFailure::Denied,
            } => Err(LaneFailure::Denied),
            _ => Err(LaneFailure::Unavailable),
        }
    }
    fn reports(
        &mut self,
        ids: &[TerminalPresentationId],
    ) -> Result<(WorkbenchRevision, Vec<PresentationAgentRow>), LaneFailure> {
        let request = PanelAgentRequest {
            protocol_version: ProtocolVersion,
            request_id: self.id()?,
            site_id: self.scope.site.clone(),
            action: PanelAgentAction::ReadPresentations {
                presentation_ids: ids.to_vec(),
            },
        };
        let outcome = self
            .client
            .panel_agent(request, self.deadline, Some(self.wake))
            .map_err(|_| LaneFailure::Transport)?
            .outcome;
        match outcome {
            PanelAgentOutcome::PresentationsRead {
                workbench_revision,
                report_revision,
                presentations,
            } if workbench_revision == self.scope.revision => {
                for row in &presentations {
                    self.binding(
                        &row.presentation_id,
                        &row.panel.panel_id,
                        row.panel.binding.as_ref(),
                    )?;
                }
                Ok((report_revision, presentations))
            }
            PanelAgentOutcome::Failed {
                reason: orchbee_workbench_protocol::PanelAgentFailure::Denied,
            } => Err(LaneFailure::Denied),
            _ => Err(LaneFailure::Unavailable),
        }
    }
    fn binding(
        &self,
        presentation: &TerminalPresentationId,
        panel: &WorkbenchPanelId,
        binding: Option<&orchbee_workbench_protocol::PresentationBinding>,
    ) -> Result<(), LaneFailure> {
        if self
            .scope
            .panels
            .iter()
            .find(|p| &p.presentation_id == presentation && &p.panel_id == panel)
            .map(|p| &p.binding)
            != binding
            || binding.is_none()
        {
            return Err(LaneFailure::Transport);
        }
        Ok(())
    }
    fn read(&mut self) -> Result<BTreeMap<WorkbenchPanelId, PanelReport>, LaneFailure> {
        let ids: Vec<_> = self
            .scope
            .panels
            .iter()
            .map(|p| p.presentation_id.clone())
            .collect();
        let mut metadata = BTreeMap::new();
        let mut reports = BTreeMap::new();
        let mut bytes = 0;
        let mut metadata_revision = None;
        let mut report_revision = None;
        for chunk in ids.chunks(MAXIMUM_PANEL_METADATA_READ_ROWS) {
            let (revision, rows) = self.metadata(chunk)?;
            same_revision(&mut metadata_revision, revision)?;
            for row in rows {
                count_bytes(
                    &mut bytes,
                    serde_json::to_vec(&row)
                        .map_err(|_| LaneFailure::Transport)?
                        .len(),
                )?;
                let row = row.panel;
                if metadata.insert(row.panel_id.clone(), row).is_some() {
                    return Err(LaneFailure::Transport);
                }
            }
        }
        for chunk in ids.chunks(MAXIMUM_PANEL_AGENT_READ_ROWS) {
            let (revision, rows) = self.reports(chunk)?;
            same_revision(&mut report_revision, revision)?;
            for row in rows {
                count_bytes(
                    &mut bytes,
                    serde_json::to_vec(&row)
                        .map_err(|_| LaneFailure::Transport)?
                        .len(),
                )?;
                let row = row.panel;
                if reports.insert(row.panel_id.clone(), row).is_some() {
                    return Err(LaneFailure::Transport);
                }
            }
        }
        let (revision, rows) = self.metadata(&[])?;
        if !rows.is_empty() {
            return Err(LaneFailure::Transport);
        }
        same_revision(&mut metadata_revision, revision)?;
        let (revision, rows) = self.reports(&[])?;
        if !rows.is_empty() {
            return Err(LaneFailure::Transport);
        }
        same_revision(&mut report_revision, revision)?;
        let mut joined = BTreeMap::new();
        for selected in &self.scope.panels {
            let id = selected.panel_id.clone();
            let metadata = metadata.remove(&id).ok_or(LaneFailure::Transport)?;
            let report = reports.remove(&id).ok_or(LaneFailure::Transport)?.report;
            let presentation = metadata.presentation(
                report.as_ref().and_then(|r| r.agent.as_deref()),
                report.as_ref().and_then(|r| r.authority_source.as_deref()),
            );
            joined.insert(
                id,
                PanelReport {
                    report,
                    metadata: presentation,
                    tokens: metadata.tokens,
                },
            );
        }
        if !metadata.is_empty() || !reports.is_empty() {
            return Err(LaneFailure::Transport);
        }
        Ok(joined)
    }
}
fn same_revision(
    held: &mut Option<WorkbenchRevision>,
    revision: WorkbenchRevision,
) -> Result<(), LaneFailure> {
    if held.is_some_and(|old| old != revision) {
        return Err(LaneFailure::Unavailable);
    }
    *held = Some(revision);
    Ok(())
}
fn count_bytes(bytes: &mut usize, count: usize) -> Result<(), LaneFailure> {
    *bytes = bytes
        .checked_add(count)
        .filter(|n| *n <= MAXIMUM_BYTES)
        .ok_or(LaneFailure::Unavailable)?;
    Ok(())
}

#[cfg(test)]
mod tests;
