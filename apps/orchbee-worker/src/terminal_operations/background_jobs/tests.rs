use super::*;
use crate::{plugin_jobs::PluginJobs, worktree_mutation_job::WorktreeMutationSlot};
use orchbee_capability_node_protocol::{
    AbsoluteCwd, OperationId, PluginCommandFailure, PluginCommandIntent, PluginCommandLog,
    PluginJobAction, PluginJobOutcome, WorktreeMutation, WorktreeMutationCompletion,
    WorktreeMutationEffect, WorktreeMutationError, WorktreeMutationFailure, WorktreeMutationIntent,
    WorktreeMutationPollOutcome, WorktreeMutationStage, WorktreeMutationStartOutcome,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

// Each fault owns its own flags because test threads run concurrently.
static TIMEOUT_CANCEL_SEEN: AtomicBool = AtomicBool::new(false);
static TIMEOUT_RELEASE: AtomicBool = AtomicBool::new(false);
static DROP_CANCEL_SEEN: AtomicBool = AtomicBool::new(false);
static DROP_RELEASE: AtomicBool = AtomicBool::new(false);
static DROP_EXITED: AtomicBool = AtomicBool::new(false);

fn id(byte: u8) -> OperationId {
    OperationId::from_bytes([byte; 16]).unwrap()
}

fn plugin_intent() -> PluginCommandIntent {
    PluginCommandIntent {
        plugin_id: "test".into(),
        command_id: "action".into(),
        directory: std::env::temp_dir().to_str().unwrap().into(),
        argv: vec!["unused.exe".into()],
        environment: vec![],
    }
}

fn mutation_intent() -> WorktreeMutationIntent {
    WorktreeMutationIntent::new(
        AbsoluteCwd::new("C:/repo").unwrap(),
        false,
        WorktreeMutation::InspectRemove {
            path: AbsoluteCwd::new("C:/repo/feature").unwrap(),
        },
    )
    .unwrap()
}

fn cancelled_log(mut log: PluginCommandLog) -> PluginCommandLog {
    log.finished_ms = Some(log.started_ms);
    log.failure = Some(PluginCommandFailure::Cancelled);
    log
}

/// Fault: a plugin owner whose settlement outlives the deadline after observing cancellation.
fn stuck_until_released(
    _: PluginCommandIntent,
    log: PluginCommandLog,
    cancel: &AtomicBool,
) -> PluginCommandLog {
    while !TIMEOUT_RELEASE.load(Ordering::SeqCst) {
        if cancel.load(Ordering::Relaxed) {
            TIMEOUT_CANCEL_SEEN.store(true, Ordering::SeqCst);
        }
        thread::sleep(Duration::from_millis(1));
    }
    cancelled_log(log)
}

/// Fault: the same stuck settlement, observed only through Drop.
fn stuck_for_drop(
    _: PluginCommandIntent,
    log: PluginCommandLog,
    cancel: &AtomicBool,
) -> PluginCommandLog {
    while !DROP_RELEASE.load(Ordering::SeqCst) {
        if cancel.load(Ordering::Relaxed) {
            DROP_CANCEL_SEEN.store(true, Ordering::SeqCst);
        }
        thread::sleep(Duration::from_millis(1));
    }
    DROP_EXITED.store(true, Ordering::SeqCst);
    cancelled_log(log)
}

/// A cooperative mutation owner that ends as soon as cancellation is observed.
fn cooperative(
    _: WorktreeMutationIntent,
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<WorktreeMutationCompletion, ()> {
    while !cancel.load(Ordering::Relaxed) && Instant::now() < deadline {
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

#[test]
fn stuck_owner_times_out_at_the_shared_deadline_without_withholding_other_stops() {
    let discovery = WorktreeDiscoverySlot::new();
    let plugins = PluginJobs::with_execute(stuck_until_released);
    let mutations = WorktreeMutationSlot::new(cooperative);
    assert!(matches!(
        plugins.execute(id(1), PluginJobAction::Start(plugin_intent())),
        PluginJobOutcome::Started { .. }
    ));
    assert_eq!(
        mutations.start(id(2), mutation_intent()),
        WorktreeMutationStartOutcome::Accepted
    );
    let jobs = BackgroundJobs {
        discovery: &discovery,
        mutations: &mutations,
        plugins: &plugins,
    };
    let started = Instant::now();
    let error = jobs
        .stop_before(started + Duration::from_millis(300))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(2), "bounded wait");
    // The stuck owner was asked to stop but has not ended.
    assert!(TIMEOUT_CANCEL_SEEN.load(Ordering::SeqCst));
    assert!(matches!(
        plugins.execute(id(3), PluginJobAction::Get { job_id: id(1) }),
        PluginJobOutcome::Logs { logs, .. } if logs[0].finished_ms.is_none()
    ));
    // The owner after it still received the request and was observed as ended.
    assert!(matches!(
        mutations.poll(id(2)),
        WorktreeMutationPollOutcome::Completed(_)
    ));
    TIMEOUT_RELEASE.store(true, Ordering::SeqCst);
    jobs.stop_before(Instant::now() + Duration::from_secs(3))
        .unwrap();
}

#[test]
fn drop_requests_stop_without_waiting_for_an_unfinished_owner() {
    let plugins = PluginJobs::with_execute(stuck_for_drop);
    assert!(matches!(
        plugins.execute(id(1), PluginJobAction::Start(plugin_intent())),
        PluginJobOutcome::Started { .. }
    ));
    let started = Instant::now();
    drop(plugins);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "Drop never waits"
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while !DROP_CANCEL_SEEN.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline, "Drop requested stop");
        thread::sleep(Duration::from_millis(1));
    }
    assert!(!DROP_EXITED.load(Ordering::SeqCst));
    // Release the detached owner so it never outlives this test process unobserved.
    DROP_RELEASE.store(true, Ordering::SeqCst);
    while !DROP_EXITED.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
}
