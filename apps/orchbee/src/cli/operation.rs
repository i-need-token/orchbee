//! `operation status|resume`: the request ID alone finds the operation. Each store that keeps
//! request IDs is asked in a fixed order and only a definite miss passes the ID on, so the first
//! store that keeps it, or that cannot say, answers with its own document. A resume is the
//! owner's existing resume, submitted once.
use super::command_interrupt::{ArmedSignals, Finish};
use super::execution::{self, Capability};
use super::{Failure, FailureKind, Output, failure_output, output};
use crate::cli_text::CliText;
use crate::{
    invocation::{
        OutputFormat, ServerStopCommand, WorkspaceGroupCloseCommand, WorktreeCommand,
        operation::Resume,
    },
    localization::Locale,
};
use orchbee_workbench_protocol::{WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration};
use serde_json::{Value, json};

/// One store's answer to a request ID.
pub(super) enum Probe {
    /// The store keeps no record of the ID and nothing is owed on exit: the next store is asked.
    Miss(Output),
    /// The store keeps the ID; the output is its answer.
    Found(Output),
    /// The store could not say (denied, unavailable, unknown, timed out or interrupted): the
    /// output is the answer and ends the lookup.
    Ended(Output),
}

impl Probe {
    /// `held` is whether the store keeps the ID, or `None` when its answer cannot say.
    pub(super) fn new(held: Option<bool>, output: Output) -> Self {
        match held {
            Some(false) => Self::Miss(output),
            Some(true) => Self::Found(output),
            None => Self::Ended(output),
        }
    }

    pub(super) fn into_output(self) -> Output {
        match self {
            Self::Miss(output) | Self::Found(output) | Self::Ended(output) => output,
        }
    }
}

impl From<Output> for Probe {
    /// An answer settled before the store was asked, such as a failure to arm signals.
    fn from(output: Output) -> Self {
        Self::Ended(output)
    }
}

/// The stores that keep request IDs. An ID a caller reused across stores resolves to the first.
#[derive(Clone, Copy)]
enum Store {
    Stop,
    Close,
    Project,
    Worktree,
    Client,
    Manifests,
}

const STORES: [Store; 6] = [
    Store::Stop,
    Store::Close,
    Store::Project,
    Store::Worktree,
    Store::Client,
    Store::Manifests,
];

impl Store {
    const fn capability(self) -> Capability {
        match self {
            Self::Stop => Capability::ServerStop,
            Self::Close => Capability::RetainedCloseOperation,
            Self::Project => Capability::WorkspaceGroupClose,
            Self::Worktree => Capability::WorktreeOperation,
            Self::Client => Capability::ClientControl,
            Self::Manifests => Capability::AgentManifests,
        }
    }

    fn read(self, id: &WorkbenchRequestId, format: OutputFormat, locale: Locale) -> Probe {
        match self {
            Self::Stop => stop(id, format, locale),
            Self::Close => close(id, None, format, locale),
            Self::Project => super::workspace_group_close::probe(
                &WorkspaceGroupCloseCommand::Status {
                    operation_id: id.clone(),
                },
                format,
                locale,
            ),
            Self::Worktree => super::worktree_operation::probe(
                &WorktreeCommand::Status {
                    operation_id: id.clone(),
                },
                format,
                locale,
            ),
            Self::Client => super::client_control::probe(id, format, locale),
            Self::Manifests => super::agent_manifests::probe(id, format, locale),
        }
    }
}

/// `operation status`, or `operation resume` when `resume` is present. A store this platform does
/// not run is skipped; with none run, its refusal is the answer.
pub(crate) fn run(
    id: &WorkbenchRequestId,
    resume: Option<Resume>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let mut refusal = None;
    let mut asked = false;
    for store in STORES {
        if let Err(failure) = execution::admit(store.capability()) {
            refusal.get_or_insert(failure);
            continue;
        }
        asked = true;
        let probe = match (store, resume) {
            // A close resume reads and submits in one connection.
            (Store::Close, Some(resume)) => close(id, Some(resume.generation), format, locale),
            _ => store.read(id, format, locale),
        };
        let output = match probe {
            Probe::Miss(_) => continue,
            Probe::Ended(output) => return output,
            Probe::Found(output) => output,
        };
        let Some(resume) = resume else {
            return output;
        };
        // Passed through unbounded: a Project or Worktree request refuses an attempt above its
        // range when it is encoded, before anything is submitted.
        let expected_attempt = u32::try_from(resume.generation.value()).unwrap_or(u32::MAX);
        return match store {
            Store::Close => output,
            Store::Project => super::workspace_group_close::probe(
                &WorkspaceGroupCloseCommand::Resume {
                    operation_id: id.clone(),
                    expected_attempt,
                    detach: resume.detach,
                    timeout_ms: resume.timeout_ms,
                },
                format,
                locale,
            )
            .into_output(),
            Store::Worktree => super::worktree_operation::probe(
                &WorktreeCommand::Resume {
                    operation_id: id.clone(),
                    expected_attempt,
                    detach: resume.detach,
                    timeout_ms: resume.timeout_ms,
                },
                format,
                locale,
            )
            .into_output(),
            Store::Stop | Store::Client | Store::Manifests => {
                let mut output = failure_output(
                    Failure::new(FailureKind::Rejected).details(super::stage_reason_details(
                        "operation-resume",
                        "resume-unsupported",
                    )),
                    Some(format),
                    locale,
                );
                output.stderr = CliText::new(locale).operation_resume_unsupported(
                    matches!(store, Store::Stop).then_some(id.as_str()),
                );
                output
            }
        };
    }
    if !asked && let Some(failure) = refusal {
        return failure_output(failure, Some(format), locale);
    }
    let mut output = failure_output(
        Failure::new(FailureKind::NotFound).details(super::stage_reason_details(
            "operation-status",
            "not-recorded",
        )),
        Some(format),
        locale,
    );
    output.stderr = CliText::new(locale).operation_not_recorded();
    output
}

/// Stop receipts survive Server exit and need no running Session.
fn stop(id: &WorkbenchRequestId, format: OutputFormat, locale: Locale) -> Probe {
    let raw = id.as_str();
    let mut bytes = [0_u8; 16];
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot =
            u8::from_str_radix(&raw[index * 2..index * 2 + 2], 16).expect("validated request ID");
    }
    let command = ServerStopCommand::Status {
        id: orchbee_local_client_protocol::StopId::new(bytes).expect("nonzero request ID"),
    };
    let answer = super::server_stop::run(&command, OutputFormat::Json, locale);
    let document = serde_json::from_slice::<Value>(&answer.stdout).ok();
    let result = document.as_ref().and_then(|v| v["result"].as_object());
    let settled = answer.finalization.is_none()
        && result.is_some_and(|r| r.get("wait").is_none_or(Value::is_null));
    let held = match result
        .and_then(|r| r.get("outcome"))
        .and_then(Value::as_str)
    {
        Some("unavailable" | "unknown") | None => None,
        Some(outcome) => settled.then_some(outcome != "not-recorded"),
    };
    if format == OutputFormat::Json {
        return Probe::new(held, answer);
    }
    // Render the observation already obtained; never perform a second status exchange to format it.
    let stdout = result
        .and_then(|r| {
            output::encode_text(&CliText::new(locale).cli_server_stop_result(
                r.get("outcome")?.as_str()?,
                r.get("reason").and_then(Value::as_str),
                raw,
                r.get("wait").and_then(Value::as_str),
            ))
            .ok()
        })
        .unwrap_or_default();
    Probe::new(held, Output { stdout, ..answer })
}

/// The Workspace, Tab and Panel close ledger. With a generation the read is followed, in the same
/// connection, by that target's close resume.
fn close(
    id: &WorkbenchRequestId,
    generation: Option<WorkbenchTabCloseAttemptGeneration>,
    format: OutputFormat,
    locale: Locale,
) -> Probe {
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            )
            .into();
        }
    };
    let (mut output, held) = match observe_close(id, generation, &mut signals) {
        Ok((result, status, held)) => {
            let bytes = match format {
                OutputFormat::Json => output::encode_document(&json!({
                    "schema":"orchbee.operation.close", "version":1, "result":result
                })),
                OutputFormat::Text => {
                    output::encode_text(&CliText::new(locale).operation_result(&result))
                }
            };
            match bytes {
                Ok(stdout) => (
                    Output {
                        status,
                        stdout,
                        stderr: String::new(),
                        finalization: None,
                    },
                    held,
                ),
                Err(_) => (
                    failure_output(
                        Failure::new(FailureKind::OutputTooLarge),
                        Some(format),
                        locale,
                    ),
                    None,
                ),
            }
        }
        Err(failure) => (failure_output(failure, Some(format), locale), None),
    };
    let held = match signals.finish() {
        Ok(Finish::Clear) => held,
        Ok(Finish::Cancelled { finalization, .. }) => {
            output.status = 130;
            output.finalization = Some(finalization);
            None
        }
        Err(_) => {
            output.status = 1;
            None
        }
    };
    Probe::new(held, output)
}

fn observe_close(
    operation_id: &WorkbenchRequestId,
    generation: Option<WorkbenchTabCloseAttemptGeneration>,
    signals: &mut ArmedSignals,
) -> Result<(Value, u8, Option<bool>), Failure> {
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
    let family = match &reply.outcome {
        O::Available { target, .. } => Some(match target {
            T::Workspace { .. } => "workspace",
            T::Tab { .. } => "tab",
            T::Panel { .. } => "panel",
        }),
        O::Unavailable {} | O::NotFound { .. } | O::Denied { .. } => None,
    };
    let mut result =
        json!({"requestId":operation_id,"family":family,"observation":reply,"resume":null});
    let O::Available { target, .. } = &reply.outcome else {
        return Ok(match reply.outcome {
            O::NotFound { .. } => (result, 3, Some(false)),
            O::Unavailable {} => (result, 4, None),
            O::Denied { .. } | O::Available { .. } => (result, 3, None),
        });
    };
    let Some(generation) = generation else {
        return Ok((result, 0, Some(true)));
    };
    use super::console_interrupt::SignalWatch;
    match signals.take() {
        Ok(None) => {}
        Ok(Some(_)) => return Ok((result, 130, Some(true))),
        Err(_) => return Ok((result, 1, Some(true))),
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
    Ok((result, status, Some(true)))
}
