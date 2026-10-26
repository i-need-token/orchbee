//! Complete machine query members independently of platform capabilities and optional selectors.
//! Existing observations win; unavailable capabilities and unselected observations stay explicit.
use crate::invocation::QueryCommand;
use serde_json::{Value, json};

fn unsupported() -> Value {
    json!({"kind":"unsupported"})
}

fn missing(value: &mut Value, keys: &[&str], fallback: &Value) {
    if let Some(object) = value.as_object_mut() {
        for key in keys {
            object
                .entry((*key).to_owned())
                .or_insert_with(|| fallback.clone());
        }
    }
}

#[cfg(not(windows))]
pub(super) fn canonical_list(
    outcome: &orchbee_agent_observation_protocol::AgentListOutcome,
) -> Value {
    use orchbee_agent_observation_protocol::{
        AgentListOutcome, AgentReportProvider, AgentSemanticStatus,
    };
    let AgentListOutcome::Listed {
        items,
        next_cursor,
        revision,
    } = outcome
    else {
        return json!(outcome);
    };
    let rows = items.iter().enumerate().map(|(index, item)| {
        let status = match item.status {
            AgentSemanticStatus::NeedsAttention { .. } => "blocked",
            AgentSemanticStatus::Failed {} | AgentSemanticStatus::Canceled {} | AgentSemanticStatus::Interrupted {} => "error",
            AgentSemanticStatus::Succeeded {} => "done",
            AgentSemanticStatus::Working { .. } => "working",
            AgentSemanticStatus::Idle {} => "idle",
            AgentSemanticStatus::Unknown { .. } => "unknown",
            AgentSemanticStatus::Ended {} => "ended",
        };
        let provider = AgentReportProvider::lookup(item.provider_id.as_str())
            .map_or(item.provider_id.as_str(), AgentReportProvider::native_provider_id);
        json!({"ordinal":index+1,"provider":provider,"status":status,"source":"canonical",
            "canonical":item,"terminal":unsupported(),"placement":unsupported(),"cwd":unsupported(),"client":unsupported()})
    }).collect::<Vec<_>>();
    json!({"kind":"listed","revision":revision,"nextCursor":next_cursor,"items":rows,"clientId":unsupported()})
}

/// Keep optional query members present without changing any observed value.
pub(in crate::cli) fn complete(command: &QueryCommand, mut result: Value) -> Value {
    let capability = if cfg!(windows) {
        json!("unknown")
    } else {
        unsupported()
    };
    let selection = if cfg!(windows) {
        Value::Null
    } else {
        unsupported()
    };
    let family = match command {
        QueryCommand::WorkspaceList { .. } | QueryCommand::WorkspaceGet(_) => {
            Some(("workspace", "workspaces"))
        }
        QueryCommand::TabList { .. } | QueryCommand::TabGet(_) => Some(("tab", "tabs")),
        QueryCommand::PanelList { .. }
        | QueryCommand::PanelGet(_)
        | QueryCommand::PanelCurrent(_) => Some(("panel", "panels")),
        _ => None,
    };
    if let Some((single, many)) = family {
        missing(&mut result, &["client"], &selection);
        missing(&mut result, &["agents"], &capability);
        if let Some(sessions) = result["agents"]
            .get_mut("sessions")
            .and_then(Value::as_array_mut)
        {
            for session in sessions {
                missing(session, &["client"], &selection);
            }
        }
        if single == "workspace" {
            missing(&mut result, &["metadataSource"], &capability);
        }
        let row = |value: &mut Value| {
            missing(value, &["focused"], &selection);
            missing(value, &["agentSessionIds", "agentStatus"], &capability);
            match single {
                "workspace" => {
                    missing(value, &["activeTabId"], &selection);
                    missing(
                        value,
                        &["worktree", "worktreeObservation", "tokens"],
                        &capability,
                    );
                }
                "panel" => missing(
                    value,
                    &[
                        "cwd",
                        "cwdSource",
                        "foregroundCwd",
                        "terminalTitle",
                        "terminalTitleStripped",
                        "terminalObservation",
                        "scroll",
                        "scrollObservation",
                        "metadata",
                        "reportedAgent",
                    ],
                    &capability,
                ),
                _ => {}
            }
        };
        if let Some(value) = result.get_mut(single) {
            row(value);
        }
        if let Some(values) = result.get_mut(many).and_then(Value::as_array_mut) {
            for value in values {
                row(value);
            }
        }
        if matches!(command, QueryCommand::PanelCurrent(_)) {
            missing(&mut result["panel"], &["currentPresentation"], &capability);
        }
    }
    match command {
        QueryCommand::AgentList { .. } => missing(&mut result, &["clientId"], &selection),
        QueryCommand::AgentGet(_) | QueryCommand::AgentClientGet { .. } => {
            if result["kind"] != "projected" {
                result = json!({"kind":"projected","agent":unsupported(),"inspection":result});
            }
            let inspection = &mut result["inspection"];
            if inspection["kind"] == "found" {
                missing(inspection, &["client"], &selection);
                missing(inspection, &["terminal"], &capability);
                missing(inspection, &["runtime", "management"], &capability);
            } else if inspection["kind"] == "terminal-found" {
                missing(inspection, &["clientView"], &selection);
            }
        }
        QueryCommand::AgentWait { .. } => {
            missing(&mut result["wait"], &["afterRevision"], &Value::Null);
            if result["kind"] == "terminal-matched" {
                missing(&mut result, &["clientView"], &selection);
            } else {
                missing(&mut result, &["client"], &selection);
                missing(&mut result, &["matchedChange"], &capability);
            }
        }
        QueryCommand::TerminalSourceWait { .. } => {
            missing(&mut result, &["after", "selection"], &Value::Null);
        }
        _ => {}
    }
    result
}
