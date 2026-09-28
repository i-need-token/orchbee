//! Agent observations joined to canonical primary Presentation membership.
use super::{
    Failure, FailureKind,
    agent_view::windows as attention,
    output::{Bounded, TooLarge},
    query,
    results::{self, AgentClientViewDoc, Projection},
    source::{ArmedSignals, SignalWatch},
};
use crate::{invocation::QueryCommand, local_client::LocalClient, localization::Locale};
use orchbee_agent_observation_protocol::{
    AgentLifecycle, AgentListCursor, AgentListOutcome, AgentObservationFreshness, AgentSafeInteger,
    AgentSemanticStatus, AgentSessionId, AgentSessionSummary, ObapRequestId,
};
use orchbee_workbench_protocol::{ClientViewRecord, Snapshot};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

const MAXIMUM_SESSIONS: usize = 512;
const PAGE_SIZE: usize = 100;
const MAXIMUM_PAGES: usize = MAXIMUM_SESSIONS.div_ceil(PAGE_SIZE);

pub(super) fn failure(reason: &'static str) -> Failure {
    Failure::new(if reason == "agent-page-invalid" {
        FailureKind::ProtocolInvalid
    } else {
        FailureKind::Rejected
    })
    .failure_schema("orchbee.cli.failure.workbench-query")
    .version(2)
    .details(super::stage_reason_details("workbench-query", reason))
}

#[derive(Debug)]
struct Session {
    summary: AgentSessionSummary,
    client: Option<AgentClientViewDoc>,
}

#[derive(Debug)]
pub(super) struct Document {
    revision: AgentSafeInteger,
    requests: Vec<ObapRequestId>,
    sessions: Vec<Session>,
    rows: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Default)]
struct Pages {
    revision: Option<AgentSafeInteger>,
    items: Vec<AgentSessionSummary>,
    requests: Vec<ObapRequestId>,
    cursors: BTreeSet<String>,
    ids: BTreeSet<AgentSessionId>,
}
impl Pages {
    fn push(
        &mut self,
        request: ObapRequestId,
        outcome: AgentListOutcome,
    ) -> Result<Option<AgentListCursor>, Failure> {
        let AgentListOutcome::Listed {
            revision,
            items,
            next_cursor,
        } = outcome
        else {
            return Err(failure("agent-page-invalid"));
        };
        if self.revision.is_some_and(|prior| prior != revision) {
            return Err(failure("agent-revision-changed"));
        }
        if items.len() > PAGE_SIZE
            || (items.is_empty() && next_cursor.is_some())
            || items.iter().any(|item| {
                !self.ids.insert(item.session_id.clone()) || item.lifecycle != AgentLifecycle::Live
            })
            || next_cursor
                .as_ref()
                .is_some_and(|cursor| !self.cursors.insert(cursor.as_str().to_owned()))
        {
            return Err(failure("agent-page-invalid"));
        }
        if self.items.len() + items.len() > MAXIMUM_SESSIONS || self.requests.len() >= MAXIMUM_PAGES
        {
            return Err(failure("agent-capacity"));
        }
        self.revision = Some(revision);
        self.items.extend(items);
        self.requests.push(request);
        Ok(next_cursor)
    }
}

fn list(
    client: &mut LocalClient,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Pages, Failure> {
    let mut pages = Pages::default();
    let mut cursor = None;
    for _ in 0..MAXIMUM_PAGES {
        signals.check_before(deadline)?;
        let (request, outcome) = query::read_agent_list(
            client,
            false,
            PAGE_SIZE as u64,
            cursor,
            deadline,
            signals.poll_fd(),
        )?;
        signals.check_before(deadline)?;
        cursor = pages.push(request, outcome)?;
        if cursor.is_none() {
            return Ok(pages);
        }
    }
    Err(failure("agent-capacity"))
}

fn membership(
    projection: &Projection,
    snapshot: &Snapshot,
) -> Result<BTreeMap<String, BTreeSet<String>>, Failure> {
    let (family, ids): (_, Vec<String>) = match projection {
        Projection::WorkspaceList(rows) => (
            "workspace",
            rows.iter().map(|row| row.workspace_id.clone()).collect(),
        ),
        Projection::WorkspaceGet(doc) => ("workspace", vec![doc.row.workspace_id.clone()]),
        Projection::TabList(rows) => ("tab", rows.iter().map(|row| row.tab_id.clone()).collect()),
        Projection::TabGet(doc) => ("tab", vec![doc.row.tab_id.clone()]),
        Projection::PanelList(rows) => (
            "panel",
            rows.iter().map(|row| row.panel_id.clone()).collect(),
        ),
        Projection::PanelGet(doc) => ("panel", vec![doc.row.panel_id.clone()]),
        _ => return Err(failure("agent-page-invalid")),
    };
    let mut rows: BTreeMap<_, BTreeSet<_>> =
        ids.into_iter().map(|id| (id, BTreeSet::new())).collect();
    let Projection::PanelList(panels) =
        results::project(&QueryCommand::PanelList { tab: None }, snapshot)?
    else {
        return Err(failure("agent-page-invalid"));
    };
    for panel in panels {
        if panel.primary_terminal_id.is_none() {
            continue;
        }
        let key = match family {
            "workspace" => panel.workspace_id.as_ref(),
            "tab" => Some(&panel.tab_id),
            _ => Some(&panel.panel_id),
        };
        if let Some(row) = key.and_then(|key| rows.get_mut(key)) {
            row.insert(panel.primary_presentation_id);
        }
    }
    Ok(rows)
}

fn matches(item: &AgentSessionSummary, presentations: &BTreeSet<String>) -> bool {
    item.lifecycle == AgentLifecycle::Live
        && item.presentation_attachments.iter().any(|attachment| {
            attachment.navigable && presentations.contains(attachment.presentation_id.as_str())
        })
}

pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    projection: &Projection,
    selected: Option<&ClientViewRecord>,
    current: Option<&super::current_presentation::Resolved>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Document, Failure> {
    let rows = if let Some(member) = current {
        BTreeMap::from([(
            member.panel_id.to_string(),
            member
                .binding
                .as_ref()
                .map(|_| member.presentation_id.to_string())
                .into_iter()
                .collect(),
        )])
    } else {
        membership(projection, snapshot)?
    };
    let presentations = rows.values().flatten().cloned().collect::<BTreeSet<_>>();
    let first = list(client, deadline, signals)?;
    let ids = first
        .items
        .iter()
        .filter(|item| matches(item, &presentations))
        .map(|item| item.session_id.clone())
        .collect::<Vec<_>>();
    let mut reads = Vec::new();
    let mut owners = BTreeMap::new();
    if let Some(selected) = selected {
        for chunk in ids.chunks(PAGE_SIZE) {
            let read = attention::read_many(
                client,
                snapshot,
                &selected.client_id,
                chunk,
                deadline,
                signals,
            )?;
            if !read.matches_view(&selected.view) {
                return Err(failure("client-changed"));
            }
            for id in chunk {
                owners.insert(id.clone(), reads.len());
            }
            reads.push(read);
        }
    }
    let (mut pages, requests) = if reads.is_empty() {
        (first, Vec::new())
    } else {
        let later = list(client, deadline, signals)?;
        if later.revision < first.revision {
            return Err(failure("agent-page-invalid"));
        }
        (later, first.requests)
    };
    let revision = pages
        .revision
        .ok_or_else(|| failure("agent-page-invalid"))?;
    let mut requests = requests;
    requests.append(&mut pages.requests);
    let items = pages
        .items
        .into_iter()
        .filter(|item| matches(item, &presentations))
        .collect::<Vec<_>>();
    let mut joined = BTreeMap::new();
    if selected.is_some() {
        for (index, read) in reads.iter().enumerate() {
            // A Session appearing after the first list is missing from that checkpoint;
            // the first authenticated batch supplies an explicit missing projection.
            let group = items
                .iter()
                .filter(|item| owners.get(&item.session_id).copied().unwrap_or(0) == index)
                .collect::<Vec<_>>();
            if group.is_empty() {
                continue;
            }
            let views =
                read.corroborate_summaries(client, snapshot, revision, &group, deadline, signals)?;
            for (item, view) in group.into_iter().zip(views) {
                joined.insert(item.session_id.clone(), view);
            }
        }
    }
    signals.check_before(deadline)?;
    Ok(Document {
        revision,
        requests,
        rows,
        sessions: items
            .into_iter()
            .map(|summary| {
                let client = joined.remove(&summary.session_id);
                Session { summary, client }
            })
            .collect(),
    })
}

fn status(session: &Session) -> Option<&'static str> {
    if session.summary.observation.freshness != AgentObservationFreshness::Fresh {
        return Some("unknown");
    }
    if let Some(client) = &session.client {
        return match client.status {
            Some("stale" | "disconnected" | "ended") => Some("unknown"),
            other => other,
        };
    }
    match session.summary.status {
        AgentSemanticStatus::NeedsAttention { .. } => Some("blocked"),
        AgentSemanticStatus::Working { .. } => Some("working"),
        AgentSemanticStatus::Unknown { .. } | AgentSemanticStatus::Ended { .. } => Some("unknown"),
        _ => None,
    }
}

fn aggregate<'a>(statuses: impl Iterator<Item = Option<&'a str>>) -> Option<&'static str> {
    let mut highest = 0;
    let mut unresolved = false;
    for status in statuses {
        highest = highest.max(match status {
            Some("blocked") => 4,
            Some("done") => 3,
            Some("working") => 2,
            Some("idle") => 1,
            None => {
                unresolved = true;
                0
            }
            _ => 0,
        });
    }
    match highest {
        4 => Some("blocked"),
        3 => Some("done"),
        _ if unresolved => None,
        2 => Some("working"),
        1 => Some("idle"),
        _ => Some("unknown"),
    }
}

impl Document {
    fn row_sessions(&self, id: &str) -> Vec<&Session> {
        self.rows.get(id).map_or_else(Vec::new, |presentations| {
            self.sessions
                .iter()
                .filter(|session| matches(&session.summary, presentations))
                .collect()
        })
    }
    pub(super) fn enrich(&self, result: &mut Value) {
        for (key, id) in [
            ("workspaces", "workspaceId"),
            ("tabs", "tabId"),
            ("panels", "panelId"),
            ("workspace", "workspaceId"),
            ("tab", "tabId"),
            ("panel", "panelId"),
        ] {
            let Some(value) = result.get_mut(key) else {
                continue;
            };
            let enrich = |row: &mut Value| {
                let sessions = self.row_sessions(row[id].as_str().unwrap_or_default());
                row["agentStatus"] =
                    json!(aggregate(sessions.iter().map(|session| status(session))));
                row["agentSessionIds"] = json!(
                    sessions
                        .iter()
                        .map(|session| &session.summary.session_id)
                        .collect::<Vec<_>>()
                );
            };
            if let Some(rows) = value.as_array_mut() {
                rows.iter_mut().for_each(enrich);
            } else {
                enrich(value);
            }
        }
        result["agents"] = json!({"revision":self.revision,"requestIds":self.requests,"sessions":self.sessions.iter().map(|session| {
            let mut row = json!({"session":session.summary});
            if let Some(client) = &session.client { row["client"] = client.document(); }
            row
        }).collect::<Vec<_>>()});
    }
    pub(super) fn render(&self, out: &mut Bounded, locale: Locale) -> Result<(), TooLarge> {
        out.write(format_args!("{}\n", locale.cli_workbench_agents_heading()))?;
        for id in self.rows.keys() {
            let sessions = self.row_sessions(id);
            let state = aggregate(sessions.iter().map(|session| status(session)));
            out.write(format_args!(
                "{}  {}  {}\n",
                id,
                locale.cli_workbench_agent_status(state),
                sessions.len()
            ))?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests;
