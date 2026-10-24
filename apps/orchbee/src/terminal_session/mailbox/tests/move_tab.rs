use super::*;
use orchbee_workbench_protocol::WorkbenchTabMoveDirection;

fn admit(
    mailbox: &CommandMailbox,
    deadline: Instant,
    clock: &dyn MailboxClock,
) -> Result<MoveOperation, MoveAdmissionError<()>> {
    mailbox.try_enqueue_move_for_using(
        &mut (),
        deadline,
        clock,
        |(), request_id| {
            Ok(MoveOperation::new(
                request_id,
                key(1, 1),
                projected_tabs(3),
                WorkbenchTabMoveDirection::Next,
                deadline,
            ))
        },
        |(), _| Ok(()),
    )
}

#[test]
fn move_admission_refusals_preserve_identity_and_attachment() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    for now in [deadline, deadline + Duration::from_nanos(1)] {
        let mailbox = bootstrapped_mailbox(1, 5);
        let before = mailbox.cursors();
        assert!(matches!(
            admit(&mailbox, deadline, &FakeClock::new(now)),
            Err(MoveAdmissionError::Deadline)
        ));
        assert_eq!(mailbox.cursors(), before);
        assert!(mailbox.lock().queue.is_empty());
        assert_eq!(
            mailbox.lock().desired.key().map(|pane| pane.anchor()),
            Some(&key(1, 1))
        );
    }
    let mailbox = bootstrapped_mailbox(1, u128::MAX);
    let before = mailbox.cursors();
    assert!(matches!(
        admit(&mailbox, deadline, &FakeClock::new(base)),
        Err(MoveAdmissionError::IdentityExhausted)
    ));
    assert_eq!(mailbox.cursors(), before);
    assert!(mailbox.lock().closed.is_none());

    let mailbox = bootstrapped_mailbox(1, 5);
    for index in 0..COMMAND_CAPACITY {
        mailbox
            .try_enqueue(resize(&mailbox, 40 + u16::try_from(index).unwrap()))
            .unwrap();
    }
    let before = mailbox.cursors();
    assert!(matches!(
        admit(&mailbox, deadline, &FakeClock::new(base)),
        Err(MoveAdmissionError::Full)
    ));
    assert_eq!(mailbox.cursors(), before);

    let mailbox = bootstrapped_mailbox(1, 5);
    let before = mailbox.cursors();
    let mut committed = false;
    let result = mailbox.try_enqueue_move_for_using(
        &mut committed,
        deadline,
        &FakeClock::new(base),
        |_, id| {
            Ok::<_, ()>(MoveOperation::new(
                id,
                key(2, 1),
                projected_tabs(3),
                WorkbenchTabMoveDirection::Next,
                deadline,
            ))
        },
        |committed, _| {
            *committed = true;
            Ok(())
        },
    );
    assert!(matches!(result, Err(MoveAdmissionError::InvalidTarget)));
    assert!(!committed);
    assert_eq!(mailbox.cursors(), before);
    let result = mailbox.try_enqueue_move_for_using(
        &mut (),
        deadline,
        &FakeClock::new(base),
        |(), _| Err::<MoveOperation, _>("build"),
        |(), _| Ok(()),
    );
    assert!(matches!(result, Err(MoveAdmissionError::Build("build"))));
    assert_eq!(mailbox.cursors(), before);
}

#[test]
fn move_fence_releases_on_deadline_free_ui_settlement_before_worker_reads_reply() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(deadline - Duration::from_nanos(1));
    let mailbox = bootstrapped_mailbox(1, 5);
    let operation = admit(&mailbox, deadline, &clock).unwrap();
    assert_eq!(mailbox.cursors(), (1, 0, 6));
    assert_eq!(operation.target(), &key(1, 1));
    assert_eq!(operation.deadline(), deadline);
    assert_eq!(
        mailbox.mark_move_in_flight(&operation).unwrap(),
        MutationResult::Stale
    );
    assert!(matches!(
        admit(&mailbox, deadline, &clock),
        Err(MoveAdmissionError::Busy)
    ));
    assert!(matches!(
        mailbox.try_enqueue_refresh(deadline, |_| Ok::<_, ()>(())),
        Err(AdmissionError::Full)
    ));
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_move_in_flight(&operation).unwrap(),
        MutationResult::Applied
    );
    assert!(matches!(
        admit(&mailbox, deadline, &clock),
        Err(MoveAdmissionError::Busy)
    ));
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::MoveResult {
                operation: operation.clone(),
                outcome: MoveResult::Unchanged(WorkbenchRevision::new(1)),
            },
            EventDeadline::PlacementOnly(deadline),
            &clock,
        )
        .unwrap();
    assert!(matches!(
        admit(&mailbox, deadline, &clock),
        Err(MoveAdmissionError::Busy)
    ));
    clock.set(deadline + Duration::from_secs(100));
    assert!(mailbox.take_event_before_using(&clock).unwrap().is_some());
    mailbox
        .deposit_ui_reply_before_using(&wait, &clock, |_| {
            Ok(UiReply::Move(MoveSettlement::new(
                MoveDecision::Continue,
                pane_key(&key(1, 1)),
            )))
        })
        .unwrap();
    let second = admit(&mailbox, deadline + Duration::from_secs(200), &clock).unwrap();
    assert_ne!(second.request_id(), operation.request_id());
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Move(_))
    ));
    mailbox.finish_lease(lease).unwrap();
    assert!(mailbox.try_recv().unwrap().is_some());
}

#[test]
fn move_admission_samples_again_after_lock_contention_without_burning_id() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let (clock, reached, resume) = ObservedClock::with_contention_checkpoint(base);
    let clock = Arc::new(clock);
    let mailbox = Arc::new(bootstrapped_mailbox(1, 5));
    let before = mailbox.cursors();
    let held = mailbox.lock();
    let task_mailbox = Arc::clone(&mailbox);
    let task_clock = Arc::clone(&clock);
    let task = std::thread::spawn(move || admit(&task_mailbox, deadline, task_clock.as_ref()));
    reached.recv_timeout(Duration::from_secs(1)).unwrap();
    clock.set(deadline);
    resume.send(()).unwrap();
    assert!(matches!(
        task.join().unwrap(),
        Err(MoveAdmissionError::Deadline)
    ));
    drop(held);
    assert_eq!(mailbox.cursors(), before);
}

#[test]
fn move_typed_result_must_be_deposited_before_captured_deadline() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let operation = admit(&mailbox, deadline, &clock).unwrap();
    let _lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_move_in_flight(&operation).unwrap();
    clock.set(deadline);
    let result = mailbox.publish_event_before_using(
        WorkerEvent::MoveResult {
            operation,
            outcome: MoveResult::NotSubmitted,
        },
        EventDeadline::PlacementOnly(deadline),
        &clock,
    );
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Deadline)
    );
}

#[test]
fn canonical_move_a_settlement_then_b_queued_preserves_exact_a_commit_at_retirement() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = ready_state(3);
    let first = mailbox
        .try_enqueue_move_for_using(
            &mut state,
            deadline,
            &clock,
            |state, request_id| {
                state
                    .plan_move(request_id, WorkbenchTabMoveDirection::Next, deadline)
                    .ok_or(())
            },
            |state, operation| state.begin_move(operation.clone()).then_some(()).ok_or(()),
        )
        .unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    mailbox.mark_move_in_flight(&first).unwrap();
    let commit = crate::local_client::workbench::WorkbenchTabMoveCommit {
        revision: WorkbenchRevision::new(2),
        terminal_context_id: first.baseline().terminal_context_id().clone(),
        tab_id: first.target().tab_id().clone(),
    };
    let tabs = [1, 0, 2]
        .into_iter()
        .map(|index| {
            let tab = &first.baseline().tabs()[index];
            Tab {
                automatic_label: false,
                tab_id: tab.tab_id().clone(),
                label: tab.tab_label().as_str().to_owned(),
                group_id: None,
                pinned: false,
                root: PanelNode::Panel {
                    right_click: Default::default(),
                    panel_id: tab.panel_id().clone(),
                    child_presentations: vec![],
                    primary_presentation: Presentation {
                        custom_label: None,
                        presentation_id: tab.presentation_id().clone(),
                        label: tab.presentation_label().as_str().to_owned(),
                        binding: Some(PresentationBinding {
                            worker_id: tab.binding().expect("bound fixture").worker_id().clone(),
                            terminal_id: tab
                                .binding()
                                .expect("bound fixture")
                                .terminal_id()
                                .clone(),
                        }),
                        launch_failure: None,
                    },
                },
            }
        })
        .collect();
    let outcome = MoveResult::Committed {
        commit: commit.clone(),
        outcome: WorkbenchSnapshotOutcome::Available {
            snapshot: Snapshot {
                protocol_version: ProtocolVersion,
                revision: commit.revision,
                site_id: first.baseline().site_id().clone(),
                navigation: Navigation {
                    project_order: Vec::new(),
                    directories: Vec::new(),
                    workspaces: Vec::new(),
                    workspace_order: Vec::new(),
                    projects: vec![],
                    standalone_terminal_context_id: commit.terminal_context_id.clone(),
                },
                terminal_contexts: vec![TerminalContext {
                    terminal_context_id: commit.terminal_context_id.clone(),
                    groups: vec![],
                    tabs,
                }],
            },
        },
    };
    let prepared = state.prepare_move(&first, outcome.clone());
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::MoveResult {
                operation: first.clone(),
                outcome,
            },
            EventDeadline::PlacementOnly(deadline),
            &clock,
        )
        .unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    mailbox
        .deposit_ui_reply_before_using(&wait, &clock, |_| {
            let decision = state.apply_prepared_move(prepared);
            assert_eq!(decision, MoveDecision::Continue);
            Ok(UiReply::Move(MoveSettlement::new(
                decision,
                state.desired().unwrap().clone(),
            )))
        })
        .unwrap();
    let second = mailbox
        .try_enqueue_move_for_using(
            &mut state,
            deadline,
            &clock,
            |state, request_id| {
                state
                    .plan_move(request_id, WorkbenchTabMoveDirection::Next, deadline)
                    .ok_or(())
            },
            |state, operation| state.begin_move(operation.clone()).then_some(()).ok_or(()),
        )
        .unwrap();
    assert_ne!(first.request_id(), second.request_id());
    assert!(mailbox.try_recv().unwrap().is_none());
    let pending =
        crate::terminal_session::state::PendingMoveCommit::new(first.clone(), commit.clone());
    mailbox.close(CarrierFailure::Closed);
    assert!(state.retain_pending_move(pending));
    assert!(state.finalize_retirement(CarrierFailure::Closed, None));
    assert_eq!(state.cache().revision(), WorkbenchRevision::new(2));
    assert_eq!(state.active().unwrap().index(), 1);
    assert!(state.invariant_holds());
    // A's exact known fact stays recognizable; a forged newer fact cannot be swallowed as A.
    let mut wrong = commit;
    wrong.revision = WorkbenchRevision::new(3);
    assert!(
        !state.retain_pending_move(crate::terminal_session::state::PendingMoveCommit::new(
            first, wrong
        ))
    );
    drop(lease);
}
