//! Public names commands project the Server's correlated outcomes. The CLI
//! owns no alias cache and never retries a mutation after transport ambiguity.

use orchbee_agent_observation_protocol::{
    AgentNameAssignOutcome, AgentNameReleaseOutcome, AgentNamesListOutcome, AgentNamesResult,
    AgentResolveOutcome,
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

#[cfg(any(target_os = "linux", windows))]
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

fn render(result: &AgentNamesResult, format: OutputFormat, locale: Locale) -> Output {
    let source = json!({ "kind": "agent-names", "requestId": result.request_id() });
    match project(result, locale) {
        Err(failure) => failure_output(
            failure.source(source).failure_schema(FAILURE_SCHEMA),
            Some(format),
            locale,
        ),
        Ok((schema, outcome, text)) => {
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

fn rejection(
    reason: Value,
    conflict: Option<&orchbee_agent_observation_protocol::AgentSessionId>,
) -> Failure {
    let kind = match reason.as_str() {
        Some("read-denied" | "manage-denied") => FailureKind::Denied,
        Some("not-found" | "name-not-found" | "session-not-found") => FailureKind::NotFound,
        _ => FailureKind::Rejected,
    };
    let mut details = serde_json::Map::from_iter([
        ("stage".to_owned(), json!("agent-names")),
        ("reason".to_owned(), reason),
    ]);
    if let Some(id) = conflict {
        details.insert("conflictSessionId".to_owned(), json!(id));
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
            AgentNamesListOutcome::Listed { bindings } => {
                let mut text = match locale {
                    Locale::EnUs => "NAME\tSESSION\n",
                    Locale::ZhCn => "名称\t会话\n",
                }
                .to_owned();
                for binding in bindings {
                    text.push_str(&format!("{}\t{}\n", binding.name, binding.session_id));
                }
                Ok(("orchbee.agent.names", json!(result.outcome), text))
            }
            AgentNamesListOutcome::Rejected { reason } => Err(rejection(json!(reason), None)),
            AgentNamesListOutcome::Unavailable { .. } => Err(unavailable()),
        },
        AgentNamesResult::NameAssign { result, .. } => match &result.outcome {
            AgentNameAssignOutcome::Assigned {
                session_id, name, ..
            }
            | AgentNameAssignOutcome::Unchanged { session_id, name } => {
                let text = match locale {
                    Locale::EnUs => format!("{name} -> {session_id}\n"),
                    Locale::ZhCn => format!("{name} → {session_id}\n"),
                };
                Ok(("orchbee.agent.rename", json!(result.outcome), text))
            }
            AgentNameAssignOutcome::Rejected {
                reason,
                conflict_session_id,
            } => Err(rejection(json!(reason), conflict_session_id.as_ref())),
            AgentNameAssignOutcome::Unavailable { .. } => Err(unavailable()),
        },
        AgentNamesResult::NameRelease { result, .. } => match &result.outcome {
            AgentNameReleaseOutcome::Released { session_id, .. }
            | AgentNameReleaseOutcome::Unchanged { session_id } => {
                let text = match locale {
                    Locale::EnUs => format!("{session_id}: name cleared\n"),
                    Locale::ZhCn => format!("{session_id}：名称已清除\n"),
                };
                Ok(("orchbee.agent.rename", json!(result.outcome), text))
            }
            AgentNameReleaseOutcome::Rejected { reason } => Err(rejection(json!(reason), None)),
            AgentNameReleaseOutcome::Unavailable { .. } => Err(unavailable()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_refusals_preserve_the_correlated_owner_and_validate_the_failure_schema() {
        let validator = jsonschema::validator_for(
            &serde_json::from_str(include_str!("schemas/orchbee.cli.failure.agent-names.json"))
                .unwrap(),
        )
        .unwrap();
        for (operation, outcome, kind) in [
            (
                "name.assign",
                json!({"kind":"rejected","reason":"name-in-use","conflictSessionId":"incumbent"}),
                "rejected",
            ),
            (
                "name.assign",
                json!({"kind":"rejected","reason":"manage-denied","conflictSessionId":null}),
                "denied",
            ),
            (
                "name.release",
                json!({"kind":"rejected","reason":"name-not-found"}),
                "not-found",
            ),
            (
                "resolve",
                json!({"kind":"unavailable","reason":"runtime-unavailable"}),
                "service-unavailable",
            ),
        ] {
            let result: AgentNamesResult = serde_json::from_value(json!({"kind":format!("agent.session.{operation}.result"),"requestId":"11111111111111111111111111111111","result":{"outcome":outcome}})).unwrap();
            for locale in [Locale::EnUs, Locale::ZhCn] {
                let output = render(&result, OutputFormat::Json, locale);
                assert_ne!(output.status, 0);
                let document: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert!(validator.is_valid(&document), "{document}");
                assert_eq!(document["failure"]["kind"], kind);
                assert_eq!(
                    document["source"]["requestId"],
                    "11111111111111111111111111111111"
                );
                if operation == "name.assign" && kind == "rejected" {
                    assert_eq!(
                        document["failure"]["details"]["conflictSessionId"],
                        "incumbent"
                    );
                }
                let text = render(&result, OutputFormat::Text, locale);
                assert!(text.stdout.is_empty());
                assert!(!text.stderr.is_empty());
            }
        }
    }
}
