//! Read explicit reports under the enclosing metadata query's deadline and aggregate budget.
use super::{Failure, FailureKind, changed, expected};
use crate::cli::{
    query,
    source::{ArmedSignals, SignalWatch},
};
use crate::local_client::{LocalClient, workbench::metadata_exchange::MetadataExchangeReason};
use orchbee_workbench_protocol::{
    MAXIMUM_PANEL_AGENT_READ_ROWS, PanelAgentAction, PanelAgentFailure, PanelAgentOutcome,
    PanelAgentRequest, PanelAgentRow, PanelAgentView, Snapshot, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchRevision,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Instant};

#[derive(Debug)]
pub(super) struct Document {
    rows: BTreeMap<WorkbenchPanelId, PanelAgentRow>,
    revision: WorkbenchRevision,
    request_ids: Vec<WorkbenchRequestId>,
    encoded_bytes: usize,
    byte_limit: usize,
}
fn send(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    action: PanelAgentAction,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(WorkbenchRequestId, PanelAgentOutcome), Failure> {
    signals.check_before(deadline)?;
    let request_id = query::request_id()?;
    let reply = client.panel_agent(
        PanelAgentRequest {
            protocol_version: snapshot.protocol_version,
            request_id: request_id.clone(),
            site_id: snapshot.site_id.clone(),
            action,
        },
        deadline,
        signals.poll_fd(),
    );
    signals.check_before(deadline)?;
    let reply = reply.map_err(|error| {
        Failure::new(match error.reason {
            MetadataExchangeReason::ConnectionUnavailable => FailureKind::ServiceUnavailable,
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
    Ok((request_id, reply.outcome))
}
fn observe(
    snapshot: &Snapshot,
    outcome: PanelAgentOutcome,
) -> Result<(WorkbenchRevision, Vec<PanelAgentRow>), Failure> {
    match outcome {
        PanelAgentOutcome::Read {
            workbench_revision,
            report_revision,
            panels,
        } => {
            if workbench_revision != snapshot.revision {
                return Err(crate::cli::workbench_agents::failure("workbench-changed"));
            }
            for row in &panels {
                if row.binding != expected(snapshot, &row.panel_id)? {
                    return Err(Failure::new(FailureKind::ProtocolInvalid));
                }
            }
            Ok((report_revision, panels))
        }
        PanelAgentOutcome::Failed {
            reason:
                PanelAgentFailure::NotFound
                | PanelAgentFailure::SiteMismatch
                | PanelAgentFailure::BindingChanged,
        } => Err(crate::cli::workbench_agents::failure("workbench-changed")),
        PanelAgentOutcome::Failed { reason } => Err(Failure::new(match reason {
            PanelAgentFailure::Denied => FailureKind::Denied,
            PanelAgentFailure::Unavailable => FailureKind::ServiceUnavailable,
            PanelAgentFailure::RevisionExhausted | PanelAgentFailure::ClockOverflow => {
                FailureKind::InternalFailure
            }
            PanelAgentFailure::ResultTooLarge => FailureKind::OutputTooLarge,
            _ => FailureKind::ProtocolInvalid,
        })),
        _ => Err(Failure::new(FailureKind::ProtocolInvalid)),
    }
}
fn exchange(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    ids: &[WorkbenchPanelId],
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(WorkbenchRequestId, WorkbenchRevision, Vec<PanelAgentRow>), Failure> {
    let (id, outcome) = send(
        client,
        snapshot,
        PanelAgentAction::Read {
            panel_ids: ids.to_vec(),
        },
        deadline,
        signals,
    )?;
    let (revision, rows) = observe(snapshot, outcome)?;
    Ok((id, revision, rows))
}

pub(super) fn read_presentation(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    member: &crate::cli::current_presentation::Resolved,
    byte_limit: usize,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Document, Failure> {
    let (id, outcome) = send(
        client,
        snapshot,
        PanelAgentAction::ReadPresentations {
            presentation_ids: vec![member.presentation_id.clone()],
        },
        deadline,
        signals,
    )?;
    let (workbench_revision, revision, mut rows) = match outcome {
        PanelAgentOutcome::PresentationsRead {
            workbench_revision,
            report_revision,
            presentations,
        } => (workbench_revision, report_revision, presentations),
        other => {
            return observe(snapshot, other)
                .and_then(|_| Err(Failure::new(FailureKind::ProtocolInvalid)));
        }
    };
    if workbench_revision != snapshot.revision {
        return Err(crate::cli::workbench_agents::failure("workbench-changed"));
    }
    if rows.len() != 1 {
        return Err(Failure::new(FailureKind::ProtocolInvalid));
    }
    let row = rows.pop().expect("one exact member");
    if row.presentation_id != member.presentation_id
        || row.panel.panel_id != member.panel_id
        || row.panel.binding != member.binding
    {
        return Err(Failure::new(FailureKind::ProtocolInvalid));
    }
    let mut document = Document {
        rows: BTreeMap::new(),
        revision,
        request_ids: vec![],
        encoded_bytes: 0,
        byte_limit,
    };
    document.admit(id, revision, vec![row.panel])?;
    Ok(document)
}

pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    ids: &[WorkbenchPanelId],
    byte_limit: usize,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Document, Failure> {
    let chunks = if ids.is_empty() {
        vec![&[][..]]
    } else {
        ids.chunks(MAXIMUM_PANEL_AGENT_READ_ROWS).collect()
    };
    let mut document = None;
    for chunk in chunks {
        let (id, revision, rows) = exchange(client, snapshot, chunk, deadline, signals)?;
        let doc = document.get_or_insert_with(|| Document {
            rows: BTreeMap::new(),
            revision,
            request_ids: vec![],
            encoded_bytes: 0,
            byte_limit,
        });
        doc.admit(id, revision, rows)?;
    }
    document.ok_or_else(|| Failure::new(FailureKind::InternalFailure))
}
impl Document {
    fn admit(
        &mut self,
        id: WorkbenchRequestId,
        revision: WorkbenchRevision,
        rows: Vec<PanelAgentRow>,
    ) -> Result<(), Failure> {
        if revision != self.revision {
            return Err(changed());
        }
        if rows.iter().any(|row| self.rows.contains_key(&row.panel_id)) {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        let mut encoded_bytes = self.encoded_bytes;
        for row in &rows {
            let size = serde_json::to_vec(row)
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?
                .len();
            encoded_bytes = encoded_bytes
                .checked_add(size)
                .filter(|size| *size <= self.byte_limit)
                .ok_or_else(|| Failure::new(FailureKind::OutputTooLarge))?;
        }
        self.encoded_bytes = encoded_bytes;
        self.rows
            .extend(rows.into_iter().map(|row| (row.panel_id.clone(), row)));
        self.request_ids.push(id);
        Ok(())
    }
    pub(super) fn verify(
        &mut self,
        client: &mut LocalClient,
        snapshot: &Snapshot,
        deadline: Instant,
        signals: &mut ArmedSignals,
    ) -> Result<(), Failure> {
        let (id, revision, rows) = exchange(client, snapshot, &[], deadline, signals)?;
        self.admit(id, revision, rows)
    }
    pub(super) fn view(&self, id: &WorkbenchPanelId) -> Option<&PanelAgentView> {
        self.rows
            .get(id)
            .expect("complete correlated report rows")
            .report
            .as_ref()
    }
    pub(super) fn value(
        &self,
        id: &WorkbenchPanelId,
        site: &orchbee_workbench_protocol::SiteId,
        workbench_revision: WorkbenchRevision,
    ) -> Value {
        let row = self.rows.get(id).expect("complete correlated report rows");
        json!({"source":{"siteId":site,"workbenchRevision":workbench_revision,
            "reportRevision":self.revision,"requestIds":self.request_ids},"binding":row.binding,"view":row.report})
    }
}
#[cfg(test)]
pub(super) fn fixture(
    snapshot: &Snapshot,
    ids: impl IntoIterator<Item = WorkbenchPanelId>,
) -> Document {
    Document {
        rows: ids
            .into_iter()
            .map(|panel_id| {
                let binding = expected(snapshot, &panel_id).unwrap();
                let report = binding.as_ref().map(|_| PanelAgentView {
                    agent: None,
                    state: orchbee_workbench_protocol::PanelAgentState::Unknown,
                    authority_source: None,
                    message: None,
                });
                (
                    panel_id.clone(),
                    PanelAgentRow {
                        panel_id,
                        binding,
                        report,
                    },
                )
            })
            .collect(),
        revision: WorkbenchRevision::new(0),
        request_ids: [902, 903]
            .map(|id| WorkbenchRequestId::parse(&format!("{id:032x}")).unwrap())
            .into(),
        encoded_bytes: 0,
        byte_limit: crate::cli::output::LIMIT,
    }
}

#[cfg(test)]
impl Document {
    pub(super) fn with_view(mut self, view: PanelAgentView) -> Self {
        for row in self.rows.values_mut() {
            if row.binding.is_some() {
                row.report = Some(view.clone());
            }
        }
        self
    }
}
#[cfg(test)]
mod tests;
