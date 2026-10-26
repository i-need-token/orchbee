//! One retained member launch; no retry, rollback, or Client focus side effect.
mod receipt;
mod targeting;
use super::{
    Failure, FailureKind, Output,
    caller_context::PanelTarget,
    client_control::{selection, windows::exchange},
    creation::directory,
    failure_output, query,
    source::{ArmedSignals, Finish, SignalWatch},
};
use crate::{
    invocation::{OutputFormat, Target, panel_terminal_creation::PanelTerminalCreationCommand},
    local_client::workbench::creation::{CreationRequest, CreationSubmission},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewOutcome, ProtocolVersion, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchPanelTerminalTarget,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(crate) fn run_parsed(
    command: &PanelTerminalCreationCommand<crate::invocation::program::ParsedLaunch>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let named = matches!(
        command.launch,
        Some(crate::invocation::program::ParsedLaunch::Path { .. })
    );
    match command.clone().into_resolved(format) {
        Ok(command) => {
            let output = run(&command, format, locale);
            super::launch_receipt::annotate(
                output,
                command.launch.as_ref().filter(|_| named),
                format,
                locale,
            )
        }
        Err(failure) => super::parse_failure(failure, locale),
    }
}

pub(crate) fn run(
    command: &PanelTerminalCreationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let settings = match super::configuration::load() {
        Ok(s) => s,
        Err(e) => return failure_output(e.failure(), Some(format), locale),
    };
    let mut command = command.clone();
    settings.apply_directory(&mut command.directory, &mut command.options);
    let deadline = Instant::now() + Duration::from_millis(u64::from(command.timeout_ms));
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(s) => s,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut document = None;
    let mut result = execute(&command, deadline, &mut signals, &mut document);
    let mut finalization = None;
    let mut cancelled = false;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            cancelled = true;
            if !matches!(&result,Err(e) if e.kind==FailureKind::OutcomeUnknown) {
                result = Err(Failure::new(FailureKind::Cancelled));
            }
        }
        Err(_) => {
            if !matches!(&result,Err(e) if e.kind==FailureKind::OutcomeUnknown) {
                result = Err(Failure::new(FailureKind::InternalFailure));
            }
        }
    }
    let mut output = if let Some(mut document) = document {
        let failure = result.err().map(|e| e.kind);
        document["failure"] = json!(failure.map(FailureKind::as_str));
        receipt::emit(&document, failure, format, locale)
    } else {
        failure_output(
            result
                .err()
                .unwrap_or_else(|| Failure::new(FailureKind::InternalFailure)),
            Some(format),
            locale,
        )
    };
    if cancelled {
        output.status = 130;
    }
    output.finalization = finalization;
    output
}
fn execute(
    command: &PanelTerminalCreationCommand,
    deadline: Instant,
    signals: &mut ArmedSignals,
    document: &mut Option<Value>,
) -> Result<(), Failure> {
    let prepared = if let Some(panel) = &command.panel {
        Some(PanelTarget::prepare(Some(panel), |_| None)?)
    } else if command.client.is_none() {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
    } else {
        None
    };
    signals.check_before(deadline)?;
    let session = query::stream_snapshot(deadline, signals.poll_fd());
    signals.check_before(deadline)?;
    let (mut client, snapshot) = session?;
    let panel = if let Some(prepared) = prepared {
        prepared.resolve(&snapshot)?
    } else {
        let reply = exchange(
            &mut client,
            &snapshot.site_id,
            ClientViewAction::List {},
            deadline,
            signals,
        )?;
        let clients = match reply.outcome {
            ClientViewOutcome::Listed { clients } => clients,
            ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
            ClientViewOutcome::Unavailable { .. } => {
                return Err(Failure::new(FailureKind::ServiceUnavailable));
            }
            ClientViewOutcome::Synchronized { .. }
            | ClientViewOutcome::Found { .. }
            | ClientViewOutcome::Operation { .. }
            | ClientViewOutcome::NotFound { .. }
            | ClientViewOutcome::Conflict { .. } => {
                return Err(Failure::new(FailureKind::ProtocolInvalid));
            }
        };
        selection::current_panel(
            selection::select(&clients, command.client.as_ref(), None)?,
            &snapshot,
        )?
    };
    let resolved = targeting::resolve(&snapshot, &panel, command)?;
    let mut options = command.options.clone();
    let directory = directory::resolve_options(
        command.directory,
        &mut options,
        &snapshot,
        Some(&Target::Id(resolved.panel.to_string())),
        &mut client,
        deadline,
        signals,
    )?;
    signals.check_before(deadline)?;
    let request_id = query::request_id()?;
    let action = WorkbenchAction::PanelCreateTerminal {
        terminal_context_id: resolved.context.clone(),
        tab_id: resolved.tab.clone(),
        panel_id: resolved.panel.clone(),
        target: resolved.target.clone(),
        geometry: command.geometry,
        launch: command.launch.clone(),
        profile: command.profile.clone(),
        options: Box::new(options),
    };
    let request = CreationRequest::try_from(WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: request_id.clone(),
        site_id: snapshot.site_id.clone(),
        expected_revision: snapshot.revision,
        action,
    })
    .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let submission = client.submit_workbench_creation(request, deadline, signals.poll_fd());
    drop(client);
    let (outcome, failure) = match submission {
        CreationSubmission::Outcome(outcome) => {
            let reused = matches!(&resolved.target, WorkbenchPanelTerminalTarget::Child { .. })
                && match &*outcome {
                    WorkbenchCommandOutcome::PanelTerminalPending {
                        presentation_id, ..
                    }
                    | WorkbenchCommandOutcome::PanelTerminalCreated {
                        presentation_id, ..
                    } => targeting::presentation_exists(&snapshot, presentation_id),
                    // This step consumes its specific mutation receipt; other command results cannot confirm the requested postcondition.
                    _ => false,
                };
            if reused {
                (
                    json!({"kind":"outcome-unknown","reason":"outcome-mismatch"}),
                    Some(FailureKind::OutcomeUnknown),
                )
            } else {
                let failure = receipt::failure(&outcome);
                (json!(*outcome), failure)
            }
        }
        CreationSubmission::Unknown { reason } => (
            json!({"kind":"outcome-unknown","reason":reason}),
            Some(FailureKind::OutcomeUnknown),
        ),
        CreationSubmission::NotSubmitted { kind, reason } => (
            json!({"kind":"not-submitted","reason":reason}),
            Some(match kind {
                std::io::ErrorKind::Interrupted => FailureKind::Cancelled,
                std::io::ErrorKind::TimedOut => FailureKind::Timeout,
                std::io::ErrorKind::InvalidInput => FailureKind::ProtocolInvalid,
                _ => FailureKind::ServiceUnavailable,
            }),
        ),
    };
    *document = Some(json!({"schema":"orchbee.panel.terminal-create","version":1,
        "resolvedLaunch":null,"source":{"sessionId":snapshot.site_id,"requestId":request_id,"baseRevision":snapshot.revision},
        "target":{"terminalContextId":resolved.context,"tabId":resolved.tab,"panelId":resolved.panel,"member":resolved.target},
        "directory":directory.document(),"result":outcome,"failure":failure.map(FailureKind::as_str)}));
    if let Some(kind) = failure {
        return Err(Failure::new(kind));
    }
    signals.check_before(deadline)
}
