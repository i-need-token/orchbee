//! Text records for `events subscribe`: one line per stream record with the
//! UTC receipt time, the record or event name, its subject and a short
//! detail. The machine envelope stays the JSON format's responsibility.
use crate::{cli::escape::quoted, localization::Locale};
use orchbee_workbench_protocol::{
    SemanticEvent, SemanticEventData as Data, SemanticEventsOutcome as Outcome,
};

/// `HH:MM:SS` UTC for a receipt instant; the events carry no time of their own.
pub(super) fn clock(now: std::time::SystemTime) -> String {
    let seconds = now
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
        % 86_400;
    format!(
        "{:02}:{:02}:{:02}Z",
        seconds / 3_600,
        seconds % 3_600 / 60,
        seconds % 60
    )
}

/// One stream record as a single text line.
pub(super) fn record(kind: &str, outcome: &Outcome, clock: &str, locale: Locale) -> String {
    let body = match outcome {
        Outcome::Matched { event, .. } => event_line(event),
        Outcome::Gap { gap, .. } => locale.events_gap(gap.after.value(), gap.through.value()),
        _ if kind == "opened" => locale.events_listening().to_owned(),
        _ => kind.to_owned(),
    };
    format!("{clock}  {body}\n")
}

/// The closing record: why the stream ended and how the subscription was released.
pub(super) fn closed(reason: &str, cleanup: &str, clock: &str) -> String {
    format!("{clock}  closed  {reason} ({cleanup})\n")
}

fn event_line(event: &SemanticEvent) -> String {
    let (subject, detail) = match &event.data {
        Data::Workspace { workspace } => (
            workspace.workspace_id.as_str().to_owned(),
            quoted(&workspace.label),
        ),
        Data::WorkspaceOrder { workspace_ids } => {
            (String::new(), format!("{} workspaces", workspace_ids.len()))
        }
        Data::WorkspaceMetadata { workspace_id, .. } => {
            (workspace_id.as_str().to_owned(), String::new())
        }
        Data::Tab { tab_id, label, .. } => (tab_id.as_str().to_owned(), quoted(label)),
        Data::Pane { pane } => (pane.panel_id.as_str().to_owned(), quoted(&pane.label)),
        Data::Layout { tab_id, .. } => (tab_id.as_str().to_owned(), String::new()),
        Data::Worktree { .. } => (String::new(), String::new()),
        Data::Client { client_id, .. } => (client_id.as_str().to_owned(), String::new()),
        Data::Agent {
            pane,
            report,
            exited,
        } => {
            let state = serde_json::to_value(report.state)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default();
            let agent = report.agent.as_deref().unwrap_or("-");
            let exited = if *exited { " exited" } else { "" };
            (
                pane.panel_id.as_str().to_owned(),
                format!("{agent} {state}{exited}"),
            )
        }
        Data::Output {
            pane, matched_line, ..
        } => (
            pane.panel_id.as_str().to_owned(),
            matched_line.as_deref().map(quoted).unwrap_or_default(),
        ),
        Data::Exited { pane, .. } => (pane.panel_id.as_str().to_owned(), String::new()),
        Data::Scroll {
            client_id, pane, ..
        } => (
            pane.panel_id.as_str().to_owned(),
            format!("client {}", client_id.as_str()),
        ),
    };
    let subject = if subject.is_empty() {
        event
            .workspace_id
            .as_ref()
            .map_or_else(|| "-".to_owned(), |id| id.as_str().to_owned())
    } else {
        subject
    };
    let mut line = format!("{}  {subject}", event.event.as_str());
    if !detail.is_empty() {
        line.push_str("  ");
        line.push_str(&detail);
    }
    line
}
