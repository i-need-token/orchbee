//! CLI adapters over one complete Agent projection: reads, selector resolution, and the list and
//! inspection documents. Command workflows compose these adapters; the adapters never call back.
use super::{
    Failure, FailureKind,
    results::{self, Projection},
};
use crate::{
    agent_projection::{
        self as model,
        cursor::Cursor,
        read::{Checkpoint, Error},
    },
    invocation::{AgentSelector, QueryCommand, Target},
    local_client::{Cancellation, LocalClient},
    localization::Locale,
};
use orchbee_agent_observation_protocol::AgentNameTarget;
use orchbee_workbench_protocol::{Snapshot, WorkbenchClientId};
use serde_json::{Value, json};
use std::time::Instant;
mod present;
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
                row["client"] = i.client.clone().unwrap_or(Value::Null);
                row
            })
            .collect::<Vec<_>>();
        let mut result = json!({"kind":"listed","revision":self.revision,"nextCursor":self.next.as_ref().map(Cursor::as_str),"items":items});
        result["clientId"] = json!(self.client);
        result
    }
}
#[derive(Debug)]
pub(crate) struct Inspection {
    pub(super) row: model::Row,
    pub(super) detail: Box<Projection>,
}
impl results::OwnedDocument for List {
    fn result(&self) -> Value {
        self.document()
    }

    fn render(
        &self,
        out: &mut super::output::Bounded,
        _command: &QueryCommand,
        locale: Locale,
    ) -> Result<(), super::output::TooLarge> {
        present::render_list(out, self, locale)
    }
}
impl results::OwnedDocument for Inspection {
    fn result(&self) -> Value {
        self.document()
    }

    fn render(
        &self,
        out: &mut super::output::Bounded,
        command: &QueryCommand,
        locale: Locale,
    ) -> Result<(), super::output::TooLarge> {
        present::render_inspection(out, self, command, locale)
    }
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
pub(super) fn classify(error: Error) -> Failure {
    let (kind, reason) = match error {
        Error::Unavailable => (FailureKind::ServiceUnavailable, "unavailable"),
        Error::Denied => (FailureKind::Denied, "denied"),
        Error::Changed => (FailureKind::Rejected, "changed"),
        Error::Invalid => (FailureKind::ProtocolInvalid, "invalid"),
        Error::TooLarge => (FailureKind::OutputTooLarge, "too-large"),
        Error::Timeout => (FailureKind::Timeout, "timeout"),
        Error::Cancelled => (FailureKind::Cancelled, "cancelled"),
    };
    let failure = failure(kind, reason);
    // A read outrun by its sources changed nothing; a fresh snapshot may read it again.
    if error == Error::Changed {
        failure.mark_stale_view()
    } else {
        failure
    }
}
pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    ended: bool,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<Checkpoint, Failure> {
    model::read::read(client, snapshot, ended, deadline, Cancellation::new(wake)).map_err(classify)
}
pub(super) fn confirm(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    read: &Checkpoint,
    ended: bool,
    deadline: Instant,
    wake: Option<orchbee_local_ipc::LocalIo<'_>>,
) -> Result<(), Failure> {
    model::read::confirm(
        client,
        snapshot,
        read,
        ended,
        deadline,
        Cancellation::new(wake),
    )
    .map_err(classify)
}
pub(super) fn source(snapshot: &Snapshot, read: &Checkpoint) -> Value {
    json!({"kind":"agent-projection","sessionId":snapshot.session_id,"workbenchRevision":snapshot.revision,
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
                binding: orchbee_workbench_client::test_support::machine_agents::physical(22)
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
        let mut binding = orchbee_workbench_client::test_support::machine_agents::physical(12);
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
