use super::{
    ClientMessage, Completion, CreateAcceptedState, CreateFailureReason, CreateOutcome,
    CreateUnavailableReason, HostLaunchState, PendingOperation, ServerMessage, StartFailureReason,
    StartMode, StartOutcome, StartTransition, StartUnavailableReason, TerminalLaunchFailure,
    TerminalLaunchOutcome, TerminalLaunchRejection, TerminalPhase, TerminalStartFailure,
    TerminalStartOutcome, TerminalStartRejection, client_geometry, create_result, start_result,
    unknown_result,
};

pub(super) fn settle(
    pending: &PendingOperation,
    completion: Completion,
    previous: TerminalPhase,
) -> (ServerMessage, TerminalPhase) {
    match (&pending.request, completion) {
        (ClientMessage::TerminalCreate(request), Completion::Launch(result))
            if result.authorization() == pending.authorization() =>
        {
            let outcome = match result.outcome() {
                TerminalLaunchOutcome::HostAccepted(accepted) => {
                    let (state, phase) = match (request.start_mode(), accepted.state()) {
                        (StartMode::Deferred, HostLaunchState::AwaitingStart) => (
                            CreateAcceptedState::AwaitingStart,
                            TerminalPhase::AwaitingStart(accepted.identity()),
                        ),
                        (StartMode::Immediate { geometry }, HostLaunchState::Started(observed))
                            if client_geometry(observed) == Some(*geometry) =>
                        {
                            (
                                CreateAcceptedState::Started {
                                    geometry: *geometry,
                                },
                                TerminalPhase::Started {
                                    identity: accepted.identity(),
                                    geometry: *geometry,
                                },
                            )
                        }
                        // Only a matching launch mode, state and geometry can confirm terminal creation.
                        (
                            StartMode::Deferred | StartMode::Immediate { .. },
                            HostLaunchState::AwaitingStart | HostLaunchState::Started(_),
                        ) => {
                            return (
                                unknown_result(&pending.request),
                                TerminalPhase::Unknown(previous.identity()),
                            );
                        }
                    };
                    return (
                        create_result(request, CreateOutcome::Accepted { state }),
                        phase,
                    );
                }
                TerminalLaunchOutcome::OutcomeUnknown { .. } => {
                    return (
                        unknown_result(&pending.request),
                        TerminalPhase::Unknown(previous.identity()),
                    );
                }
                TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::CapacityExceeded) => {
                    CreateOutcome::Unavailable {
                        reason: CreateUnavailableReason::ServerBusy,
                    }
                }
                TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
                    ..
                })
                | TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostReportedFailure {
                    ..
                }) => CreateOutcome::Failed {
                    reason: CreateFailureReason::TerminalCreateFailed,
                },
                TerminalLaunchOutcome::Rejected(_)
                | TerminalLaunchOutcome::Failed(
                    TerminalLaunchFailure::HostUnavailableBeforeHandoff,
                ) => CreateOutcome::Unavailable {
                    reason: CreateUnavailableReason::TerminalRuntimeUnavailable,
                },
            };
            (
                create_result(request, outcome),
                TerminalPhase::Unavailable(previous.identity()),
            )
        }
        (ClientMessage::TerminalStart(request), Completion::Start(result))
            if result.authorization() == pending.authorization() =>
        {
            let TerminalPhase::Starting(identity) = previous else {
                return (
                    unknown_result(&pending.request),
                    TerminalPhase::Unknown(previous.identity()),
                );
            };
            if result.target() != identity {
                return (
                    unknown_result(&pending.request),
                    TerminalPhase::Unknown(previous.identity()),
                );
            }
            let outcome = match result.outcome() {
                TerminalStartOutcome::HostAccepted(accepted) => {
                    if accepted.identity() != identity
                        || client_geometry(accepted.geometry()) != Some(request.geometry())
                    {
                        return (
                            unknown_result(&pending.request),
                            TerminalPhase::Unknown(previous.identity()),
                        );
                    }
                    let transition = match accepted.transition() {
                        orchbee_capability_node_protocol::StartTransition::Started => {
                            StartTransition::Started
                        }
                        orchbee_capability_node_protocol::StartTransition::AlreadyStarted => {
                            StartTransition::AlreadyStarted
                        }
                    };
                    return (
                        start_result(
                            request,
                            StartOutcome::Accepted {
                                geometry: request.geometry(),
                                transition,
                            },
                        ),
                        TerminalPhase::Started {
                            identity,
                            geometry: request.geometry(),
                        },
                    );
                }
                TerminalStartOutcome::OutcomeUnknown => {
                    return (
                        unknown_result(&pending.request),
                        TerminalPhase::Unknown(previous.identity()),
                    );
                }
                TerminalStartOutcome::Failed(
                    TerminalStartFailure::HostUnavailableBeforeHandoff,
                ) => {
                    return (
                        start_result(
                            request,
                            StartOutcome::Unavailable {
                                reason: StartUnavailableReason::TerminalRuntimeUnavailable,
                            },
                        ),
                        TerminalPhase::AwaitingStart(identity),
                    );
                }
                TerminalStartOutcome::Rejected(TerminalStartRejection::CapacityExceeded) => {
                    StartOutcome::Unavailable {
                        reason: StartUnavailableReason::ServerBusy,
                    }
                }
                TerminalStartOutcome::Rejected(TerminalStartRejection::HostRejected { .. })
                | TerminalStartOutcome::Failed(TerminalStartFailure::HostReportedFailure {
                    ..
                }) => StartOutcome::Failed {
                    reason: StartFailureReason::TerminalStartFailed,
                },
                TerminalStartOutcome::Rejected(_) => StartOutcome::Unavailable {
                    reason: StartUnavailableReason::TerminalRuntimeUnavailable,
                },
            };
            (
                start_result(request, outcome),
                TerminalPhase::Unavailable(previous.identity()),
            )
        }
        (
            ClientMessage::TerminalCreate(request),
            completion @ (Completion::Unavailable | Completion::Busy),
        ) => (
            create_result(
                request,
                CreateOutcome::Unavailable {
                    reason: if matches!(completion, Completion::Busy) {
                        CreateUnavailableReason::ServerBusy
                    } else {
                        CreateUnavailableReason::TerminalRuntimeUnavailable
                    },
                },
            ),
            TerminalPhase::Unavailable(previous.identity()),
        ),
        (
            ClientMessage::TerminalStart(request),
            completion @ (Completion::Unavailable | Completion::Busy),
        ) => (
            start_result(
                request,
                StartOutcome::Unavailable {
                    reason: if matches!(completion, Completion::Busy) {
                        StartUnavailableReason::ServerBusy
                    } else {
                        StartUnavailableReason::TerminalRuntimeUnavailable
                    },
                },
            ),
            match previous {
                TerminalPhase::Starting(identity) => TerminalPhase::AwaitingStart(identity),
                _ => TerminalPhase::Unavailable(previous.identity()),
            },
        ),
        // Pending terminal operations contain only Create/Start; unrelated messages cannot confirm their phase or authority.
        _ => (
            unknown_result(&pending.request),
            TerminalPhase::Unknown(previous.identity()),
        ),
    }
}
