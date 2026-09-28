//! Public names commands project the Server's correlated outcomes. The CLI
//! owns no alias cache and never retries a mutation after transport ambiguity.

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
            let version = match result {
                AgentNamesResult::Resolve { result, .. }
                    if matches!(result.outcome, AgentResolveOutcome::ResolvedTarget { .. }) =>
                {
                    3
                }
                AgentNamesResult::NamesList { result, .. }
                    if matches!(result.outcome, AgentNamesListOutcome::ListedTargets { .. }) =>
                {
                    3
                }
                AgentNamesResult::NameAssign { result, .. }
                    if matches!(
                        result.outcome,
                        AgentNameAssignOutcome::AssignedTarget { .. }
                            | AgentNameAssignOutcome::UnchangedTarget { .. }
                    ) =>
                {
                    2
                }
                AgentNamesResult::NameRelease { result, .. }
                    if matches!(
                        result.outcome,
                        AgentNameReleaseOutcome::ReleasedTarget { .. }
                            | AgentNameReleaseOutcome::UnchangedTarget { .. }
                    ) =>
                {
                    2
                }
                _ => {
                    if matches!(result, AgentNamesResult::NamesList { .. })
                        || matches!(result, AgentNamesResult::Resolve { result, .. }
                        if matches!(result.outcome, AgentResolveOutcome::Resolved {
                            resolved_by: orchbee_agent_observation_protocol::AgentResolvedBy::Presentation, ..
                        }))
                    {
                        2
                    } else {
                        1
                    }
                }
            };
            let bytes = match format {
                OutputFormat::Json => output::encode_document(
                    &json!({ "schema": schema, "version": version, "source": source, "result": outcome }),
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
    let version = match reason.as_str() {
        Some("launch-pending") => 3,
        Some("ambiguous-target") => 2,
        _ => 1,
    };
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
        details.insert("conflictSessionId".to_owned(), json!(id));
    }
    Failure::new(kind).details(details).version(version)
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
                let mut text = locale.cli_agent_names_header(true).to_owned();
                for entry in bindings {
                    text.push_str(&format!(
                        "{}\t{}\n",
                        entry.name,
                        target_text(&entry.target, locale)
                    ));
                }
                if !pending.is_empty() {
                    text.push_str(locale.cli_agent_pending_names_header());
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
                let mut text = locale.cli_agent_names_header(false).to_owned();
                for binding in bindings {
                    text.push_str(&format!("{}\t{}\n", binding.name, binding.session_id));
                }
                if !pending.is_empty() {
                    text.push_str(locale.cli_agent_pending_names_header());
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
                locale.cli_agent_name_assigned(name.as_str(), &target_text(target, locale)),
            )),
            AgentNameAssignOutcome::RejectedTarget {
                reason,
                conflict_target,
            } => Err(Failure::new(FailureKind::Rejected).version(4).details(
                serde_json::Map::from_iter([
                    ("stage".into(), json!("agent-names")),
                    ("reason".into(), json!(reason)),
                    ("conflictTarget".into(), json!(conflict_target)),
                ]),
            )),
            AgentNameAssignOutcome::Assigned {
                session_id, name, ..
            }
            | AgentNameAssignOutcome::Unchanged { session_id, name } => {
                let text = locale.cli_agent_name_assigned(name.as_str(), session_id.as_str());
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
                let text = locale.cli_agent_name_cleared(&target);
                Ok(("orchbee.agent.rename", json!(result.outcome), text))
            }
            AgentNameReleaseOutcome::Released { session_id, .. }
            | AgentNameReleaseOutcome::Unchanged { session_id } => {
                let text = locale.cli_agent_name_cleared(session_id.as_str());
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
            locale.cli_agent_terminal_name_target(binding.presentation_id.as_str())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_protocol_and_pending_names_match_published_schemas() {
        use orchbee_agent_observation_protocol::{parse_agent_request, parse_agent_result};
        let wire = jsonschema::validator_for(
            &serde_json::from_str(include_str!(
                "../../../../schemas/agent-observation/v1/agent-observation.schema.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let id = "11111111111111111111111111111111";
        let reservation = json!({"reservationId":id,"name":"alpha","providerId":"codex","binding":{"presentationId":id,"generation":id,"hostEpoch":id,"hostTerminalId":id},"remainingMs":10000,"settleRemainingMs":3000});
        for (operation, fields, outcome) in [
            (
                "reserve",
                json!({"name":"alpha","providerId":"codex","presentationId":id,"hostEpoch":id,"hostTerminalId":id,"ttlMs":10000}),
                json!({"kind":"reserved","reservation":reservation}),
            ),
            (
                "inspect",
                json!({"reservationId":id}),
                json!({"kind":"pending","reservation":reservation,"candidateSessionId":null}),
            ),
            (
                "complete",
                json!({"reservationId":id,"sessionId":"s1"}),
                json!({"kind":"completed","reservationId":id,"name":"alpha","sessionId":"s1"}),
            ),
            (
                "cancel",
                json!({"reservationId":id}),
                json!({"kind":"released","reservationId":id}),
            ),
        ] {
            let mut request = fields;
            request["kind"] = json!(format!("agent.launch.{operation}.request"));
            request["requestId"] = json!(id);
            assert!(wire.is_valid(&request), "{request}");
            assert!(parse_agent_request(&serde_json::to_vec(&request).unwrap()).is_ok());
            let result = json!({"kind":format!("agent.launch.{operation}.result"),"requestId":id,"result":{"outcome":outcome}});
            assert!(wire.is_valid(&result), "{result}");
            assert!(parse_agent_result(&serde_json::to_vec(&result).unwrap()).is_ok());
            let mut invalid = request;
            invalid["surprise"] = json!(true);
            assert!(!wire.is_valid(&invalid));
            assert!(parse_agent_request(&serde_json::to_vec(&invalid).unwrap()).is_err());
        }
        let listed = json!({"kind":"agent.session.names.list.result","requestId":id,"result":{"outcome":{"kind":"listed","bindings":[],"pending":[reservation]}}});
        assert!(wire.is_valid(&listed));
        assert!(parse_agent_result(&serde_json::to_vec(&listed).unwrap()).is_ok());
        let result = serde_json::from_value(listed).unwrap();
        let schema = jsonschema::validator_for(
            &serde_json::from_str(include_str!("schemas/orchbee.agent.names.json")).unwrap(),
        )
        .unwrap();
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let answer = render(&result, OutputFormat::Json, locale);
            let document: Value = serde_json::from_slice(&answer.stdout).unwrap();
            assert!(schema.is_valid(&document), "{document}");
            assert_eq!(document["version"], 2);
            let answer = render(&result, OutputFormat::Text, locale);
            assert!(
                String::from_utf8(answer.stdout)
                    .unwrap()
                    .contains("alpha\tcodex\t10000")
            );
        }
    }

    #[test]
    fn semantic_refusals_preserve_the_correlated_owner_and_validate_the_failure_schema() {
        let validator = jsonschema::validator_for(
            &serde_json::from_str(include_str!("schemas/orchbee.cli.failure.agent-names.json"))
                .unwrap(),
        )
        .unwrap();
        for (operation, outcome, kind) in [
            (
                "resolve",
                json!({"kind":"rejected","reason":"launch-pending"}),
                "rejected",
            ),
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
