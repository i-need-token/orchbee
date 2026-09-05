use super::*;
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostLaunchAccepted, HostTerminalId, TerminalLaunchResult,
};

fn id(value: u128) -> RequestId {
    RequestId::parse(&format!("{value:032x}")).unwrap()
}
fn worker(generation: u64) -> WorkerPlacement {
    WorkerPlacement {
        worker_id: WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
    }
}
fn expiry() -> LeaseExpiresAtUnixMilliseconds {
    LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(10_000).unwrap())
}
fn geometry() -> Geometry {
    Geometry::new(80, 24, 8, 16).unwrap()
}
fn create(value: u128) -> ClientMessage {
    ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        id(value),
        None,
        None,
        StartMode::Deferred,
    ))
}
fn start(value: u128, reference: u128) -> ClientMessage {
    ClientMessage::TerminalStart(
        TerminalStartRequest::new(id(value), id(reference), geometry()).unwrap(),
    )
}
fn dispatch(prepared: PreparedOperation) -> PendingOperation {
    match prepared {
        PreparedOperation::Dispatch(pending) => *pending,
        other => panic!("expected dispatch: {other:?}"),
    }
}
fn identity() -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([3; 16]).unwrap(),
        HostEpoch::from_bytes([4; 16]).unwrap(),
        HostTerminalId::from_bytes([5; 16]).unwrap(),
    )
}
fn accepted_launch(pending: &PendingOperation) -> Completion {
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(request) = &pending.command else {
        panic!("launch expected")
    };
    Completion::Launch(
        TerminalLaunchResult::new(
            request.authorization(),
            TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                identity(),
                HostLaunchState::AwaitingStart,
            )),
        )
        .unwrap(),
    )
}

#[test]
fn unavailable_worker_and_constraint_fail_without_allocating_authority() {
    let mut coordinator = TerminalCoordinator::new();
    assert!(
        matches!(coordinator.prepare(create(1),None,Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalCreateResult(result)) if result.outcome()==CreateOutcome::Unavailable { reason: CreateUnavailableReason::NoCompatibleWorker })
    );
    let constrained = ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        id(2),
        Some(WorkerConstraint::Exact {
            worker_id: orchbee_client_server_protocol::WorkerId::parse(&"07".repeat(16)).unwrap(),
        }),
        None,
        StartMode::Deferred,
    ));
    assert!(
        matches!(coordinator.prepare(constrained,Some(worker(1)),Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalCreateResult(result)) if result.outcome()==CreateOutcome::Rejected {reason: CreateRejectedReason::WorkerConstraintUnsatisfied})
    );
    assert!(coordinator.bindings.is_empty());
    assert_eq!(coordinator.next_identity, Some(1));
}

#[test]
fn deferred_create_start_commits_exact_identity_with_fresh_operation_and_fence() {
    let mut coordinator = TerminalCoordinator::new();
    let launch = dispatch(coordinator.prepare(create(1), Some(worker(1)), Some(expiry())));
    let observation = accepted_launch(&launch);
    let launch_authorization = launch.authorization();
    assert!(
        matches!(coordinator.complete(&launch,observation,Some(worker(1))),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::Accepted {state: CreateAcceptedState::AwaitingStart})
    );
    let pending = dispatch(coordinator.prepare(start(2, 1), Some(worker(1)), Some(expiry())));
    let CapabilityNodeMessage::AuthorizedTerminalStart(command) = pending.command else {
        panic!("start expected")
    };
    assert_eq!(command.identity(), identity());
    assert_eq!(
        command.authorization().terminal_id(),
        launch_authorization.terminal_id()
    );
    assert_ne!(
        command.authorization().operation_id(),
        launch_authorization.operation_id()
    );
    assert_ne!(
        command.authorization().lease().id(),
        launch_authorization.lease().id()
    );
    assert_eq!(command.authorization().fence().get(), 2);
    assert!(
        matches!(coordinator.prepare(start(3,1),Some(worker(1)),Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Unavailable {reason:StartUnavailableReason::ServerBusy})
    );
    let result = orchbee_capability_node_protocol::TerminalStartResult::new(
        command.authorization(),
        identity(),
        TerminalStartOutcome::HostAccepted(
            orchbee_capability_node_protocol::HostStartAccepted::new(
                identity(),
                command.geometry(),
                orchbee_capability_node_protocol::StartTransition::Started,
            ),
        ),
    )
    .unwrap();
    coordinator.complete(&pending, Completion::Start(result), Some(worker(1)));
    assert!(
        matches!(coordinator.prepare(start(4,1),Some(worker(1)),Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Accepted {geometry:geometry(),transition:StartTransition::AlreadyStarted})
    );
    let conflicting = ClientMessage::TerminalStart(
        TerminalStartRequest::new(id(5), id(1), Geometry::new(100, 30, 8, 16).unwrap()).unwrap(),
    );
    assert!(
        matches!(coordinator.prepare(conflicting,Some(worker(1)),Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Rejected {reason:StartRejectedReason::GeometryConflict})
    );
}

#[test]
fn unknown_create_and_stale_worker_results_cannot_authorize_start_or_replay() {
    for stale in [false, true] {
        let mut coordinator = TerminalCoordinator::new();
        let pending = dispatch(coordinator.prepare(create(1), Some(worker(1)), Some(expiry())));
        let outcome = if stale {
            accepted_launch(&pending)
        } else {
            Completion::Unknown
        };
        let placement = if stale { worker(2) } else { worker(1) };
        assert!(
            matches!(coordinator.complete(&pending,outcome,Some(placement)),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::OutcomeUnknown)
        );
        assert!(
            matches!(coordinator.prepare(start(2,1),Some(placement),Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Rejected {reason:StartRejectedReason::TerminalCreateUnavailable})
        );
        assert!(
            matches!(coordinator.prepare(create(1),Some(placement),Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalCreateResult(result)) if result.outcome()==CreateOutcome::Rejected {reason:CreateRejectedReason::NotAuthorized})
        );
        let another = dispatch(coordinator.prepare(create(3), Some(placement), Some(expiry())));
        assert_ne!(
            another.authorization().operation_id(),
            pending.authorization().operation_id()
        );
    }
}

#[test]
fn known_pre_delivery_start_failure_can_be_followed_by_a_new_intent_but_unknown_cannot() {
    for completion in [
        Completion::Unavailable,
        Completion::Busy,
        Completion::Unknown,
    ] {
        let unknown = matches!(completion, Completion::Unknown);
        let mut coordinator = TerminalCoordinator::new();
        let launch = dispatch(coordinator.prepare(create(1), Some(worker(1)), Some(expiry())));
        coordinator.complete(&launch, accepted_launch(&launch), Some(worker(1)));
        let pending = dispatch(coordinator.prepare(start(2, 1), Some(worker(1)), Some(expiry())));
        coordinator.complete(&pending, completion, Some(worker(1)));
        assert_eq!(
            coordinator.bindings.get(&id(1)).unwrap().phase.identity(),
            Some(identity())
        );
        let next = coordinator.prepare(start(3, 1), Some(worker(1)), Some(expiry()));
        assert_eq!(matches!(next, PreparedOperation::Dispatch(_)), !unknown);
    }
}

#[test]
fn bounds_retained_records_and_exhaustion_never_wraps_identity() {
    let mut coordinator = TerminalCoordinator::new();
    for number in 1..=MAXIMUM_TERMINAL_BINDINGS {
        dispatch(coordinator.prepare(create(number as u128), Some(worker(1)), Some(expiry())));
    }
    assert!(
        matches!(coordinator.prepare(create(2000),Some(worker(1)),Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalCreateResult(result)) if result.outcome()==CreateOutcome::Unavailable {reason:CreateUnavailableReason::ServerBusy})
    );
    let mut exhausted = TerminalCoordinator::new();
    exhausted.next_identity = None;
    assert!(matches!(
        exhausted.prepare(create(1), Some(worker(1)), Some(expiry())),
        PreparedOperation::Ready(_)
    ));
    assert!(exhausted.bindings.is_empty());
}

#[test]
fn immediate_launch_preserves_geometry_and_rejects_a_later_start() {
    let mut coordinator = TerminalCoordinator::new();
    let request = ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        id(1),
        None,
        Some(
            orchbee_client_server_protocol::WorkingDirectory::parse("/tmp/literal/../directory")
                .unwrap(),
        ),
        StartMode::Immediate {
            geometry: geometry(),
        },
    ));
    let pending = dispatch(coordinator.prepare(request, Some(worker(1)), Some(expiry())));
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(command) = &pending.command else {
        panic!("launch expected")
    };
    assert_eq!(command.cwd().unwrap().as_str(), "/tmp/literal/../directory");
    assert_eq!(
        command.mode(),
        TerminalLaunchMode::Immediate(worker_geometry(geometry()).unwrap())
    );
    let result = TerminalLaunchResult::new(
        command.authorization(),
        TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
            identity(),
            HostLaunchState::Started(worker_geometry(geometry()).unwrap()),
        )),
    )
    .unwrap();
    assert!(
        matches!(coordinator.complete(&pending,Completion::Launch(result),Some(worker(1))),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::Accepted {state:CreateAcceptedState::Started {geometry:geometry()}})
    );
    assert!(
        matches!(coordinator.prepare(start(2,1),Some(worker(1)),Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Rejected {reason:StartRejectedReason::TerminalNotAwaitingStart})
    );
}

#[test]
fn contradictory_launch_mode_and_start_geometry_freeze_the_binding() {
    let mut coordinator = TerminalCoordinator::new();
    let pending = dispatch(coordinator.prepare(create(1), Some(worker(1)), Some(expiry())));
    let contradictory = TerminalLaunchResult::new(
        pending.authorization(),
        TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
            identity(),
            HostLaunchState::Started(worker_geometry(geometry()).unwrap()),
        )),
    )
    .unwrap();
    assert!(
        matches!(coordinator.complete(&pending,Completion::Launch(contradictory),Some(worker(1))),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::OutcomeUnknown)
    );
    let launch = dispatch(coordinator.prepare(create(2), Some(worker(1)), Some(expiry())));
    coordinator.complete(&launch, accepted_launch(&launch), Some(worker(1)));
    let pending = dispatch(coordinator.prepare(start(3, 2), Some(worker(1)), Some(expiry())));
    let result = orchbee_capability_node_protocol::TerminalStartResult::new(
        pending.authorization(),
        identity(),
        TerminalStartOutcome::HostAccepted(
            orchbee_capability_node_protocol::HostStartAccepted::new(
                identity(),
                TerminalPixelGeometry::new(90, 24, 8, 16).unwrap(),
                orchbee_capability_node_protocol::StartTransition::Started,
            ),
        ),
    )
    .unwrap();
    assert!(
        matches!(coordinator.complete(&pending,Completion::Start(result),Some(worker(1))),ServerMessage::TerminalStartResult(result) if result.outcome()==StartOutcome::OutcomeUnknown)
    );
    assert!(
        matches!(coordinator.prepare(start(4,2),Some(worker(1)),Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Rejected {reason:StartRejectedReason::TerminalCreateUnavailable})
    );
}

#[test]
fn worker_replacement_and_late_completion_cannot_change_a_current_binding() {
    let mut coordinator = TerminalCoordinator::new();
    let launch = dispatch(coordinator.prepare(create(1), Some(worker(1)), Some(expiry())));
    coordinator.complete(&launch, accepted_launch(&launch), Some(worker(1)));
    assert!(
        matches!(coordinator.prepare(start(2,1),Some(worker(2)),Some(expiry())),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Unavailable {reason:StartUnavailableReason::TerminalRuntimeUnavailable})
    );
    let pending = dispatch(coordinator.prepare(start(3, 1), Some(worker(1)), Some(expiry())));
    coordinator.complete(&launch, Completion::Unknown, Some(worker(1)));
    assert!(matches!(
        coordinator.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::Starting(_)
    ));
    assert_eq!(
        coordinator.bindings.get(&id(1)).unwrap().authorization,
        pending.authorization()
    );
}

#[test]
fn worker_launch_outcomes_keep_rejection_failure_and_unknown_distinct() {
    let epoch = identity().host_epoch();
    for (observed, expected) in [
        (
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::CapacityExceeded),
            CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            },
        ),
        (
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::LeaseExpired),
            CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::TerminalRuntimeUnavailable,
            },
        ),
        (
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
                host_epoch: epoch,
            }),
            CreateOutcome::Failed {
                reason: CreateFailureReason::TerminalCreateFailed,
            },
        ),
        (
            TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostReportedFailure {
                host_epoch: epoch,
            }),
            CreateOutcome::Failed {
                reason: CreateFailureReason::TerminalCreateFailed,
            },
        ),
        (
            TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff),
            CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::TerminalRuntimeUnavailable,
            },
        ),
        (
            TerminalLaunchOutcome::OutcomeUnknown { host_epoch: epoch },
            CreateOutcome::OutcomeUnknown,
        ),
    ] {
        let mut coordinator = TerminalCoordinator::new();
        let pending = dispatch(coordinator.prepare(create(1), Some(worker(1)), Some(expiry())));
        let result = TerminalLaunchResult::new(pending.authorization(), observed).unwrap();
        assert!(
            matches!(coordinator.complete(&pending,Completion::Launch(result),Some(worker(1))),ServerMessage::TerminalCreateResult(result) if result.outcome()==expected)
        );
    }
}
