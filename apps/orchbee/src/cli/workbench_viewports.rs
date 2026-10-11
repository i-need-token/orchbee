//! An exact Client supplies scroll facts; a fresh Host viewer cannot substitute for it.
use super::{Failure, FailureKind, query, results::Projection, source::ArmedSignals};
use crate::local_client::LocalClient;
use orchbee_workbench_protocol::{
    ClientViewControl, ClientViewControlStatus, ClientViewIntent, ClientViewOutcome,
    ClientViewRecord, ClientViewportMode, ClientViewportObservation, ClientViewportPending,
    ClientViewportUnavailable, MAXIMUM_CLIENT_VIEWPORT_PANELS, Snapshot, TerminalId,
    WorkbenchPanelId, WorkbenchRequestId,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Instant};

#[derive(Debug)]
pub(super) struct Document {
    pub(super) operations: Vec<WorkbenchRequestId>,
    rows: BTreeMap<String, ClientViewportObservation>,
}
impl Document {
    pub(super) fn render(
        &self,
        out: &mut super::output::Bounded,
        locale: crate::localization::Locale,
    ) -> Result<(), super::output::TooLarge> {
        out.write(format_args!("{}\n", locale.cli_scroll_heading()))?;
        let states = locale.cli_scroll_states();
        for (id, observation) in &self.rows {
            let state = match observation {
                ClientViewportObservation::Unavailable { reason } => match reason {
                    ClientViewportUnavailable::NotAttached => 0,
                    ClientViewportUnavailable::FrameUnavailable => 1,
                },
                ClientViewportObservation::Pending { phase } => match phase {
                    ClientViewportPending::Entering => 2,
                    ClientViewportPending::AwaitingWindow => 3,
                    ClientViewportPending::Recovering => 4,
                },
                ClientViewportObservation::Observed { mode, .. } => {
                    if *mode == ClientViewportMode::Live {
                        5
                    } else {
                        6
                    }
                }
            };
            out.write(format_args!("{}  {}", id, states[state]))?;
            if let ClientViewportObservation::Observed { scroll, .. } = observation {
                out.write(format_args!(
                    "  {} / {} / {}",
                    scroll.offset_from_bottom, scroll.max_offset_from_bottom, scroll.viewport_rows
                ))?;
            }
            out.write(format_args!("\n"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn unobserved_fixture(projection: &Projection) -> Option<Document> {
    let rows: Vec<_> = match projection {
        Projection::PanelList(rows) => rows.iter().collect(),
        Projection::PanelGet(doc) => vec![&doc.row],
        _ => return None,
    };
    Some(Document {
        operations: vec![],
        rows: rows
            .into_iter()
            .map(|row| {
                (
                    row.panel_id.clone(),
                    ClientViewportObservation::Unavailable {
                        reason: ClientViewportUnavailable::NotAttached,
                    },
                )
            })
            .collect(),
    })
}
pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    projection: &Projection,
    selected: Option<&ClientViewRecord>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Option<Document>, Failure> {
    let rows: Vec<_> = match projection {
        Projection::PanelList(rows) => rows.iter().collect(),
        Projection::PanelGet(doc) => vec![&doc.row],
        _ => return Ok(None),
    };
    let Some(selected) = selected else {
        return Ok(None);
    };
    let invalid = || Failure::new(FailureKind::ProtocolInvalid);
    let mut document = Document {
        operations: vec![],
        rows: BTreeMap::new(),
    };
    for batch in rows.chunks(MAXIMUM_CLIENT_VIEWPORT_PANELS) {
        signals.check_before(deadline)?;
        let ids = batch
            .iter()
            .map(|row| WorkbenchPanelId::parse(&row.panel_id).map_err(|_| invalid()))
            .collect::<Result<Vec<_>, _>>()?;
        let command = ClientViewControl {
            operation_id: query::request_id()?,
            client_id: selected.client_id.clone(),
            expected_view_revision: selected.view_revision,
            minimum_workbench_revision: snapshot.revision,
            intent: ClientViewIntent::Viewports {
                panel_ids: ids.clone(),
            },
        };
        let outcome = super::client_control::windows::submit_control(
            client,
            &snapshot.site_id,
            command.clone(),
            deadline,
            signals,
        );
        signals.check_before(deadline)?;
        let answer = outcome?;
        let outcome = answer.reply.as_ref().map(|reply| &reply.outcome);
        if let Some(ClientViewOutcome::Operation { operation }) = outcome
            && operation.command != command
        {
            return Err(invalid());
        }
        if let Some(kind) = super::workbench_query::submission_failure(answer.exit_code(), outcome)
        {
            return Err(Failure::new(kind));
        }
        let Some(reply) = answer.reply else {
            return Err(invalid());
        };
        let ClientViewOutcome::Operation { operation } = reply.outcome else {
            return Err(invalid());
        };
        let ClientViewControlStatus::ViewportRead { viewport, view } = operation.status else {
            return Err(invalid());
        };
        if viewport.workbench_revision != snapshot.revision || view != selected.view {
            return Err(super::workbench_agents::failure("client-changed"));
        }
        if !viewport.answers(&ids, &view, snapshot.revision)
            || viewport.panels.iter().zip(batch).any(|(actual, expected)| {
                actual.primary_terminal_id.as_ref().map(TerminalId::as_str)
                    != expected.primary_terminal_id.as_deref()
            })
        {
            return Err(invalid());
        }
        document.operations.push(command.operation_id);
        for row in viewport.panels {
            document
                .rows
                .insert(row.panel_id.as_str().to_owned(), row.observation);
        }
    }
    Ok(Some(document))
}
pub(super) fn enrich(document: Option<&Document>, result: &mut Value) {
    let mut row = |row: &mut Value| {
        let observation = document.map(|document| {
            document
                .rows
                .get(row["panelId"].as_str().expect("typed Panel ID"))
                .expect("complete viewport read")
        });
        row["scroll"] = match observation {
            Some(ClientViewportObservation::Observed { scroll, .. }) => json!(scroll),
            _ => Value::Null,
        };
        row["scrollObservation"] =
            observation.map_or_else(|| json!({"state":"no-client"}), |value| json!(value));
    };
    if let Some(rows) = result.get_mut("panels").and_then(Value::as_array_mut) {
        rows.iter_mut().for_each(&mut row);
    }
    if let Some(panel) = result.get_mut("panel") {
        row(panel);
    }
}

/// Reuse the exact installed-member observation captured before enrichment.
pub(super) fn from_current(member: &super::current_presentation::Resolved) -> Option<Document> {
    member.client().map(|read| Document {
        operations: vec![],
        rows: BTreeMap::from([(
            member.panel_id.to_string(),
            read.member
                .viewport()
                .cloned()
                .unwrap_or(ClientViewportObservation::Unavailable {
                    reason: ClientViewportUnavailable::NotAttached,
                }),
        )]),
    })
}
