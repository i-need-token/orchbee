//! Startup failures retain their operation's typed evidence and mutation uncertainty.
use crate::agent_runtime::startup::{
    AdmissionError, binding::PrimaryError, observation, processes, send,
};
use crate::local_client::{
    OutcomeUnknownReason, SubmissionError,
    agent::{AgentExchangeError, AgentExchangeFailure},
    exchange_wake_fault, exchange_wake_ready,
};
use orchbee_agent_observation_protocol::{AgentLaunchRejectionReason, AgentWireError};
use std::{io, sync::Arc};

#[derive(Debug)]
pub(crate) enum Failure {
    Unsupported,
    Identity,
    Primary(PrimaryError),
    Admission(Box<AdmissionError>),
    Process(Box<processes::Error>),
    Observation(Box<observation::Error>),
    Send(Arc<send::Error>),
    Start(StartFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartFailure {
    Deadline,
    Cancelled,
    CommandTooLong,
    TerminalNotIdle,
    BindingChanged,
    RuntimeAmbiguous,
    ProviderMismatch,
    AgentExited,
    AgentBlocked,
    ReplyNotConfirmed,
    InvalidProtocol,
    RuntimeUnavailable,
    Rejected(AgentLaunchRejectionReason),
    Exchange(ExchangeFailure),
    InputNotAccepted(send::InputFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExchangeFailure {
    Timeout,
    Transport,
    Internal,
}

impl Failure {
    pub(crate) fn timed_out(&self) -> bool {
        match self {
            Self::Start(
                StartFailure::Deadline | StartFailure::Exchange(ExchangeFailure::Timeout),
            ) => true,
            Self::Admission(error) => matches!(error.as_ref(), AdmissionError::Timeout),
            Self::Process(error) => matches!(error.as_ref(), processes::Error::Timeout),
            Self::Observation(error) => match error.as_ref() {
                observation::Error::Timeout => true,
                observation::Error::Exchange(error) => observation_io(error).is_some_and(|error| {
                    !wake_marker(error) && error.kind() == io::ErrorKind::TimedOut
                }),
                _ => false,
            },
            _ => false,
        }
    }
    pub(crate) fn unknown(&self) -> bool {
        match self {
            Self::Start(
                StartFailure::ReplyNotConfirmed
                | StartFailure::InputNotAccepted(send::InputFailure::Unknown),
            ) => true,
            Self::Send(error) => {
                matches!(error.as_ref(), send::Error::Exchange(error) if matches!(&error.failure, SubmissionError::OutcomeUnknown { .. }))
            }
            _ => false,
        }
    }
    pub(crate) fn cancelled(&self) -> bool {
        matches!(self, Self::Start(StartFailure::Cancelled))
    }

    pub(super) fn transport_lost(&self) -> bool {
        match self {
            Self::Start(StartFailure::Exchange(ExchangeFailure::Transport)) => true,
            Self::Process(error) => matches!(
                error.as_ref(),
                processes::Error::Exchange(
                    SubmissionError::RequestNotSubmitted(_)
                        | SubmissionError::OutcomeUnknown {
                            reason: OutcomeUnknownReason::Write(_) | OutcomeUnknownReason::Read(_),
                            ..
                        }
                )
            ),
            Self::Observation(error) => match error.as_ref() {
                observation::Error::Exchange(error) => observation_io(error).is_some_and(|error| {
                    !wake_marker(error) && error.kind() != io::ErrorKind::TimedOut
                }),
                _ => false,
            },
            _ => false,
        }
    }
    pub(super) fn transient_sample(&self) -> bool {
        matches!(self, Self::Process(error) if matches!(error.as_ref(), processes::Error::Host(processes::HostFailure::Changed | processes::HostFailure::Unavailable)))
    }
}

fn observation_io(error: &AgentExchangeError) -> Option<&io::Error> {
    match error {
        AgentExchangeError::RequestNotSubmitted(error)
        | AgentExchangeError::Failed {
            reason:
                AgentExchangeFailure::Write(error)
                | AgentExchangeFailure::ResponseDeadline(error)
                | AgentExchangeFailure::Response(AgentWireError::Io(error)),
            ..
        } => Some(error),
        _ => None,
    }
}

fn wake_marker(error: &io::Error) -> bool {
    exchange_wake_ready(error) || exchange_wake_fault(error)
}
