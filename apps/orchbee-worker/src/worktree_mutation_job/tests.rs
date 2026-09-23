use super::*;
use orchbee_capability_node_protocol::{
    AbsoluteCwd, WorktreeMutation, WorktreeMutationEffect, WorktreeMutationError,
    WorktreeMutationFailure, WorktreeMutationStage,
};

fn id(byte: u8) -> OperationId {
    OperationId::from_bytes([byte; 16]).unwrap()
}
fn intent(path: &str) -> WorktreeMutationIntent {
    WorktreeMutationIntent::new(
        AbsoluteCwd::new("C:/repo").unwrap(),
        false,
        WorktreeMutation::InspectRemove {
            path: AbsoluteCwd::new(path).unwrap(),
        },
    )
    .unwrap()
}
fn done(_: WorktreeMutationIntent, _: &AtomicBool, _: Instant) -> Report {
    Ok(WorktreeMutationCompletion::Failed(
        WorktreeMutationFailure::new(
            WorktreeMutationStage::Probe,
            WorktreeMutationEffect::None,
            WorktreeMutationError::InvalidRepository,
            vec![],
        )
        .unwrap(),
    ))
}
fn cancelled(_: WorktreeMutationIntent, cancel: &AtomicBool, deadline: Instant) -> Report {
    while !cancel.load(Ordering::Relaxed) {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    Ok(WorktreeMutationCompletion::Failed(
        WorktreeMutationFailure::new(
            WorktreeMutationStage::Mutate,
            WorktreeMutationEffect::MayHaveChanged,
            WorktreeMutationError::Cancelled,
            vec![],
        )
        .unwrap(),
    ))
}
fn settled(slot: &WorktreeMutationSlot, id: OperationId) -> Poll {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let result = slot.poll(id);
        if result != Poll::Pending {
            return result;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn duplicate_conflict_busy_and_cancel_preserve_the_original_receipt() {
    let slot = WorktreeMutationSlot::new(cancelled);
    let original = intent("C:/one");
    assert_eq!(slot.start(id(1), original.clone()), Start::Accepted);
    assert_eq!(slot.start(id(1), original.clone()), Start::Existing);
    assert_eq!(slot.start(id(1), intent("C:/other")), Start::Conflict);
    assert_eq!(slot.start(id(2), original.clone()), Start::Busy);
    assert_eq!(slot.poll(id(1)), Poll::Pending);
    assert_eq!(slot.cancel(id(1)), Cancel::Registered);
    let outcome = settled(&slot, id(1));
    assert!(
        matches!(&outcome, Poll::Completed(WorktreeMutationCompletion::Failed(failure))
        if failure.effect == WorktreeMutationEffect::MayHaveChanged && failure.error == WorktreeMutationError::Cancelled)
    );
    assert_eq!(slot.start(id(1), original), Start::Existing);
    assert_eq!(slot.poll(id(1)), outcome);
    assert_eq!(slot.cancel(id(1)), Cancel::AlreadyTerminal);
    slot.stop_before(Instant::now() + Duration::from_secs(1))
        .unwrap();
}

#[test]
fn capacity_does_not_expire_old_mutation_identity_or_block_observation() {
    let slot = WorktreeMutationSlot::new(done);
    for byte in 1..=64 {
        assert_eq!(slot.start(id(byte), intent("C:/one")), Start::Accepted);
        assert!(matches!(settled(&slot, id(byte)), Poll::Completed(_)));
    }
    assert_eq!(slot.start(id(65), intent("C:/one")), Start::Full);
    assert_eq!(slot.start(id(1), intent("C:/one")), Start::Existing);
    assert!(matches!(slot.poll(id(1)), Poll::Completed(_)));
    assert_eq!(slot.poll(id(65)), Poll::Unknown);
    assert_eq!(slot.cancel(id(65)), Cancel::Unknown);
}

#[test]
fn physical_unknown_retires_admission_and_never_becomes_success() {
    let slot = WorktreeMutationSlot::new(|_, _, _| Err(()));
    assert_eq!(slot.start(id(1), intent("C:/one")), Start::Accepted);
    assert_eq!(settled(&slot, id(1)), Poll::PhysicalUnknown);
    assert_eq!(slot.start(id(2), intent("C:/one")), Start::PhysicalUnknown);
    assert_eq!(slot.start(id(1), intent("C:/one")), Start::Existing);
    assert_eq!(slot.cancel(id(1)), Cancel::PhysicalUnknown);
    assert!(
        slot.stop_before(Instant::now() + Duration::from_secs(1))
            .is_err()
    );
    assert_eq!(slot.poll(id(1)), Poll::PhysicalUnknown);
}

#[test]
fn stop_cancels_and_joins_the_owner_then_closes_admission() {
    let slot = WorktreeMutationSlot::new(cancelled);
    assert_eq!(slot.start(id(1), intent("C:/one")), Start::Accepted);
    slot.stop_before(Instant::now() + Duration::from_secs(3))
        .unwrap();
    assert!(matches!(slot.poll(id(1)), Poll::Completed(_)));
    assert_eq!(slot.start(id(2), intent("C:/one")), Start::PhysicalUnknown);
}

#[test]
fn stale_worker_and_generation_reject_all_actions_before_owner_access() {
    use orchbee_capability_node_protocol::{
        WorkerGeneration, WorkerId, WorktreeMutationJobAction as Action,
        WorktreeMutationOutcome as Outcome, WorktreeMutationRequest,
    };
    use std::num::NonZeroU64;
    let slot = WorktreeMutationSlot::new(done);
    for (worker, generation) in [(9, 3), (2, 4)] {
        for (action, expected) in [
            (
                Action::Start(intent("C:/one")),
                Outcome::Start(Start::RejectedFencing),
            ),
            (
                Action::Poll { job_id: id(1) },
                Outcome::Poll(Poll::RejectedFencing),
            ),
            (
                Action::Cancel { job_id: id(1) },
                Outcome::Cancel(Cancel::RejectedFencing),
            ),
        ] {
            let request = WorktreeMutationRequest {
                operation_id: id(1),
                worker_id: WorkerId::from_bytes([worker; 16]).unwrap(),
                worker_generation: WorkerGeneration::new(NonZeroU64::new(generation).unwrap()),
                action,
            };
            let result = crate::worktree_mutation_operation::execute(
                request,
                orchbee_local_site_protocol::WorkerId::from_bytes([2; 16]),
                NonZeroU64::new(3).unwrap(),
                &slot,
            );
            assert_eq!(result.outcome, expected);
            assert_eq!(result.operation_id, id(1));
            assert_eq!(result.job_id, id(1));
        }
    }
    assert_eq!(slot.poll(id(1)), Poll::Unknown);
    assert_eq!(slot.start(id(1), intent("C:/one")), Start::Accepted);
    assert!(matches!(settled(&slot, id(1)), Poll::Completed(_)));
}
