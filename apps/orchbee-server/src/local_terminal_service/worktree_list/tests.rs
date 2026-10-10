// `git worktree list --porcelain -z` fixtures: NUL-terminated attribute
// fields, one bare NUL record separator after every record, the stream
// ending after the last separator.
use orchbee_workbench_protocol::{WorktreeEntry, WorktreeEntryText};

use super::{entry_text, parse_worktree_listing};

#[test]
fn discovery_admission_bounds_waiters_and_reserves_the_next_turn_for_foreground() {
    use super::DiscoveryGate;
    use std::sync::{Arc, mpsc};
    use std::time::{Duration, Instant};

    let gate = Arc::new(DiscoveryGate::default());
    let active = gate.background().expect("initial status owner");
    let next_gate = Arc::clone(&gate);
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let next = std::thread::spawn(move || {
        let _permit = next_gate
            .foreground(Instant::now() + Duration::from_secs(5))
            .unwrap();
        entered.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let limit = Instant::now() + Duration::from_secs(2);
    while !gate.admission.lock().unwrap().foreground_waiting {
        assert!(Instant::now() < limit, "foreground did not register");
        std::thread::yield_now();
    }
    assert!(matches!(
        gate.foreground(limit),
        Err(
            orchbee_workbench_protocol::WorkbenchWorktreeListOutcome::Unavailable {
                reason: orchbee_workbench_protocol::WorktreeListUnavailableReason::ServerBusy
            }
        )
    ));
    // Even before the waiting thread is scheduled, releasing the active owner
    // must not let any of six independently refreshing sidebar rows barge in.
    drop(active);
    for _ in 0..6 {
        assert!(gate.background().is_none());
    }
    entry.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(gate.background().is_none());
    release.send(()).unwrap();
    next.join().unwrap();
    assert!(gate.background().is_some());
}

#[test]
fn discovery_wait_deadline_releases_only_the_waiting_slot() {
    use super::DiscoveryGate;
    use std::time::{Duration, Instant};

    let gate = DiscoveryGate::default();
    let active = gate.background().unwrap();
    assert!(matches!(
        gate.foreground(Instant::now() + Duration::from_millis(20)),
        Err(orchbee_workbench_protocol::WorkbenchWorktreeListOutcome::Timeout {})
    ));
    assert!(!gate.admission.lock().unwrap().foreground_waiting);
    assert!(
        gate.background().is_none(),
        "timed-out waiter must not release the active reader"
    );
    drop(active);
    let next = gate
        .foreground(Instant::now() + Duration::from_secs(1))
        .unwrap();
    assert!(gate.background().is_none());
    drop(next);
    assert!(gate.background().is_some());
}

const HEAD40: &str = "0123456789abcdef0123456789abcdef01234567";
const HEAD64: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn listing(records: &[&[&[u8]]]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for record in records {
        for field in *record {
            bytes.extend_from_slice(field);
            bytes.push(0);
        }
        bytes.push(0);
    }
    bytes
}

fn utf8(text: &str) -> WorktreeEntryText {
    WorktreeEntryText::Utf8(text.to_owned())
}

#[test]
fn single_branch_record_keeps_the_verbatim_ref() {
    let bytes = listing(&[&[
        b"worktree /repo/main",
        format!("HEAD {HEAD40}").as_bytes(),
        b"branch refs/heads/main",
    ]]);
    assert_eq!(
        parse_worktree_listing(&bytes),
        Ok(vec![WorktreeEntry {
            path: utf8("/repo/main"),
            head: Some(HEAD40.to_owned()),
            branch: Some(utf8("refs/heads/main")),
            bare: false,
            detached: false,
            locked: false,
            prunable: false,
        }])
    );
}

#[test]
fn detached_record_has_no_branch() {
    let bytes = listing(&[&[
        b"worktree /repo/detached",
        format!("HEAD {HEAD40}").as_bytes(),
        b"detached",
    ]]);
    let entries = parse_worktree_listing(&bytes).expect("detached listing");
    assert_eq!(entries.len(), 1);
    assert!(entries[0].detached);
    assert_eq!(entries[0].branch, None);
    assert_eq!(entries[0].head.as_deref(), Some(HEAD40));
}

#[test]
fn bare_record_has_no_head_and_no_branch() {
    let bytes = listing(&[&[b"worktree /repo/bare.git", b"bare"]]);
    let entries = parse_worktree_listing(&bytes).expect("bare listing");
    assert_eq!(entries.len(), 1);
    assert!(entries[0].bare);
    assert!(!entries[0].detached);
    assert_eq!(entries[0].head, None);
    assert_eq!(entries[0].branch, None);
}

#[test]
fn unborn_all_zero_head_is_preserved_verbatim() {
    for head in ["0".repeat(40), "0".repeat(64)] {
        let bytes = listing(&[&[
            b"worktree /repo/unborn",
            format!("HEAD {head}").as_bytes(),
            b"branch refs/heads/main",
        ]]);
        let entries = parse_worktree_listing(&bytes).expect("unborn listing");
        assert_eq!(entries[0].head.as_deref(), Some(head.as_str()));
        assert_eq!(entries[0].branch, Some(utf8("refs/heads/main")));
    }
}

#[test]
fn head_accepts_both_hash_widths() {
    for head in [HEAD40, HEAD64] {
        let bytes = listing(&[&[
            b"worktree /repo/w",
            format!("HEAD {head}").as_bytes(),
            b"detached",
        ]]);
        let entries = parse_worktree_listing(&bytes).expect("valid head");
        assert_eq!(entries[0].head.as_deref(), Some(head));
    }
}

#[test]
fn non_utf8_path_and_branch_stay_raw_bytes() {
    let bytes = listing(&[&[
        b"worktree /repo/\xff\xfe",
        format!("HEAD {HEAD40}").as_bytes(),
        b"branch refs/heads/\xff",
    ]]);
    let entries = parse_worktree_listing(&bytes).expect("raw listing");
    assert_eq!(
        entries[0].path,
        WorktreeEntryText::Bytes(b"/repo/\xff\xfe".to_vec())
    );
    assert_eq!(
        entries[0].branch,
        Some(WorktreeEntryText::Bytes(b"refs/heads/\xff".to_vec()))
    );
}

#[test]
fn locked_and_prunable_flags_ignore_reason_bytes() {
    let bytes = listing(&[&[
        b"worktree /repo/locked",
        format!("HEAD {HEAD40}").as_bytes(),
        b"branch refs/heads/topic",
        b"locked reason \xff not utf8",
        b"prunable stale",
    ]]);
    let entries = parse_worktree_listing(&bytes).expect("flagged listing");
    assert!(entries[0].locked);
    assert!(entries[0].prunable);
    assert!(!entries[0].bare);
}

#[test]
fn unknown_attributes_are_ignored() {
    let bytes = listing(&[&[
        b"worktree /repo/main",
        format!("HEAD {HEAD40}").as_bytes(),
        b"branch refs/heads/main",
        b"worktree-path-key extra bytes here",
    ]]);
    assert_eq!(parse_worktree_listing(&bytes).map(|e| e.len()), Ok(1));
}

#[test]
fn multiple_records_parse_in_order() {
    let bytes = listing(&[
        &[b"worktree /repo/bare.git", b"bare"],
        &[
            b"worktree /repo/a",
            format!("HEAD {HEAD40}").as_bytes(),
            b"branch refs/heads/a",
        ],
        &[
            b"worktree /repo/b",
            format!("HEAD {HEAD40}").as_bytes(),
            b"detached",
        ],
    ]);
    let entries = parse_worktree_listing(&bytes).expect("multi listing");
    assert_eq!(entries.len(), 3);
    assert!(entries[0].bare);
    assert_eq!(entries[1].branch, Some(utf8("refs/heads/a")));
    assert!(entries[2].detached);
}

#[test]
fn malformed_listings_are_rejected() {
    // No trailing record separator.
    assert!(parse_worktree_listing(b"worktree /repo/x").is_err());
    // Empty stream.
    assert!(parse_worktree_listing(b"").is_err());
    // A record without the worktree attribute.
    assert!(parse_worktree_listing(&listing(&[&[format!("HEAD {HEAD40}").as_bytes()]])).is_err());
    // A relative worktree path.
    assert!(
        parse_worktree_listing(&listing(&[&[
            b"worktree relative/path",
            format!("HEAD {HEAD40}").as_bytes(),
            b"detached",
        ]]))
        .is_err()
    );
    // A duplicated attribute key.
    assert!(
        parse_worktree_listing(&listing(&[&[
            b"worktree /repo/x",
            format!("HEAD {HEAD40}").as_bytes(),
            format!("HEAD {HEAD40}").as_bytes(),
            b"detached",
        ]]))
        .is_err()
    );
    // Branch and detached on one record.
    assert!(
        parse_worktree_listing(&listing(&[&[
            b"worktree /repo/x",
            format!("HEAD {HEAD40}").as_bytes(),
            b"branch refs/heads/x",
            b"detached",
        ]]))
        .is_err()
    );
    // A bare record that still carries a HEAD.
    assert!(
        parse_worktree_listing(&listing(&[&[
            b"worktree /repo/x",
            format!("HEAD {HEAD40}").as_bytes(),
            b"bare",
        ]]))
        .is_err()
    );
    // A non-bare record without HEAD.
    assert!(parse_worktree_listing(&listing(&[&[b"worktree /repo/x", b"detached"]])).is_err());
    // A non-bare record with neither branch nor detached.
    assert!(
        parse_worktree_listing(&listing(&[&[
            b"worktree /repo/x",
            format!("HEAD {HEAD40}").as_bytes(),
        ]]))
        .is_err()
    );
    // Uppercase or wrong-length HEAD values.
    for bad in [
        "0123456789ABCDEF0123456789abcdef01234567",
        "0123456789abcdef0123456789abcdef0123456",
        "0123456789abcdef0123456789abcdef012345678",
    ] {
        assert!(
            parse_worktree_listing(&listing(&[&[
                b"worktree /repo/x",
                format!("HEAD {bad}").as_bytes(),
                b"detached",
            ]]))
            .is_err(),
            "HEAD {bad:?} must be rejected"
        );
    }
    // A stray empty field inside a record is a record boundary, so a record
    // that is only attributes after one real record is malformed.
    let mut bytes = listing(&[&[
        b"worktree /repo/x",
        format!("HEAD {HEAD40}").as_bytes(),
        b"detached",
    ]]);
    bytes.extend_from_slice(format!("HEAD {HEAD40}").as_bytes());
    bytes.push(0);
    bytes.push(0);
    assert!(parse_worktree_listing(&bytes).is_err());
}

#[test]
fn entry_text_splits_utf8_from_raw() {
    assert_eq!(entry_text(b"/repo/x"), utf8("/repo/x"));
    assert_eq!(
        entry_text(b"/repo/\xff"),
        WorktreeEntryText::Bytes(b"/repo/\xff".to_vec())
    );
}

// Carrier failures are classified by origin: post-delivery silence is
// transport-loss, a full queue is the Server's own backpressure, and every
// other carrier failure is the Worker being unreachable.
#[test]
fn request_failure_is_classified_by_origin() {
    use orchbee_workbench_protocol::{WorkbenchWorktreeListOutcome, WorktreeListUnavailableReason};

    use super::{job_request_failure, unavailable};
    use crate::worker_obcn_carrier::{
        WorkerRequestError, WorkerResponseFailure, WorkerTransportFailure,
    };
    let request_failure = |error| WorkbenchWorktreeListOutcome::from(job_request_failure(error));

    assert_eq!(
        request_failure(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::Disconnected
        )),
        WorkbenchWorktreeListOutcome::TransportLoss {}
    );
    assert_eq!(
        request_failure(WorkerRequestError::PostDeliveryOutcomeUnknown(
            WorkerResponseFailure::TimedOut
        )),
        WorkbenchWorktreeListOutcome::TransportLoss {}
    );
    assert_eq!(
        request_failure(WorkerRequestError::Backpressure),
        unavailable(WorktreeListUnavailableReason::ServerBusy)
    );
    for error in [
        WorkerRequestError::Unavailable,
        WorkerRequestError::WrongWorker,
        WorkerRequestError::WrongGeneration,
        WorkerRequestError::NotDelivered(WorkerTransportFailure::Closed),
        WorkerRequestError::NotDelivered(WorkerTransportFailure::Io(
            std::io::ErrorKind::BrokenPipe,
        )),
    ] {
        assert_eq!(
            request_failure(error),
            unavailable(WorktreeListUnavailableReason::WorkerUnavailable),
            "{error:?}"
        );
    }
}

// The backend's physical-unknown is a truthful owner state; the public read
// surface reports worker-unavailable while job-unknown stays distinct.
#[test]
fn poll_physical_unknown_is_worker_unavailable() {
    use orchbee_capability_node_protocol::WorktreeJobPollOutcome;
    use orchbee_workbench_protocol::{WorkbenchWorktreeListOutcome, WorktreeListUnavailableReason};

    use super::{JobTerminal, PollStep, poll_step, unavailable};

    let PollStep::Done(JobTerminal::Failed(outcome)) =
        poll_step(WorktreeJobPollOutcome::PhysicalUnknown)
    else {
        panic!("physical unknown must terminate");
    };
    assert_eq!(
        outcome,
        unavailable(WorktreeListUnavailableReason::WorkerUnavailable)
    );

    let PollStep::Done(JobTerminal::Failed(outcome)) = poll_step(WorktreeJobPollOutcome::Unknown)
    else {
        panic!("job unknown must terminate");
    };
    assert_eq!(
        outcome,
        unavailable(WorktreeListUnavailableReason::JobUnknown)
    );

    let PollStep::Done(JobTerminal::Failed(outcome)) =
        poll_step(WorktreeJobPollOutcome::RejectedFencing)
    else {
        panic!("fencing must terminate");
    };
    assert!(matches!(
        outcome,
        WorkbenchWorktreeListOutcome::Unavailable { .. }
    ));
}

// Both spawn arms keep their wire variants here; the CLI maps them together
// to service-unavailable/worker-unavailable.
#[test]
fn poll_spawn_failures_keep_the_wire_variants() {
    use orchbee_capability_node_protocol::{WorktreeJobPollOutcome, WorktreeJobStage};
    use orchbee_workbench_protocol::{WorkbenchWorktreeListOutcome, WorktreeListStage};

    use super::{JobTerminal, PollStep, poll_step};

    let PollStep::Done(JobTerminal::Failed(outcome)) =
        poll_step(WorktreeJobPollOutcome::SpawnUnavailable {
            stage: WorktreeJobStage::Probe,
        })
    else {
        panic!("spawn-unavailable must terminate");
    };
    assert_eq!(
        outcome,
        WorkbenchWorktreeListOutcome::SpawnUnavailable {
            stage: WorktreeListStage::Probe
        }
    );

    let PollStep::Done(JobTerminal::Failed(outcome)) =
        poll_step(WorktreeJobPollOutcome::SpawnFailed {
            stage: WorktreeJobStage::List,
        })
    else {
        panic!("spawn-failed must terminate");
    };
    assert_eq!(
        outcome,
        WorkbenchWorktreeListOutcome::SpawnFailed {
            stage: WorktreeListStage::List
        }
    );
}

// The exact Git exit status rides through verbatim — `None` when the process
// was signaled, so the CLI emits an explicit JSON null rather than a guess.
#[test]
fn poll_git_failed_keeps_the_status_verbatim() {
    use orchbee_capability_node_protocol::{WorktreeJobPollOutcome, WorktreeJobStage};
    use orchbee_workbench_protocol::{WorkbenchWorktreeListOutcome, WorktreeListStage};

    use super::{JobTerminal, PollStep, poll_step};

    for status in [Some(1), Some(128), None] {
        let PollStep::Done(JobTerminal::Failed(outcome)) =
            poll_step(WorktreeJobPollOutcome::GitFailed {
                stage: WorktreeJobStage::List,
                status,
                stderr: Vec::new(),
            })
        else {
            panic!("git-failed must terminate");
        };
        assert_eq!(
            outcome,
            WorkbenchWorktreeListOutcome::GitFailed {
                stage: WorktreeListStage::List,
                status
            }
        );
    }
}

#[test]
fn poll_pending_and_completed_pass_through() {
    use orchbee_capability_node_protocol::WorktreeJobPollOutcome;

    use super::{JobTerminal, PollStep, poll_step};

    assert!(matches!(
        poll_step(WorktreeJobPollOutcome::Pending),
        PollStep::Pending
    ));

    let listing = b"worktree /repo\0\0".to_vec();
    let PollStep::Done(JobTerminal::Completed(observed)) =
        poll_step(WorktreeJobPollOutcome::Completed {
            common_dir: b"/repo/.git".to_vec(),
            listing: listing.clone(),
            stderr: Vec::new(),
        })
    else {
        panic!("completed must terminate with the listing");
    };
    assert_eq!(observed.listing, listing);
    assert_eq!(observed.source_checkout, None);
}

#[test]
fn windows_drive_paths_use_shared_absolute_path_validation() {
    for path in ["C:/repo/main", "D:/repo with spaces/linked"] {
        let bytes = listing(&[&[
            format!("worktree {path}").as_bytes(),
            format!("HEAD {HEAD40}").as_bytes(),
            b"branch refs/heads/main",
        ]]);
        let entries = parse_worktree_listing(&bytes).unwrap();
        assert_eq!(entries[0].path, utf8(path));
    }
    for path in ["relative", "C:relative", ""] {
        assert!(
            parse_worktree_listing(&listing(&[&[
                format!("worktree {path}").as_bytes(),
                b"bare"
            ]]))
            .is_err()
        );
    }
}
