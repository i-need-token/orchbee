//! Exact placements preserve immutable pin/group metadata or refuse atomically.
use super::*;
use orchbee_workbench_protocol::{
    Snapshot, Tab, TabGroup, TabGroupColor, WorkbenchTabGroupId, WorkbenchTabInsertIndex,
};

fn grouped(workspace: bool) -> (WorkbenchAuthority, WorkbenchGrant, usize) {
    let (mut owner, grant) = if workspace {
        super::workspace::workspace_authority(7)
    } else {
        authority(7)
    };
    let index = usize::from(workspace);
    let mut snapshot = owner.snapshot().clone();
    let context = &mut snapshot.terminal_contexts[index];
    for n in 0..3 {
        context.groups.push(TabGroup {
            group_id: WorkbenchTabGroupId::parse(&identity(800 + n)).unwrap(),
            label: format!("group {n}"),
            color: TabGroupColor::Blue,
        });
    }
    for n in 0..7 {
        context.tabs[n].pinned = n < 2;
        context.tabs[n].group_id = match n {
            2 | 3 => Some(context.groups[0].group_id.clone()),
            4 | 5 => Some(context.groups[1].group_id.clone()),
            _ => None,
        };
    }
    install(&mut owner, snapshot);
    (owner, grant, index)
}

fn install(owner: &mut WorkbenchAuthority, snapshot: Snapshot) {
    owner.projection = WorkbenchProjectionStore::load(
        &serialize_document(&WorkbenchProtocolDocument::Snapshot(snapshot)).unwrap(),
    )
    .unwrap();
}

// Independent oracle: groups occupy complete spans and all pinned positions
// equal the prefix of that size. It does not use the production transition scan.
fn legal(tabs: &[Tab]) -> bool {
    let pinned: Vec<_> = tabs
        .iter()
        .enumerate()
        .filter_map(|(n, t)| t.pinned.then_some(n))
        .collect();
    pinned == (0..pinned.len()).collect::<Vec<_>>()
        && tabs.iter().all(|tab| {
            let Some(group) = &tab.group_id else {
                return true;
            };
            let positions: Vec<_> = tabs
                .iter()
                .enumerate()
                .filter_map(|(n, t)| (t.group_id.as_ref() == Some(group)).then_some(n))
                .collect();
            positions.last().unwrap() - positions[0] + 1 == positions.len()
        })
}

#[test]
fn every_grouped_pinned_placement_matches_exact_permutation_or_retained_refusal() {
    for workspace in [false, true] {
        for source in 0..7 {
            for selector in 0..12 {
                let (mut owner, grant, index) = grouped(workspace);
                let before = owner.snapshot().clone();
                let context = &before.terminal_contexts[index];
                let tab_id = context.tabs[source].tab_id.clone();
                let terminal_context_id = context.terminal_context_id.clone();
                let (action, destination) = if selector < 8 {
                    (
                        WorkbenchAction::TabMoveIndex {
                            terminal_context_id,
                            tab_id,
                            insert_index: WorkbenchTabInsertIndex::new(selector).unwrap(),
                        },
                        if source < selector {
                            selector - 1
                        } else {
                            selector
                        },
                    )
                } else {
                    let (direction, destination) = match selector {
                        8 => (WorkbenchTabMoveDirection::First, 0),
                        9 => (WorkbenchTabMoveDirection::Last, 6),
                        10 => (
                            WorkbenchTabMoveDirection::Previous,
                            source.saturating_sub(1),
                        ),
                        _ => (WorkbenchTabMoveDirection::Next, (source + 1).min(6)),
                    };
                    (
                        WorkbenchAction::TabMove {
                            terminal_context_id,
                            tab_id,
                            direction,
                        },
                        destination,
                    )
                };
                let request = WorkbenchCommand {
                    protocol_version: orchbee_workbench_protocol::ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&identity(9000)).unwrap(),
                    session_id: before.session_id.clone(),
                    expected_revision: before.revision,
                    action,
                };
                let mut expected = before.clone();
                let tabs = &mut expected.terminal_contexts[index].tabs;
                let moved = tabs.remove(source);
                tabs.insert(destination, moved);
                let valid = legal(tabs);
                let last_identity = owner.last_identity;
                let result = submit(&mut owner, &grant, request.clone());
                if source == destination {
                    assert!(matches!(
                        result.outcome,
                        WorkbenchCommandOutcome::Unchanged { .. }
                    ));
                } else if valid {
                    expected.revision = WorkbenchRevision::new(before.revision.value() + 1);
                    assert!(matches!(
                        result.outcome,
                        WorkbenchCommandOutcome::TabMoved { .. }
                    ));
                } else {
                    expected = before;
                    assert_eq!(
                        result.outcome,
                        WorkbenchCommandOutcome::Rejected {
                            revision: expected.revision,
                            reason: WorkbenchRejectionReason::TabMoveOrderConflict
                        }
                    );
                }
                assert_eq!(
                    owner.snapshot(),
                    &expected,
                    "workspace={workspace} source={source} selector={selector}"
                );
                assert_eq!(owner.last_identity, last_identity);
                assert!(!owner.retired);
                assert_eq!(submit(&mut owner, &grant, request), result);
                // Even after a refused placement, normal legal movement remains usable.
                let mut followup = command(&owner, 9001, 0, WorkbenchTabMoveDirection::Next);
                followup.action = WorkbenchAction::TabMove {
                    terminal_context_id: expected.terminal_contexts[index]
                        .terminal_context_id
                        .clone(),
                    tab_id: expected.terminal_contexts[index].tabs[0].tab_id.clone(),
                    direction: WorkbenchTabMoveDirection::Next,
                };
                assert!(matches!(
                    submit(&mut owner, &grant, followup).outcome,
                    WorkbenchCommandOutcome::TabMoved { .. }
                ));
            }
        }
    }
}

#[test]
fn order_conflict_and_noop_precede_revision_exhaustion() {
    for (source, direction, reason) in [
        (
            0,
            WorkbenchTabMoveDirection::Last,
            Some(WorkbenchRejectionReason::TabMoveOrderConflict),
        ),
        (0, WorkbenchTabMoveDirection::First, None),
        (
            0,
            WorkbenchTabMoveDirection::Next,
            Some(WorkbenchRejectionReason::RevisionExhausted),
        ),
    ] {
        let (mut owner, grant, _) = grouped(false);
        let mut before = owner.snapshot().clone();
        before.revision = WorkbenchRevision::new(u64::MAX);
        install(&mut owner, before.clone());
        let request = command(&owner, 9000, source, direction);
        let result = submit(&mut owner, &grant, request);
        let expected = reason.map_or(
            WorkbenchCommandOutcome::Unchanged {
                revision: before.revision,
            },
            |reason| WorkbenchCommandOutcome::Rejected {
                revision: before.revision,
                reason,
            },
        );
        assert_eq!(result.outcome, expected);
        assert_eq!(owner.snapshot(), &before);
        assert!(!owner.retired);
    }
}
