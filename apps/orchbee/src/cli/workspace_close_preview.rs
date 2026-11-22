//! `workspace close --dry-run`: the terminals a close would end, as `terminal list` lists them, and
//! whether each runs a program, by the Server's read-only idle check, the evidence a conditional
//! Workspace Close reads. A Server without the check, or one that does not answer in time, leaves
//! every terminal unknown, never idle. Nothing is submitted.
use super::{
    Failure, FailureKind, Output, failure_output, handles, output,
    results::{self, Projection, TerminalListDetails, TerminalListDocument, TerminalRow},
    success_output, terminal_list,
};
use crate::{
    cli_text::{CliText, Stops},
    invocation::{OutputFormat, QueryCommand, Target, TerminalListFilter},
    local_client::LocalClient,
    localization::Locale,
    terminal_title::{display_name, program_name},
};
use orchbee_connection_handshake::WORKSPACE_IDLE;
use orchbee_workbench_protocol::{
    ProtocolVersion, Snapshot, WorkbenchWorkspaceId, WorkspaceIdleMember, WorkspaceIdleOutcome,
    WorkspaceIdleReason, WorkspaceIdleRequest,
};
use serde_json::{Map, Value, json};
use std::time::{Duration, Instant};

const RESULT_ID: &str = "orchbee.workspace.close-preview";

/// The Server reads for up to four seconds; the rest carries the answer.
const IDLE_READ: Duration = Duration::from_secs(6);

/// What one terminal of the Workspace is doing, as the idle check read it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Activity {
    Idle,
    Running,
    Unknown,
}

impl Activity {
    const fn token(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Unknown => "unknown",
        }
    }
}

pub(crate) fn run(target: &Target, format: OutputFormat, locale: Locale) -> Output {
    let command = QueryCommand::TerminalList {
        filter: Some(TerminalListFilter::Workspace(target.clone())),
    };
    terminal_list::inspect(format, locale, |signals| {
        let (mut client, snapshot, rows, details) =
            terminal_list::observed(&command, terminal_list::LISTING, signals)?;
        let workspace = results::resolve_workspace(&snapshot, target)
            .map_err(|failure| failure.source(results::snapshot_source(&snapshot)))?;
        let (workspace_id, label) = (workspace.workspace_id.clone(), workspace.label.clone());
        let busy = read(&mut client, &snapshot, &workspace_id);
        drop(client);
        // Every busy member must be a listed terminal; one the listing missed leaves all unknown.
        let busy = busy.filter(|members| {
            members.iter().all(|member| {
                rows.iter()
                    .any(|row| row.presentation_id == member.presentation_id.as_str())
            })
        });
        let activities: Vec<Activity> = rows
            .iter()
            .map(|row| activity(busy.as_deref(), row))
            .collect();
        let stops = names(&rows, &details, &activities, locale);
        let idle = busy.as_ref().is_some_and(Vec::is_empty);
        let source = results::snapshot_source(&snapshot);
        let document = TerminalListDocument { rows, details };
        Ok(match format {
            OutputFormat::Json => {
                let mut terminals = Projection::TerminalList(document).result()["terminals"].take();
                if let Some(items) = terminals.as_array_mut() {
                    for (item, activity) in items.iter_mut().zip(&activities) {
                        if let Some(item) = item.as_object_mut() {
                            item.insert("activity".to_owned(), json!(activity.token()));
                        }
                    }
                }
                let document = json!({
                    "schema": RESULT_ID,
                    "version": 1,
                    "source": source,
                    "result": {
                        "workspace": {"workspaceId": workspace_id.as_str(), "label": label},
                        "evidence": if busy.is_some() { "read" } else { "unavailable" },
                        "idle": idle,
                        "terminals": terminals,
                    },
                });
                match output::encode_document(&document) {
                    Ok(stdout) => Output {
                        status: 0,
                        stdout,
                        stderr: String::new(),
                        finalization: None,
                    },
                    Err(_) => too_large(source, format, locale),
                }
            }
            OutputFormat::Text => {
                let words = CliText::new(locale);
                let facts = match (&busy, idle) {
                    (None, _) => Stops::Unread,
                    (Some(_), true) => Stops::Idle,
                    (Some(_), false) => Stops::Busy(&stops),
                };
                let mut output = success_output(
                    Projection::TerminalList(document),
                    source,
                    &command,
                    format,
                    locale,
                );
                if output.status == 0 {
                    let mut text = words.cli_close_preview("workspace");
                    text.push_str(&words.cli_close_facts(facts));
                    let mut text = text.into_bytes();
                    text.append(&mut output.stdout);
                    output.stdout = text;
                }
                output
            }
        })
    })
}

/// The members that keep the Workspace from closing unasked, empty when it is idle, or `None`
/// when the check could not be read.
fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    workspace_id: &WorkbenchWorkspaceId,
) -> Option<Vec<WorkspaceIdleMember>> {
    if !client.server_serves(WORKSPACE_IDLE) {
        return None;
    }
    let request = WorkspaceIdleRequest {
        protocol_version: ProtocolVersion,
        request_id: super::query::request_id().ok()?,
        session_id: snapshot.session_id.clone(),
        workspace_id: workspace_id.clone(),
    };
    let result = client
        .workspace_idle_before(request, Instant::now() + IDLE_READ)
        .ok()?;
    match result.outcome {
        WorkspaceIdleOutcome::Idle {} => Some(Vec::new()),
        WorkspaceIdleOutcome::Busy { members } => Some(members),
        WorkspaceIdleOutcome::Failed { .. } => None,
    }
}

fn activity(busy: Option<&[WorkspaceIdleMember]>, row: &TerminalRow) -> Activity {
    let Some(busy) = busy else {
        return Activity::Unknown;
    };
    match busy
        .iter()
        .find(|member| member.presentation_id.as_str() == row.presentation_id)
        .map(|member| member.reason)
    {
        None => Activity::Idle,
        Some(WorkspaceIdleReason::Running) => Activity::Running,
        Some(WorkspaceIdleReason::Unknown) => Activity::Unknown,
    }
}

/// The names of the terminals that are not idle, as the text table names them.
fn names(
    rows: &[TerminalRow],
    details: &[TerminalListDetails],
    activities: &[Activity],
    locale: Locale,
) -> Vec<String> {
    rows.iter()
        .zip(activities)
        .enumerate()
        .filter(|(_, (_, activity))| **activity != Activity::Idle)
        .map(|(index, (row, _))| {
            let details = details.get(index);
            details
                .and_then(|details| details.agent.clone())
                .or_else(|| {
                    details
                        .and_then(|details| details.title.as_deref())
                        .and_then(|title| display_name(title, locale))
                })
                .or_else(|| {
                    details
                        .and_then(|details| details.placement.as_ref())
                        .and_then(|placement| placement.program.as_deref())
                        .and_then(|program| program_name(program, locale))
                })
                .unwrap_or_else(|| {
                    details
                        .and_then(|details| details.placement.as_ref())
                        .map_or_else(
                            || handles::stable("terminal", &[row.terminal_id.as_str()]).remove(0),
                            |placement| placement.handle.clone(),
                        )
                })
        })
        .collect()
}

fn too_large(source: Value, format: OutputFormat, locale: Locale) -> Output {
    failure_output(
        Failure::new(FailureKind::OutputTooLarge)
            .details(Map::from_iter([(
                "limitBytes".to_owned(),
                json!(output::LIMIT),
            )]))
            .source(source),
        Some(format),
        locale,
    )
}
