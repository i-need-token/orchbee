use super::super::*;
use crate::action_menu::MenuAction;
use crate::local_client::{
    tests::server::adjustable_operation_clock, workbench::WorkbenchTabCreateCommit,
};
use crate::terminal_session::{
    mailbox::{CommandLease, EventDeadline},
    state::{DuplicateStop, PaneRef, TabSelectionChange},
};
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use orchbee_workbench_protocol::*;
use std::sync::atomic::{AtomicU64, Ordering};

fn ready() -> (
    NativeUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    let mut snapshot = super::super::tests::membership::snapshot(3, 1);
    let group = TabGroup {
        group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 1900)).unwrap(),
        label: String::new(),
        color: TabGroupColor::Grey,
    };
    snapshot.terminal_contexts[0].tabs[0].pinned = true;
    for tab in &mut snapshot.terminal_contexts[0].tabs[1..] {
        tab.group_id = Some(group.group_id.clone());
    }
    snapshot.terminal_contexts[0].groups.push(group);
    let other_context = WorkbenchTerminalContextId::parse(&format!("{:032x}", 9000)).unwrap();
    let other_group = TabGroup {
        group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 9001)).unwrap(),
        label: "Other group".into(),
        color: TabGroupColor::Green,
    };
    let mut other_tab = super::super::tests::membership::snapshot(40, 1).terminal_contexts[0]
        .tabs
        .pop()
        .unwrap();
    other_tab.group_id = Some(other_group.group_id.clone());
    snapshot.navigation.workspaces.extend(vec![Workspace {
        project_id: Some((WorkbenchProjectId::parse(&format!("{:032x}", 9002)).unwrap()).clone()),
        workspace_id: WorkbenchWorkspaceId::parse(&format!("{:032x}", 9003)).unwrap(),
        label: "Other workspace".into(),
        primary_directory_id: None,
        terminal_context_id: other_context.clone(),
    }]);
    snapshot.navigation.projects.push(Project {
        project_id: WorkbenchProjectId::parse(&format!("{:032x}", 9002)).unwrap(),
        label: "Other project".into(),
    });
    snapshot
        .navigation
        .workspace_order
        .push(snapshot.navigation.workspaces[0].workspace_id.clone());
    snapshot.terminal_contexts.push(TerminalContext {
        terminal_context_id: other_context,
        tabs: vec![other_tab],
        groups: vec![other_group],
    });
    let (mut ui, mailbox, _, _) = super::super::tests::workspace_setup::ready_snapshot(snapshot);
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    ui.clock = clock.clone();
    let state = ui.state.as_mut().unwrap();
    let source = state.cache().tabs()[1].tab_id().clone();
    state
        .mark_tabs(&source, TabSelectionChange::Toggle)
        .unwrap();
    (ui, mailbox, clock, elapsed)
}

#[test]
fn selected_duplicate_later_snapshot_with_unrelated_group_revision_installs_fact_but_never_admits_tail()
 {
    let (mut ui, mailbox, clock, _) = ready();
    let sources = start(&mut ui);
    let (lease, key, mut snapshot, index) = receive(&ui, &mailbox, &sources[0]);
    commit(&mut ui, &mailbox, &clock, &key, &snapshot, index, started());
    snapshot.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    snapshot.terminal_contexts[1].groups[0].label = "Peer change outside selected context".into();
    let cleanup = candidate(&mut ui, &mailbox, &clock, &key, snapshot.clone());
    let pane = install(&mut ui, &mailbox, &clock, cleanup);
    reattach(&mut ui, &mailbox, &clock, &pane);
    mailbox.finish_lease(lease).unwrap();
    let cursors = mailbox.cursors();
    ui.tick();
    assert_eq!(mailbox.cursors().2, cursors.2);
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        &snapshot
    );
    let progress = ui.view().duplicate_progress().unwrap();
    assert_eq!(progress.stop, Some(DuplicateStop::Changed));
    assert_eq!(
        (progress.submitted, progress.committed, progress.started),
        (1, 1, 1)
    );
}

#[test]
fn selected_duplicate_snapshot_unavailable_and_late_commit_never_restart_the_captured_tail() {
    let (mut ui, mailbox, clock, elapsed) = ready();
    let sources = start(&mut ui);
    let (lease, key, snapshot, index) = receive(&ui, &mailbox, &sources[0]);
    // Expire the member while its transport outcome is still unknown; the known late
    // commit is retained through the total-cleanup owner without authorizing another Start.
    elapsed.store(45_000, Ordering::SeqCst);
    ui.tick();
    assert_eq!(
        ui.view().duplicate_progress().unwrap().stop,
        Some(DuplicateStop::Deadline)
    );
    commit(&mut ui, &mailbox, &clock, &key, &snapshot, index, started());
    let progress = ui.view().duplicate_progress().unwrap();
    assert_eq!((progress.committed, progress.started), (1, 1));
    reply(
        &mut ui,
        &mailbox,
        &clock,
        WorkerEvent::CreateSnapshotResult {
            operation: key,
            outcome: WorkbenchSnapshotOutcome::Unavailable {
                reason: WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
        },
    );
    mailbox.finish_lease(lease).unwrap();
    ui.tick();
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(ui.view().phase(), NativeDisplayPhase::ProjectionUnavailable);
    assert_eq!(ui.view().selected_tab_ids(), sources);
}

#[test]
fn selected_duplicate_total_budget_is_fixed_and_each_operation_has_one_cleanup_reserve() {
    let (mut ui, mailbox, clock, elapsed) = ready();
    let first = ui.view().selected_tab_ids()[0].clone();
    ui.state
        .as_mut()
        .unwrap()
        .mark_tabs(&first, TabSelectionChange::All)
        .unwrap();
    let sources = ui.view().selected_tab_ids();
    let beginning = clock.now();
    ui.apply_tab_menu(MenuAction::DuplicateTab, Some(&first));
    for (ordinal, source) in sources.iter().enumerate() {
        let (lease, key, snapshot, index) = receive(&ui, &mailbox, source);
        let QueuedCommand::Create(marker) = lease.record() else {
            panic!("Create")
        };
        let deadlines = marker.operation().deadlines();
        assert_eq!(
            deadlines.total().duration_since(beginning).as_secs(),
            [50, 94, 120][ordinal]
        );
        assert_eq!(
            deadlines
                .total()
                .duration_since(deadlines.forward_cutoff())
                .as_secs(),
            5
        );
        if ordinal == 2 {
            elapsed.store(115_000, Ordering::SeqCst);
            ui.tick();
            assert_eq!(
                ui.view().duplicate_progress().unwrap().stop,
                Some(DuplicateStop::Deadline)
            );
            mailbox.finish_lease(lease).unwrap();
            break;
        }
        elapsed.store((ordinal as u64 + 1) * 44_000, Ordering::SeqCst);
        commit(&mut ui, &mailbox, &clock, &key, &snapshot, index, started());
        let cleanup = candidate(&mut ui, &mailbox, &clock, &key, snapshot);
        let pane = install(&mut ui, &mailbox, &clock, cleanup);
        reattach(&mut ui, &mailbox, &clock, &pane);
        mailbox.finish_lease(lease).unwrap();
        ui.tick();
    }
    let progress = ui.view().duplicate_progress().unwrap();
    assert_eq!(
        (progress.submitted, progress.committed, progress.started),
        (3, 2, 2)
    );
}

fn start(ui: &mut NativeUi) -> Vec<WorkbenchTabId> {
    let sources = ui.view().selected_tab_ids();
    ui.apply_tab_menu(MenuAction::DuplicateTab, Some(&sources[1]));
    assert_eq!(ui.view().duplicate_progress().unwrap().total, 2);
    sources
}

fn reply(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    clock: &LocalOperationClock,
    event: WorkerEvent,
) -> UiReply {
    let wait = mailbox
        .publish_event_before(
            event,
            EventDeadline::Forward(clock.now() + std::time::Duration::from_secs(40)),
            clock,
        )
        .unwrap();
    ui.tick();
    mailbox
        .take_deposited_reply(&wait)
        .unwrap()
        .expect("exact reply")
}

fn receive(
    ui: &NativeUi,
    mailbox: &CommandMailbox,
    source: &WorkbenchTabId,
) -> (CommandLease, CreateOperationKey, Snapshot, usize) {
    let lease = mailbox.try_recv().unwrap().expect("one next member");
    let QueuedCommand::Create(marker) = lease.record() else {
        panic!("existing Create owner")
    };
    assert!(marker.operation().preserves_selection());
    let placement = marker.operation().placement_for_test();
    assert_eq!(
        placement,
        Some(&TabCreationPlacement::Duplicate {
            tab_id: source.clone()
        })
    );
    let key = marker.operation().key().clone();
    let baseline = ui.state.as_ref().unwrap().cache().canonical_snapshot();
    let count = baseline.terminal_contexts[0].tabs.len();
    let mut fresh_snapshot =
        super::super::tests::membership::snapshot(count + 1, baseline.revision.value() + 1);
    let fresh = fresh_snapshot.terminal_contexts[0].tabs.pop().unwrap();
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = fresh.root
    else {
        panic!("fixture")
    };
    let id = fresh.tab_id;
    let mut candidate = baseline.clone();
    candidate.revision = fresh_snapshot.revision;
    candidate.terminal_contexts[0] = create_terminal_tab(
        &baseline.terminal_contexts[0],
        placement,
        NewTerminalTab {
            tab_id: id.clone(),
            panel_id,
            presentation_id: primary_presentation.presentation_id,
            label: WorkbenchLabel::parse("unused duplicate fallback").unwrap(),
            automatic_label: true,
            binding: primary_presentation.binding.unwrap(),
        },
        None,
    )
    .unwrap();
    let index = candidate.terminal_contexts[0]
        .tabs
        .iter()
        .position(|tab| tab.tab_id == id)
        .unwrap();
    (lease, key, candidate, index)
}

fn commit(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    clock: &LocalOperationClock,
    key: &CreateOperationKey,
    candidate: &Snapshot,
    index: usize,
    outcome: Option<WorkbenchTerminalOutcome>,
) {
    assert!(matches!(
        reply(
            ui,
            mailbox,
            clock,
            WorkerEvent::CreateCommittedKnown {
                operation: key.clone(),
                commit: receipt(candidate, index, outcome),
            }
        ),
        UiReply::CommitRetained
    ));
}

fn receipt(
    candidate: &Snapshot,
    index: usize,
    outcome: Option<WorkbenchTerminalOutcome>,
) -> WorkbenchTabCreateCommit {
    let tab = &candidate.terminal_contexts[0].tabs[index];
    let PanelNode::Panel {
        panel_id,
        primary_presentation,
        ..
    } = &tab.root
    else {
        panic!("fresh terminal")
    };
    if let Some(terminal_outcome) = outcome {
        WorkbenchTabCreateCommit::Created {
            revision: candidate.revision,
            terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
            tab_id: tab.tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: primary_presentation.presentation_id.clone(),
            terminal_outcome,
        }
    } else {
        WorkbenchTabCreateCommit::Pending {
            revision: candidate.revision,
            terminal_context_id: candidate.terminal_contexts[0].terminal_context_id.clone(),
            tab_id: tab.tab_id.clone(),
            panel_id: panel_id.clone(),
            presentation_id: primary_presentation.presentation_id.clone(),
        }
    }
}

fn started() -> Option<WorkbenchTerminalOutcome> {
    Some(WorkbenchTerminalOutcome::Started {
        geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap(),
    })
}

fn candidate(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    clock: &LocalOperationClock,
    key: &CreateOperationKey,
    snapshot: Snapshot,
) -> crate::terminal_session::state::MembershipCleanup {
    let UiReply::CreateProjection(CreateProjectionDecision::Cleanup(cleanup)) = reply(
        ui,
        mailbox,
        clock,
        WorkerEvent::CreateSnapshotResult {
            operation: key.clone(),
            outcome: WorkbenchSnapshotOutcome::Available { snapshot },
        },
    ) else {
        panic!("prepare before detach")
    };
    cleanup
}

fn install(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    clock: &LocalOperationClock,
    cleanup: crate::terminal_session::state::MembershipCleanup,
) -> PaneRef {
    let detached = cleanup.old().clone();
    assert!(matches!(
        reply(
            ui,
            mailbox,
            clock,
            WorkerEvent::MembershipDetached { cleanup, detached }
        ),
        UiReply::Membership(_)
    ));
    ui.state.as_ref().unwrap().focus_pane_ref().unwrap()
}

fn reattach(
    ui: &mut NativeUi,
    mailbox: &Arc<CommandMailbox>,
    clock: &LocalOperationClock,
    pane: &PaneRef,
) {
    let projection = TerminalProjection::new(
        AttachmentId::parse(&format!("{:032x}", 7000 + pane.incarnation())).unwrap(),
        HostEpoch::parse(&format!("{:032x}", 7001)).unwrap(),
    );
    assert!(matches!(
        reply(
            ui,
            mailbox,
            clock,
            WorkerEvent::Attached {
                key: pane.clone(),
                projection
            }
        ),
        UiReply::AttachedSettled(_)
    ));
    reply(
        ui,
        mailbox,
        clock,
        WorkerEvent::SetupConfigured {
            key: pane.clone(),
            mode: ControllerMode::ReadOnly,
            applied_geometry: None,
        },
    );
    assert!(
        ui.state
            .as_mut()
            .unwrap()
            .stage_drawn_frame(pane, FrameIdentity::new(1))
    );
    reply(
        ui,
        mailbox,
        clock,
        WorkerEvent::FeedbackAccepted {
            key: pane.clone(),
            frame: FrameIdentity::new(1),
        },
    );
    reply(
        ui,
        mailbox,
        clock,
        WorkerEvent::ReadyCandidate {
            key: pane.clone(),
            frame: FrameIdentity::new(1),
            mode: ControllerMode::ReadOnly,
            last_accepted_geometry: None,
        },
    );
    assert_eq!(ui.view().phase(), NativeDisplayPhase::Ready);
}

#[test]
fn selected_duplicate_real_membership_and_reattach_keep_original_sources_marks_and_selection() {
    let (mut ui, mailbox, clock, _) = ready();
    let original = ui
        .state
        .as_ref()
        .unwrap()
        .cache()
        .canonical_snapshot()
        .clone();
    let original_pane = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    let sources = start(&mut ui);
    let mut last_pane = original_pane.clone();
    let mut keys = Vec::new();
    for source in &sources {
        let (lease, key, snapshot, index) = receive(&ui, &mailbox, source);
        assert!(!keys.contains(&key));
        keys.push(key.clone());
        commit(&mut ui, &mailbox, &clock, &key, &snapshot, index, started());
        let cleanup = candidate(&mut ui, &mailbox, &clock, &key, snapshot.clone());
        assert_eq!(
            ui.state.as_ref().unwrap().focus_pane_ref(),
            Some(last_pane.clone())
        );
        let pane = install(&mut ui, &mailbox, &clock, cleanup);
        assert_ne!(pane, last_pane);
        assert_eq!(pane.pane().panel(), original_pane.pane().panel());
        assert_eq!(pane.anchor().tab_id(), original_pane.anchor().tab_id());
        assert_eq!(ui.view().selected_tab_ids(), sources);
        reattach(&mut ui, &mailbox, &clock, &pane);
        let cursors = mailbox.cursors();
        for _ in 0..3 {
            ui.tick();
        }
        assert_eq!(
            mailbox.cursors(),
            cursors,
            "Ready does not bypass still-active lease"
        );
        assert!(mailbox.try_recv().unwrap().is_none());
        mailbox.finish_lease(lease).unwrap();
        ui.tick();
        assert_eq!(
            ui.state.as_ref().unwrap().cache().canonical_snapshot(),
            &snapshot
        );
        for source in &original.terminal_contexts[0].tabs {
            assert_eq!(
                snapshot.terminal_contexts[0]
                    .tabs
                    .iter()
                    .find(|tab| tab.tab_id == source.tab_id),
                Some(source)
            );
        }
        last_pane = pane;
    }
    let progress = ui.view().duplicate_progress().unwrap();
    assert!(progress.complete);
    assert_eq!(
        (progress.submitted, progress.committed, progress.started),
        (2, 2, 2)
    );
    assert_eq!(progress.stop, None);
}

#[test]
fn selected_duplicate_preinstall_rejects_cleared_marks_and_concrete_same_key_reappearance() {
    for detach in [false, true] {
        let (mut ui, mailbox, clock, _) = ready();
        let sources = start(&mut ui);
        let (lease, key, snapshot, index) = receive(&ui, &mailbox, &sources[0]);
        commit(&mut ui, &mailbox, &clock, &key, &snapshot, index, started());
        let cleanup = candidate(&mut ui, &mailbox, &clock, &key, snapshot.clone());
        let baseline = ui.state.as_ref().unwrap().cache().clone();
        let state = ui.state.as_mut().unwrap();
        let old = state.focus_pane_ref().unwrap();
        if detach {
            state.sync_pane_set(&[], clock.now());
            assert!(state.settle_pane_detached(&old));
            state.sync_pane_set(
                &[(old.pane().panel().clone(), 0, state.latest_geometry())],
                clock.now(),
            );
            let replacement = state.focus_pane_ref().unwrap();
            assert_eq!(replacement.pane(), old.pane());
            assert_ne!(replacement, old, "actual record disposal and reincarnation");
        } else {
            state.clear_tab_selection();
            assert_eq!(state.selected_tab_ids(), vec![sources[0].clone()]);
        }
        let detached = cleanup.old().clone();
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::MembershipDetached { cleanup, detached },
                EventDeadline::Forward(clock.now() + std::time::Duration::from_secs(40)),
                &clock,
            )
            .unwrap();
        let event = mailbox.take_event_before(&clock).unwrap().unwrap();
        ui.apply_event(&mailbox, event); // Deliberately bypass the tick observer before install.
        if detach {
            // A record with the same logical key is not the old live viewer. The existing
            // Create invariant refuses this unowned reappearance before even minting an install.
            assert!(ui.retirement_requested());
            assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
            let facts = ui
                .state
                .as_ref()
                .unwrap()
                .duplicate_facts_for_test()
                .unwrap();
            assert_eq!(facts.stop, Some(DuplicateStop::Changed));
            assert_eq!((facts.submitted, facts.committed, facts.started), (1, 1, 1));
            assert!(ui.state.as_ref().unwrap().committed_target().is_some());
            let _ = mailbox.finish_lease(lease);
            continue;
        }
        assert!(matches!(
            mailbox.take_deposited_reply(&wait).unwrap(),
            Some(UiReply::Membership(_))
        ));
        assert_eq!(
            ui.state.as_ref().unwrap().cache().canonical_snapshot(),
            &snapshot,
            "known member still installs"
        );
        let progress = ui.view().duplicate_progress().unwrap();
        assert_eq!(progress.stop, Some(DuplicateStop::Changed));
        assert_eq!(
            (progress.submitted, progress.committed, progress.started),
            (1, 1, 1)
        );
        mailbox.finish_lease(lease).unwrap();
        assert!(!ui.drive_duplicate_sequence(&mailbox));
    }
}

#[test]
fn selected_duplicate_ready_mark_navigation_and_host_focus_aba_cannot_restart_tail() {
    for intent in 0..3 {
        let (mut ui, mailbox, clock, _) = ready();
        let sources = start(&mut ui);
        let (lease, key, snapshot, index) = receive(&ui, &mailbox, &sources[0]);
        commit(&mut ui, &mailbox, &clock, &key, &snapshot, index, started());
        let cleanup = candidate(&mut ui, &mailbox, &clock, &key, snapshot);
        let pane = install(&mut ui, &mailbox, &clock, cleanup);
        reattach(&mut ui, &mailbox, &clock, &pane);
        match intent {
            0 => {
                let state = ui.state.as_mut().unwrap();
                state.clear_tab_selection();
                state
                    .mark_tabs(&sources[1], TabSelectionChange::Toggle)
                    .unwrap();
                assert_eq!(state.selected_tab_ids(), sources);
            }
            1 => {
                ui.select_tab_generation(ClientAction::SelectTab(2));
            }
            _ => {
                let prior = ui.host_focus();
                ui.inherit_host_focus(Some(false));
                ui.inherit_host_focus(prior);
                assert_eq!(ui.host_focus(), prior);
            }
        }
        let cursors = mailbox.cursors();
        mailbox.finish_lease(lease).unwrap();
        ui.drive_duplicate_sequence(&mailbox);
        assert_eq!(
            mailbox.cursors(),
            cursors,
            "no new creation identity after cancellation"
        );
        let progress = ui.view().duplicate_progress().unwrap();
        assert_eq!(progress.stop, Some(DuplicateStop::Changed));
        assert_eq!(
            (progress.submitted, progress.committed, progress.started),
            (1, 1, 1)
        );
    }
}

#[test]
fn selected_duplicate_busy_until_total_expiry_never_allocates_or_sends_a_member() {
    let (mut ui, mailbox, clock, elapsed) = ready();
    let pane = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(pane, ())))
        .unwrap();
    let cursors = mailbox.cursors();
    ui.next_empty_poll = Some(clock.now());
    start(&mut ui);
    assert_eq!(
        ui.next_poll_delay(),
        None,
        "an expired ordinary poll timer cannot busy-spin"
    );
    assert_eq!(ui.view().duplicate_progress().unwrap().submitted, 0);
    for _ in 0..3 {
        ui.tick();
    }
    assert_eq!(
        mailbox.cursors(),
        cursors,
        "busy does not mint a replacement request"
    );
    elapsed.store(115_000, Ordering::SeqCst);
    ui.tick();
    let progress = ui.view().duplicate_progress().unwrap();
    assert_eq!(progress.stop, Some(DuplicateStop::Deadline));
    assert_eq!(
        (progress.submitted, progress.committed, progress.started),
        (0, 0, 0)
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Poll(_)));
    mailbox.finish_lease(lease).unwrap();
    ui.tick();
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(mailbox.cursors(), cursors);
}

#[test]
fn selected_duplicate_capacity_refusal_retains_committed_prefix_without_rollback() {
    let snapshot = super::super::tests::membership::snapshot(MAXIMUM_TABS_PER_CONTEXT - 1, 1);
    let (mut ui, mailbox, clock, _) =
        super::super::tests::workspace_setup::ready_snapshot(snapshot);
    let second = ui.state.as_ref().unwrap().cache().tabs()[1]
        .tab_id()
        .clone();
    ui.state
        .as_mut()
        .unwrap()
        .mark_tabs(&second, TabSelectionChange::Toggle)
        .unwrap();
    let sources = start(&mut ui);
    let (lease, key, snapshot, index) = receive(&ui, &mailbox, &sources[0]);
    commit(&mut ui, &mailbox, &clock, &key, &snapshot, index, started());
    let cleanup = candidate(&mut ui, &mailbox, &clock, &key, snapshot.clone());
    let pane = install(&mut ui, &mailbox, &clock, cleanup);
    reattach(&mut ui, &mailbox, &clock, &pane);
    mailbox.finish_lease(lease).unwrap();
    let cursors = mailbox.cursors();
    assert!(
        ui.drive_duplicate_sequence(&mailbox),
        "local refusal must render its retained progress"
    );
    assert_eq!(mailbox.cursors(), cursors);
    assert_eq!(
        ui.state.as_ref().unwrap().cache().canonical_snapshot(),
        &snapshot
    );
    let progress = ui.view().duplicate_progress().unwrap();
    assert_eq!(progress.stop, Some(DuplicateStop::NotCreated));
    assert_eq!(
        (progress.submitted, progress.committed, progress.started),
        (1, 1, 1)
    );
}

#[test]
fn selected_duplicate_exact_receipt_cannot_hide_unrelated_context_write() {
    let (mut ui, mailbox, clock, _) = ready();
    let sources = start(&mut ui);
    let (lease, key, mut snapshot, index) = receive(&ui, &mailbox, &sources[0]);
    commit(&mut ui, &mailbox, &clock, &key, &snapshot, index, started());
    let prior = ui.state.as_ref().unwrap().cache().clone();
    snapshot.terminal_contexts[1].groups[0].label = "not a single duplicate transition".into();
    mailbox
        .publish_event_before(
            WorkerEvent::CreateSnapshotResult {
                operation: key,
                outcome: WorkbenchSnapshotOutcome::Available { snapshot },
            },
            EventDeadline::Forward(clock.now() + std::time::Duration::from_secs(40)),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(ui.retirement_requested());
    assert_eq!(ui.state.as_ref().unwrap().cache(), &prior);
    let facts = ui
        .state
        .as_ref()
        .unwrap()
        .duplicate_facts_for_test()
        .unwrap();
    assert_eq!((facts.submitted, facts.committed, facts.started), (1, 1, 1));
    assert_eq!(facts.stop, Some(DuplicateStop::Recovery));
    let _ = mailbox.finish_lease(lease); // Retirement owns any unfinished lease cleanup.
}

#[test]
fn selected_duplicate_nonstarted_commit_halts_tail_and_keeps_truthful_counts_after_notice_expiry() {
    for outcome in [
        None,
        Some(WorkbenchTerminalOutcome::OutcomeUnknown {}),
        Some(WorkbenchTerminalOutcome::Failed {
            reason: WorkbenchTerminalFailureReason::TerminalCreateFailed,
        }),
        Some(WorkbenchTerminalOutcome::Unavailable {
            reason: WorkbenchTerminalUnavailableReason::TerminalRuntimeUnavailable,
        }),
    ] {
        let (mut ui, mailbox, clock, elapsed) = ready();
        let sources = start(&mut ui);
        let (lease, key, snapshot, index) = receive(&ui, &mailbox, &sources[0]);
        commit(&mut ui, &mailbox, &clock, &key, &snapshot, index, outcome);
        let progress = ui.view().duplicate_progress().unwrap();
        assert_eq!(
            (progress.submitted, progress.committed, progress.started),
            (1, 1, 0)
        );
        assert_eq!(progress.stop, Some(DuplicateStop::TerminalNotStarted));
        let cleanup = candidate(&mut ui, &mailbox, &clock, &key, snapshot);
        let pane = install(&mut ui, &mailbox, &clock, cleanup);
        reattach(&mut ui, &mailbox, &clock, &pane);
        mailbox.finish_lease(lease).unwrap();
        elapsed.store(6_000, Ordering::SeqCst);
        ui.tick();
        assert!(ui.view().duplicate_progress().is_none());
        assert_eq!(
            ui.state
                .as_ref()
                .unwrap()
                .duplicate_facts_for_test()
                .unwrap(),
            progress,
            "notice expiry retains sequence facts"
        );
        assert!(
            ui.state
                .as_ref()
                .unwrap()
                .settled_create_for_test()
                .is_some(),
            "original commit/provenance outlive status"
        );
        assert_eq!(ui.view().selected_tab_ids(), sources);
    }
}

#[test]
fn selected_duplicate_preflight_observes_unrelated_context_refresh_before_first_admission() {
    let (mut ui, mailbox, clock, _) = ready();
    let pane = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(pane, ())))
        .unwrap();
    start(&mut ui);
    let cursors = mailbox.cursors();
    let state = ui.state.as_mut().unwrap();
    let mut snapshot = state.cache().canonical_snapshot().clone();
    snapshot.revision = WorkbenchRevision::new(snapshot.revision.value() + 1);
    snapshot.terminal_contexts[1].groups[0].label = "Peer edit outside selected context".into();
    assert!(state.start_pending_refresh(44));
    let prepared = state.prepare_refresh(
        44,
        WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot.clone(),
        },
    );
    assert_eq!(
        state.apply_prepared_refresh(prepared, clock.now()),
        RefreshDecision::Continue
    );
    assert_eq!(state.cache().canonical_snapshot(), &snapshot);
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    assert!(ui.drive_duplicate_sequence(&mailbox));
    assert_eq!(mailbox.cursors(), cursors);
    assert!(mailbox.try_recv().unwrap().is_none());
    let progress = ui.view().duplicate_progress().unwrap();
    assert_eq!(progress.stop, Some(DuplicateStop::Changed));
    assert_eq!(
        (progress.submitted, progress.committed, progress.started),
        (0, 0, 0)
    );
}

#[test]
fn selected_duplicate_definite_precommit_refusal_expires_without_hiding_a_later_notice() {
    use crate::localization::Locale;
    let (mut ui, mailbox, clock, elapsed) = ready();
    let sources = start(&mut ui);
    let (lease, key, _, _) = receive(&ui, &mailbox, &sources[0]);
    assert!(matches!(
        reply(
            &mut ui,
            &mailbox,
            &clock,
            WorkerEvent::CreatePrecommit {
                operation: key,
                outcome: CreatePrecommitOutcome::NotSubmitted,
                applied_geometry: None,
            }
        ),
        UiReply::PrecommitRestored
    ));
    mailbox.finish_lease(lease).unwrap();
    let progress = ui.view().duplicate_progress().unwrap();
    assert_eq!(progress.stop, Some(DuplicateStop::NotCreated));
    assert_eq!(
        (progress.submitted, progress.committed, progress.started),
        (1, 0, 0)
    );
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let sequence_status = locale.native_status(&ui.view(), u16::MAX);
        ui.state
            .as_mut()
            .unwrap()
            .set_notice(SessionNotice::LocalBusy);
        assert_ne!(
            locale.native_status(&ui.view(), u16::MAX),
            sequence_status,
            "later notice wins"
        );
        ui.state.as_mut().unwrap().clear_notice();
    }
    elapsed.store(6_000, Ordering::SeqCst);
    ui.tick();
    assert!(ui.view().duplicate_progress().is_none());
    assert_eq!(
        ui.state.as_ref().unwrap().duplicate_facts_for_test(),
        Some(progress)
    );
}

#[test]
fn selected_duplicate_malformed_precommit_does_not_claim_definite_noncreation() {
    let (mut ui, mailbox, clock, _) = ready();
    let sources = start(&mut ui);
    let (lease, key, _, _) = receive(&ui, &mailbox, &sources[0]);
    let baseline = ui.state.as_ref().unwrap().cache().clone();
    let cursors = mailbox.cursors();
    mailbox
        .publish_event_before(
            WorkerEvent::CreatePrecommit {
                operation: key,
                outcome: CreatePrecommitOutcome::NotSubmitted,
                // This original read-only viewer cannot have an applied controller resize.
                applied_geometry: Some(Geometry::new(80, 24, 8, 16).unwrap()),
            },
            EventDeadline::Forward(clock.now() + std::time::Duration::from_secs(40)),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(ui.retirement_requested());
    let progress = ui.view().duplicate_progress().unwrap();
    assert_eq!(progress.stop, Some(DuplicateStop::Recovery));
    assert_eq!(
        (progress.submitted, progress.committed, progress.started),
        (1, 0, 0)
    );
    assert_eq!(ui.state.as_ref().unwrap().cache(), &baseline);
    assert_eq!(mailbox.cursors(), cursors);
    let _ = mailbox.finish_lease(lease);
}

#[test]
fn selected_duplicate_unknown_submission_halts_immediately_and_join_only_refines_committed_facts() {
    for late_commit in [false, true] {
        let (mut ui, mailbox, _, elapsed) = ready();
        let sources = start(&mut ui);
        let (lease, _, snapshot, index) = receive(&ui, &mailbox, &sources[0]);
        let QueuedCommand::Create(marker) = lease.record() else {
            panic!("Create")
        };
        let pending = late_commit.then(|| {
            PendingCreateCommit::new(
                marker.operation().clone(),
                receipt(&snapshot, index, started()),
            )
        });
        let cursors = mailbox.cursors();
        ui.retire(CarrierFailure::OutcomeUnknown);
        let progress = ui.view().duplicate_progress().unwrap();
        assert_eq!(progress.stop, Some(DuplicateStop::Recovery));
        assert_eq!(
            (progress.submitted, progress.committed, progress.started),
            (1, 0, 0)
        );
        assert!(ui.finalize_retirement(
            pending,
            None,
            None,
            crate::terminal_session::state::CloseWorkerReturn::Returned(None)
        ));
        let expected = usize::from(late_commit);
        let progress = ui.view().duplicate_progress().unwrap();
        assert_eq!((progress.committed, progress.started), (expected, expected));
        assert_eq!(progress.stop, Some(DuplicateStop::Recovery));
        let _ = mailbox.finish_lease(lease);
        ui.tick();
        elapsed.store(6_000, Ordering::SeqCst);
        ui.tick();
        assert_eq!(mailbox.cursors(), cursors);
        assert!(ui.view().duplicate_progress().is_none());
        assert_eq!(
            ui.state.as_ref().unwrap().duplicate_facts_for_test(),
            Some(progress)
        );
    }
}
