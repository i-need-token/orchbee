//! One retained semantic subscription. No reconnect, replay, PTY input or navigation effect.
use super::{
    Failure, FailureKind, Output, SourceFinalization, failure_output, output, query, results,
    terminal_stream::relay::Relay,
};
use crate::{
    invocation::{OutputFormat, Resource, events::Command},
    local_client::LocalClient,
    localization::Locale,
    windows_signal::ControlInterrupt,
};
use orchbee_workbench_protocol::{
    ProtocolVersion, SemanticEventCursor, SemanticEventData, SemanticEventFilter as Filter,
    SemanticEventKind as Kind, SemanticEventMatch, SemanticEventPosition,
    SemanticEventScope as Scope, SemanticEventSource, SemanticEventsAction as Action,
    SemanticEventsFailure as Reason, SemanticEventsOutcome as Outcome, SemanticEventsRequest,
    SemanticEventsResult, Snapshot, WorkbenchRevision,
};
use serde_json::json;
use std::time::{Duration, Instant};
mod filters;
mod text;

pub(crate) fn run(command: &Command, format: OutputFormat, locale: Locale) -> Output {
    let signal = match ControlInterrupt::arm() {
        Ok(signal) => signal,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut relay = if command.wait {
        None
    } else {
        match Relay::start() {
            Ok(relay) => Some(relay),
            Err(_) => {
                return failure_output(
                    Failure::new(FailureKind::InternalFailure),
                    Some(format),
                    locale,
                );
            }
        }
    };
    let result = execute(command, format, locale, &signal, relay.as_mut());
    drop(relay);
    let mut result = result.unwrap_or_else(|failure| {
        let failure = if failure.details.is_none() {
            failure
                .failure_schema("orchbee.cli.failure.events")
                .details(serde_json::Map::from_iter([(
                    "stage".into(),
                    json!("events"),
                )]))
        } else {
            failure
        };
        failure_output(failure, Some(format), locale)
    });
    if !signal.finish_clear() {
        result.status = FailureKind::Cancelled.exit_code();
        result.finalization = Some(SourceFinalization::new(signal));
    }
    result
}

fn execute(
    command: &Command,
    format: OutputFormat,
    locale: Locale,
    signal: &ControlInterrupt,
    mut relay: Option<&mut Relay>,
) -> Result<Output, Failure> {
    let (mut client, snapshot) =
        query::stream_snapshot(Instant::now() + Duration::from_secs(5), Some(signal.wake()))?;
    let filters = filters::resolve(command, &snapshot)?;
    let subscription_id = query::request_id()?;
    let opened = exchange(
        &mut client,
        &snapshot,
        Action::Subscribe {
            subscription_id: subscription_id.clone(),
            filters: filters.iter().map(|(filter, _)| filter.clone()).collect(),
            after: command.after.clone(),
        },
        signal,
    )?;
    let Outcome::Opened {
        cursor,
        existing: false,
        ..
    } = &opened.outcome
    else {
        return finish_reply(opened, "not-opened", &snapshot, format, locale);
    };
    let mut tracker = Tracker {
        cursor: cursor.clone(),
        probe_sequence: WorkbenchRevision::new(0),
    };
    if let Some(relay) = relay.as_mut() {
        emit(relay, "opened", locale, &opened, &snapshot, format, signal)?;
    }
    let deadline = command
        .timeout_ms
        .map(|ms| Instant::now() + Duration::from_millis(u64::from(ms)));
    let mut first = true;
    let mut last_wait = None;
    let monitored = (|| {
        loop {
            if signal.requested() {
                break Err(Failure::new(FailureKind::Cancelled));
            }
            if !first && deadline.is_some_and(|end| Instant::now() >= end) {
                if command.wait {
                    break last_wait
                        .take()
                        .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid));
                }
                break Err(Failure::new(FailureKind::Timeout));
            }
            first = false;
            let timeout = deadline.map_or(1000, |end| {
                end.saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(1000) as u16
            });
            let reply = exchange(
                &mut client,
                &snapshot,
                Action::Wait {
                    subscription_id: subscription_id.clone(),
                    timeout_milliseconds: timeout,
                },
                signal,
            )?;
            tracker.observe(&reply)?;
            match &reply.outcome {
                Outcome::Matched { event, .. } => {
                    if !filters
                        .iter()
                        .any(|(filter, regex)| filters::matches(event, filter, regex.as_ref()))
                    {
                        break Err(Failure::new(FailureKind::ProtocolInvalid));
                    }
                    if let Some(relay) = relay.as_mut() {
                        emit(relay, "event", locale, &reply, &snapshot, format, signal)?;
                    } else {
                        break Ok(reply);
                    }
                }
                Outcome::Gap { .. } if command.wait => break Ok(reply),
                Outcome::Gap { .. } => {
                    if let Some(relay) = relay.as_mut() {
                        emit(relay, "gap", locale, &reply, &snapshot, format, signal)?;
                    }
                }
                Outcome::TimedOut { .. } => {
                    if deadline.is_some_and(|end| Instant::now() >= end) {
                        break Ok(reply);
                    }
                    last_wait = Some(reply);
                }
                Outcome::Failed {
                    reason: Reason::Busy,
                } if deadline.is_none_or(|end| Instant::now() < end) => {
                    last_wait = Some(reply);
                    std::thread::sleep(Duration::from_millis(20));
                }
                Outcome::Failed { .. } => break Ok(reply),
                _ => break Err(Failure::new(FailureKind::ProtocolInvalid)),
            }
        }
    })();
    // Close shares the established connection. Any lost close leaves cleanup with EOF ownership.
    let cleanup = match exchange(
        &mut client,
        &snapshot,
        Action::Close { subscription_id },
        signal,
    ) {
        Ok(SemanticEventsResult {
            outcome: Outcome::Closed { .. },
            ..
        }) => "closed",
        _ => "session-released",
    };
    drop(client);
    if let Some(relay) = relay {
        let (status, reason) = match &monitored {
            Ok(reply) => (status(&reply.outcome), reason(&reply.outcome)),
            Err(failure) => (failure.kind.exit_code(), failure.kind.as_str()),
        };
        if relay.healthy() {
            let record = json!({"schema":"orchbee.events.stream","version":4,"kind":"closed","source":source(&snapshot),"reason":reason,"cleanup":cleanup});
            let bytes = if format == OutputFormat::Json {
                output::encode_record(&record)
            } else {
                output::encode_text(&text::closed(
                    reason,
                    cleanup,
                    &text::clock(std::time::SystemTime::now()),
                ))
            }
            .map_err(|_| Failure::new(FailureKind::OutputTooLarge))?;
            relay
                .record(&bytes, Instant::now() + Duration::from_secs(2), &|| false)
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
        }
        return Ok(Output {
            status,
            stdout: Vec::new(),
            stderr: if status == 0 {
                String::new()
            } else {
                locale.events_subscribe_status(status).into()
            },
            finalization: None,
        });
    }
    finish_reply(monitored?, cleanup, &snapshot, format, locale)
}

fn source(snapshot: &Snapshot) -> serde_json::Value {
    json!({"kind":"server","siteId":snapshot.site_id,"revision":snapshot.revision})
}
fn public_reply(reply: &SemanticEventsResult) -> Result<serde_json::Value, Failure> {
    let mut value =
        serde_json::to_value(reply).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    if let Outcome::Matched { event, .. } = &reply.outcome {
        value["outcome"]["event"]["event"] =
            json!(crate::invocation::events::public_name(event.event));
    }
    Ok(value)
}
fn finish_reply(
    reply: SemanticEventsResult,
    cleanup: &str,
    snapshot: &Snapshot,
    format: OutputFormat,
    locale: Locale,
) -> Result<Output, Failure> {
    let status = status(&reply.outcome);
    let stdout = match format {
        OutputFormat::Json => output::encode_record(&json!({"schema":"orchbee.events.wait","version":4,"source":source(snapshot),"result":public_reply(&reply)?,"cleanup":cleanup})),
        OutputFormat::Text => {
            let mut text = locale.events_status(status).to_owned();
            if let Outcome::Matched {event, ..} = &reply.outcome { text.push_str(&format!("{}\t{}\n", crate::invocation::events::public_name(event.event), super::escape::escape(&serde_json::to_string(&event.data).map_err(|_| Failure::new(FailureKind::InternalFailure))?))); }
            output::encode_text(&text)
        }
    }.map_err(|_| Failure::new(FailureKind::OutputTooLarge))?;
    Ok(Output {
        status,
        stdout,
        stderr: String::new(),
        finalization: None,
    })
}
fn status(outcome: &Outcome) -> u8 {
    match outcome {
        Outcome::Matched { .. } => 0,
        Outcome::TimedOut { .. } => 7,
        Outcome::Gap { .. } => 8,
        Outcome::Failed {
            reason: Reason::PhysicalUnknown,
        } => 8,
        Outcome::Failed {
            reason: Reason::Unavailable | Reason::SourceUnavailable | Reason::RevisionExhausted,
        } => 4,
        Outcome::Failed { .. } => 3,
        _ => 5,
    }
}
fn reason(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::TimedOut { .. } => "timeout",
        Outcome::Gap { .. } => "history-lost",
        Outcome::Failed {
            reason: Reason::PhysicalUnknown,
        } => "physical-unknown",
        Outcome::Failed { .. } => "refused",
        _ => "completed",
    }
}
fn exchange(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    action: Action,
    signal: &ControlInterrupt,
) -> Result<SemanticEventsResult, Failure> {
    use crate::local_client::workbench::metadata_exchange::MetadataExchangeReason as E;
    if signal.requested() {
        return Err(Failure::new(FailureKind::Cancelled));
    }
    client
        .semantic_events(
            SemanticEventsRequest {
                protocol_version: ProtocolVersion,
                request_id: query::request_id()?,
                site_id: snapshot.site_id.clone(),
                action,
            },
            Instant::now() + Duration::from_secs(3),
            Some(signal.wake()),
        )
        .map_err(|error| {
            Failure::new(if signal.requested() {
                FailureKind::Cancelled
            } else {
                match error.reason {
                    E::Protocol | E::Correlation | E::Encoding => FailureKind::ProtocolInvalid,
                    E::Deadline | E::ResponseDeadline => FailureKind::Timeout,
                    E::SessionUnavailable | E::NotSubmitted => FailureKind::ServiceUnavailable,
                    _ => FailureKind::TransportLoss,
                }
            })
        })
}
fn emit(
    relay: &mut Relay,
    kind: &str,
    locale: Locale,
    reply: &SemanticEventsResult,
    snapshot: &Snapshot,
    format: OutputFormat,
    signal: &ControlInterrupt,
) -> Result<(), Failure> {
    let bytes = if format == OutputFormat::Json { output::encode_record(&json!({"schema":"orchbee.events.stream","version":4,"kind":kind,"source":source(snapshot),"result":public_reply(reply)?})) }
    else { output::encode_text(&text::record(kind, &reply.outcome, &text::clock(std::time::SystemTime::now()), locale)) }.map_err(|_| Failure::new(FailureKind::OutputTooLarge))?;
    relay
        .record(&bytes, Instant::now() + Duration::from_secs(3), &|| {
            signal.requested()
        })
        .map_err(|error| {
            Failure::new(match error.kind() {
                std::io::ErrorKind::Interrupted => FailureKind::Cancelled,
                std::io::ErrorKind::TimedOut => FailureKind::Timeout,
                _ => FailureKind::InternalFailure,
            })
        })
}

struct Tracker {
    cursor: SemanticEventCursor,
    probe_sequence: WorkbenchRevision,
}
impl Tracker {
    fn observe(&mut self, reply: &SemanticEventsResult) -> Result<(), Failure> {
        reply
            .validate()
            .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))?;
        let cursor = match &reply.outcome {
            Outcome::Matched { cursor, .. }
            | Outcome::TimedOut { cursor, .. }
            | Outcome::Gap { cursor, .. } => cursor,
            Outcome::Failed { .. } => return Ok(()),
            _ => return Err(Failure::new(FailureKind::ProtocolInvalid)),
        };
        if cursor.epoch != self.cursor.epoch || cursor.sequence < self.cursor.sequence {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        if let Outcome::Gap { gap, .. } = &reply.outcome
            && gap.after != self.cursor.sequence
        {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        if let Outcome::Matched { event, .. } = &reply.outcome {
            match &event.position {
                SemanticEventPosition::Journal { cursor }
                    if cursor.sequence <= self.cursor.sequence =>
                {
                    return Err(Failure::new(FailureKind::ProtocolInvalid));
                }
                SemanticEventPosition::Probe { sequence, .. } => {
                    if *sequence <= self.probe_sequence {
                        return Err(Failure::new(FailureKind::ProtocolInvalid));
                    }
                    self.probe_sequence = *sequence;
                }
                _ => {}
            }
        }
        self.cursor = cursor.clone();
        Ok(())
    }
}
#[cfg(test)]
mod tests;
