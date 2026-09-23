//! Names use one admitted session and one non-replayed OBAP exchange. A
//! received result remains authoritative even if cancellation races it;
//! an ambiguous mutation is always outcome-unknown, including on Ctrl-C.

use super::*;
use crate::cli::agent_names::{Execution, FAILURE_SCHEMA};
use crate::invocation::AgentNamesCommand;
use orchbee_agent_observation_protocol::{AgentNamesRequest, AgentNamesResult};
use serde_json::{Map, json};

pub(super) fn execute(command: &AgentNamesCommand) -> Execution {
    let total = match deadline_after(AGENT_TOTAL_BUDGET) {
        Ok(total) => total,
        Err(failure) => return Execution::failed(failure),
    };
    let request_id = match obap_request_id() {
        Ok(id) => id,
        Err(failure) => return Execution::failed(failure),
    };
    let request = match command {
        AgentNamesCommand::List => AgentNamesRequest::NamesList { request_id },
        AgentNamesCommand::Resolve(target) => AgentNamesRequest::Resolve {
            request_id,
            target: target.clone(),
        },
        AgentNamesCommand::Assign { target, name } => AgentNamesRequest::NameAssign {
            request_id,
            target: target.clone(),
            name: name.clone(),
        },
        AgentNamesCommand::Release(target) => AgentNamesRequest::NameRelease {
            request_id,
            target: target.clone(),
        },
    };
    let armed = match ArmedSignals::arm_interrupts() {
        Ok(armed) => armed,
        Err(_) => return Execution::failed(Failure::new(FailureKind::InternalFailure)),
    };
    let attempt = span(total, SignalWatch::poll_fd(&armed), request);
    let finish = armed.finish();
    settle(command, attempt, finish)
}

enum Attempt {
    Admission(AgentAdmissionError),
    Exchange(Result<AgentNamesResult, AgentExchangeError>),
}

fn span(total: Instant, wake: Option<BorrowedFd<'_>>, request: AgentNamesRequest) -> Attempt {
    match agent_session_until(total, wake) {
        Err(error) => Attempt::Admission(error),
        Ok(mut client) => {
            let result = client.submit_agent_names_before_cancellable(request, total, wake);
            drop(client);
            Attempt::Exchange(result)
        }
    }
}

fn settle(
    command: &AgentNamesCommand,
    attempt: Attempt,
    finish: Result<Finish, crate::cli::source::SignalError>,
) -> Execution {
    let (cancelled, fault, finalization) = match finish {
        Ok(Finish::Clear) => (false, false, None),
        Ok(Finish::Cancelled { finalization, .. }) => (true, false, Some(finalization)),
        Err(_) => (false, true, None),
    };
    let mut execution = match attempt {
        // Do not erase evidence of a committed mutation with a later signal.
        Attempt::Exchange(Ok(result)) => Execution {
            result: Ok(result),
            finalization: None,
        },
        Attempt::Exchange(Err(AgentExchangeError::Failed { request_id, .. }))
            if command.is_mutation() =>
        {
            Execution::failed(
                Failure::new(FailureKind::OutcomeUnknown)
                    .failure_schema(FAILURE_SCHEMA)
                    .details(Map::from_iter([
                        ("stage".to_owned(), json!("agent-names")),
                        ("operation".to_owned(), json!(command.operation())),
                        ("reason".to_owned(), json!("reply-not-confirmed")),
                        ("requestId".to_owned(), json!(request_id)),
                    ])),
            )
        }
        _ if cancelled => {
            Execution::failed(Failure::new(FailureKind::Cancelled).failure_schema(FAILURE_SCHEMA))
        }
        _ if fault => Execution::failed(
            Failure::new(FailureKind::InternalFailure).failure_schema(FAILURE_SCHEMA),
        ),
        Attempt::Admission(AgentAdmissionError::Preamble(failure)) => Execution::failed(failure),
        Attempt::Admission(AgentAdmissionError::Connect(error)) => {
            Execution::failed(classify_connect(error))
        }
        Attempt::Exchange(Err(error)) => Execution::failed(exchange_failure(error)),
    };
    execution.finalization = finalization;
    execution
}

fn exchange_failure(error: AgentExchangeError) -> Failure {
    let kind = match error {
        AgentExchangeError::SessionUnavailable | AgentExchangeError::RequestEncoding(_) => {
            FailureKind::InternalFailure
        }
        AgentExchangeError::RequestNotSubmitted(error) => io_failure_kind(&error),
        AgentExchangeError::Failed { reason, .. } => match reason {
            AgentExchangeFailure::Write(error)
            | AgentExchangeFailure::ResponseDeadline(error)
            | AgentExchangeFailure::Response(
                orchbee_agent_observation_protocol::AgentWireError::Io(error),
            ) => io_failure_kind(&error),
            _ => FailureKind::ProtocolInvalid,
        },
    };
    Failure::new(kind).failure_schema(FAILURE_SCHEMA)
}

fn io_failure_kind(error: &std::io::Error) -> FailureKind {
    if wake_marker_failure(error).is_some() {
        FailureKind::InternalFailure
    } else if is_deadline_elapsed(error) {
        FailureKind::Timeout
    } else {
        FailureKind::TransportLoss
    }
}

/// A name is resolved exactly once on the caller's held session. Selectors
/// that cannot name an alias retain the existing direct-ID read contract.
pub(super) fn resolve_on_session(
    client: &mut LocalClient,
    target: &orchbee_agent_observation_protocol::AgentSessionSelector,
    total: Instant,
    wake: Option<BorrowedFd<'_>>,
) -> Result<AgentSessionId, Failure> {
    use orchbee_agent_observation_protocol::{
        AgentName, AgentResolveOutcome, AgentSessionSelector,
    };
    match target {
        AgentSessionSelector::Session { session_id } => return Ok(session_id.clone()),
        AgentSessionSelector::Auto { target } if AgentName::parse(target.as_str()).is_err() => {
            return Ok(target.clone());
        }
        _ => {}
    }
    let request_id = obap_request_id()?;
    let answer = client.submit_agent_names_before_cancellable(
        AgentNamesRequest::Resolve {
            request_id,
            target: target.clone(),
        },
        total,
        wake,
    );
    if Instant::now() >= total {
        return Err(Failure::new(FailureKind::Timeout).failure_schema(FAILURE_SCHEMA));
    }
    let result = answer.map_err(exchange_failure)?;
    let source = json!({"kind":"agent-names", "requestId": result.request_id()});
    let AgentNamesResult::Resolve { result, .. } = result else {
        return Err(Failure::new(FailureKind::ProtocolInvalid).failure_schema(FAILURE_SCHEMA));
    };
    match result.outcome {
        AgentResolveOutcome::Resolved { session_id, .. } => Ok(session_id),
        AgentResolveOutcome::Rejected { reason } => {
            Err(crate::cli::agent_names::rejection(json!(reason), None)
                .source(source)
                .failure_schema(FAILURE_SCHEMA))
        }
        AgentResolveOutcome::Unavailable { .. } => {
            Err(Failure::new(FailureKind::ServiceUnavailable)
                .source(source)
                .failure_schema(FAILURE_SCHEMA))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchbee_agent_observation_protocol::{AgentName, AgentSessionId, AgentSessionSelector};

    #[test]
    fn lost_mutation_reply_is_unknown_while_a_pre_submission_failure_is_not() {
        let command = AgentNamesCommand::Assign {
            target: AgentSessionSelector::Session {
                session_id: AgentSessionId::parse("a").unwrap(),
            },
            name: AgentName::parse("alpha").unwrap(),
        };
        let execution = settle(
            &command,
            Attempt::Exchange(Err(AgentExchangeError::Failed {
                request_id: ObapRequestId::parse("11111111111111111111111111111111").unwrap(),
                reason: AgentExchangeFailure::ResponseDeadline(std::io::Error::from(
                    std::io::ErrorKind::TimedOut,
                )),
            })),
            Ok(Finish::Clear),
        );
        assert!(matches!(
            execution.result,
            Err(Failure {
                kind: FailureKind::OutcomeUnknown,
                ..
            })
        ));
        let execution = settle(
            &command,
            Attempt::Exchange(Err(AgentExchangeError::RequestNotSubmitted(
                std::io::Error::from(std::io::ErrorKind::TimedOut),
            ))),
            Ok(Finish::Clear),
        );
        assert!(matches!(
            execution.result,
            Err(Failure {
                kind: FailureKind::Timeout,
                ..
            })
        ));
    }
}
