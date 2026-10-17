//! Invocation-owned terminal observation and control streams. The transport and Viewer remain single owners.
use super::{Failure, FailureKind, Output, output, query};
use crate::{
    invocation::TerminalObserveCommand, local_client::LocalClient, localization::Locale,
    terminal_projection::TerminalProjection, windows_signal::ControlInterrupt,
};
use orchbee_client_server_protocol::{
    TerminalViewerAction as Action, TerminalViewerOutcome as Outcome, TerminalViewerRequest,
    ViewerId,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
mod attach;
mod control;
mod display;
pub(crate) use attach::run as run_attach;
mod frame;
mod history;
mod input;
pub(crate) use control::run as run_control;
pub(in crate::cli) mod relay;
const STEP: Duration = Duration::from_secs(5);
const CLEANUP: Duration = Duration::from_secs(2);
pub(crate) const SCHEMA: &str = "orchbee.terminal.stream";

pub(crate) fn run(command: &TerminalObserveCommand, locale: Locale) -> Output {
    let started = Instant::now();
    let total = command
        .timeout_ms
        .map(|ms| started + Duration::from_millis(u64::from(ms)));
    let failure_output = |failure: Failure| Output {
        status: failure.kind.exit_code(),
        stdout: Vec::new(),
        stderr: locale.terminal_stream_failure(&failure),
        finalization: None,
    };
    let signal = match ControlInterrupt::arm() {
        Ok(signal) => signal,
        Err(_) => return failure_output(Failure::new(FailureKind::InternalFailure)),
    };
    let mut relay = match relay::Relay::start() {
        Ok(relay) => relay,
        Err(_) => return failure_output(Failure::new(FailureKind::InternalFailure)),
    };
    let gate = Gate {
        cancelled: &|| signal.requested(),
        wake: Some(signal.wake()),
        total,
    };
    let mut connection = Connection {
        client: None,
        viewer: None,
    };
    let result = observe(command, started + STEP, &gate, &mut connection, &mut relay);
    let cleanup = connection.cleanup();
    let failure = result
        .err()
        .unwrap_or_else(|| Failure::new(FailureKind::InternalFailure));
    // Keep interrupt ownership until the terminal record and relay cleanup finish.
    let failure = if signal.requested() {
        Failure::new(FailureKind::Cancelled)
    } else {
        failure
    };
    if relay.healthy() {
        let record = json!({"kind":"closed", "reason":failure.kind.as_str(), "cleanup":cleanup});
        if let Err(output_failure) = emit(&mut relay, record, Instant::now() + STEP, &|| false) {
            return failure_output(output_failure);
        }
    }
    // Relay and socket are released before the process's final diagnostic is returned.
    drop(relay);
    failure_output(failure)
}

struct Gate<'a> {
    cancelled: &'a dyn Fn() -> bool,
    wake: Option<orchbee_local_ipc::LocalIo<'a>>,
    total: Option<Instant>,
}
impl Gate<'_> {
    fn check(&self, deadline: Instant) -> Result<(), Failure> {
        if (self.cancelled)() {
            return Err(Failure::new(FailureKind::Cancelled));
        }
        if Instant::now() >= deadline || self.total.is_some_and(|end| Instant::now() >= end) {
            return Err(Failure::new(FailureKind::Timeout));
        }
        Ok(())
    }
    fn step(&self) -> Result<Instant, Failure> {
        let deadline = self
            .total
            .map_or(Instant::now() + STEP, |end| end.min(Instant::now() + STEP));
        self.check(deadline)?;
        Ok(deadline)
    }
}

struct Connection {
    client: Option<LocalClient>,
    viewer: Option<ViewerId>,
}
impl Connection {
    fn cleanup(&mut self) -> &'static str {
        let Some(mut client) = self.client.take() else {
            return "not-attached";
        };
        let Some(viewer) = self.viewer.take() else {
            return "session-released";
        };
        let Ok(id) = query::obcs_request_id() else {
            return "session-released";
        };
        let result = client.submit_terminal_viewer_before(
            TerminalViewerRequest::new(
                id,
                Action::Detach {
                    attachment_id: viewer,
                },
            ),
            Instant::now() + CLEANUP,
        );
        if result.is_ok_and(|result| matches!(result.outcome(), Outcome::Detached {})) {
            "detached"
        } else {
            "session-released"
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn observe(
    command: &TerminalObserveCommand,
    admission: Instant,
    gate: &Gate<'_>,
    connection: &mut Connection,
    relay: &mut relay::Relay,
) -> Result<(), Failure> {
    let deadline = admission.min(gate.step()?);
    let snapshot = query::stream_snapshot(deadline, gate.wake);
    gate.check(deadline)?;
    let (client, snapshot) = snapshot?;
    connection.client = Some(client);
    let client = connection.client.as_mut().expect("admitted client");
    let attached =
        query::attach_viewer_cancellable(client, deadline, &snapshot, &command.target, gate.wake);
    // Retain a known viewer even when the response arrives at the cancellation boundary.
    let (terminal, viewer, attachment, epoch, mut projection) = match attached {
        Ok(attached) => attached,
        Err(failure) => {
            gate.check(deadline)?;
            return Err(failure);
        }
    };
    connection.viewer = Some(viewer.clone());
    gate.check(deadline)?;
    emit(
        relay,
        json!({"kind":"opened", "mode":"observe", "source":{
        "siteId":snapshot.site_id, "revision":snapshot.revision,
        "terminalId":terminal.terminal_id, "workerId":terminal.worker_id,
        "viewerId":viewer, "attachmentId":attachment.to_text(), "hostEpoch":epoch.to_text()
    },"width":command.columns,"height":command.rows}),
        gate.step()?,
        &|| (gate.cancelled)(),
    )?;
    observe_frames(client, &viewer, &mut projection, command, gate, relay)
}

fn observe_frames(
    client: &mut LocalClient,
    viewer: &ViewerId,
    projection: &mut TerminalProjection,
    command: &TerminalObserveCommand,
    gate: &Gate<'_>,
    relay: &mut relay::Relay,
) -> Result<(), Failure> {
    loop {
        let outcome = exchange(
            client,
            Action::Poll {
                attachment_id: viewer.clone(),
            },
            gate,
        )?;
        match outcome {
            Outcome::Empty {} => std::thread::sleep(Duration::from_millis(25)),
            Outcome::Record(record) => {
                if projection
                    .receive_lifecycle(&record)
                    .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))?
                {
                    continue;
                }
                let feedback = projection
                    .receive_for_geometry(&record, None)
                    .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))?;
                let Some(feedback) = feedback else {
                    emit(
                        relay,
                        json!({"kind":"reset", "reason":"projection-blocked"}),
                        gate.step()?,
                        &|| (gate.cancelled)(),
                    )?;
                    continue;
                };
                if matches!(feedback, Action::FrameApplied { .. }) {
                    let frame = projection
                        .frame()
                        .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?;
                    emit(
                        relay,
                        frame::record(frame, command.columns, command.rows)?,
                        gate.step()?,
                        &|| (gate.cancelled)(),
                    )?;
                } else if let Action::NeedSnapshot { reason, .. } = &feedback {
                    emit(
                        relay,
                        json!({"kind":"reset", "reason":reason}),
                        gate.step()?,
                        &|| (gate.cancelled)(),
                    )?;
                }
                // Successful stdout flush is a precondition for FrameApplied.
                match exchange(client, feedback, gate)? {
                    Outcome::FeedbackAccepted {} => {}
                    other => return Err(classify(other)),
                }
            }
            other => return Err(classify(other)),
        }
    }
}
fn exchange(client: &mut LocalClient, action: Action, gate: &Gate<'_>) -> Result<Outcome, Failure> {
    let deadline = gate.step()?;
    let result = client.submit_terminal_viewer_cancellable(
        TerminalViewerRequest::new(query::obcs_request_id()?, action),
        Some(deadline),
        gate.wake,
    );
    gate.check(deadline)?;
    result
        .map(|result| result.into_outcome())
        .map_err(query::classify_viewer)
}
fn classify(outcome: Outcome) -> Failure {
    match outcome {
        Outcome::Rejected { reason } => query::classify_viewer_rejected(reason),
        Outcome::Unavailable { .. } => Failure::new(FailureKind::ServiceUnavailable),
        _ => Failure::new(FailureKind::ProtocolInvalid),
    }
}
fn emit(
    relay: &mut relay::Relay,
    mut record: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Failure> {
    record["schema"] = json!(SCHEMA);
    record["version"] = json!(2);
    let bytes =
        output::encode_record(&record).map_err(|_| Failure::new(FailureKind::OutputTooLarge))?;
    relay.record(&bytes, deadline, cancelled).map_err(|error| {
        Failure::new(match error.kind() {
            std::io::ErrorKind::Interrupted => FailureKind::Cancelled,
            std::io::ErrorKind::TimedOut => FailureKind::Timeout,
            _ => FailureKind::InternalFailure,
        })
    })
}

#[cfg(test)]
mod tests;
