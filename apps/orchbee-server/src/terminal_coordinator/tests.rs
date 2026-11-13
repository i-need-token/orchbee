use super::*;
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostLaunchAccepted, HostTerminalId, TerminalLaunchResult,
};

#[test]
fn close_fence_refuses_only_new_start_effects_without_consuming_the_cursor() {
    let mut coordinator = TerminalCoordinator::new();
    let launch = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let accepted = accepted_launch(&launch);
    commit_completion(&mut coordinator, launch, accepted, Some(worker(1)));
    let _other = dispatch(coordinator.prepare(
        false,
        create(9),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let cursor = coordinator.next_identity;
    assert!(matches!(
        coordinator.prepare(true, start(2, 1), Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),
        PreparedOperation::Ready(ServerMessage::TerminalStartResult(result))
            if result.outcome() == StartOutcome::Rejected { reason: StartRejectedReason::TerminalCreateUnavailable }
    ));
    assert_eq!(coordinator.next_identity, cursor);
    assert_eq!(
        coordinator.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::AwaitingStart(identity())
    );
    assert!(matches!(
        coordinator.prepare(true, start(9, 1), Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),
        PreparedOperation::Ready(ServerMessage::TerminalStartResult(result))
            if result.outcome() == StartOutcome::Rejected { reason: StartRejectedReason::NotAuthorized }
    ));
    coordinator.bindings.get_mut(&id(1)).unwrap().phase = TerminalPhase::Started {
        identity: identity(),
        geometry: geometry(),
    };
    assert!(matches!(
        coordinator.prepare(true, start(3, 1), Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),
        PreparedOperation::Ready(ServerMessage::TerminalStartResult(result))
            if result.outcome() == StartOutcome::Accepted { geometry: geometry(), transition: StartTransition::AlreadyStarted }
    ));
    let conflict = ClientMessage::TerminalStart(
        TerminalStartRequest::new(id(4), id(1), Geometry::new(90, 30, 8, 16).unwrap()).unwrap(),
    );
    assert!(matches!(
        coordinator.prepare(true, conflict, Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),
        PreparedOperation::Ready(ServerMessage::TerminalStartResult(result))
            if result.outcome() == StartOutcome::Rejected { reason: StartRejectedReason::GeometryConflict }
    ));
    assert_eq!(coordinator.next_identity, cursor);
}

#[test]
fn close_runtime_requires_one_settled_exact_generation_and_retained_identity() {
    let mut coordinator = TerminalCoordinator::new();
    let launch = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let authorization = launch.authorization();
    assert_eq!(
        coordinator.retained_runtime_target(
            authorization.terminal_id(),
            authorization.worker_id(),
            None
        ),
        Err(RetainedRuntimeTargetError::Busy)
    );
    let accepted = accepted_launch(&launch);
    commit_completion(&mut coordinator, launch, accepted, Some(worker(1)));
    let expected = coordinator
        .retained_runtime_target(
            authorization.terminal_id(),
            authorization.worker_id(),
            Some(worker(1)),
        )
        .unwrap();
    assert_eq!(expected.0.identity, identity());
    assert_eq!(expected.0.terminal_id, authorization.terminal_id());
    assert_eq!(expected.1, authorization.fence());
    assert_eq!(
        coordinator.retained_runtime_target(
            authorization.terminal_id(),
            authorization.worker_id(),
            Some(worker(2))
        ),
        Err(RetainedRuntimeTargetError::Unavailable)
    );
    for phase in [
        TerminalPhase::Starting(identity()),
        TerminalPhase::Unknown(Some(identity())),
        TerminalPhase::Unavailable(None),
    ] {
        coordinator.bindings.get_mut(&id(1)).unwrap().phase = phase;
        assert_eq!(
            coordinator.retained_runtime_target(
                authorization.terminal_id(),
                authorization.worker_id(),
                Some(worker(1))
            ),
            Err(if matches!(phase, TerminalPhase::Starting(_)) {
                RetainedRuntimeTargetError::Busy
            } else {
                RetainedRuntimeTargetError::Unavailable
            })
        );
    }
    coordinator.bindings.get_mut(&id(1)).unwrap().phase = TerminalPhase::Started {
        identity: identity(),
        geometry: geometry(),
    };
    let other = dispatch(coordinator.prepare(
        false,
        create(2),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    coordinator.bindings.get_mut(&id(2)).unwrap().authorization = authorization;
    assert_eq!(
        coordinator.retained_runtime_target(
            authorization.terminal_id(),
            authorization.worker_id(),
            Some(worker(1))
        ),
        Err(RetainedRuntimeTargetError::Unavailable)
    );
    drop(other);
}

fn id(value: u128) -> RequestId {
    RequestId::parse(&format!("{value:032x}")).unwrap()
}
fn worker(generation: u64) -> WorkerPlacement {
    WorkerPlacement {
        worker_id: WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
    }
}
fn other_worker(generation: u64) -> WorkerPlacement {
    WorkerPlacement {
        worker_id: WorkerId::from_bytes([8; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
    }
}
fn expiry() -> LeaseExpiresAtUnixMilliseconds {
    LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(10_000).unwrap())
}
fn geometry() -> Geometry {
    Geometry::new(80, 24, 8, 16).unwrap()
}
fn create_request(value: u128) -> TerminalCreateRequest {
    TerminalCreateRequest::new(id(value), None, None, StartMode::Deferred, None)
}
fn create(value: u128) -> ClientMessage {
    ClientMessage::TerminalCreate(create_request(value))
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
fn duplicate_pending(pending: &PendingOperation) -> PendingOperation {
    PendingOperation {
        command: pending.command.clone(),
        request: pending.request.clone(),
        create_id: pending.create_id.clone(),
        completion_seal: Arc::clone(&pending.completion_seal),
    }
}
fn commit_completion(
    coordinator: &mut TerminalCoordinator,
    pending: PendingOperation,
    completion: Completion,
    current_worker: Option<WorkerPlacement>,
) -> ServerMessage {
    coordinator
        .prepare_completion(Box::new(pending), completion, current_worker)
        .unwrap()
        .commit()
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

fn create_refusal(result: Result<PlannedCreate<'_>, ServerMessage>) -> CreateOutcome {
    match result {
        Err(ServerMessage::TerminalCreateResult(result)) => result.outcome(),
        other => panic!("expected create refusal: {other:?}"),
    }
}

#[test]
fn abandoned_create_plan_leaves_the_original_identity_available() {
    let mut coordinator = TerminalCoordinator::new();
    let plan = coordinator
        .plan_create(
            create_request(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        )
        .unwrap();
    let authorization = plan.authorization();
    assert_eq!(plan.create_reference(), &id(1));
    assert!(matches!(
        plan.command(),
        CapabilityNodeMessage::AuthorizedTerminalLaunch(_)
    ));

    drop(plan);
    assert!(coordinator.bindings.is_empty());
    assert_eq!(coordinator.next_identity, Some(1));

    let pending = dispatch(coordinator.prepare(
        false,
        create(2),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    assert_eq!(pending.authorization(), authorization);
    assert_eq!(pending.create_reference(), &id(2));
    assert!(!coordinator.bindings.contains_key(&id(1)));
    assert!(coordinator.bindings.contains_key(&id(2)));
    assert_eq!(coordinator.next_identity, Some(2));
}

#[test]
fn create_plan_commit_installs_and_dispatches_the_complete_plan() {
    let mut coordinator = TerminalCoordinator::new();
    let request = TerminalCreateRequest::new(
        id(7),
        None,
        Some(
            orchbee_client_server_protocol::WorkingDirectory::parse("/tmp/literal/../directory")
                .unwrap(),
        ),
        StartMode::Immediate {
            geometry: geometry(),
        },
        None,
    );
    let reference = request.request_id().clone();
    let expected_request = request.clone();
    let plan = coordinator
        .plan_create(
            request,
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        )
        .unwrap();
    let authorization = plan.authorization();

    let pending = plan.commit();
    assert_eq!(pending.create_reference(), &reference);
    assert_eq!(
        pending.command,
        CapabilityNodeMessage::AuthorizedTerminalLaunch(AuthorizedTerminalLaunch::new(
            authorization,
            Some(AbsoluteCwd::new("/tmp/literal/../directory").unwrap()),
            TerminalLaunchMode::Immediate(worker_geometry(geometry()).unwrap()),
            None
        ))
    );
    assert_eq!(
        pending.request,
        ClientMessage::TerminalCreate(expected_request)
    );
    let binding = coordinator.bindings.get(&reference).unwrap();
    assert_eq!(binding.authorization, authorization);
    assert!(Arc::ptr_eq(
        &binding.completion_seal,
        &pending.completion_seal,
    ));
    assert!(!binding.deferred);
    assert!(matches!(binding.phase, TerminalPhase::Creating));
    assert_eq!(coordinator.next_identity, Some(2));
}

#[test]
fn create_plan_refusals_leave_capacity_identity_lease_and_constraint_state_unchanged() {
    let mut capacity = TerminalCoordinator::new();
    for number in 1..=MAXIMUM_TERMINAL_BINDINGS {
        dispatch(capacity.prepare(
            false,
            create(number as u128),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
    }
    let capacity_cursor = capacity.next_identity;
    assert_eq!(
        create_refusal(capacity.plan_create(
            create_request(2_000),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        )),
        CreateOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy,
        }
    );
    assert_eq!(capacity.bindings.len(), MAXIMUM_TERMINAL_BINDINGS);
    assert_eq!(capacity.next_identity, capacity_cursor);

    let mut exhausted = TerminalCoordinator::new();
    exhausted.next_identity = None;
    assert_eq!(
        create_refusal(exhausted.plan_create(
            create_request(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent()
        )),
        CreateOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy,
        }
    );
    assert!(exhausted.bindings.is_empty());
    assert_eq!(exhausted.next_identity, None);

    let mut zero = TerminalCoordinator::new();
    zero.next_identity = Some(0);
    assert_eq!(
        create_refusal(zero.plan_create(
            create_request(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent()
        )),
        CreateOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy,
        }
    );
    assert!(zero.bindings.is_empty());
    assert_eq!(zero.next_identity, Some(0));

    let mut no_lease = TerminalCoordinator::new();
    assert_eq!(
        create_refusal(no_lease.plan_create(
            create_request(1),
            Some(worker(1)),
            None,
            &LaunchCapture::absent()
        )),
        CreateOutcome::Unavailable {
            reason: CreateUnavailableReason::ServerBusy,
        }
    );
    assert!(no_lease.bindings.is_empty());
    assert_eq!(no_lease.next_identity, Some(1));

    let mut constrained = TerminalCoordinator::new();
    let request = TerminalCreateRequest::new(
        id(1),
        Some(WorkerConstraint::Exact {
            worker_id: orchbee_client_server_protocol::WorkerId::parse(&"07".repeat(16)).unwrap(),
        }),
        None,
        StartMode::Deferred,
        None,
    );
    assert_eq!(
        create_refusal(constrained.plan_create(
            request,
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent()
        )),
        CreateOutcome::Rejected {
            reason: CreateRejectedReason::WorkerConstraintUnsatisfied,
        }
    );
    assert!(constrained.bindings.is_empty());
    assert_eq!(constrained.next_identity, Some(1));
}

#[test]
fn maximum_identity_plan_is_pure_until_commit_and_exhausts_only_on_commit() {
    let mut coordinator = TerminalCoordinator::new();
    coordinator.next_identity = Some(u128::MAX);
    let plan = coordinator
        .plan_create(
            create_request(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        )
        .unwrap();
    let authorization = plan.authorization();

    assert_eq!(
        authorization.terminal_id().as_bytes(),
        u128::MAX.to_be_bytes()
    );
    drop(plan);
    assert!(coordinator.bindings.is_empty());
    assert_eq!(coordinator.next_identity, Some(u128::MAX));

    let plan = coordinator
        .plan_create(
            create_request(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        )
        .unwrap();
    assert_eq!(plan.authorization(), authorization);
    let pending = plan.commit();
    assert_eq!(
        pending.authorization().terminal_id().as_bytes(),
        u128::MAX.to_be_bytes()
    );
    assert!(coordinator.bindings.contains_key(&id(1)));
    assert_eq!(coordinator.next_identity, None);
}

#[test]
fn equal_state_foreign_owner_and_dropped_completion_plan_leave_records_unchanged() {
    let mut owner_a = TerminalCoordinator::new();
    let pending_a = dispatch(owner_a.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let mut owner_b = TerminalCoordinator::new();
    let pending_b = dispatch(owner_b.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));

    assert_eq!(pending_a.authorization(), pending_b.authorization());
    assert_eq!(owner_a.next_identity, owner_b.next_identity);
    assert!(!Arc::ptr_eq(
        &pending_a.completion_seal,
        &pending_b.completion_seal,
    ));
    assert_eq!(
        owner_b
            .prepare_completion(Box::new(pending_a), Completion::Unknown, Some(worker(1)),)
            .unwrap_err(),
        TerminalCompletionTokenMismatch
    );
    assert!(matches!(
        owner_a.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::Creating
    ));
    assert!(matches!(
        owner_b.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::Creating
    ));

    let stale_pending_b = duplicate_pending(&pending_b);
    let prepared = owner_b
        .prepare_completion(
            Box::new(duplicate_pending(&pending_b)),
            Completion::Unavailable,
            Some(worker(1)),
        )
        .unwrap();
    assert!(
        matches!(prepared.result(),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::Unavailable {reason:CreateUnavailableReason::TerminalRuntimeUnavailable})
    );
    assert_eq!(prepared.binding.phase, TerminalPhase::Creating);
    drop(prepared);
    assert_eq!(owner_b.bindings.get(&id(1)).unwrap().create_outcome, None);
    assert!(matches!(
        owner_b.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::Creating
    ));

    assert!(
        matches!(commit_completion(&mut owner_b,pending_b,Completion::Unavailable,Some(worker(1))),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::Unavailable {reason:CreateUnavailableReason::TerminalRuntimeUnavailable})
    );
    assert!(matches!(
        owner_b.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::Unavailable(None)
    ));
    assert_eq!(
        owner_b
            .prepare_completion(
                Box::new(stale_pending_b),
                Completion::Unknown,
                Some(worker(1)),
            )
            .unwrap_err(),
        TerminalCompletionTokenMismatch
    );
    assert!(matches!(
        owner_b.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::Unavailable(None)
    ));
}

#[test]
fn malformed_delivery_is_prepared_as_unknown_before_mutation() {
    let mut coordinator = TerminalCoordinator::new();
    let pending = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let malformed = TerminalLaunchResult::new(
        pending.authorization(),
        TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
            identity(),
            HostLaunchState::Started(worker_geometry(geometry()).unwrap()),
        )),
    )
    .unwrap();

    let prepared = coordinator
        .prepare_completion(
            Box::new(pending),
            Completion::Launch(malformed),
            Some(worker(1)),
        )
        .unwrap();
    assert!(
        matches!(prepared.result(),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::OutcomeUnknown)
    );
    assert_eq!(prepared.binding.phase, TerminalPhase::Creating);

    assert!(
        matches!(prepared.commit(),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::OutcomeUnknown)
    );
    assert!(matches!(
        coordinator.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::Unknown(None)
    ));
}

#[test]
fn pre_delivery_unavailability_and_post_response_replacement_remain_distinct() {
    let mut unavailable = TerminalCoordinator::new();
    let pending = dispatch(unavailable.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    assert!(
        matches!(commit_completion(&mut unavailable,pending,Completion::Unavailable,Some(worker(2))),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::Unavailable {reason:CreateUnavailableReason::TerminalRuntimeUnavailable})
    );
    assert!(matches!(
        unavailable.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::Unavailable(None)
    ));

    let mut replaced = TerminalCoordinator::new();
    let pending = dispatch(replaced.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let accepted = accepted_launch(&pending);
    assert!(
        matches!(commit_completion(&mut replaced,pending,accepted,Some(worker(2))),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::OutcomeUnknown)
    );
    assert!(matches!(
        replaced.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::Unknown(None)
    ));
}

#[test]
fn a_pre_handoff_launch_failure_stays_definite_after_its_worker_retires() {
    for current in [None, Some(worker(2))] {
        let mut coordinator = TerminalCoordinator::new();
        let pending = dispatch(coordinator.prepare(
            false,
            create(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
        let CapabilityNodeMessage::AuthorizedTerminalLaunch(request) = &pending.command else {
            panic!("launch expected")
        };
        let failed = Completion::Launch(
            TerminalLaunchResult::new(
                request.authorization(),
                TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff),
            )
            .unwrap(),
        );
        assert!(
            matches!(commit_completion(&mut coordinator,pending,failed,current),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::Unavailable {reason:CreateUnavailableReason::TerminalRuntimeUnavailable}),
            "{current:?}"
        );
    }
}

#[test]
fn retained_authorization_is_phase_independent_and_effect_free() {
    let mut coordinator = TerminalCoordinator::new();
    let pending = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let authorization = pending.authorization();
    let cursor = coordinator.next_identity;

    for phase in [
        TerminalPhase::Creating,
        TerminalPhase::AwaitingStart(identity()),
        TerminalPhase::Starting(identity()),
        TerminalPhase::Started {
            identity: identity(),
            geometry: geometry(),
        },
        TerminalPhase::Unknown(Some(identity())),
        TerminalPhase::Unavailable(Some(identity())),
    ] {
        coordinator.bindings.get_mut(&id(1)).unwrap().phase = phase;
        assert_eq!(
            coordinator.retained_authorization(&id(1)),
            Some(authorization)
        );
        assert_eq!(coordinator.next_identity, cursor);
    }
    assert_eq!(coordinator.retained_authorization(&id(2)), None);
}

#[test]
fn retained_create_preserves_precise_outcome_independently_of_later_phase() {
    for (completion, expected) in [
        (
            Completion::Busy,
            CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::ServerBusy,
            },
        ),
        (
            Completion::Unavailable,
            CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::TerminalRuntimeUnavailable,
            },
        ),
        (Completion::Unknown, CreateOutcome::OutcomeUnknown),
    ] {
        let mut coordinator = TerminalCoordinator::new();
        let pending = dispatch(coordinator.prepare(
            false,
            create(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
        let authorization = pending.authorization();
        let retained = coordinator
            .retained_create_for_binding(authorization.terminal_id(), authorization.worker_id())
            .unwrap();
        assert_eq!(retained.reference, &id(1));
        assert_eq!(retained.outcome, None);
        commit_completion(&mut coordinator, pending, completion, Some(worker(1)));
        for phase in [
            TerminalPhase::Unknown(None),
            TerminalPhase::Unavailable(None),
        ] {
            coordinator.bindings.get_mut(&id(1)).unwrap().phase = phase;
            let retained = coordinator
                .retained_create_for_binding(authorization.terminal_id(), authorization.worker_id())
                .unwrap();
            assert_eq!(retained.reference, &id(1));
            assert_eq!(retained.outcome, Some(expected));
        }
        assert_eq!(coordinator.next_identity, Some(2));
    }
}

#[test]
fn retained_create_lookup_rejects_missing_completion_and_duplicate_terminal_binding() {
    let mut coordinator = TerminalCoordinator::new();
    let pending = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let authorization = pending.authorization();
    coordinator.bindings.get_mut(&id(1)).unwrap().phase = TerminalPhase::Unavailable(None);
    assert!(
        coordinator
            .retained_create_for_binding(authorization.terminal_id(), authorization.worker_id(),)
            .is_none()
    );
    coordinator.bindings.get_mut(&id(1)).unwrap().phase = TerminalPhase::Creating;
    let _other = dispatch(coordinator.prepare(
        false,
        create(2),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    coordinator.bindings.get_mut(&id(2)).unwrap().authorization = authorization;
    assert!(
        coordinator
            .retained_create_for_binding(authorization.terminal_id(), authorization.worker_id(),)
            .is_none()
    );
}

#[test]
fn unavailable_worker_and_constraint_fail_without_allocating_authority() {
    let mut coordinator = TerminalCoordinator::new();
    assert!(
        matches!(coordinator.prepare(false, create(1), None, Some(expiry()), &LaunchCapture::absent()),PreparedOperation::Ready(ServerMessage::TerminalCreateResult(result)) if result.outcome()==CreateOutcome::Unavailable { reason: CreateUnavailableReason::NoCompatibleWorker })
    );
    let constrained = ClientMessage::TerminalCreate(TerminalCreateRequest::new(
        id(2),
        Some(WorkerConstraint::Exact {
            worker_id: orchbee_client_server_protocol::WorkerId::parse(&"07".repeat(16)).unwrap(),
        }),
        None,
        StartMode::Deferred,
        None,
    ));
    assert!(
        matches!(coordinator.prepare(false, constrained, Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),PreparedOperation::Ready(ServerMessage::TerminalCreateResult(result)) if result.outcome()==CreateOutcome::Rejected {reason: CreateRejectedReason::WorkerConstraintUnsatisfied})
    );
    assert!(coordinator.bindings.is_empty());
    assert_eq!(coordinator.next_identity, Some(1));
}

#[test]
fn deferred_create_start_commits_exact_identity_with_fresh_operation_and_fence() {
    let mut coordinator = TerminalCoordinator::new();
    let launch = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let observation = accepted_launch(&launch);
    let launch_authorization = launch.authorization();
    assert!(
        matches!(commit_completion(&mut coordinator,launch,observation,Some(worker(1))),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::Accepted {state: CreateAcceptedState::AwaitingStart})
    );
    let pending = dispatch(coordinator.prepare(
        false,
        start(2, 1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let CapabilityNodeMessage::AuthorizedTerminalStart(command) = &pending.command else {
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
        matches!(coordinator.prepare(false, start(3,1), Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Unavailable {reason:StartUnavailableReason::ServerBusy})
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
    commit_completion(
        &mut coordinator,
        pending,
        Completion::Start(result),
        Some(worker(1)),
    );
    let retained = coordinator
        .retained_create_for_binding(
            launch_authorization.terminal_id(),
            launch_authorization.worker_id(),
        )
        .unwrap();
    assert_eq!(
        retained.outcome,
        Some(CreateOutcome::Accepted {
            state: CreateAcceptedState::AwaitingStart,
        })
    );
    assert!(
        matches!(coordinator.prepare(false, start(4,1), Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Accepted {geometry:geometry(),transition:StartTransition::AlreadyStarted})
    );
    let conflicting = ClientMessage::TerminalStart(
        TerminalStartRequest::new(id(5), id(1), Geometry::new(100, 30, 8, 16).unwrap()).unwrap(),
    );
    assert!(
        matches!(coordinator.prepare(false, conflicting, Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Rejected {reason:StartRejectedReason::GeometryConflict})
    );
}

#[test]
fn unknown_create_and_stale_worker_results_cannot_authorize_start_or_replay() {
    for stale in [false, true] {
        let mut coordinator = TerminalCoordinator::new();
        let pending = dispatch(coordinator.prepare(
            false,
            create(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
        let outcome = if stale {
            accepted_launch(&pending)
        } else {
            Completion::Unknown
        };
        let placement = if stale { worker(2) } else { worker(1) };
        let authorization = pending.authorization();
        assert!(
            matches!(commit_completion(&mut coordinator,pending,outcome,Some(placement)),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::OutcomeUnknown)
        );
        assert!(
            matches!(coordinator.prepare(false, start(2,1), Some(placement), Some(expiry()), &LaunchCapture::absent()),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Rejected {reason:StartRejectedReason::TerminalCreateUnavailable})
        );
        assert!(
            matches!(coordinator.prepare(false, create(1), Some(placement), Some(expiry()), &LaunchCapture::absent()),PreparedOperation::Ready(ServerMessage::TerminalCreateResult(result)) if result.outcome()==CreateOutcome::Rejected {reason:CreateRejectedReason::NotAuthorized})
        );
        let another = dispatch(coordinator.prepare(
            false,
            create(3),
            Some(placement),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
        assert_ne!(
            another.authorization().operation_id(),
            authorization.operation_id()
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
        let launch = dispatch(coordinator.prepare(
            false,
            create(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
        let accepted = accepted_launch(&launch);
        commit_completion(&mut coordinator, launch, accepted, Some(worker(1)));
        let pending = dispatch(coordinator.prepare(
            false,
            start(2, 1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
        commit_completion(&mut coordinator, pending, completion, Some(worker(1)));
        assert_eq!(
            coordinator.bindings.get(&id(1)).unwrap().phase.identity(),
            Some(identity())
        );
        let next = coordinator.prepare(
            false,
            start(3, 1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        );
        assert_eq!(matches!(next, PreparedOperation::Dispatch(_)), !unknown);
    }
}

#[test]
fn bounds_retained_records_and_exhaustion_never_wraps_identity() {
    let mut coordinator = TerminalCoordinator::new();
    for number in 1..=MAXIMUM_TERMINAL_BINDINGS {
        dispatch(coordinator.prepare(
            false,
            create(number as u128),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
    }
    assert!(
        matches!(coordinator.prepare(false, create(2000), Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),PreparedOperation::Ready(ServerMessage::TerminalCreateResult(result)) if result.outcome()==CreateOutcome::Unavailable {reason:CreateUnavailableReason::ServerBusy})
    );
    let mut exhausted = TerminalCoordinator::new();
    exhausted.next_identity = None;
    assert!(matches!(
        exhausted.prepare(
            false,
            create(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent()
        ),
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
        None,
    ));
    let pending = dispatch(coordinator.prepare(
        false,
        request,
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
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
        matches!(commit_completion(&mut coordinator,pending,Completion::Launch(result),Some(worker(1))),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::Accepted {state:CreateAcceptedState::Started {geometry:geometry()}})
    );
    assert!(
        matches!(coordinator.prepare(false, start(2,1), Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Rejected {reason:StartRejectedReason::TerminalNotAwaitingStart})
    );
}

#[test]
fn contradictory_launch_mode_and_start_geometry_freeze_the_binding() {
    let mut coordinator = TerminalCoordinator::new();
    let pending = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let contradictory = TerminalLaunchResult::new(
        pending.authorization(),
        TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
            identity(),
            HostLaunchState::Started(worker_geometry(geometry()).unwrap()),
        )),
    )
    .unwrap();
    assert!(
        matches!(commit_completion(&mut coordinator,pending,Completion::Launch(contradictory),Some(worker(1))),ServerMessage::TerminalCreateResult(result) if result.outcome()==CreateOutcome::OutcomeUnknown)
    );
    let launch = dispatch(coordinator.prepare(
        false,
        create(2),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let accepted = accepted_launch(&launch);
    commit_completion(&mut coordinator, launch, accepted, Some(worker(1)));
    let pending = dispatch(coordinator.prepare(
        false,
        start(3, 2),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
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
        matches!(commit_completion(&mut coordinator,pending,Completion::Start(result),Some(worker(1))),ServerMessage::TerminalStartResult(result) if result.outcome()==StartOutcome::OutcomeUnknown)
    );
    assert!(
        matches!(coordinator.prepare(false, start(4,2), Some(worker(1)), Some(expiry()), &LaunchCapture::absent()),PreparedOperation::Ready(ServerMessage::TerminalStartResult(result)) if result.outcome()==StartOutcome::Rejected {reason:StartRejectedReason::TerminalCreateUnavailable})
    );
}

#[test]
fn stale_completion_seal_cannot_change_a_current_binding() {
    let mut coordinator = TerminalCoordinator::new();
    let pending = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let authorization = pending.authorization();
    let cursor = coordinator.next_identity;
    coordinator
        .bindings
        .get_mut(&id(1))
        .unwrap()
        .completion_seal = Arc::new(TerminalCompletionSeal);
    assert!(!Arc::ptr_eq(
        &coordinator.bindings.get(&id(1)).unwrap().completion_seal,
        &pending.completion_seal,
    ));
    assert_eq!(
        coordinator
            .prepare_completion(Box::new(pending), Completion::Unknown, Some(worker(1)),)
            .unwrap_err(),
        TerminalCompletionTokenMismatch
    );
    assert!(matches!(
        coordinator.bindings.get(&id(1)).unwrap().phase,
        TerminalPhase::Creating
    ));
    assert_eq!(
        coordinator.bindings.get(&id(1)).unwrap().authorization,
        authorization
    );
    assert_eq!(coordinator.next_identity, cursor);
}

#[test]
fn worker_launch_outcomes_keep_rejection_failure_and_unknown_distinct() {
    let epoch = identity().host_epoch();
    for (observed, expected) in [
        (
            TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::LifecycleActive),
            CreateOutcome::Unavailable {
                reason: CreateUnavailableReason::TerminalRuntimeUnavailable,
            },
        ),
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
        let pending = dispatch(coordinator.prepare(
            false,
            create(1),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
        let result = TerminalLaunchResult::new(pending.authorization(), observed).unwrap();
        assert!(
            matches!(commit_completion(&mut coordinator,pending,Completion::Launch(result),Some(worker(1))),ServerMessage::TerminalCreateResult(result) if result.outcome()==expected)
        );
    }
}

#[test]
fn lifecycle_active_start_refusal_is_runtime_unavailable_not_host_failure() {
    let mut coordinator = TerminalCoordinator::new();
    let launch = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let accepted = accepted_launch(&launch);
    commit_completion(&mut coordinator, launch, accepted, Some(worker(1)));
    let pending = dispatch(coordinator.prepare(
        false,
        start(2, 1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let result = orchbee_capability_node_protocol::TerminalStartResult::new(
        pending.authorization(),
        identity(),
        TerminalStartOutcome::Rejected(TerminalStartRejection::LifecycleActive),
    )
    .unwrap();
    assert!(matches!(
        commit_completion(&mut coordinator,pending,Completion::Start(result),Some(worker(1))),
        ServerMessage::TerminalStartResult(result) if result.outcome() == StartOutcome::Unavailable {
            reason: StartUnavailableReason::TerminalRuntimeUnavailable,
        }
    ));
}

#[test]
fn exact_binding_lookup_accepts_only_awaiting_or_started_retained_terminal() {
    let mut coordinator = TerminalCoordinator::new();
    let launch = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let authorization = launch.authorization();
    let accepted = accepted_launch(&launch);
    commit_completion(&mut coordinator, launch, accepted, Some(worker(1)));
    let cursor = coordinator.next_identity;

    assert_eq!(
        coordinator.binding_viewer_target(
            authorization.terminal_id(),
            authorization.worker_id(),
            Some(worker(1)),
        ),
        Ok(BindingViewerTarget {
            terminal_id: authorization.terminal_id(),
            identity: identity(),
            worker: worker(1),
        })
    );
    coordinator.bindings.get_mut(&id(1)).unwrap().phase = TerminalPhase::Started {
        identity: identity(),
        geometry: geometry(),
    };
    assert_eq!(
        coordinator.binding_viewer_target(
            authorization.terminal_id(),
            authorization.worker_id(),
            Some(worker(1)),
        ),
        Ok(BindingViewerTarget {
            terminal_id: authorization.terminal_id(),
            identity: identity(),
            worker: worker(1),
        })
    );
    assert_eq!(coordinator.next_identity, cursor);
    assert_eq!(
        coordinator.bindings.get(&id(1)).unwrap().authorization,
        authorization
    );
}

#[test]
fn binding_lookup_precedence_distinguishes_current_worker_record_generation_and_phase() {
    let mut coordinator = TerminalCoordinator::new();
    let launch = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let authorization = launch.authorization();
    let selected_terminal = authorization.terminal_id();
    let selected_worker = authorization.worker_id();
    let cursor = coordinator.next_identity;

    assert_eq!(
        coordinator.binding_viewer_target(selected_terminal, selected_worker, None),
        Err(BindingViewerTargetError::TerminalRuntimeUnavailable)
    );
    assert_eq!(
        coordinator.binding_viewer_target(
            selected_terminal,
            selected_worker,
            Some(other_worker(1)),
        ),
        Err(BindingViewerTargetError::TerminalRuntimeUnavailable)
    );
    assert_eq!(
        coordinator.binding_viewer_target(
            TerminalId::from_bytes([7; 16]).unwrap(),
            selected_worker,
            Some(worker(1)),
        ),
        Err(BindingViewerTargetError::TerminalUnavailable)
    );
    assert_eq!(
        coordinator.binding_viewer_target(
            selected_terminal,
            WorkerId::from_bytes([8; 16]).unwrap(),
            Some(other_worker(1)),
        ),
        Err(BindingViewerTargetError::TerminalUnavailable)
    );

    for phase in [
        TerminalPhase::Creating,
        TerminalPhase::Starting(identity()),
        TerminalPhase::Unknown(Some(identity())),
        TerminalPhase::Unavailable(Some(identity())),
    ] {
        coordinator.bindings.get_mut(&id(1)).unwrap().phase = phase;
        assert_eq!(
            coordinator.binding_viewer_target(selected_terminal, selected_worker, Some(worker(2)),),
            Err(BindingViewerTargetError::TerminalRuntimeUnavailable),
            "generation must be checked before phase for {phase:?}"
        );
        assert_eq!(
            coordinator.binding_viewer_target(selected_terminal, selected_worker, Some(worker(1)),),
            Err(BindingViewerTargetError::TerminalUnavailable)
        );
    }
    assert_eq!(coordinator.next_identity, cursor);
    assert_eq!(
        coordinator.bindings.get(&id(1)).unwrap().authorization,
        authorization
    );
}

#[test]
fn binding_lookup_is_bounded_by_the_retained_record_limit_and_finds_the_last_record() {
    let mut coordinator = TerminalCoordinator::new();
    let mut selected = None;
    for number in 1..=MAXIMUM_TERMINAL_BINDINGS {
        let pending = dispatch(coordinator.prepare(
            false,
            create(number as u128),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        ));
        selected = Some(pending.authorization());
    }
    let selected = selected.unwrap();
    coordinator
        .bindings
        .get_mut(&id(MAXIMUM_TERMINAL_BINDINGS as u128))
        .unwrap()
        .phase = TerminalPhase::AwaitingStart(identity());
    let cursor = coordinator.next_identity;

    assert_eq!(
        coordinator.binding_viewer_target(
            selected.terminal_id(),
            selected.worker_id(),
            Some(worker(1)),
        ),
        Ok(BindingViewerTarget {
            terminal_id: selected.terminal_id(),
            identity: identity(),
            worker: worker(1),
        })
    );
    assert_eq!(coordinator.bindings.len(), MAXIMUM_TERMINAL_BINDINGS);
    assert_eq!(coordinator.next_identity, cursor);
}

#[test]
fn viewer_identity_allocation_is_atomic_at_zero_and_exhaustion_boundaries() {
    let mut coordinator = TerminalCoordinator::new();
    coordinator.next_identity = Some(u128::MAX - 2);
    let allocated = coordinator.allocate_viewer_identities().unwrap();
    assert_eq!(
        allocated.attachment.as_bytes(),
        (u128::MAX - 2).to_be_bytes()
    );
    assert_eq!(allocated.request.as_bytes(), (u128::MAX - 1).to_be_bytes());
    assert_eq!(allocated.cleanup.as_bytes(), u128::MAX.to_be_bytes());
    assert_eq!(coordinator.next_identity, None);

    for cursor in [0, u128::MAX - 1, u128::MAX] {
        let mut coordinator = TerminalCoordinator::new();
        coordinator.next_identity = Some(cursor);
        assert_eq!(coordinator.allocate_viewer_identities(), None);
        assert_eq!(coordinator.next_identity, Some(cursor));
        assert!(coordinator.bindings.is_empty());
    }
}

#[test]
fn uncreated_close_requires_unique_settled_failed_create_and_no_retained_identity() {
    let mut coordinator = TerminalCoordinator::new();
    let launch = dispatch(coordinator.prepare(
        false,
        create(1),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    let authorization = launch.authorization();
    let uncreated = |coordinator: &TerminalCoordinator| {
        coordinator
            .is_definitively_uncreated(authorization.terminal_id(), authorization.worker_id())
    };
    assert!(!uncreated(&coordinator));
    commit_completion(
        &mut coordinator,
        launch,
        Completion::Unavailable,
        Some(worker(1)),
    );
    assert!(uncreated(&coordinator));
    for phase in [
        TerminalPhase::Creating,
        TerminalPhase::Starting(identity()),
        TerminalPhase::AwaitingStart(identity()),
        TerminalPhase::Started {
            identity: identity(),
            geometry: geometry(),
        },
        TerminalPhase::Unknown(None),
        TerminalPhase::Unknown(Some(identity())),
        TerminalPhase::Unavailable(Some(identity())),
    ] {
        coordinator.bindings.get_mut(&id(1)).unwrap().phase = phase;
        assert!(!uncreated(&coordinator), "{phase:?}");
    }
    coordinator.bindings.get_mut(&id(1)).unwrap().phase = TerminalPhase::Unavailable(None);
    for outcome in [
        None,
        Some(CreateOutcome::OutcomeUnknown {}),
        Some(CreateOutcome::Accepted {
            state: CreateAcceptedState::AwaitingStart,
        }),
    ] {
        coordinator.bindings.get_mut(&id(1)).unwrap().create_outcome = outcome;
        assert!(!uncreated(&coordinator));
    }
    coordinator.bindings.get_mut(&id(1)).unwrap().create_outcome = Some(CreateOutcome::Failed {
        reason: CreateFailureReason::TerminalCreateFailed,
    });
    assert!(uncreated(&coordinator));
    let _other = dispatch(coordinator.prepare(
        false,
        create(2),
        Some(worker(1)),
        Some(expiry()),
        &LaunchCapture::absent(),
    ));
    coordinator.bindings.get_mut(&id(2)).unwrap().authorization = authorization;
    assert!(!uncreated(&coordinator));
}

#[test]
fn plugin_terminal_launch_rejects_oversized_frames_before_reserving_and_preserves_metadata() {
    let mut coordinator = TerminalCoordinator::new();
    let request = |large: bool| {
        let mut entries: Vec<_> = orchbee_client_server_protocol::PLUGIN_TERMINAL_ENVIRONMENT_NAMES
            [..7]
            .iter()
            .map(|key| ((*key).into(), "literal".to_owned()))
            .collect();
        if large {
            entries[5].1 = "x".repeat(20 * 1024);
        }
        TerminalCreateRequest::new(
            RequestId::parse(&format!("{:032x}", 91)).unwrap(),
            None,
            None,
            StartMode::Deferred,
            Some(
                orchbee_client_server_protocol::LaunchSelection::new(
                    "C:/plugin.exe".into(),
                    vec![if large {
                        "x".repeat(30 * 1024 - 100)
                    } else {
                        "argument".into()
                    }],
                )
                .unwrap(),
            ),
        )
        .with_plugin_environment(Some(
            orchbee_client_server_protocol::PluginTerminalEnvironment::new(entries).unwrap(),
        ))
        .unwrap()
    };
    let identity = coordinator.next_identity;
    assert!(matches!(
        create_refusal(coordinator.plan_create(
            request(true),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent()
        )),
        CreateOutcome::Rejected {
            reason: CreateRejectedReason::LaunchRequestTooLarge
        }
    ));
    assert_eq!(coordinator.next_identity, identity);
    assert!(coordinator.bindings.is_empty());
    let intent = request(false);
    let plan = coordinator
        .plan_create(
            intent.clone(),
            Some(worker(1)),
            Some(expiry()),
            &LaunchCapture::absent(),
        )
        .unwrap();
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = plan.command() else {
        panic!()
    };
    assert_eq!(
        launch.plugin_environment().unwrap().entries(),
        intent.plugin_environment().unwrap().entries()
    );
    let pending = plan.commit();
    assert_eq!(pending.request, ClientMessage::TerminalCreate(intent));
    assert_eq!(coordinator.bindings.len(), 1);
}
