//! Close must account for every physical Presentation in its canonical scope.
use super::*;
use crate::test_support::WorkbenchFixture;
use orchbee_workbench_protocol::{Snapshot, TabGroup, TabGroupColor, WorkbenchTabGroupId};

fn load(authority: &mut WorkbenchAuthority, snapshot: Snapshot) {
    authority.projection = WorkbenchProjectionStore::load(
        &orchbee_workbench_protocol::serialize_document(
            &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot),
        )
        .unwrap(),
    )
    .unwrap();
}

fn add_children(node: &mut PanelNode, next: &mut u128) {
    match node {
        PanelNode::Panel {
            child_presentations,
            ..
        } => {
            for _ in 0..2 {
                *next += 1;
                child_presentations.push(Presentation {
                    custom_label: None,
                    binding: Some(binding(*next)),
                    label: format!("child {next}"),
                    presentation_id: TerminalPresentationId::parse(&identity(*next + 1000))
                        .unwrap(),
                    launch_failure: None,
                    agent_not_resumed: None,
                    launch_program: None,
                });
            }
        }
        PanelNode::Split { children, .. } => {
            for child in children.iter_mut() {
                add_children(&mut child.node, next);
            }
        }
    }
}

fn decorate(authority: &mut WorkbenchAuthority, context: usize) {
    let mut snapshot = authority.snapshot().clone();
    let context = &mut snapshot.terminal_contexts[context];
    let group_id = WorkbenchTabGroupId::parse(&identity(30000)).unwrap();
    context.groups.push(TabGroup {
        color: TabGroupColor::Cyan,
        group_id: group_id.clone(),
        label: "retained group".into(),
    });
    let mut next = 20000;
    for (index, tab) in context.tabs.iter_mut().enumerate() {
        tab.pinned = index == 0;
        tab.group_id = (index != 0).then(|| group_id.clone());
        add_children(&mut tab.root, &mut next);
    }
    load(authority, snapshot);
}

fn close_primary<'a>(
    authority: &'a mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    close: WorkbenchCommand,
) -> WorkbenchTabClosePrimaryAdmission<'a> {
    if matches!(close.action, WorkbenchAction::WorkspaceClose { .. }) {
        let super::super::super::WorkbenchWorkspaceCloseBegin::Admitted(admission) =
            authority.begin_workspace_close(Some(grant), close)
        else {
            panic!("Workspace admission")
        };
        let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
        else {
            panic!("Workspace primary")
        };
        *primary
    } else {
        primary(authority, grant, close)
    }
}

fn admit(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    close: &WorkbenchCommand,
    now: Instant,
) -> Vec<WorkbenchTerminalTab> {
    let primary = close_primary(authority, grant, close.clone());
    let members = primary.members().to_vec();
    let runtimes = members
        .iter()
        .map(|m| (runtime(m), Some(LifecycleTerminalPhase::AwaitingStart)))
        .collect();
    let WorkbenchTabClosePreparation::Prepared(prepared) = primary.finish_members(runtimes, now)
    else {
        panic!("complete Close preparation")
    };
    let _ = prepared.commit();
    members
}

fn settle_all(
    authority: &mut WorkbenchAuthority,
    close: &WorkbenchCommand,
    now: Instant,
) -> Vec<WorkbenchTerminalTab> {
    let mut disposed = Vec::new();
    for _ in 0..64 {
        let step = authority
            .next_terminal_tab_close_step_target(&close.request_id, now)
            .unwrap();
        match step.next() {
            CloseContinuationNext::Lifecycle { .. } => {
                disposed.push(step.target().unwrap().member().clone());
                settle_disposed(authority, &close.request_id, step, now);
            }
            CloseContinuationNext::TargetCleanup => {
                let runtime = step.target().unwrap().runtime().unwrap();
                authority
                    .complete_terminal_tab_close_target_cleanup(&close.request_id, runtime)
                    .unwrap();
            }
            CloseContinuationNext::Publication => return disposed,
            next => panic!("unexpected step {next:?}"),
        }
    }
    panic!("bounded fixture did not settle")
}

fn first_children(node: &mut PanelNode) -> &mut Vec<Presentation> {
    match node {
        PanelNode::Panel {
            child_presentations,
            ..
        } => child_presentations,
        PanelNode::Split { children, .. } => first_children(&mut children[0].node),
    }
}

fn unbind(node: &mut PanelNode) {
    match node {
        PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } => {
            primary_presentation.binding = None;
            for child in child_presentations {
                child.binding = None;
            }
        }
        PanelNode::Split { children, .. } => {
            for child in children.iter_mut() {
                unbind(&mut child.node);
            }
        }
    }
}

#[test]
fn mixed_panel_close_preserves_unbound_sibling_and_accepts_bound_child_of_unbound_primary() {
    for child_only in [false, true] {
        let (mut authority, grant) = split_fixture(1);
        let mut snapshot = authority.snapshot().clone();
        let context_id = snapshot.terminal_contexts[0].terminal_context_id.clone();
        let tab = &mut snapshot.terminal_contexts[0].tabs[0];
        let tab_id = tab.tab_id.clone();
        let PanelNode::Split { children, .. } = &mut tab.root else {
            panic!()
        };
        unbind(&mut children[1].node);
        let survivor = children[1].node.clone();
        let PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } = &mut children[0].node
        else {
            panic!()
        };
        let panel_id = panel_id.clone();
        if child_only {
            let mut child = primary_presentation.clone();
            child.presentation_id = TerminalPresentationId::parse(&identity(60000)).unwrap();
            primary_presentation.binding = None;
            child_presentations.push(child);
        }
        load(&mut authority, snapshot);
        let close = command(
            &authority,
            60001,
            WorkbenchAction::PanelClose {
                terminal_context_id: context_id,
                tab_id,
                panel_id,
            },
        );
        let now = Instant::now();
        let captured = admit(&mut authority, &grant, &close, now);
        assert_eq!(captured.len(), 1);
        assert_eq!(
            authority.close_record(&close.request_id).unwrap().scope,
            CloseScope::Panel
        );
        assert_eq!(settle_all(&mut authority, &close, now), captured);
        let _ = authority
            .prepare_terminal_tab_close_publication(&close.request_id, now)
            .unwrap()
            .commit();
        assert_eq!(
            authority.snapshot().terminal_contexts[0].tabs[0].root,
            survivor
        );
        assert!(
            authority
                .fixture_presentation_membership()
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn unbound_panel_and_tab_close_use_addressed_publication_with_replay_and_scope_fences() {
    for whole_tab in [false, true] {
        let (mut authority, grant) = split_fixture(2);
        let mut snapshot = authority.snapshot().clone();
        let context_id = snapshot.terminal_contexts[0].terminal_context_id.clone();
        let tab = &mut snapshot.terminal_contexts[0].tabs[0];
        let tab_id = tab.tab_id.clone();
        unbind(&mut tab.root);
        let PanelNode::Split { children, .. } = &tab.root else {
            panic!()
        };
        let PanelNode::Panel { panel_id, .. } = &children[0].node else {
            panic!()
        };
        let panel_id = panel_id.clone();
        let survivor = children[1].node.clone();
        load(&mut authority, snapshot.clone());
        let close = command(
            &authority,
            60002,
            if whole_tab {
                WorkbenchAction::TabClose {
                    terminal_context_id: context_id.clone(),
                    tab_id: tab_id.clone(),
                }
            } else {
                WorkbenchAction::PanelClose {
                    terminal_context_id: context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id: panel_id.clone(),
                }
            },
        );
        let now = Instant::now();
        assert!(admit(&mut authority, &grant, &close, now).is_empty());
        assert_eq!(authority.snapshot(), &snapshot);
        assert!(authority.open_close_covers_panel(&context_id, &tab_id, &panel_id));
        assert_eq!(
            authority.aggregate_close_fences_tab(&context_id, &tab_id),
            whole_tab
        );
        let original = authority
            .close_record(&close.request_id)
            .unwrap()
            .result
            .clone();
        let WorkbenchTabCloseBegin::Ready(replay) =
            authority.begin_terminal_tab_close(Some(&grant), close.clone())
        else {
            panic!()
        };
        assert_eq!(replay, original);
        let mut alias = close.clone();
        alias.request_id = WorkbenchRequestId::parse(&identity(60003)).unwrap();
        let WorkbenchTabCloseBegin::Admitted(admission) =
            authority.begin_terminal_tab_close(Some(&grant), alias)
        else {
            panic!()
        };
        let WorkbenchTabCloseResolution::Ready(joined) = admission.resolve_after_composition()
        else {
            panic!()
        };
        assert_eq!(joined.outcome, original.outcome);
        assert!(matches!(authority.outstanding_close_credits(), Ok(1)));
        let competing = command(
            &authority,
            60004,
            if whole_tab {
                WorkbenchAction::PanelClose {
                    terminal_context_id: context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id: panel_id.clone(),
                }
            } else {
                WorkbenchAction::TabClose {
                    terminal_context_id: context_id.clone(),
                    tab_id: tab_id.clone(),
                }
            },
        );
        let WorkbenchTabCloseBegin::Admitted(admission) =
            authority.begin_terminal_tab_close(Some(&grant), competing)
        else {
            panic!()
        };
        assert!(matches!(
            admission.resolve_after_composition(),
            WorkbenchTabCloseResolution::Ready(WorkbenchCommandResult {
                outcome: WorkbenchCommandOutcome::Unavailable {
                    reason: WorkbenchCommandUnavailableReason::TerminalBusy
                },
                ..
            })
        ));
        authority
            .close_record_mut(&close.request_id)
            .unwrap()
            .publication_failed(WorkbenchTabClosePublicationReason::PreparationFailed);
        assert!(matches!(
            authority.terminal_tab_close_status(&close.request_id),
            Some(WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                resumable: true,
                ..
            })
        ));
        let resume = WorkbenchTabCloseResume {
            protocol_version: ProtocolVersion,
            request_id: request_id(60005),
            session_id: close.session_id.clone(),
            terminal_context_id: context_id.clone(),
            tab_id: tab_id.clone(),
            close_request_id: close.request_id.clone(),
            expected_attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
        };
        let WorkbenchTabCloseResumeBegin::Admitted(admission) =
            authority.begin_terminal_tab_close_resume(Some(&grant), resume)
        else {
            panic!()
        };
        let WorkbenchTabCloseResumePreparation::Prepared(prepared) = admission.finish(now) else {
            panic!()
        };
        assert!(matches!(
            prepared.commit().outcome,
            WorkbenchTabCloseResumeOutcome::Resumed { .. }
        ));
        assert!(settle_all(&mut authority, &close, now).is_empty());
        let _ = authority
            .prepare_terminal_tab_close_publication(&close.request_id, now)
            .unwrap()
            .commit();
        assert!(!authority.open_close_covers_panel(&context_id, &tab_id, &panel_id));
        let after = authority.snapshot();
        assert_eq!(after.revision.value(), snapshot.revision.value() + 1);
        assert_eq!(
            after.terminal_contexts[0].tabs.last(),
            snapshot.terminal_contexts[0].tabs.last()
        );
        assert_eq!(
            after.terminal_contexts[0].tabs.len(),
            if whole_tab { 1 } else { 2 }
        );
        if !whole_tab {
            assert_eq!(after.terminal_contexts[0].tabs[0].root, survivor);
        }
        assert!(matches!(
            authority.terminal_tab_close_status(&close.request_id),
            Some(WorkbenchTabCloseStatus::Closed { .. })
        ));
    }
}

#[test]
fn unbound_sibling_closes_remain_distinct_and_join_after_tree_collapses() {
    let (mut authority, grant) = split_fixture(1);
    let mut snapshot = authority.snapshot().clone();
    let context_id = snapshot.terminal_contexts[0].terminal_context_id.clone();
    let tab = &mut snapshot.terminal_contexts[0].tabs[0];
    let tab_id = tab.tab_id.clone();
    unbind(&mut tab.root);
    let PanelNode::Split { children, .. } = &tab.root else {
        panic!()
    };
    let panels: Vec<_> = children
        .iter()
        .map(|slot| {
            let PanelNode::Panel { panel_id, .. } = &slot.node else {
                panic!()
            };
            panel_id.clone()
        })
        .collect();
    load(&mut authority, snapshot);
    let closes: Vec<_> = panels
        .into_iter()
        .enumerate()
        .map(|(index, panel_id)| {
            command(
                &authority,
                61000 + index as u128,
                WorkbenchAction::PanelClose {
                    terminal_context_id: context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id,
                },
            )
        })
        .collect();
    let now = Instant::now();
    for close in &closes {
        assert!(admit(&mut authority, &grant, close, now).is_empty());
    }
    assert!(matches!(authority.outstanding_close_credits(), Ok(2)));
    let _ = authority
        .prepare_terminal_tab_close_publication(&closes[1].request_id, now)
        .unwrap()
        .commit();
    let alias = command(&authority, 61002, closes[0].action.clone());
    let WorkbenchTabCloseBegin::Admitted(admission) =
        authority.begin_terminal_tab_close(Some(&grant), alias)
    else {
        panic!()
    };
    let WorkbenchTabCloseResolution::Ready(joined) = admission.resolve_after_composition() else {
        panic!()
    };
    assert!(
        matches!(joined.outcome, WorkbenchCommandOutcome::PanelCloseAccepted { close_request_id, .. } if close_request_id == closes[0].request_id)
    );
    let _ = authority
        .prepare_terminal_tab_close_publication(&closes[0].request_id, now)
        .unwrap()
        .commit();
    assert!(authority.snapshot().terminal_contexts[0].tabs.is_empty());
    assert!(!authority.retired);
}

#[test]
fn panel_tab_and_workspace_close_capture_all_children_and_preserve_survivors() {
    for scope in 0..3 {
        let (mut authority, grant) = if scope == 2 {
            super::super::workspace::workspace_fixture(2)
        } else {
            split_fixture(2)
        };
        let context_index = usize::from(scope == 2);
        decorate(&mut authority, context_index);
        let before = authority.snapshot().clone();
        let context_id = before.terminal_contexts[context_index]
            .terminal_context_id
            .clone();
        let membership = authority.presentation_membership(&context_id).unwrap();
        let first = membership.members()[0].clone();
        let action = match scope {
            0 => WorkbenchAction::PanelClose {
                terminal_context_id: context_id.clone(),
                tab_id: first.tab_id().clone(),
                panel_id: first.panel_id().clone(),
            },
            1 => WorkbenchAction::TabClose {
                terminal_context_id: context_id.clone(),
                tab_id: first.tab_id().clone(),
            },
            // The fixture Workspace precedes the scenario's first project Workspace.
            _ => WorkbenchAction::WorkspaceClose {
                workspace_id: before.navigation.workspaces[1].workspace_id.clone(),
            },
        };
        let close = command(&authority, 50000, action);
        let now = Instant::now();
        let captured = admit(&mut authority, &grant, &close, now);
        assert_eq!(captured.len(), if scope == 0 { 3 } else { 6 });
        assert_eq!(authority.snapshot(), &before);
        assert_eq!(settle_all(&mut authority, &close, now), captured);
        assert_eq!(
            authority.snapshot(),
            &before,
            "disposal alone cannot remove structure"
        );
        let prepared = authority
            .prepare_terminal_tab_close_publication(&close.request_id, now)
            .unwrap();
        assert_eq!(prepared.exact_removed_members(), captured);
        let mut expected = before;
        expected.revision = prepared.revision();
        match scope {
            0 => {
                let PanelNode::Split { children, .. } = &expected.terminal_contexts[0].tabs[0].root
                else {
                    panic!()
                };
                expected.terminal_contexts[0].tabs[0].root = children[1].node.clone();
            }
            1 => {
                expected.terminal_contexts[0].tabs.remove(0);
            }
            _ => {
                let removed = expected.navigation.workspaces.remove(1);
                expected
                    .navigation
                    .workspace_order
                    .retain(|id| *id != removed.workspace_id);
                expected.terminal_contexts.remove(context_index);
            }
        }
        let _ = prepared.commit();
        assert_eq!(authority.snapshot(), &expected);
    }
}

#[test]
fn only_panel_with_multiple_presentations_is_a_whole_tab_scope() {
    let (mut authority, grant) = fixture(1);
    decorate(&mut authority, 0);
    let membership = authority.fixture_presentation_membership().unwrap();
    let member = &membership.members()[0];
    let close = command(
        &authority,
        50001,
        WorkbenchAction::PanelClose {
            terminal_context_id: membership.terminal_context_id().clone(),
            tab_id: member.tab_id().clone(),
            panel_id: member.panel_id().clone(),
        },
    );
    let now = Instant::now();
    assert_eq!(admit(&mut authority, &grant, &close, now).len(), 3);
    assert_eq!(
        authority.close_record(&close.request_id).unwrap().scope,
        CloseScope::Tab
    );
    settle_all(&mut authority, &close, now);
    let _ = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap()
        .commit();
    assert!(authority.snapshot().terminal_contexts[0].tabs.is_empty());
    assert_eq!(authority.snapshot().terminal_contexts[0].groups.len(), 1);
}

#[test]
fn panel_publication_rejects_child_membership_changes_after_disposal() {
    for change in 0..4 {
        let (mut authority, grant) = split_fixture(1);
        decorate(&mut authority, 0);
        let membership = authority.fixture_presentation_membership().unwrap();
        let member = &membership.members()[0];
        let close = command(
            &authority,
            50002,
            WorkbenchAction::PanelClose {
                terminal_context_id: membership.terminal_context_id().clone(),
                tab_id: member.tab_id().clone(),
                panel_id: member.panel_id().clone(),
            },
        );
        let now = Instant::now();
        admit(&mut authority, &grant, &close, now);
        settle_all(&mut authority, &close, now);
        let mut snapshot = authority.snapshot().clone();
        let children = first_children(&mut snapshot.terminal_contexts[0].tabs[0].root);
        match change {
            0 => {
                children.swap(0, 1);
            }
            1 => {
                children.pop();
            }
            2 => {
                children[0].binding = Some(binding(25000));
            }
            _ => {
                let mut extra = children[0].clone();
                extra.presentation_id = TerminalPresentationId::parse(&identity(25001)).unwrap();
                extra.binding = Some(binding(25001));
                children.push(extra);
            }
        }
        load(&mut authority, snapshot.clone());
        assert!(matches!(
            authority.prepare_terminal_tab_close_publication(&close.request_id, now),
            Err(WorkbenchTabClosePublicationError::CompositionMismatch)
        ));
        assert_eq!(authority.snapshot(), &snapshot);
    }
}

#[test]
fn unknown_child_prevents_publication_and_keeps_every_captured_target() {
    let (mut authority, grant) = fixture(1);
    decorate(&mut authority, 0);
    let snapshot = authority.snapshot().clone();
    let context = &snapshot.terminal_contexts[0];
    let close = command(
        &authority,
        50003,
        WorkbenchAction::TabClose {
            terminal_context_id: context.terminal_context_id.clone(),
            tab_id: context.tabs[0].tab_id.clone(),
        },
    );
    let now = Instant::now();
    let members = admit(&mut authority, &grant, &close, now);
    let first = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    settle_disposed(&mut authority, &close.request_id, first, now);
    assert!(matches!(
        authority.prepare_terminal_tab_close_publication(&close.request_id, now),
        Err(WorkbenchTabClosePublicationError::NotReady)
    ));
    let child = authority
        .next_terminal_tab_close_step_target(&close.request_id, now)
        .unwrap();
    assert!(members[1..].contains(child.target().unwrap().member()));
    settle_unknown(&mut authority, &close.request_id, child, now);
    assert!(matches!(
        authority.prepare_terminal_tab_close_publication(&close.request_id, now),
        Err(WorkbenchTabClosePublicationError::NotReady)
    ));
    assert_eq!(
        authority
            .close_record(&close.request_id)
            .unwrap()
            .target_count(),
        3
    );
    assert_eq!(authority.snapshot(), &snapshot);
}
