use super::*;
use crate::site_coordinator::BindingViewerPreparationError;
use orchbee_client_server_protocol::{
    SiteId as ClientSiteId, TerminalId as ClientTerminalId, WorkerId as ClientWorkerId,
};
use orchbee_workbench_protocol::{PresentationBinding, TerminalId, WorkerId as WorkbenchWorkerId};

#[test]
fn accepted_close_fences_both_attach_forms_and_start_before_identity_allocation() {
    use crate::site_coordinator::TargetLifecycle;
    use orchbee_client_server_protocol::{StartOutcome, StartRejectedReason, TerminalStartRequest};
    let (mut coordinator, authorization) = prepared_site(true);
    let selected = selection(&coordinator, authorization);
    let target = coordinator.viewer_target(&id(1), worker(1)).unwrap();
    let (_close_id, _runtime, _now) = accept_target_close(&mut coordinator, authorization);
    assert_eq!(
        coordinator.target_lifecycle(target),
        TargetLifecycle::Fenced
    );
    assert_eq!(coordinator.viewer_target(&id(1), worker(1)), None);
    assert_eq!(
        coordinator.prepare_binding_viewer(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1))
        ),
        Err(BindingViewerPreparationError::TerminalUnavailable)
    );
    assert!(matches!(
        coordinator.prepare(ClientMessage::TerminalStart(TerminalStartRequest::new(id(20), id(1), geometry()).unwrap()), Some(worker(1)), Some(expiry())),
        PreparedOperation::Ready(ServerMessage::TerminalStartResult(result))
            if result.outcome() == StartOutcome::Rejected { reason: StartRejectedReason::TerminalCreateUnavailable }
    ));
    assert_eq!(coordinator.allocate_identity(), Some(2_u128.to_be_bytes()));
}

#[test]
fn disposed_attachment_proof_is_exact_and_primary_cleanup_keeps_its_accepted_fence() {
    use crate::site_coordinator::TargetLifecycle;
    use orchbee_capability_node_protocol::{
        LifecycleStepOrdinal, OperationId, TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
        TerminalLifecycleAction, TerminalLifecycleAuthorization, TerminalLifecycleOutcome,
        TerminalLifecycleRequest, TerminalLifecycleResult,
    };
    use orchbee_server::{
        CloseContinuationNext, CloseStepObservation, WorkbenchTabCloseRuntimeTarget,
    };
    let (mut coordinator, authorization) = prepared_site(true);
    let target = coordinator.viewer_target(&id(1), worker(1)).unwrap();
    let (close_id, runtime, now) = accept_target_close(&mut coordinator, authorization);
    let authority = coordinator.workbench.as_mut().unwrap();
    assert!(matches!(
        authority
            .next_terminal_tab_close_step(&close_id, now)
            .unwrap(),
        CloseContinuationNext::Lifecycle {
            action: TerminalLifecycleAction::Dispose,
            ..
        }
    ));
    let request = TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: runtime.terminal_id(),
            worker_id: runtime.worker_id(),
            worker_generation: runtime.worker_generation(),
            binding_fence: runtime.fence(),
            lease: authorization.lease(),
        },
        close_operation_id: OperationId::from_bytes(50_u128.to_be_bytes()).unwrap(),
        request_id: OperationId::from_bytes(51_u128.to_be_bytes()).unwrap(),
        attempt_generation: NonZeroU64::MIN,
        step_ordinal: LifecycleStepOrdinal::new(1).unwrap(),
        target: runtime.host_identity(),
        action: TerminalLifecycleAction::Dispose,
    };
    let token = authority
        .reserve_terminal_tab_close_step(&close_id, now, request)
        .unwrap();
    authority
        .confirm_terminal_tab_close_step_admitted(&close_id, &token, now)
        .unwrap();
    let result = TerminalLifecycleResult::new(request, TerminalLifecycleOutcome::Disposed).unwrap();
    authority
        .settle_terminal_tab_close_step(
            &close_id,
            token,
            now,
            CloseStepObservation::Result(&result),
        )
        .unwrap();
    assert_eq!(
        coordinator.target_lifecycle(target),
        TargetLifecycle::Disposed
    );
    assert!(coordinator.is_disposed_close_target(&close_id, runtime));
    let wrong_fence = WorkbenchTabCloseRuntimeTarget::new(
        runtime.worker_id(),
        runtime.worker_generation(),
        runtime.terminal_id(),
        runtime.host_identity(),
        orchbee_capability_node_protocol::TerminalFence::new(NonZeroU64::new(2).unwrap()),
    );
    assert!(!coordinator.is_disposed_close_target(&close_id, wrong_fence));
    let wrong_creation = crate::terminal_coordinator::BindingViewerTarget {
        identity: HostTerminalIdentity::new(
            HostCreationId::from_bytes([7; 16]).unwrap(),
            target.identity.host_epoch(),
            target.identity.terminal_id(),
        ),
        ..target
    };
    let wrong_generation = crate::terminal_coordinator::BindingViewerTarget {
        worker: worker(2),
        ..target
    };
    for wrong in [wrong_creation, wrong_generation] {
        assert_eq!(coordinator.target_lifecycle(wrong), TargetLifecycle::Fenced);
    }
    // The Server service/maintenance tests separately exercise the local controller cleanup consumer.
}

fn accept_target_close(
    coordinator: &mut SiteCoordinator,
    authorization: TerminalOperationAuthorization,
) -> (
    WorkbenchRequestId,
    orchbee_server::WorkbenchTabCloseRuntimeTarget,
    std::time::Instant,
) {
    use orchbee_capability_node_protocol::LifecycleTerminalPhase;
    use orchbee_server::{
        WorkbenchTabCloseBegin, WorkbenchTabClosePreparation, WorkbenchTabCloseResolution,
    };
    use orchbee_workbench_protocol::{WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome};
    let before = snapshot(coordinator);
    let authority = coordinator.workbench.as_mut().unwrap();
    let membership = authority.standalone_membership().unwrap();
    let member = membership.members().first().unwrap();
    let (runtime, phase) = SiteCoordinator::terminal_tab_close_runtime(
        &coordinator.terminals,
        member.binding(),
        Some(worker(1)),
    )
    .unwrap();
    assert_eq!(phase, None);
    assert_eq!(runtime.fence(), authorization.fence());
    let close_id = WorkbenchRequestId::parse(&format!("{:032x}", 50)).unwrap();
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: close_id.clone(),
        site_id: before.site_id,
        expected_revision: before.revision,
        action: WorkbenchAction::TabClose {
            terminal_context_id: membership.terminal_context_id().clone(),
            tab_id: member.tab_id().clone(),
        },
    };
    let grant = WorkbenchGrant::read_only(authority.site_id().clone()).with_terminal_lifecycle();
    let WorkbenchTabCloseBegin::Admitted(admission) =
        authority.begin_terminal_tab_close(Some(&grant), command)
    else {
        panic!("Close admission")
    };
    let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
    else {
        panic!("primary Close")
    };
    let now = std::time::Instant::now();
    let WorkbenchTabClosePreparation::Prepared(prepared) =
        primary.finish(runtime, Some(LifecycleTerminalPhase::AwaitingStart), now)
    else {
        panic!("Close preparation")
    };
    assert!(matches!(
        prepared.commit().outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    (close_id, runtime, now)
}

struct Selection {
    site_id: ClientSiteId,
    worker_id: ClientWorkerId,
    terminal_id: ClientTerminalId,
}

fn selection(
    coordinator: &SiteCoordinator,
    authorization: TerminalOperationAuthorization,
) -> Selection {
    Selection {
        site_id: ClientSiteId::parse(coordinator.workbench.as_ref().unwrap().site_id().as_str())
            .unwrap(),
        worker_id: ClientWorkerId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(authorization.worker_id().as_bytes())
        ))
        .unwrap(),
        terminal_id: ClientTerminalId::parse(&format!(
            "{:032x}",
            u128::from_be_bytes(authorization.terminal_id().as_bytes())
        ))
        .unwrap(),
    }
}

fn expose(coordinator: &mut SiteCoordinator, authorization: TerminalOperationAuthorization) {
    coordinator
        .workbench
        .as_mut()
        .unwrap()
        .commit_standalone_intent(PresentationBinding {
            terminal_id: TerminalId::parse(&format!(
                "{:032x}",
                u128::from_be_bytes(authorization.terminal_id().as_bytes())
            ))
            .unwrap(),
            worker_id: WorkbenchWorkerId::parse(&format!(
                "{:032x}",
                u128::from_be_bytes(authorization.worker_id().as_bytes())
            ))
            .unwrap(),
        })
        .unwrap();
}

fn prepared_site(settle: bool) -> (SiteCoordinator, TerminalOperationAuthorization) {
    let mut coordinator = site();
    let pending = dispatch(coordinator.prepare(create(1), Some(worker(1)), Some(expiry())));
    let authorization = launch_authorization(&pending);
    expose(&mut coordinator, authorization);
    if settle {
        coordinator
            .complete(
                pending,
                Completion::Launch(
                    TerminalLaunchResult::new(
                        authorization,
                        TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                            identity(),
                            HostLaunchState::AwaitingStart,
                        )),
                    )
                    .unwrap(),
                ),
                Some(worker(1)),
            )
            .unwrap();
    }
    (coordinator, authorization)
}

fn prepared_started_site() -> (SiteCoordinator, TerminalOperationAuthorization) {
    use orchbee_capability_node_protocol::{HostStartAccepted, StartTransition};
    let (mut coordinator, _authorization) = prepared_site(true);
    let pending = dispatch(
        coordinator.prepare(
            ClientMessage::TerminalStart(
                orchbee_client_server_protocol::TerminalStartRequest::new(id(9), id(1), geometry())
                    .unwrap(),
            ),
            Some(worker(1)),
            Some(expiry()),
        ),
    );
    let CapabilityNodeMessage::AuthorizedTerminalStart(request) = pending.command() else {
        panic!("start expected");
    };
    let start_authorization = request.authorization();
    let start_geometry = request.geometry();
    coordinator
        .complete(
            pending,
            Completion::Start(
                orchbee_capability_node_protocol::TerminalStartResult::new(
                    start_authorization,
                    identity(),
                    orchbee_capability_node_protocol::TerminalStartOutcome::HostAccepted(
                        HostStartAccepted::new(
                            identity(),
                            start_geometry,
                            StartTransition::Started,
                        ),
                    ),
                )
                .unwrap(),
            ),
            Some(worker(1)),
        )
        .unwrap();
    (coordinator, start_authorization)
}

#[test]
fn exact_exposed_binding_prepares_the_retained_host_without_changing_workbench() {
    let (mut coordinator, authorization) = prepared_site(true);
    let selected = selection(&coordinator, authorization);
    let before = snapshot(&mut coordinator);
    let prepared = coordinator
        .prepare_binding_viewer(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1)),
        )
        .unwrap();

    assert_eq!(prepared.target.identity, identity());
    assert_eq!(prepared.target.worker, worker(1));
    assert_eq!(
        prepared.identities.attachment.as_bytes(),
        2_u128.to_be_bytes()
    );
    assert_eq!(prepared.identities.request.as_bytes(), 3_u128.to_be_bytes());
    assert_eq!(prepared.identities.cleanup.as_bytes(), 4_u128.to_be_bytes());
    assert_eq!(snapshot(&mut coordinator), before);
    assert_eq!(coordinator.allocate_identity(), Some(5_u128.to_be_bytes()));
}

#[test]
fn workbench_refusals_precede_runtime_interpretation_and_consume_no_identity() {
    let (mut coordinator, authorization) = prepared_site(true);
    let selected = selection(&coordinator, authorization);
    let wrong_site = ClientSiteId::parse(&format!("{:032x}", 9_001)).unwrap();
    let wrong_worker = ClientWorkerId::parse(&"08".repeat(16)).unwrap();
    let wrong_terminal = ClientTerminalId::parse(&"07".repeat(16)).unwrap();
    for (site_id, worker_id, terminal_id) in [
        (&wrong_site, &selected.worker_id, &selected.terminal_id),
        (&selected.site_id, &wrong_worker, &selected.terminal_id),
        (&selected.site_id, &selected.worker_id, &wrong_terminal),
    ] {
        assert_eq!(
            coordinator.prepare_binding_viewer(site_id, worker_id, terminal_id, None),
            Err(BindingViewerPreparationError::TerminalUnavailable)
        );
    }
    assert_eq!(coordinator.allocate_identity(), Some(2_u128.to_be_bytes()));
}

#[test]
fn retained_terminal_not_exposed_by_workbench_is_unavailable_without_consuming_identity() {
    let (mut coordinator, _exposed) = prepared_site(true);
    let hidden = dispatch(coordinator.prepare(create(2), Some(worker(1)), Some(expiry())));
    let hidden_authorization = launch_authorization(&hidden);
    coordinator
        .complete(
            hidden,
            Completion::Launch(
                TerminalLaunchResult::new(
                    hidden_authorization,
                    TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                        identity(),
                        HostLaunchState::AwaitingStart,
                    )),
                )
                .unwrap(),
            ),
            Some(worker(1)),
        )
        .unwrap();
    let selected = selection(&coordinator, hidden_authorization);
    let before = snapshot(&mut coordinator);

    assert_eq!(
        coordinator.prepare_binding_viewer(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1)),
        ),
        Err(BindingViewerPreparationError::TerminalUnavailable)
    );
    assert_eq!(snapshot(&mut coordinator), before);
    assert_eq!(coordinator.allocate_identity(), Some(3_u128.to_be_bytes()));
}

#[test]
fn missing_workbench_and_current_worker_have_distinct_precedence() {
    let selected = Selection {
        site_id: ClientSiteId::parse(&format!("{:032x}", 91)).unwrap(),
        worker_id: ClientWorkerId::parse(&"09".repeat(16)).unwrap(),
        terminal_id: ClientTerminalId::parse(&"07".repeat(16)).unwrap(),
    };
    let mut missing = SiteCoordinator::new(None);
    assert_eq!(
        missing.prepare_binding_viewer(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            None,
        ),
        Err(BindingViewerPreparationError::WorkbenchUnavailable)
    );
    assert_eq!(missing.allocate_identity(), Some(1_u128.to_be_bytes()));

    let (mut coordinator, authorization) = prepared_site(true);
    let selected = selection(&coordinator, authorization);
    assert_eq!(
        coordinator.prepare_binding_viewer(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            None,
        ),
        Err(BindingViewerPreparationError::TerminalRuntimeUnavailable)
    );
    assert_eq!(coordinator.allocate_identity(), Some(2_u128.to_be_bytes()));
}

#[test]
fn exposed_binding_requires_an_exact_retained_record_and_generation_before_phase() {
    let (mut creating, authorization) = prepared_site(false);
    let selected = selection(&creating, authorization);
    assert_eq!(
        creating.prepare_binding_viewer(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(2)),
        ),
        Err(BindingViewerPreparationError::TerminalRuntimeUnavailable)
    );
    assert_eq!(
        creating.prepare_binding_viewer(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1)),
        ),
        Err(BindingViewerPreparationError::TerminalUnavailable)
    );
    assert_eq!(creating.allocate_identity(), Some(2_u128.to_be_bytes()));

    let mut missing = site();
    let arbitrary = TerminalOperationAuthorization::new(
        orchbee_capability_node_protocol::TerminalId::from_bytes([7; 16]).unwrap(),
        orchbee_capability_node_protocol::OperationId::from_bytes([6; 16]).unwrap(),
        worker(1).worker_id,
        worker(1).generation,
        orchbee_capability_node_protocol::TerminalFence::new(NonZeroU64::MIN),
        orchbee_capability_node_protocol::TerminalOperationLease::new(
            orchbee_capability_node_protocol::LeaseId::from_bytes([5; 16]).unwrap(),
            expiry(),
        ),
        orchbee_capability_node_protocol::TERMINAL_LAUNCH_CAPABILITY_VERSION,
    );
    expose(&mut missing, arbitrary);
    let selected = selection(&missing, arbitrary);
    assert_eq!(
        missing.prepare_binding_viewer(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1)),
        ),
        Err(BindingViewerPreparationError::TerminalUnavailable)
    );
    assert_eq!(missing.allocate_identity(), Some(1_u128.to_be_bytes()));
}

#[test]
fn viewer_identity_exhaustion_is_server_busy_and_preserves_the_cursor() {
    let (mut coordinator, authorization) = prepared_site(true);
    let selected = selection(&coordinator, authorization);
    coordinator
        .terminals
        .set_next_identity_for_test(Some(u128::MAX - 1));
    assert_eq!(
        coordinator.prepare_binding_viewer(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1)),
        ),
        Err(BindingViewerPreparationError::ServerBusy)
    );
    assert_eq!(
        coordinator.allocate_identity(),
        Some((u128::MAX - 1).to_be_bytes())
    );
}

#[test]
fn send_prepares_pinned_started_target_with_two_distinct_identities() {
    let (mut coordinator, authorization) = prepared_started_site();
    let selected = selection(&coordinator, authorization);
    let before = snapshot(&mut coordinator);
    coordinator.terminals.set_next_identity_for_test(Some(100));
    let prepared = coordinator
        .prepare_send_target(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1)),
        )
        .unwrap();
    assert_eq!(prepared.target.identity, identity());
    assert_eq!(prepared.target.worker, worker(1));
    assert_ne!(
        prepared.identities.operation,
        prepared.identities.attachment
    );
    assert_eq!(
        prepared.identities.operation.as_bytes(),
        100_u128.to_be_bytes()
    );
    assert_eq!(
        prepared.identities.attachment.as_bytes(),
        101_u128.to_be_bytes()
    );
    assert_eq!(snapshot(&mut coordinator), before);
    let second = coordinator
        .prepare_send_target(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1)),
        )
        .unwrap();
    assert_eq!(
        second.identities.operation.as_bytes(),
        102_u128.to_be_bytes()
    );
    assert_eq!(
        coordinator.allocate_identity(),
        Some(104_u128.to_be_bytes())
    );
}

#[test]
fn send_rejects_awaiting_start_binding_where_viewer_would_admit() {
    let (mut coordinator, authorization) = prepared_site(true);
    let selected = selection(&coordinator, authorization);
    assert_eq!(
        coordinator.prepare_send_target(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1)),
        ),
        Err(BindingViewerPreparationError::TerminalUnavailable)
    );
    assert_eq!(coordinator.allocate_identity(), Some(2_u128.to_be_bytes()));
    assert!(
        coordinator
            .prepare_binding_viewer(
                &selected.site_id,
                &selected.worker_id,
                &selected.terminal_id,
                Some(worker(1)),
            )
            .is_ok()
    );
    assert_eq!(coordinator.allocate_identity(), Some(6_u128.to_be_bytes()));
}

#[test]
fn send_rejects_unexposed_wrong_site_and_missing_workbench_without_identity() {
    let (mut coordinator, authorization) = prepared_started_site();
    let selected = selection(&coordinator, authorization);
    let wrong_site = ClientSiteId::parse(&format!("{:032x}", 9_001)).unwrap();
    let wrong_worker = ClientWorkerId::parse(&"08".repeat(16)).unwrap();
    let wrong_terminal = ClientTerminalId::parse(&"07".repeat(16)).unwrap();
    for (site_id, worker_id, terminal_id) in [
        (&wrong_site, &selected.worker_id, &selected.terminal_id),
        (&selected.site_id, &wrong_worker, &selected.terminal_id),
        (&selected.site_id, &selected.worker_id, &wrong_terminal),
    ] {
        assert_eq!(
            coordinator.prepare_send_target(site_id, worker_id, terminal_id, None),
            Err(BindingViewerPreparationError::TerminalUnavailable)
        );
    }
    let hidden = dispatch(coordinator.prepare(create(7), Some(worker(1)), Some(expiry())));
    let hidden_authorization = launch_authorization(&hidden);
    coordinator
        .complete(
            hidden,
            Completion::Launch(
                TerminalLaunchResult::new(
                    hidden_authorization,
                    TerminalLaunchOutcome::HostAccepted(HostLaunchAccepted::new(
                        identity(),
                        HostLaunchState::AwaitingStart,
                    )),
                )
                .unwrap(),
            ),
            Some(worker(1)),
        )
        .unwrap();
    let hidden_selection = selection(&coordinator, hidden_authorization);
    assert_eq!(
        coordinator.prepare_send_target(
            &hidden_selection.site_id,
            &hidden_selection.worker_id,
            &hidden_selection.terminal_id,
            Some(worker(1)),
        ),
        Err(BindingViewerPreparationError::TerminalUnavailable)
    );

    let mut missing = site();
    let arbitrary = TerminalOperationAuthorization::new(
        orchbee_capability_node_protocol::TerminalId::from_bytes([7; 16]).unwrap(),
        orchbee_capability_node_protocol::OperationId::from_bytes([6; 16]).unwrap(),
        worker(1).worker_id,
        worker(1).generation,
        orchbee_capability_node_protocol::TerminalFence::new(NonZeroU64::MIN),
        orchbee_capability_node_protocol::TerminalOperationLease::new(
            orchbee_capability_node_protocol::LeaseId::from_bytes([5; 16]).unwrap(),
            expiry(),
        ),
        orchbee_capability_node_protocol::TERMINAL_LAUNCH_CAPABILITY_VERSION,
    );
    expose(&mut missing, arbitrary);
    let missing_selection = selection(&missing, arbitrary);
    assert_eq!(
        missing.prepare_send_target(
            &missing_selection.site_id,
            &missing_selection.worker_id,
            &missing_selection.terminal_id,
            Some(worker(1)),
        ),
        Err(BindingViewerPreparationError::TerminalUnavailable)
    );
}

#[test]
fn send_rejects_accepted_close_and_stale_or_missing_worker() {
    let (mut coordinator, authorization) = prepared_started_site();
    let selected = selection(&coordinator, authorization);
    for placement in [None, Some(worker(2))] {
        assert_eq!(
            coordinator.prepare_send_target(
                &selected.site_id,
                &selected.worker_id,
                &selected.terminal_id,
                placement,
            ),
            Err(BindingViewerPreparationError::TerminalRuntimeUnavailable)
        );
    }
    let target = coordinator.viewer_target(&id(1), worker(1)).unwrap();
    let (_close_id, _runtime, _now) = accept_target_close(&mut coordinator, authorization);
    assert_eq!(
        coordinator.target_lifecycle(target),
        TargetLifecycle::Fenced
    );
    assert_eq!(
        coordinator.prepare_send_target(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1)),
        ),
        Err(BindingViewerPreparationError::TerminalUnavailable)
    );
}

#[test]
fn send_identity_exhaustion_is_server_busy_and_preserves_the_cursor() {
    let (mut coordinator, authorization) = prepared_started_site();
    let selected = selection(&coordinator, authorization);
    coordinator
        .terminals
        .set_next_identity_for_test(Some(u128::MAX));
    assert_eq!(
        coordinator.prepare_send_target(
            &selected.site_id,
            &selected.worker_id,
            &selected.terminal_id,
            Some(worker(1)),
        ),
        Err(BindingViewerPreparationError::ServerBusy)
    );
    assert_eq!(
        coordinator.allocate_identity(),
        Some(u128::MAX.to_be_bytes())
    );
    coordinator
        .terminals
        .set_next_identity_for_test(Some(u128::MAX - 1));
    assert!(
        coordinator
            .prepare_send_target(
                &selected.site_id,
                &selected.worker_id,
                &selected.terminal_id,
                Some(worker(1)),
            )
            .is_ok()
    );
    assert_eq!(coordinator.allocate_identity(), None);
}
