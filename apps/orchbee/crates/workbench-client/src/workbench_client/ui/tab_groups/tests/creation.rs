use super::*;
use crate::local_client::workbench::WorkbenchTabCreateCommit;
use crate::workbench_client::mailbox::EventDeadline;
use orchbee_workbench_protocol::{
    NewTerminalTab, PanelNode, TabCreationPlacement, WorkbenchSnapshotOutcome,
    WorkbenchSnapshotUnavailableReason, WorkbenchTerminalOutcome, create_terminal_tab,
};

fn created(baseline: &Snapshot, placement: Option<&TabCreationPlacement>) -> (Snapshot, usize) {
    let mut candidate = baseline.clone();
    candidate.revision = WorkbenchRevision::new(2);
    let fresh =
        super::super::super::tests::membership::snapshot(4, 2).terminal_contexts[0].tabs[3].clone();
    let id = fresh.tab_id.clone();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = fresh.root
    else {
        panic!("fixture")
    };
    let group = WorkbenchTabGroupId::parse(&format!("{:032x}", 1901)).unwrap();
    candidate.terminal_contexts[0] = create_terminal_tab(
        &baseline.terminal_contexts[0],
        placement,
        NewTerminalTab {
            tab_id: id.clone(),
            panel_id,
            presentation_id: primary_presentation.presentation_id,
            label: orchbee_workbench_protocol::WorkbenchLabel::parse("4").unwrap(),
            automatic_label: true,
            binding: primary_presentation.binding.unwrap(),
            launch_program: None,
        },
        matches!(placement, Some(TabCreationPlacement::NewGroup {})).then_some(&group),
    )
    .unwrap();
    let index = candidate.terminal_contexts[0]
        .tabs
        .iter()
        .position(|tab| tab.tab_id == id)
        .unwrap();
    (candidate, index)
}

fn commit(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    candidate: &Snapshot,
    index: usize,
) -> (CommandLease, CreateOperationKey) {
    let lease = mailbox.try_recv().unwrap().expect("one Create command");
    let QueuedCommand::Create(marker) = lease.record() else {
        panic!("creation lifecycle owner")
    };
    let key = marker.operation().key().clone();
    let tab = &candidate.terminal_contexts[0].tabs[index];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &tab.root
    else {
        panic!("fresh Panel")
    };
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::CreateCommittedKnown {
                operation: key.clone(),
                commit: WorkbenchTabCreateCommit::Created {
                    revision: candidate.revision,
                    terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
                    tab_id: tab.tab_id.clone(),
                    panel_id: panel_id.clone(),
                    presentation_id: primary_presentation.presentation_id.clone(),
                    terminal_outcome: WorkbenchTerminalOutcome::OutcomeUnknown {},
                },
            },
            EventDeadline::TotalCleanup(marker.operation().deadlines().total()),
            clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::CommitRetained)
    ));
    (lease, key)
}

#[test]
fn creation_routes_install_foreground_marks_and_preserve_duplicate_or_removed_tab_marks() {
    for (kind, removed) in [0, 2, 3, 4]
        .into_iter()
        .flat_map(|kind| [(kind, false), (kind, true)])
    {
        let (mut ui, mailbox, clock, baseline) = ready_snapshot(grouped());
        let source = baseline.terminal_contexts[0].tabs[1].tab_id.clone();
        let group = baseline.terminal_contexts[0].groups[0].group_id.clone();
        let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
        let marked_target = if kind == 2 {
            baseline.terminal_contexts[0].tabs[2].tab_id.clone()
        } else {
            source.clone()
        };
        // Ctrl+click marks a Tab beside the current one.
        ui.apply_client_action(ClientAction::TabMenu {
            action: MenuAction::SelectTab,
            tab_id: Some(marked_target),
        });
        let marked = ui.view().selected_tab_ids();
        assert_eq!(marked.len(), 2);
        ui.toggle_tab_group(&group);
        let (action, placement) = match kind {
            0 => (
                MenuAction::NewTabInGroup,
                Some(TabCreationPlacement::InGroup {
                    group_id: group.clone(),
                }),
            ),
            2 => (
                MenuAction::DuplicateTab,
                Some(TabCreationPlacement::Duplicate {
                    tab_id: source.clone(),
                }),
            ),
            3 => (MenuAction::NewTab, None),
            _ => (
                MenuAction::NewTabToRight,
                Some(TabCreationPlacement::AfterTab {
                    tab_id: source.clone(),
                }),
            ),
        };
        match kind {
            0 => {
                let target = MenuTarget::TabGroups {
                    tab_id: source.clone(),
                    colors: false,
                };
                assert!(ui.open_action_menu(target, None, false));
                choose(&mut ui, action);
            }
            3 => {
                // Ordinary creation uses the painted quick-add route, with no background navigation.
                let target = ui.view().tab_create_target().unwrap();
                let action = ui.tab_create_action(&target).unwrap();
                ui.apply_client_action(action);
            }
            _ => {
                // The menus' shared carrier addresses this background Tab, outside the marks.
                ui.apply_client_action(ClientAction::TabMenu {
                    action,
                    tab_id: Some(source.clone()),
                });
            }
        }
        assert_eq!(
            ui.state.as_ref().unwrap().desired(),
            Some(&old),
            "no optimistic selection"
        );
        assert_eq!(
            ui.view().selected_tab_ids(),
            marked,
            "admission keeps marks"
        );
        let (mut candidate, index) = created(&baseline, placement.as_ref());
        let (lease, key) = commit(&mut ui, &mailbox, &clock, &candidate, index);
        assert_eq!(
            ui.view().selected_tab_ids(),
            marked,
            "receipt alone keeps marks"
        );
        if removed {
            candidate.revision = WorkbenchRevision::new(3);
            candidate.terminal_contexts[0].tabs.remove(index);
        }
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::CreateSnapshotResult {
                    operation: key.clone(),
                    outcome: WorkbenchSnapshotOutcome::Available {
                        snapshot: candidate.clone(),
                    },
                },
                EventDeadline::PlacementOnly(clock.now() + std::time::Duration::from_secs(45)),
                &clock,
            )
            .unwrap();
        ui.tick();
        let Some(UiReply::CreateProjection(CreateProjectionDecision::Cleanup(cleanup))) =
            mailbox.take_deposited_reply(&wait).unwrap_or_else(|error| {
                panic!("create route kind={kind}, removed={removed}: {error:?}")
            })
        else {
            panic!("candidate before cleanup")
        };
        assert_eq!(
            ui.state.as_ref().unwrap().cache().canonical_snapshot(),
            &baseline
        );
        assert_eq!(
            ui.view().selected_tab_ids(),
            marked,
            "cleanup has not installed yet"
        );
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::MembershipDetached {
                    cleanup: cleanup.clone(),
                    detached: old.anchor().clone(),
                },
                EventDeadline::TotalCleanup(cleanup.deadlines().total()),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(matches!(
            mailbox.take_deposited_reply(&wait).unwrap(),
            Some(UiReply::Membership(_))
        ));
        let state = ui.state.as_ref().unwrap();
        assert_eq!(state.cache().canonical_snapshot(), &candidate);
        if removed {
            assert!(
                marked
                    .iter()
                    .all(|id| state.selected_tab_ids().contains(id))
            );
            assert!(ui.view().group_collapsed(&group));
        } else {
            assert_eq!(
                state.provenance(index),
                Some(&CreateTerminalProvenance::OutcomeUnknown)
            );
            let expected = if kind == 2 {
                old.anchor().tab_id()
            } else {
                &candidate.terminal_contexts[0].tabs[index].tab_id
            };
            assert_eq!(state.desired().unwrap().anchor().tab_id(), expected);
            assert_eq!(
                state.selected_tab_ids(),
                if kind == 2 {
                    marked
                } else {
                    vec![expected.clone()]
                }
            );
        }
        if kind == 2 {
            assert_eq!(
                ui.view().notice(),
                Some(WorkbenchClientNotice::DuplicateOutcomeUnknown)
            );
            assert!(ui.view().group_collapsed(&group));
        } else if matches!(kind, 0 | 4) && !removed {
            assert!(!ui.view().group_collapsed(&group));
        }
        assert!(state.invariant_holds());
        mailbox.finish_lease(lease).unwrap();
    }
}

#[test]
fn duplicate_unavailable_snapshot_recovers_by_read_without_replaying_create() {
    let (mut ui, mailbox, clock, baseline) = ready_snapshot(grouped());
    let source = baseline.terminal_contexts[0].tabs[1].tab_id.clone();
    let old = ui.state.as_ref().unwrap().desired().unwrap().clone();
    ui.apply_tab_menu(MenuAction::DuplicateTab, Some(&source));
    let (candidate, index) = created(
        &baseline,
        Some(&TabCreationPlacement::Duplicate { tab_id: source }),
    );
    let (lease, key) = commit(&mut ui, &mailbox, &clock, &candidate, index);
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::CreateSnapshotResult {
                operation: key,
                outcome: WorkbenchSnapshotOutcome::Unavailable {
                    reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
                },
            },
            EventDeadline::PlacementOnly(clock.now() + std::time::Duration::from_secs(45)),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::CreateProjection(
            CreateProjectionDecision::ProjectionUnavailable
        ))
    ));
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(ui.state.as_ref().unwrap().desired(), Some(&old));
    assert!(mailbox.try_recv().unwrap().is_none());
    let (lease, wait) = super::super::super::tests::membership::place_refresh(
        &mut ui,
        &mailbox,
        &clock,
        candidate.clone(),
    );
    ui.tick();
    let Some(UiReply::Refresh(settlement)) = mailbox.take_deposited_reply(&wait).unwrap() else {
        panic!("snapshot-only recovery")
    };
    let RefreshDecision::Cleanup(cleanup) = settlement.decision() else {
        panic!("exact cleanup")
    };
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: old.anchor().clone(),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Membership(_))
    ));
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        &candidate
    );
    assert_eq!(
        ui.state
            .as_ref()
            .unwrap()
            .desired()
            .unwrap()
            .anchor()
            .tab_id(),
        old.anchor().tab_id()
    );
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::DuplicateOutcomeUnknown)
    );
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn relative_creation_keeps_the_current_tab_without_navigation_and_refuses_inside_pinned_prefix() {
    for pinned in [2, 1] {
        let mut snapshot = grouped();
        snapshot.terminal_contexts[0].groups.clear();
        for (index, tab) in snapshot.terminal_contexts[0].tabs.iter_mut().enumerate() {
            tab.group_id = None;
            tab.pinned = index < pinned;
        }
        let (mut ui, mailbox, _, _) = ready_snapshot(snapshot);
        let old = ui.state.as_ref().unwrap().desired().cloned();
        // The global row acts on the current, first Tab.
        assert!(ui.open_action_menu(MenuTarget::Global, None, false));
        choose(&mut ui, MenuAction::NewTabToRight);
        if pinned == 2 {
            assert_eq!(
                ui.view().notice(),
                Some(WorkbenchClientNotice::CreateOrderConflict)
            );
            assert!(mailbox.try_recv().unwrap().is_none());
        } else {
            let lease = mailbox
                .try_recv()
                .unwrap()
                .expect("one creation, no navigation command");
            let QueuedCommand::Create(marker) = lease.record() else {
                panic!("relative creation")
            };
            assert!(!marker.operation().preserves_selection());
        }
        assert_eq!(ui.state.as_ref().unwrap().desired(), old.as_ref());
    }
}
