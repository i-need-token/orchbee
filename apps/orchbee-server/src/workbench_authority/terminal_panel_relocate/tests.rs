//! Focused `panel.relocate` authority tests: identity retention, direction
//! order, the rejection matrix, lifecycle fencing, and the limit surface.

use std::num::NonZeroU64;
use std::time::Instant;

use super::*;
use crate::workbench_authority::{
    WorkbenchPanelCreateBegin, WorkbenchPanelCreatePreparation, WorkbenchTabCloseBegin,
    WorkbenchTabClosePreparation, WorkbenchTabCloseResolution, WorkbenchTabCloseRuntimeTarget,
    WorkbenchTabCreateBegin, WorkbenchTabCreatePreparation, WorkbenchTerminalTab,
};
use crate::workbench_projection::WorkbenchProjectionStore;
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, HostTerminalIdentity, LifecycleTerminalPhase,
    TerminalFence, TerminalId as RuntimeTerminalId, WorkerGeneration, WorkerId as RuntimeWorkerId,
};
use orchbee_workbench_protocol::{
    MAXIMUM_PANEL_NODE_DEPTH, Presentation, PresentationBinding, ProtocolVersion, SiteId,
    TerminalId, WorkbenchLabel, WorkbenchProtocolDocument, WorkbenchRequestId,
    WorkbenchTerminalGeometry, WorkbenchTerminalOutcome, WorkerId, serialize_document,
};

fn authority() -> (WorkbenchAuthority, WorkbenchGrant) {
    let mut authority = WorkbenchAuthority::new(SiteId::parse(&identity(100)).unwrap()).unwrap();
    let grant = WorkbenchGrant::manage_navigation_and_terminal_create(authority.site_id().clone())
        .with_terminal_lifecycle();
    authority.commit_standalone_intent(binding(700)).unwrap();
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
        site_id: authority.site_id().clone(),
        expected_revision: authority.projection.snapshot().revision,
        action,
    }
}

fn standalone_context(authority: &WorkbenchAuthority) -> WorkbenchTerminalContextId {
    authority
        .projection
        .snapshot()
        .navigation
        .standalone_terminal_context_id
        .clone()
}

fn submit(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) -> WorkbenchCommandResult {
    match authority.begin_terminal_panel_relocate(Some(grant), command) {
        WorkbenchPanelRelocateBegin::Ready(result) => result,
        WorkbenchPanelRelocateBegin::Admitted(admission) => match admission.finish() {
            WorkbenchPanelRelocatePreparation::Ready(result) => result,
            WorkbenchPanelRelocatePreparation::Prepared(prepared) => prepared.commit(),
        },
    }
}

/// Drives a real `tab.create` through admission and settlement so the context
/// gains one single-leaf Tab; returns its Tab identity.
fn create_tab(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command_id: u128,
    label: &str,
    binding_value: u128,
) -> WorkbenchTabId {
    let WorkbenchTabCreateBegin::Admitted(admission) = authority.begin_terminal_tab_create(
        Some(grant),
        command(
            authority,
            command_id,
            WorkbenchAction::TabCreate {
                automatic_label: false,
                terminal_context_id: standalone_context(authority),
                label: WorkbenchLabel::parse(label).unwrap(),
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
        ),
    ) else {
        panic!("tab create admission")
    };
    let WorkbenchTabCreatePreparation::Prepared(prepared) =
        admission.finish(binding(binding_value))
    else {
        panic!("tab create preparation")
    };
    let pending = prepared.commit().into_pending();
    let settled = authority
        .prepare_terminal_tab_settlement(
            pending,
            WorkbenchTerminalOutcome::Started {
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            },
        )
        .unwrap()
        .commit();
    let WorkbenchCommandOutcome::TabCreated { tab_id, .. } = settled.outcome else {
        panic!("tab create settlement")
    };
    tab_id
}

/// Splits `panel_id` once through the real admission chain and settles the
/// terminal, so the Tab gains a fresh leaf in `direction` order.
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
                right_click: Default::default(),
                terminal_context_id: standalone_context(authority),
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
        admission.finish(binding(9000 + command_id))
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

fn relocate_command(
    authority: &WorkbenchAuthority,
    id: u128,
    tab_id: &WorkbenchTabId,
    panel_id: &WorkbenchPanelId,
    destination: WorkbenchPanelRelocateDestination,
) -> WorkbenchCommand {
    command(
        authority,
        id,
        WorkbenchAction::PanelRelocate {
            terminal_context_id: standalone_context(authority),
            tab_id: tab_id.clone(),
            panel_id: panel_id.clone(),
            destination,
        },
    )
}

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

fn tab_root(authority: &WorkbenchAuthority, tab_id: &WorkbenchTabId) -> PanelNode {
    authority.projection.snapshot().terminal_contexts[0]
        .tabs
        .iter()
        .find(|tab| &tab.tab_id == tab_id)
        .unwrap()
        .root
        .clone()
}

#[test]
fn relocate_leaf_into_existing_tab_preserves_identity_and_mints_only_the_graft() {
    let (mut authority, grant) = authority();
    let membership = authority.tree_standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let source_tab = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &source_tab,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let target_tab = create_tab(&mut authority, &grant, 200, "two", 800);
    let membership = authority.tree_standalone_membership().unwrap();
    let source_members: Vec<_> = membership.members_for_tab(&source_tab).cloned().collect();
    let panel_b = source_members
        .iter()
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();

    let before = authority.projection.snapshot().clone();
    let source_root = tab_root(&authority, &source_tab);
    let destination_root = tab_root(&authority, &target_tab);
    let moved_leaf = leaves(&source_root)
        .into_iter()
        .find(|node| leaf_panel_id(node) == panel_b)
        .unwrap();
    let last_identity = authority.last_identity;

    let request = relocate_command(
        &authority,
        10,
        &source_tab,
        &panel_b,
        WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id: None,
            target_panel_id: None,
            tab_id: target_tab.clone(),
            direction: WorkbenchPanelSplitDirection::Below,
        },
    );
    let result = submit(&mut authority, &grant, request.clone());
    let WorkbenchCommandOutcome::PanelRelocated {
        revision,
        terminal_context_id,
        tab_id: outcome_source,
        panel_id: outcome_panel,
        target_tab_id,
        split_id,
        first_slot_id,
        second_slot_id,
        transfer: None,
    } = &result.outcome
    else {
        panic!("expected panel-relocated: {:?}", result.outcome)
    };
    assert_eq!(revision.value(), before.revision.value() + 1);
    assert_eq!(terminal_context_id, &standalone_context(&authority));
    assert_eq!(outcome_source, &source_tab);
    assert_eq!(outcome_panel, &panel_b);
    assert_eq!(target_tab_id, &target_tab);

    // The source Split collapsed onto its surviving leaf — the leaf keeps its
    // complete node identity and the source Tab stays open.
    let after_source = tab_root(&authority, &source_tab);
    let PanelNode::Panel {
        panel_id: surviving,
        ..
    } = &after_source
    else {
        panic!("source Tab must collapse to its surviving leaf")
    };
    assert_eq!(surviving, &panel_a);

    // `below` puts the destination's previous root first and the moved leaf
    // second under a fresh vertical root Split; the moved node is bitwise the
    // retained leaf.
    let after_destination = tab_root(&authority, &target_tab);
    let PanelNode::Split {
        children,
        orientation: PanelOrientation::Vertical,
        split_id: grafted,
        ..
    } = &after_destination
    else {
        panic!("destination root must be a fresh vertical Split")
    };
    assert_eq!(grafted, split_id);
    assert_eq!(children[0].slot_id, *first_slot_id);
    assert_eq!(children[1].slot_id, *second_slot_id);
    assert_eq!(children[0].node, destination_root);
    assert_eq!(children[1].node, moved_leaf);
    // Exactly three identities were consumed: Split + two Slots.
    assert_eq!(authority.last_identity, last_identity + 3);
    // Idempotent replay returns the retained result without growing state.
    assert_eq!(submit(&mut authority, &grant, request), result);
    assert!(!authority.retired);
}

#[test]
fn relocate_leaf_into_new_tab_appends_a_single_leaf_tab() {
    let (mut authority, grant) = authority();
    let membership = authority.tree_standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let source_tab = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &source_tab,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let membership = authority.tree_standalone_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&source_tab)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();
    let moved_leaf = leaves(&tab_root(&authority, &source_tab))
        .into_iter()
        .find(|node| leaf_panel_id(node) == panel_b)
        .unwrap();
    let before = authority.projection.snapshot().clone();
    let last_identity = authority.last_identity;

    let request = relocate_command(
        &authority,
        10,
        &source_tab,
        &panel_b,
        WorkbenchPanelRelocateDestination::NewTab {
            terminal_context_id: None,
            label: Some(WorkbenchLabel::parse("logs").unwrap()),
        },
    );
    let result = submit(&mut authority, &grant, request.clone());
    let WorkbenchCommandOutcome::PanelRelocatedToNewTab {
        revision,
        tab_id: outcome_source,
        panel_id: outcome_panel,
        new_tab_id,
        ..
    } = &result.outcome
    else {
        panic!("expected panel-relocated-to-new-tab: {:?}", result.outcome)
    };
    assert_eq!(revision.value(), before.revision.value() + 1);
    assert_eq!(outcome_source, &source_tab);
    assert_eq!(outcome_panel, &panel_b);

    let after = authority.projection.snapshot();
    let context = &after.terminal_contexts[0];
    assert_eq!(context.tabs.len(), 2);
    let appended = context
        .tabs
        .iter()
        .find(|tab| &tab.tab_id == new_tab_id)
        .unwrap();
    assert_eq!(appended.label, "logs");
    assert_eq!(appended.root, moved_leaf);
    let PanelNode::Panel {
        panel_id: surviving,
        ..
    } = &tab_root(&authority, &source_tab)
    else {
        panic!("source Tab must collapse to its surviving leaf")
    };
    assert_eq!(surviving, &panel_a);
    // Only the fresh Tab identity was consumed.
    assert_eq!(authority.last_identity, last_identity + 1);
    assert_eq!(submit(&mut authority, &grant, request), result);
    assert!(!authority.retired);
}

#[test]
fn relocate_direction_places_the_moved_leaf_first_for_above_and_left() {
    let (mut authority, grant) = authority();
    let membership = authority.tree_standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let source_tab = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &source_tab,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let target_tab = create_tab(&mut authority, &grant, 200, "two", 800);
    let membership = authority.tree_standalone_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&source_tab)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();
    let destination_root = tab_root(&authority, &target_tab);
    let moved_leaf = leaves(&tab_root(&authority, &source_tab))
        .into_iter()
        .find(|node| leaf_panel_id(node) == panel_b)
        .unwrap();

    let request = relocate_command(
        &authority,
        10,
        &source_tab,
        &panel_b,
        WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id: None,
            target_panel_id: None,
            tab_id: target_tab.clone(),
            direction: WorkbenchPanelSplitDirection::Left,
        },
    );
    let result = submit(&mut authority, &grant, request);
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::PanelRelocated { .. }
    ));
    let PanelNode::Split {
        children,
        orientation: PanelOrientation::Horizontal,
        ..
    } = &tab_root(&authority, &target_tab)
    else {
        panic!("destination root must be a fresh horizontal Split")
    };
    assert_eq!(children[0].node, moved_leaf);
    assert_eq!(children[1].node, destination_root);
}

#[test]
fn relocate_rejections_are_typed_retained_and_leave_the_projection_untouched() {
    let (mut authority, grant) = authority();
    let membership = authority.tree_standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let source_tab = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &source_tab,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let target_tab = create_tab(&mut authority, &grant, 200, "two", 800);
    let membership = authority.tree_standalone_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&source_tab)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();

    let cases: Vec<(u128, WorkbenchAction, WorkbenchRejectionReason)> = vec![
        (
            11,
            WorkbenchAction::PanelRelocate {
                terminal_context_id: standalone_context(&authority),
                tab_id: WorkbenchTabId::parse(&identity(888_888)).unwrap(),
                panel_id: panel_b.clone(),
                destination: WorkbenchPanelRelocateDestination::Tab {
                    terminal_context_id: None,
                    target_panel_id: None,
                    tab_id: target_tab.clone(),
                    direction: WorkbenchPanelSplitDirection::Below,
                },
            },
            WorkbenchRejectionReason::TabUnavailable,
        ),
        (
            12,
            WorkbenchAction::PanelRelocate {
                terminal_context_id: standalone_context(&authority),
                tab_id: source_tab.clone(),
                panel_id: WorkbenchPanelId::parse(&identity(999_999)).unwrap(),
                destination: WorkbenchPanelRelocateDestination::Tab {
                    terminal_context_id: None,
                    target_panel_id: None,
                    tab_id: target_tab.clone(),
                    direction: WorkbenchPanelSplitDirection::Below,
                },
            },
            WorkbenchRejectionReason::PanelUnavailable,
        ),
        (
            13,
            WorkbenchAction::PanelRelocate {
                terminal_context_id: standalone_context(&authority),
                tab_id: source_tab.clone(),
                panel_id: panel_b.clone(),
                destination: WorkbenchPanelRelocateDestination::Tab {
                    terminal_context_id: None,
                    target_panel_id: None,
                    tab_id: source_tab.clone(),
                    direction: WorkbenchPanelSplitDirection::Below,
                },
            },
            WorkbenchRejectionReason::SameTab,
        ),
        (
            14,
            WorkbenchAction::PanelRelocate {
                terminal_context_id: standalone_context(&authority),
                tab_id: source_tab.clone(),
                panel_id: panel_b.clone(),
                destination: WorkbenchPanelRelocateDestination::Tab {
                    terminal_context_id: None,
                    target_panel_id: None,
                    tab_id: WorkbenchTabId::parse(&identity(777_777)).unwrap(),
                    direction: WorkbenchPanelSplitDirection::Below,
                },
            },
            WorkbenchRejectionReason::TabUnavailable,
        ),
    ];
    for (id, action, reason) in cases {
        let snapshot = authority.projection.snapshot().clone();
        let ledger_len = authority.ledger.len();
        let request = command(&authority, id, action);
        let result = submit(&mut authority, &grant, request.clone());
        assert_eq!(
            result.outcome,
            WorkbenchCommandOutcome::Rejected {
                revision: snapshot.revision,
                reason,
            },
            "request {id}"
        );
        assert_eq!(authority.projection.snapshot(), &snapshot);
        assert_eq!(authority.ledger.len(), ledger_len + 1);
        // The retained rejection replays verbatim.
        assert_eq!(submit(&mut authority, &grant, request), result);
    }
    assert!(!authority.retired);
}

#[test]
fn relocate_preserves_the_complete_panel_with_child_presentations() {
    let (mut authority, grant) = authority();
    let membership = authority.tree_standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let source_tab = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &source_tab,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let target_tab = create_tab(&mut authority, &grant, 200, "two", 800);
    let membership = authority.tree_standalone_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&source_tab)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();

    // A restored canonical Panel may own children; relocation preserves the
    // complete payload without allocating a replacement terminal or Presentation.
    let mut snapshot = authority.projection.snapshot().clone();
    let mut marked = false;
    for tab in &mut snapshot.terminal_contexts[0].tabs {
        let mut stack = vec![&mut tab.root];
        while let Some(node) = stack.pop() {
            match node {
                PanelNode::Panel {
                    right_click: _,
                    panel_id,
                    child_presentations,
                    primary_presentation,
                } if *panel_id == panel_b => {
                    child_presentations.push(Presentation {
                        custom_label: None,
                        presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(
                            &identity(424_242),
                        )
                        .unwrap(),
                        label: "stacked".to_owned(),
                        binding: Some(binding(4242)),
                    });
                    let _ = primary_presentation;
                    marked = true;
                }
                PanelNode::Panel { .. } => {}
                PanelNode::Split { children, .. } => {
                    for slot in children.iter_mut() {
                        stack.push(&mut slot.node);
                    }
                }
            }
        }
    }
    assert!(marked);
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    authority.last_identity = 424_242;

    let snapshot = authority.projection.snapshot().clone();
    let moved = leaves(&tab_root(&authority, &source_tab))
        .into_iter()
        .find(|leaf| leaf_panel_id(leaf) == panel_b)
        .unwrap();
    let request = relocate_command(
        &authority,
        10,
        &source_tab,
        &panel_b,
        WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id: None,
            target_panel_id: None,
            tab_id: target_tab.clone(),
            direction: WorkbenchPanelSplitDirection::Below,
        },
    );
    let result = submit(&mut authority, &grant, request.clone());
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::PanelRelocated { .. }
    ));
    assert_eq!(
        authority.snapshot().revision.value(),
        snapshot.revision.value() + 1
    );
    assert!(!leaves(&tab_root(&authority, &source_tab)).contains(&moved));
    assert!(leaves(&tab_root(&authority, &target_tab)).contains(&moved));
    assert!(!authority.retired);
    assert_eq!(submit(&mut authority, &grant, request), result);
}

#[test]
fn relocate_without_navigation_grant_is_denied() {
    let (mut authority, _grant) = authority();
    let membership = authority.tree_standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let source_tab = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    let create_grant =
        WorkbenchGrant::manage_navigation_and_terminal_create(authority.site_id().clone());
    split(
        &mut authority,
        &create_grant,
        100,
        &source_tab,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let target_tab = create_tab(&mut authority, &create_grant, 200, "two", 800);
    let before = authority.projection.snapshot().clone();
    let retained = authority.ledger.len();
    let read_only = WorkbenchGrant::read_only(authority.site_id().clone());
    let request = relocate_command(
        &authority,
        10,
        &source_tab,
        &panel_a,
        WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id: None,
            target_panel_id: None,
            tab_id: target_tab.clone(),
            direction: WorkbenchPanelSplitDirection::Below,
        },
    );
    assert_eq!(
        submit(&mut authority, &read_only, request.clone()).outcome,
        WorkbenchCommandOutcome::Denied {}
    );
    assert!(matches!(
        authority.begin_terminal_panel_relocate(None, request),
        WorkbenchPanelRelocateBegin::Ready(result)
            if result.outcome == WorkbenchCommandOutcome::Denied {}
    ));
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.ledger.len(), retained);
}

/// The exact retained runtime association a Site owner supplies for `member`,
/// matching the construction used by the Close authority tests.
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

/// Opens an unsettled leaf-scoped `panel.close` on `panel_id` and returns its
/// request identity; the Close record stays open inside the ledger.
fn open_leaf_close(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command_id: u128,
    tab_id: &WorkbenchTabId,
    panel_id: &WorkbenchPanelId,
) -> WorkbenchRequestId {
    let WorkbenchTabCloseBegin::Admitted(admission) = authority.begin_terminal_tab_close(
        Some(grant),
        command(
            authority,
            command_id,
            WorkbenchAction::PanelClose {
                terminal_context_id: standalone_context(authority),
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
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish(
        runtime,
        Some(LifecycleTerminalPhase::AwaitingStart),
        Instant::now(),
    ) else {
        panic!("panel close preparation")
    };
    prepared.commit().request_id
}

/// Opens an unsettled aggregate `tab.close` on `tab_id` and returns its request
/// identity.
fn open_aggregate_close(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command_id: u128,
    tab_id: &WorkbenchTabId,
) -> WorkbenchRequestId {
    let WorkbenchTabCloseBegin::Admitted(admission) = authority.begin_terminal_tab_close(
        Some(grant),
        command(
            authority,
            command_id,
            WorkbenchAction::TabClose {
                terminal_context_id: authority
                    .snapshot()
                    .terminal_contexts
                    .iter()
                    .find(|context| context.tabs.iter().any(|tab| &tab.tab_id == tab_id))
                    .unwrap()
                    .terminal_context_id
                    .clone(),
                tab_id: tab_id.clone(),
            },
        ),
    ) else {
        panic!("tab close admission")
    };
    let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
    else {
        panic!("tab close resolution")
    };
    let runtimes = primary
        .members()
        .iter()
        .map(|member| {
            (
                close_runtime(member),
                Some(LifecycleTerminalPhase::AwaitingStart),
            )
        })
        .collect();
    let WorkbenchTabClosePreparation::Prepared(prepared) =
        primary.finish_members(runtimes, Instant::now())
    else {
        panic!("tab close preparation")
    };
    prepared.commit().request_id
}

#[test]
fn an_open_leaf_close_on_the_moved_member_fences_relocate() {
    let (mut authority, grant) = authority();
    let membership = authority.tree_standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let source_tab = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &source_tab,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let target_tab = create_tab(&mut authority, &grant, 200, "two", 800);
    let membership = authority.tree_standalone_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&source_tab)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();

    // The unsettled leaf Close re-proves the captured `(Tab, Panel)` tuple at
    // publication; relocating that member under another Tab would strand it.
    open_leaf_close(&mut authority, &grant, 300, &source_tab, &panel_b);
    let retained = authority.ledger.len();
    let request = relocate_command(
        &authority,
        10,
        &source_tab,
        &panel_b,
        WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id: None,
            target_panel_id: None,
            tab_id: target_tab.clone(),
            direction: WorkbenchPanelSplitDirection::Below,
        },
    );
    let result = submit(&mut authority, &grant, request.clone());
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    // The transient busy retains nothing: replay re-evaluates identically.
    assert_eq!(authority.ledger.len(), retained);
    assert_eq!(submit(&mut authority, &grant, request), result);
    assert_eq!(authority.ledger.len(), retained);
    assert!(!authority.retired);
}

#[test]
fn an_open_aggregate_close_on_either_endpoint_fences_relocate() {
    let (mut authority, grant) = authority();
    let membership = authority.tree_standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let source_tab = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &source_tab,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let target_tab = create_tab(&mut authority, &grant, 200, "two", 800);
    let membership = authority.tree_standalone_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&source_tab)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();

    // An aggregate Close on the DESTINATION Tab fences just like one on the
    // source — its publication re-proves the complete captured leaf membership.
    open_aggregate_close(&mut authority, &grant, 300, &target_tab);
    let retained = authority.ledger.len();
    let request = relocate_command(
        &authority,
        10,
        &source_tab,
        &panel_b,
        WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id: None,
            target_panel_id: None,
            tab_id: target_tab.clone(),
            direction: WorkbenchPanelSplitDirection::Below,
        },
    );
    let result = submit(&mut authority, &grant, request);
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    assert_eq!(authority.ledger.len(), retained);

    // An aggregate Close on the SOURCE Tab fences the same way.
    open_aggregate_close(&mut authority, &grant, 301, &source_tab);
    let retained = authority.ledger.len();
    let request = relocate_command(
        &authority,
        11,
        &source_tab,
        &panel_b,
        WorkbenchPanelRelocateDestination::NewTab {
            terminal_context_id: None,
            label: Some(WorkbenchLabel::parse("logs").unwrap()),
        },
    );
    let result = submit(&mut authority, &grant, request);
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    assert_eq!(authority.ledger.len(), retained);
    assert!(!authority.retired);
}

#[test]
fn a_pending_split_settlement_on_the_moved_leaf_fences_relocate() {
    let (mut authority, grant) = authority();
    let membership = authority.tree_standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let source_tab = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    let target_tab = create_tab(&mut authority, &grant, 200, "two", 800);

    // Commit a split whose terminal settlement is still outstanding: the
    // minted leaf is projected but its pending record still owes a result.
    let WorkbenchPanelCreateBegin::Admitted(admission) = authority.begin_terminal_panel_create(
        Some(&grant),
        command(
            &authority,
            300,
            WorkbenchAction::PanelSplit {
                right_click: Default::default(),
                terminal_context_id: standalone_context(&authority),
                tab_id: source_tab.clone(),
                panel_id: panel_a.clone(),
                direction: WorkbenchPanelSplitDirection::Right,
                label: WorkbenchLabel::parse("pending").unwrap(),
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
                launch: None,
                profile: None,
                options: Default::default(),
            },
        ),
    ) else {
        panic!("panel split admission")
    };
    let WorkbenchPanelCreatePreparation::Prepared(prepared) = admission.finish(binding(9100))
    else {
        panic!("panel split preparation")
    };
    let _pending = prepared.commit_into_pending();
    let membership = authority.tree_standalone_membership().unwrap();
    let pending_leaf = membership
        .members_for_tab(&source_tab)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();

    let retained = authority.ledger.len();
    let request = relocate_command(
        &authority,
        10,
        &source_tab,
        &pending_leaf,
        WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id: None,
            target_panel_id: None,
            tab_id: target_tab.clone(),
            direction: WorkbenchPanelSplitDirection::Below,
        },
    );
    let result = submit(&mut authority, &grant, request);
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    assert_eq!(authority.ledger.len(), retained);
    assert!(!authority.retired);
}

/// A left-leaning Split chain of exactly `depth` nodes on the deepest path,
/// with a leaf Panel on every shallow branch so the shape stays a valid tree.
fn deep_tree(depth: usize, seed: u128) -> PanelNode {
    if depth == 1 {
        return PanelNode::Panel {
            right_click: Default::default(),
            panel_id: WorkbenchPanelId::parse(&identity(seed)).unwrap(),
            primary_presentation: Presentation {
                custom_label: None,
                presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(
                    &identity(seed + 1),
                )
                .unwrap(),
                label: "deep".to_owned(),
                binding: Some(binding(seed)),
            },
            child_presentations: Vec::new(),
        };
    }
    PanelNode::Split {
        ratio: Default::default(),
        split_id: WorkbenchSplitId::parse(&identity(seed)).unwrap(),
        orientation: PanelOrientation::Vertical,
        children: Box::new([
            PanelSlot {
                slot_id: WorkbenchPanelSlotId::parse(&identity(seed + 1)).unwrap(),
                node: PanelNode::Panel {
                    right_click: Default::default(),
                    panel_id: WorkbenchPanelId::parse(&identity(seed + 2)).unwrap(),
                    primary_presentation: Presentation {
                        custom_label: None,
                        presentation_id: orchbee_workbench_protocol::TerminalPresentationId::parse(
                            &identity(seed + 3),
                        )
                        .unwrap(),
                        label: "shallow".to_owned(),
                        binding: Some(binding(seed + 2)),
                    },
                    child_presentations: Vec::new(),
                },
            },
            PanelSlot {
                slot_id: WorkbenchPanelSlotId::parse(&identity(seed + 4)).unwrap(),
                node: deep_tree(depth - 1, seed + 5),
            },
        ]),
    }
}

#[test]
fn relocate_into_a_maximum_depth_destination_is_a_resource_limit() {
    let (mut authority, grant) = authority();
    let membership = authority.tree_standalone_membership().unwrap();
    let member = membership.members().first().unwrap().clone();
    let source_tab = member.tab_id().clone();
    let panel_a = member.panel_id().clone();
    split(
        &mut authority,
        &grant,
        100,
        &source_tab,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
    );
    let target_tab = create_tab(&mut authority, &grant, 200, "two", 800);
    let membership = authority.tree_standalone_membership().unwrap();
    let panel_b = membership
        .members_for_tab(&source_tab)
        .find(|member| member.panel_id() != &panel_a)
        .unwrap()
        .panel_id()
        .clone();

    // Replace the destination root with a tree already at the depth bound;
    // wrapping it in the relocation Split would exceed `MAXIMUM_PANEL_NODE_DEPTH`.
    let mut snapshot = authority.projection.snapshot().clone();
    let destination = snapshot.terminal_contexts[0]
        .tabs
        .iter_mut()
        .find(|tab| tab.tab_id == target_tab)
        .unwrap();
    destination.root = deep_tree(MAXIMUM_PANEL_NODE_DEPTH, 500_000);
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();

    let snapshot = authority.projection.snapshot().clone();
    let request = relocate_command(
        &authority,
        10,
        &source_tab,
        &panel_b,
        WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id: None,
            target_panel_id: None,
            tab_id: target_tab.clone(),
            direction: WorkbenchPanelSplitDirection::Below,
        },
    );
    let result = submit(&mut authority, &grant, request);
    assert_eq!(
        result.outcome,
        WorkbenchCommandOutcome::Rejected {
            revision: snapshot.revision,
            reason: WorkbenchRejectionReason::ResourceLimit,
        }
    );
    assert_eq!(authority.projection.snapshot(), &snapshot);
    assert!(!authority.retired);
}

mod presentations;
mod transfer;
