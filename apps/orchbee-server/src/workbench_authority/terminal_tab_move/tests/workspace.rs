use super::*;
use orchbee_workbench_protocol::{
    PanelNode, PanelOrientation, PanelSlot, Presentation, Project, Snapshot, TabGroup,
    TabGroupColor, TerminalPresentationId, WorkbenchPanelSlotId, WorkbenchProjectId,
    WorkbenchSplitId, WorkbenchTabGroupId, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
    Workspace,
};

fn workspace_authority(count: usize) -> (WorkbenchAuthority, WorkbenchGrant) {
    let (mut authority, grant) = authority(3);
    let mut snapshot = authority.projection.snapshot().clone();
    for workspace in 0..2_u128 {
        let context_id = WorkbenchTerminalContextId::parse(&identity(100 + workspace)).unwrap();
        snapshot.navigation.projects.push(Project {
            project_id: WorkbenchProjectId::parse(&identity(200 + workspace)).unwrap(),
            label: format!("project {workspace}"),
            directories: Vec::new(),
            workspaces: vec![Workspace {
                workspace_id: WorkbenchWorkspaceId::parse(&identity(300 + workspace)).unwrap(),
                terminal_context_id: context_id.clone(),
                label: format!("workspace {workspace}"),
                primary_directory_id: None,
            }],
        });
        let mut context = crate::workbench_authority::empty_context(context_id);
        for index in 0..count {
            append_simple_tab(
                &mut context,
                &parse_tab_ids(404 + workspace * 100 + index as u128 * 3).unwrap(),
                &format!("workspace {workspace} tab {index}"),
                false,
                PresentationBinding {
                    terminal_id: TerminalId::parse(&identity(
                        3000 + workspace * 100 + index as u128,
                    ))
                    .unwrap(),
                    worker_id: WorkerId::parse(&identity(2000)).unwrap(),
                },
            );
        }
        snapshot.terminal_contexts.push(context);
    }
    load(&mut authority, snapshot);
    authority.last_identity = u128::MAX;
    (authority, grant)
}

fn load(authority: &mut WorkbenchAuthority, snapshot: Snapshot) {
    authority.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
    )
    .unwrap();
}

fn workspace_command(
    authority: &WorkbenchAuthority,
    request: u128,
    index: usize,
    direction: WorkbenchTabMoveDirection,
) -> WorkbenchCommand {
    let mut request = command(authority, request, 0, direction);
    request.action = WorkbenchAction::TabMove {
        terminal_context_id: authority.projection.snapshot().terminal_contexts[1]
            .terminal_context_id
            .clone(),
        tab_id: WorkbenchTabId::parse(&identity(402 + index as u128 * 3)).unwrap(),
        direction,
    };
    request
}

#[test]
fn every_workspace_direction_preserves_complete_tabs_and_the_rest_of_the_snapshot() {
    use WorkbenchTabMoveDirection::{First, Last, Next, Previous};
    for (source, direction, order) in [
        (0, Previous, [0, 1, 2]),
        (0, Next, [1, 0, 2]),
        (0, First, [0, 1, 2]),
        (0, Last, [1, 2, 0]),
        (1, Previous, [1, 0, 2]),
        (1, Next, [0, 2, 1]),
        (1, First, [1, 0, 2]),
        (1, Last, [0, 2, 1]),
        (2, Previous, [0, 2, 1]),
        (2, Next, [0, 1, 2]),
        (2, First, [2, 0, 1]),
        (2, Last, [0, 1, 2]),
    ] {
        let (mut authority, grant) = workspace_authority(3);
        let before = authority.projection.snapshot().clone();
        let request = workspace_command(&authority, 10, source, direction);
        let response = submit(&mut authority, &grant, request.clone());
        let mut expected = before.clone();
        expected.terminal_contexts[1].tabs = order
            .map(|index| before.terminal_contexts[1].tabs[index].clone())
            .to_vec();
        expected.revision = WorkbenchRevision::new(3 + u64::from(order != [0, 1, 2]));
        let expected_outcome = if order == [0, 1, 2] {
            WorkbenchCommandOutcome::Unchanged {
                revision: before.revision,
            }
        } else {
            WorkbenchCommandOutcome::TabMoved {
                revision: expected.revision,
                terminal_context_id: before.terminal_contexts[1].terminal_context_id.clone(),
                tab_id: before.terminal_contexts[1].tabs[source].tab_id.clone(),
            }
        };
        assert_eq!(response.outcome, expected_outcome);
        assert_eq!(authority.projection.snapshot(), &expected);
        assert_eq!(submit(&mut authority, &grant, request), response);
        assert_eq!(authority.last_identity, u128::MAX);
        assert_eq!(authority.ledger.len(), 1);
    }
}

#[test]
fn workspace_singletons_and_empty_contexts_preserve_noop_and_target_precedence() {
    for count in [0, 1] {
        for direction in [
            WorkbenchTabMoveDirection::Previous,
            WorkbenchTabMoveDirection::Next,
            WorkbenchTabMoveDirection::First,
            WorkbenchTabMoveDirection::Last,
        ] {
            let (mut authority, grant) = workspace_authority(count);
            let mut before = authority.projection.snapshot().clone();
            before.revision = WorkbenchRevision::new(u64::MAX);
            load(&mut authority, before.clone());
            let request = workspace_command(&authority, 10, 0, direction);
            let response = submit(&mut authority, &grant, request);
            assert_eq!(
                response.outcome,
                if count == 0 {
                    WorkbenchCommandOutcome::Rejected {
                        revision: before.revision,
                        reason: WorkbenchRejectionReason::TabUnavailable,
                    }
                } else {
                    WorkbenchCommandOutcome::Unchanged {
                        revision: before.revision,
                    }
                }
            );
            assert_eq!(authority.projection.snapshot(), &before);
            assert!(!authority.retired);
        }
    }
}

#[test]
fn target_in_another_context_is_unavailable_and_unknown_context_retains_its_own_rejection() {
    let (mut authority, grant) = workspace_authority(3);
    let before = authority.projection.snapshot().clone();
    for (index, target) in [
        before.terminal_contexts[0].tabs[0].tab_id.clone(),
        before.terminal_contexts[2].tabs[0].tab_id.clone(),
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = workspace_command(
            &authority,
            10 + index as u128,
            0,
            WorkbenchTabMoveDirection::First,
        );
        let WorkbenchAction::TabMove { tab_id, .. } = &mut request.action else {
            unreachable!()
        };
        *tab_id = target;
        assert!(matches!(
            submit(&mut authority, &grant, request).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::TabUnavailable,
                ..
            }
        ));
    }
    let mut request = workspace_command(&authority, 12, 0, WorkbenchTabMoveDirection::First);
    let WorkbenchAction::TabMove {
        terminal_context_id,
        ..
    } = &mut request.action
    else {
        unreachable!()
    };
    *terminal_context_id = WorkbenchTerminalContextId::parse(&identity(999)).unwrap();
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
fn workspace_replay_keeps_its_revision_after_other_contexts_move_and_context_changes_conflict() {
    let (mut authority, grant) = workspace_authority(3);
    let first = workspace_command(&authority, 10, 0, WorkbenchTabMoveDirection::Last);
    let response = submit(&mut authority, &grant, first.clone());
    let standalone = command(&authority, 11, 0, WorkbenchTabMoveDirection::Last);
    submit(&mut authority, &grant, standalone);
    let mut other = workspace_command(&authority, 12, 0, WorkbenchTabMoveDirection::Last);
    let WorkbenchAction::TabMove {
        terminal_context_id,
        tab_id,
        ..
    } = &mut other.action
    else {
        unreachable!()
    };
    *terminal_context_id = authority.projection.snapshot().terminal_contexts[2]
        .terminal_context_id
        .clone();
    *tab_id = authority.projection.snapshot().terminal_contexts[2].tabs[0]
        .tab_id
        .clone();
    submit(&mut authority, &grant, other);
    let before = authority.projection.snapshot().clone();
    assert_eq!(before.revision, WorkbenchRevision::new(6));
    assert_eq!(submit(&mut authority, &grant, first.clone()), response);
    let mut conflict = first;
    let WorkbenchAction::TabMove {
        terminal_context_id,
        ..
    } = &mut conflict.action
    else {
        unreachable!()
    };
    *terminal_context_id = before.navigation.standalone_terminal_context_id.clone();
    assert!(matches!(
        submit(&mut authority, &grant, conflict).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.ledger.len(), 3);
}

#[test]
fn dropping_workspace_preparation_preserves_every_owner_fact() {
    let (mut authority, grant) = workspace_authority(3);
    let before = authority.projection.snapshot().clone();
    let request = workspace_command(&authority, 10, 0, WorkbenchTabMoveDirection::Last);
    let WorkbenchTabMoveBegin::Admitted(admission) =
        authority.begin_terminal_tab_move(Some(&grant), request.clone())
    else {
        panic!("admission expected")
    };
    assert!(admission.standalone_membership().is_none());
    drop(admission);
    let WorkbenchTabMoveBegin::Admitted(admission) =
        authority.begin_terminal_tab_move(Some(&grant), request)
    else {
        panic!("admission expected")
    };
    let WorkbenchTabMovePreparation::Prepared(prepared) = admission.finish() else {
        panic!("preparation expected")
    };
    drop(prepared);
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.last_identity, u128::MAX);
    assert_eq!(authority.ledger_bytes, 0);
    assert!(authority.ledger.is_empty());
    assert!(!authority.retired);
}

#[test]
fn workspace_move_does_not_validate_another_contexts_layout() {
    let (mut authority, grant) = workspace_authority(3);
    let mut before = authority.projection.snapshot().clone();
    before.terminal_contexts[0].tabs[0].pinned = true;
    before.terminal_contexts[2].tabs[0].pinned = true;
    load(&mut authority, before.clone());
    let request = workspace_command(&authority, 10, 0, WorkbenchTabMoveDirection::Next);
    assert!(matches!(
        submit(&mut authority, &grant, request).outcome,
        WorkbenchCommandOutcome::TabMoved { .. }
    ));
    before.terminal_contexts[1].tabs.swap(0, 1);
    before.revision = WorkbenchRevision::new(4);
    assert_eq!(authority.projection.snapshot(), &before);
    assert!(!authority.retired);
}

#[test]
fn workspace_strict_layout_precedes_missing_target_and_boundary_noop() {
    for layout in 0..6 {
        for target in [0, 99] {
            let (mut authority, grant) = workspace_authority(3);
            let mut before = authority.projection.snapshot().clone();
            let context = &mut before.terminal_contexts[1];
            match layout {
                0 => {
                    context.tabs[0].pinned = true;
                    context.tabs[1].pinned = true;
                }
                1 | 2 => {
                    let group_id = WorkbenchTabGroupId::parse(&identity(800)).unwrap();
                    context.groups.push(TabGroup {
                        group_id: group_id.clone(),
                        label: "group".to_owned(),
                        color: TabGroupColor::Blue,
                    });
                    if layout == 2 {
                        context.tabs[1].group_id = Some(group_id);
                    }
                }
                3 | 4 => {
                    let PanelNode::Panel {
                        primary_presentation,
                        child_presentations,
                        ..
                    } = &mut context.tabs[1].root
                    else {
                        unreachable!()
                    };
                    if layout == 3 {
                        primary_presentation.binding = None;
                    } else {
                        child_presentations.push(Presentation {
                            custom_label: None,
                            presentation_id: TerminalPresentationId::parse(&identity(800)).unwrap(),
                            label: "child".to_owned(),
                            binding: None,
                        });
                    }
                }
                _ => {
                    let mut sibling = context.tabs[2].root.clone();
                    let PanelNode::Panel {
                        panel_id,
                        primary_presentation,
                        ..
                    } = &mut sibling
                    else {
                        unreachable!()
                    };
                    *panel_id = orchbee_workbench_protocol::WorkbenchPanelId::parse(&identity(801))
                        .unwrap();
                    primary_presentation.presentation_id =
                        TerminalPresentationId::parse(&identity(802)).unwrap();
                    primary_presentation.binding = None;
                    context.tabs[1].root = PanelNode::Split {
                        split_id: WorkbenchSplitId::parse(&identity(800)).unwrap(),
                        orientation: PanelOrientation::Horizontal,
                        children: Box::new([
                            PanelSlot {
                                slot_id: WorkbenchPanelSlotId::parse(&identity(803)).unwrap(),
                                node: context.tabs[1].root.clone(),
                            },
                            PanelSlot {
                                slot_id: WorkbenchPanelSlotId::parse(&identity(804)).unwrap(),
                                node: sibling,
                            },
                        ]),
                    };
                }
            }
            load(&mut authority, before.clone());
            let request =
                workspace_command(&authority, 10, target, WorkbenchTabMoveDirection::First);
            assert_eq!(
                submit(&mut authority, &grant, request).outcome,
                unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable)
            );
            assert_eq!(authority.projection.snapshot(), &before);
            assert!(authority.retired);
            assert!(authority.ledger.is_empty());
            assert_eq!(authority.last_identity, u128::MAX);
        }
    }
}

#[test]
fn indexed_workspace_move_preserves_complete_split_records_and_other_contexts() {
    use orchbee_workbench_protocol::{
        PanelCustomLabel, PanelRightClickTarget, WorkbenchTabInsertIndex,
    };
    for source in 0..3 {
        for boundary in 0..=3 {
            let (mut authority, grant) = workspace_authority(3);
            let mut before = authority.projection.snapshot().clone();
            let context = &mut before.terminal_contexts[1];
            let mut sibling = context.tabs[2].root.clone();
            let PanelNode::Panel {
                panel_id,
                primary_presentation,
                right_click,
                ..
            } = &mut sibling
            else {
                unreachable!()
            };
            *panel_id =
                orchbee_workbench_protocol::WorkbenchPanelId::parse(&identity(801)).unwrap();
            primary_presentation.presentation_id =
                TerminalPresentationId::parse(&identity(802)).unwrap();
            primary_presentation.binding.as_mut().unwrap().terminal_id =
                TerminalId::parse(&identity(9999)).unwrap();
            primary_presentation.custom_label = Some(PanelCustomLabel::parse("保留 😀").unwrap());
            *right_click = PanelRightClickTarget::Panel;
            context.tabs[1].root = PanelNode::Split {
                split_id: WorkbenchSplitId::parse(&identity(800)).unwrap(),
                orientation: PanelOrientation::Horizontal,
                children: Box::new([
                    PanelSlot {
                        slot_id: WorkbenchPanelSlotId::parse(&identity(803)).unwrap(),
                        node: context.tabs[1].root.clone(),
                    },
                    PanelSlot {
                        slot_id: WorkbenchPanelSlotId::parse(&identity(804)).unwrap(),
                        node: sibling,
                    },
                ]),
            };
            load(&mut authority, before.clone());
            let request = WorkbenchCommand {
                protocol_version: orchbee_workbench_protocol::ProtocolVersion,
                request_id: WorkbenchRequestId::parse(&identity(10)).unwrap(),
                site_id: before.site_id.clone(),
                expected_revision: before.revision,
                action: WorkbenchAction::TabMoveIndex {
                    terminal_context_id: before.terminal_contexts[1].terminal_context_id.clone(),
                    tab_id: before.terminal_contexts[1].tabs[source].tab_id.clone(),
                    insert_index: WorkbenchTabInsertIndex::new(boundary).unwrap(),
                },
            };
            let result = submit(&mut authority, &grant, request.clone());
            let mut expected = before.clone();
            let original = &before.terminal_contexts[1].tabs;
            let mut tabs = Vec::new();
            for gap in 0..=3 {
                if gap == boundary {
                    tabs.push(original[source].clone());
                }
                if gap < 3 && gap != source {
                    tabs.push(original[gap].clone());
                }
            }
            expected.revision =
                WorkbenchRevision::new(before.revision.value() + u64::from(tabs != *original));
            expected.terminal_contexts[1].tabs = tabs;
            assert_eq!(authority.projection.snapshot(), &expected);
            assert_eq!(authority.last_identity, u128::MAX);
            assert_eq!(submit(&mut authority, &grant, request), result);
        }
    }
}
