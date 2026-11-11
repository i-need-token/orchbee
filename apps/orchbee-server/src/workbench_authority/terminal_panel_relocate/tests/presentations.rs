//! Complete Panel payloads survive structural operations through real admission.
use super::*;
use crate::test_support::WorkbenchFixture;
use crate::workbench_authority::{
    WorkbenchPanelPlacementBegin, WorkbenchPanelPlacementPreparation, WorkbenchTabMoveBegin,
    WorkbenchTabMovePreparation,
};
use orchbee_workbench_protocol::{TerminalPresentationId, WorkbenchTabMoveDirection};

pub(super) fn add_children(node: &mut PanelNode, next: &mut u128) {
    match node {
        PanelNode::Panel {
            child_presentations,
            ..
        } => {
            for label in ["first child", "second child"] {
                *next += 1;
                child_presentations.push(Presentation {
                    presentation_id: TerminalPresentationId::parse(&identity(*next)).unwrap(),
                    label: label.to_owned(),
                    custom_label: Some(
                        orchbee_workbench_protocol::PanelCustomLabel::parse(
                            "retained custom label",
                        )
                        .unwrap(),
                    ),
                    binding: Some(binding(*next + 100_000)),
                    launch_failure: None,
                    agent_not_resumed: None,
                    agent_start_failure: None,
                    launch_program: None,
                });
            }
        }
        PanelNode::Split { children, .. } => {
            for slot in children.iter_mut() {
                add_children(&mut slot.node, next);
            }
        }
    }
}

#[test]
fn children_survive_tab_creation_ordering_split_move_and_swap() {
    let (mut owner, grant) = authority();
    let mut snapshot = owner.snapshot().clone();
    let mut next = 500_000;
    let group_id =
        orchbee_workbench_protocol::WorkbenchTabGroupId::parse(&identity(499_999)).unwrap();
    snapshot.terminal_contexts[0]
        .groups
        .push(orchbee_workbench_protocol::TabGroup {
            group_id: group_id.clone(),
            label: "preserved group".to_owned(),
            color: orchbee_workbench_protocol::TabGroupColor::Blue,
        });
    let tab = &mut snapshot.terminal_contexts[0].tabs[0];
    tab.group_id = Some(group_id.clone());
    add_children(&mut tab.root, &mut next);
    let original = tab.root.clone();
    let source_tab = tab.tab_id.clone();
    let source_panel = leaf_panel_id(&original);
    owner.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
    )
    .unwrap();
    owner.last_identity = next;
    let membership = owner.fixture_tree_membership().unwrap();
    assert_eq!(membership.members().len(), 3);
    assert_eq!(
        membership.member_for_panel(&source_tab, &source_panel),
        membership.members().first()
    );
    assert!(
        owner.fixture_membership().is_err(),
        "legacy contract stays single-Presentation"
    );

    let target_tab = create_tab(&mut owner, &grant, 100, "new tab", 800);
    assert_eq!(tab_root(&owner, &source_tab), original);
    assert_eq!(owner.snapshot().terminal_contexts[0].tabs[1].group_id, None);
    let request = command(
        &owner,
        101,
        WorkbenchAction::TabMove {
            terminal_context_id: initial_workspace_context(&owner),
            tab_id: source_tab.clone(),
            direction: WorkbenchTabMoveDirection::Next,
        },
    );
    let result = match owner.begin_terminal_tab_move(Some(&grant), request) {
        WorkbenchTabMoveBegin::Ready(result) => result,
        WorkbenchTabMoveBegin::Admitted(admission) => match admission.finish() {
            WorkbenchTabMovePreparation::Ready(result) => result,
            WorkbenchTabMovePreparation::Prepared(prepared) => prepared.commit(),
        },
    };
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::TabMoved { .. }
    ));
    assert_eq!(
        owner.snapshot().terminal_contexts[0].tabs[0].tab_id,
        target_tab
    );
    assert_eq!(tab_root(&owner, &source_tab), original);

    split(
        &mut owner,
        &grant,
        102,
        &source_tab,
        &source_panel,
        WorkbenchPanelSplitDirection::Right,
    );
    let split_leaves = leaves(&tab_root(&owner, &source_tab));
    assert_eq!(split_leaves.len(), 2);
    assert_eq!(split_leaves[0], original);
    let sibling = split_leaves[1].clone();
    let target_panel = leaf_panel_id(&sibling);
    assert_eq!(owner.fixture_tree_membership().unwrap().members().len(), 5);

    for swap in [false, true] {
        let action = if swap {
            WorkbenchAction::PanelSwap {
                terminal_context_id: initial_workspace_context(&owner),
                tab_id: source_tab.clone(),
                panel_id: source_panel.clone(),
                target_panel_id: target_panel.clone(),
            }
        } else {
            WorkbenchAction::PanelMove {
                terminal_context_id: initial_workspace_context(&owner),
                tab_id: source_tab.clone(),
                panel_id: source_panel.clone(),
                target_panel_id: target_panel.clone(),
                direction: WorkbenchPanelSplitDirection::Below,
            }
        };
        let request = command(&owner, 103 + u128::from(swap), action);
        let result = match owner.begin_terminal_panel_placement(Some(&grant), request) {
            WorkbenchPanelPlacementBegin::Ready(result) => result,
            WorkbenchPanelPlacementBegin::Admitted(admission) => match admission.finish() {
                WorkbenchPanelPlacementPreparation::Ready(result) => result,
                WorkbenchPanelPlacementPreparation::Prepared(prepared) => prepared.commit(),
            },
        };
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::PanelMoved { .. }
                | WorkbenchCommandOutcome::PanelSwapped { .. }
        ));
        let expected = if swap {
            vec![original.clone(), sibling.clone()]
        } else {
            vec![sibling.clone(), original.clone()]
        };
        assert_eq!(leaves(&tab_root(&owner, &source_tab)), expected);
        assert_eq!(owner.fixture_tree_membership().unwrap().members().len(), 5);
        assert!(!owner.retired);
        assert_eq!(
            owner.snapshot().terminal_contexts[0].tabs[1].group_id,
            Some(group_id.clone())
        );
    }
}
