use super::*;
use crate::workbench_authority::terminal_membership::{append_simple_tab, parse_tab_ids};
use orchbee_workbench_protocol::{
    Directory, PanelNode, Project, Snapshot, WorkbenchDirectoryId, WorkbenchDirectoryPath,
    WorkbenchProjectId, WorkbenchWorkspaceId, Workspace,
};

pub(super) fn workspace_fixture(count: u128) -> (WorkbenchAuthority, WorkbenchGrant) {
    let (mut authority, grant) = fixture(1);
    let mut snapshot = authority.projection.snapshot().clone();
    for index in 0..2_u128 {
        let context_id = WorkbenchTerminalContextId::parse(&identity(100 + index)).unwrap();
        let directory_id = WorkbenchDirectoryId::parse(&identity(200 + index)).unwrap();
        snapshot.navigation.projects.push(Project {
            project_id: WorkbenchProjectId::parse(&identity(300 + index)).unwrap(),
            label: format!("project {index}"),
        });
        snapshot.navigation.directories.extend(vec![Directory {
            directory_id: directory_id.clone(),
            worker_id: binding(700).worker_id,
            path: WorkbenchDirectoryPath::parse("/workspace").unwrap(),
        }]);
        snapshot.navigation.workspaces.extend(vec![Workspace {
            project_id: Some(WorkbenchProjectId::parse(&identity(300 + index)).unwrap()),
            workspace_id: WorkbenchWorkspaceId::parse(&identity(400 + index)).unwrap(),
            terminal_context_id: context_id.clone(),
            label: format!("workspace {index}"),
            primary_directory_id: Some(directory_id),
        }]);
        snapshot.navigation.workspace_order = snapshot
            .navigation
            .workspaces
            .iter()
            .map(|workspace| workspace.workspace_id.clone())
            .collect();
        let mut context = crate::workbench_authority::empty_context(context_id);
        for member in 0..count {
            append_simple_tab(
                &mut context,
                &parse_tab_ids(500 + index * 100 + member * 3).unwrap(),
                &format!("member {member}"),
                false,
                binding(3000 + index * 100 + member),
            );
        }
        snapshot.terminal_contexts.push(context);
    }
    load(&mut authority, snapshot);
    authority.last_identity = 4000;
    (authority, grant)
}

fn load(authority: &mut WorkbenchAuthority, snapshot: Snapshot) {
    authority.projection = WorkbenchProjectionStore::load(
        &orchbee_workbench_protocol::serialize_document(
            &orchbee_workbench_protocol::WorkbenchProtocolDocument::Snapshot(snapshot),
        )
        .unwrap(),
    )
    .unwrap();
}

fn workspace_close(
    authority: &WorkbenchAuthority,
    id: u128,
    context: usize,
    member: usize,
) -> WorkbenchCommand {
    let context = &authority.projection.snapshot().terminal_contexts[context];
    command(
        authority,
        id,
        WorkbenchAction::TabClose {
            terminal_context_id: context.terminal_context_id.clone(),
            tab_id: context.tabs[member].tab_id.clone(),
        },
    )
}

#[test]
fn workspace_preparation_drop_alias_and_closed_replay_preserve_retained_identity() {
    let (mut authority, grant) = workspace_fixture(1);
    let now = Instant::now();
    let close = workspace_close(&authority, 10000, 1, 0);
    let before = authority.projection.snapshot().clone();
    let primary = primary(&mut authority, &grant, close.clone());
    let runtime = runtime(primary.member());
    let WorkbenchTabClosePreparation::Prepared(prepared) =
        primary.finish(runtime, Some(LifecycleTerminalPhase::AwaitingStart), now)
    else {
        panic!("prepared workspace Close")
    };
    drop(prepared);
    assert_eq!(authority.projection.snapshot(), &before);
    assert!(authority.ledger.is_empty());
    let accepted = accept(&mut authority, &grant, close.clone(), now);
    let rename = command(
        &authority,
        10001,
        WorkbenchAction::ProjectRename {
            project_id: before.navigation.projects[1].project_id.clone(),
            label: WorkbenchLabel::parse("other project changed").unwrap(),
        },
    );
    submit(&mut authority, &grant, rename);
    let alias = workspace_close(&authority, 10002, 1, 0);
    let WorkbenchTabCloseBegin::Admitted(admission) =
        authority.begin_terminal_tab_close(Some(&grant), alias.clone())
    else {
        panic!("alias admission")
    };
    assert!(!admission.terminal_membership().is_standalone());
    let WorkbenchTabCloseResolution::Ready(joined) = admission.resolve_after_composition() else {
        panic!("alias does not resolve runtime")
    };
    assert_eq!(joined.outcome, accepted.outcome);
    assert_eq!(joined.request_id, alias.request_id);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
    dispose_and_cleanup(&mut authority, &close.request_id, now);
    let before_publication = authority.projection.snapshot().clone();
    let prepared = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap();
    assert!(!prepared.before_membership().is_standalone());
    assert!(!prepared.after_membership().is_standalone());
    assert!(prepared.after_membership().is_empty());
    drop(prepared);
    assert_eq!(authority.projection.snapshot(), &before_publication);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
    let closed = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap()
        .commit();
    let mut expected = before_publication;
    expected.revision = WorkbenchRevision::new(expected.revision.value() + 1);
    expected.terminal_contexts[1].tabs.clear();
    assert_eq!(authority.projection.snapshot(), &expected);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(0));
    assert!(matches!(status(&authority, &grant, &close),
        WorkbenchTabCloseReadOutcome::Available { status, .. } if status == closed));
    authority.retire_standalone_composition();
    for (request, expected) in [(close, accepted), (alias, joined)] {
        assert!(
            matches!(authority.begin_terminal_tab_close(Some(&grant), request),
            WorkbenchTabCloseBegin::Ready(result) if result == expected)
        );
    }
}

#[test]
fn workspace_publication_rebases_move_rename_and_pending_create_then_settles_original_token() {
    let (mut authority, grant) = workspace_fixture(2);
    let now = Instant::now();
    let close = workspace_close(&authority, 10000, 1, 0);
    accept(&mut authority, &grant, close.clone(), now);
    dispose_and_cleanup(&mut authority, &close.request_id, now);
    let context_id = authority.projection.snapshot().terminal_contexts[1]
        .terminal_context_id
        .clone();
    let create = command(
        &authority,
        10001,
        WorkbenchAction::TabCreate {
            placement: None,
            automatic_label: false,
            terminal_context_id: context_id.clone(),
            label: WorkbenchLabel::parse("pending sibling").unwrap(),
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            launch: None,
            profile: None,
            options: Default::default(),
        },
    );
    let WorkbenchTabCreateBegin::Admitted(admission) =
        authority.begin_terminal_tab_create(Some(&grant), create.clone())
    else {
        panic!("workspace Create")
    };
    let WorkbenchTabCreatePreparation::Prepared(prepared) = admission.finish(binding(3200)) else {
        panic!("prepared workspace Create")
    };
    let pending = prepared.commit().into_pending();
    let create_revision = authority.projection.snapshot().revision;
    let sibling = authority.projection.snapshot().terminal_contexts[1].tabs[1]
        .tab_id
        .clone();
    let move_request = command(
        &authority,
        10002,
        WorkbenchAction::TabMove {
            terminal_context_id: context_id.clone(),
            tab_id: sibling.clone(),
            direction: WorkbenchTabMoveDirection::Last,
        },
    );
    let WorkbenchTabMoveBegin::Admitted(admission) =
        authority.begin_terminal_tab_move(Some(&grant), move_request)
    else {
        panic!("workspace Move")
    };
    let WorkbenchTabMovePreparation::Prepared(prepared) = admission.finish() else {
        panic!("prepared workspace Move")
    };
    let _moved = prepared.commit();
    let rename = command(
        &authority,
        10003,
        WorkbenchAction::TabRename {
            terminal_context_id: context_id,
            tab_id: sibling,
            label: WorkbenchLabel::parse("renamed sibling").unwrap(),
        },
    );
    submit(&mut authority, &grant, rename);
    let mut expected = authority.projection.snapshot().clone();
    expected.terminal_contexts[1].tabs.remove(0);
    expected.revision = WorkbenchRevision::new(expected.revision.value() + 1);
    let _closed = authority
        .prepare_terminal_tab_close_publication(&close.request_id, now)
        .unwrap()
        .commit();
    assert_eq!(authority.projection.snapshot(), &expected);
    let settled = authority
        .prepare_terminal_tab_settlement(
            pending,
            WorkbenchTerminalOutcome::Started {
                geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
            },
        )
        .unwrap()
        .commit();
    assert!(matches!(settled.outcome,
        WorkbenchCommandOutcome::TabCreated { revision, terminal_outcome: WorkbenchTerminalOutcome::Started { .. }, .. }
        if revision == create_revision));
    assert!(
        matches!(authority.begin_terminal_tab_create(Some(&grant), create),
        WorkbenchTabCreateBegin::Ready(result) if result == settled)
    );
    assert_eq!(authority.projection.snapshot(), &expected);
}

#[test]
fn workspace_close_ignores_directory_availability_and_last_removal_keeps_navigation() {
    for configured in [false, true] {
        let (mut authority, grant) = workspace_fixture(1);
        let mut snapshot = authority.projection.snapshot().clone();
        if !configured {
            snapshot.navigation.workspaces[0].primary_directory_id = None;
        }
        load(&mut authority, snapshot);
        let navigation = authority.projection.snapshot().navigation.clone();
        let close = workspace_close(&authority, 10000, 1, 0);
        let now = Instant::now();
        accept(&mut authority, &grant, close.clone(), now);
        dispose_and_cleanup(&mut authority, &close.request_id, now);
        let _closed = authority
            .prepare_terminal_tab_close_publication(&close.request_id, now)
            .unwrap()
            .commit();
        assert_eq!(authority.projection.snapshot().navigation, navigation);
        assert!(
            authority.projection.snapshot().terminal_contexts[1]
                .tabs
                .is_empty()
        );
        assert_eq!(
            authority.projection.snapshot().terminal_contexts[0]
                .tabs
                .len(),
            1
        );
        assert_eq!(
            authority.projection.snapshot().terminal_contexts[2]
                .tabs
                .len(),
            1
        );
    }
}

#[test]
fn standalone_and_workspace_close_share_revision_credits_in_either_publication_order() {
    for reverse in [false, true] {
        let (mut authority, grant) = workspace_fixture(1);
        set_revision(&mut authority, u64::MAX - 2);
        let now = Instant::now();
        let standalone = close_command(&authority, 10000, 0);
        let workspace = workspace_close(&authority, 10001, 1, 0);
        accept(&mut authority, &grant, standalone.clone(), now);
        accept(&mut authority, &grant, workspace.clone(), now);
        assert_eq!(authority.outstanding_close_credits().ok(), Some(2));
        let third = workspace_close(&authority, 10002, 2, 0);
        let primary = primary(&mut authority, &grant, third);
        let runtime = runtime(primary.member());
        assert!(matches!(
            primary.finish(runtime, None, now),
            WorkbenchTabClosePreparation::Ready(WorkbenchCommandResult {
                outcome: WorkbenchCommandOutcome::Rejected {
                    reason: WorkbenchRejectionReason::RevisionExhausted,
                    ..
                },
                ..
            })
        ));
        let rename = command(
            &authority,
            10003,
            WorkbenchAction::TabRename {
                terminal_context_id: authority.projection.snapshot().terminal_contexts[2]
                    .terminal_context_id
                    .clone(),
                tab_id: authority.projection.snapshot().terminal_contexts[2].tabs[0]
                    .tab_id
                    .clone(),
                label: WorkbenchLabel::parse("blocked unrelated write").unwrap(),
            },
        );
        assert!(matches!(
            submit(&mut authority, &grant, rename).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::RevisionExhausted,
                ..
            }
        ));
        let order = if reverse {
            [workspace, standalone]
        } else {
            [standalone, workspace]
        };
        for (index, close) in order.iter().enumerate() {
            dispose_and_cleanup(&mut authority, &close.request_id, now);
            assert_eq!(
                authority
                    .prepare_terminal_tab_close_publication(&close.request_id, now)
                    .unwrap()
                    .commit(),
                WorkbenchTabCloseStatus::Closed {
                    revision: WorkbenchRevision::new(u64::MAX - 1 + index as u64)
                }
            );
            assert_eq!(
                authority.outstanding_close_credits().ok(),
                Some(1 - index as u64)
            );
        }
        assert_eq!(
            authority.projection.snapshot().terminal_contexts[2]
                .tabs
                .len(),
            1
        );
    }
}

#[test]
fn workspace_rejects_foreign_tabs_and_unknown_context_without_removal() {
    let (mut authority, grant) = workspace_fixture(1);
    let before = authority.projection.snapshot().clone();
    let mut wrong_tab = workspace_close(&authority, 10000, 1, 0);
    let WorkbenchAction::TabClose { tab_id, .. } = &mut wrong_tab.action else {
        panic!()
    };
    *tab_id = before.terminal_contexts[2].tabs[0].tab_id.clone();
    let WorkbenchTabCloseBegin::Admitted(admission) =
        authority.begin_terminal_tab_close(Some(&grant), wrong_tab)
    else {
        panic!("validate context first")
    };
    assert!(matches!(
        admission.resolve_after_composition(),
        WorkbenchTabCloseResolution::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::TabUnavailable,
                ..
            },
            ..
        })
    ));
    let mut unknown = workspace_close(&authority, 10001, 1, 0);
    let WorkbenchAction::TabClose {
        terminal_context_id,
        ..
    } = &mut unknown.action
    else {
        panic!()
    };
    *terminal_context_id = WorkbenchTerminalContextId::parse(&identity(99999)).unwrap();
    assert!(matches!(
        authority.begin_terminal_tab_close(Some(&grant), unknown),
        WorkbenchTabCloseBegin::Ready(WorkbenchCommandResult {
            outcome: WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::TerminalContextUnavailable,
                ..
            },
            ..
        })
    ));
    assert_eq!(authority.projection.snapshot(), &before);
    assert_eq!(authority.outstanding_close_credits().ok(), Some(0));
    assert!(!authority.retired);
}

#[test]
fn workspace_publication_rejects_changed_exact_tuple_without_removing_any_tab() {
    let (mut authority, grant) = workspace_fixture(1);
    let now = Instant::now();
    let close = workspace_close(&authority, 10000, 1, 0);
    accept(&mut authority, &grant, close.clone(), now);
    dispose_and_cleanup(&mut authority, &close.request_id, now);
    let mut snapshot = authority.projection.snapshot().clone();
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut snapshot.terminal_contexts[1].tabs[0].root
    else {
        panic!()
    };
    primary_presentation.binding = Some(binding(3900));
    load(&mut authority, snapshot.clone());
    assert!(matches!(
        authority.prepare_terminal_tab_close_publication(&close.request_id, now),
        Err(WorkbenchTabClosePublicationError::CompositionMismatch)
    ));
    assert_eq!(authority.projection.snapshot(), &snapshot);
    assert!(authority.retired);
    assert!(matches!(
        status(&authority, &grant, &close),
        WorkbenchTabCloseReadOutcome::Available {
            status: WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. },
            ..
        }
    ));
}

#[test]
fn workspace_membership_admits_pinned_tabs_and_unbound_presentations() {
    let (mut authority, grant) = workspace_fixture(1);
    let mut snapshot = authority.projection.snapshot().clone();
    snapshot.terminal_contexts[0].tabs[0].pinned = true;
    let context_id = snapshot.terminal_contexts[1].terminal_context_id.clone();
    load(&mut authority, snapshot);
    assert!(authority.standalone_membership().is_err());
    let membership = authority.terminal_membership(&context_id).unwrap();
    assert!(!membership.is_standalone());
    assert_eq!(membership.members().len(), 1);
    let close = workspace_close(&authority, 10000, 1, 0);
    accept(&mut authority, &grant, close, Instant::now());
    assert!(!authority.retired);

    for pinned in [false, true] {
        let (mut authority, grant) = workspace_fixture(1);
        let close = workspace_close(&authority, 10000, 1, 0);
        let mut snapshot = authority.projection.snapshot().clone();
        let tab = &mut snapshot.terminal_contexts[1].tabs[0];
        if pinned {
            tab.pinned = true;
        } else {
            let PanelNode::Panel {
                primary_presentation,
                ..
            } = &mut tab.root
            else {
                panic!()
            };
            primary_presentation.binding = None;
        }
        load(&mut authority, snapshot.clone());
        if pinned {
            let _ = accept(&mut authority, &grant, close, Instant::now());
            assert!(!authority.retired);
            assert_eq!(authority.projection.snapshot(), &snapshot);
            continue;
        }
        let WorkbenchTabCloseBegin::Admitted(admission) =
            authority.begin_terminal_tab_close(Some(&grant), close.clone())
        else {
            panic!()
        };
        let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
        else {
            panic!()
        };
        assert!(primary.members().is_empty());
        let now = Instant::now();
        let WorkbenchTabClosePreparation::Prepared(prepared) =
            primary.finish_resolved_members(vec![], now)
        else {
            panic!()
        };
        let _ = prepared.commit();
        assert!(!authority.retired);
        assert_eq!(authority.projection.snapshot(), &snapshot);
        assert_eq!(authority.outstanding_close_credits().ok(), Some(1));
        let _ = authority
            .prepare_terminal_tab_close_publication(&close.request_id, now)
            .unwrap()
            .commit();
        assert!(authority.snapshot().terminal_contexts[1].tabs.is_empty());
        assert_eq!(
            authority.snapshot().terminal_contexts[0],
            snapshot.terminal_contexts[0]
        );
        assert_eq!(authority.snapshot().navigation, snapshot.navigation);
    }
}
