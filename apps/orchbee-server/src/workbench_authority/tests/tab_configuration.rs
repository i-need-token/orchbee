use super::*;
use orchbee_workbench_protocol::{
    TabConfiguration, TabGroupColor, TabGroupMoveDirection, WorkbenchTabGroupId,
};

fn configure(
    owner: &WorkbenchAuthority,
    id: u128,
    operation: TabConfiguration,
) -> WorkbenchCommand {
    command(
        id,
        owner.snapshot().revision.value(),
        WorkbenchAction::TabsConfigure {
            terminal_context_id: owner
                .snapshot()
                .navigation
                .standalone_terminal_context_id
                .clone(),
            operation,
        },
    )
}

#[test]
fn tab_configuration_atomic_commit_replay_and_all_operations_keep_bound_members() {
    let (mut owner, grant) = authority();
    let (first, second, _pending) = install_two_tabs(&mut owner);
    let baseline = owner.snapshot().clone();
    let cursor = owner.last_identity;
    let create = configure(
        &owner,
        3000,
        TabConfiguration::CreateGroup {
            tab_id: first.tab_id().clone(),
            label: orchbee_workbench_protocol::WorkbenchTabGroupLabel::parse("组 🦀").unwrap(),
            color: TabGroupColor::Purple,
        },
    );
    let plan = owner
        .plan(
            &create,
            PureWorkbenchAction::from_action(&create.action).unwrap(),
        )
        .unwrap_or_else(|_| panic!("plan"));
    let prepared = owner
        .prepare_command_commit(create.clone(), plan)
        .unwrap_or_else(|_| panic!("prepare"));
    drop(prepared);
    assert_eq!(owner.snapshot(), &baseline);
    assert_eq!(owner.last_identity, cursor);
    let accepted = submit(&mut owner, &grant, create.clone());
    let WorkbenchCommandOutcome::TabsConfigured {
        group_id: Some(group),
        revision,
    } = &accepted.outcome
    else {
        panic!("created group receipt")
    };
    let group = group.clone();
    assert_eq!(revision.value(), baseline.revision.value() + 1);
    assert_eq!(owner.last_identity, cursor + 1);
    let after_create = owner.snapshot().clone();
    assert_eq!(submit(&mut owner, &grant, create), accepted);
    assert_eq!(owner.snapshot(), &after_create);
    for (index, operation) in [
        TabConfiguration::SetGroup {
            tab_id: second.tab_id().clone(),
            group_id: Some(group.clone()),
        },
        TabConfiguration::SetPinned {
            tab_id: first.tab_id().clone(),
            pinned: true,
        },
        TabConfiguration::SetPinned {
            tab_id: first.tab_id().clone(),
            pinned: false,
        },
        TabConfiguration::MoveGroup {
            group_id: group.clone(),
            direction: TabGroupMoveDirection::Previous,
        },
        TabConfiguration::MoveGroup {
            group_id: group.clone(),
            direction: TabGroupMoveDirection::Next,
        },
        TabConfiguration::UpdateGroup {
            group_id: group.clone(),
            label: orchbee_workbench_protocol::WorkbenchTabGroupLabel::parse("Renamed").unwrap(),
            color: TabGroupColor::Orange,
        },
        TabConfiguration::SetGroup {
            tab_id: first.tab_id().clone(),
            group_id: Some(group.clone()),
        },
        TabConfiguration::SetGroup {
            tab_id: first.tab_id().clone(),
            group_id: None,
        },
        TabConfiguration::Ungroup {
            group_id: group.clone(),
        },
        TabConfiguration::MoveTabsByDrag {
            tab_ids: vec![first.tab_id().clone()].try_into().unwrap(),
            insert_index: orchbee_workbench_protocol::WorkbenchTabInsertIndex::new(0).unwrap(),
            group_id: None,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let before = owner.snapshot().clone();
        let request = configure(&owner, 3010 + u128::try_from(index).unwrap(), operation);
        let result = submit(&mut owner, &grant, request.clone());
        assert!(matches!(
            result.outcome,
            WorkbenchCommandOutcome::TabsConfigured { group_id: None, .. }
        ));
        let after = owner.snapshot().clone();
        assert_eq!(after.revision.value(), before.revision.value() + 1);
        assert_eq!(after.navigation, baseline.navigation);
        assert_eq!(
            after.terminal_contexts.len(),
            baseline.terminal_contexts.len()
        );
        for tab in &after.terminal_contexts[0].tabs {
            let old = baseline.terminal_contexts[0]
                .tabs
                .iter()
                .find(|old| old.tab_id == tab.tab_id)
                .unwrap();
            assert_eq!(
                tab.root, old.root,
                "layout, Panel, Presentation and binding must survive"
            );
            assert_eq!(tab.label, old.label);
            assert_eq!(tab.automatic_label, old.automatic_label);
        }
        assert_eq!(owner.last_identity, cursor + 1);
        assert_eq!(submit(&mut owner, &grant, request), result);
        assert_eq!(owner.snapshot(), &after);
    }
    assert!(owner.snapshot().terminal_contexts[0].groups.is_empty());
    let before = owner.snapshot().clone();
    let request = configure(
        &owner,
        3050,
        TabConfiguration::SetPinned {
            tab_id: first.tab_id().clone(),
            pinned: false,
        },
    );
    assert_eq!(
        submit(&mut owner, &grant, request).outcome,
        WorkbenchCommandOutcome::Unchanged {
            revision: before.revision
        }
    );
    assert_eq!(owner.snapshot(), &before);
}

#[test]
fn tab_configuration_stale_missing_and_close_fences_have_no_partial_writes() {
    let (mut owner, grant) = authority();
    let (first, second, _pending) = install_two_tabs(&mut owner);
    let operation = TabConfiguration::CreateGroupForTabs {
        tab_ids: vec![first.tab_id().clone(), second.tab_id().clone()]
            .try_into()
            .unwrap(),
        label: orchbee_workbench_protocol::WorkbenchTabGroupLabel::parse("Group").unwrap(),
        color: TabGroupColor::Blue,
    };
    let before = owner.snapshot().clone();
    let cursor = owner.last_identity;
    let mut stale = configure(&owner, 3100, operation.clone());
    stale.expected_revision = WorkbenchRevision::new(0);
    assert!(matches!(
        submit(&mut owner, &grant, stale).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RevisionMismatch,
            ..
        }
    ));
    let missing = configure(
        &owner,
        3101,
        TabConfiguration::SetGroup {
            tab_id: first.tab_id().clone(),
            group_id: Some(WorkbenchTabGroupId::parse(&identity(99999)).unwrap()),
        },
    );
    assert!(matches!(
        submit(&mut owner, &grant, missing).outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::TabGroupUnavailable,
            ..
        }
    ));
    assert_eq!(owner.snapshot(), &before);
    assert_eq!(owner.last_identity, cursor);

    // An unbound layout has no Worker effect but still owns a real concurrent Tab Close fence.
    let create = command(
        3102,
        before.revision.value(),
        super::layout_create::action(&owner),
    );
    let WorkbenchCommandOutcome::TabLayoutCreated {
        tab_id,
        terminal_context_id,
        ..
    } = submit(&mut owner, &grant, create).outcome
    else {
        panic!("layout")
    };
    let close_grant =
        WorkbenchGrant::manage_navigation(owner.site_id().clone()).with_terminal_lifecycle();
    let close = command(
        3103,
        owner.snapshot().revision.value(),
        WorkbenchAction::TabClose {
            terminal_context_id,
            tab_id,
        },
    );
    let WorkbenchTabCloseBegin::Admitted(admission) =
        owner.begin_terminal_tab_close(Some(&close_grant), close)
    else {
        panic!("close")
    };
    let WorkbenchTabCloseResolution::Primary(primary) = admission.resolve_after_composition()
    else {
        panic!("primary")
    };
    let WorkbenchTabClosePreparation::Prepared(prepared) =
        primary.finish_members(vec![], std::time::Instant::now())
    else {
        panic!("prepare close")
    };
    assert!(matches!(
        prepared.commit().outcome,
        WorkbenchCommandOutcome::TabCloseAccepted { .. }
    ));
    let before = owner.snapshot().clone();
    let cursor = owner.last_identity;
    let request = configure(&owner, 3104, operation);
    assert!(matches!(
        submit(&mut owner, &grant, request).outcome,
        WorkbenchCommandOutcome::Unavailable {
            reason: WorkbenchCommandUnavailableReason::TerminalBusy
        }
    ));
    assert_eq!(owner.snapshot(), &before);
    assert_eq!(owner.last_identity, cursor);
}

#[test]
fn tab_configuration_selected_set_is_one_commit_and_missing_member_cannot_partially_apply() {
    let (mut owner, grant) = authority();
    let (first, second, _pending) = install_two_tabs(&mut owner);
    let baseline = owner.snapshot().clone();
    let cursor = owner.last_identity;
    let missing = WorkbenchTabId::parse(&identity(88888)).unwrap();
    let invalid = vec![first.tab_id().clone(), missing].try_into().unwrap();
    for (index, operation) in [
        TabConfiguration::CreateGroupForTabs {
            tab_ids: invalid,
            label: orchbee_workbench_protocol::WorkbenchTabGroupLabel::parse("All").unwrap(),
            color: TabGroupColor::Grey,
        },
        TabConfiguration::SetTabsPinned {
            tab_ids: vec![
                first.tab_id().clone(),
                WorkbenchTabId::parse(&identity(88888)).unwrap(),
            ]
            .try_into()
            .unwrap(),
            pinned: true,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let request = configure(&owner, 3200 + index as u128, operation);
        assert!(matches!(
            submit(&mut owner, &grant, request).outcome,
            WorkbenchCommandOutcome::Rejected {
                reason: WorkbenchRejectionReason::TabUnavailable,
                ..
            }
        ));
        assert_eq!(owner.snapshot(), &baseline);
        assert_eq!(owner.last_identity, cursor);
    }
    let request = configure(
        &owner,
        3202,
        TabConfiguration::CreateGroupForTabs {
            tab_ids: vec![second.tab_id().clone(), first.tab_id().clone()]
                .try_into()
                .unwrap(),
            label: orchbee_workbench_protocol::WorkbenchTabGroupLabel::parse("Both").unwrap(),
            color: TabGroupColor::Cyan,
        },
    );
    let result = submit(&mut owner, &grant, request.clone());
    let WorkbenchCommandOutcome::TabsConfigured {
        revision,
        group_id: Some(group),
    } = &result.outcome
    else {
        panic!("one created group");
    };
    assert_eq!(revision.value(), baseline.revision.value() + 1);
    assert_eq!(owner.last_identity, cursor + 1);
    for (index, tab) in owner.snapshot().terminal_contexts[0]
        .tabs
        .iter()
        .enumerate()
    {
        let mut expected = baseline.terminal_contexts[0].tabs[index].clone();
        expected.group_id = Some(group.clone());
        assert_eq!(tab, &expected);
    }
    assert_eq!(submit(&mut owner, &grant, request), result);
}
