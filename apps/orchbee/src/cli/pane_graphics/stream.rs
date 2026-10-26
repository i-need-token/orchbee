//! One retained producer and bounded stdin/stdout relays. EOF releases only its own layer.
use super::{
    Action, Command, Failure, FailureKind, Observation, Outcome, Output, OutputFormat,
    PaneGraphicsRequest, PaneGraphicsTarget, Signals, Snapshot, exchange, framing, output, query,
    request, resolve,
};
use crate::{
    cli::{
        console_interrupt::SignalWatch,
        stdin_bytes::{Poll, Reader},
        terminal_stream::relay::Relay,
    },
    invocation::pane_graphics::LayerSpec,
    localization::Locale,
};
use orchbee_workbench_protocol::{PaneGraphicsStreamImage, WorkbenchRevision};
use serde_json::json;
use std::{
    io::{self, IsTerminal},
    time::{Duration, Instant},
};

pub(super) fn run(
    command: &Command,
    spec: &LayerSpec,
    format: OutputFormat,
    locale: Locale,
    signals: &mut Signals,
) -> Result<Output, Failure> {
    if io::stdin().is_terminal() {
        return Err(Failure::new(FailureKind::InvalidUsage));
    }
    let deadline = command
        .timeout_ms
        .map(|ms| Instant::now() + Duration::from_millis(u64::from(ms)));
    let admission = deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(5));
    signals.check_before(admission)?;
    let mut input = Reader::start().map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let mut relay = Relay::start().map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let (mut client, snapshot) = query::stream_snapshot(admission, signals.poll_fd())?;
    let target = resolve(command, &snapshot)?;
    let stream_id = query::request_id()?;
    let open = request(
        &snapshot,
        Action::StreamOpen {
            stream_id: stream_id.clone(),
            target: target.clone(),
            layer_id: spec.layer_id.clone(),
            z_index: spec.z_index,
        },
    )?;
    let close = request(
        &snapshot,
        Action::StreamClose {
            stream_id: stream_id.clone(),
        },
    )?;
    let opened = exchange(&mut client, &open, admission, signals)?;
    let mut last_reply = None;
    let mut last_revision = WorkbenchRevision::new(0);
    let mut exact_target = target.clone();
    let mut file_root = None;
    let active = if let Observation::Server(reply) = &opened {
        if let Outcome::StreamOpened {
            target,
            revision,
            file_frame_directory,
            ..
        } = &reply.outcome
        {
            exact_target = target.clone();
            last_revision = *revision;
            file_root = file_frame_directory.clone();
            true
        } else {
            false
        }
    } else {
        false
    };
    let opened_status = opened.status();
    let started = emit(
        &mut relay,
        &snapshot,
        &target,
        spec,
        (&open, &opened),
        format,
        signals,
    );
    if let Observation::Server(reply) = opened {
        last_reply = Some(reply);
    }
    let monitored = if !active {
        Err((
            opened_status,
            if opened_status == 8 {
                "outcome-unknown"
            } else {
                "refused"
            },
        ))
    } else if let Err(failure) = started {
        Err((failure.kind.exit_code(), failure.kind.as_str()))
    } else {
        (|| {
            let mut parser = framing::Parser::new();
            let mut frame_started: Option<Instant> = None;
            let mut last_byte = Instant::now();
            loop {
                if signals.requested() {
                    return Err((130, "cancelled"));
                }
                let now = Instant::now();
                if deadline.is_some_and(|end| now >= end)
                    || frame_started
                        .is_some_and(|start| now.duration_since(start) >= Duration::from_secs(30))
                    || frame_started.is_some()
                        && now.duration_since(last_byte) >= Duration::from_secs(30)
                {
                    return Err((7, "timeout"));
                }
                while let Some(frame) = parser.next().map_err(|reason| (2, reason))? {
                    if let PaneGraphicsStreamImage::File { path, .. } = &frame.image {
                        let Some(root) = &file_root else {
                            return Err((3, "file-unavailable"));
                        };
                        // Server owns the lease and authoritative path check. Refuse unsolicited paths early.
                        let file = std::path::Path::new(path);
                        if file.parent() != Some(std::path::Path::new(root))
                            || file.components().any(|part| {
                                matches!(
                                    part,
                                    std::path::Component::CurDir | std::path::Component::ParentDir
                                )
                            })
                            || path
                                .split(['/', '\\'])
                                .any(|part| matches!(part, "." | ".."))
                        {
                            return Err((2, "invalid-frame"));
                        }
                    }
                    let sequence = frame.sequence;
                    let submitted = request(
                        &snapshot,
                        Action::StreamFrame {
                            stream_id: stream_id.clone(),
                            sequence,
                            revision: frame.revision,
                            image: frame.image,
                            damage: frame.damage,
                        },
                    )
                    .map_err(|failure| (failure.kind.exit_code(), failure.kind.as_str()))?;
                    let exchange_deadline = deadline.map_or_else(
                        || Instant::now() + Duration::from_secs(10),
                        |end| end.min(Instant::now() + Duration::from_secs(10)),
                    );
                    let observed = exchange(&mut client, &submitted, exchange_deadline, signals)
                        .map_err(|failure| (failure.kind.exit_code(), failure.kind.as_str()))?;
                    if let Observation::Server(reply) = &observed
                        && let Outcome::FrameApplied {
                            target, revision, ..
                        } = &reply.outcome
                    {
                        if target != &exact_target || *revision < last_revision {
                            return Err((5, "protocol-invalid"));
                        }
                        last_revision = *revision;
                        parser.accept(sequence);
                    }
                    let receipt_status = observed.status();
                    let emitted = emit(
                        &mut relay,
                        &snapshot,
                        &exact_target,
                        spec,
                        (&submitted, &observed),
                        format,
                        signals,
                    );
                    if let Observation::Server(reply) = observed {
                        last_reply = Some(reply);
                    }
                    if receipt_status != 0 {
                        return Err((
                            receipt_status,
                            if receipt_status == 8 {
                                "outcome-unknown"
                            } else {
                                "refused"
                            },
                        ));
                    }
                    emitted.map_err(|failure| (failure.kind.exit_code(), failure.kind.as_str()))?;
                    frame_started = None;
                    last_byte = Instant::now();
                }
                if parser.in_progress() {
                    frame_started.get_or_insert_with(Instant::now);
                } else {
                    frame_started = None;
                }
                let mut chunk = [0; 8192];
                match input.poll(&mut chunk).map_err(|_| (5, "input-failed"))? {
                    Poll::Bytes(count) => {
                        parser.feed(&chunk[..count]).map_err(|reason| (2, reason))?;
                        last_byte = Instant::now();
                        frame_started.get_or_insert(last_byte);
                    }
                    Poll::Pending => std::thread::sleep(Duration::from_millis(2)),
                    Poll::Eof => {
                        parser.finish().map_err(|reason| (2, reason))?;
                        return Ok(());
                    }
                }
            }
        })()
    };
    // Stop the sole stdin writer before releasing producer state. Drop also waits for the helper.
    drop(input);
    let (mut final_status, reason) = monitored.err().unwrap_or((0, "eof"));
    let (cleanup, close_reply) = if active {
        match client.pane_graphics(&close, Instant::now() + Duration::from_secs(2), None) {
            Ok(reply) if matches!(reply.outcome, Outcome::Closed { .. }) => ("closed", Some(reply)),
            Ok(reply)
                if matches!(
                    reply.outcome,
                    Outcome::Failed {
                        reason: orchbee_workbench_protocol::PaneGraphicsFailure::StreamClosed
                            | orchbee_workbench_protocol::PaneGraphicsFailure::SourceChanged
                            | orchbee_workbench_protocol::PaneGraphicsFailure::NotFound
                    }
                ) =>
            {
                ("retired", Some(reply))
            }
            Ok(reply) => {
                final_status = 8;
                ("unknown", Some(reply))
            }
            Err(error) if error.submitted => {
                final_status = 8;
                ("unknown", None)
            }
            Err(_) => {
                if final_status == 0 {
                    final_status = 8;
                }
                ("connection-ended", None)
            }
        }
    } else if opened_status == 8 {
        ("unknown", None)
    } else {
        ("not-opened", None)
    };
    drop(client);
    if relay.healthy() {
        let document = json!({"schema":"orchbee.panel.graphics.stream", "version":1, "kind":"closed", "sessionId":snapshot.session_id, "target":exact_target, "layerId":spec.layer_id, "reason":reason, "cleanup":cleanup, "lastReply":last_reply, "closeReply":close_reply});
        let bytes = match format {
            OutputFormat::Json => output::encode_record(&document),
            OutputFormat::Text => output::encode_text(&format!("closed\t{reason}\t{cleanup}\n")),
        }
        .map_err(|_| Failure::new(FailureKind::OutputTooLarge))?;
        if relay
            .record(&bytes, Instant::now() + Duration::from_secs(2), &|| false)
            .is_err()
            && final_status != 8
        {
            final_status = 1;
        }
    }
    drop(relay);
    Ok(Output {
        status: final_status,
        stdout: Vec::new(),
        stderr: if final_status == 0 {
            String::new()
        } else {
            locale.pane_graphics_status(final_status).into()
        },
        finalization: None,
    })
}
fn emit(
    relay: &mut Relay,
    snapshot: &Snapshot,
    target: &PaneGraphicsTarget,
    spec: &LayerSpec,
    receipt: (&PaneGraphicsRequest, &Observation),
    format: OutputFormat,
    signals: &Signals,
) -> Result<(), Failure> {
    let (request, observation) = receipt;
    let document = json!({"schema":"orchbee.panel.graphics.stream", "version":1, "kind":"reply", "sessionId":snapshot.session_id, "target":target, "layerId":spec.layer_id, "requestId":request.request_id, "observation":observation.document()});
    let bytes = match format {
        OutputFormat::Json => output::encode_record(&document),
        OutputFormat::Text => output::encode_text(&format!(
            "reply\t{}\n",
            crate::cli::escape::escape(
                &serde_json::to_string(&observation.document())
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))?
            )
        )),
    }
    .map_err(|_| Failure::new(FailureKind::OutputTooLarge))?;
    relay
        .record(&bytes, Instant::now() + Duration::from_secs(3), &|| {
            signals.requested()
        })
        .map_err(|error| {
            Failure::new(match error.kind() {
                io::ErrorKind::Interrupted => FailureKind::Cancelled,
                io::ErrorKind::TimedOut => FailureKind::Timeout,
                _ => FailureKind::InternalFailure,
            })
        })
}
