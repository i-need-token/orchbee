//! An overlapping complete metadata/report checkpoint joined by exact member identity.
use super::{
    AtomicU8, Instant, LocalClient, MAXIMUM_BYTES, MAXIMUM_PANEL_AGENT_READ_ROWS,
    MAXIMUM_PANEL_METADATA_READ_ROWS, PanelAgentAction, PanelAgentOutcome, PanelAgentRequest,
    PanelAgentView, PanelMetadataAction, PanelMetadataOutcome, PanelMetadataRequest,
    PanelMetadataRow, Placement, PresentationAgentRow, PresentationBinding,
    PresentationMetadataRow, ProtocolVersion, Snapshot, TerminalPresentationId, WorkbenchPanelId,
    WorkbenchRevision, check, request_id,
};
use std::collections::BTreeMap;

pub(super) struct Extra {
    pub(super) metadata: PanelMetadataRow,
    pub(super) report: Option<PanelAgentView>,
}
struct Reader<'a> {
    client: &'a mut LocalClient,
    snapshot: &'a Snapshot,
    placements: &'a [Placement],
    deadline: Instant,
    cancel: &'a AtomicU8,
    bytes: &'a mut usize,
    physical_revision: Option<WorkbenchRevision>,
}
impl Reader<'_> {
    fn count<T: serde::Serialize>(&mut self, row: &T) -> Result<(), ()> {
        *self.bytes = self
            .bytes
            .checked_add(serde_json::to_vec(row).map_err(|_| ())?.len())
            .ok_or(())?;
        if *self.bytes > MAXIMUM_BYTES {
            Err(())
        } else {
            Ok(())
        }
    }
    fn binding(
        &self,
        member: &TerminalPresentationId,
        panel: &WorkbenchPanelId,
        binding: Option<&PresentationBinding>,
    ) -> Result<(), ()> {
        self.placements
            .iter()
            .find(|p| &p.member == member)
            .filter(|p| &p.panel == panel && Some(&p.terminal) == binding)
            .map(|_| ())
            .ok_or(())
    }
    fn metadata(
        &mut self,
        ids: &[TerminalPresentationId],
    ) -> Result<(WorkbenchRevision, Vec<PresentationMetadataRow>), ()> {
        check(self.deadline, self.cancel)?;
        let reply = self
            .client
            .panel_metadata(
                PanelMetadataRequest {
                    protocol_version: ProtocolVersion,
                    request_id: request_id()?,
                    site_id: self.snapshot.site_id.clone(),
                    action: PanelMetadataAction::ReadPresentations {
                        presentation_ids: ids.to_vec(),
                    },
                },
                self.deadline,
                Some(orchbee_local_ipc::LocalIo::Wake(self.cancel, 1)),
            )
            .map_err(|_| ())?;
        match reply.outcome {
            PanelMetadataOutcome::PresentationsRead {
                workbench_revision,
                metadata_revision,
                presentations,
            } if workbench_revision == self.snapshot.revision
                && presentations.len() == ids.len() =>
            {
                for row in &presentations {
                    self.binding(
                        &row.presentation_id,
                        &row.panel.panel_id,
                        row.panel.binding.as_ref(),
                    )?;
                    self.count(row)?;
                }
                Ok((metadata_revision, presentations))
            }
            PanelMetadataOutcome::PresentationsRead { .. }
            | PanelMetadataOutcome::Read { .. }
            | PanelMetadataOutcome::PresentationReported { .. }
            | PanelMetadataOutcome::Reported { .. }
            | PanelMetadataOutcome::Failed { .. } => Err(()),
        }
    }
    fn reports(
        &mut self,
        ids: &[TerminalPresentationId],
    ) -> Result<(WorkbenchRevision, Vec<PresentationAgentRow>), ()> {
        check(self.deadline, self.cancel)?;
        let reply = self
            .client
            .panel_agent(
                PanelAgentRequest {
                    protocol_version: ProtocolVersion,
                    request_id: request_id()?,
                    site_id: self.snapshot.site_id.clone(),
                    action: PanelAgentAction::ReadPresentations {
                        presentation_ids: ids.to_vec(),
                    },
                },
                self.deadline,
                Some(orchbee_local_ipc::LocalIo::Wake(self.cancel, 1)),
            )
            .map_err(|_| ())?;
        match reply.outcome {
            PanelAgentOutcome::PresentationsRead {
                workbench_revision,
                report_revision,
                presentations,
            } if workbench_revision == self.snapshot.revision
                && presentations.len() == ids.len()
                && self
                    .physical_revision
                    .is_none_or(|head| head == report_revision) =>
            {
                for row in &presentations {
                    self.binding(
                        &row.presentation_id,
                        &row.panel.panel_id,
                        row.panel.binding.as_ref(),
                    )?;
                    self.count(row)?;
                }
                Ok((report_revision, presentations))
            }
            PanelAgentOutcome::Titles { .. }
            | PanelAgentOutcome::PresentationsRead { .. }
            | PanelAgentOutcome::Explained { .. }
            | PanelAgentOutcome::States { .. }
            | PanelAgentOutcome::Observed { .. }
            | PanelAgentOutcome::Read { .. }
            | PanelAgentOutcome::PresentationReported { .. }
            | PanelAgentOutcome::Reported { .. }
            | PanelAgentOutcome::Failed { .. } => Err(()),
        }
    }
    fn read(&mut self) -> Result<BTreeMap<TerminalPresentationId, Extra>, ()> {
        let ids: Vec<_> = self.placements.iter().map(|p| p.member.clone()).collect();
        let mut metadata = BTreeMap::new();
        let mut reports = BTreeMap::new();
        let mut meta_revision = None;
        let mut report_revision = None;
        for chunk in ids.chunks(MAXIMUM_PANEL_METADATA_READ_ROWS) {
            let (revision, rows) = self.metadata(chunk)?;
            same(&mut meta_revision, revision)?;
            for row in rows {
                if metadata.insert(row.presentation_id, row.panel).is_some() {
                    return Err(());
                }
            }
        }
        for chunk in ids.chunks(MAXIMUM_PANEL_AGENT_READ_ROWS) {
            let (revision, rows) = self.reports(chunk)?;
            same(&mut report_revision, revision)?;
            for row in rows {
                if reports
                    .insert(row.presentation_id, row.panel.report)
                    .is_some()
                {
                    return Err(());
                }
            }
        }
        let (revision, rows) = self.metadata(&[])?;
        if !rows.is_empty() {
            return Err(());
        }
        same(&mut meta_revision, revision)?;
        let (revision, rows) = self.reports(&[])?;
        if !rows.is_empty() {
            return Err(());
        }
        same(&mut report_revision, revision)?;
        check(self.deadline, self.cancel)?;
        ids.into_iter()
            .map(|id| {
                let row = Extra {
                    metadata: metadata.remove(&id).ok_or(())?,
                    report: reports.remove(&id).ok_or(())?,
                };
                Ok((id, row))
            })
            .collect()
    }
}
fn same(held: &mut Option<WorkbenchRevision>, revision: WorkbenchRevision) -> Result<(), ()> {
    if held.is_some_and(|old| old != revision) {
        return Err(());
    }
    *held = Some(revision);
    Ok(())
}
pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    placements: &[Placement],
    deadline: Instant,
    cancel: &AtomicU8,
    bytes: &mut usize,
    physical_revision: Option<WorkbenchRevision>,
) -> Result<BTreeMap<TerminalPresentationId, Extra>, ()> {
    Reader {
        client,
        snapshot,
        placements,
        deadline,
        cancel,
        bytes,
        physical_revision,
    }
    .read()
}
