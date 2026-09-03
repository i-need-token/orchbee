use std::num::{NonZeroU16, NonZeroU64};

use orchbee_capability_node_protocol::{
    AbsoluteCwd, AuthorizedTerminalLaunch, AuthorizedTerminalStart, HostCreationId, HostEpoch,
    HostLaunchAccepted, HostLaunchState, HostStartAccepted, HostTerminalId, HostTerminalIdentity,
    LeaseExpiresAtUnixMilliseconds, LeaseId, OperationId, ProtocolError, StartTransition,
    TERMINAL_LAUNCH_CAPABILITY_VERSION, TerminalFence, TerminalId, TerminalLaunchCapabilityVersion,
    TerminalLaunchFailure, TerminalLaunchMode, TerminalLaunchOutcome, TerminalLaunchRejection,
    TerminalLaunchResult, TerminalOperationAuthorization, TerminalOperationLease,
    TerminalPixelGeometry, TerminalStartFailure, TerminalStartOutcome, TerminalStartRejection,
    TerminalStartResult, WorkerGeneration, WorkerId,
};
use orchbee_local_site_protocol::WorkerId as LocalSiteWorkerId;

use super::{
    AuthorizationKernelError, AuthorizedHostDispatch, AuthorizedHostOperation,
    AuthorizedHostOperationKernel, AuthorizedHostOperationResult, HostHandoffAdmission,
    HostOperationAdmission, LaunchHostCompletion, StartHostCompletion,
};

const CURRENT_LOCAL_WORKER_ID: LocalSiteWorkerId = LocalSiteWorkerId::from_bytes([3; 16]);
const NOW: u64 = 100;
const LIVE_EXPIRY: u64 = 200;

#[derive(Clone, Copy)]
struct AuthorizationSpec {
    capability: u16,
    expiry: u64,
    fence: u64,
    generation: u64,
    lease: u8,
    operation: u8,
    terminal: u8,
    worker: u8,
}

impl AuthorizationSpec {
    const fn standard(terminal: u8, operation: u8) -> Self {
        Self {
            capability: 1,
            expiry: LIVE_EXPIRY,
            fence: 1,
            generation: 7,
            lease: operation,
            operation,
            terminal,
            worker: 3,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RequestKind {
    Launch,
    Start,
}

#[derive(Clone, Copy, Debug)]
enum ExpectedAdmissionRejection {
    WrongWorker,
    StaleWorkerGeneration,
    LeaseExpired,
    StaleFence,
    ConflictingReplay,
    UnsupportedCapability,
}

#[test]
fn rejects_invalid_authority_for_launch_and_start_without_dispatch() -> TestResult {
    let cases = [
        (
            "wrong Worker",
            AuthorizationSpec {
                worker: 9,
                ..AuthorizationSpec::standard(1, 1)
            },
            ExpectedAdmissionRejection::WrongWorker,
        ),
        (
            "stale Worker generation",
            AuthorizationSpec {
                generation: 6,
                ..AuthorizationSpec::standard(1, 1)
            },
            ExpectedAdmissionRejection::StaleWorkerGeneration,
        ),
        (
            "unsupported capability",
            AuthorizationSpec {
                capability: 2,
                ..AuthorizationSpec::standard(1, 1)
            },
            ExpectedAdmissionRejection::UnsupportedCapability,
        ),
        (
            "expired lease",
            AuthorizationSpec {
                expiry: NOW,
                ..AuthorizationSpec::standard(1, 1)
            },
            ExpectedAdmissionRejection::LeaseExpired,
        ),
    ];

    for request_kind in [RequestKind::Launch, RequestKind::Start] {
        for (case_name, spec, expected) in cases {
            let mut kernel = kernel()?;
            let operation = operation(request_kind, spec)?;
            let expected_binding = operation.clone();
            assert_eq!(
                admit(&mut kernel, operation, NOW)?,
                HostOperationAdmission::ResultReady,
                "{request_kind:?}: {case_name}"
            );
            let result = take_required_result(&mut kernel);
            assert_result_binding(&result, &expected_binding);
            assert_rejection(result, expected, request_kind);
        }
    }
    Ok(())
}

#[test]
fn same_fence_conflicts_cover_complete_bindings_and_fences_are_per_terminal() -> TestResult {
    let mut kernel = kernel()?;
    let base_spec = AuthorizationSpec {
        fence: 2,
        ..AuthorizationSpec::standard(1, 1)
    };
    let base = launch_with(base_spec, Some("/base"), TerminalLaunchMode::Deferred)?;
    let base_dispatch = expect_dispatch(kernel.admit_launch(base.clone(), NOW)?);
    kernel.record_pre_handoff_failure(&base_dispatch, NOW)?;
    let cached_base = take_required_result(&mut kernel);

    let lower_unsupported = launch_with(
        AuthorizationSpec {
            capability: 2,
            expiry: NOW,
            fence: 1,
            ..base_spec
        },
        Some("/different"),
        TerminalLaunchMode::Deferred,
    )?;
    assert_eq!(
        kernel.admit_launch(lower_unsupported, NOW)?,
        HostOperationAdmission::ResultReady
    );
    assert_rejection(
        take_required_result(&mut kernel),
        ExpectedAdmissionRejection::UnsupportedCapability,
        RequestKind::Launch,
    );

    let lower_expired = launch_with(
        AuthorizationSpec {
            expiry: NOW,
            fence: 1,
            ..base_spec
        },
        Some("/different"),
        TerminalLaunchMode::Deferred,
    )?;
    assert_eq!(
        kernel.admit_launch(lower_expired, NOW)?,
        HostOperationAdmission::ResultReady
    );
    assert_rejection(
        take_required_result(&mut kernel),
        ExpectedAdmissionRejection::LeaseExpired,
        RequestKind::Launch,
    );

    let lower = launch_with(
        AuthorizationSpec {
            fence: 1,
            ..base_spec
        },
        Some("/different"),
        TerminalLaunchMode::Deferred,
    )?;
    assert_eq!(
        kernel.admit_launch(lower, NOW)?,
        HostOperationAdmission::ResultReady
    );
    assert_rejection(
        take_required_result(&mut kernel),
        ExpectedAdmissionRejection::StaleFence,
        RequestKind::Launch,
    );

    for (changed_authority, expected) in [
        (
            AuthorizationSpec {
                capability: 2,
                ..base_spec
            },
            ExpectedAdmissionRejection::UnsupportedCapability,
        ),
        (
            AuthorizationSpec {
                expiry: NOW,
                ..base_spec
            },
            ExpectedAdmissionRejection::LeaseExpired,
        ),
    ] {
        assert_eq!(
            kernel.admit_launch(
                launch_with(
                    changed_authority,
                    Some("/base"),
                    TerminalLaunchMode::Deferred,
                )?,
                NOW,
            )?,
            HostOperationAdmission::ResultReady
        );
        assert_rejection(
            take_required_result(&mut kernel),
            expected,
            RequestKind::Launch,
        );
    }

    let conflicting_launches = [
        launch_with(
            AuthorizationSpec {
                operation: 9,
                ..base_spec
            },
            Some("/base"),
            TerminalLaunchMode::Deferred,
        )?,
        launch_with(
            AuthorizationSpec {
                lease: 9,
                ..base_spec
            },
            Some("/base"),
            TerminalLaunchMode::Deferred,
        )?,
        launch_with(
            AuthorizationSpec {
                expiry: LIVE_EXPIRY + 1,
                ..base_spec
            },
            Some("/base"),
            TerminalLaunchMode::Deferred,
        )?,
        launch_with(base_spec, Some("/other"), TerminalLaunchMode::Deferred)?,
        launch_with(
            base_spec,
            Some("/base"),
            TerminalLaunchMode::Immediate(geometry(80)?),
        )?,
    ];
    for conflicting in conflicting_launches {
        assert_eq!(
            kernel.admit_launch(conflicting, NOW)?,
            HostOperationAdmission::ResultReady
        );
        assert_rejection(
            take_required_result(&mut kernel),
            ExpectedAdmissionRejection::ConflictingReplay,
            RequestKind::Launch,
        );
    }
    assert_eq!(
        kernel.admit_launch(base, NOW)?,
        HostOperationAdmission::ResultReady
    );
    assert_eq!(take_required_result(&mut kernel), cached_base);

    let cross_kind = start(base_spec, host_identity(5, 6, 7)?, geometry(80)?)?;
    assert_eq!(
        admit_start_at_target(&mut kernel, cross_kind, NOW)?,
        HostOperationAdmission::ResultReady
    );
    assert_rejection(
        take_required_result(&mut kernel),
        ExpectedAdmissionRejection::ConflictingReplay,
        RequestKind::Start,
    );

    let independent_terminal = launch(AuthorizationSpec::standard(2, 2))?;
    let independent_dispatch = expect_dispatch(kernel.admit_launch(independent_terminal, NOW)?);
    kernel.record_pre_handoff_failure(&independent_dispatch, NOW)?;
    let _independent_result = take_required_result(&mut kernel);

    let start_spec = AuthorizationSpec::standard(3, 3);
    let target = host_identity(8, 6, 9)?;
    let base_start = start(start_spec, target, geometry(80)?)?;
    let start_dispatch = expect_dispatch(admit_start_at_target(&mut kernel, base_start, NOW)?);
    kernel.record_pre_handoff_failure(&start_dispatch, NOW)?;
    let _cached_start = take_required_result(&mut kernel);

    for conflicting in [
        start(start_spec, host_identity(10, 6, 11)?, geometry(80)?)?,
        start(start_spec, target, geometry(81)?)?,
    ] {
        assert_eq!(
            admit_start_at_target(&mut kernel, conflicting, NOW)?,
            HostOperationAdmission::ResultReady
        );
        assert_rejection(
            take_required_result(&mut kernel),
            ExpectedAdmissionRejection::ConflictingReplay,
            RequestKind::Start,
        );
    }
    Ok(())
}

#[test]
fn exact_launch_replay_preserves_every_cached_outcome_after_expiry() -> TestResult {
    #[derive(Clone, Copy)]
    enum Scenario {
        AcceptedImmediate,
        AcceptedDeferred,
        HostRejected,
        HostFailed,
        PreHandoffFailed,
        OutcomeUnknown,
    }

    for (index, scenario) in [
        Scenario::AcceptedImmediate,
        Scenario::AcceptedDeferred,
        Scenario::HostRejected,
        Scenario::HostFailed,
        Scenario::PreHandoffFailed,
        Scenario::OutcomeUnknown,
    ]
    .into_iter()
    .enumerate()
    {
        let mut kernel = kernel()?;
        let id = u8::try_from(index + 1)?;
        let request = match scenario {
            Scenario::AcceptedImmediate => launch_with(
                AuthorizationSpec::standard(id, id),
                None,
                TerminalLaunchMode::Immediate(geometry(80)?),
            )?,
            Scenario::AcceptedDeferred
            | Scenario::HostRejected
            | Scenario::HostFailed
            | Scenario::PreHandoffFailed
            | Scenario::OutcomeUnknown => launch(AuthorizationSpec::standard(id, id))?,
        };
        let dispatch = expect_dispatch(kernel.admit_launch(request.clone(), NOW)?);
        let epoch = host_epoch(6)?;
        let expected_outcome = match scenario {
            Scenario::AcceptedImmediate | Scenario::AcceptedDeferred => {
                assert_eq!(
                    kernel.begin_host_handoff(&dispatch, NOW, epoch)?,
                    HostHandoffAdmission::HandedOff
                );
                let identity = host_identity(20, 6, id)?;
                kernel.record_launch_completion(
                    &dispatch,
                    LaunchHostCompletion::Accepted { identity },
                )?;
                let state = match scenario {
                    Scenario::AcceptedImmediate => HostLaunchState::Started(geometry(80)?),
                    Scenario::AcceptedDeferred => HostLaunchState::AwaitingStart,
                    Scenario::HostRejected
                    | Scenario::HostFailed
                    | Scenario::PreHandoffFailed
                    | Scenario::OutcomeUnknown => unreachable!(),
                };
                let accepted = HostLaunchAccepted::new(identity, state);
                TerminalLaunchOutcome::HostAccepted(accepted)
            }
            Scenario::HostRejected => {
                kernel.begin_host_handoff(&dispatch, NOW, epoch)?;
                kernel.record_launch_completion(
                    &dispatch,
                    LaunchHostCompletion::Rejected { host_epoch: epoch },
                )?;
                TerminalLaunchOutcome::Rejected(TerminalLaunchRejection::HostRejected {
                    host_epoch: epoch,
                })
            }
            Scenario::HostFailed => {
                kernel.begin_host_handoff(&dispatch, NOW, epoch)?;
                kernel.record_launch_completion(
                    &dispatch,
                    LaunchHostCompletion::Failed { host_epoch: epoch },
                )?;
                TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostReportedFailure {
                    host_epoch: epoch,
                })
            }
            Scenario::PreHandoffFailed => {
                kernel.record_pre_handoff_failure(&dispatch, NOW)?;
                TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff)
            }
            Scenario::OutcomeUnknown => {
                kernel.begin_host_handoff(&dispatch, NOW, epoch)?;
                kernel.record_outcome_unknown(&dispatch)?;
                TerminalLaunchOutcome::OutcomeUnknown { host_epoch: epoch }
            }
        };

        let original = take_required_result(&mut kernel);
        assert_eq!(
            original,
            AuthorizedHostOperationResult::Launch(TerminalLaunchResult::new(
                request.authorization(),
                expected_outcome,
            ))
        );
        assert_eq!(
            kernel.admit_launch(request, LIVE_EXPIRY)?,
            HostOperationAdmission::ResultReady
        );
        assert_eq!(take_required_result(&mut kernel), original);
    }
    Ok(())
}

#[test]
fn exact_start_replay_preserves_host_results_and_both_success_transitions() -> TestResult {
    #[derive(Clone, Copy)]
    enum Scenario {
        Started,
        AlreadyStarted,
        HostRejected,
        HostFailed,
        PreHandoffFailed,
        OutcomeUnknown,
    }

    for (index, scenario) in [
        Scenario::Started,
        Scenario::AlreadyStarted,
        Scenario::HostRejected,
        Scenario::HostFailed,
        Scenario::PreHandoffFailed,
        Scenario::OutcomeUnknown,
    ]
    .into_iter()
    .enumerate()
    {
        let mut kernel = kernel()?;
        let id = u8::try_from(index + 20)?;
        let target = host_identity(id, 6, id.saturating_add(1))?;
        let request = start(AuthorizationSpec::standard(id, id), target, geometry(80)?)?;
        let dispatch = expect_dispatch(admit_start_at_target(&mut kernel, request, NOW)?);
        let epoch = target.host_epoch();
        let expected_outcome = match scenario {
            Scenario::Started | Scenario::AlreadyStarted => {
                kernel.begin_host_handoff(&dispatch, NOW, epoch)?;
                let transition = match scenario {
                    Scenario::Started => StartTransition::Started,
                    Scenario::AlreadyStarted => StartTransition::AlreadyStarted,
                    Scenario::HostRejected
                    | Scenario::HostFailed
                    | Scenario::PreHandoffFailed
                    | Scenario::OutcomeUnknown => unreachable!(),
                };
                let accepted = HostStartAccepted::new(target, request.geometry(), transition);
                kernel
                    .record_start_completion(&dispatch, StartHostCompletion::Accepted(accepted))?;
                TerminalStartOutcome::HostAccepted(accepted)
            }
            Scenario::HostRejected => {
                kernel.begin_host_handoff(&dispatch, NOW, epoch)?;
                kernel.record_start_completion(
                    &dispatch,
                    StartHostCompletion::Rejected { host_epoch: epoch },
                )?;
                TerminalStartOutcome::Rejected(TerminalStartRejection::HostRejected {
                    host_epoch: epoch,
                })
            }
            Scenario::HostFailed => {
                kernel.begin_host_handoff(&dispatch, NOW, epoch)?;
                kernel.record_start_completion(
                    &dispatch,
                    StartHostCompletion::Failed { host_epoch: epoch },
                )?;
                TerminalStartOutcome::Failed(TerminalStartFailure::HostReportedFailure {
                    host_epoch: epoch,
                })
            }
            Scenario::PreHandoffFailed => {
                kernel.record_pre_handoff_failure(&dispatch, NOW)?;
                TerminalStartOutcome::Failed(TerminalStartFailure::HostUnavailableBeforeHandoff)
            }
            Scenario::OutcomeUnknown => {
                kernel.begin_host_handoff(&dispatch, NOW, epoch)?;
                kernel.record_outcome_unknown(&dispatch)?;
                TerminalStartOutcome::OutcomeUnknown
            }
        };

        let original = take_required_result(&mut kernel);
        assert_eq!(
            original,
            AuthorizedHostOperationResult::Start(TerminalStartResult::new(
                request.authorization(),
                target,
                expected_outcome,
            )?)
        );
        assert_eq!(
            admit_start_at_target(&mut kernel, request, LIVE_EXPIRY)?,
            HostOperationAdmission::ResultReady
        );
        assert_eq!(take_required_result(&mut kernel), original);
    }
    Ok(())
}

#[test]
fn higher_fence_rebinds_authority_clears_cache_and_rejects_old_dispatch() -> TestResult {
    let mut kernel = kernel()?;
    let low_request = launch(AuthorizationSpec::standard(1, 1))?;
    let low_dispatch = expect_dispatch(kernel.admit_launch(low_request.clone(), NOW)?);
    let epoch = host_epoch(6)?;
    kernel.begin_host_handoff(&low_dispatch, NOW, epoch)?;
    kernel.record_launch_completion(
        &low_dispatch,
        LaunchHostCompletion::Rejected { host_epoch: epoch },
    )?;
    let _low_cached_result = take_required_result(&mut kernel);

    let unsupported_higher = launch(AuthorizationSpec {
        capability: 2,
        fence: 3,
        lease: 3,
        operation: 3,
        ..AuthorizationSpec::standard(1, 1)
    })?;
    assert_eq!(
        kernel.admit_launch(unsupported_higher, NOW)?,
        HostOperationAdmission::ResultReady
    );
    assert_rejection(
        take_required_result(&mut kernel),
        ExpectedAdmissionRejection::UnsupportedCapability,
        RequestKind::Launch,
    );

    let high_request = launch_with(
        AuthorizationSpec {
            fence: 2,
            lease: 2,
            operation: 2,
            ..AuthorizationSpec::standard(1, 1)
        },
        Some("/new-binding"),
        TerminalLaunchMode::Deferred,
    )?;
    let high_dispatch = expect_dispatch(kernel.admit_launch(high_request.clone(), NOW)?);
    assert_eq!(
        kernel.record_outcome_unknown(&low_dispatch),
        Err(AuthorizationKernelError::DispatchMismatch)
    );
    assert_eq!(
        kernel.admit_launch(high_request.clone(), NOW)?,
        HostOperationAdmission::SlotOccupied
    );
    kernel.record_pre_handoff_failure(&high_dispatch, NOW)?;
    let high_result = take_required_result(&mut kernel);

    assert_eq!(
        kernel.admit_launch(low_request, NOW)?,
        HostOperationAdmission::ResultReady
    );
    assert_rejection(
        take_required_result(&mut kernel),
        ExpectedAdmissionRejection::StaleFence,
        RequestKind::Launch,
    );
    assert_eq!(
        kernel.admit_launch(high_request, LIVE_EXPIRY)?,
        HostOperationAdmission::ResultReady
    );
    assert_eq!(take_required_result(&mut kernel), high_result);
    Ok(())
}

#[test]
fn higher_start_fence_rebinds_target_and_exact_result() -> TestResult {
    let mut kernel = kernel()?;
    let low_request = start(
        AuthorizationSpec::standard(1, 1),
        host_identity(5, 6, 7)?,
        geometry(80)?,
    )?;
    let low_dispatch = expect_dispatch(admit_start_at_target(&mut kernel, low_request, NOW)?);
    kernel.record_pre_handoff_failure(&low_dispatch, NOW)?;
    let _low_result = take_required_result(&mut kernel);

    let high_request = start(
        AuthorizationSpec {
            fence: 2,
            lease: 2,
            operation: 2,
            ..AuthorizationSpec::standard(1, 1)
        },
        host_identity(8, 6, 9)?,
        geometry(81)?,
    )?;
    let high_dispatch = expect_dispatch(admit_start_at_target(&mut kernel, high_request, NOW)?);
    assert_eq!(
        kernel.record_pre_handoff_failure(&low_dispatch, NOW),
        Err(AuthorizationKernelError::DispatchMismatch)
    );
    kernel.record_pre_handoff_failure(&high_dispatch, NOW)?;
    let high_result = take_required_result(&mut kernel);

    assert_eq!(
        admit_start_at_target(&mut kernel, low_request, NOW)?,
        HostOperationAdmission::ResultReady
    );
    assert_rejection(
        take_required_result(&mut kernel),
        ExpectedAdmissionRejection::StaleFence,
        RequestKind::Start,
    );
    assert_eq!(
        admit_start_at_target(&mut kernel, high_request, LIVE_EXPIRY)?,
        HostOperationAdmission::ResultReady
    );
    assert_eq!(take_required_result(&mut kernel), high_result);
    Ok(())
}

#[test]
fn initial_start_identity_mismatch_preserves_the_previous_binding_and_cache() -> TestResult {
    let mut kernel = kernel()?;
    let target = host_identity(5, 6, 7)?;
    let low_request = start(AuthorizationSpec::standard(1, 1), target, geometry(80)?)?;
    let low_dispatch = expect_dispatch(admit_start_at_target(&mut kernel, low_request, NOW)?);
    kernel.record_pre_handoff_failure(&low_dispatch, NOW)?;
    let low_result = take_required_result(&mut kernel);

    let high_request = start(
        AuthorizationSpec {
            fence: 2,
            lease: 2,
            operation: 2,
            ..AuthorizationSpec::standard(1, 1)
        },
        target,
        geometry(81)?,
    )?;
    let replacement_epoch = host_epoch(8)?;
    assert_eq!(
        kernel.admit_start(high_request, NOW, replacement_epoch)?,
        HostOperationAdmission::ResultReady
    );
    let AuthorizedHostOperationResult::Start(identity_changed) = take_required_result(&mut kernel)
    else {
        return Err("expected initial Start identity-change rejection".into());
    };
    assert_eq!(
        identity_changed.authorization(),
        high_request.authorization()
    );
    assert_eq!(identity_changed.target(), target);
    assert_eq!(
        identity_changed.outcome(),
        TerminalStartOutcome::Rejected(TerminalStartRejection::HostIdentityChanged {
            observed_host_epoch: replacement_epoch,
        })
    );

    assert_eq!(
        kernel.admit_start(low_request, NOW, replacement_epoch)?,
        HostOperationAdmission::ResultReady
    );
    assert_eq!(take_required_result(&mut kernel), low_result);
    let high_dispatch = expect_dispatch(admit_start_at_target(&mut kernel, high_request, NOW)?);
    kernel.record_pre_handoff_failure(&high_dispatch, NOW)?;
    Ok(())
}

#[test]
fn one_slot_blocks_requests_during_every_operation_and_result_phase() -> TestResult {
    let mut kernel = kernel()?;
    let first = launch(AuthorizationSpec::standard(1, 1))?;
    let second = start(
        AuthorizationSpec::standard(2, 2),
        host_identity(5, 6, 7)?,
        geometry(80)?,
    )?;
    let dispatch = expect_dispatch(kernel.admit_launch(first, NOW)?);
    assert!(kernel.take_result().is_none());
    assert_eq!(
        admit_start_at_target(&mut kernel, second, NOW)?,
        HostOperationAdmission::SlotOccupied
    );

    let epoch = host_epoch(6)?;
    kernel.begin_host_handoff(&dispatch, NOW, epoch)?;
    assert_eq!(
        admit_start_at_target(&mut kernel, second, NOW)?,
        HostOperationAdmission::SlotOccupied
    );
    kernel.record_outcome_unknown(&dispatch)?;
    assert_eq!(
        admit_start_at_target(&mut kernel, second, NOW)?,
        HostOperationAdmission::SlotOccupied
    );

    let _first_result = take_required_result(&mut kernel);
    let second_dispatch = expect_dispatch(admit_start_at_target(&mut kernel, second, NOW)?);
    kernel.record_pre_handoff_failure(&second_dispatch, NOW)?;
    assert!(kernel.take_result().is_some());
    Ok(())
}

#[test]
fn dispatch_tokens_reject_wrong_stale_and_out_of_phase_observations() -> TestResult {
    let mut subject = kernel()?;
    let request = launch(AuthorizationSpec::standard(1, 1))?;
    let dispatch = expect_dispatch(subject.admit_launch(request, NOW)?);
    let epoch = host_epoch(6)?;
    assert_eq!(
        subject.record_launch_completion(
            &dispatch,
            LaunchHostCompletion::Rejected { host_epoch: epoch }
        ),
        Err(AuthorizationKernelError::InvalidDispatchPhase)
    );

    let mut other_kernel = kernel()?;
    let other_dispatch = expect_dispatch(
        other_kernel.admit_launch(launch(AuthorizationSpec::standard(2, 2))?, NOW)?,
    );
    assert_eq!(
        subject.begin_host_handoff(&other_dispatch, NOW, epoch),
        Err(AuthorizationKernelError::DispatchMismatch)
    );

    let mut next_generation = AuthorizedHostOperationKernel::new(
        CURRENT_LOCAL_WORKER_ID,
        nonzero_u64(8)?,
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    )?;
    let next_generation_dispatch = expect_dispatch(next_generation.admit_launch(
        launch(AuthorizationSpec {
            generation: 8,
            ..AuthorizationSpec::standard(1, 1)
        })?,
        NOW,
    )?);
    assert_eq!(
        next_generation.begin_host_handoff(&dispatch, NOW, epoch),
        Err(AuthorizationKernelError::DispatchMismatch)
    );
    next_generation.record_pre_handoff_failure(&next_generation_dispatch, NOW)?;

    subject.begin_host_handoff(&dispatch, NOW, epoch)?;
    assert_eq!(
        subject.record_start_completion(
            &dispatch,
            StartHostCompletion::Rejected { host_epoch: epoch }
        ),
        Err(AuthorizationKernelError::OperationKindMismatch)
    );
    assert_eq!(
        subject.record_pre_handoff_failure(&dispatch, NOW),
        Err(AuthorizationKernelError::InvalidDispatchPhase)
    );
    assert_eq!(
        subject.record_launch_completion(
            &dispatch,
            LaunchHostCompletion::Rejected {
                host_epoch: host_epoch(7)?
            }
        ),
        Err(AuthorizationKernelError::Protocol(
            ProtocolError::ContradictoryHostObservation
        ))
    );

    subject.record_outcome_unknown(&dispatch)?;
    assert_eq!(
        subject.record_outcome_unknown(&dispatch),
        Err(AuthorizationKernelError::InvalidDispatchPhase)
    );
    let _result = take_required_result(&mut subject);
    assert_eq!(
        subject.record_outcome_unknown(&dispatch),
        Err(AuthorizationKernelError::DispatchMismatch)
    );
    Ok(())
}

#[test]
fn expiry_between_admission_and_handoff_is_cached_instead_of_dispatched() -> TestResult {
    let mut launch_kernel = kernel()?;
    let spec = AuthorizationSpec {
        expiry: NOW + 1,
        ..AuthorizationSpec::standard(1, 1)
    };
    let request = launch(spec)?;
    let dispatch = expect_dispatch(launch_kernel.admit_launch(request.clone(), NOW)?);
    assert_eq!(
        launch_kernel.begin_host_handoff(&dispatch, NOW + 1, host_epoch(6)?)?,
        HostHandoffAdmission::ResultReady
    );
    let expired = take_required_result(&mut launch_kernel);
    assert_rejection(
        expired.clone(),
        ExpectedAdmissionRejection::LeaseExpired,
        RequestKind::Launch,
    );
    assert_eq!(
        launch_kernel.admit_launch(request, NOW + 50)?,
        HostOperationAdmission::ResultReady
    );
    assert_eq!(take_required_result(&mut launch_kernel), expired);

    let mut start_kernel = kernel()?;
    let start_request = start(
        AuthorizationSpec {
            expiry: NOW + 1,
            ..AuthorizationSpec::standard(2, 2)
        },
        host_identity(5, 6, 7)?,
        geometry(80)?,
    )?;
    let start_dispatch = expect_dispatch(admit_start_at_target(
        &mut start_kernel,
        start_request,
        NOW,
    )?);
    start_kernel.record_pre_handoff_failure(&start_dispatch, NOW + 1)?;
    assert_rejection(
        take_required_result(&mut start_kernel),
        ExpectedAdmissionRejection::LeaseExpired,
        RequestKind::Start,
    );
    Ok(())
}

#[test]
fn expired_authority_does_not_revive_when_observed_time_moves_backward() -> TestResult {
    for request_kind in [RequestKind::Launch, RequestKind::Start] {
        let mut kernel = kernel()?;
        let operation = operation(
            request_kind,
            AuthorizationSpec {
                expiry: NOW,
                ..AuthorizationSpec::standard(1, 1)
            },
        )?;
        assert_eq!(
            admit(&mut kernel, operation.clone(), NOW)?,
            HostOperationAdmission::ResultReady
        );
        assert_rejection(
            take_required_result(&mut kernel),
            ExpectedAdmissionRejection::LeaseExpired,
            request_kind,
        );
        assert_eq!(
            admit(&mut kernel, operation, NOW - 1)?,
            HostOperationAdmission::ResultReady
        );
        assert_rejection(
            take_required_result(&mut kernel),
            ExpectedAdmissionRejection::LeaseExpired,
            request_kind,
        );
    }

    let mut kernel = kernel()?;
    let request = launch(AuthorizationSpec {
        expiry: NOW + 10,
        ..AuthorizationSpec::standard(2, 2)
    })?;
    let dispatch = expect_dispatch(kernel.admit_launch(request.clone(), NOW)?);
    assert_eq!(
        kernel.admit_launch(request.clone(), NOW + 10)?,
        HostOperationAdmission::SlotOccupied
    );
    assert_eq!(
        kernel.begin_host_handoff(&dispatch, NOW - 1, host_epoch(6)?)?,
        HostHandoffAdmission::ResultReady
    );
    let expired = take_required_result(&mut kernel);
    assert_rejection(
        expired.clone(),
        ExpectedAdmissionRejection::LeaseExpired,
        RequestKind::Launch,
    );
    assert_eq!(
        kernel.admit_launch(request, NOW - 1)?,
        HostOperationAdmission::ResultReady
    );
    assert_eq!(take_required_result(&mut kernel), expired);
    Ok(())
}

#[test]
fn start_handoff_and_completion_preserve_exact_target_host_identity() -> TestResult {
    let mut identity_kernel = kernel()?;
    let target = host_identity(5, 6, 7)?;
    let changed_epoch_request = start(AuthorizationSpec::standard(1, 1), target, geometry(80)?)?;
    let replacement_epoch = host_epoch(8)?;
    assert_eq!(
        identity_kernel.admit_start(changed_epoch_request, NOW, replacement_epoch)?,
        HostOperationAdmission::ResultReady
    );
    let AuthorizedHostOperationResult::Start(pre_admission_change) =
        take_required_result(&mut identity_kernel)
    else {
        return Err("expected pre-admission Start identity-change result".into());
    };
    assert_eq!(pre_admission_change.target(), target);
    assert_eq!(
        pre_admission_change.outcome(),
        TerminalStartOutcome::Rejected(TerminalStartRejection::HostIdentityChanged {
            observed_host_epoch: replacement_epoch,
        })
    );

    let changed_epoch_dispatch = expect_dispatch(admit_start_at_target(
        &mut identity_kernel,
        changed_epoch_request,
        NOW,
    )?);
    assert_eq!(
        identity_kernel.begin_host_handoff(&changed_epoch_dispatch, NOW, replacement_epoch)?,
        HostHandoffAdmission::ResultReady
    );
    let raced_change = take_required_result(&mut identity_kernel);
    let AuthorizedHostOperationResult::Start(changed_epoch_result) = raced_change.clone() else {
        return Err("expected post-admission Start identity-change result".into());
    };
    assert_eq!(changed_epoch_result.target(), target);
    assert_eq!(
        changed_epoch_result.outcome(),
        TerminalStartOutcome::Rejected(TerminalStartRejection::HostIdentityChanged {
            observed_host_epoch: replacement_epoch,
        })
    );
    assert_eq!(
        admit_start_at_target(&mut identity_kernel, changed_epoch_request, LIVE_EXPIRY,)?,
        HostOperationAdmission::ResultReady
    );
    assert_eq!(take_required_result(&mut identity_kernel), raced_change);

    let mut completion_kernel = kernel()?;
    let accepted_target = host_identity(10, 6, 11)?;
    let accepted_request = start(
        AuthorizationSpec::standard(2, 2),
        accepted_target,
        geometry(80)?,
    )?;
    let accepted_dispatch = expect_dispatch(admit_start_at_target(
        &mut completion_kernel,
        accepted_request,
        NOW,
    )?);
    completion_kernel.begin_host_handoff(&accepted_dispatch, NOW, accepted_target.host_epoch())?;
    let contradictory = HostStartAccepted::new(
        host_identity(12, 6, 13)?,
        geometry(80)?,
        StartTransition::Started,
    );
    assert_eq!(
        completion_kernel.record_start_completion(
            &accepted_dispatch,
            StartHostCompletion::Accepted(contradictory)
        ),
        Err(AuthorizationKernelError::Protocol(
            ProtocolError::ContradictoryHostObservation
        ))
    );
    completion_kernel.record_outcome_unknown(&accepted_dispatch)?;
    let AuthorizedHostOperationResult::Start(unknown) =
        take_required_result(&mut completion_kernel)
    else {
        return Err("expected Start unknown result".into());
    };
    assert_eq!(unknown.target(), accepted_target);
    assert_eq!(unknown.outcome(), TerminalStartOutcome::OutcomeUnknown);
    Ok(())
}

#[test]
fn start_completion_rejects_changed_geometry_without_consuming_handoff() -> TestResult {
    let mut kernel = kernel()?;
    let target = host_identity(14, 6, 15)?;
    let request = start(AuthorizationSpec::standard(3, 3), target, geometry(80)?)?;
    let dispatch = expect_dispatch(admit_start_at_target(&mut kernel, request, NOW)?);
    kernel.begin_host_handoff(&dispatch, NOW, target.host_epoch())?;

    let contradictory =
        HostStartAccepted::new(target, geometry(81)?, StartTransition::AlreadyStarted);
    assert_eq!(
        kernel.record_start_completion(&dispatch, StartHostCompletion::Accepted(contradictory)),
        Err(AuthorizationKernelError::Protocol(
            ProtocolError::ContradictoryHostObservation
        ))
    );
    assert_eq!(kernel.take_result(), None);

    let accepted =
        HostStartAccepted::new(target, request.geometry(), StartTransition::AlreadyStarted);
    kernel.record_start_completion(&dispatch, StartHostCompletion::Accepted(accepted))?;
    assert_eq!(
        take_required_result(&mut kernel),
        AuthorizedHostOperationResult::Start(TerminalStartResult::new(
            request.authorization(),
            target,
            TerminalStartOutcome::HostAccepted(accepted),
        )?)
    );
    Ok(())
}

#[test]
fn rejects_zero_local_site_worker_identity_at_the_kernel_boundary() {
    let result = AuthorizedHostOperationKernel::new(
        LocalSiteWorkerId::from_bytes([0; 16]),
        NonZeroU64::MIN,
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    );
    assert_eq!(result, Err(ProtocolError::InvalidIdentity));
}

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn kernel() -> Result<AuthorizedHostOperationKernel, ProtocolError> {
    AuthorizedHostOperationKernel::new(
        CURRENT_LOCAL_WORKER_ID,
        nonzero_u64(7)?,
        TERMINAL_LAUNCH_CAPABILITY_VERSION,
    )
}

fn operation(
    kind: RequestKind,
    spec: AuthorizationSpec,
) -> Result<AuthorizedHostOperation, ProtocolError> {
    match kind {
        RequestKind::Launch => launch(spec).map(AuthorizedHostOperation::Launch),
        RequestKind::Start => {
            start(spec, host_identity(5, 6, 7)?, geometry(80)?).map(AuthorizedHostOperation::Start)
        }
    }
}

fn admit(
    kernel: &mut AuthorizedHostOperationKernel,
    operation: AuthorizedHostOperation,
    now_unix_milliseconds: u64,
) -> Result<HostOperationAdmission, AuthorizationKernelError> {
    match operation {
        AuthorizedHostOperation::Launch(launch) => {
            kernel.admit_launch(launch, now_unix_milliseconds)
        }
        AuthorizedHostOperation::Start(start) => {
            admit_start_at_target(kernel, start, now_unix_milliseconds)
        }
    }
}

fn authorization(spec: AuthorizationSpec) -> Result<TerminalOperationAuthorization, ProtocolError> {
    Ok(TerminalOperationAuthorization::new(
        TerminalId::from_bytes([spec.terminal; 16])?,
        OperationId::from_bytes([spec.operation; 16])?,
        WorkerId::from_bytes([spec.worker; 16])?,
        WorkerGeneration::new(nonzero_u64(spec.generation)?),
        TerminalFence::new(nonzero_u64(spec.fence)?),
        TerminalOperationLease::new(
            LeaseId::from_bytes([spec.lease; 16])?,
            LeaseExpiresAtUnixMilliseconds::new(nonzero_u64(spec.expiry)?),
        ),
        TerminalLaunchCapabilityVersion::new(nonzero_u16(spec.capability)?),
    ))
}

fn admit_start_at_target(
    kernel: &mut AuthorizedHostOperationKernel,
    start: AuthorizedTerminalStart,
    now_unix_milliseconds: u64,
) -> Result<HostOperationAdmission, AuthorizationKernelError> {
    kernel.admit_start(start, now_unix_milliseconds, start.identity().host_epoch())
}

fn launch(spec: AuthorizationSpec) -> Result<AuthorizedTerminalLaunch, ProtocolError> {
    launch_with(spec, None, TerminalLaunchMode::Deferred)
}

fn launch_with(
    spec: AuthorizationSpec,
    cwd: Option<&str>,
    mode: TerminalLaunchMode,
) -> Result<AuthorizedTerminalLaunch, ProtocolError> {
    Ok(AuthorizedTerminalLaunch::new(
        authorization(spec)?,
        cwd.map(AbsoluteCwd::new).transpose()?,
        mode,
    ))
}

fn start(
    spec: AuthorizationSpec,
    target: HostTerminalIdentity,
    geometry: TerminalPixelGeometry,
) -> Result<AuthorizedTerminalStart, ProtocolError> {
    Ok(AuthorizedTerminalStart::new(
        authorization(spec)?,
        target,
        geometry,
    ))
}

fn host_identity(
    creation: u8,
    epoch: u8,
    terminal: u8,
) -> Result<HostTerminalIdentity, ProtocolError> {
    Ok(HostTerminalIdentity::new(
        HostCreationId::from_bytes([creation; 16])?,
        host_epoch(epoch)?,
        HostTerminalId::from_bytes([terminal; 16])?,
    ))
}

fn host_epoch(value: u8) -> Result<HostEpoch, ProtocolError> {
    HostEpoch::from_bytes([value; 16])
}

fn geometry(columns: u16) -> Result<TerminalPixelGeometry, ProtocolError> {
    TerminalPixelGeometry::new(columns, 24, 8, 16)
}

fn nonzero_u16(value: u16) -> Result<NonZeroU16, ProtocolError> {
    NonZeroU16::new(value).ok_or(ProtocolError::InvalidPayload)
}

fn nonzero_u64(value: u64) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(value).ok_or(ProtocolError::InvalidPayload)
}

fn expect_dispatch(admission: HostOperationAdmission) -> AuthorizedHostDispatch {
    let HostOperationAdmission::Dispatch(dispatch) = admission else {
        panic!("expected an authorized Host dispatch, received {admission:?}");
    };
    dispatch
}

fn take_required_result(
    kernel: &mut AuthorizedHostOperationKernel,
) -> AuthorizedHostOperationResult {
    let Some(result) = kernel.take_result() else {
        panic!("expected one pending authorized Host-operation result");
    };
    result
}

fn assert_rejection(
    result: AuthorizedHostOperationResult,
    expected: ExpectedAdmissionRejection,
    request_kind: RequestKind,
) {
    match (request_kind, result) {
        (RequestKind::Launch, AuthorizedHostOperationResult::Launch(result)) => {
            let TerminalLaunchOutcome::Rejected(actual) = result.outcome() else {
                panic!("expected Launch rejection, received {result:?}");
            };
            assert_eq!(actual, launch_rejection(expected));
        }
        (RequestKind::Start, AuthorizedHostOperationResult::Start(result)) => {
            let TerminalStartOutcome::Rejected(actual) = result.outcome() else {
                panic!("expected Start rejection, received {result:?}");
            };
            assert_eq!(actual, start_rejection(expected));
        }
        (_, result) => panic!("result kind did not match request kind: {result:?}"),
    }
}

fn assert_result_binding(
    result: &AuthorizedHostOperationResult,
    operation: &AuthorizedHostOperation,
) {
    match (result, operation) {
        (
            AuthorizedHostOperationResult::Launch(result),
            AuthorizedHostOperation::Launch(launch),
        ) => assert_eq!(result.authorization(), launch.authorization()),
        (AuthorizedHostOperationResult::Start(result), AuthorizedHostOperation::Start(start)) => {
            assert_eq!(result.authorization(), start.authorization());
            assert_eq!(result.target(), start.identity());
        }
        _ => panic!("result kind did not match its admitted operation"),
    }
}

const fn launch_rejection(expected: ExpectedAdmissionRejection) -> TerminalLaunchRejection {
    match expected {
        ExpectedAdmissionRejection::WrongWorker => TerminalLaunchRejection::WrongWorker,
        ExpectedAdmissionRejection::StaleWorkerGeneration => {
            TerminalLaunchRejection::StaleWorkerGeneration
        }
        ExpectedAdmissionRejection::LeaseExpired => TerminalLaunchRejection::LeaseExpired,
        ExpectedAdmissionRejection::StaleFence => TerminalLaunchRejection::StaleFence,
        ExpectedAdmissionRejection::ConflictingReplay => TerminalLaunchRejection::ConflictingReplay,
        ExpectedAdmissionRejection::UnsupportedCapability => {
            TerminalLaunchRejection::UnsupportedCapability
        }
    }
}

const fn start_rejection(expected: ExpectedAdmissionRejection) -> TerminalStartRejection {
    match expected {
        ExpectedAdmissionRejection::WrongWorker => TerminalStartRejection::WrongWorker,
        ExpectedAdmissionRejection::StaleWorkerGeneration => {
            TerminalStartRejection::StaleWorkerGeneration
        }
        ExpectedAdmissionRejection::LeaseExpired => TerminalStartRejection::LeaseExpired,
        ExpectedAdmissionRejection::StaleFence => TerminalStartRejection::StaleFence,
        ExpectedAdmissionRejection::ConflictingReplay => TerminalStartRejection::ConflictingReplay,
        ExpectedAdmissionRejection::UnsupportedCapability => {
            TerminalStartRejection::UnsupportedCapability
        }
    }
}

#[test]
fn cached_failure_variants_remain_distinct() -> TestResult {
    let mut launch_kernel = kernel()?;
    let launch = launch(AuthorizationSpec::standard(1, 1))?;
    let launch_dispatch = expect_dispatch(launch_kernel.admit_launch(launch, NOW)?);
    launch_kernel.record_pre_handoff_failure(&launch_dispatch, NOW)?;
    let AuthorizedHostOperationResult::Launch(launch_result) =
        take_required_result(&mut launch_kernel)
    else {
        return Err("expected Launch failure".into());
    };
    assert_eq!(
        launch_result.outcome(),
        TerminalLaunchOutcome::Failed(TerminalLaunchFailure::HostUnavailableBeforeHandoff)
    );

    let mut start_kernel = kernel()?;
    let start = start(
        AuthorizationSpec::standard(2, 2),
        host_identity(5, 6, 7)?,
        geometry(80)?,
    )?;
    let start_dispatch = expect_dispatch(admit_start_at_target(&mut start_kernel, start, NOW)?);
    start_kernel.record_pre_handoff_failure(&start_dispatch, NOW)?;
    let AuthorizedHostOperationResult::Start(start_result) =
        take_required_result(&mut start_kernel)
    else {
        return Err("expected Start failure".into());
    };
    assert_eq!(
        start_result.outcome(),
        TerminalStartOutcome::Failed(TerminalStartFailure::HostUnavailableBeforeHandoff)
    );
    Ok(())
}
