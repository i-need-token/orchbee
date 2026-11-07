//! Lists the earlier layouts the Server keeps and reopens one by save time. A reopen is submitted
//! once: an unknown outcome is reported, never replayed, and the CLI never moves focus.
use super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish, SignalWatch},
    failure_output, output, query,
};
use crate::cli_text::CliText;
use crate::{invocation::OutputFormat, local_client::MetadataExchangeReason, localization::Locale};
use orchbee_workbench_protocol::{
    LayoutHistoryAction, LayoutHistoryOutcome, SessionId, WorkbenchLayoutHistoryRequest,
    WorkbenchLayoutHistoryResult, WorkbenchRequestId, WorkbenchResponse,
};
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug)]
enum Observation {
    Server(WorkbenchLayoutHistoryResult),
    Unknown(&'static str),
}

#[derive(Debug)]
struct Answer {
    schema: &'static str,
    request_id: WorkbenchRequestId,
    session_id: SessionId,
    observation: Observation,
    interrupted: bool,
    cleanup_failed: bool,
}

impl Answer {
    fn document(&self) -> serde_json::Value {
        let observation = match &self.observation {
            Observation::Server(reply) => serde_json::json!({"kind":"server",
                "reply":WorkbenchResponse::LayoutHistory(reply.clone())}),
            Observation::Unknown(reason) => serde_json::json!({"kind":"unknown", "reason":reason}),
        };
        serde_json::json!({"schema":self.schema, "version":1, "requestId":self.request_id,
            "sessionId":self.session_id, "observation":observation,
            "interrupted":self.interrupted, "cleanupFailed":self.cleanup_failed})
    }

    fn exit_code(&self) -> u8 {
        let Observation::Server(reply) = &self.observation else {
            return 8;
        };
        if self.cleanup_failed {
            return 1;
        }
        match reply.outcome {
            LayoutHistoryOutcome::Listed { .. }
            | LayoutHistoryOutcome::Reopened { .. }
            | LayoutHistoryOutcome::NothingToReopen {} => 0,
            LayoutHistoryOutcome::NotFound {}
            | LayoutHistoryOutcome::Rejected { .. }
            | LayoutHistoryOutcome::Denied {} => 3,
            LayoutHistoryOutcome::Unavailable { .. } => 4,
        }
    }

    fn emit(&self, format: OutputFormat, locale: Locale) -> Output {
        let encoded = match format {
            OutputFormat::Json => output::encode_document(&self.document()),
            OutputFormat::Text => {
                let text = CliText::new(locale);
                output::encode_text(&match &self.observation {
                    Observation::Server(reply) => {
                        text.cli_layout_history(&reply.outcome, SystemTime::now())
                    }
                    Observation::Unknown(reason) => text.cli_layout_reopen_unknown(reason),
                })
            }
        };
        match encoded {
            Ok(stdout) => Output {
                status: self.exit_code(),
                stdout,
                stderr: if self.cleanup_failed {
                    CliText::new(locale).cli_failure(&Failure::new(FailureKind::InternalFailure))
                } else {
                    String::new()
                },
                finalization: None,
            },
            Err(_) => failure_output(
                Failure::new(FailureKind::OutputTooLarge),
                Some(format),
                locale,
            ),
        }
    }
}

pub(crate) fn list(format: OutputFormat, locale: Locale) -> Output {
    run(
        "orchbee.layout.history",
        LayoutHistoryAction::List {},
        format,
        locale,
    )
}

pub(crate) fn reopen(saved_at: u64, format: OutputFormat, locale: Locale) -> Output {
    run(
        "orchbee.layout.reopen",
        LayoutHistoryAction::Reopen { saved_at },
        format,
        locale,
    )
}

fn run(
    schema: &'static str,
    action: LayoutHistoryAction,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::Layout) {
        return failure_output(failure, Some(format), locale);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
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
    let mut result = submit(schema, action, &mut signals, deadline);
    let mut finalization = None;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            match &mut result {
                Ok(answer) => answer.interrupted = true,
                Err(_) => result = Err(Failure::new(FailureKind::Cancelled)),
            }
        }
        Err(_) => match &mut result {
            Ok(answer) => answer.cleanup_failed = true,
            Err(_) => result = Err(Failure::new(FailureKind::InternalFailure)),
        },
    }
    let mut output = match result {
        Ok(answer) => answer.emit(format, locale),
        Err(failure) => failure_output(failure, Some(format), locale),
    };
    output.finalization = finalization;
    output
}

fn submit(
    schema: &'static str,
    action: LayoutHistoryAction,
    signals: &mut ArmedSignals,
    deadline: Instant,
) -> Result<Answer, Failure> {
    signals.check_before(deadline)?;
    let (mut client, _, snapshot) = query::snapshot_session_until(deadline)?;
    let request = WorkbenchLayoutHistoryRequest {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        session_id: snapshot.session_id,
        action,
    };
    signals.check_before(deadline)?;
    // After handoff, never replace a receipt or unknown outcome with timeout or cancellation.
    let observation = match client.layout_history(request.clone(), deadline, signals.poll_fd()) {
        Ok(reply) => Observation::Server(reply),
        Err(error) if error.submitted => Observation::Unknown(error.reason.as_str()),
        Err(error) => {
            signals.check_before(deadline)?;
            return Err(Failure::new(match error.reason {
                MetadataExchangeReason::ConnectionUnavailable => FailureKind::ServiceUnavailable,
                MetadataExchangeReason::Encoding => FailureKind::InternalFailure,
                MetadataExchangeReason::Deadline => FailureKind::Timeout,
                _ => FailureKind::TransportLoss,
            }));
        }
    };
    Ok(Answer {
        schema,
        request_id: request.request_id,
        session_id: request.session_id,
        observation,
        interrupted: false,
        cleanup_failed: false,
    })
}

#[cfg(test)]
mod tests;
