//! Bounded Site view requests with mutation handoff uncertainty retained in the receipt.
use super::{Failure, FailureKind, Output, failure_output, output, query};
use crate::cli::source::{ArmedSignals, Finish, SignalWatch};
use crate::local_client::workbench::metadata_exchange::MetadataExchangeReason;
use crate::{invocation::OutputFormat, localization::Locale};
use orchbee_workbench_protocol::{
    AgentViewAction, AgentViewFailure, AgentViewOutcome, AgentViewRequest, AgentViewResult,
};
use std::time::{Duration, Instant};

enum Observation {
    Server(AgentViewResult),
    Unknown(&'static str),
    ExchangeFailed(MetadataExchangeReason),
}
struct Answer {
    request: AgentViewRequest,
    observation: Observation,
    interrupted: bool,
    cleanup_failed: bool,
}
fn operation(action: &AgentViewAction) -> &'static str {
    match action {
        AgentViewAction::Read {} => "get",
        AgentViewAction::Set { .. } => "set",
        AgentViewAction::Clear { .. } => "clear",
    }
}
impl Answer {
    fn document(&self) -> serde_json::Value {
        let result = match &self.observation {
            Observation::Server(reply) => {
                serde_json::to_value(&reply.outcome).expect("view result encodes")
            }
            Observation::Unknown(reason) => serde_json::json!({"kind":"unknown","reason":reason}),
            Observation::ExchangeFailed(reason) => {
                serde_json::json!({"kind":"exchange-failed","reason":reason.as_str()})
            }
        };
        serde_json::json!({"schema":"orchbee.agent.view","version":2,"requestId":self.request.request_id,"siteId":self.request.site_id,"operation":operation(&self.request.action),"result":result,"interrupted":self.interrupted,"cleanupFailed":self.cleanup_failed})
    }
    fn status(&self) -> u8 {
        let status = match &self.observation {
            Observation::Unknown(_) => 8,
            Observation::ExchangeFailed(reason) => match reason {
                MetadataExchangeReason::ConnectionUnavailable => 4,
                MetadataExchangeReason::Encoding => 1,
                MetadataExchangeReason::Deadline | MetadataExchangeReason::ResponseDeadline => 7,
                _ => 5,
            },
            Observation::Server(r) => match r.outcome {
                AgentViewOutcome::Available { .. } => 0,
                AgentViewOutcome::Failed { reason } => match reason {
                    AgentViewFailure::Denied
                    | AgentViewFailure::SiteMismatch
                    | AgentViewFailure::PluginNotFound
                    | AgentViewFailure::PluginDisabled => 3,
                    AgentViewFailure::Unavailable | AgentViewFailure::SourceUnavailable => 4,
                    AgentViewFailure::InvalidRequest => 5,
                    AgentViewFailure::RevisionExhausted => 1,
                },
            },
        };
        if status != 8 && self.cleanup_failed {
            1
        } else {
            status
        }
    }
    fn emit(&self, format: OutputFormat, locale: Locale) -> Output {
        let encoded = match format {
            OutputFormat::Json => output::encode_document(&self.document()),
            OutputFormat::Text => output::encode_text(&match &self.observation {
                Observation::ExchangeFailed(_) => locale.cli_agent_view_exchange_failed().into(),
                Observation::Server(r) => locale.cli_agent_view(Some(&r.outcome)),
                Observation::Unknown(_) => locale.cli_agent_view(None),
            }),
        };
        match encoded {
            Ok(stdout) => Output {
                status: self.status(),
                stdout,
                stderr: if self.cleanup_failed {
                    locale.cli_failure(&Failure::new(FailureKind::InternalFailure))
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
pub(crate) fn run(action: &AgentViewAction, format: OutputFormat, locale: Locale) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
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
    let mut answer = submit(action, &mut signals, deadline);
    let mut finalization = None;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            match &mut answer {
                Ok(a) => a.interrupted = true,
                Err(_) => answer = Err(Failure::new(FailureKind::Cancelled)),
            }
        }
        Err(_) => match &mut answer {
            Ok(a) => a.cleanup_failed = true,
            Err(_) => answer = Err(Failure::new(FailureKind::InternalFailure)),
        },
    }
    let mut result = match answer {
        Ok(a) => a.emit(format, locale),
        Err(e) => failure_output(e, Some(format), locale),
    };
    result.finalization = finalization;
    result
}
fn submit(
    action: &AgentViewAction,
    signals: &mut ArmedSignals,
    deadline: Instant,
) -> Result<Answer, Failure> {
    action
        .validate()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    signals.check_before(deadline)?;
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    let request = AgentViewRequest {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        site_id: snapshot.site_id,
        action: action.clone(),
    };
    signals.check_before(deadline)?;
    let observation = match client.agent_view(request.clone(), deadline, signals.poll_fd()) {
        Ok(reply) => Observation::Server(reply),
        Err(e) if e.submitted && action.mutating() => Observation::Unknown(e.reason.as_str()),
        Err(e) => Observation::ExchangeFailed(e.reason),
    };
    Ok(Answer {
        request,
        observation,
        interrupted: false,
        cleanup_failed: false,
    })
}
#[cfg(test)]
mod tests;
