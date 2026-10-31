//! Retained status uses receipt identity; resume submits once with an explicit generation.
use super::command_interrupt::{ArmedSignals, Finish};
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::cli_text::CliText;
use crate::{
    invocation::{OutputFormat, operation::CloseFamily},
    localization::Locale,
};
use orchbee_workbench_protocol::{WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration};
use serde_json::{Value, json};

pub(crate) fn run_close(
    family: CloseFamily,
    operation_id: &WorkbenchRequestId,
    generation: Option<WorkbenchTabCloseAttemptGeneration>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let Err(failure) =
        super::execution::admit(super::execution::Capability::RetainedCloseOperation)
    {
        return failure_output(failure, Some(format), locale);
    }
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let answer = observe_close(family, operation_id, generation, &mut signals);
    let mut output = match answer {
        Ok((result, status)) => {
            let bytes = match format {
                OutputFormat::Json => output::encode_document(&json!({
                    "schema":"orchbee.operation.close", "version":1, "result":result
                })),
                OutputFormat::Text => {
                    output::encode_text(&CliText::new(locale).operation_result(&result))
                }
            };
            match bytes {
                Ok(stdout) => Output {
                    status,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(_) => failure_output(
                    Failure::new(FailureKind::OutputTooLarge),
                    Some(format),
                    locale,
                ),
            }
        }
        Err(failure) => failure_output(failure, Some(format), locale),
    };
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled { finalization, .. }) => {
            output.status = 130;
            output.finalization = Some(finalization);
        }
        Err(_) => {
            output.status = 1;
        }
    }
    output
}

fn observe_close(
    family: CloseFamily,
    operation_id: &WorkbenchRequestId,
    generation: Option<WorkbenchTabCloseAttemptGeneration>,
    signals: &mut ArmedSignals,
) -> Result<(Value, u8), Failure> {
    use crate::local_client::{
        WorkbenchTabCloseResumeSubmission as Tab,
        WorkbenchWorkspaceCloseResumeSubmission as Workspace,
    };
    use orchbee_workbench_protocol::{
        CloseOperationOutcome as O, CloseOperationRead, CloseOperationTarget as T, ProtocolVersion,
        WorkbenchTabCloseResume, WorkbenchWorkspaceCloseResume,
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let (mut client, deadline, snapshot) = super::query::snapshot_session_until(deadline)?;
    let read = CloseOperationRead {
        protocol_version: ProtocolVersion,
        request_id: super::query::request_id()?,
        session_id: snapshot.session_id.clone(),
        operation_id: operation_id.clone(),
    };
    let reply = client.close_operation(read, deadline, None).map_err(|e| {
        Failure::new(FailureKind::ServiceUnavailable).details(super::stage_reason_details(
            "operation-status",
            e.reason.as_str(),
        ))
    })?;
    let mut result =
        json!({"requestId":operation_id,"family":family.token(),"observation":reply,"resume":null});
    let O::Available { target, .. } = &reply.outcome else {
        let status = match reply.outcome {
            O::Unavailable {} => 4,
            O::NotFound { .. } | O::Denied { .. } | O::Available { .. } => 3,
        };
        return Ok((result, status));
    };
    let matches = matches!(
        (family, target),
        (CloseFamily::Workspace, T::Workspace { .. })
            | (CloseFamily::Tab, T::Tab { .. })
            | (CloseFamily::Panel, T::Panel { .. })
    );
    if !matches {
        return Err(
            Failure::new(FailureKind::Rejected).details(super::stage_reason_details(
                "operation-status",
                "family-mismatch",
            )),
        );
    }
    let Some(generation) = generation else {
        return Ok((result, 0));
    };
    use super::console_interrupt::SignalWatch;
    match signals.take() {
        Ok(None) => {}
        Ok(Some(_)) => return Ok((result, 130)),
        Err(_) => return Ok((result, 1)),
    }
    let resume_id = super::query::request_id()?;
    let (submission, status) = match target {
        T::Workspace { workspace_id } => {
            let request = WorkbenchWorkspaceCloseResume {
                protocol_version: ProtocolVersion,
                request_id: resume_id.clone(),
                session_id: snapshot.session_id,
                workspace_id: workspace_id.clone(),
                close_request_id: operation_id.clone(),
                expected_attempt_generation: generation,
            };
            match client.submit_workbench_workspace_close_resume(request, deadline) {
                Workspace::Returned(outcome) => {
                    let status = if matches!(
                        outcome,
                        orchbee_workbench_protocol::WorkbenchWorkspaceCloseResumeOutcome::Resumed { .. }
                    ) {
                        0
                    } else {
                        3
                    };
                    (json!({"kind":"returned","outcome":outcome}), status)
                }
                Workspace::DefinitelyNotSubmitted(_) => (json!({"kind":"not-submitted"}), 4),
                Workspace::OutcomeUnknown { .. } => (json!({"kind":"outcome-unknown"}), 8),
            }
        }
        T::Tab {
            terminal_context_id,
            tab_id,
        }
        | T::Panel {
            terminal_context_id,
            tab_id,
        } => {
            let request = WorkbenchTabCloseResume {
                protocol_version: ProtocolVersion,
                request_id: resume_id.clone(),
                session_id: snapshot.session_id,
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                close_request_id: operation_id.clone(),
                expected_attempt_generation: generation,
            };
            match client.submit_workbench_tab_close_resume(request, deadline) {
                Tab::Returned(outcome) => {
                    let status = if matches!(
                        outcome,
                        orchbee_workbench_protocol::WorkbenchTabCloseResumeOutcome::Resumed { .. }
                    ) {
                        0
                    } else {
                        3
                    };
                    (json!({"kind":"returned","outcome":outcome}), status)
                }
                Tab::DefinitelyNotSubmitted(_) => (json!({"kind":"not-submitted"}), 4),
                Tab::OutcomeUnknown => (json!({"kind":"outcome-unknown"}), 8),
            }
        }
    };
    result["resume"] =
        json!({"requestId":resume_id,"expectedGeneration":generation,"submission":submission});
    Ok((result, status))
}

/// Stop receipts survive Server exit. Inspect them first; only a definite miss routes to manifests.
pub(crate) fn run_server(id: &WorkbenchRequestId, format: OutputFormat, locale: Locale) -> Output {
    use crate::invocation::{ManifestCommand, ServerStopCommand};
    let raw = id.as_str();
    let mut bytes = [0_u8; 16];
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot =
            u8::from_str_radix(&raw[index * 2..index * 2 + 2], 16).expect("validated request ID");
    }
    let stop = ServerStopCommand::Status {
        id: orchbee_local_client_protocol::StopId::new(bytes).expect("nonzero request ID"),
    };
    let answer = super::server_stop::run(&stop, OutputFormat::Json, locale);
    let doc = serde_json::from_slice::<Value>(&answer.stdout).ok();
    if doc
        .as_ref()
        .is_some_and(|v| v["result"]["outcome"] == "not-recorded")
        && answer.finalization.is_none()
    {
        return super::agent_manifests::run(
            &ManifestCommand::Status {
                operation_id: id.clone(),
            },
            format,
            locale,
        );
    }
    if format == OutputFormat::Json {
        return answer;
    }
    // Render the observation already obtained; never perform a second status exchange to format it.
    let stdout = doc
        .as_ref()
        .and_then(|v| v["result"].as_object())
        .and_then(|r| {
            output::encode_text(&CliText::new(locale).cli_server_stop_result(
                r.get("outcome")?.as_str()?,
                raw,
                r.get("wait").and_then(Value::as_str),
            ))
            .ok()
        })
        .unwrap_or_default();
    Output { stdout, ..answer }
}
