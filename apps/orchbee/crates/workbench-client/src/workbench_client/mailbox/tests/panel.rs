use super::*;
use crate::client_view::PanelDirection;
use crate::local_client::workbench::panel_close::WorkbenchPanelCloseAccepted;
use crate::local_client::workbench::panel_placement::WorkbenchPanelPlacementCommit;
use crate::local_client::workbench::panel_split::WorkbenchPanelSplitCommit;
use crate::workbench_state::{
    MembershipInstallDecision, PanelCommit, PanelDecision, PanelIntent, PanelOperation,
    PanelRequestKind, PanelResult, PendingPanelCommit,
};
use orchbee_workbench_protocol::{
    PanelOrientation, PanelSlot, WorkbenchPanelSlotId, WorkbenchSplitId,
};

const LEAF_A_PANEL: u128 = 5;
const LEAF_B_PANEL: u128 = 3_010;

fn leaf(panel: u128, presentation: u128, worker: u128, terminal: u128, label: &str) -> PanelNode {
    PanelNode::Panel {
        right_click: Default::default(),
        child_presentations: Vec::new(),
        panel_id: WorkbenchPanelId::parse(&id(panel)).unwrap(),
        primary_presentation: Presentation {
            custom_label: None,
            binding: Some(PresentationBinding {
                terminal_id: TerminalId::parse(&id(terminal)).unwrap(),
                worker_id: WorkerId::parse(&id(worker)).unwrap(),
            }),
            label: label.to_owned(),
            presentation_id: TerminalPresentationId::parse(&id(presentation)).unwrap(),
            launch_failure: None,
            agent_not_resumed: None,
            agent_start_failure: None,
            launch_program: None,
        },
    }
}

fn leaf_b() -> PanelNode {
    leaf(LEAF_B_PANEL, 3_011, 3_012, 3_013, "1-b")
}

fn split_root() -> PanelNode {
    PanelNode::Split {
        ratio: Default::default(),
        children: Box::new([
            PanelSlot {
                node: leaf(LEAF_A_PANEL, 4, 6, 7, "1"),
                slot_id: WorkbenchPanelSlotId::parse(&id(3_001)).unwrap(),
            },
            PanelSlot {
                node: leaf_b(),
                slot_id: WorkbenchPanelSlotId::parse(&id(3_014)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&id(3_020)).unwrap(),
    }
}

fn panel_snapshot(revision: u64, root: PanelNode) -> Snapshot {
    Snapshot {
        navigation: {
            let fixture_context_id = WorkbenchTerminalContextId::parse(&id(3)).unwrap();
            let fixture_workspace_id = orchbee_workbench_protocol::WorkbenchWorkspaceId::parse(
                fixture_context_id.as_str(),
            )
            .unwrap();
            Navigation {
                project_order: Vec::new(),
                directories: Vec::new(),
                workspaces: std::iter::once(orchbee_workbench_protocol::Workspace {
                    terminal_directory: None,
                    automatic_label: false,
                    workspace_id: fixture_workspace_id.clone(),
                    terminal_context_id: fixture_context_id.clone(),
                    project_id: None,
                    primary_directory_id: None,
                    label: "Workspace".into(),
                })
                .chain(Vec::new())
                .collect(),
                workspace_order: Vec::new(),
                projects: Vec::new(),
            }
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(revision),
        session_id: SessionId::parse(&id(2)).unwrap(),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            tabs: vec![Tab {
                automatic_label: false,
                group_id: None,
                label: "1".to_owned(),
                pinned: false,
                root,
                tab_id: WorkbenchTabId::parse(&id(1)).unwrap(),
            }],
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(3)).unwrap(),
        }],
    }
}

/// A ready, attached session whose single Tab is the two-leaf split (A selected, then B), so
/// panel.move has a real focus target and a superseding leaf-loss projection stays reachable.
fn split_ready_state() -> WorkbenchClientState {
    let (mut state, _target) = WorkbenchClientState::starting(
        project_preferred_workspace(&panel_snapshot(1, split_root())).unwrap(),
        1,
        geometry(80),
    )
    .unwrap();
    let key = state.desired().unwrap().clone();
    assert!(state.install_projection_t(&key));
    assert!(state.record_setup_configuration_t(
        &key,
        ControllerMode::Controlling(ControllerId::parse(&id(8)).unwrap()),
        Some(geometry(80)),
    ));
    assert!(state.stage_drawn_frame_t(&key, FrameIdentity::new(1)));
    assert!(state.mark_feedback_accepted_t(&key, FrameIdentity::new(1)));
    assert!(state.publish_ready_t(
        &key,
        FrameIdentity::new(1),
        ControllerMode::Controlling(ControllerId::parse(&id(8)).unwrap()),
        Some(geometry(80)),
    ));
    state
}

fn admit_panel(
    mailbox: &CommandMailbox,
    state: &mut WorkbenchClientState,
    clock: &dyn MailboxClock,
    deadline: Instant,
    kind: PanelRequestKind,
) -> Result<PanelOperation, MoveAdmissionError<()>> {
    mailbox.try_enqueue_panel_for_using(
        state,
        deadline,
        clock,
        |state, request_id| state.plan_panel(request_id, kind, deadline).ok_or(()),
        |state, operation| state.begin_panel(operation.clone()).then_some(()).ok_or(()),
    )
}

fn admit_move(
    mailbox: &CommandMailbox,
    state: &mut WorkbenchClientState,
    clock: &dyn MailboxClock,
    deadline: Instant,
) -> Result<PanelOperation, MoveAdmissionError<()>> {
    admit_panel(
        mailbox,
        state,
        clock,
        deadline,
        PanelRequestKind::Move {
            direction: PanelDirection::Right,
        },
    )
}

fn move_commit(operation: &PanelOperation, revision: u64) -> PanelCommit {
    let PanelIntent::Move {
        target_panel_id, ..
    } = operation.intent()
    else {
        panic!("fixture operation must carry a move intent");
    };
    PanelCommit::Move(WorkbenchPanelPlacementCommit {
        revision: WorkbenchRevision::new(revision),
        terminal_context_id: operation.baseline().terminal_context_id().unwrap().clone(),
        tab_id: operation.target().tab_id().clone(),
        panel_id: operation.panel_id().clone(),
        target_panel_id: target_panel_id.clone(),
    })
}

fn committed_move(operation: &PanelOperation, snapshot: Snapshot) -> PanelResult {
    let revision = snapshot.revision.value() - 1;
    PanelResult::Committed {
        commit: Box::new(move_commit(operation, revision)),
        outcome: WorkbenchSnapshotOutcome::Available { snapshot },
    }
}

fn publish_panel_result(
    mailbox: &CommandMailbox,
    operation: &PanelOperation,
    outcome: PanelResult,
    clock: &dyn MailboxClock,
) -> Result<UiWaitKey, EventError> {
    mailbox.publish_event_before_using(
        WorkerEvent::PanelResult {
            operation: operation.clone(),
            outcome,
        },
        EventDeadline::PlacementOnly(operation.deadline()),
        clock,
    )
}

struct StagedCleanup {
    operation: PanelOperation,
    cleanup: MembershipCleanup,
    lease: CommandLease,
}

/// Drives a committed panel.move through the superseding leaf-loss path up to the staged
/// membership cleanup: the Panel flight is still pending behind the deposited Cleanup receipt.
fn staged_leaf_loss_cleanup(
    mailbox: &CommandMailbox,
    state: &mut WorkbenchClientState,
    clock: &dyn MailboxClock,
    base: Instant,
) -> StagedCleanup {
    let deadline = base + Duration::from_secs(50);
    let operation = admit_move(mailbox, state, clock, deadline).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_panel_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    let outcome = committed_move(&operation, panel_snapshot(3, leaf_b()));
    let wait = publish_panel_result(mailbox, &operation, outcome.clone(), clock).unwrap();
    mailbox.take_event_before_using(clock).unwrap().unwrap();
    // The UI decides on the taken event: retain the committed fact, then stage the paired
    // membership cleanup before depositing the Cleanup settlement reply.
    let prepared = state.prepare_panel(&operation, outcome);
    assert!(prepared.membership_candidate().is_some());
    let (retained, candidate) = state
        .retain_prepared_panel_membership(prepared)
        .expect("committed leaf-loss membership must retain");
    assert_eq!(&retained, &operation);
    let cleanup = state
        .stage_panel_membership_cleanup(
            operation.request_id(),
            candidate,
            OperationDeadlines::from_sample(base).unwrap(),
        )
        .unwrap();
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, clock, |_| {
                Ok(UiReply::Panel(PanelSettlement::new(
                    PanelDecision::Cleanup(cleanup.clone()),
                    state.desired().unwrap().clone(),
                )))
            })
            .unwrap(),
        MutationResult::Applied
    );
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Panel(_))
    ));
    StagedCleanup {
        operation,
        cleanup,
        lease,
    }
}

#[test]
fn panel_admission_marks_queue_and_fences_identity() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    // A wrong-target operation is refused before the commit closure runs and burns nothing.
    let baseline = state.cache().clone();
    let mut committed = false;
    let before = mailbox.cursors();
    let invalid = mailbox.try_enqueue_panel_for_using(
        &mut state,
        deadline,
        &clock,
        |_, request_id| {
            Ok::<_, ()>(PanelOperation::new(
                request_id,
                key(9, 9),
                WorkbenchPanelId::parse(&id(LEAF_A_PANEL)).unwrap(),
                baseline,
                PanelRequestKind::Close,
                PanelIntent::Close,
                deadline,
            ))
        },
        |_, _| {
            committed = true;
            Ok(())
        },
    );
    assert!(matches!(invalid, Err(MoveAdmissionError::InvalidTarget)));
    assert!(
        !committed,
        "rejected admission must not invoke the commit closure"
    );
    assert_eq!(mailbox.cursors(), before);
    let operation = admit_move(&mailbox, &mut state, &clock, deadline).unwrap();
    assert_eq!(operation.target(), &key(1, 1));
    assert_eq!(
        operation.panel_id(),
        &WorkbenchPanelId::parse(&id(LEAF_A_PANEL)).unwrap()
    );
    {
        let locked = mailbox.lock();
        assert!(matches!(locked.queue.back().map(|record| &record.command),
            Some(QueuedCommand::Panel(marker)) if marker.operation() == &operation));
        assert!(matches!(&locked.structural_mutation_pending,
            Some(StructuralMutationFlight::Panel(flight))
                if flight.operation == operation
                    && flight.stage == PendingRefreshStage::Queued
                    && flight.serial == 1));
    }
    assert!(mailbox.cursors().2 > before.2);
    assert!(matches!(
        admit_panel(
            &mailbox,
            &mut state,
            &clock,
            deadline,
            PanelRequestKind::Move {
                direction: PanelDirection::Left,
            },
        ),
        Err(MoveAdmissionError::Busy)
    ));
    mailbox.close(CarrierFailure::Closed);
    assert!(matches!(
        admit_panel(
            &mailbox,
            &mut state,
            &clock,
            deadline,
            PanelRequestKind::Close,
        ),
        Err(MoveAdmissionError::Closed(_))
    ));
}

#[test]
fn panel_settlement_reaches_closure_and_consumes_result_once() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let operation = admit_move(&mailbox, &mut state, &clock, deadline).unwrap();
    assert_eq!(
        mailbox.mark_panel_in_flight(&operation).unwrap(),
        MutationResult::Stale,
        "marking before the worker lease is not applied"
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(lease.serial(), 1);
    assert_eq!(
        mailbox.mark_panel_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    let outcome = committed_move(&operation, panel_snapshot(3, split_root()));
    let wait = publish_panel_result(&mailbox, &operation, outcome.clone(), &clock).unwrap();
    let taken = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert_eq!(taken.wait_key, Some(wait.clone()));
    assert!(matches!(
        taken.event,
        WorkerEvent::PanelResult {
            operation: ref actual,
            ..
        } if actual == &operation
    ));
    let settled = AtomicBool::new(false);
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                settled.store(true, Ordering::SeqCst);
                let prepared = state.prepare_panel(&operation, outcome);
                assert!(prepared.membership_candidate().is_none());
                let decision = state.apply_prepared_panel(prepared);
                assert_eq!(decision, PanelDecision::Continue);
                Ok(UiReply::Panel(PanelSettlement::new(
                    decision,
                    state.desired().unwrap().clone(),
                )))
            })
            .unwrap(),
        MutationResult::Applied
    );
    assert!(settled.load(Ordering::SeqCst));
    assert!(mailbox.lock().structural_mutation_pending.is_none());
    // The superseding candidate kept both leaves: the committed move merges and its commit fact
    // stays attributed to the exact request that produced it.
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &operation
    );
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                panic!("an already consumed wait must never reach the settlement closure")
            })
            .unwrap(),
        MutationResult::Stale
    );
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Panel(_))
    ));
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn panel_result_placement_requires_inflight_anchor_and_deadline() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);

    // A queued-but-unmarked flight refuses the worker result.
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let operation = admit_panel(
        &mailbox,
        &mut state,
        &clock,
        deadline,
        PanelRequestKind::Close,
    )
    .unwrap();
    let _lease = mailbox.try_recv().unwrap().unwrap();
    let result = publish_panel_result(
        &mailbox,
        &operation,
        PanelResult::Unchanged(WorkbenchRevision::new(1)),
        &clock,
    );
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Protocol)
    );

    // A deadline that is not the exact captured PlacementOnly fails closed.
    for wrong in [
        EventDeadline::Forward(deadline),
        EventDeadline::PlacementOnly(deadline + Duration::from_secs(1)),
    ] {
        let mailbox = bootstrapped_mailbox(1, 5);
        let mut state = split_ready_state();
        let operation = admit_panel(
            &mailbox,
            &mut state,
            &clock,
            deadline,
            PanelRequestKind::Close,
        )
        .unwrap();
        let _lease = mailbox.try_recv().unwrap().unwrap();
        mailbox.mark_panel_in_flight(&operation).unwrap();
        let result = mailbox.publish_event_before_using(
            WorkerEvent::PanelResult {
                operation,
                outcome: PanelResult::Unchanged(WorkbenchRevision::new(1)),
            },
            wrong,
            &clock,
        );
        assert_eq!(
            result.unwrap_err().closed_failure(),
            Some(CarrierFailure::Protocol)
        );
    }

    // A result carrying request B's operation cannot land on request A's flight.
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let first = admit_panel(
        &mailbox,
        &mut state,
        &clock,
        deadline,
        PanelRequestKind::Close,
    )
    .unwrap();
    let _lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_panel_in_flight(&first).unwrap();
    let other_mailbox = bootstrapped_mailbox(1, 6);
    let mut other_state = split_ready_state();
    let second = admit_panel(
        &other_mailbox,
        &mut other_state,
        &clock,
        deadline,
        PanelRequestKind::Close,
    )
    .unwrap();
    assert_ne!(first.request_id(), second.request_id());
    let result = publish_panel_result(
        &mailbox,
        &second,
        PanelResult::Unchanged(WorkbenchRevision::new(1)),
        &clock,
    );
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Protocol)
    );

    // A result published after the lease moved on to another serial fails closed.
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let operation = admit_panel(
        &mailbox,
        &mut state,
        &clock,
        deadline,
        PanelRequestKind::Close,
    )
    .unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_panel_in_flight(&operation).unwrap();
    mailbox.finish_lease(lease).unwrap();
    mailbox.try_enqueue(input(&mailbox, 0)).unwrap();
    assert_eq!(mailbox.try_recv().unwrap().unwrap().serial(), 2);
    let result = publish_panel_result(
        &mailbox,
        &operation,
        PanelResult::Unchanged(WorkbenchRevision::new(1)),
        &clock,
    );
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Protocol)
    );

    // A result that only arrives after the mailbox retired is refused without a protocol error.
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let operation = admit_panel(
        &mailbox,
        &mut state,
        &clock,
        deadline,
        PanelRequestKind::Close,
    )
    .unwrap();
    mailbox.close(CarrierFailure::Closed);
    let result = publish_panel_result(
        &mailbox,
        &operation,
        PanelResult::Unchanged(WorkbenchRevision::new(1)),
        &clock,
    );
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Closed)
    );
}

#[test]
fn panel_reply_mismatches_close_protocol_without_running_the_settlement() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let outcome =
        |operation: &PanelOperation| committed_move(operation, panel_snapshot(3, split_root()));

    // Depositing while the published event is still untaken fails closed: place_panel_result
    // already set the flight to AwaitingUi, so the untaken-event gate is what fires.
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let operation = admit_move(&mailbox, &mut state, &clock, deadline).unwrap();
    let _lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_panel_in_flight(&operation).unwrap();
    let wait = publish_panel_result(&mailbox, &operation, outcome(&operation), &clock).unwrap();
    let settled = AtomicBool::new(false);
    let result = mailbox.deposit_ui_reply_before_using(&wait, &clock, |_| {
        settled.store(true, Ordering::SeqCst);
        Ok(UiReply::Panel(PanelSettlement::new(
            PanelDecision::Continue,
            pane_key(&key(1, 1)),
        )))
    });
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Protocol)
    );
    assert!(!settled.load(Ordering::SeqCst));

    // A stale-serial wait is refused without disturbing the current wait; a future serial that
    // cannot exist yet is a protocol violation. Both leave the settlement closure unrun.
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let operation = admit_move(&mailbox, &mut state, &clock, deadline).unwrap();
    let _lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_panel_in_flight(&operation).unwrap();
    let wait = publish_panel_result(&mailbox, &operation, outcome(&operation), &clock).unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    let mut stale = wait.clone();
    stale.serial -= 1;
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&stale, &clock, |_| {
                panic!("a stale wait must never reach the settlement closure")
            })
            .unwrap(),
        MutationResult::Stale
    );
    let mut future = wait.clone();
    future.serial += 1;
    let settled = AtomicBool::new(false);
    let result = mailbox.deposit_ui_reply_before_using(&future, &clock, |_| {
        settled.store(true, Ordering::SeqCst);
        Ok(UiReply::Panel(PanelSettlement::new(
            PanelDecision::Continue,
            pane_key(&key(1, 1)),
        )))
    });
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Protocol)
    );
    assert!(!settled.load(Ordering::SeqCst));

    // A deposit arriving after the worker lease moved on to another serial fails closed: the
    // reply can no longer be attributed to the serial that owned the event anchor.
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let operation = admit_move(&mailbox, &mut state, &clock, deadline).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_panel_in_flight(&operation).unwrap();
    let wait = publish_panel_result(&mailbox, &operation, outcome(&operation), &clock).unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    mailbox.try_enqueue(input(&mailbox, 0)).unwrap();
    assert_eq!(mailbox.try_recv().unwrap().unwrap().serial(), 2);
    let settled = AtomicBool::new(false);
    let result = mailbox.deposit_ui_reply_before_using(&wait, &clock, |_| {
        settled.store(true, Ordering::SeqCst);
        Ok(UiReply::Panel(PanelSettlement::new(
            PanelDecision::Continue,
            pane_key(&key(1, 1)),
        )))
    });
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Protocol)
    );
    assert!(!settled.load(Ordering::SeqCst));

    // The exact wait still settles the original request after every refused forgery attempt.
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let operation = admit_move(&mailbox, &mut state, &clock, deadline).unwrap();
    let _lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_panel_in_flight(&operation).unwrap();
    let wait = publish_panel_result(&mailbox, &operation, outcome(&operation), &clock).unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                let prepared = state.prepare_panel(&operation, outcome(&operation));
                Ok(UiReply::Panel(PanelSettlement::new(
                    state.apply_prepared_panel(prepared),
                    state.desired().unwrap().clone(),
                )))
            })
            .unwrap(),
        MutationResult::Applied
    );
}

#[test]
fn panel_membership_cleanup_holds_flight_until_exact_install() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let staged = staged_leaf_loss_cleanup(&mailbox, &mut state, &clock, base);
    let operation = staged.operation;
    let cleanup = staged.cleanup;
    assert!(matches!(
        cleanup.result(),
        MembershipResultKey::Panel(request_id) if request_id == operation.request_id()
    ));
    assert_eq!(cleanup.old(), &key(1, 1));
    // The cleanup receipt is not a settlement: the flight stays pending until the membership
    // result for the exact old lease arrives, and no new request can interleave.
    assert!(matches!(
        mailbox.lock().structural_mutation_pending,
        Some(StructuralMutationFlight::Panel(_))
    ));
    assert!(matches!(
        admit_panel(
            &mailbox,
            &mut state,
            &clock,
            base + Duration::from_secs(50),
            PanelRequestKind::Close,
        ),
        Err(MoveAdmissionError::Busy)
    ));
    let detached_wait = mailbox
        .publish_event_before_using(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: cleanup.old().clone(),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    let taken = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert_eq!(taken.wait_key, Some(detached_wait.clone()));
    let settled = AtomicBool::new(false);
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&detached_wait, &clock, |transaction| {
                settled.store(true, Ordering::SeqCst);
                let prepared = state
                    .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
                    .unwrap();
                let next_key = key(state.desired().unwrap().generation() + 1, 1);
                let decision = state
                    .prepare_cleaned_membership_install(&cleanup, prepared, Some(next_key))
                    .unwrap()
                    .commit();
                let MembershipInstallDecision::Attach(target) = decision else {
                    panic!("a surviving attachable leaf must produce a re-attach plan");
                };
                let install = transaction
                    .prepare_desired_install(DesiredInstall::Fresh {
                        tab_id: WorkbenchTabId::parse(&id(1)).unwrap(),
                        panel: target.selected().key().panel().clone(),
                    })
                    .unwrap();
                let desired = install.desired().cloned();
                install.commit();
                Ok(UiReply::Membership(RefreshSettlement::new(
                    RefreshDecision::AttachRecovered(target),
                    desired,
                )))
            })
            .unwrap(),
        MutationResult::Applied
    );
    assert!(settled.load(Ordering::SeqCst));
    // Old-lease cleanup and the new install settle atomically: the desired attachment advanced
    // to the fresh generation and the flight was consumed exactly once.
    assert_eq!(
        mailbox.lock().desired.key().map(|pane| pane.anchor()),
        Some(&key(2, 1))
    );
    assert!(mailbox.lock().structural_mutation_pending.is_none());
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(3));
    assert_eq!(state.desired().map(|pane| pane.anchor()), Some(&key(2, 1)));
    assert!(state.attached_key().is_none());
    assert_eq!(state.phase_kind(), ClientPhaseKind::Switching);
    assert!(state.invariant_holds());
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&detached_wait, &clock, |_| {
                panic!("an already consumed wait must never reach the settlement closure")
            })
            .unwrap(),
        MutationResult::Stale
    );
    assert!(matches!(
        mailbox.take_deposited_reply(&detached_wait).unwrap(),
        Some(UiReply::Membership(_))
    ));
    // A late panel result for the consumed request has nowhere to land and fails closed.
    let result = publish_panel_result(
        &mailbox,
        &operation,
        PanelResult::Unchanged(WorkbenchRevision::new(3)),
        &clock,
    );
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Protocol)
    );
    drop(staged.lease);
}

#[test]
fn panel_membership_reply_rejects_a_foreign_detached_lease() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let staged = staged_leaf_loss_cleanup(&mailbox, &mut state, &clock, base);
    let cleanup = staged.cleanup;
    // The worker reports detaching a lease that is not this request's attachment target.
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::MembershipDetached {
                cleanup: cleanup.clone(),
                detached: key(9, 9),
            },
            EventDeadline::TotalCleanup(cleanup.deadlines().total()),
            &clock,
        )
        .unwrap();
    let taken = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert_eq!(taken.wait_key, Some(wait.clone()));
    let settled = AtomicBool::new(false);
    let result = mailbox.deposit_ui_reply_before_using(&wait, &clock, |_| {
        settled.store(true, Ordering::SeqCst);
        Ok(UiReply::Membership(RefreshSettlement::new(
            RefreshDecision::Continue,
            pane_key(&key(1, 1)),
        )))
    });
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Protocol)
    );
    assert!(!settled.load(Ordering::SeqCst));
    drop(staged.lease);
}

#[test]
fn panel_close_accepted_receipt_preserves_committed_fact_and_publication_debt() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let first = admit_move(&mailbox, &mut state, &clock, deadline).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_panel_in_flight(&first).unwrap();
    let outcome = committed_move(&first, panel_snapshot(3, split_root()));
    let wait = publish_panel_result(&mailbox, &first, outcome.clone(), &clock).unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    mailbox
        .deposit_ui_reply_before_using(&wait, &clock, |_| {
            let prepared = state.prepare_panel(&first, outcome);
            let decision = state.apply_prepared_panel(prepared);
            assert_eq!(decision, PanelDecision::Continue);
            Ok(UiReply::Panel(PanelSettlement::new(
                decision,
                state.desired().unwrap().clone(),
            )))
        })
        .unwrap();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &first,
        "the committed move fact must be recorded before the receipt test"
    );

    // A panel close receipt is publication debt, not a merged projection: it consumes the new
    // request's flight once, keeps the earlier commit fact, and marks publication owed.
    let second = admit_panel(
        &mailbox,
        &mut state,
        &clock,
        deadline,
        PanelRequestKind::Close,
    )
    .unwrap();
    assert_ne!(first.request_id(), second.request_id());
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(lease.serial(), 2);
    mailbox.mark_panel_in_flight(&second).unwrap();
    let accepted = WorkbenchPanelCloseAccepted {
        close_request_id: second.request_id().clone(),
        terminal_context_id: second.baseline().terminal_context_id().unwrap().clone(),
        tab_id: second.target().tab_id().clone(),
        panel_id: second.panel_id().clone(),
        accepted_revision: second.baseline().revision(),
    };
    let wait = publish_panel_result(
        &mailbox,
        &second,
        PanelResult::CloseAccepted(accepted),
        &clock,
    )
    .unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    let settled = AtomicBool::new(false);
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                settled.store(true, Ordering::SeqCst);
                let prepared = state.prepare_panel(
                    &second,
                    PanelResult::CloseAccepted(WorkbenchPanelCloseAccepted {
                        close_request_id: second.request_id().clone(),
                        terminal_context_id: second
                            .baseline()
                            .terminal_context_id()
                            .unwrap()
                            .clone(),
                        tab_id: second.target().tab_id().clone(),
                        panel_id: second.panel_id().clone(),
                        accepted_revision: second.baseline().revision(),
                    }),
                );
                let decision = state.apply_prepared_panel(prepared);
                assert_eq!(decision, PanelDecision::Continue);
                Ok(UiReply::Panel(PanelSettlement::new(
                    decision,
                    state.desired().unwrap().clone(),
                )))
            })
            .unwrap(),
        MutationResult::Applied
    );
    assert!(settled.load(Ordering::SeqCst));
    assert!(state.publication_debt_pending());
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &first,
        "a receipt-only close must not erase the earlier committed move fact"
    );
    assert!(mailbox.lock().structural_mutation_pending.is_none());
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                panic!("an already consumed wait must never reach the settlement closure")
            })
            .unwrap(),
        MutationResult::Stale
    );
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn panel_commit_facts_stay_attributed_to_their_own_request_at_retirement() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = split_ready_state();
    let first = admit_move(&mailbox, &mut state, &clock, deadline).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_panel_in_flight(&first).unwrap();
    let outcome = committed_move(&first, panel_snapshot(3, split_root()));
    let commit = match &outcome {
        PanelResult::Committed { commit, .. } => (**commit).clone(),
        _ => panic!("fixture must be a committed result"),
    };
    let wait = publish_panel_result(&mailbox, &first, outcome.clone(), &clock).unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    mailbox
        .deposit_ui_reply_before_using(&wait, &clock, |_| {
            let prepared = state.prepare_panel(&first, outcome);
            Ok(UiReply::Panel(PanelSettlement::new(
                state.apply_prepared_panel(prepared),
                state.desired().unwrap().clone(),
            )))
        })
        .unwrap();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap();
    let second = admit_panel(
        &mailbox,
        &mut state,
        &clock,
        deadline,
        PanelRequestKind::Split {
            direction: PanelDirection::Right,
        },
    )
    .unwrap();
    assert_ne!(first.request_id(), second.request_id());
    mailbox.close(CarrierFailure::Closed);

    // Request A's known commit is already retained; re-presenting the exact same fact is an
    // idempotent no-op rather than a new settlement.
    assert_eq!(state.retained_panel_for_test().unwrap().operation(), &first);
    let a_pending = PendingPanelCommit::new(first.clone(), commit.clone());
    assert!(state.retain_pending_panel(a_pending.clone()));
    // But a fact fabricated for A that breaks the revision chain cannot be smuggled in while B
    // owns the pending slot.
    let mut forged = match commit.clone() {
        PanelCommit::Move(mut commit) => {
            commit.revision = WorkbenchRevision::new(3);
            commit
        }
        _ => panic!("fixture must be a move commit"),
    };
    assert!(!state.retain_pending_panel(PendingPanelCommit::new(
        first.clone(),
        PanelCommit::Move(forged.clone()),
    )));
    forged.tab_id = WorkbenchTabId::parse(&id(9)).unwrap();
    assert!(!state.retain_pending_panel(PendingPanelCommit::new(
        first.clone(),
        PanelCommit::Move(forged),
    )));
    // A fabricated commit for B that breaks the baseline+1 revision chain is refused, while B's
    // own exact commit is admitted and replaces A's retained fact at retirement.
    assert!(matches!(second.intent(), PanelIntent::Split { .. }));
    let split_commit = |revision: u64| {
        PanelCommit::Split(WorkbenchPanelSplitCommit::Pending {
            revision: WorkbenchRevision::new(revision),
            terminal_context_id: second.baseline().terminal_context_id().unwrap().clone(),
            tab_id: second.target().tab_id().clone(),
            panel_id: second.panel_id().clone(),
            new_panel_id: WorkbenchPanelId::parse(&id(4_010)).unwrap(),
            split_id: WorkbenchSplitId::parse(&id(4_020)).unwrap(),
            first_slot_id: WorkbenchPanelSlotId::parse(&id(4_001)).unwrap(),
            second_slot_id: WorkbenchPanelSlotId::parse(&id(4_014)).unwrap(),
            presentation_id: TerminalPresentationId::parse(&id(4_004)).unwrap(),
        })
    };
    assert!(!state.retain_pending_panel(PendingPanelCommit::new(second.clone(), split_commit(3),)));
    assert!(state.retain_pending_panel(PendingPanelCommit::new(second.clone(), split_commit(4),)));
    // With B's commit retained and B owning the pending slot, A's own commit is still
    // self-valid: the identical pending retains on a fresh state where A is the pending
    // operation. Re-presented here it must be refused by the ownership guard alone, leaving
    // B's retained fact untouched.
    let replay_mailbox = bootstrapped_mailbox(1, 5);
    let mut replay_state = split_ready_state();
    let replay_first = admit_move(&replay_mailbox, &mut replay_state, &clock, deadline).unwrap();
    assert_eq!(replay_first, first);
    assert!(replay_state.retain_pending_panel(a_pending.clone()));
    assert_ne!(state.retained_panel_for_test(), Some(&a_pending));
    assert!(!state.retain_pending_panel(a_pending));
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &second,
        "a foreign request's commit cannot evict B's retained fact"
    );
    assert!(state.finalize_retirement(CarrierFailure::Closed, None));
    assert!(state.invariant_holds());
    assert_eq!(
        state.retained_panel_for_test().unwrap().operation(),
        &second,
        "only B's own exact commit can replace A's retained fact"
    );
    drop(lease);
}
