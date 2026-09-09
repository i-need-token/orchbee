use super::*;
use crate::local_client::workbench::workspace_setup::WorkbenchSetupCommit;
use crate::terminal_session::state::workspace_setup::{
    NativeSetupEdit, NativeSetupToken, SetupWorkerEvidence,
};
use orchbee_workbench_protocol::WorkbenchProjectId;
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
                mailbox.try_enqueue(poll(&mailbox)).unwrap();
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
