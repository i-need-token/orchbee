use super::*;
use orchbee_workbench_protocol::{MAXIMUM_TAB_GROUPS_PER_CONTEXT, TabGroup, TabGroupColor};

fn placed(mut command: WorkbenchCommand, value: TabCreationPlacement) -> WorkbenchCommand {
    let WorkbenchAction::TabCreate { placement, .. } = &mut command.action else {
        unreachable!()
    };
    *placement = Some(value);
    command
}

fn grouped() -> (WorkbenchAuthority, WorkbenchGrant, WorkbenchTabGroupId) {
    let (mut authority, grant) = authority();
    for index in 0..3 {
        let request = tab_command(&authority, index + 1, index as u64, "Source");
        let (pending, _) = commit(&mut authority, &grant, request, binding(20 + index));
        let _ = authority
            .prepare_terminal_tab_settlement(
                pending,
                WorkbenchTerminalOutcome::Started {
                    geometry: geometry(80),
                },
            )
            .unwrap()
            .commit();
    }
    let id = WorkbenchTabGroupId::parse(&identity(10000)).unwrap();
    let mut snapshot = authority.projection.snapshot().clone();
    let context = &mut snapshot.terminal_contexts[0];
    context.groups.push(TabGroup {
        group_id: id.clone(),
        label: "".into(),
        color: TabGroupColor::Grey,
    });
    for tab in &mut context.tabs[..2] {
        tab.group_id = Some(id.clone());
    }
    context.tabs[0].automatic_label = true;
    let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap();
    authority.projection = WorkbenchProjectionStore::load(&encoded).unwrap();
    (authority, grant, id)
}

#[test]
fn placement_is_atomic_on_drop_commit_and_exact_replay_with_unknown_start() {
    for kind in 0..4 {
        let (mut authority, grant, group_id) = grouped();
        let baseline = authority.projection.snapshot().clone();
        let source = baseline.terminal_contexts[0].tabs[0].tab_id.clone();
        let placement = match kind {
            0 => TabCreationPlacement::InGroup { group_id },
            1 => TabCreationPlacement::NewGroup {},
            2 => TabCreationPlacement::Duplicate {
                tab_id: source.clone(),
            },
            _ => TabCreationPlacement::AfterTab {
                tab_id: source.clone(),
            },
        };
        let command = placed(tab_command(&authority, 99, 3, "fresh"), placement);
        let cursor = authority.last_identity;
        {
            let WorkbenchTabCreateBegin::Admitted(admitted) =
                authority.begin_terminal_tab_create(Some(&grant), command.clone())
            else {
                panic!("admit")
            };
            let WorkbenchTabCreatePreparation::Prepared(prepared) =
                admitted.finish(binding(99), None)
            else {
                panic!("prepare")
            };
            drop(prepared);
        }
        assert_eq!(authority.projection.snapshot(), &baseline);
        assert_eq!(authority.last_identity, cursor);
        let (pending, metadata) = commit(&mut authority, &grant, command.clone(), binding(99));
        let committed = authority.projection.snapshot().clone();
        let context = &committed.terminal_contexts[0];
        let index = context
            .tabs
            .iter()
            .position(|tab| &tab.tab_id == metadata.tab_id())
            .unwrap();
        assert_eq!(
            index,
            match kind {
                0 => 2,
                1 => 3,
                _ => 1,
            }
        );
        for prior in &baseline.terminal_contexts[0].tabs {
            assert!(context.tabs.contains(prior));
        }
        if kind == 1 {
            let group = context.groups.last().unwrap();
            assert_eq!(group.label, "");
            assert_eq!(group.color, TabGroupColor::Grey);
            assert_eq!(context.tabs[index].group_id.as_ref(), Some(&group.group_id));
        }
        if kind == 2 {
            assert!(context.tabs[index].automatic_label);
            assert_eq!(context.tabs[index].label, "Source");
        }
        let WorkbenchTabCreateBegin::Ready(replay) =
            authority.begin_terminal_tab_create(Some(&grant), command.clone())
        else {
            panic!("pending replay")
        };
        assert!(matches!(
            replay.outcome,
            WorkbenchCommandOutcome::TabCreatePending { .. }
        ));
        assert_eq!(authority.projection.snapshot(), &committed);
        let result = authority
            .prepare_terminal_tab_settlement(pending, WorkbenchTerminalOutcome::OutcomeUnknown {})
            .unwrap()
            .commit();
        let WorkbenchTabCreateBegin::Ready(replay) =
            authority.begin_terminal_tab_create(Some(&grant), command.clone())
        else {
            panic!("unknown replay")
        };
        assert_eq!(result, replay);
        assert!(matches!(
            replay.outcome,
            WorkbenchCommandOutcome::TabCreated {
                terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
                ..
            }
        ));
        assert_eq!(authority.projection.snapshot(), &committed);
        let conflict = placed(command, TabCreationPlacement::Duplicate { tab_id: source });
        if kind != 2 {
            assert_retained_conflict(&mut authority, &grant, conflict);
        }
    }
}

fn assert_retained_conflict(
    authority: &mut WorkbenchAuthority,
    grant: &WorkbenchGrant,
    command: WorkbenchCommand,
) {
    let WorkbenchTabCreateBegin::Ready(result) =
        authority.begin_terminal_tab_create(Some(grant), command)
    else {
        panic!("conflict")
    };
    assert!(matches!(
        result.outcome,
        WorkbenchCommandOutcome::Rejected {
            reason: WorkbenchRejectionReason::RequestIdentityConflict,
            ..
        }
    ));
}

#[test]
fn relative_creation_refuses_inside_pinned_prefix_before_reserving_runtime_and_admits_its_end() {
    let (mut authority, grant, _) = grouped();
    let mut snapshot = authority.projection.snapshot().clone();
    let context = &mut snapshot.terminal_contexts[0];
    context.groups.clear();
    for (index, tab) in context.tabs.iter_mut().enumerate() {
        tab.group_id = None;
        tab.pinned = index < 2;
    }
    let first = context.tabs[0].tab_id.clone();
    let last = context.tabs[1].tab_id.clone();
    authority.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
    )
    .unwrap();
    let cursor = authority.last_identity;
    let invalid = placed(
        tab_command(&authority, 90, 3, "new"),
        TabCreationPlacement::AfterTab { tab_id: first },
    );
    assert_retained_rejection(
        &mut authority,
        &grant,
        invalid,
        WorkbenchRejectionReason::TabMoveOrderConflict,
    );
    assert_eq!(
        authority.last_identity, cursor,
        "no allocation/reservation before refusal"
    );
    assert_eq!(authority.projection.snapshot(), &snapshot);
    let valid = placed(
        tab_command(&authority, 91, 3, "new"),
        TabCreationPlacement::AfterTab { tab_id: last },
    );
    let (_, metadata) = commit(&mut authority, &grant, valid, binding(90));
    let tabs = &authority.projection.snapshot().terminal_contexts[0].tabs;
    assert_eq!(&tabs[2].tab_id, metadata.tab_id());
    assert!(!tabs[2].pinned && tabs[2].group_id.is_none());
    assert_eq!(&tabs[..2], &snapshot.terminal_contexts[0].tabs[..2]);
}

#[test]
fn stale_and_missing_placement_and_group_capacity_never_reserve_creation() {
    let (mut authority, grant, _) = grouped();
    let missing = TabCreationPlacement::InGroup {
        group_id: WorkbenchTabGroupId::parse(&identity(20000)).unwrap(),
    };
    let stale = placed(tab_command(&authority, 80, 2, "new"), missing.clone());
    assert_retained_rejection(
        &mut authority,
        &grant,
        stale,
        WorkbenchRejectionReason::RevisionMismatch,
    );
    let missing_group = placed(tab_command(&authority, 81, 3, "new"), missing);
    assert_retained_rejection(
        &mut authority,
        &grant,
        missing_group,
        WorkbenchRejectionReason::TabGroupUnavailable,
    );
    let absent = WorkbenchTabId::parse(&identity(20000)).unwrap();
    for (request, placement) in [
        (
            82,
            TabCreationPlacement::Duplicate {
                tab_id: absent.clone(),
            },
        ),
        (85, TabCreationPlacement::AfterTab { tab_id: absent }),
    ] {
        let missing_source = placed(tab_command(&authority, request, 3, "new"), placement);
        assert_retained_rejection(
            &mut authority,
            &grant,
            missing_source,
            WorkbenchRejectionReason::TabUnavailable,
        );
    }
    let mut snapshot = authority.projection.snapshot().clone();
    snapshot.terminal_contexts[0].groups = (0..MAXIMUM_TAB_GROUPS_PER_CONTEXT)
        .map(|i| TabGroup {
            group_id: WorkbenchTabGroupId::parse(&identity(10000 + i as u128)).unwrap(),
            label: "".into(),
            color: TabGroupColor::Grey,
        })
        .collect();
    // A full metadata list with empty groups still admits creation: GUI cleanup runs atomically.
    authority.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot.clone())).unwrap(),
    )
    .unwrap();
    let replacement = placed(
        tab_command(&authority, 83, 3, "new"),
        TabCreationPlacement::NewGroup {},
    );
    let (pending, _) = commit(&mut authority, &grant, replacement, binding(101));
    let _ = authority
        .prepare_terminal_tab_settlement(
            pending,
            WorkbenchTerminalOutcome::Started {
                geometry: geometry(80),
            },
        )
        .unwrap()
        .commit();
    assert_eq!(
        authority.projection.snapshot().terminal_contexts[0]
            .groups
            .len(),
        2
    );
    let template = snapshot.terminal_contexts[0].tabs[0].clone();
    snapshot.terminal_contexts[0].tabs = snapshot.terminal_contexts[0]
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let mut tab = template.clone();
            tab.tab_id = WorkbenchTabId::parse(&identity(20000 + index as u128)).unwrap();
            tab.group_id = Some(group.group_id.clone());
            let PanelNode::Panel {
                panel_id,
                primary_presentation,
                ..
            } = &mut tab.root
            else {
                unreachable!()
            };
            *panel_id = WorkbenchPanelId::parse(&identity(30000 + index as u128)).unwrap();
            primary_presentation.presentation_id =
                TerminalPresentationId::parse(&identity(40000 + index as u128)).unwrap();
            primary_presentation.binding = None;
            tab
        })
        .collect();
    authority.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
    )
    .unwrap();
    let full = placed(
        tab_command(&authority, 84, 3, "new"),
        TabCreationPlacement::NewGroup {},
    );
    assert_retained_rejection(
        &mut authority,
        &grant,
        full,
        WorkbenchRejectionReason::ResourceLimit,
    );
}
