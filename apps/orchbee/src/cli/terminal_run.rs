//! One shell submission followed by bounded observation, preserving every receipt: a correlated
//! managed-shell completion under `--wait`, or text markers in any shell under `--capture`.
mod capture;

use crate::cli::command_interrupt::{self, ArmedSignals, Finish};
use crate::cli::console_interrupt::{SignalWatch, SourceFailure};
use crate::cli::{Failure, FailureKind, Output, output, query, send, terminal_output};
use crate::cli_text::CliText;
use crate::invocation::{
    CaptureShell, ExpectedValue, MAXIMUM_SEND_PASTE_BYTES, OutputFormat, OutputSelection,
    OutputSource, RunCommand, Target, UsageContext, UsageReason,
};
use crate::local_client::LocalClient;
use crate::localization::Locale;
use crate::terminal_projection::TerminalProjection;
use orchbee_client_server_protocol::ViewerId;
use orchbee_terminal_protocol::{TerminalObservedCommand, TerminalTextFrame};
use serde_json::{Value, json};
use std::num::NonZeroU64;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Attempt {
    source: Option<Value>,
    run_id: String,
    terminal_id: Option<String>,
    cursor: Option<String>,
    receipt: Option<send::Receipt>,
    completion: Option<Value>,
    shell: Option<CaptureShell>,
    captured: Option<capture::Captured>,
    cleanup: &'static str,
}

pub(in crate::cli) fn run(
    target: &Target,
    command: &RunCommand,
    timeout_ms: u32,
    shell: Option<CaptureShell>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let Err(failure) =
        crate::cli::execution::admit(crate::cli::execution::Capability::TerminalRun)
    {
        return crate::cli::failure_output(failure, Some(format), locale);
    }
    let started = Instant::now();
    let Some(mut deadline) = started.checked_add(Duration::from_millis(u64::from(timeout_ms)))
    else {
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
    let prepared = query::obcs_request_id().and_then(|id| {
        let run_id = id.as_str().to_owned();
        let input = submitted(command, shell, &run_id, &mut deadline, &mut signals)?;
        Ok((run_id, input))
    });
    let (run_id, input) = match prepared {
        Ok(prepared) => prepared,
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
        shell,
        cleanup: "not-attached",
        ..Attempt::default()
    };
    let mut failure = execute(target, &input, deadline, &mut signals, &mut attempt).err();
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
    let exit_code = match (&attempt.completion, &attempt.captured) {
        (Some(completion), _) => completion["exitCode"]
            .as_i64()
            .and_then(|code| i32::try_from(code).ok()),
        (None, Some(captured)) => captured.status,
        (None, None) => None,
    };
    let accepted = attempt
        .receipt
        .as_ref()
        .is_some_and(send::Receipt::accepted);
    let status = match (&failure, &attempt.receipt) {
        (Some(failure), _) => failure.kind.exit_code(),
        (None, Some(receipt)) if !accepted => receipt.exit_status(),
        (None, _) if exit_code.is_some_and(|code| code != 0) => 1,
        (None, _) => 0,
    };
    let document = json!({
        "schema": "orchbee.terminal.run", "version": 1, "source": attempt.source,
        "result": {
            "runId": attempt.run_id,
            "terminalId": attempt.terminal_id,
            "cursor": attempt.cursor,
            "submission": attempt.receipt.as_ref().map(send::Receipt::document),
            "completion": attempt.completion,
            "capture": attempt.shell.map(|shell| capture_document(shell, attempt.captured.as_ref())),
            "failure": failure.as_ref().map(Failure::observation),
            "cleanup": attempt.cleanup,
            "timeoutMs": timeout_ms,
            "elapsedMs": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
        }
    });
    let text = CliText::new(locale);
    let terminal = attempt.terminal_id.as_deref().unwrap_or("");
    // A capture that saw its end marker reports the command's output whatever its status was.
    let captured = attempt
        .captured
        .as_ref()
        .filter(|captured| failure.is_none() && captured.status.is_some());
    let stderr = match (attempt.shell, captured) {
        (None, _) if status == 0 => String::new(),
        (None, _) => text.cli_terminal_run_result(terminal, false),
        (Some(_), Some(captured)) => {
            let mut notes = String::new();
            if captured.output.is_none() {
                notes.push_str(&text.cli_terminal_run_output_unmarked());
            } else if captured.truncated {
                notes.push_str(&text.cli_terminal_run_output_truncated());
            }
            if let Some(code) = captured.status.filter(|code| *code != 0) {
                notes.push_str(&text.cli_terminal_run_exited(terminal, code));
            }
            notes
        }
        (Some(_), None) => text.cli_terminal_run_not_captured(terminal, &attempt.run_id, accepted),
    };
    let stdout = match (format, attempt.shell, captured) {
        (OutputFormat::Json, ..) => output::encode_document(&document),
        (OutputFormat::Text, Some(_), Some(captured)) => {
            output::encode_text(&captured.output.as_deref().map_or_else(String::new, lines))
        }
        (OutputFormat::Text, None, _) if status == 0 => {
            output::encode_text(&text.cli_terminal_run_result(terminal, true))
        }
        (OutputFormat::Text, ..) => output::encode_text(""),
    };
    match stdout {
        Ok(stdout) => Output {
            stdout,
            stderr,
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

/// The text one paste submits: the command, read under the run's own deadline when a source
/// declares it, inside this run's markers or managed-shell prefix. A declared source holds UTF-8
/// text without its trailing line breaks; a capture also submits CRLF line breaks as LF.
fn submitted(
    command: &RunCommand,
    shell: Option<CaptureShell>,
    run_id: &str,
    deadline: &mut Instant,
    signals: &mut ArmedSignals,
) -> Result<String, Failure> {
    let (option, path) = match command {
        RunCommand::Operand(_) => ("<command>", None),
        RunCommand::File(path) => ("--file", Some(path.as_str())),
        RunCommand::Stdin => ("--stdin", None),
    };
    let invalid = |expected| Failure {
        usage: Some(Box::new(UsageReason::InvalidValue {
            context: UsageContext::Path("terminal run"),
            option,
            value: path.unwrap_or_default().to_owned(),
            expected,
        })),
        ..Failure::new(FailureKind::InvalidUsage)
    };
    let text = match command {
        RunCommand::Operand(text) => text.clone(),
        RunCommand::File(_) | RunCommand::Stdin => {
            let bytes = command_interrupt::acquire_document(
                path,
                MAXIMUM_SEND_PASTE_BYTES,
                deadline,
                signals,
            )
            .map_err(|failure| match failure {
                SourceFailure::Standard(failure) => failure,
                SourceFailure::Timeout { .. } => error(FailureKind::Timeout, "input-unavailable"),
                SourceFailure::Cancelled { .. } => {
                    error(FailureKind::Cancelled, "input-unavailable")
                }
            })?;
            let text = String::from_utf8(bytes).map_err(|_| invalid(ExpectedValue::CommandText))?;
            let text = text.replace("\r\n", "\n");
            let text = text.trim_end_matches(['\r', '\n']);
            if text.is_empty() {
                return Err(invalid(ExpectedValue::CommandText));
            }
            if shell == Some(CaptureShell::Cmd) && text.contains(['\n', '\r']) {
                return Err(invalid(ExpectedValue::CmdCommandLine));
            }
            text.to_owned()
        }
    };
    let input = match shell {
        Some(shell) => capture::wrapped(shell, run_id, &text.replace("\r\n", "\n")),
        None => submitted_line(run_id, &text),
    };
    if input.len() > MAXIMUM_SEND_PASTE_BYTES {
        return Err(invalid(ExpectedValue::CommandText));
    }
    Ok(input)
}

fn execute(
    target: &Target,
    input: &str,
    deadline: Instant,
    signals: &mut ArmedSignals,
    attempt: &mut Attempt,
) -> Result<(), Failure> {
    check(signals, deadline)?;
    let (mut client, snapshot) = query::stream_snapshot(step(deadline), signals.poll_fd())?;
    attempt.source = Some(crate::cli::results::snapshot_source(&snapshot));
    check(signals, deadline)?;
    let (terminal, viewer, _, _, mut projection) = query::attach_viewer_cancellable(
        &mut client,
        step(deadline),
        &snapshot,
        target,
        signals.poll_fd(),
    )?;
    attempt.terminal_id = Some(terminal.terminal_id.clone());
    attempt.cleanup = "requested";
    let outcome: Result<(), Failure> = (|| {
        query::poll_applied_frame(
            &mut client,
            step(deadline),
            &viewer,
            &mut projection,
            signals.poll_fd(),
        )?;
        check(signals, deadline)?;
        let frame = projection
            .frame()
            .ok_or_else(query::viewer_protocol_invalid)?;
        query::acknowledge_frame(&mut client, step(deadline), &viewer, frame)?;
        if attempt.shell.is_some() {
            // A full-screen program would take the markers as its own input.
            if frame.alternate_screen {
                return Err(error(FailureKind::Rejected, "full-screen-program"));
            }
        } else {
            // Only an explicitly advertised integration strips the run prefix.
            frame
                .command_observation
                .filter(|value| value.ready)
                .ok_or_else(|| error(FailureKind::Rejected, "shell-not-ready-or-unsupported"))?;
        }
        attempt.cursor = terminal_output::capture_cursor(&terminal.terminal_id, frame)
            .map(|cursor| cursor.encode());
        check(signals, deadline)?;
        attempt.receipt = Some(send::submit_prompt_on_session_cancellable(
            &mut client,
            &snapshot,
            &Target::Id(terminal.terminal_id.clone()),
            input,
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
        if attempt.shell.is_some() {
            let mut observer = Observer {
                client: &mut client,
                viewer: &viewer,
                projection: &mut projection,
                signals,
                sequence: NonZeroU64::MIN,
            };
            let markers = capture::Markers::new(&attempt.run_id);
            return observer.capture(&markers, deadline, &mut attempt.captured);
        }
        loop {
            check(signals, deadline)?;
            query::poll_applied_frame(
                &mut client,
                deadline,
                &viewer,
                &mut projection,
                signals.poll_fd(),
            )?;
            check(signals, deadline)?;
            let frame = projection
                .frame()
                .ok_or_else(query::viewer_protocol_invalid)?;
            query::acknowledge_frame(&mut client, step(deadline), &viewer, frame)?;
            if let Some(command) = completed(frame, &attempt.run_id) {
                attempt.completion = Some(json!({
                    "exitCode": command.exit_code,
                    "statusKind": STATUS_KIND,
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
        query::wait_detach(&mut client, deadline, &viewer)
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

/// The read-only viewer a capture watches the terminal through after its submission.
struct Observer<'a> {
    client: &'a mut LocalClient,
    viewer: &'a ViewerId,
    projection: &'a mut TerminalProjection,
    signals: &'a mut ArmedSignals,
    /// Strictly increasing across this viewer's row reads.
    sequence: NonZeroU64,
}

impl Observer<'_> {
    /// Watch the screen until this run's end marker appears, then read back to its begin
    /// marker. When the observation budget ends first, one more read keeps the output printed
    /// so far; the rest of the run's time is left for releasing the viewer.
    fn capture(
        &mut self,
        markers: &capture::Markers,
        deadline: Instant,
        captured: &mut Option<capture::Captured>,
    ) -> Result<(), Failure> {
        let allowance =
            (deadline.saturating_duration_since(Instant::now()) / 5).min(Duration::from_secs(2));
        let observe = deadline - allowance;
        let ended = loop {
            if let Err(failure) = self.next_frame(observe) {
                break failure;
            }
            let frame = self
                .projection
                .frame()
                .ok_or_else(query::viewer_protocol_invalid)?;
            if frame.alternate_screen {
                continue;
            }
            let visible = terminal_output::frame_rows(
                frame,
                OutputSelection {
                    source: OutputSource::Visible,
                    lines: None,
                },
            )?;
            if !markers.ended_in(&visible) {
                continue;
            }
            // `None` means the screen changed during the reads; the next frame retries.
            if let Some(found) = scan(
                self.client,
                self.viewer,
                &mut self.sequence,
                frame,
                markers,
                step(deadline),
            )? && found.status.is_some()
            {
                *captured = Some(found);
                return Ok(());
            }
        };
        if ended.kind != FailureKind::Timeout || Instant::now() < observe {
            return Err(ended);
        }
        let timeout = error(FailureKind::Timeout, "timeout");
        let settle = deadline - allowance / 2;
        loop {
            let frame = self
                .projection
                .frame()
                .ok_or_else(query::viewer_protocol_invalid)?;
            if frame.alternate_screen {
                return Err(timeout);
            }
            // The end marker may also have arrived after the last observed frame.
            match scan(
                self.client,
                self.viewer,
                &mut self.sequence,
                frame,
                markers,
                settle,
            ) {
                Ok(Some(found)) => {
                    let finished = found.status.is_some();
                    *captured = Some(found);
                    return if finished { Ok(()) } else { Err(timeout) };
                }
                Ok(None) if self.next_frame(settle).is_ok() => {}
                Ok(None) | Err(_) => return Err(timeout),
            }
        }
    }

    /// Wait for and acknowledge the next applied frame before `deadline`.
    fn next_frame(&mut self, deadline: Instant) -> Result<(), Failure> {
        check(self.signals, deadline)?;
        if let Err(failure) = query::poll_applied_frame(
            self.client,
            deadline,
            self.viewer,
            self.projection,
            self.signals.poll_fd(),
        ) {
            return Err(check(self.signals, deadline).err().unwrap_or(failure));
        }
        check(self.signals, deadline)?;
        let frame = self
            .projection
            .frame()
            .ok_or_else(query::viewer_protocol_invalid)?;
        query::acknowledge_frame(self.client, step(deadline), self.viewer, frame)
    }
}

/// Read `frame`'s rows from the bottom upward, one bounded window at a time, until the begin
/// marker or the oldest retained row; `None` when the terminal changed during the reads.
fn scan(
    client: &mut LocalClient,
    viewer: &ViewerId,
    sequence: &mut NonZeroU64,
    frame: &TerminalTextFrame,
    markers: &capture::Markers,
    deadline: Instant,
) -> Result<Option<capture::Captured>, Failure> {
    let mut end = frame
        .scrollback_rows
        .checked_add(u64::from(frame.geometry.rows))
        .ok_or_else(query::viewer_protocol_invalid)?;
    let window = (orchbee_terminal_protocol::MAXIMUM_FRAME_CELLS
        / usize::from(frame.geometry.columns.max(1)))
    .clamp(1, orchbee_terminal_protocol::MAXIMUM_ROW_WINDOW_ROWS);
    let window = u64::try_from(window).map_err(|_| query::viewer_protocol_invalid())?;
    let mut scan = capture::Scan::new(markers);
    loop {
        let start = end.saturating_sub(window);
        let rows = if start == end {
            Vec::new()
        } else {
            let count = u16::try_from(end - start).map_err(|_| query::viewer_protocol_invalid())?;
            let read =
                query::read_text_rows(client, deadline, viewer, frame, (start, count), *sequence)?;
            *sequence = sequence
                .checked_add(1)
                .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
            match read {
                Ok(rows) => rows,
                Err(_) => return Ok(None),
            }
        };
        if let Some(found) = scan.push(rows, start == 0) {
            return Ok(Some(found));
        }
        end = start;
    }
}

fn capture_document(shell: CaptureShell, captured: Option<&capture::Captured>) -> Value {
    json!({
        "shell": shell.token(),
        "exitCode": captured.and_then(|captured| captured.status),
        "output": captured.and_then(|captured| captured.output.as_deref()),
        "truncated": captured.is_some_and(|captured| captured.truncated),
    })
}

/// Captured output for people, one escaped line per printed line.
fn lines(output: &str) -> String {
    let mut rendered = String::new();
    for line in output.lines() {
        rendered.push_str(&crate::cli_text::escape::unquoted(line));
        rendered.push('\n');
    }
    rendered
}

/// The shells that advertise readiness on this host strip this run prefix: on Windows only managed
/// PowerShell does, which reads `<#orchbee-run:<id>#> ` as its comment form; on macOS and Linux only
/// the managed zsh and bash do, which take the `: orchbee-run:<id>; ` no-op form.
fn submitted_line(run_id: &str, command: &str) -> String {
    if cfg!(windows) {
        format!("<#orchbee-run:{run_id}#> {command}")
    } else {
        format!(": orchbee-run:{run_id}; {command}")
    }
}

/// What `exitCode` means: managed PowerShell reports `$?` as 0 or 1, and managed zsh and bash
/// report `$?`, the last command's exit status.
const STATUS_KIND: &str = if cfg!(windows) {
    "powershell-success"
} else {
    "shell-exit-status"
};

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
