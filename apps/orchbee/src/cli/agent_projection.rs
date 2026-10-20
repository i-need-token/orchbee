//! CLI adapters consume one complete projection for pagination, selectors, and panel tables.
use super::{
    Failure, FailureKind, Output, failure_output, query,
    results::{self, Projection},
    source::{ArmedSignals, Finish, SignalWatch},
    success_output,
};
use crate::{
    agent_projection::{
        self as model,
        cursor::Cursor,
        read::{Checkpoint, Error},
    },
    invocation::{AgentSelector, OutputFormat, QueryCommand, Target},
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_agent_observation_protocol::AgentNameTarget;
use orchbee_workbench_protocol::{Snapshot, WorkbenchClientId};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::atomic::AtomicU8,
    time::{Duration, Instant},
};
mod present;
pub(super) use present::{render_inspection, render_list};
pub(super) const FAILURE_SCHEMA: &str = "orchbee.cli.failure.agent-projection";

#[derive(Debug)]
pub(crate) struct Item {
    pub(super) ordinal: usize,
    pub(super) row: model::Row,
    pub(super) panel_handle: Option<String>,
    pub(super) cwd: Option<String>,
    pub(super) client: Option<Value>,
}
#[derive(Debug)]
pub(crate) struct List {
    pub(super) items: Vec<Item>,
    pub(super) revision: orchbee_agent_observation_protocol::AgentSafeInteger,
    pub(super) next: Option<Cursor>,
    pub(super) client: Option<WorkbenchClientId>,
}
impl List {
    pub(super) fn document(&self) -> Value {
        let items = self
            .items
            .iter()
            .map(|i| {
                let mut row = i.row.document(i.ordinal, i.cwd.as_deref());
                if let Some(client) = &i.client {
                    row["client"] = client.clone()
                }
                row
            })
            .collect::<Vec<_>>();
        let mut result = json!({"kind":"listed","revision":self.revision,"nextCursor":self.next.as_ref().map(Cursor::as_str),"items":items});
        if let Some(client) = &self.client {
            result["clientId"] = json!(client)
        }
        result
    }
}
#[derive(Debug)]
pub(crate) struct Inspection {
    pub(super) row: model::Row,
    pub(super) detail: Box<Projection>,
}
impl Inspection {
    pub(super) fn document(&self) -> Value {
        let detail = self.detail.result();
        let mut row = self.row.document(1, detail["terminal"]["cwd"].as_str());
        row.as_object_mut()
            .expect("typed Agent row")
            .remove("ordinal");
        json!({"kind":"projected","agent":row,"inspection":detail})
    }
}
pub(super) fn failure(kind: FailureKind, reason: &str) -> Failure {
    Failure::new(kind)
        .failure_schema(FAILURE_SCHEMA)
        .details(super::stage_reason_details("agent-projection", reason))
}
fn classify(error: Error) -> Failure {
    let (kind, reason) = match error {
        Error::Unavailable => (FailureKind::ServiceUnavailable, "unavailable"),
        Error::Denied => (FailureKind::Denied, "denied"),
        Error::Changed => (FailureKind::Rejected, "changed"),
        Error::Invalid => (FailureKind::ProtocolInvalid, "invalid"),
        Error::TooLarge => (FailureKind::OutputTooLarge, "too-large"),
        Error::Timeout => (FailureKind::Timeout, "timeout"),
        Error::Cancelled => (FailureKind::Cancelled, "cancelled"),
    };
    failure(kind, reason)
}
fn cancellation<'a>(
    wake: Option<orchbee_local_ipc::LocalIo<'a>>,
    idle: &'a AtomicU8,
) -> Result<&'a AtomicU8, Failure> {
    match wake {
        Some(orchbee_local_ipc::LocalIo::Wake(flag, _)) => Ok(flag),
        None => Ok(idle),
        _ => Err(classify(Error::Invalid)),
    }
}
pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    ended: bool,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<Checkpoint, Failure> {
    let idle = AtomicU8::new(0);
    model::read::read(
        client,
        snapshot,
        ended,
        deadline,
        cancellation(wake, &idle)?,
    )
    .map_err(classify)
}
pub(super) fn confirm(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    read: &Checkpoint,
    ended: bool,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<(), Failure> {
    let idle = AtomicU8::new(0);
    model::read::confirm(
        client,
        snapshot,
        read,
        ended,
        deadline,
        cancellation(wake, &idle)?,
    )
    .map_err(classify)
}
pub(super) fn source(snapshot: &Snapshot, read: &Checkpoint) -> Value {
    json!({"kind":"agent-projection","siteId":snapshot.site_id,"workbenchRevision":snapshot.revision,
        "reportRevision":read.report_revision,"canonicalRevision":read.canonical_revision})
}

pub(super) fn select<'a>(
    rows: &'a [model::Row],
    selector: &AgentSelector,
) -> Vec<(usize, &'a model::Row)> {
    match selector {
        AgentSelector::Ordinal(n) => rows.get(n - 1).map(|r| vec![(*n, r)]).unwrap_or_default(),
        AgentSelector::Kind(kind) => {
            let kind = model::provider(kind).unwrap_or(kind);
            rows.iter()
                .enumerate()
                .filter(|(_, r)| r.provider() == kind)
                .map(|(i, r)| (i + 1, r))
                .collect()
        }
    }
}
fn choose(matches: Vec<(usize, &model::Row)>, token: &str) -> Result<AgentNameTarget, Failure> {
    if let [(_, row)] = matches.as_slice() {
        return Ok(row.target());
    }
    let kind = if matches.is_empty() {
        FailureKind::NotFound
    } else {
        FailureKind::Ambiguous
    };
    let mut details = super::stage_reason_details(
        "agent-projection",
        if matches.is_empty() {
            "target-not-found"
        } else {
            "target-ambiguous"
        },
    );
    details.insert("selector".into(), json!(token));
    details.insert("candidates".into(),json!(matches.iter().take(16).map(|(n,row)|json!({
        "selector":format!("@agent:{n}"),"sessionId":row.canonical().map(|c|&c.session_id),
        "panelId":row.placement.as_ref().map(|p|&p.panel),"binding":row.terminal().map(|t|&t.binding),
    })).collect::<Vec<_>>()));
    Err(Failure::new(kind)
        .failure_schema(FAILURE_SCHEMA)
        .details(details))
}
pub(super) fn resolve(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    selector: &AgentSelector,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<AgentNameTarget, Failure> {
    let checkpoint = read(client, snapshot, false, deadline, wake)?;
    choose(select(&checkpoint.rows, selector), &selector.token())
}
pub(super) fn panel(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    target: &Target,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<AgentNameTarget, Failure> {
    let Projection::PanelGet(panel) =
        results::project(&QueryCommand::PanelGet(target.clone()), snapshot)?
    else {
        return Err(classify(Error::Invalid));
    };
    let checkpoint = read(client, snapshot, false, deadline, wake)?;
    choose(
        checkpoint
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.in_presentation(&panel.primary_presentation.presentation_id))
            .map(|(i, r)| (i + 1, r))
            .collect(),
        &panel.row.panel_id,
    )
}
pub(super) fn terminal(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    terminal: &results::TerminalRow,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<AgentNameTarget, Failure> {
    let checkpoint = read(client, snapshot, false, deadline, wake)?;
    choose(
        checkpoint
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.in_presentation(&terminal.presentation_id))
            .map(|(index, row)| (index + 1, row))
            .collect(),
        &terminal.terminal_id,
    )
}
pub(super) fn inspection_row(
    checkpoint: &Checkpoint,
    target: &AgentNameTarget,
) -> Result<model::Row, Failure> {
    checkpoint
        .rows
        .iter()
        .find(|row| match target {
            AgentNameTarget::Session { session_id } => {
                row.canonical().is_some_and(|c| &c.session_id == session_id)
            }
            AgentNameTarget::Terminal { binding } => {
                row.terminal().is_some_and(|t| &t.binding == binding)
            }
        })
        .cloned()
        .ok_or_else(|| failure(FailureKind::NotFound, "target-not-found"))
}

pub(super) fn run(command: &QueryCommand, format: OutputFormat, locale: Locale) -> Output {
    let QueryCommand::AgentList {
        client: selected,
        include_ended,
        limit,
        cursor,
    } = command
    else {
        return failure_output(classify(Error::Invalid), Some(format), locale);
    };
    if cursor.as_ref().is_some_and(Cursor::legacy) {
        return failure_output(
            failure(FailureKind::Rejected, "cursor-restart"),
            Some(format),
            locale,
        );
    }
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
    let answer = (|| {
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
    })();
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
    use super::agent_view::windows as attention;
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
        if !read.matches_view(&initial.view) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_projection::tests::{rows, state, summary};

    #[test]
    fn selectors_preserve_global_ordinals_aliases_and_ambiguity() {
        let rows = rows(
            vec![summary("session-a", 12, "codex")],
            vec![
                state(12, Some("codex")),
                state(22, Some("grok-build")),
                state(23, Some("codex")),
            ],
        );
        let selected = select(&rows, &AgentSelector::Ordinal(2));
        assert_eq!(selected[0].0, 2);
        assert_eq!(
            choose(selected, "@agent:2").unwrap(),
            AgentNameTarget::Terminal {
                binding: crate::machine_agents::tests::physical(22)
            }
        );
        assert_eq!(
            select(&rows, &AgentSelector::Kind("grok-build".into()))[0].0,
            2
        );
        let failure = choose(
            select(&rows, &AgentSelector::Kind("codex".into())),
            "@agent:codex",
        )
        .unwrap_err();
        assert_eq!(failure.kind, FailureKind::Ambiguous);
        let details = json!(failure.details);
        assert_eq!(details["candidates"][0]["selector"], "@agent:1");
        assert_eq!(details["candidates"][0]["sessionId"], "session-a");
        assert_eq!(details["candidates"][1]["selector"], "@agent:3");
        assert!(details["candidates"][1]["sessionId"].is_null());
        assert_eq!(
            choose(select(&rows, &AgentSelector::Ordinal(4)), "@agent:4")
                .unwrap_err()
                .kind,
            FailureKind::NotFound
        );
    }
    #[test]
    fn inspection_never_substitutes_a_replaced_terminal_incarnation() {
        let checkpoint = Checkpoint {
            canonical_revision: orchbee_agent_observation_protocol::AgentSafeInteger::new(2),
            report_revision: orchbee_workbench_protocol::WorkbenchRevision::new(7),
            rows: rows(Vec::new(), vec![state(12, Some("codex"))]),
        };
        let mut binding = crate::machine_agents::tests::physical(12);
        binding.host_epoch =
            orchbee_agent_observation_protocol::AgentHostEpoch::parse(&format!("{:032x}", 9))
                .unwrap();
        assert_eq!(
            inspection_row(&checkpoint, &AgentNameTarget::Terminal { binding })
                .unwrap_err()
                .kind,
            FailureKind::NotFound
        );
    }
}
