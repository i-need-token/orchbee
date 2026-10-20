//! One correlated shell submission followed by bounded observation, preserving every receipt.
use crate::cli::source::{ArmedSignals, Finish, SignalWatch};
use crate::cli::{Failure, FailureKind, Output, output, send};
use crate::invocation::{OutputFormat, Target};
use crate::localization::Locale;
use orchbee_terminal_protocol::{TerminalObservedCommand, TerminalTextFrame};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Attempt {
    source: Option<Value>,
    run_id: String,
    terminal_id: Option<String>,
    cursor: Option<String>,
    receipt: Option<send::Receipt>,
    completion: Option<Value>,
    cleanup: &'static str,
}

pub(in crate::cli) fn run(
    target: &Target,
    command: &str,
    timeout_ms: u32,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let started = Instant::now();
    let Some(deadline) = started.checked_add(Duration::from_millis(u64::from(timeout_ms))) else {
        return crate::cli::failure_output(
            Failure::new(FailureKind::InternalFailure),
            Some(format),
            locale,
        );
    };
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return crate::cli::failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let run_id = match super::obcs_request_id() {
        Ok(id) => id.as_str().to_owned(),
        Err(failure) => {
            let mut result = crate::cli::failure_output(failure, Some(format), locale);
            if let Ok(Finish::Cancelled { finalization, .. }) = signals.finish() {
                result.finalization = Some(finalization);
            }
            return result;
        }
    };
    let mut attempt = Attempt {
        run_id,
        cleanup: "not-attached",
        ..Attempt::default()
    };
    let mut failure = execute(target, command, deadline, &mut signals, &mut attempt).err();
    let finalization = match signals.finish() {
        Ok(Finish::Clear) => None,
        Ok(Finish::Cancelled { finalization, .. }) => {
            if !failure
                .as_ref()
                .is_some_and(|error| error.kind == FailureKind::OutcomeUnknown)
            {
                failure = Some(error(FailureKind::Cancelled, "cancelled"));
            }
            Some(finalization)
        }
        Err(_) => {
            if failure.is_none() {
                failure = Some(error(
                    FailureKind::InternalFailure,
                    "signal-finalization-failed",
                ));
            }
            None
        }
    };
    let status = failure.as_ref().map_or_else(
        || {
            attempt.receipt.as_ref().map_or(0, |receipt| {
                if !receipt.accepted() {
                    receipt.exit_status()
                } else if attempt
                    .completion
                    .as_ref()
                    .is_some_and(|value| value["exitCode"] != 0)
                {
                    1
                } else {
                    0
                }
            })
        },
        |failure| failure.kind.exit_code(),
    );
    let document = json!({
        "schema": "orchbee.terminal.run", "version": 1, "source": attempt.source,
        "result": {
            "runId": attempt.run_id,
            "terminalId": attempt.terminal_id,
            "cursor": attempt.cursor,
            "submission": attempt.receipt.as_ref().map(send::Receipt::document),
            "completion": attempt.completion,
            "failure": failure.as_ref().map(Failure::observation),
            "cleanup": attempt.cleanup,
            "timeoutMs": timeout_ms,
            "elapsedMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
        }
    });
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text if status == 0 => output::encode_text(
            &locale.cli_terminal_run_result(attempt.terminal_id.as_deref().unwrap_or(""), true),
        ),
        OutputFormat::Text => output::encode_text(""),
    };
    match encoded {
        Ok(stdout) => Output {
            stdout,
            stderr: if status == 0 {
                String::new()
            } else {
                locale.cli_terminal_run_result(attempt.terminal_id.as_deref().unwrap_or(""), false)
            },
            status,
            finalization,
        },
        Err(_) => {
            let mut result = crate::cli::failure_output(
                Failure::new(FailureKind::OutputTooLarge),
                Some(format),
                locale,
            );
            result.finalization = finalization;
            result
        }
    }
}

fn execute(
    target: &Target,
    command: &str,
    deadline: Instant,
    signals: &mut ArmedSignals,
    attempt: &mut Attempt,
) -> Result<(), Failure> {
    check(signals, deadline)?;
    let (mut client, snapshot) = super::stream_snapshot(step(deadline), signals.poll_fd())?;
    attempt.source = Some(crate::cli::results::snapshot_source(&snapshot));
    check(signals, deadline)?;
    let (terminal, viewer, _, _, mut projection) = super::attach_viewer_cancellable(
        &mut client,
        step(deadline),
        &snapshot,
        target,
        signals.poll_fd(),
    )?;
    attempt.terminal_id = Some(terminal.terminal_id.clone());
    attempt.cleanup = "requested";
    let outcome: Result<(), Failure> = (|| {
        super::poll_applied_frame(
            &mut client,
            step(deadline),
            &viewer,
            &mut projection,
            signals.poll_fd(),
        )?;
        check(signals, deadline)?;
        let frame = projection
            .frame()
            .ok_or_else(super::viewer_protocol_invalid)?;
        super::acknowledge_frame(&mut client, step(deadline), &viewer, frame)?;
        frame
            .command_observation
            .filter(|value| value.ready)
            .ok_or_else(|| error(FailureKind::Rejected, "shell-not-ready-or-unsupported"))?;
        attempt.cursor = crate::cli::terminal_output::capture_cursor(&terminal.terminal_id, frame)
            .map(|cursor| cursor.encode());
        // Only an explicitly advertised integration strips this exact prefix.
        let input = format!("<#orchbee-run:{}#> {command}", attempt.run_id);
        check(signals, deadline)?;
        attempt.receipt = Some(send::submit_prompt_on_session_cancellable(
            &mut client,
            &snapshot,
            &Target::Id(terminal.terminal_id.clone()),
            &input,
            false,
            step(deadline),
            signals.poll_fd(),
        )?);
        if !attempt
            .receipt
            .as_ref()
            .is_some_and(send::Receipt::accepted)
        {
            return Ok(());
        }
        loop {
            check(signals, deadline)?;
            super::poll_applied_frame(
                &mut client,
                deadline,
                &viewer,
                &mut projection,
                signals.poll_fd(),
            )?;
            check(signals, deadline)?;
            let frame = projection
                .frame()
                .ok_or_else(super::viewer_protocol_invalid)?;
            super::acknowledge_frame(&mut client, step(deadline), &viewer, frame)?;
            if let Some(command) = completed(frame, &attempt.run_id) {
                attempt.completion = Some(json!({
                    "exitCode": command.exit_code,
                    "statusKind": "powershell-success",
                    "outputRange": {
                        "generation": command.generation.get().to_string(),
                        "outputEpoch": command.output_epoch.get().to_string(),
                        "startRow": command.start_row.to_string(),
                        "startColumn": command.start_column,
                        "endRow": command.end_row.to_string(),
                        "endColumn": command.end_column,
                        "retained": frame.route.generation == command.generation
                            && frame.command_observation.is_some_and(|value| value.output_epoch == command.output_epoch)
                    }
                }));
                return Ok(());
            }
        }
    })();
    let cleanup = if Instant::now() < deadline {
        super::wait_detach(&mut client, deadline, &viewer)
    } else {
        Err(error(FailureKind::Timeout, "timeout"))
    };
    attempt.cleanup = if cleanup.is_ok() {
        "confirmed"
    } else {
        "requested"
    };
    drop(client);
    // Preserve an unknown input outcome and any known submission before settlement.
    if let Err(failure) = outcome {
        if failure.kind == FailureKind::OutcomeUnknown {
            return Err(failure);
        }
        check(signals, deadline)?;
        return Err(failure);
    }
    check(signals, deadline)?;
    cleanup
}

fn completed(frame: &TerminalTextFrame, run_id: &str) -> Option<TerminalObservedCommand> {
    let command = frame.command_observation?.command?;
    let text = command
        .id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    (command.completed && text == run_id).then_some(command)
}

fn step(deadline: Instant) -> Instant {
    deadline.min(Instant::now() + Duration::from_secs(5))
}

fn check(signals: &mut ArmedSignals, deadline: Instant) -> Result<(), Failure> {
    if signals
        .take()
        .map_err(|_| error(FailureKind::InternalFailure, "signal-read-failed"))?
        .is_some()
    {
        return Err(error(FailureKind::Cancelled, "cancelled"));
    }
    if Instant::now() >= deadline {
        return Err(error(FailureKind::Timeout, "timeout"));
    }
    Ok(())
}

fn error(kind: FailureKind, reason: &'static str) -> Failure {
    Failure::new(kind).details(crate::cli::stage_reason_details("terminal-run", reason))
}

#[cfg(test)]
mod tests;
