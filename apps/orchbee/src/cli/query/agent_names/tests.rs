use super::*;
use crate::invocation::AgentTarget;
use crate::local_client::agent::AgentExchangeFailure;
use orchbee_agent_observation_protocol::ObapRequestId;
use orchbee_agent_observation_protocol::{AgentName, AgentSessionId};

#[test]
fn lost_mutation_reply_is_unknown_while_a_pre_submission_failure_is_not() {
    let command = AgentNamesCommand::Assign {
        target: AgentTarget::Session {
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
