//! Projects a retained Host terminal into a lifecycle snapshot and validates directed replies.

use orchbee_capability_node_protocol::{LifecycleTargetObservation, LifecycleTerminalPhase};
use orchbee_terminal_protocol::{
    ConnectionControl, DecodedEnvelope, LifecycleControl, ReadTerminalLifecycle,
    TerminalLifecycleSnapshot, TerminalLifecycleState, TerminalSummary, decode_connection_control,
    decode_lifecycle_control,
};

pub(crate) enum LookupFailure {
    Refused,
    IdentityConflict,
    InvalidResponse,
}

/// The caller validates the response envelope against the request before reading its payload.
pub(crate) fn decode(
    target: ReadTerminalLifecycle,
    envelope: &DecodedEnvelope<'_>,
) -> Result<Option<TerminalLifecycleSnapshot>, LookupFailure> {
    if let Ok(ConnectionControl::TypedFailure(failure)) =
        decode_connection_control(envelope.kind, envelope.role, envelope.payload)
    {
        return match (failure.code.as_str(), failure.retryable) {
            ("missing-resource", false) => Ok(None),
            ("identity-conflict", false) => Err(LookupFailure::IdentityConflict),
            _ => Err(LookupFailure::Refused),
        };
    }
    let Ok(LifecycleControl::TerminalLifecycleSnapshot(snapshot)) =
        decode_lifecycle_control(envelope.kind, envelope.role, envelope.payload)
    else {
        return Err(LookupFailure::InvalidResponse);
    };
    if Some(snapshot.identity.host_epoch) != envelope.host_epoch {
        return Err(LookupFailure::InvalidResponse);
    }
    if snapshot.identity.terminal_id != target.terminal_id
        || target
            .creation_id
            .is_some_and(|creation_id| snapshot.identity.creation_id != creation_id)
    {
        return Err(LookupFailure::IdentityConflict);
    }
    Ok(Some(snapshot))
}

pub(super) fn observation(snapshot: TerminalLifecycleSnapshot) -> LifecycleTargetObservation {
    use LifecycleTerminalPhase as Phase;
    use TerminalLifecycleState as State;

    let phase = match snapshot.state {
        State::AwaitingStart => Phase::AwaitingStart,
        State::Running {
            terminate_requested,
        } => Phase::Running {
            terminate_requested,
        },
        State::Finishing {
            terminate_requested,
        } => Phase::Finishing {
            terminate_requested,
        },
        State::ContainingFailure => Phase::ContainingFailure,
        State::Exited {
            terminate_requested,
            ..
        } => Phase::Exited {
            terminate_requested,
        },
        State::Failed { .. } => Phase::Failed,
    };
    LifecycleTargetObservation {
        revision: snapshot.revision,
        phase,
    }
}

pub(crate) fn from_summary(summary: &TerminalSummary) -> TerminalLifecycleSnapshot {
    use TerminalLifecycleState as State;

    let (core, state) = match summary {
        TerminalSummary::AwaitingStart { core } => (core, State::AwaitingStart),
        TerminalSummary::Running {
            core,
            terminate_requested,
            ..
        } => (
            core,
            State::Running {
                terminate_requested: *terminate_requested,
            },
        ),
        TerminalSummary::Finishing {
            core,
            terminate_requested,
            ..
        } => (
            core,
            State::Finishing {
                terminate_requested: *terminate_requested,
            },
        ),
        TerminalSummary::ContainingFailure { core, .. } => (core, State::ContainingFailure),
        TerminalSummary::Exited {
            core,
            terminate_requested,
            exit,
            ..
        } => (
            core,
            State::Exited {
                terminate_requested: *terminate_requested,
                exit: *exit,
            },
        ),
        TerminalSummary::Failed {
            core,
            failure,
            process,
            ..
        } => (
            core,
            State::Failed {
                failure: *failure,
                process: *process,
            },
        ),
    };
    TerminalLifecycleSnapshot {
        identity: core.identity,
        revision: core.revision,
        state,
    }
}
