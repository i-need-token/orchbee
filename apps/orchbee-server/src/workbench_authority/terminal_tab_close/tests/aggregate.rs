use super::*;
use orchbee_capability_node_protocol::{
    LifecycleStepOrdinal, LifecycleUnknownReason, TerminalLifecycleAction,
    TerminalLifecycleAuthorization,
};
use orchbee_workbench_protocol::{
    PanelNode, PanelOrientation, PanelSlot, Presentation, TerminalPresentationId, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchSplitId,
};

fn split_authority() -> (WorkbenchAuthority, WorkbenchGrant) {
    let (mut authority, grant) = fixture(1);
    let mut snapshot = authority.projection.snapshot().clone();
    let tab = &mut snapshot.terminal_contexts[0].tabs[0];
    let left = tab.root.clone();
    tab.root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: left,
                slot_id: WorkbenchPanelSlotId::parse(&identity(101)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&identity(102)).unwrap(),
                    primary_presentation: Presentation {
                        binding: Some(binding(701)),
                        label: "right".to_owned(),
                        presentation_id: TerminalPresentationId::parse(&identity(103)).unwrap(),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&identity(104)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&identity(100)).unwrap(),
    };
    authority.projection = WorkbenchProjectionStore::load(
        &orchbee_workbench_protocol::serialize_document(
            &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot),
        )
        .unwrap(),
    )
    .unwrap();
    (authority, grant)
}

fn step_request(
    close_id: &WorkbenchRequestId,
    target: &WorkbenchTabCloseTarget,
    action: TerminalLifecycleAction,
    step_ordinal: LifecycleStepOrdinal,
    generation: WorkbenchTabCloseAttemptGeneration,
) -> TerminalLifecycleRequest {
    let runtime = target.runtime();
    TerminalLifecycleRequest {
        authorization: TerminalLifecycleAuthorization {
            capability_version: TERMINAL_LIFECYCLE_CAPABILITY_VERSION,
            terminal_id: runtime.terminal_id(),
            worker_id: runtime.worker_id(),
            worker_generation: runtime.worker_generation(),
            binding_fence: runtime.fence(),
            lease: TerminalOperationLease::new(
                LeaseId::from_bytes(8000_u128.to_be_bytes()).unwrap(),
                LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::MAX),
            ),
        },
        close_operation_id: OperationId::from_bytes(
            u128::from_str_radix(close_id.as_str(), 16)
                .unwrap()
                .to_be_bytes(),
        )
        .unwrap(),
        request_id: OperationId::from_bytes(
            (8100 + u128::from(runtime.terminal_id().as_bytes()[15])).to_be_bytes(),
        )
        .unwrap(),
        attempt_generation: NonZeroU64::new(generation.value()).unwrap(),
        step_ordinal,
        target: runtime.host_identity(),
        action,
    }
}

fn settle_disposed(
    authority: &mut WorkbenchAuthority,
    close_id: &WorkbenchRequestId,
    step: WorkbenchTabCloseStepTarget,
    now: Instant,
) {
    let target = step.target().expect("targeted lifecycle step");
    let CloseContinuationNext::Lifecycle {
        action,
        step_ordinal,
    } = step.next()
    else {
        panic!("expected lifecycle step");
    };
    let generation = authority.terminal_tab_close_generation(close_id).unwrap();
    let request = step_request(close_id, target, action, step_ordinal, generation);
    let token = authority
        .reserve_terminal_tab_close_step(close_id, now, request)
        .unwrap();
    authority
        .confirm_terminal_tab_close_step_admitted(close_id, &token, now)
        .unwrap();
    let result = TerminalLifecycleResult::new(request, TerminalLifecycleOutcome::Disposed).unwrap();
    authority
        .settle_terminal_tab_close_step(close_id, token, now, CloseStepObservation::Result(&result))
        .unwrap();
}

fn settle_unknown(
    authority: &mut WorkbenchAuthority,
    close_id: &WorkbenchRequestId,
    step: WorkbenchTabCloseStepTarget,
    now: Instant,
) {
    let target = step.target().expect("targeted lifecycle step");
    let CloseContinuationNext::Lifecycle {
        action,
        step_ordinal,
    } = step.next()
    else {
        panic!("expected lifecycle step");
    };
    let generation = authority.terminal_tab_close_generation(close_id).unwrap();
    let request = step_request(close_id, target, action, step_ordinal, generation);
    let token = authority
        .reserve_terminal_tab_close_step(close_id, now, request)
        .unwrap();
    authority
        .confirm_terminal_tab_close_step_admitted(close_id, &token, now)
        .unwrap();
    authority
        .settle_terminal_tab_close_step(
            close_id,
            token,
            now,
            CloseStepObservation::OutcomeUnknown(LifecycleUnknownReason::TransportLost),
        )
        .unwrap();
}

#[test]
fn split_tab_close_routes_each_frozen_leaf_then_publishes_one_tab_removal() {
    let (mut authority, grant) = split_authority();
    let context_id = authority
        .projection
        .snapshot()
        .navigation
        .standalone_terminal_context_id
        .clone();
    let tab_id = authority.projection.snapshot().terminal_contexts[0].tabs[0]
        .tab_id
        .clone();
    let close = command(
        &authority,
        10000,
        WorkbenchAction::TabClose {
            terminal_context_id: context_id.clone(),
            tab_id: tab_id.clone(),
        },
    );
    let now = Instant::now();
    let primary = primary(&mut authority, &grant, close.clone());
    assert_eq!(primary.members().len(), 2);
    let runtimes = primary
        .members()
        .iter()
        .map(|member| (runtime(member), Some(LifecycleTerminalPhase::AwaitingStart)))
        .collect();
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish_members(runtimes, now)
    else {
        panic!("split Close preparation");
    };
    let _accepted = prepared.commit();

    let first = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    let first_binding = first.target().unwrap().member().binding().clone();
    settle_disposed(&mut authority, &close.request_id, first, now);
    let second = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    assert_ne!(second.target().unwrap().member().binding(), &first_binding);
    settle_disposed(&mut authority, &close.request_id, second, now);

    for _ in 0..2 {
        let step = authority
            .next_terminal_tab_close_step_target(&close.request_id, now)
            .unwrap();
        let runtime = step.target().unwrap().runtime();
        assert_eq!(step.next(), CloseContinuationNext::TargetCleanup);
        authority
            .complete_terminal_tab_close_target_cleanup(&close.request_id, runtime)
            .unwrap();
    }
    assert_eq!(
        authority
            .next_terminal_tab_close_step_target(&close.request_id, now)
            .unwrap()
            .next(),
        CloseContinuationNext::Publication
    );
    let closed = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap()
        .commit();
    assert!(matches!(closed, WorkbenchTabCloseStatus::Closed { .. }));
    assert!(
        authority.projection.snapshot().terminal_contexts[0]
            .tabs
            .is_empty()
    );
    assert_eq!(
        authority
            .terminal_tab_close_target_for_binding(&close.request_id, &first_binding)
            .unwrap()
            .member()
            .tab_id(),
        &tab_id
    );
}

#[test]
fn split_tab_close_quarantines_siblings_after_unknown_but_keeps_known_cleanup() {
    let (mut authority, grant) = split_authority();
    let context_id = authority
        .projection
        .snapshot()
        .navigation
        .standalone_terminal_context_id
        .clone();
    let tab_id = authority.projection.snapshot().terminal_contexts[0].tabs[0]
        .tab_id
        .clone();
    let close = command(
        &authority,
        10001,
        WorkbenchAction::TabClose {
            terminal_context_id: context_id,
            tab_id,
        },
    );
    let now = Instant::now();
    let primary = primary(&mut authority, &grant, close.clone());
    let runtimes = primary
        .members()
        .iter()
        .map(|member| (runtime(member), Some(LifecycleTerminalPhase::AwaitingStart)))
        .collect();
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish_members(runtimes, now)
    else {
        panic!("split Close preparation");
    };
    let _ = prepared.commit();

    let first = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    let first_runtime = first.target().unwrap().runtime();
    let first_binding = first.target().unwrap().member().binding().clone();
    settle_unknown(&mut authority, &close.request_id, first, now);
    assert!(matches!(
        authority.terminal_tab_close_status(&close.request_id),
        Some(WorkbenchTabCloseStatus::OutcomeUnknown { .. })
    ));
    assert_eq!(
        authority
            .next_terminal_tab_close_step_target(&close.request_id, now)
            .unwrap()
            .next(),
        CloseContinuationNext::Parked
    );

    let second_binding = authority
        .terminal_tree_membership(
            &authority
                .terminal_tab_close_target(&close.request_id)
                .unwrap()
                .terminal_context_id()
                .clone(),
        )
        .unwrap()
        .members()
        .iter()
        .find(|member| member.binding() != &first_binding)
        .unwrap()
        .binding()
        .clone();
    let second = authority
        .terminal_tab_close_target_for_binding(&close.request_id, &second_binding)
        .unwrap();
    assert_ne!(second.runtime(), first_runtime);
    assert!(matches!(
        authority
            .terminal_tab_close_target_status(&close.request_id, second.runtime())
            .unwrap(),
        WorkbenchTabCloseStatus::Closing { .. }
    ));
}
