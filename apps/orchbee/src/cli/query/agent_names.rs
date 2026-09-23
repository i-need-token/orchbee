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
        Attempt::Exchange(Err(error)) => {
            let kind = match error {
                AgentExchangeError::SessionUnavailable | AgentExchangeError::RequestEncoding(_) => {
                    FailureKind::InternalFailure
                }
                AgentExchangeError::RequestNotSubmitted(error)
                    if error.kind() == std::io::ErrorKind::TimedOut =>
                {
                    FailureKind::Timeout
                }
                AgentExchangeError::RequestNotSubmitted(_) => FailureKind::TransportLoss,
                AgentExchangeError::Failed { reason, .. } => match reason {
                    AgentExchangeFailure::ResponseDeadline(_) => FailureKind::Timeout,
                    AgentExchangeFailure::Write(_)
                    | AgentExchangeFailure::Response(
                        orchbee_agent_observation_protocol::AgentWireError::Io(_),
                    ) => FailureKind::TransportLoss,
                    _ => FailureKind::ProtocolInvalid,
                },
            };
            Execution::failed(Failure::new(kind).failure_schema(FAILURE_SCHEMA))
        }
    };
    execution.finalization = finalization;
    execution
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
