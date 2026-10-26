//! One revision-consistent observation of logical-terminal reports, shared by Panel and Agent reads.
mod reports;
use super::{
    Failure, FailureKind,
    output::{Bounded, TooLarge},
    query,
    source::{ArmedSignals, SignalWatch},
};
use crate::{
    local_client::{LocalClient, workbench::metadata_exchange::MetadataExchangeReason},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    MAXIMUM_PANEL_METADATA_READ_ROWS, PanelMetadataAction, PanelMetadataFailure,
    PanelMetadataOutcome, PanelMetadataRequest, PanelMetadataRow, PanelNode, PresentationBinding,
    Snapshot, WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

#[derive(Debug)]
pub(super) struct Document {
    reports: Option<reports::Document>,
    rows: BTreeMap<WorkbenchPanelId, PanelMetadataRow>,
    encoded_row_bytes: usize,
    revision: WorkbenchRevision,
    request_ids: Vec<WorkbenchRequestId>,
    site: orchbee_workbench_protocol::SiteId,
    workbench_revision: WorkbenchRevision,
}
fn binding<'a>(
    node: &'a PanelNode,
    id: &WorkbenchPanelId,
) -> Option<&'a Option<PresentationBinding>> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } if panel_id == id => Some(&primary_presentation.binding),
        PanelNode::Panel { .. } => None,
        PanelNode::Split { children, .. } => {
            children.iter().find_map(|child| binding(&child.node, id))
        }
    }
}
fn expected(
    snapshot: &Snapshot,
    id: &WorkbenchPanelId,
) -> Result<Option<PresentationBinding>, Failure> {
    snapshot
        .terminal_contexts
        .iter()
        .flat_map(|context| &context.tabs)
        .find_map(|tab| binding(&tab.root, id))
        .cloned()
        .ok_or_else(|| Failure::new(FailureKind::NotFound))
}
fn changed() -> Failure {
    super::workbench_agents::failure("metadata-changed")
}
fn observe(
    snapshot: &Snapshot,
    outcome: PanelMetadataOutcome,
) -> Result<(WorkbenchRevision, Vec<PanelMetadataRow>), Failure> {
    match outcome {
        PanelMetadataOutcome::Read {
            workbench_revision,
            metadata_revision,
            panels,
        } => {
            if workbench_revision != snapshot.revision {
                return Err(super::workbench_agents::failure("workbench-changed"));
            }
            for row in &panels {
                if row.binding != expected(snapshot, &row.panel_id)? {
                    return Err(Failure::new(FailureKind::ProtocolInvalid));
                }
            }
            Ok((metadata_revision, panels))
        }
        PanelMetadataOutcome::Failed {
            reason:
                PanelMetadataFailure::NotFound
                | PanelMetadataFailure::SiteMismatch
                | PanelMetadataFailure::BindingChanged,
        } => Err(super::workbench_agents::failure("workbench-changed")),
        PanelMetadataOutcome::Failed { reason } => Err(Failure::new(match reason {
            PanelMetadataFailure::Denied => FailureKind::Denied,
            PanelMetadataFailure::Unavailable => FailureKind::ServiceUnavailable,
            PanelMetadataFailure::RevisionExhausted | PanelMetadataFailure::DeadlineOverflow => {
                FailureKind::InternalFailure
            }
            PanelMetadataFailure::ResultTooLarge => FailureKind::OutputTooLarge,
            _ => FailureKind::ProtocolInvalid,
        })),
        _ => Err(Failure::new(FailureKind::ProtocolInvalid)),
    }
}
fn send(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    action: PanelMetadataAction,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(WorkbenchRequestId, PanelMetadataOutcome), Failure> {
    signals.check_before(deadline)?;
    let request_id = query::request_id()?;
    let reply = client.panel_metadata(
        PanelMetadataRequest {
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
fn exchange(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    ids: &[WorkbenchPanelId],
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(WorkbenchRequestId, WorkbenchRevision, Vec<PanelMetadataRow>), Failure> {
    let (id, outcome) = send(
        client,
        snapshot,
        PanelMetadataAction::Read {
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
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Document, Failure> {
    let (id, outcome) = send(
        client,
        snapshot,
        PanelMetadataAction::ReadPresentations {
            presentation_ids: vec![member.presentation_id.clone()],
        },
        deadline,
        signals,
    )?;
    let (workbench_revision, revision, mut rows) = match outcome {
        PanelMetadataOutcome::PresentationsRead {
            workbench_revision,
            metadata_revision,
            presentations,
        } => (workbench_revision, metadata_revision, presentations),
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
        reports: None,
        rows: BTreeMap::new(),
        encoded_row_bytes: 0,
        revision,
        request_ids: vec![],
        site: snapshot.site_id.clone(),
        workbench_revision,
    };
    document.admit(id, revision, vec![row.panel])?;
    document.reports = Some(reports::read_presentation(
        client,
        snapshot,
        member,
        super::output::LIMIT - document.encoded_row_bytes,
        deadline,
        signals,
    )?);
    Ok(document)
}

pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    ids: &[WorkbenchPanelId],
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Document, Failure> {
    if ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(Failure::new(FailureKind::InternalFailure));
    }
    for id in ids {
        expected(snapshot, id)?;
    }
    let chunks = if ids.is_empty() {
        vec![&[][..]]
    } else {
        ids.chunks(MAXIMUM_PANEL_METADATA_READ_ROWS).collect()
    };
    let mut document = None;
    for chunk in chunks {
        let (request_id, revision, rows) = exchange(client, snapshot, chunk, deadline, signals)?;
        let doc = document.get_or_insert_with(|| Document {
            reports: None,
            rows: BTreeMap::new(),
            encoded_row_bytes: 0,
            revision,
            request_ids: vec![],
            site: snapshot.site_id.clone(),
            workbench_revision: snapshot.revision,
        });
        doc.admit(request_id, revision, rows)?;
    }
    let mut document = document.ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    document.reports = Some(reports::read(
        client,
        snapshot,
        ids,
        super::output::LIMIT - document.encoded_row_bytes,
        deadline,
        signals,
    )?);
    Ok(document)
}
impl Document {
    fn admit(
        &mut self,
        id: WorkbenchRequestId,
        revision: WorkbenchRevision,
        rows: Vec<PanelMetadataRow>,
    ) -> Result<(), Failure> {
        if revision != self.revision {
            return Err(changed());
        }
        if rows.iter().any(|row| self.rows.contains_key(&row.panel_id)) {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        let mut encoded_row_bytes = self.encoded_row_bytes;
        for row in &rows {
            let size = serde_json::to_vec(row)
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?
                .len();
            encoded_row_bytes = encoded_row_bytes
                .checked_add(size)
                .filter(|size| *size <= super::output::LIMIT)
                .ok_or_else(|| Failure::new(FailureKind::OutputTooLarge))?;
        }
        self.encoded_row_bytes = encoded_row_bytes;
        self.request_ids.push(id);
        self.rows
            .extend(rows.into_iter().map(|row| (row.panel_id.clone(), row)));
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
        self.admit(id, revision, rows)?;
        self.reports
            .as_mut()
            .expect("completed report read")
            .verify(client, snapshot, deadline, signals)
    }
    pub(super) fn value(&self, id: &WorkbenchPanelId) -> Value {
        let row = self
            .rows
            .get(id)
            .expect("complete correlated metadata rows");
        let view = self
            .reports
            .as_ref()
            .expect("completed report read")
            .view(id);
        let agent = view.and_then(|view| view.agent.as_deref());
        let authority = view.and_then(|view| view.authority_source.as_deref());
        json!({"source":{"siteId":self.site,"workbenchRevision":self.workbench_revision,"metadataRevision":self.revision,"requestIds":self.request_ids},"binding":row.binding,"tokens":row.tokens,"presentations":row.presentations,"context":{"agent":agent,"authoritySource":authority},"presentation":row.presentation(agent,authority)})
    }
    pub(super) fn report_value(&self, id: &WorkbenchPanelId) -> Value {
        self.reports.as_ref().expect("completed report read").value(
            id,
            &self.site,
            self.workbench_revision,
        )
    }
    pub(super) fn enrich(&self, result: &mut Value) {
        let enrich = |row: &mut Value| {
            let id = WorkbenchPanelId::parse(row["panelId"].as_str().expect("typed Panel row"))
                .expect("typed Panel ID");
            row["metadata"] = self.value(&id);
            row["reportedAgent"] = self.report_value(&id);
        };
        if let Some(rows) = result.get_mut("panels").and_then(Value::as_array_mut) {
            rows.iter_mut().for_each(enrich);
        }
        if let Some(row) = result.get_mut("panel") {
            enrich(row);
        }
    }
    pub(super) fn render(&self, out: &mut Bounded, locale: Locale) -> Result<(), TooLarge> {
        out.write(format_args!("{}\n", locale.cli_panel_metadata_heading()))?;
        for (id, row) in &self.rows {
            let report = self
                .reports
                .as_ref()
                .expect("completed report read")
                .view(id);
            let presentation = row.presentation(
                report.and_then(|v| v.agent.as_deref()),
                report.and_then(|v| v.authority_source.as_deref()),
            );
            out.write(format_args!(
                "{}  {}  {}\n",
                id,
                super::escape::escape(presentation.title.as_ref().map_or("-", |v| v.as_str())),
                super::escape::escape(
                    presentation
                        .display_agent
                        .as_ref()
                        .map_or("-", |v| v.as_str())
                )
            ))?;
            render_report(out, &self.report_value(id), locale)?;
            for (key, value) in row.tokens.as_map() {
                out.write(format_args!(
                    "{}  {}={}\n",
                    id,
                    super::escape::escape(key.as_str()),
                    super::escape::escape(value.as_str())
                ))?;
            }
            for (state, value) in presentation.state_labels {
                out.write(format_args!(
                    "{}  {}={}\n",
                    id,
                    serde_json::to_string(&state).expect("closed state"),
                    super::escape::escape(value.as_str())
                ))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn empty_fixture(
    snapshot: &Snapshot,
    projection: &super::results::Projection,
) -> Option<Document> {
    let rows: Vec<_> = match projection {
        super::results::Projection::PanelList(rows) => rows.iter().collect(),
        super::results::Projection::PanelGet(doc) => vec![&doc.row],
        _ => return None,
    };
    Some(Document {
        reports: Some(reports::fixture(
            snapshot,
            rows.iter()
                .map(|row| WorkbenchPanelId::parse(&row.panel_id).unwrap()),
        )),
        encoded_row_bytes: 0,
        rows: rows
            .into_iter()
            .map(|row| {
                let panel_id = WorkbenchPanelId::parse(&row.panel_id).unwrap();
                let row = PanelMetadataRow {
                    binding: expected(snapshot, &panel_id).unwrap(),
                    panel_id: panel_id.clone(),
                    tokens: Default::default(),
                    presentations: vec![],
                };
                (panel_id, row)
            })
            .collect(),
        revision: WorkbenchRevision::new(0),
        request_ids: vec![
            WorkbenchRequestId::parse(&format!("{:032x}", 900)).unwrap(),
            WorkbenchRequestId::parse(&format!("{:032x}", 901)).unwrap(),
        ],
        site: snapshot.site_id.clone(),
        workbench_revision: snapshot.revision,
    })
}
#[cfg(test)]
mod tests;

pub(in crate::cli) fn render_report(
    out: &mut Bounded,
    report: &Value,
    locale: Locale,
) -> Result<(), TooLarge> {
    let view = &report["view"];
    if view.is_null() {
        return Ok(());
    }
    let labels = locale.cli_reported_agent_fields();
    for (label, value) in labels[..4].iter().zip([
        &view["agent"],
        &view["state"],
        &view["authoritySource"],
        &view["message"],
    ]) {
        out.write(format_args!(
            "{label}: {}\n",
            super::escape::escape(value.as_str().unwrap_or("-"))
        ))?;
    }
    let reference = &view["session"]["reference"];
    if !reference.is_null() {
        out.write(format_args!(
            "{}: {} {}\n",
            labels[4],
            super::escape::escape(reference["kind"].as_str().expect("typed reference kind")),
            super::escape::escape(reference["value"].as_str().expect("typed reference value"))
        ))?;
    }
    Ok(())
}
