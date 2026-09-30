use std::{
    error::Error,
    ffi::OsString,
    path::Path,
    sync::{Arc, Mutex, atomic::AtomicBool},
    thread,
    time::{Duration, Instant},
};

use super::{
    CancelOutcome, Faults, JobId, JobOutcome, JobWitness, OwnerReport, Physical, PollOutcome,
    Receipt, Stage, StartOutcome, TestHooks, WorktreeDiscoverySlot,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const DIR: &str = "/tmp";
const SHORT_TTL: Duration = Duration::from_millis(400);

fn id(n: u8) -> JobId {
    JobId([n; 16])
}

fn sh(script: &str) -> super::CommandSpec {
    super::CommandSpec {
        program: OsString::from("/bin/sh"),
        args: vec![OsString::from("-c"), OsString::from(script)],
    }
}

fn commands(probe: &str, list: &str) -> super::JobCommands {
    super::JobCommands {
        source: None,
        probe: sh(probe),
        list: Some(sh(list)),
        status: false,
        test_hooks: None,
    }
}

fn hooked(
    probe: &str,
    list: Some(&str),
    faults: Faults,
) -> (Duration, super::JobCommands, Arc<Mutex<JobWitness>>) {
    let witness = Arc::new(Mutex::new(JobWitness::default()));
    let mut cmds = commands(probe, list);
    cmds.test_hooks = Some(TestHooks {
        faults,
        witness: Arc::clone(&witness),
    });
    (Duration::from_secs(30), cmds, witness)
}

fn fast(probe: &str, list: &str) -> (Duration, super::JobCommands) {
    (Duration::from_secs(30), commands(probe, list))
}

fn settle_poll(slot: &WorktreeDiscoverySlot, job_id: JobId) -> PollOutcome {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match slot.poll(job_id) {
            PollOutcome::Pending if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            outcome => return outcome,
        }
    }
}

fn fd_held(fd: i32, target: &str) -> bool {
    std::fs::read_link(format!("/proc/self/fd/{fd}"))
        .map(|link| link.display().to_string() == target)
        .unwrap_or(false)
}

fn pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

fn descendant_pid_file(tag: &str) -> String {
    format!(
        "/tmp/orchbee-discovery-test-{}-{tag}.pid",
        std::process::id()
    )
}

// A fixture descendant is orphaned to init when its shell exits, so the
// test cannot wait() it and never claims that reap itself. What the
// test CAN honestly establish is bounded exit observation: the fixture
// hands back its exact descendant pid through a test-owned file, and
// /proc/<pid> disappearing is the exit+init-reap witness. A pid still
// present after the self-expiry bound fails the test rather than being
// explained away.
fn await_descendant_exit(pid_file: &str, bound: Duration) -> TestResult {
    let raw = std::fs::read_to_string(pid_file)?;
    let pid: u32 = raw.trim().parse()?;
    let deadline = Instant::now() + bound;
    while pid_alive(pid) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
    let _ = std::fs::remove_file(pid_file);
    assert!(
        !pid_alive(pid),
        "fixture descendant pid {pid} must exit within its self-expiry bound"
    );
    Ok(())
}

#[test]
fn start_completes_and_poll_collects_raw_bytes() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("printf probe-bytes", "printf 'list\\0bytes'");
    assert_eq!(
        slot.start_with(id(1), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    match settle_poll(&slot, id(1)) {
        PollOutcome::Completed(JobOutcome::Completed {
            common_dir,
            listing,
            stderr,
        }) => {
            assert_eq!(common_dir, b"probe-bytes");
            assert_eq!(listing, b"list\0bytes");
            assert!(stderr.is_empty());
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    Ok(())
}

#[test]
fn nonzero_exit_is_typed_git_failed_with_status() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("printf fixme >&2; exit 3", "printf never");
    assert_eq!(
        slot.start_with(id(2), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    match settle_poll(&slot, id(2)) {
        PollOutcome::Completed(JobOutcome::GitFailed {
            stage,
            status,
            stderr,
        }) => {
            assert_eq!(stage, Stage::Probe);
            assert_eq!(status, Some(3));
            assert_eq!(stderr, b"fixme");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    Ok(())
}

#[test]
fn second_start_is_typed_busy_not_fatal() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("sleep 30", "sleep 30");
    assert_eq!(
        slot.start_with(id(3), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    let (ttl2, cmds2) = fast("true", "true");
    assert_eq!(
        slot.start_with(id(4), Path::new(DIR), ttl2, cmds2),
        StartOutcome::Busy
    );
    assert_eq!(
        slot.cancel(id(3)),
        CancelOutcome::Registered,
        "cancel must register without blocking"
    );
    assert!(matches!(
        settle_poll(&slot, id(3)),
        PollOutcome::Completed(JobOutcome::Cancelled)
    ));
    Ok(())
}

#[test]
fn wrong_and_unknown_ids_are_literally_unknown() {
    let slot = WorktreeDiscoverySlot::new();
    assert_eq!(slot.poll(id(90)), PollOutcome::Unknown);
    assert_eq!(slot.cancel(id(90)), CancelOutcome::Unknown);
}

#[test]
fn invalid_directory_is_rejected_before_effect() {
    let slot = WorktreeDiscoverySlot::new();
    assert_eq!(
        slot.start_with(
            id(5),
            Path::new("relative/dir"),
            Duration::from_secs(1),
            commands("true", "true"),
        ),
        StartOutcome::InvalidDirectory
    );
    assert_eq!(
        slot.start_with(
            id(5),
            Path::new(""),
            Duration::from_secs(1),
            commands("true", "true"),
        ),
        StartOutcome::InvalidDirectory
    );
}

#[test]
fn stdout_overflow_across_pipes_is_typed_too_large() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast(
        "printf p",
        "head -c 200000 /dev/zero; head -c 200000 /dev/zero >&2",
    );
    assert_eq!(
        slot.start_with(id(6), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    assert!(matches!(
        settle_poll(&slot, id(6)),
        PollOutcome::Completed(JobOutcome::TooLarge { stage: Stage::List })
    ));
    Ok(())
}

// The 64 KiB stdout budget is shared across both stages: a probe that fits
// alone still leaves its bytes on the meter, so a modest list output that
// pushes the combined total over the cap is attributed to the list stage.
#[test]
fn shared_stdout_budget_attributes_overflow_to_the_crossing_stage() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("head -c 60000 /dev/zero", "head -c 10000 /dev/zero");
    assert_eq!(
        slot.start_with(id(20), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    assert!(matches!(
        settle_poll(&slot, id(20)),
        PollOutcome::Completed(JobOutcome::TooLarge { stage: Stage::List })
    ));
    Ok(())
}

#[test]
fn probe_overflow_alone_is_attributed_to_probe() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("head -c 200000 /dev/zero", "printf l");
    assert_eq!(
        slot.start_with(id(21), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    assert!(matches!(
        settle_poll(&slot, id(21)),
        PollOutcome::Completed(JobOutcome::TooLarge {
            stage: Stage::Probe
        })
    ));
    Ok(())
}

#[test]
fn slow_child_cancel_kills_and_reaps_without_eof_wait() -> TestResult {
    // The direct child is `sleep` (via exec) and holds both pipe write
    // ends; cancel must still kill+reap it without waiting for EOF.
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("printf ok", "exec sleep 60");
    assert_eq!(
        slot.start_with(id(7), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    thread::sleep(Duration::from_millis(100));
    let began = Instant::now();
    assert_eq!(slot.cancel(id(7)), CancelOutcome::Registered);
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "cancel arm must not block on kill+wait"
    );
    assert!(matches!(
        settle_poll(&slot, id(7)),
        PollOutcome::Completed(JobOutcome::Cancelled)
    ));
    Ok(())
}

#[test]
fn owner_deadline_is_shared_across_both_commands() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let cmds = commands("sleep 0.3; printf probe", "exec sleep 60");
    let began = Instant::now();
    assert_eq!(
        slot.start_with(id(8), Path::new(DIR), SHORT_TTL, cmds),
        StartOutcome::Accepted
    );
    let outcome = settle_poll(&slot, id(8));
    assert!(
        began.elapsed() < Duration::from_secs(3),
        "shared deadline must bound the whole job"
    );
    assert!(matches!(
        outcome,
        PollOutcome::Completed(JobOutcome::Timeout)
    ));
    Ok(())
}

#[test]
fn spawn_unavailable_is_distinct_from_other_failures() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let cmds = super::JobCommands {
        source: None,
        probe: super::CommandSpec {
            program: OsString::from("/nonexistent-orchbee-fixture"),
            args: Vec::new(),
        },
        list: Some(sh("true")),
        status: false,
        test_hooks: None,
    };
    assert_eq!(
        slot.start_with(id(9), Path::new(DIR), Duration::from_secs(5), cmds),
        StartOutcome::Accepted
    );
    assert!(matches!(
        settle_poll(&slot, id(9)),
        PollOutcome::Completed(JobOutcome::SpawnUnavailable {
            stage: Stage::Probe
        })
    ));
    Ok(())
}

#[test]
fn uncollected_receipt_frees_capacity_and_new_start_evicts() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("printf p1", "printf l1");
    assert_eq!(
        slot.start_with(id(10), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    // Let the job finish without ever polling (abandoned receipt).
    thread::sleep(Duration::from_millis(300));
    // Physical slot frees on the NEXT touch: a new Start settles the
    // finished owner, proves cleanup, and evicts the old receipt.
    let (ttl2, cmds2) = fast("printf p2", "printf l2");
    assert_eq!(
        slot.start_with(id(11), Path::new(DIR), ttl2, cmds2),
        StartOutcome::Accepted
    );
    assert_eq!(
        slot.poll(id(10)),
        PollOutcome::Unknown,
        "evicted receipt is literally unknown, never another caller's job"
    );
    assert!(matches!(
        settle_poll(&slot, id(11)),
        PollOutcome::Completed(JobOutcome::Completed { .. })
    ));
    Ok(())
}

#[test]
fn expired_receipt_is_evicted_at_access() {
    let slot = WorktreeDiscoverySlot::new();
    {
        let mut inner = slot.inner.lock().unwrap();
        inner.receipt = Some(Receipt {
            job_id: id(12),
            outcome: JobOutcome::Timeout,
            expires: Instant::now() - Duration::from_secs(1),
        });
    }
    assert_eq!(slot.poll(id(12)), PollOutcome::Unknown);
    assert!(
        slot.inner.lock().unwrap().receipt.is_none(),
        "an expired receipt must be cleared by the access that sees it"
    );
}

#[test]
fn expired_receipt_is_purged_by_any_access_not_just_matching() {
    let slot = WorktreeDiscoverySlot::new();
    {
        let mut inner = slot.inner.lock().unwrap();
        inner.receipt = Some(Receipt {
            job_id: id(21),
            outcome: JobOutcome::Cancelled,
            expires: Instant::now() - Duration::from_secs(1),
        });
    }
    assert_eq!(slot.cancel(id(99)), CancelOutcome::Unknown);
    assert!(
        slot.inner.lock().unwrap().receipt.is_none(),
        "a wrong-id access must still clear the expired cell"
    );
    {
        let mut inner = slot.inner.lock().unwrap();
        inner.receipt = Some(Receipt {
            job_id: id(22),
            outcome: JobOutcome::Cancelled,
            expires: Instant::now() - Duration::from_secs(1),
        });
    }
    assert_eq!(slot.poll(id(99)), PollOutcome::Unknown);
    assert!(slot.inner.lock().unwrap().receipt.is_none());
}

#[test]
fn panicking_owner_marks_slot_dead_never_cleanup_fact() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    {
        let mut inner = slot.inner.lock().unwrap();
        inner.physical = Some(Physical {
            job_id: id(13),
            cancel: Arc::new(AtomicBool::new(false)),
            join: thread::spawn(|| -> OwnerReport { panic!("fixture panic") }),
        });
    }
    thread::sleep(Duration::from_millis(50));
    assert_eq!(slot.poll(id(13)), PollOutcome::PhysicalUnknown);
    let (ttl, cmds) = fast("true", "true");
    assert_eq!(
        slot.start_with(id(14), Path::new(DIR), ttl, cmds),
        StartOutcome::Dead,
        "a dead slot never frees capacity on a panic claim"
    );
    assert_eq!(slot.cancel(id(13)), CancelOutcome::PhysicalUnknown);
    Ok(())
}

#[test]
fn malformed_cleanup_confirmation_is_physical_unknown() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    {
        let mut inner = slot.inner.lock().unwrap();
        inner.physical = Some(Physical {
            job_id: id(15),
            cancel: Arc::new(AtomicBool::new(false)),
            join: thread::spawn(|| OwnerReport {
                outcome: JobOutcome::Cancelled,
                child_reaped: false,
                pipes_closed: true,
            }),
        });
    }
    thread::sleep(Duration::from_millis(50));
    assert_eq!(slot.poll(id(15)), PollOutcome::PhysicalUnknown);
    let (ttl, cmds) = fast("true", "true");
    assert_eq!(
        slot.start_with(id(44), Path::new(DIR), ttl, cmds),
        StartOutcome::Dead,
        "a malformed confirmation leaves the slot dead, never busy-for-new-work"
    );
    Ok(())
}

#[test]
fn busy_physical_and_dead_are_distinct_typed_outcomes() -> TestResult {
    // A physically occupied slot answers Busy; a permanently dead slot
    // answers Dead. The two must never collapse into one variant.
    let busy_slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("sleep 30", "sleep 30");
    assert_eq!(
        busy_slot.start_with(id(45), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    let (ttl2, cmds2) = fast("true", "true");
    assert_eq!(
        busy_slot.start_with(id(46), Path::new(DIR), ttl2, cmds2),
        StartOutcome::Busy,
        "physical occupancy is Busy, not Dead"
    );
    assert_eq!(busy_slot.cancel(id(45)), CancelOutcome::Registered);
    assert!(matches!(
        settle_poll(&busy_slot, id(45)),
        PollOutcome::Completed(JobOutcome::Cancelled)
    ));

    let dead_slot = WorktreeDiscoverySlot::new();
    {
        let mut inner = dead_slot.inner.lock().unwrap();
        inner.physical = Some(Physical {
            job_id: id(47),
            cancel: Arc::new(AtomicBool::new(false)),
            join: thread::spawn(|| -> OwnerReport { panic!("fixture panic") }),
        });
    }
    thread::sleep(Duration::from_millis(50));
    let (ttl3, cmds3) = fast("true", "true");
    assert_eq!(
        dead_slot.start_with(id(48), Path::new(DIR), ttl3, cmds3),
        StartOutcome::Dead,
        "physical uncertainty is Dead, not Busy"
    );
    assert_eq!(dead_slot.poll(id(47)), PollOutcome::PhysicalUnknown);
    Ok(())
}

#[test]
fn cancel_on_terminal_receipt_reports_terminal_not_free() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("printf p", "printf l");
    assert_eq!(
        slot.start_with(id(16), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    assert!(matches!(
        settle_poll(&slot, id(16)),
        PollOutcome::Completed(JobOutcome::Completed { .. })
    ));
    assert!(matches!(
        slot.cancel(id(16)),
        CancelOutcome::AlreadyTerminal(JobOutcome::Completed { .. })
    ));
    Ok(())
}

#[test]
fn real_git_entrypoint_produces_typed_outcome() {
    // Exercises the fixed argv, JOB_TTL, and GIT_TERMINAL_PROMPT path.
    // /tmp is not a repository: a real git answers either
    // GitFailed(Probe) or SpawnUnavailable — both are typed outcomes.
    let slot = WorktreeDiscoverySlot::new();
    assert_eq!(slot.start(id(20), Path::new(DIR)), StartOutcome::Accepted);
    assert!(matches!(
        settle_poll(&slot, id(20)),
        PollOutcome::Completed(JobOutcome::GitFailed {
            stage: Stage::Probe,
            ..
        }) | PollOutcome::Completed(JobOutcome::SpawnUnavailable {
            stage: Stage::Probe
        })
    ));
}

#[test]
fn pipe_fds_are_released_after_completion() -> TestResult {
    // Owned-fd witness: the owner records the exact pipe fd numbers it
    // held, and each one must be absent from /proc/self/fd after the
    // job settles — immune to sibling-test fd churn.
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds, witness) = hooked("printf p", "printf l", Faults::default());
    assert_eq!(
        slot.start_with(id(17), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    assert!(matches!(
        settle_poll(&slot, id(17)),
        PollOutcome::Completed { .. }
    ));
    drop(slot);
    let held: Vec<(i32, String)> = witness.lock().unwrap().pipe_ends.clone();
    assert_eq!(held.len(), 4, "two children, two pipe ends each");
    assert!(
        held.iter().all(|(_, target)| !target.is_empty()),
        "the witness must capture each pipe's /proc/self/fd target"
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while held.iter().any(|(fd, t)| fd_held(*fd, t)) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        held.iter().all(|(fd, t)| !fd_held(*fd, t)),
        "every recorded owner pipe end must be closed: {held:?}"
    );
    Ok(())
}

#[test]
fn handle_operations_never_block_on_a_running_job() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("printf p", "head -c 500000 /dev/zero; exec sleep 60");
    assert_eq!(
        slot.start_with(id(18), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    let began = Instant::now();
    for step in 0..50 {
        match step % 3 {
            0 => {
                let _ = slot.poll(id(18));
            }
            1 => {
                let _ = slot.cancel(id(90));
            }
            _ => {
                let (t, c) = fast("true", "true");
                assert_eq!(
                    slot.start_with(id(19), Path::new(DIR), t, c),
                    StartOutcome::Busy
                );
            }
        }
    }
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "handle operations must stay prompt while the child floods"
    );
    slot.cancel(id(18));
    assert!(matches!(
        settle_poll(&slot, id(18)),
        PollOutcome::Completed(_)
    ));
    Ok(())
}

#[test]
fn injected_drain_failure_is_owner_io_and_the_real_child_is_reaped() -> TestResult {
    // A post-spawn pipe-read failure must not fabricate cleanup: the
    // owner kills+waits the still-live child, reports OwnerIo with a
    // truthful reap fact, and the slot frees only on that proof.
    let slot = WorktreeDiscoverySlot::new();
    let faults = Faults {
        drain: true,
        ..Faults::default()
    };
    let (ttl, cmds, witness) = hooked("printf x; exec sleep 30", "printf l", faults);
    assert_eq!(
        slot.start_with(id(30), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    assert!(matches!(
        settle_poll(&slot, id(30)),
        PollOutcome::Completed(JobOutcome::OwnerIo {
            stage: Some(Stage::Probe)
        })
    ));
    {
        let witness = witness.lock().unwrap();
        assert_eq!(witness.spawned.len(), 1);
        assert_eq!(
            witness.reaped, witness.spawned,
            "the injected io error must still kill+reap the live child"
        );
        assert!(
            !pid_alive(witness.spawned[0]),
            "reaped pid must be gone from /proc, not a lingering zombie"
        );
    }
    let (ttl2, cmds2) = fast("printf p", "printf l");
    assert_eq!(
        slot.start_with(id(31), Path::new(DIR), ttl2, cmds2),
        StartOutcome::Accepted,
        "a truthful OwnerIo frees the slot for the next job"
    );
    Ok(())
}

#[test]
fn injected_nonblocking_setup_failure_is_owner_io_and_reaped() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let faults = Faults {
        nonblock: true,
        ..Faults::default()
    };
    let (ttl, cmds, witness) = hooked("exec sleep 30", "printf l", faults);
    assert_eq!(
        slot.start_with(id(32), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    assert!(matches!(
        settle_poll(&slot, id(32)),
        PollOutcome::Completed(JobOutcome::OwnerIo {
            stage: Some(Stage::Probe)
        })
    ));
    let witness = witness.lock().unwrap();
    assert_eq!(witness.reaped, witness.spawned);
    assert!(witness.spawned.iter().all(|pid| !pid_alive(*pid)));
    Ok(())
}

#[test]
fn sustained_stderr_flood_cancellation_stays_prompt() -> TestResult {
    // A continuous stderr stream must not starve the owner's control
    // checks: each drain turn is quantum-bounded.
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast("printf p", "while :; do head -c 65536 /dev/zero >&2; done");
    assert_eq!(
        slot.start_with(id(33), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    thread::sleep(Duration::from_millis(200));
    let began = Instant::now();
    assert_eq!(slot.cancel(id(33)), CancelOutcome::Registered);
    let outcome = settle_poll(&slot, id(33));
    assert!(
        began.elapsed() < Duration::from_secs(3),
        "cancel must reach the owner even under a sustained stderr flood"
    );
    assert!(matches!(
        outcome,
        PollOutcome::Completed(JobOutcome::Cancelled)
    ));
    Ok(())
}

#[test]
fn sustained_stderr_flood_deadline_still_fires() -> TestResult {
    let slot = WorktreeDiscoverySlot::new();
    let cmds = commands("printf p", "while :; do head -c 65536 /dev/zero >&2; done");
    let began = Instant::now();
    assert_eq!(
        slot.start_with(id(34), Path::new(DIR), SHORT_TTL, cmds),
        StartOutcome::Accepted
    );
    let outcome = settle_poll(&slot, id(34));
    assert!(
        began.elapsed() < Duration::from_secs(3),
        "the shared deadline must bound a stderr flood"
    );
    assert!(matches!(
        outcome,
        PollOutcome::Completed(JobOutcome::Timeout)
    ));
    Ok(())
}

#[test]
fn descendant_held_pipe_ends_do_not_block_completion() -> TestResult {
    // The direct child prints its listing and exits while a real
    // descendant (backgrounded sleep, not exec) still holds both pipe
    // write ends. The owner must complete on the parent's buffered
    // output alone instead of waiting for descendant EOF. The helper
    // descendant is self-expiring and its exact pid is handed back via
    // a test-owned file; the test then observes its bounded exit. The
    // descendant outlives the completed job by design — an earlier
    // revision's "no fixture descendant is left running" claim was
    // wrong; the honest claim is bounded lifetime plus observed exit.
    let pid_file = descendant_pid_file("held-ends");
    let slot = WorktreeDiscoverySlot::new();
    let (ttl, cmds) = fast(
        "printf p",
        &format!("printf done; sleep 4 & echo $! > '{pid_file}'; exit 0"),
    );
    let began = Instant::now();
    assert_eq!(
        slot.start_with(id(35), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    let outcome = settle_poll(&slot, id(35));
    assert!(
        began.elapsed() < Duration::from_secs(4),
        "completion must not wait for the descendant-held EOF"
    );
    match outcome {
        PollOutcome::Completed(JobOutcome::Completed { listing, .. }) => {
            assert_eq!(listing, b"done");
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
    await_descendant_exit(&pid_file, Duration::from_secs(8))?;
    Ok(())
}

#[test]
fn tail_read_failure_is_owner_io_not_success() -> TestResult {
    // A read error in the post-exit drain must stay a typed failure;
    // a truncated buffer is never reported as a complete listing. The
    // fixture keeps a write end open through a real backgrounded
    // descendant so the tail drain is the path exercised; that
    // descendant is self-expiring and its bounded exit is observed via
    // its exact pid (see await_descendant_exit).
    let pid_file = descendant_pid_file("tail-fault");
    let slot = WorktreeDiscoverySlot::new();
    let faults = Faults {
        tail: true,
        ..Faults::default()
    };
    let (ttl, cmds, witness) = hooked(
        &format!("printf p; sleep 4 & echo $! > '{pid_file}'; exit 0"),
        "printf l",
        faults,
    );
    assert_eq!(
        slot.start_with(id(36), Path::new(DIR), ttl, cmds),
        StartOutcome::Accepted
    );
    assert!(
        matches!(
            settle_poll(&slot, id(36)),
            PollOutcome::Completed(JobOutcome::OwnerIo { stage: Some(_) })
        ),
        "a tail read error must surface as OwnerIo, never Completed"
    );
    let witness = witness.lock().unwrap();
    assert_eq!(witness.reaped, witness.spawned);
    drop(witness);
    await_descendant_exit(&pid_file, Duration::from_secs(8))?;
    Ok(())
}

#[test]
fn real_git_repository_lists_actual_worktrees() -> TestResult {
    // One actual Git fixture: a disposable private repository with a
    // linked detached worktree. Every git invocation carries explicit
    // per-command `-c` identity — no global user configuration is read
    // or written. The slot drives the real fixed argv (probe + list).
    let tag = std::process::id();
    let root = format!("/tmp/orchbee-worktree-repo-{tag}");
    let linked = format!("{root}-linked");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&linked);
    std::fs::create_dir_all(&root)?;
    let git = |args: &[&str]| -> TestResult<Vec<u8>> {
        let output = std::process::Command::new("git")
            .arg("-c")
            .arg("user.email=fixture@example.invalid")
            .arg("-c")
            .arg("user.name=fixture")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()?;
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(output.stdout)
    };
    git(&["init", "-b", "main"])?;
    std::fs::write(format!("{root}/seed.txt"), "seed")?;
    git(&["add", "seed.txt"])?;
    git(&["commit", "-m", "seed"])?;
    git(&["worktree", "add", "--detach", &linked])?;
    let head = git(&["rev-parse", "HEAD"])?;
    let head = String::from_utf8(head)?.trim().to_owned();
    assert_eq!(head.len(), 40);

    let slot = WorktreeDiscoverySlot::new();
    assert_eq!(slot.start(id(40), Path::new(&root)), StartOutcome::Accepted);
    match settle_poll(&slot, id(40)) {
        PollOutcome::Completed(JobOutcome::Completed {
            common_dir,
            listing,
            stderr,
        }) => {
            let common_dir = String::from_utf8(common_dir)?;
            assert_eq!(common_dir.trim(), format!("{root}/.git"));
            // NUL-terminated `attr value` fields; a NUL-only record is
            // the entry separator.
            let text = String::from_utf8_lossy(&listing);
            assert!(listing.contains(&0), "porcelain -z must be NUL-separated");
            assert!(
                text.contains(&format!("worktree {root}\0")),
                "main worktree record: {text:?}"
            );
            assert!(
                text.contains(&format!("worktree {linked}\0")),
                "linked worktree record: {text:?}"
            );
            assert!(
                text.contains(&format!("HEAD {head}\0")),
                "verbatim HEAD: {text:?}"
            );
            assert!(
                text.contains("branch refs/heads/main\0"),
                "verbatim full ref, never stripped: {text:?}"
            );
            assert!(
                text.contains("detached\0"),
                "linked worktree is detached: {text:?}"
            );
            assert!(stderr.is_empty());
        }
        other => panic!("real fixture must complete: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&linked);
    Ok(())
}
