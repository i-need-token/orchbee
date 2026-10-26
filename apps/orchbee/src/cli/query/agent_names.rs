//! Names use one admitted session and one non-replayed OBAP exchange. A
//! received result remains authoritative even if cancellation races it;
//! an ambiguous mutation is always outcome-unknown, including on Ctrl-C.

use super::agent_names_failure::exchange_failure;
use super::{
    AGENT_TOTAL_BUDGET, AgentAdmissionError, AgentExchangeError, ArmedSignals, BorrowedFd, Failure,
    FailureKind, Finish, Instant, SignalWatch, agent_session_until, classify_connect,
    deadline_after, obap_request_id,
};
use crate::cli::agent_names::{Execution, FAILURE_SCHEMA};
use crate::invocation::AgentNamesCommand;
use orchbee_agent_observation_protocol::{AgentNamesRequest, AgentNamesResult};
use serde_json::{Map, json};

pub(super) fn execute(command: &AgentNamesCommand) -> Execution {
    let total = match deadline_after(AGENT_TOTAL_BUDGET) {
        Ok(total) => total,
        Err(failure) => return Execution::failed(failure),
    };
    let armed = match ArmedSignals::arm_interrupts() {
        Ok(armed) => armed,
        Err(_) => return Execution::failed(Failure::new(FailureKind::InternalFailure)),
    };
    let attempt = span(total, SignalWatch::poll_fd(&armed), command);
    let finish = armed.finish();
    settle(command, attempt, finish)
}

enum Attempt {
    Admission(AgentAdmissionError),
    Preparation(Failure),
    Exchange(Result<AgentNamesResult, AgentExchangeError>),
}

fn span(total: Instant, wake: Option<BorrowedFd<'_>>, command: &AgentNamesCommand) -> Attempt {
    let mut client = match agent_session_until(total, wake) {
        Err(error) => return Attempt::Admission(error),
        Ok(client) => client,
    };
    let prepared = match command {
        AgentNamesCommand::List => None,
        AgentNamesCommand::Resolve(target)
        | AgentNamesCommand::Release(target)
        | AgentNamesCommand::Assign { target, .. } => {
            match super::agent_target::prepare(&mut client, target, None, total, wake) {
                Ok(prepared) => Some(prepared),
                Err(failure) => return Attempt::Preparation(failure),
            }
        }
    };
    if matches!(command, AgentNamesCommand::Resolve(_))
        && let Some(answer) = prepared
            .as_ref()
            .and_then(|prepared| prepared.resolved.as_ref())
    {
        return Attempt::Exchange(Ok(answer.clone()));
    }
    let request_id = match obap_request_id() {
        Ok(id) => id,
        Err(failure) => return Attempt::Preparation(failure),
    };
    let request = match (command, prepared) {
        (AgentNamesCommand::List, _) => AgentNamesRequest::NamesList { request_id },
        (AgentNamesCommand::Resolve(_), Some(prepared)) => AgentNamesRequest::Resolve {
            request_id,
            target: prepared.selector,
        },
        (AgentNamesCommand::Assign { name, .. }, Some(prepared)) => AgentNamesRequest::NameAssign {
            request_id,
            target: prepared.selector,
            name: name.clone(),
        },
        (AgentNamesCommand::Release(_), Some(prepared)) => AgentNamesRequest::NameRelease {
            request_id,
            target: prepared.selector,
        },
        _ => return Attempt::Preparation(Failure::new(FailureKind::InternalFailure)),
    };
    let result = client.submit_agent_names_before_cancellable(request, total, wake);
    drop(client);
    Attempt::Exchange(result)
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
        Attempt::Admission(AgentAdmissionError::Preamble(failure))
        | Attempt::Preparation(failure) => Execution::failed(failure),
        Attempt::Admission(AgentAdmissionError::Connect(error)) => {
            Execution::failed(classify_connect(error))
        }
        Attempt::Exchange(Err(error)) => Execution::failed(exchange_failure(error)),
    };
    execution.finalization = finalization;
    execution
}

#[cfg(test)]
mod tests;
