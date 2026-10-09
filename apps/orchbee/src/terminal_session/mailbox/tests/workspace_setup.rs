use super::*;
use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
use crate::terminal_session::state::workspace_setup::{
    NativeSetupEdit, NativeSetupToken, SetupWorkerEvidence,
};
use orchbee_workbench_protocol::WorkbenchProjectId;

fn project_rename_fixture(
    deadline: Instant,
) -> (
    NativeSessionState,
    Box<crate::terminal_session::ProjectRenameRequest>,
) {
    let project = orchbee_workbench_protocol::Project {
        project_id: WorkbenchProjectId::parse(&id(70)).unwrap(),
        label: "Project".into(),
        workspaces: Vec::new(),
        directories: Vec::new(),
    };
    let context = WorkbenchTerminalContextId::parse(&id(3)).unwrap();
    let snapshot = Snapshot {
        protocol_version: ProtocolVersion,
        site_id: SiteId::parse(&id(2)).unwrap(),
        revision: WorkbenchRevision::new(1),
        navigation: Navigation {
            standalone_terminal_context_id: context.clone(),
            workspace_order: Vec::new(),
            projects: vec![project.clone()],
        },
        terminal_contexts: vec![TerminalContext {
            terminal_context_id: context,
            groups: Vec::new(),
            tabs: Vec::new(),
        }],
    };
    let state = NativeSessionState::bootstrap(
        project_native_standalone_tabs(&snapshot).unwrap(),
        1,
        geometry(80),
    )
    .unwrap()
    .0;
    let request =
        crate::terminal_session::ProjectRenameRequest::capture(&state, &project, deadline).unwrap();
    (state, Box::new(request))
}

#[test]
fn targeted_project_rename_lock_contention_expires_without_draft_or_cursor_allocation() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(30);
    let (clock, reached, resume) = ObservedClock::with_contention_checkpoint(base);
    let clock = Arc::new(clock);
    let mailbox = Arc::new(bootstrapped_mailbox(1, 5));
    mailbox.lock().desired = MailboxMembership::Live {
        desired: None,
        panes: Vec::new(),
    };
    let before = mailbox.cursors();
    let held = mailbox.lock();
    let worker_mailbox = Arc::clone(&mailbox);
    let worker_clock = Arc::clone(&clock);
    let worker = std::thread::spawn(move || {
        let (mut state, request) = project_rename_fixture(deadline);
        assert!(
            worker_mailbox
                .open_project_rename_draft_for_using(&mut state, request, worker_clock.as_ref())
                .is_none()
        );
        assert!(!state.setup_blocks_structure());
        assert!(state.setup_draft().is_none());
    });
    reached.recv_timeout(Duration::from_secs(1)).unwrap();
    clock.set(deadline);
    resume.send(()).unwrap();
    worker.join().unwrap();
    drop(held);
    assert_eq!(mailbox.cursors(), before);
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn targeted_project_rename_rechecks_deadline_after_acquiring_lock() {
    struct AdmissionClock {
        base: Instant,
        expire_at_call: usize,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl MailboxClock for AdmissionClock {
        fn now(&self) -> Instant {
            if self.calls.fetch_add(1, Ordering::Relaxed) < self.expire_at_call {
                self.base
            } else {
                self.base + Duration::from_secs(30)
            }
        }
    }
    let base = Instant::now();
    for expire_at_call in [1, 2] {
        let mailbox = bootstrapped_mailbox(1, 5);
        mailbox.lock().desired = MailboxMembership::Live {
            desired: None,
            panes: Vec::new(),
        };
        let (mut state, request) = project_rename_fixture(base + Duration::from_secs(30));
        let cursors = mailbox.cursors();
        let clock = AdmissionClock {
            base,
            expire_at_call,
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        assert!(
            mailbox
                .open_project_rename_draft_for_using(&mut state, request, &clock)
                .is_none()
        );
        assert!(!state.setup_blocks_structure());
        assert!(state.setup_draft().is_none());
        assert_eq!(mailbox.cursors(), cursors);
        assert!(mailbox.try_recv().unwrap().is_none());
    }
}

fn draft(state: &mut NativeSessionState) -> NativeSetupToken {
    let token = state
        .open_setup_draft(
            crate::terminal_session::state::workspace_setup::NativeSetupIntent::CreateWorkspace,
        )
        .unwrap();
    state.acknowledge_setup_draw(Some(token));
    assert!(state.advance_setup_picker(token));
    let token = state.setup_draft().unwrap().token();
    state.edit_setup_draft(token, NativeSetupEdit::Append("Project"));
    state.acknowledge_setup_draw(Some(token));
    token
}
#[test]
fn setup_admission_expiry_and_full_slot_preserve_draft_and_every_cursor() {
    let base = Instant::now();
    let deadlines = OperationDeadlines::from_sample(base).unwrap();
    for expired in [false, true] {
        let mailbox = bootstrapped_mailbox(1, 5);
        let mut state = ready_state(1);
        let token = draft(&mut state);
        if !expired {
            for _ in 0..COMMAND_CAPACITY {
                mailbox.try_enqueue(input(&mailbox, 0)).unwrap();
            }
        }
        let cursors = mailbox.cursors();
        let next = mailbox.lock().next_command_serial;
        assert!(
            mailbox
                .try_enqueue_setup_for_using(
                    &mut state,
                    token,
                    deadlines,
                    &FakeClock::new(if expired {
                        deadlines.forward_cutoff()
                    } else {
                        base
                    })
                )
                .is_err()
        );
        assert_eq!(mailbox.cursors(), cursors);
        assert_eq!(mailbox.lock().next_command_serial, next);
        assert_eq!(state.setup_draft().unwrap().text(), "Project");
        assert!(state.pending_refresh_generation().is_none());
    }
}
#[test]
fn setup_mutex_wait_is_inside_original_admission_budget_without_relocking_on_expiry() {
    let base = Instant::now();
    let deadlines = OperationDeadlines::from_sample(base).unwrap();
    let (clock, reached, resume) = ObservedClock::with_contention_checkpoint(base);
    let clock = Arc::new(clock);
    let mailbox = Arc::new(bootstrapped_mailbox(1, 5));
    let before = mailbox.cursors();
    let held = mailbox.lock();
    let worker_mailbox = Arc::clone(&mailbox);
    let worker_clock = Arc::clone(&clock);
    let worker = std::thread::spawn(move || {
        let mut state = ready_state(1);
        let token = draft(&mut state);
        let result = worker_mailbox.try_enqueue_setup_for_using(
            &mut state,
            token,
            deadlines,
            worker_clock.as_ref(),
        );
        assert!(matches!(result, Err(MoveAdmissionError::Deadline)));
        assert_eq!(state.setup_draft().unwrap().token(), token);
    });
    reached.recv_timeout(Duration::from_secs(1)).unwrap();
    clock.set(deadlines.forward_cutoff());
    resume.send(()).unwrap();
    worker.join().unwrap();
    drop(held);
    assert_eq!(mailbox.cursors(), before);
}
#[test]
fn setup_known_and_definite_evidence_survive_panic_close_before_result_delivery() {
    for committed in [false, true] {
        let base = Instant::now();
        let deadlines = OperationDeadlines::from_sample(base).unwrap();
        let mailbox = bootstrapped_mailbox(1, 5);
        let mut state = ready_state(1);
        let token = draft(&mut state);
        mailbox
            .try_enqueue_setup_for_using(&mut state, token, deadlines, &FakeClock::new(base))
            .unwrap();
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::WorkspaceSetup(marker) = lease.record() else {
            panic!("one setup command");
        };
        let operation = marker.0.clone();
        assert_eq!(
            mailbox.mark_setup_in_flight(&operation).unwrap(),
            MutationResult::Applied
        );
        let evidence = if committed {
            let known = operation
                .known_commit(WorkbenchSetupCommit::ProjectCreated {
                    revision: WorkbenchRevision::new(2),
                    project_id: WorkbenchProjectId::parse(&id(80)).unwrap(),
                })
                .unwrap();
            assert!(mailbox.retain_setup_known(&known));
            SetupWorkerEvidence::Known(known)
        } else {
            assert!(mailbox.retain_setup_not_applied(&operation));
            SetupWorkerEvidence::NotApplied(operation)
        };
        mailbox.close(CarrierFailure::Transport);
        assert_eq!(mailbox.setup_worker_loss(), Some(evidence.clone()));
        assert!(state.retain_setup_worker(evidence));
        assert_eq!(state.setup_recovery(), committed);
    }
}
#[test]
fn queued_setup_is_definitely_not_applied_when_earlier_carrier_work_retires() {
    let base = Instant::now();
    let deadlines = OperationDeadlines::from_sample(base).unwrap();
    let mailbox = bootstrapped_mailbox(1, 5);
    let mut state = ready_state(1);
    let token = draft(&mut state);
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    mailbox
        .try_enqueue_setup_for_using(&mut state, token, deadlines, &FakeClock::new(base))
        .unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(lease.record(), QueuedCommand::Poll(_)));
    mailbox.close(CarrierFailure::Transport);
    let Some(SetupWorkerEvidence::NotApplied(operation)) = mailbox.setup_worker_loss() else {
        panic!("queued command never submitted");
    };
    assert!(state.retain_setup_worker(SetupWorkerEvidence::NotApplied(operation)));
    state.retire(CarrierFailure::Transport);
    assert!(!state.setup_blocks_structure());
    assert_eq!(
        state.setup_status(),
        Some(crate::terminal_session::state::workspace_setup::NativeSetupStatus::NotApplied)
    );
}
