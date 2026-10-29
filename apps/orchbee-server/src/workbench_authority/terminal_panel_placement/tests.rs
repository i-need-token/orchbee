use crate::test_support::WorkbenchFixture;
use std::{num::NonZeroU64, time::Instant};

mod swap;

use super::*;
use crate::workbench_authority::{
    CloseContinuationNext, CloseStepObservation, WorkbenchPanelCreateBegin,
    WorkbenchPanelCreatePreparation, WorkbenchReadOrNavigationRequest, WorkbenchTabCloseBegin,
    WorkbenchTabClosePreparation, WorkbenchTabCloseResolution, WorkbenchTabCloseRuntimeTarget,
    WorkbenchTerminalTab, identity,
};
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity,
    LeaseExpiresAtUnixMilliseconds, LeaseId, LifecycleTerminalPhase, OperationId,
    TERMINAL_LIFECYCLE_CAPABILITY_VERSION, TerminalFence, TerminalId as RuntimeTerminalId,
    TerminalLifecycleAuthorization, TerminalLifecycleOutcome, TerminalLifecycleRequest,
    TerminalLifecycleResult, TerminalOperationLease, WorkerGeneration, WorkerId as RuntimeWorkerId,
};
use orchbee_workbench_protocol::{
    PresentationBinding, ProtocolVersion, SessionId, TerminalId, WorkbenchLabel, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome, WorkerId,
};

fn authority() -> (WorkbenchAuthority, WorkbenchGrant) {
    let mut authority =
        WorkbenchAuthority::workspace_fixture(SessionId::parse(&identity(100)).unwrap()).unwrap();
    let grant =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone());
    authority.append_fixture_tab(binding(700)).unwrap();
    (authority, grant)
}

fn binding(value: u128) -> PresentationBinding {
    PresentationBinding {
        worker_id: WorkerId::parse(&identity(2000)).unwrap(),
        terminal_id: TerminalId::parse(&identity(value)).unwrap(),
    }
}

fn command(authority: &WorkbenchAuthority, id: u128, action: WorkbenchAction) -> WorkbenchCommand {
    WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: WorkbenchRequestId::parse(&identity(id)).unwrap(),
        session_id: authority.session_id().clone(),
        expected_revision: authority.projection.snapshot().revision,
        action,
    }
}

fn initial_workspace_context(authority: &WorkbenchAuthority) -> WorkbenchTerminalContextId {
    authority.projection.snapshot().navigation.workspaces[0]
        .terminal_context_id
        .clone()
}

fn submit(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> WorkbenchCommandResult {
    match authority.begin_terminal_panel_placement(Some(grant), command) {
        WorkbenchPanelPlacementBegin::Ready(result) => result,
        WorkbenchPanelPlacementBegin::Admitted(admission) => match admission.finish() {
            WorkbenchPanelPlacementPreparation::Ready(result) => result,
            WorkbenchPanelPlacementPreparation::Prepared(prepared) => prepared.commit(),
        },
    }
}

/// Splits `panel_id` once through the real admission chain; returns nothing but mutates the
/// authority so the Tab gains a fresh leaf in `direction` order.
fn split(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command_id: u128,
    tab_id: &WorkbenchTabId,
    panel_id: &WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
) {
    let WorkbenchPanelCreateBegin::Admitted(admission) = authority.begin_terminal_panel_create(
        Some(grant),
        command(
            authority,
            command_id,
            WorkbenchAction::PanelSplit {
                right_click: orchbee_workbench_protocol::PanelRightClickTarget::Panel,
                terminal_context_id: initial_workspace_context(authority),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                direction,
                label: WorkbenchLabel::parse("split").unwrap(),
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
        ),
    ) else {
        panic!("panel split admission")
    };
    let WorkbenchPanelCreatePreparation::Prepared(prepared) =
        admission.finish(binding(9000 + command_id), None)
    else {
        panic!("panel split preparation")
    };
    let pending = prepared.commit_into_pending();
    let _ = authority
        .prepare_terminal_panel_create_settlement(
            pending,
            WorkbenchTerminalOutcome::Started {
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            },
        )
        .unwrap()
        .commit();
}

fn move_command(
    authority: &WorkbenchAuthority,
    id: u128,
    tab_id: &WorkbenchTabId,
    panel_id: &WorkbenchPanelId,
    target_panel_id: &WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
) -> WorkbenchCommand {
    command(
        authority,
        id,
        WorkbenchAction::PanelMove {
            terminal_context_id: initial_workspace_context(authority),
            tab_id: tab_id.clone(),
            panel_id: panel_id.clone(),
            target_panel_id: target_panel_id.clone(),
            direction,
        },
    )
}

/// Collects every leaf Panel node in DFS order.
fn leaves(node: &PanelNode) -> Vec<PanelNode> {
    match node {
        PanelNode::Panel { .. } => vec![node.clone()],
        PanelNode::Split { children, .. } => children
            .iter()
            .flat_map(|slot| leaves(&slot.node))
            .collect(),
    }
}

fn leaf_panel_id(node: &PanelNode) -> WorkbenchPanelId {
    match node {
        PanelNode::Panel { panel_id, .. } => panel_id.clone(),
        PanelNode::Split { .. } => panic!("expected a leaf Panel"),
    }
}

fn split_ids(node: &PanelNode, output: &mut Vec<WorkbenchSplitId>) {
    if let PanelNode::Split {
        children, split_id, ..
    } = node
    {
        output.push(split_id.clone());
        for slot in children.iter() {
            split_ids(&slot.node, output);
        }
    }
}

fn slot_ids(node: &PanelNode, output: &mut Vec<WorkbenchPanelSlotId>) {
    if let PanelNode::Split { children, .. } = node {
        for slot in children.iter() {
            output.push(slot.slot_id.clone());
            slot_ids(&slot.node, output);
        }
    }
}

#[test]
fn panel_move_relocates_a_leaf_right_with_fresh_graft_identities() {
    let (mut authority, grant) = authority();
    let membership = authority.fixture_tree_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let tab_id = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &tab_id,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let membership = authority.fixture_tree_membership().unwrap();
    let members: Vec<_> = membership.members_for_tab(&tab_id).cloned().collect();
    let panel_b = members
        .iter()
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();

    let before = authority.projection.snapshot().clone();
    let tab = &before.terminal_contexts[0].tabs[0];
    let PanelNode::Split {
        children: before_children,
        orientation: PanelOrientation::Horizontal,
        split_id: before_split_id,
        ..
    } = &tab.root
    else {
        panic!("expected a horizontal Split root")
    };
    assert_eq!(leaf_panel_id(&before_children[0].node), panel_a);
    assert_eq!(leaf_panel_id(&before_children[1].node), panel_b);
    let leaf_a = before_children[0].node.clone();
    let leaf_b = before_children[1].node.clone();
    let last_identity = authority.last_identity;
    let ledger = authority.ledger.len();

    let request = move_command(
        &authority,
        10,
        &tab_id,
        &panel_a,
        &panel_b,
        WorkbenchPanelSplitDirection::Right,
    );
    // panel.move routes through the Session-composed executor, not the pure navigation path.
    assert_eq!(
        WorkbenchReadOrNavigationRequest::try_from(WorkbenchRequest::Command(request.clone()))
            .err(),
        Some(WorkbenchRequest::Command(request.clone()))
    );
    let result = submit(&mut authority, &grant, request.clone());
    let WorkbenchCommandOutcome::PanelMoved {
        revision,
        terminal_context_id,
        tab_id: outcome_tab,
        panel_id: outcome_panel,
        target_panel_id: outcome_target,
    } = &result.outcome
    else {
        panic!("expected panel-moved: {:?}", result.outcome)
    };
    assert_eq!(revision.value(), before.revision.value() + 1);
    assert_eq!(terminal_context_id, &initial_workspace_context(&authority));
    assert_eq!(outcome_tab, &tab_id);
    assert_eq!(outcome_panel, &panel_a);
    assert_eq!(outcome_target, &panel_b);

    let after = authority.projection.snapshot();
    assert_eq!(&after.revision, revision);
    let tab = &after.terminal_contexts[0].tabs[0];
    let PanelNode::Split {
        children,
        orientation: PanelOrientation::Horizontal,
        split_id,
        ..
    } = &tab.root
    else {
        panic!("expected a horizontal Split root")
    };
    // The retained leaves swapped order; both keep their complete node identity.
    assert_eq!(children[0].node, leaf_b);
    assert_eq!(children[1].node, leaf_a);
    assert_ne!(split_id, before_split_id);
    assert_ne!(children[0].slot_id, before_children[0].slot_id);
    assert_ne!(children[0].slot_id, before_children[1].slot_id);
    assert_ne!(children[1].slot_id, before_children[0].slot_id);
    assert_ne!(children[1].slot_id, before_children[1].slot_id);
    assert_ne!(children[0].slot_id, children[1].slot_id);
    // Exactly three identities were consumed: Split + two Slots.
    assert_eq!(authority.last_identity, last_identity + 3);
    // Idempotent replay returns the retained result.
    assert_eq!(submit(&mut authority, &grant, request), result);
    assert_eq!(authority.ledger.len(), ledger + 1);
}

#[test]
fn panel_move_is_unchanged_when_the_placement_is_already_satisfied() {
    let (mut authority, grant) = authority();
    let membership = authority.fixture_tree_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let tab_id = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &tab_id,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let membership = authority.fixture_tree_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&tab_id)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();

    let before = authority.projection.snapshot().clone();
    let last_identity = authority.last_identity;
    for (index, (panel, target, direction)) in [
        (
            panel_a.clone(),
            panel_b.clone(),
            WorkbenchPanelSplitDirection::Left,
        ),
        (
            panel_b.clone(),
            panel_a.clone(),
            WorkbenchPanelSplitDirection::Right,
        ),
        (
            panel_a.clone(),
            panel_a.clone(),
            WorkbenchPanelSplitDirection::Right,
        ),
        (
            panel_b.clone(),
            panel_b.clone(),
            WorkbenchPanelSplitDirection::Above,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let request = move_command(
            &authority,
            10 + index as u128,
            &tab_id,
            &panel,
            &target,
            direction,
        );
        assert_eq!(
            submit(&mut authority, &grant, request).outcome,
            WorkbenchCommandOutcome::Unchanged {
                revision: before.revision
            },
            "{panel} -> {target} {direction:?}"
        );
    }
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, last_identity);
}

#[test]
fn panel_move_rejects_missing_members_and_unknown_scopes() {
    let (mut authority, grant) = authority();
    let membership = authority.fixture_tree_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let tab_id = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &tab_id,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let membership = authority.fixture_tree_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&tab_id)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();
    let missing = WorkbenchPanelId::parse(&identity(424242)).unwrap();
    let missing_tab = WorkbenchTabId::parse(&identity(434343)).unwrap();
    let missing_context = WorkbenchTerminalContextId::parse(&identity(454545)).unwrap();
    let before = authority.projection.snapshot().clone();

    for (index, mut request) in [
        move_command(
            &authority,
            10,
            &tab_id,
            &missing,
            &panel_b,
            WorkbenchPanelSplitDirection::Right,
        ),
        move_command(
            &authority,
            11,
            &tab_id,
            &panel_a,
            &missing,
            WorkbenchPanelSplitDirection::Right,
        ),
        move_command(
            &authority,
            12,
            &missing_tab,
            &panel_a,
            &panel_b,
            WorkbenchPanelSplitDirection::Right,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        request.request_id = WorkbenchRequestId::parse(&identity(10 + index as u128)).unwrap();
        assert!(matches!(
            submit(&mut authority, &grant, request).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::PanelUnavailable
                    | WorkbenchRejectionReason::TabUnavailable,
                ..
            }
        ));
    }
    let mut request = move_command(
        &authority,
        13,
        &tab_id,
        &panel_a,
        &panel_b,
        WorkbenchPanelSplitDirection::Right,
    );
    let WorkbenchAction::PanelMove {
        terminal_context_id,
        ..
    } = &mut request.action
    else {
        unreachable!()
    };
    *terminal_context_id = missing_context;
    assert!(matches!(
        submit(&mut authority, &grant, request).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::TerminalContextUnavailable,
            ..
        }
    ));
    assert_eq!(authority.projection.snapshot(), &before);
    assert!(!authority.retired);
}

#[test]
fn panel_move_restructures_nested_subtrees_and_preserves_untouched_identities() {
    let (mut authority, grant) = authority();
    let membership = authority.fixture_tree_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let tab_id = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &tab_id,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let panel_b = {
        let membership = authority.fixture_tree_membership().unwrap();
        membership
            .members_for_tab(&tab_id)
            .find(|member| member.panel_id() != &panel_a)
            .unwrap()
            .panel_id()
            .clone()
    };
    split(
        &mut authority,
        &grant,
        101,
        &tab_id,
        &panel_b,
        WorkbenchPanelSplitDirection::Right,
    );
    // Tree is now Split(A, Split(B, C)).
    let membership = authority.fixture_tree_membership().unwrap();
    let panel_c = membership
        .members_for_tab(&tab_id)
        .find(|member| member.panel_id() != &panel_a && member.panel_id() != &panel_b)
        .unwrap()
        .panel_id()
        .clone();
    let before = authority.projection.snapshot().clone();
    let PanelNode::Split {
        children: outer,
        split_id: outer_split,
        ..
    } = &before.terminal_contexts[0].tabs[0].root
    else {
        panic!("expected outer Split")
    };
    let PanelNode::Split {
        children: inner, ..
    } = &outer[1].node
    else {
        panic!("expected inner Split")
    };
    let leaf_a = outer[0].node.clone();
    let leaf_b = inner[0].node.clone();
    let leaf_c = inner[1].node.clone();
    assert_eq!(leaf_panel_id(&leaf_c), panel_c);

    // Move C left of A: take C (inner Split collapses to B), graft Split(C, A) at A's slot.
    let request = move_command(
        &authority,
        10,
        &tab_id,
        &panel_c,
        &panel_a,
        WorkbenchPanelSplitDirection::Left,
    );
    let result = submit(&mut authority, &grant, request);
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::PanelMoved { .. }
    ));

    let tab = &authority.projection.snapshot().terminal_contexts[0].tabs[0];
    // Expected: Split( Split(C, A), B ) — outer Split retains its identity, inner collapses away.
    let PanelNode::Split {
        children: new_outer,
        orientation: PanelOrientation::Horizontal,
        split_id: new_outer_split,
        ..
    } = &tab.root
    else {
        panic!("expected horizontal outer Split")
    };
    assert_eq!(new_outer_split, outer_split);
    assert_eq!(new_outer[0].slot_id, outer[0].slot_id);
    assert_eq!(new_outer[1].slot_id, outer[1].slot_id);
    assert_eq!(new_outer[1].node, leaf_b);
    let PanelNode::Split {
        children: graft,
        orientation: PanelOrientation::Horizontal,
        split_id: graft_split,
        ..
    } = &new_outer[0].node
    else {
        panic!("expected grafted Split")
    };
    let mut known_splits = Vec::new();
    split_ids(&before.terminal_contexts[0].tabs[0].root, &mut known_splits);
    assert!(!known_splits.contains(graft_split));
    let mut known_slots = Vec::new();
    slot_ids(&before.terminal_contexts[0].tabs[0].root, &mut known_slots);
    assert!(!known_slots.contains(&graft[0].slot_id));
    assert!(!known_slots.contains(&graft[1].slot_id));
    assert_eq!(graft[0].node, leaf_c);
    assert_eq!(graft[1].node, leaf_a);
    assert_eq!(leaves(&tab.root).len(), 3);
}

#[test]
fn panel_move_without_navigation_grant_is_denied() {
    let (mut authority, _grant) = authority();
    let membership = authority.fixture_tree_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let tab_id = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    let create_grant =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.session_id().clone());
    split(
        &mut authority,
        &create_grant,
        100,
        &tab_id,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let membership = authority.fixture_tree_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&tab_id)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();
    let before = authority.projection.snapshot().clone();
    let read_only = WorkbenchGrant::read_only(authority.session_id().clone());
    let request = move_command(
        &authority,
        10,
        &tab_id,
        &panel_a,
        &panel_b,
        WorkbenchPanelSplitDirection::Right,
    );
    assert_eq!(
        submit(&mut authority, &read_only, request.clone()).outcome,
        WorkbenchCommandOutcome::Denied {}
    );
    assert!(matches!(
        authority.begin_terminal_panel_placement(None, request),
        WorkbenchPanelPlacementBegin::Ready(result) if result.outcome == WorkbenchCommandOutcome::Denied {}
    ));
    assert_eq!(authority.projection.snapshot(), &before);
}

/// The exact retained runtime association a Session owner supplies for `member`, matching the
/// construction used by the Close authority tests.
fn close_runtime(member: &WorkbenchTerminalTab) -> WorkbenchTabCloseRuntimeTarget {
    let terminal = u128::from_str_radix(member.binding().terminal_id.as_str(), 16).unwrap();
    WorkbenchTabCloseRuntimeTarget::new(
        RuntimeWorkerId::from_bytes(2000_u128.to_be_bytes()).unwrap(),
        WorkerGeneration::new(NonZeroU64::MIN),
        RuntimeTerminalId::from_bytes(terminal.to_be_bytes()).unwrap(),
        HostTerminalIdentity::new(
            HostCreationId::from_bytes((terminal + 1000).to_be_bytes()).unwrap(),
            HostEpoch::from_bytes(9000_u128.to_be_bytes()).unwrap(),
            HostTerminalId::from_bytes((terminal + 2000).to_be_bytes()).unwrap(),
        ),
        TerminalFence::new(NonZeroU64::MIN),
    )
}

/// The next lifecycle request the real Close continuation expects, built the same way the
/// service owner builds it.
fn close_step_request(
    authority: &mut WorkbenchAuthority,
    close_id: &WorkbenchRequestId,
    now: Instant,
) -> TerminalLifecycleRequest {
    let CloseContinuationNext::Lifecycle {
        action,
        step_ordinal,
    } = authority
        .next_terminal_tab_close_step(close_id, now)
        .unwrap()
    else {
        panic!("lifecycle step")
    };
    let runtime = authority
        .terminal_tab_close_target(close_id)
        .unwrap()
        .runtime()
        .expect("created Host runtime");
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
        request_id: OperationId::from_bytes((8100 + u128::from(step_ordinal.get())).to_be_bytes())
            .unwrap(),
        attempt_generation: NonZeroU64::new(
            authority
                .terminal_tab_close_generation(close_id)
                .unwrap()
                .value(),
        )
        .unwrap(),
        step_ordinal,
        target: runtime.host_identity(),
        action,
    }
}

/// Drives a real leaf-scoped `panel.close` through admission, lifecycle settlement, target
/// cleanup, and publication so the projection removes `panel_id` and advances the revision.
fn publish_panel_close(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command_id: u128,
    terminal_context_id: &WorkbenchTerminalContextId,
    tab_id: &WorkbenchTabId,
    panel_id: &WorkbenchPanelId,
    now: Instant,
) {
    let WorkbenchTabCloseBegin::Admitted(admission) = authority.begin_terminal_tab_close(
        Some(grant),
        command(
            authority,
            command_id,
            WorkbenchAction::PanelClose {
                terminal_context_id: terminal_context_id.clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
            },
        ),
    ) else {
        panic!("panel close admission")
    };
    let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
    else {
        panic!("panel close resolution")
    };
    let runtime = close_runtime(primary.member());
    let WorkbenchTabClosePreparation::Prepared(prepared) =
        primary.finish(runtime, Some(LifecycleTerminalPhase::AwaitingStart), now)
    else {
        panic!("panel close preparation")
    };
    let result = prepared.commit();
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::PanelCloseAccepted { .. }
    ));
    let close_id = result.request_id.clone();
    let request = close_step_request(authority, &close_id, now);
    let token = authority
        .reserve_terminal_tab_close_step(&close_id, now, request)
        .unwrap();
    authority
        .confirm_terminal_tab_close_step_admitted(&close_id, &token, now)
        .unwrap();
    authority
        .settle_terminal_tab_close_step(
            &close_id,
            token,
            now,
            CloseStepObservation::Result(
                &TerminalLifecycleResult::new(request, TerminalLifecycleOutcome::Disposed).unwrap(),
            ),
        )
        .unwrap();
    let runtime = authority
        .terminal_tab_close_target(&close_id)
        .unwrap()
        .runtime()
        .expect("created Host runtime");
    authority
        .complete_terminal_tab_close_target_cleanup(&close_id, runtime)
        .unwrap();
    let _ = authority
        .prepare_terminal_tab_close_publication(&close_id, now)
        .unwrap()
        .commit();
}

/// A Move minted before a peer's legal removal must meet the revision gate before any
/// membership lookup: the retained answer is `RevisionMismatch` carrying the CURRENT
/// revision, never `TabUnavailable`/`PanelUnavailable` for the now-absent target. The
/// same-revision degenerate requests below prove the membership predicates still fire, but
/// only behind the gate; a client holding a coherent baseline can never mint them.
#[test]
fn revision_gate_precedes_membership_after_a_legal_peer_close() {
    let (mut authority, grant) = authority();
    let lifecycle =
        WorkbenchGrant::read_only(authority.session_id().clone()).with_terminal_lifecycle();
    let membership = authority.fixture_tree_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let tab_id = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        6000,
        &tab_id,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let membership = authority.fixture_tree_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&tab_id)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();
    let context = initial_workspace_context(&authority);
    let stale_revision = authority.projection.snapshot().revision;
    // Minted while the target leaf is still projected; the unique request id keeps the
    // ledger from replaying an older entry into this control flow.
    let stale_move = move_command(
        &authority,
        7001,
        &tab_id,
        &panel_a,
        &panel_b,
        WorkbenchPanelSplitDirection::Right,
    );
    assert_eq!(stale_move.expected_revision, stale_revision);

    // A peer's real panel.close removes the target leaf and publishes the revision bump.
    publish_panel_close(
        &mut authority,
        &lifecycle,
        8001,
        &context,
        &tab_id,
        &panel_b,
        Instant::now(),
    );
    let current = authority.projection.snapshot().revision;
    assert!(current.value() > stale_revision.value());
    let remaining: Vec<_> =
        leaves(&authority.projection.snapshot().terminal_contexts[0].tabs[0].root)
            .iter()
            .map(leaf_panel_id)
            .collect();
    assert_eq!(remaining, vec![panel_a.clone()]);

    let WorkbenchPanelPlacementBegin::Ready(result) =
        authority.begin_terminal_panel_placement(Some(&grant), stale_move.clone())
    else {
        panic!("a stale move is rejected at the revision gate, never admitted")
    };
    assert_eq!(
        result.outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: current,
            reason: WorkbenchRejectionReason::RevisionMismatch,
        }
    );
    // The rejection is retained: replaying the identical command returns the stored entry
    // rather than re-running the gate.
    let WorkbenchPanelPlacementBegin::Ready(replay) =
        authority.begin_terminal_panel_placement(Some(&grant), stale_move)
    else {
        panic!("a replayed stale move stays a ready result")
    };
    assert_eq!(replay.outcome, result.outcome);

    // Degenerate controls at the current revision: the membership predicates still fire
    // behind the gate. A client holding a coherent baseline can never mint these shapes.
    let missing_panel = WorkbenchPanelId::parse(&identity(999_999)).unwrap();
    let missing_target = move_command(
        &authority,
        9001,
        &tab_id,
        &panel_a,
        &missing_panel,
        WorkbenchPanelSplitDirection::Right,
    );
    assert_eq!(
        submit(&mut authority, &grant, missing_target).outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: current,
            reason: WorkbenchRejectionReason::PanelUnavailable,
        }
    );
    let missing_tab = WorkbenchTabId::parse(&identity(888_888)).unwrap();
    let missing_tab_move = move_command(
        &authority,
        9002,
        &missing_tab,
        &panel_a,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    assert_eq!(
        submit(&mut authority, &grant, missing_tab_move).outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: current,
            reason: WorkbenchRejectionReason::TabUnavailable,
        }
    );
    assert!(!authority.retired);
}
