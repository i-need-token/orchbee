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

/// Retain the failing operation's typed evidence without assigning a CLI status or document.
/// Reservation and input-delivery facts remain in the accompanying startup Report.
#[derive(Debug)]
pub enum Failure {
    Unsupported,
    Identity,
    Primary(PrimaryError),
    Admission(Box<AdmissionError>),
    Process(Box<processes::Error>),
    Observation(Box<observation::Error>),
    Send(Arc<send::Error>),
    Start(StartFailure),
}

/// A startup workflow decision after observing its deadline, binding, reservation or input result.
/// These decisions do not replace independently known delivery or reservation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartFailure {
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

/// Classify a launch exchange failure after reservation uncertainty has been recorded separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExchangeFailure {
    Timeout,
    Transport,
    Internal,
}

impl Failure {
    /// Identify a deadline failure for presentation alongside the separately retained delivery fact.
    /// A cancellation wake marker is never interpreted as an observation timeout.
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
    /// Identify explicit unconfirmed replies or unknown submissions in this failure.
    /// Callers must also inspect reservation and delivery state before deciding retry safety.
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
    /// Identify the workflow's settled cancellation decision, without erasing prior input effects.
    pub(crate) fn cancelled(&self) -> bool {
        matches!(self, Self::Start(StartFailure::Cancelled))
    }

    /// Select transport failures eligible for the workflow's existing recovery classification.
    /// Observation wake markers and response deadlines keep their separate meanings.
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
    /// Permit another read only for the Host's changed or temporarily unavailable process sample.
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
