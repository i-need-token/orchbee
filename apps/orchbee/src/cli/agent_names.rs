//! Public names commands project the Server's correlated outcomes. The CLI
//! owns no alias cache and never retries a mutation after transport ambiguity.

use crate::cli_text::CliText;
use orchbee_agent_observation_protocol::{
    AgentNameAssignOutcome, AgentNameReleaseOutcome, AgentNameTarget, AgentNamesListOutcome,
    AgentNamesResult, AgentResolveOutcome,
};
use serde_json::{Value, json};

use super::{Failure, FailureKind, Output, SourceFinalization, failure_output, output};
use crate::invocation::{AgentNamesCommand, OutputFormat};
use crate::localization::Locale;

pub(crate) const FAILURE_SCHEMA: &str = "orchbee.cli.failure.agent-names";

pub(super) struct Execution {
    pub(super) result: Result<AgentNamesResult, Failure>,
    pub(super) finalization: Option<SourceFinalization>,
}

impl Execution {
    pub(super) fn failed(failure: Failure) -> Self {
        Self {
            result: Err(failure),
            finalization: None,
        }
    }
}

impl AgentNamesCommand {
    pub(crate) const fn operation(&self) -> &'static str {
        match self {
            Self::List => "names",
            Self::Resolve(_) => "resolve",
            Self::Assign { .. } => "rename",
            Self::Release(_) => "clear-name",
        }
    }

    pub(crate) const fn is_mutation(&self) -> bool {
        matches!(self, Self::Assign { .. } | Self::Release(_))
    }
}

pub(crate) fn run(command: AgentNamesCommand, format: OutputFormat, locale: Locale) -> Output {
    let Execution {
        result,
        finalization,
    } = super::query::names_command(&command);
    let mut answer = match result {
        Err(failure) => failure_output(failure, Some(format), locale),
        Ok(result) => render(&result, format, locale),
    };
    answer.finalization = finalization;
    answer
}

/// Canonical and physical identities share the same public target vocabulary.
fn current_outcome(mut outcome: Value) -> Value {
    if let Some(rows) = outcome.get_mut("bindings").and_then(Value::as_array_mut) {
        for row in rows {
            if let Some(object) = row.as_object_mut()
                && let Some(session) = object.remove("sessionId")
            {
                object.remove("lifecycle");
                object.insert(
                    "target".into(),
                    json!({"kind":"session","sessionId":session}),
                );
            }
        }
    }
    if let Some(object) = outcome.as_object_mut()
        && let Some(session) = object.remove("sessionId")
    {
        object.insert(
            "target".into(),
            json!({"kind":"session","sessionId":session}),
        );
    }
    let kind = match outcome["kind"].as_str() {
        Some("resolved") => Some("resolved-target"),
        Some("listed") => Some("listed-targets"),
        Some("assigned") => Some("assigned-target"),
        Some("unchanged") => Some("unchanged-target"),
        Some("released") => Some("released-target"),
        _ => None,
    };
    if let Some(kind) = kind {
        outcome["kind"] = json!(kind);
    }
    outcome
}

fn render(result: &AgentNamesResult, format: OutputFormat, locale: Locale) -> Output {
    let source = json!({ "kind": "agent-names", "requestId": result.request_id() });
    match project(result, locale) {
        Err(failure) => failure_output(
            failure.source(source).failure_schema(FAILURE_SCHEMA),
            Some(format),
            locale,
        ),
        Ok((schema, outcome, text)) => {
            let outcome = current_outcome(outcome);
            let bytes = match format {
                OutputFormat::Json => output::encode_document(
                    &json!({ "schema": schema, "version": 1, "source": source, "result": outcome }),
                ),
                OutputFormat::Text => output::encode_text(&text),
            };
            match bytes {
                Ok(stdout) => Output {
                    status: 0,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(_) => failure_output(
                    Failure::new(FailureKind::OutputTooLarge).failure_schema(FAILURE_SCHEMA),
                    Some(format),
                    locale,
                ),
            }
        }
    }
}

pub(super) fn rejection(
    reason: Value,
    conflict: Option<&orchbee_agent_observation_protocol::AgentSessionId>,
) -> Failure {
    let kind = match reason.as_str() {
        Some("ambiguous-target") => FailureKind::Ambiguous,
        Some("read-denied" | "manage-denied") => FailureKind::Denied,
        Some("not-found" | "name-not-found" | "session-not-found") => FailureKind::NotFound,
        _ => FailureKind::Rejected,
    };
    let mut details = serde_json::Map::from_iter([
        ("stage".to_owned(), json!("agent-names")),
        ("reason".to_owned(), reason),
    ]);
    if let Some(id) = conflict {
        details.insert(
            "conflictTarget".to_owned(),
            json!({"kind":"session","sessionId":id}),
        );
    }
    Failure::new(kind).details(details)
}

fn project(
    result: &AgentNamesResult,
    locale: Locale,
) -> Result<(&'static str, Value, String), Failure> {
    let unavailable = || Failure::new(FailureKind::ServiceUnavailable);
    match result {
        AgentNamesResult::Resolve { result, .. } => match &result.outcome {
            AgentResolveOutcome::ResolvedTarget {
                target,
                resolved_by,
            } => Ok((
                "orchbee.agent.resolve",
                json!(result.outcome),
                format!(
                    "{}\t{}\n",
                    target_text(target, locale),
                    json!(resolved_by).as_str().unwrap_or_default()
                ),
            )),
            AgentResolveOutcome::Resolved {
                session_id,
                resolved_by,
            } => Ok((
                "orchbee.agent.resolve",
                json!(result.outcome),
                format!(
                    "{session_id}\t{}\n",
                    json!(resolved_by).as_str().unwrap_or_default()
                ),
            )),
            AgentResolveOutcome::Rejected { reason } => Err(rejection(json!(reason), None)),
            AgentResolveOutcome::Unavailable { .. } => Err(unavailable()),
        },
        AgentNamesResult::NamesList { result, .. } => match &result.outcome {
            AgentNamesListOutcome::ListedTargets { bindings, pending } => {
                let mut text = CliText::new(locale).cli_agent_names_header(true).to_owned();
                for entry in bindings {
                    text.push_str(&format!(
                        "{}\t{}\n",
                        entry.name,
                        target_text(&entry.target, locale)
                    ));
                }
                if !pending.is_empty() {
                    text.push_str(CliText::new(locale).cli_agent_pending_names_header());
                    for entry in pending {
                        text.push_str(&format!(
                            "{}\t{}\t{}\t{}\n",
                            entry.name,
                            entry.provider_id,
                            entry.remaining_ms.value(),
                            entry.reservation_id
                        ));
                    }
                }
                Ok(("orchbee.agent.names", json!(result.outcome), text))
            }
            AgentNamesListOutcome::Listed { bindings, pending } => {
                let mut text = CliText::new(locale)
                    .cli_agent_names_header(false)
                    .to_owned();
                for binding in bindings {
                    text.push_str(&format!("{}\t{}\n", binding.name, binding.session_id));
                }
                if !pending.is_empty() {
                    text.push_str(CliText::new(locale).cli_agent_pending_names_header());
                    for launch in pending {
                        text.push_str(&format!(
                            "{}\t{}\t{}\t{}\n",
                            launch.name,
                            launch.provider_id,
                            launch.remaining_ms.value(),
                            launch.reservation_id
                        ));
                    }
                }
                Ok(("orchbee.agent.names", json!(result.outcome), text))
            }
            AgentNamesListOutcome::Rejected { reason } => Err(rejection(json!(reason), None)),
            AgentNamesListOutcome::Unavailable { .. } => Err(unavailable()),
        },
        AgentNamesResult::NameAssign { result, .. } => match &result.outcome {
            AgentNameAssignOutcome::AssignedTarget { target, name, .. }
            | AgentNameAssignOutcome::UnchangedTarget { target, name } => Ok((
                "orchbee.agent.rename",
                json!(result.outcome),
                CliText::new(locale)
                    .cli_agent_name_assigned(name.as_str(), &target_text(target, locale)),
            )),
            AgentNameAssignOutcome::RejectedTarget {
                reason,
                conflict_target,
            } => Err(
                Failure::new(FailureKind::Rejected).details(serde_json::Map::from_iter([
                    ("stage".into(), json!("agent-names")),
                    ("reason".into(), json!(reason)),
                    ("conflictTarget".into(), json!(conflict_target)),
                ])),
            ),
            AgentNameAssignOutcome::Assigned {
                session_id, name, ..
            }
            | AgentNameAssignOutcome::Unchanged { session_id, name } => {
                let text = CliText::new(locale)
                    .cli_agent_name_assigned(name.as_str(), session_id.as_str());
                Ok(("orchbee.agent.rename", json!(result.outcome), text))
            }
            AgentNameAssignOutcome::Rejected {
                reason,
                conflict_session_id,
            } => Err(rejection(json!(reason), conflict_session_id.as_ref())),
            AgentNameAssignOutcome::Unavailable { .. } => Err(unavailable()),
        },
        AgentNamesResult::NameRelease { result, .. } => match &result.outcome {
            AgentNameReleaseOutcome::ReleasedTarget { target, .. }
            | AgentNameReleaseOutcome::UnchangedTarget { target } => {
                let target = target_text(target, locale);
                let text = CliText::new(locale).cli_agent_name_cleared(&target);
                Ok(("orchbee.agent.rename", json!(result.outcome), text))
            }
            AgentNameReleaseOutcome::Released { session_id, .. }
            | AgentNameReleaseOutcome::Unchanged { session_id } => {
                let text = CliText::new(locale).cli_agent_name_cleared(session_id.as_str());
                Ok(("orchbee.agent.rename", json!(result.outcome), text))
            }
            AgentNameReleaseOutcome::Rejected { reason } => Err(rejection(json!(reason), None)),
            AgentNameReleaseOutcome::Unavailable { .. } => Err(unavailable()),
        },
    }
}

fn target_text(target: &AgentNameTarget, locale: Locale) -> String {
    match target {
        AgentNameTarget::Session { session_id } => session_id.to_string(),
        AgentNameTarget::Terminal { binding } => {
            CliText::new(locale).cli_agent_terminal_name_target(binding.presentation_id.as_str())
        }
    }
}

#[cfg(test)]
mod tests;
