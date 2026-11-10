//! `agent list` over the unified Agent projection: one complete projection read, cursor
//! pagination, observed working directories, and optional selected-Client attention per row.
use super::{
    Failure, FailureKind, Output,
    agent_projection::{Item, List, classify, confirm, failure, read, source},
    console_interrupt::{ArmedSignals, Finish, SignalWatch},
    failure_output, query,
    results::{self, Projection},
    stale_retry::{Ended, StaleRetry},
    success_output,
};
use crate::{
    agent_projection::{
        cursor::Cursor,
        read::{Checkpoint, Error},
    },
    invocation::{OutputFormat, QueryCommand, Target},
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_workbench_protocol::{Snapshot, WorkbenchClientId};
use serde_json::json;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

/// Lists one page of the unified projection and, with a selected Client, that Client's attention
/// for every listed Agent.
pub(super) fn run(command: &QueryCommand, format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::AgentProjection) {
        return failure_output(failure, Some(format), locale);
    }
    let QueryCommand::AgentList {
        client: selected,
        include_ended,
        limit,
        cursor,
    } = command
    else {
        return failure_output(classify(Error::Invalid), Some(format), locale);
    };
    let cursor = match super::agent_request::list_cursor(cursor.as_deref(), format, Cursor::parse) {
        Ok(cursor) => cursor,
        Err(failure) => return super::parse_failure(failure, locale),
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(s) => s,
        Err(_) => {
            return failure_output(
                failure(FailureKind::InternalFailure, "unavailable"),
                Some(format),
                locale,
            );
        }
    };
    let mut retry = StaleRetry::<()>::new(true);
    let mut attempt = || {
        let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
        signals.check_before(deadline)?;
        let checkpoint = read(
            &mut client,
            &snapshot,
            *include_ended,
            deadline,
            signals.poll_fd(),
        )?;
        let offset = cursor
            .as_ref()
            .map(|c| {
                c.offset(&snapshot, &checkpoint, *include_ended, selected.as_ref())
                    .map_err(|_| failure(FailureKind::Rejected, "cursor-restart"))
            })
            .transpose()?
            .unwrap_or(0);
        let end = (offset + *limit as usize).min(checkpoint.rows.len());
        let Projection::PanelList(panels) = results::project(
            &QueryCommand::PanelList {
                tab: None,
                workspace: None,
            },
            &snapshot,
        )?
        else {
            return Err(classify(Error::Invalid));
        };
        let handles = super::handles::stable(
            "panel",
            &panels
                .iter()
                .map(|p| p.panel_id.as_str())
                .collect::<Vec<_>>(),
        );
        let handles = panels
            .into_iter()
            .zip(handles)
            .map(|(p, h)| (p.panel_id, h))
            .collect::<BTreeMap<_, _>>();
        let mut items = checkpoint.rows[offset..end]
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, row)| Item {
                ordinal: offset + i + 1,
                panel_handle: row
                    .placement
                    .as_ref()
                    .and_then(|p| handles.get(&p.panel))
                    .cloned(),
                row,
                cwd: None,
                client: None,
            })
            .collect::<Vec<_>>();
        for item in items.iter_mut().take(64) {
            if let Some(p) = &item.row.placement {
                let metadata = query::terminal_metadata::read(
                    &mut client,
                    &snapshot,
                    &Target::Id(p.terminal.clone()),
                    deadline,
                    signals.poll_fd(),
                );
                signals.check_before(deadline)?;
                match metadata {
                    Ok(metadata) => {
                        if item.row.terminal().is_some_and(|t| {
                            metadata.frame.route.host_epoch.to_text()
                                != t.binding.host_epoch.as_str()
                        }) {
                            return Err(classify(Error::Changed));
                        }
                        let fields =
                            super::terminal_observation::Observation::Observed(Box::new(metadata))
                                .fields();
                        item.cwd = fields["cwd"].as_str().map(str::to_owned);
                    }
                    Err(error) if error.kind == FailureKind::ServiceUnavailable => {}
                    Err(error) => return Err(error),
                }
            }
        }
        if let Some(selected) = selected {
            client_memory(
                &mut client,
                &snapshot,
                &checkpoint,
                &mut items,
                selected,
                deadline,
                &mut signals,
            )?;
        }
        confirm(
            &mut client,
            &snapshot,
            &checkpoint,
            *include_ended,
            deadline,
            signals.poll_fd(),
        )?;
        signals.check_before(deadline)?;
        let next = if end < checkpoint.rows.len() {
            Some(
                Cursor::next(
                    &snapshot,
                    &checkpoint,
                    *include_ended,
                    selected.as_ref(),
                    end,
                )
                .map_err(|_| classify(Error::Invalid))?,
            )
        } else {
            None
        };
        Ok((
            source(&snapshot, &checkpoint),
            Projection::UnifiedAgentList(Box::new(List {
                items,
                revision: checkpoint.canonical_revision,
                next,
                client: selected.clone(),
            })),
        ))
    };
    // A read the Workbench outran (ORC-413) repeats under the same deadline while the retry grants
    // fresh reads; the last failure stands once they are spent.
    let answer = loop {
        match attempt() {
            Err(failure) if retry.reread(Ended::resolution(&failure)) => {}
            settled => break settled,
        }
    };
    let (answer, finalization) = match signals.finish() {
        Ok(Finish::Clear) => (answer, None),
        Ok(Finish::Cancelled { finalization, .. }) => {
            (Err(classify(Error::Cancelled)), Some(finalization))
        }
        Err(_) => (
            Err(failure(FailureKind::InternalFailure, "unavailable")),
            None,
        ),
    };
    let mut output = match answer {
        Ok((source, projection)) => success_output(projection, source, command, format, locale),
        Err(error) => failure_output(error, Some(format), locale),
    };
    output.finalization = finalization;
    output
}

fn client_memory(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    checkpoint: &Checkpoint,
    items: &mut [Item],
    selected: &WorkbenchClientId,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(), Failure> {
    use super::agent_view::workflow as attention;
    let initial = attention::current_client(client, snapshot, selected, deadline, signals)?;
    let summaries = items
        .iter()
        .filter_map(|i| i.row.canonical())
        .collect::<Vec<_>>();
    let mut canonical = BTreeMap::new();
    for chunk in summaries.chunks(100) {
        let ids = chunk
            .iter()
            .map(|c| c.session_id.clone())
            .collect::<Vec<_>>();
        let read = attention::read_many(client, snapshot, selected, &ids, deadline, signals)?;
        if *read.view() != initial.view {
            return Err(classify(Error::Changed));
        }
        let views = read.corroborate_summaries(
            client,
            snapshot,
            checkpoint.canonical_revision,
            chunk,
            deadline,
            signals,
        )?;
        for (summary, view) in chunk.iter().zip(views) {
            canonical.insert(summary.session_id.clone(), view.document());
        }
    }
    for item in items {
        let canonical = item
            .row
            .canonical()
            .and_then(|c| canonical.remove(&c.session_id));
        let physical = if let Some(row) = item.row.terminal() {
            let read = attention::terminal_attention::read(
                client,
                snapshot,
                selected,
                &row.binding,
                deadline,
                signals,
            )?;
            if !read.matches_view(&initial.view) {
                return Err(classify(Error::Changed));
            }
            let semantic = query::terminal_agent::read(
                client,
                snapshot,
                &row.binding,
                None,
                deadline,
                signals.poll_fd(),
            )?;
            if semantic.report_revision != checkpoint.report_revision {
                return Err(classify(Error::Changed));
            }
            Some(read.corroborate(&semantic).document())
        } else {
            None
        };
        item.client = Some(json!({"canonical":canonical,"terminal":physical}));
    }
    let final_client = attention::current_client(client, snapshot, selected, deadline, signals)?;
    if final_client.view_revision != initial.view_revision || final_client.view != initial.view {
        return Err(classify(Error::Changed));
    }
    Ok(())
}
