//! Exact Workspace token batches and a final metadata revision fence on one connection.
use super::{
    Failure, FailureKind,
    output::{Bounded, TooLarge},
    query,
    results::Projection,
    source::{ArmedSignals, SignalWatch},
};
use crate::{
    local_client::{LocalClient, workbench::metadata_exchange::MetadataExchangeReason},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    MAXIMUM_WORKSPACE_METADATA_READ_ROWS, MetadataTokens, Snapshot, WorkbenchRequestId,
    WorkbenchRevision, WorkbenchWorkspaceId, WorkspaceMetadataAction,
    WorkspaceMetadataFailure as Reason, WorkspaceMetadataOutcome as Outcome,
    WorkspaceMetadataRequest,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Instant};
#[derive(Debug)]
pub(super) struct Document {
    rows: BTreeMap<WorkbenchWorkspaceId, MetadataTokens>,
    revision: WorkbenchRevision,
    request_ids: Vec<WorkbenchRequestId>,
    source: Value,
}
fn exchange(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    ids: Vec<WorkbenchWorkspaceId>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<
    (
        WorkbenchRequestId,
        WorkbenchRevision,
        Vec<orchbee_workbench_protocol::WorkspaceMetadataRow>,
    ),
    Failure,
> {
    signals.check_before(deadline)?;
    let request_id = query::request_id()?;
    let reply = client.workspace_metadata(
        WorkspaceMetadataRequest {
            protocol_version: snapshot.protocol_version,
            request_id: request_id.clone(),
            site_id: snapshot.site_id.clone(),
            action: WorkspaceMetadataAction::Read { workspace_ids: ids },
        },
        deadline,
        signals.poll_fd(),
    );
    signals.check_before(deadline)?;
    let reply = reply.map_err(|error| {
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
    match reply.outcome {
        Outcome::Read {
            workbench_revision,
            metadata_revision,
            workspaces,
        } => {
            if workbench_revision != snapshot.revision {
                return Err(super::workbench_agents::failure("workbench-changed"));
            }
            Ok((request_id, metadata_revision, workspaces))
        }
        Outcome::Failed {
            reason: Reason::NotFound | Reason::SiteMismatch,
        } => Err(super::workbench_agents::failure("workbench-changed")),
        Outcome::Failed { reason } => Err(Failure::new(match reason {
            Reason::Denied => FailureKind::Denied,
            Reason::Unavailable => FailureKind::ServiceUnavailable,
            Reason::RevisionExhausted | Reason::DeadlineOverflow => FailureKind::InternalFailure,
            Reason::ResultTooLarge => FailureKind::OutputTooLarge,
            _ => FailureKind::ProtocolInvalid,
        })),
        _ => Err(Failure::new(FailureKind::ProtocolInvalid)),
    }
}
pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    projection: &Projection,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Option<Document>, Failure> {
    let rows: Vec<_> = match projection {
        Projection::WorkspaceList(rows) => rows.iter().collect(),
        Projection::WorkspaceGet(doc) => vec![&doc.row],
        _ => return Ok(None),
    };
    let ids = rows
        .iter()
        .map(|row| {
            WorkbenchWorkspaceId::parse(&row.workspace_id)
                .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut document: Option<Document> = None;
    let chunks = if ids.is_empty() {
        vec![&[][..]]
    } else {
        ids.chunks(MAXIMUM_WORKSPACE_METADATA_READ_ROWS).collect()
    };
    for chunk in chunks {
        let (request_id, revision, rows) =
            exchange(client, snapshot, chunk.to_vec(), deadline, signals)?;
        let doc = document.get_or_insert_with(|| Document { rows: BTreeMap::new(), revision, request_ids: vec![],
            source: json!({"siteId":snapshot.site_id,"workbenchRevision":snapshot.revision,"metadataRevision":revision}) });
        if doc.revision != revision {
            return Err(super::workbench_agents::failure("metadata-changed"));
        }
        doc.request_ids.push(request_id);
        doc.rows
            .extend(rows.into_iter().map(|row| (row.workspace_id, row.tokens)));
    }
    Ok(document)
}
impl Document {
    pub(super) fn verify(
        &mut self,
        client: &mut LocalClient,
        snapshot: &Snapshot,
        deadline: Instant,
        signals: &mut ArmedSignals,
    ) -> Result<(), Failure> {
        let (id, revision, _) = exchange(client, snapshot, vec![], deadline, signals)?;
        if self.revision != revision {
            return Err(super::workbench_agents::failure("metadata-changed"));
        }
        self.request_ids.push(id);
        Ok(())
    }
    pub(super) fn enrich(&self, value: &mut Value) {
        let mut source = self.source.clone();
        source["requestIds"] = json!(self.request_ids);
        value["metadataSource"] = source;
        let enrich = |row: &mut Value| {
            let id =
                WorkbenchWorkspaceId::parse(row["workspaceId"].as_str().expect("typed workspace"))
                    .expect("typed identity");
            row["tokens"] = json!(self.rows.get(&id).expect("complete exact token rows"));
        };
        if let Some(rows) = value.get_mut("workspaces").and_then(Value::as_array_mut) {
            rows.iter_mut().for_each(enrich);
        }
        if let Some(row) = value.get_mut("workspace") {
            enrich(row);
        }
    }
    pub(super) fn render(&self, out: &mut Bounded, locale: Locale) -> Result<(), TooLarge> {
        out.write(format_args!(
            "{}\n",
            locale.cli_workspace_metadata_heading()
        ))?;
        for (id, tokens) in &self.rows {
            for (key, value) in tokens.as_map() {
                out.write(format_args!(
                    "{}  {}={}\n",
                    id,
                    super::escape::escape(key.as_str()),
                    super::escape::escape(value.as_str())
                ))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn empty_fixture(snapshot: &Snapshot, projection: &Projection) -> Option<Document> {
    let rows: Vec<_> = match projection {
        Projection::WorkspaceList(rows) => rows.iter().collect(),
        Projection::WorkspaceGet(doc) => vec![&doc.row],
        _ => return None,
    };
    Some(Document {
        rows: rows
            .iter()
            .map(|row| {
                (
                    WorkbenchWorkspaceId::parse(&row.workspace_id).unwrap(),
                    MetadataTokens::default(),
                )
            })
            .collect(),
        revision: WorkbenchRevision::new(0),
        request_ids: vec![WorkbenchRequestId::parse(&format!("{:032x}", 900)).unwrap()],
        source: json!({"siteId":snapshot.site_id,"workbenchRevision":snapshot.revision,"metadataRevision":"0"}),
    })
}
