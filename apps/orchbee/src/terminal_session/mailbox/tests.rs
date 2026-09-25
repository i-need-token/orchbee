use super::*;
use crate::{
    client_view::ClientAction,
    terminal_session::{
        state::{NativeSessionState, SelectionPlan, SessionNotice, SessionPhaseKind},
        workbench_projection::project_native_standalone_tabs,
    },
};
use orchbee_client_server_protocol::{ControllerId, ViewerId};
use orchbee_workbench_protocol::{
    Navigation, PanelNode, Presentation, PresentationBinding, ProtocolVersion, SiteId, Snapshot,
    Tab, TerminalContext, TerminalId, TerminalPresentationId, WorkbenchAction, WorkbenchCommand,
    WorkbenchLabel, WorkbenchPanelId, WorkbenchRevision, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchTerminalGeometry, WorkerId,
};
use std::sync::{
    Arc, Barrier, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};

mod close_reconciliation;
mod context_selection;
mod generated;
mod lifecycle_generated;
mod membership;
mod move_tab;
mod panel;
mod switch_membership;
mod tab_close;
mod tab_rename;
mod workspace_setup;

fn fixture_viewer_id() -> ViewerId {
    ViewerId::parse("11111111111111111111111111111111")
        .expect("mailbox viewer fixture must use a valid ViewerId")
}

#[test]
fn live_empty_mailbox_admits_and_dispatches_refresh_without_rebootstrapping() {
    let mailbox = bootstrapped_mailbox(7, 9);
    mailbox.lock().desired = MailboxMembership::Live {
        desired: None,
        panes: Vec::new(),
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    assert_eq!(
        mailbox.try_enqueue_refresh(deadline, |_generation| Ok::<_, ()>(())),
        Ok(1)
    );
    let lease = mailbox
        .try_recv()
        .unwrap()
        .expect("live Empty dispatches Refresh");
    assert!(matches!(lease.record, QueuedCommand::Refresh(_)));
    assert_eq!(mailbox.lock().attachment_generation, 7);
    let cold = CommandMailbox::new(9, geometry(80));
    assert_eq!(
        cold.try_enqueue_refresh(deadline, |_generation| Ok::<_, ()>(())),
        Err(AdmissionError::BootstrapPending)
    );
}

#[test]
fn live_empty_mailbox_rejects_old_attachment_commands_and_second_bootstrap() {
    let mailbox = bootstrapped_mailbox(7, 9);
    mailbox.lock().desired = MailboxMembership::Live {
        desired: None,
        panes: Vec::new(),
    };
    assert!(matches!(
        mailbox.try_enqueue(QueuedCommand::Poll(Tagged::new(pane_ref(&key(7, 1)), ()))),
        Err(AdmissionError::StaleAttachment)
    ));
    let base = Instant::now();
    let error = mailbox.publish_event_before_using(
        WorkerEvent::Bootstrap {
            cache: projected_one_tab(),
        },
        EventDeadline::Startup(base + Duration::from_secs(10)),
        &FakeClock::new(base),
    );
    assert!(matches!(
        error,
        Err(EventError::Closed(MailboxClosed {
            failure: CarrierFailure::Protocol
        }))
    ));
    assert_eq!(mailbox.lock().attachment_generation, 7);
}

#[derive(Debug)]
struct FakeClock(Mutex<Instant>);

impl FakeClock {
    fn new(now: Instant) -> Self {
        Self(Mutex::new(now))
    }

    fn set(&self, now: Instant) {
        *self.0.lock().unwrap_or_else(|poison| poison.into_inner()) = now;
    }
}

impl MailboxClock for FakeClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }
}

#[derive(Debug)]
struct ObservedClock {
    now: Mutex<Instant>,
    observations: Mutex<usize>,
    observed: Condvar,
    contention: Mutex<Option<ContentionCheckpoint>>,
}

#[derive(Debug)]
struct ContentionCheckpoint {
    reached: std::sync::mpsc::Sender<()>,
    resume: std::sync::mpsc::Receiver<()>,
}

impl ObservedClock {
    fn new(now: Instant) -> Self {
        Self {
            now: Mutex::new(now),
            observations: Mutex::new(0),
            observed: Condvar::new(),
            contention: Mutex::new(None),
        }
    }

    fn with_contention_checkpoint(
        now: Instant,
    ) -> (
        Self,
        std::sync::mpsc::Receiver<()>,
        std::sync::mpsc::Sender<()>,
    ) {
        let (reached_tx, reached_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let clock = Self {
            now: Mutex::new(now),
            observations: Mutex::new(0),
            observed: Condvar::new(),
            contention: Mutex::new(Some(ContentionCheckpoint {
                reached: reached_tx,
                resume: resume_rx,
            })),
        };
        (clock, reached_rx, resume_tx)
    }

    fn set(&self, now: Instant) {
        *self.now.lock().unwrap_or_else(|poison| poison.into_inner()) = now;
    }

    fn observation_count(&self) -> usize {
        *self
            .observations
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn wait_for_observation_after(&self, previous: usize) {
        let mut observations = self
            .observations
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        while *observations <= previous {
            observations = self
                .observed
                .wait(observations)
                .unwrap_or_else(|poison| poison.into_inner());
        }
    }
}

impl MailboxClock for ObservedClock {
    fn now(&self) -> Instant {
        let now = *self.now.lock().unwrap_or_else(|poison| poison.into_inner());
        let mut observations = self
            .observations
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *observations += 1;
        self.observed.notify_all();
        now
    }

    fn after_mailbox_contention(&self) {
        let checkpoint = self
            .contention
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take();
        let Some(checkpoint) = checkpoint else {
            return;
        };
        checkpoint.reached.send(()).unwrap();
        checkpoint
            .resume
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
    }
}

#[test]
fn test_event_await_uses_existing_cell_and_real_wall_guard() {
    let base = Instant::now();
    let clock = Arc::new(FakeClock::new(base));
    let mailbox = Arc::new(CommandMailbox::new(10, geometry(80)));
    let start = Arc::new(Barrier::new(2));
    let publisher_mailbox = Arc::clone(&mailbox);
    let publisher_clock = Arc::clone(&clock);
    let publisher_start = Arc::clone(&start);
    let publisher = std::thread::spawn(move || {
        publisher_start.wait();
        publisher_mailbox
            .publish_event_before_using(
                WorkerEvent::Bootstrap {
                    cache: projected_one_tab(),
                },
                EventDeadline::Startup(base + Duration::from_secs(1)),
                publisher_clock.as_ref(),
            )
            .unwrap();
    });

    start.wait();
    let event = mailbox
        .await_event_before_using(clock.as_ref(), Instant::now() + Duration::from_secs(1))
        .unwrap()
        .unwrap();
    assert!(matches!(event.into_event(), WorkerEvent::Bootstrap { .. }));
    publisher.join().unwrap();

    assert!(
        mailbox
            .await_event_before_using(clock.as_ref(), Instant::now())
            .unwrap()
            .is_none()
    );
}

#[test]
fn bootstrap_uses_latest_geometry_and_publishes_first_desired_key_with_its_reply() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let initial = Geometry::new(80, 24, 8, 16).unwrap();
    let latest = Geometry::new(90, 24, 8, 16).unwrap();
    let mailbox = CommandMailbox::new(10, initial);
    assert!(matches!(
        mailbox.try_enqueue(QueuedCommand::Poll(Tagged::new(pane_ref(&key(1, 1)), ()))),
        Err(AdmissionError::BootstrapPending)
    ));
    mailbox.observe_geometry(latest).unwrap();
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::Bootstrap {
                cache: projected_one_tab(),
            },
            EventDeadline::Startup(base + Duration::from_secs(1)),
            &clock,
        )
        .unwrap();
    let event = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    let WorkerEvent::Bootstrap { cache } = event.into_event() else {
        panic!("expected bootstrap event")
    };
    let mut canonical = None;
    assert_eq!(
        mailbox
            .deposit_bootstrap_reply_before_using(&wait, &clock, |generation, geometry| {
                let (state, target) = NativeSessionState::starting(cache, generation, geometry)
                    .ok_or(CarrierFailure::AuthorityDivergence)?;
                canonical = Some(state);
                Ok(target)
            })
            .unwrap(),
        MutationResult::Applied
    );
    let state = canonical.unwrap();
    assert_eq!(state.latest_geometry(), latest);
    assert_eq!(mailbox.latest_geometry().unwrap(), latest);
    assert_eq!(mailbox.lock().desired.key(), Some(state.desired().unwrap()));
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::BootstrapApplied(target))
            if target.selected().key() == state.desired().unwrap()
    ));
    assert_eq!(mailbox.cursors(), (1, 0, 10));

    let newer = mailbox
        .publish_event_before_using(
            WorkerEvent::SetupConfigured {
                key: state.focus_pane_ref().unwrap(),
                mode: ControllerMode::ReadOnly,
                applied_geometry: None,
            },
            EventDeadline::Startup(base + Duration::from_secs(1)),
            &clock,
        )
        .unwrap();
    let stale_ran = AtomicBool::new(false);
    assert_eq!(
        mailbox
            .deposit_bootstrap_reply_before_using(&wait, &clock, |_generation, _geometry| {
                stale_ran.store(true, Ordering::SeqCst);
                Err(CarrierFailure::Protocol)
            })
            .unwrap(),
        MutationResult::Stale
    );
    assert!(!stale_ran.load(Ordering::SeqCst));
    assert_eq!(mailbox.cells(), ("event", "empty"));
    assert_eq!(
        mailbox
            .take_event_before_using(&clock)
            .unwrap()
            .unwrap()
            .wait_key(),
        Some(&newer)
    );
}

#[test]
fn fifo_has_exactly_eight_records_and_keeps_causal_settlement_outside_the_capacity() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 10);
    // The first record must be the Poll so the lease below carries a poll key; the
    // remaining slots take Inputs because only one ordinary Poll may be outstanding.
    assert!(mailbox.try_enqueue(poll(&mailbox)).is_ok());
    for _ in 1..COMMAND_CAPACITY {
        assert!(mailbox.try_enqueue(input(&mailbox, 0)).is_ok());
    }
    assert!(matches!(
        mailbox.try_enqueue(poll(&mailbox)),
        Err(AdmissionError::Full)
    ));

    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    mailbox
        .publish_poll_result_before_using(
            poll_key.clone(),
            Some(vec![1, 2, 3]),
            base + Duration::from_secs(1),
            &clock,
        )
        .unwrap();
    let taken = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert!(matches!(
        taken.into_event(),
        WorkerEvent::PollResult {
            record: Some(record),
            ..
        } if record == vec![1, 2, 3]
    ));
    assert_eq!(mailbox.cells(), ("empty", "awaiting"));
    // The Poll lease is still active, so the drained slot takes an Input.
    assert!(mailbox.try_enqueue(input(&mailbox, 0)).is_ok());
    assert!(matches!(
        mailbox.try_enqueue(poll(&mailbox)),
        Err(AdmissionError::Full)
    ));
    assert_eq!(
        mailbox
            .deposit_causal(&poll_key, CausalSettlement::NoFeedback)
            .unwrap(),
        MutationResult::Applied
    );
    assert_eq!(mailbox.cells(), ("empty", "settled"));
    assert_eq!(
        mailbox
            .take_causal_before_using(&poll_key, base + Duration::from_secs(1), &clock)
            .unwrap(),
        Some(CausalSettlement::NoFeedback)
    );
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn capacity_generation_builder_and_identity_failures_do_not_burn_other_cursors() {
    let full = bootstrapped_mailbox(7, 40);
    for _ in 0..COMMAND_CAPACITY {
        full.try_enqueue(input(&full, 0)).unwrap();
    }
    let before = full.cursors();
    let result = full.try_enqueue_create(|_, _| -> Result<CreateMarker, ()> {
        panic!("full admission must not invoke the builder")
    });
    assert!(matches!(result, Err(AdmissionError::Full)));
    assert_eq!(full.cursors(), before);

    let exhausted_generation = bootstrapped_mailbox(u64::MAX, 40);
    let before = exhausted_generation.cursors();
    let result = exhausted_generation.try_enqueue_create(|_, _| -> Result<CreateMarker, ()> {
        panic!("generation exhaustion must not invoke the builder")
    });
    assert!(matches!(
        result,
        Err(AdmissionError::AttachmentGenerationExhausted)
    ));
    assert_eq!(exhausted_generation.cursors(), before);
    assert_eq!(exhausted_generation.is_closed(), None);

    let exhausted_identity = bootstrapped_mailbox(7, u128::MAX);
    let before = exhausted_identity.cursors();
    let result = exhausted_identity.try_enqueue_create(|_, _| -> Result<CreateMarker, ()> {
        panic!("identity exhaustion must not invoke the builder")
    });
    assert!(matches!(
        result,
        Err(AdmissionError::Closed(MailboxClosed {
            failure: CarrierFailure::IdentityExhausted
        }))
    ));
    assert_eq!(exhausted_identity.cursors(), before);
}

#[test]
fn one_identity_cursor_allocates_obcs_and_workbench_ids_without_reuse() {
    let mailbox = bootstrapped_mailbox(1, 0x10);
    let obcs = mailbox.allocate_obcs_request_id().unwrap();
    let workbench = mailbox.allocate_workbench_request_id().unwrap();
    assert_eq!(obcs.to_string(), "00000000000000000000000000000011");
    assert_eq!(workbench.to_string(), "00000000000000000000000000000012");
    assert_eq!(mailbox.cursors(), (1, 0, 0x12));

    let exhausted = bootstrapped_mailbox(1, u128::MAX);
    assert_eq!(
        exhausted.allocate_obcs_request_id().unwrap_err().failure(),
        CarrierFailure::IdentityExhausted
    );
    assert_eq!(
        exhausted
            .allocate_workbench_request_id()
            .unwrap_err()
            .failure(),
        CarrierFailure::IdentityExhausted
    );
}

#[test]
fn create_commits_generation_and_identity_only_after_infallible_append_point() {
    let base = Instant::now();
    let mailbox = bootstrapped_mailbox(3, 20);
    let failed = mailbox.try_enqueue_create(|_, _| Err::<CreateMarker, _>("state refused"));
    assert_eq!(failed, Err(AdmissionError::Build("state refused")));
    assert_eq!(mailbox.cursors(), (3, 0, 20));

    let operation = mailbox
        .try_enqueue_create(|generation, request_id| {
            Ok::<_, ()>(create_marker(base, generation, request_id))
        })
        .unwrap();
    assert_eq!(operation.reserved_generation(), 4);
    assert_eq!(operation.request_id().to_string(), id(21));
    assert_eq!(mailbox.cursors(), (4, 0, 21));
    let mut lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Create(marker) = lease.record_mut() else {
        panic!("expected create marker")
    };
    assert!(marker.take_request().is_some());
    assert!(marker.take_request().is_none());
    assert_eq!(marker.operation().key(), &operation);
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn ui_reply_cell_has_no_second_payload_and_stale_wait_cannot_satisfy_newer_event() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let first = mailbox
        .publish_event_before_using(
            cleanup_event(key(1, 1)),
            EventDeadline::Forward(base + Duration::from_secs(2)),
            &clock,
        )
        .unwrap();
    assert_eq!(mailbox.cells(), ("event", "empty"));
    let taken = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert_eq!(taken.wait_key(), Some(&first));
    assert_eq!(mailbox.cells(), ("awaiting-ui", "empty"));
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&first, &clock, |_| {
                Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
            })
            .unwrap(),
        MutationResult::Applied
    );
    assert_eq!(mailbox.cells(), ("reply", "empty"));
    assert!(matches!(
        mailbox.take_deposited_reply(&first).unwrap(),
        Some(UiReply::CleanupApplied(CleanupContinuation::Stable))
    ));
    assert_eq!(mailbox.cells(), ("empty", "empty"));

    let second = mailbox
        .publish_event_before_using(
            cleanup_event(key(2, 2)),
            EventDeadline::Forward(base + Duration::from_secs(2)),
            &clock,
        )
        .unwrap();
    assert!(second.serial() > first.serial());
    let stale_ran = AtomicBool::new(false);
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&first, &clock, |_| {
                stale_ran.store(true, Ordering::SeqCst);
                Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
            })
            .unwrap(),
        MutationResult::Stale
    );
    assert!(!stale_ran.load(Ordering::SeqCst));
    assert_eq!(mailbox.cells(), ("event", "empty"));
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&first, &clock, |_| {
                Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
            })
            .unwrap(),
        MutationResult::Stale
    );
    assert_eq!(mailbox.cells(), ("awaiting-ui", "empty"));
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&second, &clock, |_| {
                Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
            })
            .unwrap(),
        MutationResult::Applied
    );
}

#[test]
fn genuinely_current_ui_key_mismatch_closes_as_protocol_failure() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let wait_key = mailbox
        .publish_event_before_using(
            cleanup_event(key(1, 1)),
            EventDeadline::Forward(base + Duration::from_secs(2)),
            &clock,
        )
        .unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    let mut mismatched = wait_key.clone();
    mismatched.correlation = UiCorrelation::Attachment(pane_ref(&key(1, 99)));
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&mismatched, &clock, |_| {
                Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
            })
            .unwrap_err()
            .closed_failure(),
        Some(CarrierFailure::Protocol)
    );
}

#[test]
fn cleanup_settlement_uses_strict_total_but_deposited_reply_has_no_take_deadline() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let total = base + Duration::from_secs(1);
    let wait = mailbox
        .publish_event_before_using(
            cleanup_event(key(1, 1)),
            EventDeadline::TotalCleanup(total),
            &clock,
        )
        .unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_none());
    clock.set(total - Duration::from_nanos(1));
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
            })
            .unwrap(),
        MutationResult::Applied
    );
    clock.set(total + Duration::from_secs(20));
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::CleanupApplied(CleanupContinuation::Stable))
    ));

    let equality = bootstrapped_mailbox(1, 5);
    clock.set(base);
    let wait = equality
        .publish_event_before_using(
            cleanup_event(key(1, 1)),
            EventDeadline::TotalCleanup(total),
            &clock,
        )
        .unwrap();
    equality.take_event_before_using(&clock).unwrap().unwrap();
    clock.set(total);
    assert_eq!(
        equality
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
            })
            .unwrap_err()
            .closed_failure(),
        Some(CarrierFailure::Deadline)
    );
}

#[test]
fn reply_settlement_resamples_after_mailbox_contention_and_never_runs_late_closure() {
    let base = Instant::now();
    let total = base + Duration::from_secs(1);
    let (clock, contention_reached, resume_after_contention) =
        ObservedClock::with_contention_checkpoint(base);
    let clock = Arc::new(clock);
    let mailbox = Arc::new(bootstrapped_mailbox(1, 5));
    let wait = mailbox
        .publish_event_before_using(
            cleanup_event(key(1, 1)),
            EventDeadline::TotalCleanup(total),
            clock.as_ref(),
        )
        .unwrap();
    mailbox
        .take_event_before_using(clock.as_ref())
        .unwrap()
        .unwrap();

    let held = mailbox.lock();
    let closure_ran = Arc::new(AtomicBool::new(false));
    let task_mailbox = Arc::clone(&mailbox);
    let task_clock = Arc::clone(&clock);
    let task_ran = Arc::clone(&closure_ran);
    let task_wait = wait.clone();
    let previous_observations = clock.observation_count();
    let task = std::thread::spawn(move || {
        task_mailbox.deposit_ui_reply_before_using(&task_wait, task_clock.as_ref(), |_| {
            task_ran.store(true, Ordering::SeqCst);
            Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
        })
    });
    contention_reached
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    clock.set(total);
    drop(held);
    resume_after_contention.send(()).unwrap();

    let result = task.join().unwrap();
    assert_eq!(
        result.unwrap_err().closed_failure(),
        Some(CarrierFailure::Deadline)
    );
    assert!(!closure_ran.load(Ordering::SeqCst));
    assert!(clock.observation_count() >= previous_observations + 2);
}

#[test]
fn forward_expiry_invalidates_only_exact_wait_and_late_ui_callback_is_stale() {
    let base = Instant::now();
    let cutoff = base + Duration::from_secs(1);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let wait = mailbox
        .publish_event_before_using(
            cleanup_event(key(1, 1)),
            EventDeadline::Forward(cutoff),
            &clock,
        )
        .unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    clock.set(cutoff);
    let closure_ran = AtomicBool::new(false);
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                closure_ran.store(true, Ordering::SeqCst);
                Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
            })
            .unwrap_err(),
        EventError::ForwardExpired
    );
    assert!(!closure_ran.load(Ordering::SeqCst));
    assert_eq!(mailbox.is_closed(), None);
    assert_eq!(mailbox.cells(), ("forward-expired", "empty"));
    assert_eq!(
        mailbox
            .await_ui_reply_before_using(&wait, cutoff, &clock)
            .unwrap_err(),
        EventError::ForwardExpired
    );
    assert_eq!(mailbox.cells(), ("empty", "empty"));
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                closure_ran.store(true, Ordering::SeqCst);
                Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
            })
            .unwrap(),
        MutationResult::Stale
    );
    assert!(!closure_ran.load(Ordering::SeqCst));
}

#[test]
fn exact_forward_expiry_receipt_survives_ui_settlement_until_worker_observes_it() {
    let base = Instant::now();
    let cutoff = base + Duration::from_secs(1);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::SetupConfigured {
                key: pane_ref(&key(1, 1)),
                mode: ControllerMode::Controlling(ControllerId::parse(&id(8)).unwrap()),
                applied_geometry: Some(geometry(80)),
            },
            EventDeadline::Forward(cutoff),
            &clock,
        )
        .unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();

    clock.set(cutoff);
    let closure_ran = AtomicBool::new(false);
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                closure_ran.store(true, Ordering::SeqCst);
                Ok(UiReply::SetupConfigurationSettled(
                    SetupEventDisposition::Applied,
                ))
            })
            .unwrap_err(),
        EventError::ForwardExpired
    );
    assert!(!closure_ran.load(Ordering::SeqCst));
    assert_eq!(mailbox.cells(), ("forward-expired", "empty"));
    assert_eq!(mailbox.is_closed(), None);

    assert_eq!(
        mailbox
            .await_ui_reply_before_using(&wait, cutoff, &clock)
            .unwrap_err(),
        EventError::ForwardExpired
    );
    assert_eq!(mailbox.cells(), ("empty", "empty"));
    assert_eq!(mailbox.is_closed(), None);
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                Ok(UiReply::SetupConfigurationSettled(
                    SetupEventDisposition::Applied,
                ))
            })
            .unwrap(),
        MutationResult::Stale
    );

    clock.set(base);
    let mismatched = bootstrapped_mailbox(1, 5);
    let wait = mismatched
        .publish_event_before_using(
            WorkerEvent::SetupConfigured {
                key: pane_ref(&key(1, 1)),
                mode: ControllerMode::Controlling(ControllerId::parse(&id(8)).unwrap()),
                applied_geometry: Some(geometry(80)),
            },
            EventDeadline::Forward(cutoff),
            &clock,
        )
        .unwrap();
    mismatched.take_event_before_using(&clock).unwrap().unwrap();
    clock.set(cutoff);
    assert_eq!(
        mismatched
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                Ok(UiReply::SetupConfigurationSettled(
                    SetupEventDisposition::Applied,
                ))
            })
            .unwrap_err(),
        EventError::ForwardExpired
    );
    let mut wrong = wait.clone();
    wrong.correlation = UiCorrelation::Attachment(pane_ref(&key(1, 99)));
    assert_eq!(
        mismatched
            .await_ui_reply_before_using(&wrong, cutoff, &clock)
            .unwrap_err()
            .closed_failure(),
        Some(CarrierFailure::Protocol)
    );
}

#[test]
fn forward_expiry_preserves_exact_worker_receipt_and_clears_causal_wait() {
    let base = Instant::now();
    let cutoff = base + Duration::from_secs(1);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    let wait = mailbox
        .publish_event_before_using(
            cleanup_event(key(1, 1)),
            EventDeadline::Forward(cutoff),
            &clock,
        )
        .unwrap();
    clock.set(cutoff);
    assert!(matches!(
        mailbox.take_event_before_using(&clock),
        Err(EventError::ForwardExpired)
    ));
    assert_eq!(mailbox.cells(), ("forward-expired", "empty"));
    assert_eq!(
        mailbox
            .await_ui_reply_before_using(&wait, cutoff, &clock)
            .unwrap_err(),
        EventError::ForwardExpired
    );
    assert_eq!(mailbox.cells(), ("empty", "empty"));
    assert_eq!(mailbox.is_closed(), None);

    clock.set(base);
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    mailbox
        .publish_poll_result_before_using(poll_key.clone(), Some(vec![1]), cutoff, &clock)
        .unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    clock.set(cutoff);
    assert_eq!(
        mailbox
            .take_causal_before_using(&poll_key, cutoff, &clock)
            .unwrap_err(),
        EventError::ForwardExpired
    );
    assert_eq!(mailbox.cells(), ("empty", "empty"));
    assert_eq!(mailbox.is_closed(), None);
    assert_eq!(
        mailbox
            .deposit_causal(&poll_key, CausalSettlement::NoFeedback)
            .unwrap(),
        MutationResult::Stale
    );
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn poll_publication_reserves_causal_wait_before_ui_takes_the_event() {
    let base = Instant::now();
    let clock = Arc::new(FakeClock::new(base));
    let mailbox = Arc::new(bootstrapped_mailbox(1, 5));
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    assert_eq!(
        mailbox
            .publish_poll_result_before_using(
                poll_key.clone(),
                Some(vec![1]),
                base + Duration::from_secs(1),
                clock.as_ref(),
            )
            .unwrap(),
        MutationResult::Applied
    );
    assert_eq!(mailbox.cells(), ("event", "awaiting"));

    let worker_mailbox = Arc::clone(&mailbox);
    let worker_clock = Arc::clone(&clock);
    let worker_key = poll_key.clone();
    let (started_tx, started_rx) = std::sync::mpsc::sync_channel(0);
    let (finished_tx, finished_rx) = std::sync::mpsc::sync_channel(0);
    let worker = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = worker_mailbox.take_causal_before_using(
            &worker_key,
            base + Duration::from_secs(1),
            worker_clock.as_ref(),
        );
        finished_tx.send(result).unwrap();
    });
    started_rx.recv().unwrap();
    assert!(matches!(
        finished_rx.recv_timeout(Duration::from_millis(20)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));

    let event = mailbox
        .take_event_before_using(clock.as_ref())
        .unwrap()
        .unwrap();
    assert!(matches!(
        event.into_event(),
        WorkerEvent::PollResult {
            record: Some(record),
            ..
        } if record == vec![1]
    ));
    mailbox.complete_empty_poll(&poll_key).unwrap();
    assert_eq!(
        finished_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
        Ok(Some(CausalSettlement::NoFeedback))
    );
    worker.join().unwrap();
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn superseded_poll_result_is_stale_and_does_not_publish_or_retire() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    mailbox.lock().attachment_generation = 2;
    mailbox.publish_desired(pane_key(&key(2, 2))).unwrap();
    assert_eq!(
        mailbox
            .publish_poll_result_before_using(
                poll_key.clone(),
                Some(vec![1]),
                base + Duration::from_secs(1),
                &clock,
            )
            .unwrap(),
        MutationResult::Stale
    );
    assert_eq!(mailbox.cells(), ("empty", "empty"));
    assert_eq!(mailbox.is_closed(), None);
    assert_eq!(
        mailbox
            .take_causal_before_using(&poll_key, base + Duration::from_secs(1), &clock,)
            .unwrap(),
        None
    );
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn causal_wait_clamps_to_the_earliest_later_barrier() {
    let base = Instant::now();
    let barrier_cutoff = base + Duration::from_secs(5);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    mailbox
        .publish_poll_result_before_using(
            poll_key.clone(),
            Some(vec![1]),
            base + Duration::from_secs(50),
            &clock,
        )
        .unwrap();
    mailbox
        .try_enqueue_refresh(barrier_cutoff, |_| Ok::<_, ()>(()))
        .unwrap();
    clock.set(barrier_cutoff);
    assert!(matches!(
        mailbox.take_causal_before_using(&poll_key, base + Duration::from_secs(50), &clock,),
        Err(EventError::ForwardExpired)
    ));
    assert_eq!(mailbox.cells(), ("empty", "empty"));
    assert_eq!(mailbox.is_closed(), None);
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn poll_event_publication_and_ui_take_clamp_to_a_later_create_cutoff() {
    let base = Instant::now();
    let create_cutoff = OperationDeadlines::from_sample(base)
        .unwrap()
        .forward_cutoff();
    let poll_cutoff = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);

    let before_publication = bootstrapped_mailbox(1, 5);
    let desired = before_publication.lock().desired.key().unwrap().clone();
    before_publication
        .try_enqueue(poll(&before_publication))
        .unwrap();
    let lease = before_publication.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    before_publication
        .try_enqueue_create(|generation, request_id| {
            Ok::<_, ()>(create_marker(base, generation, request_id))
        })
        .unwrap();
    clock.set(create_cutoff);
    assert_eq!(
        before_publication.publish_poll_result_before_using(
            poll_key,
            Some(vec![1]),
            poll_cutoff,
            &clock,
        ),
        Err(EventError::ForwardExpired)
    );
    assert_eq!(before_publication.cells(), ("empty", "empty"));
    assert_eq!(before_publication.lock().desired.key(), Some(&desired));
    assert_eq!(before_publication.is_closed(), None);
    before_publication.finish_lease(lease).unwrap();

    clock.set(base);
    let before_take = bootstrapped_mailbox(1, 5);
    let desired = before_take.lock().desired.key().unwrap().clone();
    before_take.try_enqueue(poll(&before_take)).unwrap();
    let lease = before_take.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    before_take
        .publish_poll_result_before_using(poll_key, Some(vec![2]), poll_cutoff, &clock)
        .unwrap();
    before_take
        .try_enqueue_create(|generation, request_id| {
            Ok::<_, ()>(create_marker(base, generation, request_id))
        })
        .unwrap();
    clock.set(create_cutoff);
    assert!(matches!(
        before_take.take_event_before_using(&clock),
        Err(EventError::ForwardExpired)
    ));
    assert_eq!(before_take.cells(), ("empty", "empty"));
    assert_eq!(before_take.lock().desired.key(), Some(&desired));
    assert_eq!(before_take.is_closed(), None);
    before_take.finish_lease(lease).unwrap();
}

#[test]
fn generic_forward_events_clamp_placement_take_and_ui_settlement_to_later_barriers() {
    let base = Instant::now();
    let deadlines = OperationDeadlines::from_sample(base).unwrap();
    let create_cutoff = deadlines.forward_cutoff();
    let ordinary_cutoff = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);

    for stage in 0..3 {
        let mailbox = bootstrapped_mailbox(1, 5);
        let desired = mailbox.lock().desired.key().unwrap().clone();
        mailbox
            .try_enqueue(QueuedCommand::Resize(Tagged::new(
                pane_ref(desired.anchor()),
                geometry(81),
            )))
            .unwrap();
        let lease = mailbox.try_recv().unwrap().unwrap();
        let event = || {
            if stage == 2 {
                WorkerEvent::FeedbackAccepted {
                    key: pane_ref(desired.anchor()),
                    frame: FrameIdentity::new(7),
                }
            } else {
                WorkerEvent::SetupConfigured {
                    key: pane_ref(desired.anchor()),
                    mode: ControllerMode::Controlling(ControllerId::parse(&id(8)).unwrap()),
                    applied_geometry: Some(geometry(81)),
                }
            }
        };
        let mut wait = None;
        if stage != 0 {
            wait = Some(
                mailbox
                    .publish_event_before_using(
                        event(),
                        EventDeadline::Forward(ordinary_cutoff),
                        &clock,
                    )
                    .unwrap(),
            );
            if stage == 2 {
                mailbox.take_event_before_using(&clock).unwrap().unwrap();
            }
        }
        mailbox
            .try_enqueue_create(|generation, request_id| {
                Ok::<_, ()>(create_marker(base, generation, request_id))
            })
            .unwrap();
        clock.set(create_cutoff);
        match stage {
            0 => assert!(matches!(
                mailbox.publish_event_before_using(
                    event(),
                    EventDeadline::Forward(ordinary_cutoff),
                    &clock,
                ),
                Err(EventError::ForwardExpired)
            )),
            1 => assert!(matches!(
                mailbox.take_event_before_using(&clock),
                Err(EventError::ForwardExpired)
            )),
            2 => {
                let closure_ran = AtomicBool::new(false);
                assert!(matches!(
                    mailbox.deposit_ui_reply_before_using(wait.as_ref().unwrap(), &clock, |_| {
                        closure_ran.store(true, Ordering::SeqCst);
                        Ok(UiReply::FeedbackSettled(SetupEventDisposition::Applied))
                    },),
                    Err(EventError::ForwardExpired)
                ));
                assert!(!closure_ran.load(Ordering::SeqCst));
            }
            _ => unreachable!(),
        }
        if let Some(wait) = &wait {
            assert_eq!(mailbox.cells(), ("forward-expired", "empty"));
            assert_eq!(
                mailbox
                    .await_ui_reply_before_using(wait, create_cutoff, &clock)
                    .unwrap_err(),
                EventError::ForwardExpired
            );
        }
        assert_eq!(mailbox.cells(), ("empty", "empty"));
        assert_eq!(mailbox.lock().desired.key(), Some(&desired));
        assert_eq!(mailbox.is_closed(), None);
        mailbox.finish_lease(lease).unwrap();
        clock.set(base);
    }

    let current_marker = bootstrapped_mailbox(1, 5);
    current_marker
        .try_enqueue_create(|generation, request_id| {
            Ok::<_, ()>(create_marker(base, generation, request_id))
        })
        .unwrap();
    let lease = current_marker.try_recv().unwrap().unwrap();
    let operation = match lease.record() {
        QueuedCommand::Create(marker) => marker.operation().key().clone(),
        _ => panic!("expected current Create marker"),
    };
    clock.set(create_cutoff - Duration::from_nanos(1));
    let wait = current_marker
        .publish_event_before_using(
            WorkerEvent::CreatePrecommit {
                operation,
                outcome: CreatePrecommitOutcome::NotSubmitted,
                applied_geometry: Some(geometry(80)),
            },
            EventDeadline::Forward(create_cutoff),
            &clock,
        )
        .unwrap();
    current_marker
        .take_event_before_using(&clock)
        .unwrap()
        .unwrap();
    assert_eq!(
        current_marker
            .deposit_ui_reply_before_using(&wait, &clock, |_| Ok(UiReply::PrecommitRestored))
            .unwrap(),
        MutationResult::Applied
    );
    assert!(matches!(
        current_marker.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::PrecommitRestored)
    ));
    current_marker.finish_lease(lease).unwrap();
}

#[test]
fn same_serial_wrong_ui_proof_is_protocol_failure_in_every_current_cell() {
    for stage in 0..3 {
        let base = Instant::now();
        let clock = FakeClock::new(base);
        let mailbox = bootstrapped_mailbox(1, 5);
        let wait = mailbox
            .publish_event_before_using(
                cleanup_event(key(1, 1)),
                EventDeadline::Forward(base + Duration::from_secs(1)),
                &clock,
            )
            .unwrap();
        if stage >= 1 {
            mailbox.take_event_before_using(&clock).unwrap().unwrap();
        }
        if stage == 2 {
            mailbox
                .deposit_ui_reply_before_using(&wait, &clock, |_| {
                    Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
                })
                .unwrap();
        }
        let mut wrong = wait.clone();
        wrong.correlation = UiCorrelation::Attachment(pane_ref(&key(1, 99)));
        assert_eq!(
            mailbox.take_deposited_reply(&wrong).unwrap_err().failure(),
            CarrierFailure::Protocol
        );
    }

    for take_first in [false, true] {
        let base = Instant::now();
        let clock = FakeClock::new(base);
        let mailbox = bootstrapped_mailbox(1, 5);
        let wait = mailbox
            .publish_event_before_using(
                cleanup_event(key(1, 1)),
                EventDeadline::Forward(base + Duration::from_secs(1)),
                &clock,
            )
            .unwrap();
        if take_first {
            mailbox.take_event_before_using(&clock).unwrap().unwrap();
            mailbox
                .deposit_ui_reply_before_using(&wait, &clock, |_| {
                    Ok(UiReply::CleanupApplied(CleanupContinuation::Stable))
                })
                .unwrap();
        }
        let mut wrong = wait.clone();
        wrong.reply_kind = UiReplyKind::Ready;
        let closure_ran = AtomicBool::new(false);
        assert_eq!(
            mailbox
                .deposit_ui_reply_before_using(&wrong, &clock, |_| {
                    closure_ran.store(true, Ordering::SeqCst);
                    Ok(UiReply::Ready(ReadyDecision::Ready))
                })
                .unwrap_err()
                .closed_failure(),
            Some(CarrierFailure::Protocol)
        );
        assert!(!closure_ran.load(Ordering::SeqCst));
    }
}

#[test]
fn stale_after_draw_settlement_is_inert_after_desired_generation_changes() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    mailbox
        .publish_poll_result_before_using(
            poll_key.clone(),
            Some(vec![7]),
            base + Duration::from_secs(1),
            &clock,
        )
        .unwrap();
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    mailbox.lock().attachment_generation = 2;
    mailbox.publish_desired(pane_key(&key(2, 2))).unwrap();
    assert_eq!(mailbox.cells(), ("empty", "empty"));
    assert_eq!(
        mailbox
            .deposit_causal(
                &poll_key,
                CausalSettlement::Feedback(TerminalViewerAction::Poll {
                    attachment_id: fixture_viewer_id(),
                })
            )
            .unwrap(),
        MutationResult::Stale
    );
    assert_eq!(
        mailbox
            .take_causal_before_using(&poll_key, base + Duration::from_secs(1), &clock)
            .unwrap(),
        None
    );
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn empty_poll_requires_explicit_ui_settlement() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    mailbox
        .publish_poll_result_before_using(
            poll_key.clone(),
            None,
            base + Duration::from_secs(1),
            &clock,
        )
        .unwrap();
    let event = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert!(matches!(
        event.into_event(),
        WorkerEvent::PollResult { record: None, .. }
    ));
    assert_eq!(mailbox.cells(), ("empty", "awaiting"));
    assert_eq!(
        mailbox.complete_empty_poll(&poll_key).unwrap(),
        MutationResult::Applied
    );
    assert_eq!(
        mailbox
            .take_causal_before_using(&poll_key, base + Duration::from_secs(1), &clock)
            .unwrap(),
        Some(CausalSettlement::NoFeedback)
    );
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn execution_fence_dynamically_observes_earliest_later_barrier() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    mailbox.try_enqueue(input(&mailbox, 1)).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(mailbox.execution_fence(&lease).barrier(), None);

    let refresh_deadline = base + Duration::from_secs(30);
    assert_eq!(
        mailbox
            .try_enqueue_refresh(refresh_deadline, |_| Ok::<_, ()>(()))
            .unwrap(),
        1
    );
    let fence = mailbox.execution_fence(&lease);
    let barrier = fence.barrier().unwrap();
    assert_eq!(barrier.kind(), BarrierKind::Refresh);
    assert_eq!(barrier.forward_cutoff(), refresh_deadline);
    assert_eq!(barrier.total(), refresh_deadline);
    assert_eq!(
        fence.desired().key().map(|pane| pane.anchor()),
        Some(&key(1, 1))
    );
    assert!(fence.lease_current());
    let waited = mailbox
        .await_execution_change_before_using(&lease, base, &clock)
        .unwrap();
    assert_eq!(waited.barrier(), Some(barrier));
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn startup_and_setup_polls_reserve_fresh_serials_without_replacing_the_parent_lease() {
    let base = Instant::now();
    let startup = bootstrapped_mailbox(1, 5);
    let desired = key(1, 1);
    let first = startup
        .allocate_startup_poll_key(&pane_ref(&desired))
        .unwrap();
    let second = startup
        .allocate_startup_poll_key(&pane_ref(&desired))
        .unwrap();
    assert_eq!(first.serial(), 1);
    assert_eq!(second.serial(), 2);
    assert_eq!(first.attachment(), &desired);
    assert_eq!(startup.cursors(), (1, 0, 5));

    let setup = bootstrapped_mailbox(1, 5);
    setup
        .try_enqueue_create(|generation, request_id| {
            Ok::<_, ()>(create_marker(base, generation, request_id))
        })
        .unwrap();
    let lease = setup.try_recv().unwrap().unwrap();
    let SetupPollKey::Allocated(first) = setup
        .allocate_setup_poll_key(&lease, &pane_ref(&desired))
        .unwrap()
    else {
        panic!("current setup must allocate a Poll key")
    };
    let SetupPollKey::Allocated(second) = setup
        .allocate_setup_poll_key(&lease, &pane_ref(&desired))
        .unwrap()
    else {
        panic!("current setup must allocate a Poll key")
    };
    assert_eq!(first.serial(), lease.serial() + 1);
    assert_eq!(second.serial(), first.serial() + 1);
    assert!(setup.execution_fence(&lease).lease_current());
    setup.finish_lease(lease).unwrap();

    let exhausted = bootstrapped_mailbox(1, 5);
    exhausted.lock().next_command_serial = u64::MAX;
    assert_eq!(
        exhausted
            .allocate_startup_poll_key(&pane_ref(&desired))
            .unwrap_err()
            .failure(),
        CarrierFailure::IdentityExhausted
    );

    let mut canonical = ready_state(2);
    let superseded = mailbox_for_desired(canonical.desired().unwrap().clone(), 5);
    superseded
        .try_enqueue_create(|generation, request_id| {
            Ok::<_, ()>(create_marker(base, generation, request_id))
        })
        .unwrap();
    let parent = superseded.try_recv().unwrap().unwrap();
    superseded
        .try_enqueue_switch(|generation| {
            let next_key =
                AttachmentKey::new(generation, canonical.tab(1).unwrap().tab_id().clone());
            let target = canonical
                .target_for_switch(1, next_key, OperationDeadlines::from_sample(base).unwrap())
                .unwrap();
            assert!(canonical.begin_switch(target.clone()));
            Ok::<_, ()>(SwitchMarker::new(target))
        })
        .unwrap();
    let before = superseded.cursors();
    let SetupPollKey::Superseded(desired) = superseded
        .allocate_setup_poll_key(&parent, &pane_ref(&key(1, 1)))
        .unwrap()
    else {
        panic!("newer desired generation must supersede setup Poll allocation")
    };
    assert_eq!(desired.key(), Some(canonical.desired().unwrap()));
    assert_eq!(superseded.cursors(), before);
    assert_eq!(superseded.is_closed(), None);
    superseded.finish_lease(parent).unwrap();
}

#[test]
fn startup_change_wait_has_a_strict_cutoff_and_wakes_on_close() {
    let base = Instant::now();
    let cutoff = base + Duration::from_secs(1);
    let equality_clock = FakeClock::new(cutoff);
    let equality = bootstrapped_mailbox(1, 5);
    assert_eq!(
        equality
            .await_startup_change_before_using(&pane_ref(&key(1, 1)), cutoff, &equality_clock)
            .unwrap_err()
            .closed_failure(),
        Some(CarrierFailure::Deadline)
    );

    let clock = Arc::new(ObservedClock::new(base));
    let mailbox = Arc::new(bootstrapped_mailbox(1, 5));
    let previous = clock.observation_count();
    let task_mailbox = Arc::clone(&mailbox);
    let task_clock = Arc::clone(&clock);
    let waiter = std::thread::spawn(move || {
        task_mailbox.await_startup_change_before_using(
            &pane_ref(&key(1, 1)),
            base + Duration::from_secs(60),
            task_clock.as_ref(),
        )
    });
    clock.wait_for_observation_after(previous);
    mailbox.close(CarrierFailure::Transport);
    assert_eq!(
        waiter.join().unwrap().unwrap_err().closed_failure(),
        Some(CarrierFailure::Transport)
    );
}

#[test]
fn busy_backoffs_ignore_ordinary_and_spurious_notifications_until_the_fixed_cap() {
    let base = Instant::now();
    let cutoff = base + Duration::from_secs(1);

    let execution_clock = Arc::new(ObservedClock::new(base));
    let execution_mailbox = Arc::new(bootstrapped_mailbox(1, 5));
    execution_mailbox
        .try_enqueue(input(&execution_mailbox, 1))
        .unwrap();
    let lease = execution_mailbox.try_recv().unwrap().unwrap();
    let previous = execution_clock.observation_count();
    std::thread::scope(|scope| {
        let (finished_tx, finished_rx) = std::sync::mpsc::sync_channel(0);
        let task_mailbox = &execution_mailbox;
        let task_clock = &execution_clock;
        let task_lease = &lease;
        let waiter = scope.spawn(move || {
            let result = task_mailbox.await_execution_change_before_using(
                task_lease,
                cutoff,
                task_clock.as_ref(),
            );
            finished_tx.send(result).unwrap();
        });
        execution_clock.wait_for_observation_after(previous);
        execution_mailbox
            .try_enqueue(input(&execution_mailbox, 2))
            .unwrap();
        assert!(matches!(
            finished_rx.recv_timeout(Duration::from_millis(25)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        execution_clock.set(base + CONDITION_WAIT_SLICE);
        execution_mailbox.wake_waiters();
        let fence = finished_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();
        assert!(fence.lease_current());
        assert_eq!(fence.barrier(), None);
        waiter.join().unwrap();
    });
    execution_mailbox.finish_lease(lease).unwrap();

    let startup_clock = Arc::new(ObservedClock::new(base));
    let startup_mailbox = Arc::new(bootstrapped_mailbox(1, 5));
    let previous = startup_clock.observation_count();
    std::thread::scope(|scope| {
        let (finished_tx, finished_rx) = std::sync::mpsc::sync_channel(0);
        let task_mailbox = &startup_mailbox;
        let task_clock = &startup_clock;
        let waiter = scope.spawn(move || {
            let result = task_mailbox.await_startup_change_before_using(
                &pane_ref(&key(1, 1)),
                cutoff,
                task_clock.as_ref(),
            );
            finished_tx.send(result).unwrap();
        });
        startup_clock.wait_for_observation_after(previous);
        startup_mailbox.wake_waiters();
        assert!(matches!(
            finished_rx.recv_timeout(Duration::from_millis(25)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        startup_clock.set(base + CONDITION_WAIT_SLICE);
        startup_mailbox.wake_waiters();
        assert_eq!(
            finished_rx
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap()
                .key()
                .map(|pane| pane.anchor()),
            Some(&key(1, 1))
        );
        waiter.join().unwrap();
    });
}

#[test]
fn refresh_generation_is_single_pending_and_moves_through_three_exact_stages() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(1);
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    assert_eq!(
        mailbox
            .try_enqueue_refresh(deadline, |_| Ok::<_, ()>(()))
            .unwrap(),
        1
    );
    assert_eq!(
        mailbox.pending_refresh_stage(),
        Some((1, PendingRefreshStage::Queued))
    );
    assert!(matches!(
        mailbox.try_enqueue_refresh(deadline, |_| Ok::<_, ()>(())),
        Err(AdmissionError::Full)
    ));
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_refresh_in_flight(1).unwrap(),
        MutationResult::Applied
    );
    assert_eq!(
        mailbox.pending_refresh_stage(),
        Some((1, PendingRefreshStage::InFlight))
    );
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::RefreshSnapshotResult {
                generation: 1,
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
            EventDeadline::PlacementOnly(deadline),
            &clock,
        )
        .unwrap();
    assert_eq!(
        mailbox.pending_refresh_stage(),
        Some((1, PendingRefreshStage::AwaitingUi))
    );
    clock.set(deadline + Duration::from_secs(1));
    mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                Ok(UiReply::Refresh(RefreshSettlement::new(
                    RefreshDecision::SettledWithoutAttachment,
                    pane_key(&key(1, 1)),
                )))
            })
            .unwrap(),
        MutationResult::Applied
    );
    assert_eq!(mailbox.pending_refresh_stage(), None);
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Refresh(settlement))
            if settlement.decision() == &RefreshDecision::SettledWithoutAttachment
                && settlement.desired().map(|pane| pane.anchor()) == Some(&key(1, 1))
    ));
    mailbox.finish_lease(lease).unwrap();

    {
        let mut state = mailbox.lock();
        state.refresh_generation = u64::MAX;
    }
    let before = mailbox.cursors();
    assert!(matches!(
        mailbox.try_enqueue_refresh(deadline, |_| Ok::<_, ()>(())),
        Err(AdmissionError::RefreshGenerationExhausted)
    ));
    assert_eq!(mailbox.cursors(), before);
}

#[test]
fn deposited_refresh_reply_can_be_superseded_before_worker_reacquires_mailbox() {
    for reply_first in [false, true] {
        for bounded in [false, true] {
            assert_refresh_reply_resume_window(reply_first, bounded);
        }
    }
}

fn assert_refresh_reply_resume_window(reply_first: bool, bounded: bool) {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(50);
    let clock = FakeClock::new(base);
    let mut session = ready_state(2);
    let original = session.desired().unwrap().clone();
    let mailbox = mailbox_for_desired(original.clone(), 5);
    let refresh_generation = mailbox
        .try_enqueue_refresh(deadline, |generation| {
            assert!(session.start_pending_refresh(generation));
            Ok::<_, ()>(())
        })
        .unwrap();
    let refresh_lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_refresh_in_flight(refresh_generation).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::RefreshSnapshotResult {
                generation: refresh_generation,
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
            EventDeadline::PlacementOnly(deadline),
            &clock,
        )
        .unwrap();
    let event = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert!(matches!(
        event.into_event(),
        WorkerEvent::RefreshSnapshotResult {
            generation,
            outcome: WorkbenchSnapshotOutcome::Denied {},
        } if generation == refresh_generation
    ));
    let prepared = session.prepare_refresh(refresh_generation, WorkbenchSnapshotOutcome::Denied {});
    let checkpoint = mailbox.pause_next_ui_reply_resume_for_test();
    std::thread::scope(|scope| {
        let spawn_worker = || {
            scope.spawn(|| {
                let reply = if bounded {
                    mailbox
                        .await_ui_reply_before_using(&wait, deadline, &clock)
                        .unwrap()
                } else {
                    mailbox.await_ui_reply(&wait).unwrap()
                };
                (reply, mailbox.execution_fence(&refresh_lease))
            })
        };
        let worker = if reply_first {
            None
        } else {
            Some(spawn_worker())
        };
        if !reply_first {
            assert!(mailbox.await_condition_waiter_before(Instant::now() + Duration::from_secs(2)));
        }
        assert_eq!(
            mailbox
                .deposit_ui_reply_before_using(&wait, &clock, |_| {
                    let decision =
                        session.apply_prepared_refresh(prepared, std::time::Instant::now());
                    assert_eq!(decision, RefreshDecision::Continue);
                    Ok(UiReply::Refresh(RefreshSettlement::new(
                        decision,
                        session.desired().unwrap().clone(),
                    )))
                })
                .unwrap(),
            MutationResult::Applied
        );
        assert_eq!(mailbox.pending_refresh_stage(), None);
        let worker = worker.unwrap_or_else(spawn_worker);
        assert!(checkpoint.await_reached_before(Instant::now() + Duration::from_secs(2)));
        let admitted = mailbox
            .try_enqueue_switch_for(
                &mut session,
                |session, generation| {
                    let target_key =
                        AttachmentKey::new(generation, session.tab(1).unwrap().tab_id().clone());
                    let target = session
                        .target_for_switch(
                            1,
                            target_key,
                            OperationDeadlines::from_sample(base).unwrap(),
                        )
                        .unwrap();
                    assert!(session.begin_switch(target.clone()));
                    Ok::<_, ()>(SwitchMarker::new(target))
                },
                |session, result, facts| {
                    assert!(result.is_ok());
                    session.finish_switch_membership_admission(
                        facts.discarded(),
                        facts.inherited(),
                        facts.authorized_cleanup(),
                    );
                },
            )
            .unwrap();
        assert_eq!(admitted, original.generation() + 1);
        drop(checkpoint);

        let (reply, fence) = worker.join().unwrap();
        assert!(matches!(
            reply,
            Some(UiReply::Refresh(settlement))
                if settlement.decision() == &RefreshDecision::Continue
                    && settlement.desired() == Some(&original)
        ));
        assert_eq!(fence.desired().key(), Some(session.desired().unwrap()));
        assert_eq!(fence.barrier().unwrap().kind(), BarrierKind::Switch);
        assert!(fence.lease_current());
    });
    mailbox.finish_lease(refresh_lease).unwrap();
    let switch = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(switch.record(), QueuedCommand::Switch(_)));
    mailbox.finish_lease(switch).unwrap();
}

#[test]
fn pending_refresh_blocks_retryable_selection_at_all_three_stages_until_ui_settlement() {
    let base = Instant::now();
    let deadline = base + Duration::from_secs(1);
    let clock = FakeClock::new(base);
    let mut session = retryable_two_tab_state(base);
    let desired = session.desired().unwrap().clone();
    let active_position = u8::try_from(session.active().unwrap().index() + 1).unwrap();
    let different_position = if active_position == 1 { 2 } else { 1 };
    let mailbox = mailbox_for_desired(desired.clone(), 5);

    let generation = mailbox
        .try_enqueue_refresh(deadline, |generation| {
            assert!(session.start_pending_refresh(generation));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_refresh_selection_blocked(
        &session,
        &mailbox,
        generation,
        PendingRefreshStage::Queued,
        active_position,
        different_position,
    );

    let lease = mailbox.try_recv().unwrap().unwrap();
    assert_eq!(
        mailbox.mark_refresh_in_flight(generation).unwrap(),
        MutationResult::Applied
    );
    assert_refresh_selection_blocked(
        &session,
        &mailbox,
        generation,
        PendingRefreshStage::InFlight,
        active_position,
        different_position,
    );

    let wait = mailbox
        .publish_event_before_using(
            WorkerEvent::RefreshSnapshotResult {
                generation,
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
            EventDeadline::PlacementOnly(deadline),
            &clock,
        )
        .unwrap();
    assert_refresh_selection_blocked(
        &session,
        &mailbox,
        generation,
        PendingRefreshStage::AwaitingUi,
        active_position,
        different_position,
    );
    let event = mailbox.take_event_before_using(&clock).unwrap().unwrap();
    assert!(matches!(
        event.into_event(),
        WorkerEvent::RefreshSnapshotResult {
            generation: current,
            outcome: WorkbenchSnapshotOutcome::Denied {},
        } if current == generation
    ));
    let prepared = session.prepare_refresh(generation, WorkbenchSnapshotOutcome::Denied {});
    assert_eq!(
        mailbox
            .deposit_ui_reply_before_using(&wait, &clock, |_| {
                let decision = session.apply_prepared_refresh(prepared, std::time::Instant::now());
                Ok(UiReply::Refresh(RefreshSettlement::new(
                    decision,
                    session.desired().unwrap().clone(),
                )))
            })
            .unwrap(),
        MutationResult::Applied
    );
    assert_eq!(session.pending_refresh_generation(), None);
    assert_eq!(mailbox.pending_refresh_stage(), None);
    assert_eq!(
        session.plan_selection(ClientAction::SelectTab(active_position)),
        SelectionPlan::Switch {
            index: usize::from(active_position - 1),
        }
    );
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Refresh(settlement))
            if settlement.decision() == &RefreshDecision::Continue
                && settlement.desired() == Some(&desired)
    ));
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn same_geometry_observation_retries_after_full_fifo_without_clearing_precommit_notice() {
    let base = Instant::now();
    let mut session = ready_state(1);
    let desired = session.desired().unwrap().clone();
    let operation = create_marker(base, 2, WorkbenchRequestId::parse(&id(90)).unwrap())
        .operation()
        .clone();
    let operation_key = operation.key().clone();
    assert!(session.begin_create(operation));
    assert!(session.restore_precommit(&operation_key, Some(geometry(80))));
    assert_eq!(session.notice(), Some(SessionNotice::CreateNotCreated));

    let mailbox = mailbox_for_desired(desired.clone(), 5);
    for byte in 0..COMMAND_CAPACITY {
        mailbox
            .try_enqueue(QueuedCommand::Input(Tagged::new(
                pane_ref(desired.anchor()),
                ControlInput::new(vec![u8::try_from(byte).unwrap()]).unwrap(),
            )))
            .unwrap();
    }
    let resized = geometry(91);
    session.observe_geometry(resized);
    mailbox.observe_geometry(resized).unwrap();
    assert_eq!(mailbox.latest_geometry().unwrap(), resized);
    assert!(matches!(
        mailbox.try_enqueue(QueuedCommand::Resize(Tagged::new(
            pane_ref(desired.anchor()),
            resized,
        ))),
        Err(AdmissionError::Full)
    ));
    assert_eq!(session.last_admitted_geometry_t(&desired), None);
    assert_eq!(session.notice(), Some(SessionNotice::CreateNotCreated));

    let first = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(first).unwrap();
    session.observe_geometry(resized);
    mailbox.observe_geometry(resized).unwrap();
    assert_ne!(session.last_admitted_geometry_t(&desired), Some(resized));
    mailbox
        .try_enqueue(QueuedCommand::Resize(Tagged::new(
            pane_ref(desired.anchor()),
            resized,
        )))
        .unwrap();
    assert!(session.record_admitted_geometry_t(&desired, resized));

    for _ in 1..COMMAND_CAPACITY {
        let lease = mailbox.try_recv().unwrap().unwrap();
        assert!(matches!(lease.record(), QueuedCommand::Input(_)));
        mailbox.finish_lease(lease).unwrap();
    }
    let resize = mailbox.try_recv().unwrap().unwrap();
    assert!(matches!(
        resize.record(),
        QueuedCommand::Resize(tagged)
            if tagged.key() == &desired && *tagged.value() == resized
    ));
    mailbox.finish_lease(resize).unwrap();
    assert_eq!(session.last_admitted_geometry_t(&desired), Some(resized));
    assert_eq!(session.notice(), Some(SessionNotice::CreateNotCreated));
}

#[test]
fn completed_client_action_updates_notice_inside_the_admission_lock_on_success_and_refusal() {
    let base = Instant::now();
    let mut session = ready_state(1);
    session.set_notice(SessionNotice::CreateNotCreated);
    let desired = session.desired().unwrap().clone();
    let mailbox = mailbox_for_desired(desired.clone(), 5);
    for byte in 0..COMMAND_CAPACITY {
        mailbox
            .try_enqueue(input(&mailbox, u8::try_from(byte).unwrap()))
            .unwrap();
    }

    let build_ran = AtomicBool::new(false);
    assert!(matches!(
        mailbox.try_enqueue_create_for(
            &mut session,
            |_session, _generation, _request_id| -> Result<CreateMarker, &'static str> {
                build_ran.store(true, Ordering::SeqCst);
                Err("must not build while full")
            },
            |session, result| {
                assert!(matches!(result, Err(AdmissionError::Full)));
                session.set_notice(SessionNotice::LocalBusy);
            },
        ),
        Err(AdmissionError::Full)
    ));
    assert!(!build_ran.load(Ordering::SeqCst));
    assert_eq!(session.notice(), Some(SessionNotice::LocalBusy));

    let first = mailbox.try_recv().unwrap().unwrap();
    mailbox.finish_lease(first).unwrap();
    let admitted = mailbox
        .try_enqueue_create_for(
            &mut session,
            |session, generation, request_id| {
                let marker = create_marker(base, generation, request_id);
                if !session.begin_create(marker.operation().clone()) {
                    return Err("state rejected create");
                }
                Ok(marker)
            },
            |session, result| {
                if result.is_ok() {
                    session.clear_notice();
                } else {
                    session.set_notice(SessionNotice::LocalBusy);
                }
            },
        )
        .unwrap();
    assert_eq!(session.phase_kind(), SessionPhaseKind::Creating);
    assert_eq!(session.notice(), None);
    assert_eq!(admitted.reserved_generation(), 2);

    session.set_notice(SessionNotice::CreateNotCreated);
    mailbox.apply_local_for(&mut session, NativeSessionState::clear_notice);
    assert_eq!(session.notice(), None);
}

#[test]
fn close_is_idempotent_preserves_first_reason_and_clears_all_bounded_storage() {
    let base = Instant::now();
    let clock = FakeClock::new(base);
    let mailbox = bootstrapped_mailbox(1, 5);
    mailbox.try_enqueue(poll(&mailbox)).unwrap();
    let lease = mailbox.try_recv().unwrap().unwrap();
    let poll_key = lease.poll_key().unwrap();
    mailbox
        .publish_poll_result_before_using(
            poll_key,
            Some(vec![1]),
            base + Duration::from_secs(1),
            &clock,
        )
        .unwrap();
    assert_eq!(
        mailbox.close(CarrierFailure::OutcomeUnknown).failure(),
        CarrierFailure::OutcomeUnknown
    );
    assert_eq!(
        mailbox.close(CarrierFailure::Transport).failure(),
        CarrierFailure::OutcomeUnknown
    );
    assert_eq!(mailbox.cells(), ("empty", "empty"));
    assert!(matches!(
        mailbox.try_recv(),
        Err(MailboxClosed {
            failure: CarrierFailure::OutcomeUnknown
        })
    ));
}

#[test]
fn publication_hint_latch_survives_stale_publish_and_captures_once() {
    let mailbox = bootstrapped_mailbox(7, 9);
    let clock = FakeClock::new(Instant::now());
    let cutoff = Instant::now() + Duration::from_secs(10);

    mailbox.try_enqueue(poll(&mailbox)).expect("Poll admission");
    let lease = mailbox.try_recv().unwrap().expect("queued Poll");
    let poll_key = lease.poll_key().expect("Poll lease carries a poll key");

    // The desired moved after the poll leased: the publish is Stale, but the
    // latch was registered before the stale branch and must survive it.
    mailbox.lock().desired = MailboxMembership::Live {
        desired: Some(pane_key(&key(8, 1))),
        panes: vec![pane_ref(&key(8, 1))],
    };
    assert_eq!(
        mailbox
            .publish_poll_result_hint_before_using(poll_key.clone(), None, true, cutoff, &clock)
            .expect("stale publish"),
        MutationResult::Stale
    );
    let mut canonical = ready_state(1);
    assert!(
        mailbox
            .capture_publication_hint_for(&mut canonical, NativeSessionState::note_publication_hint)
            .expect("capture")
    );
    assert!(canonical.publication_debt_pending());
    // The latch is consumed by the first capture.
    assert!(
        !mailbox
            .capture_publication_hint_for(&mut canonical, NativeSessionState::note_publication_hint)
            .expect("capture")
    );
    mailbox.finish_lease(lease).expect("lease finish");
}

#[test]
fn publication_hint_latch_survives_event_and_causal_cleanup() {
    let mailbox = bootstrapped_mailbox(7, 9);
    let clock = FakeClock::new(Instant::now());
    let cutoff = Instant::now() + Duration::from_secs(10);

    mailbox.try_enqueue(poll(&mailbox)).expect("Poll admission");
    let lease = mailbox.try_recv().unwrap().expect("queued Poll");
    let poll_key = lease.poll_key().expect("Poll lease carries a poll key");
    assert_eq!(
        mailbox
            .publish_poll_result_hint_before_using(poll_key.clone(), None, true, cutoff, &clock)
            .expect("publish"),
        MutationResult::Applied
    );

    // Event selection, causal deposit, and causal take all run their cleanup
    // paths; none may clear the ingress latch.
    let taken = mailbox
        .take_event_before_using(&clock)
        .expect("event take")
        .expect("PollResult event");
    drop(taken);
    assert_eq!(
        mailbox
            .deposit_causal(&poll_key, CausalSettlement::NoFeedback)
            .expect("causal deposit"),
        MutationResult::Applied
    );
    assert!(matches!(
        mailbox
            .take_causal_before_using(&poll_key, cutoff, &clock)
            .expect("causal take"),
        Some(CausalSettlement::NoFeedback)
    ));
    mailbox.finish_lease(lease).expect("lease finish");

    let mut canonical = ready_state(1);
    assert!(
        mailbox
            .capture_publication_hint_for(&mut canonical, NativeSessionState::note_publication_hint)
            .expect("capture")
    );
    assert!(canonical.publication_debt_pending());
}

#[test]
fn second_ordinary_poll_admission_is_busy_while_queued_or_leased() {
    let mailbox = bootstrapped_mailbox(7, 9);
    let cursors = mailbox.cursors();

    let first = mailbox.try_enqueue(poll(&mailbox)).expect("first Poll");
    // A second ordinary Poll while the first is queued is a local no-op, not a
    // retirement condition, and burns no cursor.
    assert_eq!(
        mailbox.try_enqueue(poll(&mailbox)),
        Err(AdmissionError::Busy)
    );
    assert_eq!(mailbox.cursors(), cursors);
    let lease = mailbox.try_recv().unwrap().expect("queued Poll");
    // Still Busy while the first poll executes.
    assert_eq!(
        mailbox.try_enqueue(poll(&mailbox)),
        Err(AdmissionError::Busy)
    );
    assert!(mailbox.ordinary_poll_continues(first).expect("continues"));
    mailbox.finish_lease(lease).expect("lease finish");
    // Once the obligation settles, a fresh ordinary Poll is admissible again.
    let second = mailbox.try_enqueue(poll(&mailbox)).expect("second Poll");
    assert_ne!(first, second);
}

#[test]
fn ordinary_poll_continues_tracks_exact_serial_until_finish() {
    let mailbox = bootstrapped_mailbox(7, 9);
    let serial = mailbox.try_enqueue(poll(&mailbox)).expect("Poll admission");
    assert!(mailbox.ordinary_poll_continues(serial).expect("queued"));
    assert!(
        !mailbox
            .ordinary_poll_continues(serial + 1)
            .expect("other serial")
    );
    let lease = mailbox.try_recv().unwrap().expect("queued Poll");
    assert!(mailbox.ordinary_poll_continues(serial).expect("leased"));
    mailbox.finish_lease(lease).expect("lease finish");
    assert!(!mailbox.ordinary_poll_continues(serial).expect("finished"));
}

#[test]
fn setup_poll_key_is_not_an_ordinary_poll_obligation() {
    let mailbox = bootstrapped_mailbox(7, 9);
    let deadline = Instant::now() + Duration::from_secs(10);
    mailbox
        .try_enqueue_refresh(deadline, |_generation| Ok::<_, ()>(()))
        .expect("refresh admission");
    let lease = mailbox.try_recv().unwrap().expect("queued Refresh");
    let desired = mailbox.lock().desired.focus_pane_ref().unwrap();
    let SetupPollKey::Allocated(setup_key) = mailbox
        .allocate_setup_poll_key(&lease, &desired)
        .expect("setup poll key")
    else {
        panic!("desired still current")
    };
    // The setup key shares a serial lane but is never an ordinary Poll
    // obligation, so an ordinary Poll remains admissible while it exists.
    assert!(
        !mailbox
            .ordinary_poll_continues(setup_key.serial())
            .expect("setup serial")
    );
    assert!(mailbox.try_enqueue(poll(&mailbox)).is_ok());
    mailbox.finish_lease(lease).expect("lease finish");
}

#[test]
fn peer_resize_requires_exact_membership_and_does_not_open_peer_input() {
    let mailbox = bootstrapped_mailbox(1, 5);
    let focus = mailbox.lock().desired.key().unwrap().clone();
    let peer = PaneRef::new(
        PaneKey::new(
            focus.anchor().clone(),
            WorkbenchPanelId::parse(&id(91)).unwrap(),
        ),
        2,
    );
    mailbox.lock().desired = MailboxMembership::Live {
        desired: Some(focus.clone()),
        panes: vec![PaneRef::new(focus.clone(), 1), peer.clone()],
    };
    assert!(matches!(
        mailbox.try_enqueue(QueuedCommand::Input(Tagged::new(
            peer.clone(),
            ControlInput::new(vec![b'x']).unwrap()
        ))),
        Err(AdmissionError::StaleAttachment)
    ));
    let stale = PaneRef::new(peer.pane().clone(), 1);
    assert!(matches!(
        mailbox.try_enqueue(QueuedCommand::Resize(Tagged::new(stale, geometry(64)))),
        Err(AdmissionError::StaleAttachment)
    ));
    mailbox
        .try_enqueue(QueuedCommand::Resize(Tagged::new(
            peer.clone(),
            geometry(64),
        )))
        .unwrap();
    assert_eq!(mailbox.lock().desired.key(), Some(&focus));
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(
        matches!(lease.record(), QueuedCommand::Resize(tagged) if tagged.key() == &peer && *tagged.value() == geometry(64))
    );
    mailbox.finish_lease(lease).unwrap();
}

fn bootstrapped_mailbox(generation: u64, identity: u128) -> CommandMailbox {
    let mailbox = CommandMailbox::new(identity, Geometry::new(80, 24, 8, 16).unwrap());
    {
        let mut state = mailbox.lock();
        state.attachment_generation = generation;
        state.desired = MailboxMembership::Live {
            desired: Some(pane_key(&key(generation, 1))),
            panes: vec![pane_ref(&key(generation, 1))],
        };
    }
    mailbox
}

fn mailbox_for_desired(desired: PaneKey, identity: u128) -> CommandMailbox {
    let mailbox = CommandMailbox::new(identity, geometry(80));
    {
        let mut state = mailbox.lock();
        state.attachment_generation = desired.anchor().generation();
        state.desired = MailboxMembership::Live {
            desired: Some(desired.clone()),
            panes: vec![PaneRef::new(desired, 1)],
        };
    }
    mailbox
}

fn ready_state(tab_count: usize) -> NativeSessionState {
    let (mut state, _target) =
        NativeSessionState::starting(projected_tabs(tab_count), 1, geometry(80)).unwrap();
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

fn retryable_two_tab_state(base: Instant) -> NativeSessionState {
    let mut state = ready_state(2);
    let target_key = AttachmentKey::new(2, state.tab(1).unwrap().tab_id().clone());
    let target = state
        .target_for_switch(
            1,
            target_key.clone(),
            OperationDeadlines::from_sample(base).unwrap(),
        )
        .unwrap();
    assert!(state.begin_switch(target.clone()));
    let prepared = state
        .prepare_membership_candidate(
            state.cache().clone(),
            Some(target_key.tab_id()),
            Some(1),
            None,
            None,
        )
        .unwrap();
    let cleanup = state
        .stage_switch_membership_cleanup_k(&target_key, prepared, target.deadlines())
        .unwrap();
    let prepared = state
        .prepare_cleaned_membership_candidate(&cleanup, cleanup.old())
        .unwrap();
    let target_key = AttachmentKey::new(3, target_key.tab_id().clone());
    assert!(matches!(
        state
            .prepare_cleaned_membership_install(&cleanup, prepared, Some(target_key.clone()))
            .unwrap()
            .commit(),
        crate::terminal_session::state::MembershipInstallDecision::Attach(_)
    ));
    assert_eq!(
        state.settle_unattached_setup_t(&target_key, 1, &CreateTerminalProvenance::BindingOnly,),
        Some(CleanupContinuation::Stable)
    );
    assert_eq!(
        state.phase_kind(),
        SessionPhaseKind::RetryableAttachUnavailable
    );
    state
}

fn assert_refresh_selection_blocked(
    session: &NativeSessionState,
    mailbox: &CommandMailbox,
    generation: u64,
    stage: PendingRefreshStage,
    active_position: u8,
    different_position: u8,
) {
    assert_eq!(mailbox.pending_refresh_stage(), Some((generation, stage)));
    assert_eq!(
        session.plan_selection(ClientAction::SelectTab(active_position)),
        SelectionPlan::NoOp
    );
    assert_eq!(
        session.plan_selection(ClientAction::SelectTab(different_position)),
        SelectionPlan::Busy
    );
}

fn poll(mailbox: &CommandMailbox) -> QueuedCommand {
    QueuedCommand::Poll(Tagged::new(
        mailbox.lock().desired.focus_pane_ref().unwrap(),
        (),
    ))
}

fn input(mailbox: &CommandMailbox, byte: u8) -> QueuedCommand {
    QueuedCommand::Input(Tagged::new(
        mailbox.lock().desired.focus_pane_ref().unwrap(),
        ControlInput::new(vec![byte]).unwrap(),
    ))
}

fn cleanup_event(key: AttachmentKey) -> WorkerEvent {
    WorkerEvent::CleanupSettled {
        key: pane_ref(&key),
        target_index: 0,
        target_provenance: CreateTerminalProvenance::BindingOnly,
    }
}

fn create_marker(base: Instant, generation: u64, request_id: WorkbenchRequestId) -> CreateMarker {
    let site_id = SiteId::parse(&id(2)).unwrap();
    let context_id = WorkbenchTerminalContextId::parse(&id(3)).unwrap();
    let label = WorkbenchLabel::parse("2").unwrap();
    let workbench_geometry = WorkbenchTerminalGeometry::new(80, 24, 8, 16).unwrap();
    let command = WorkbenchCommand {
        protocol_version: ProtocolVersion,
        request_id: request_id.clone(),
        site_id: site_id.clone(),
        expected_revision: WorkbenchRevision::new(1),
        action: WorkbenchAction::TabCreate {
            terminal_context_id: context_id.clone(),
            label: label.clone(),
            geometry: workbench_geometry,
            launch: None,
            profile: None,
            options: Default::default(),
        },
    };
    let request = WorkbenchTabCreateRequest::try_from(command).unwrap();
    let operation = CreateOperation::new(
        CreateOperationKey::new(request_id, generation),
        site_id,
        WorkbenchRevision::new(1),
        context_id,
        label,
        Geometry::new(80, 24, 8, 16).unwrap(),
        OperationDeadlines::from_sample(base).unwrap(),
    );
    CreateMarker::new(request, operation)
}

fn key(generation: u64, tab: u128) -> AttachmentKey {
    AttachmentKey::new(generation, WorkbenchTabId::parse(&id(tab)).unwrap())
}

/// Mailbox fixtures fabricate one panel identity per anchor — the mailbox only
/// needs a consistent logical pane, never a real workbench panel. `id(5)`
/// matches the `projected_tabs` fixture's first leaf so mailbox-seeded
/// desired keys agree with state fixtures built on it.
fn pane_key(anchor: &AttachmentKey) -> PaneKey {
    PaneKey::new(anchor.clone(), WorkbenchPanelId::parse(&id(5)).unwrap())
}

fn pane_ref(anchor: &AttachmentKey) -> PaneRef {
    PaneRef::new(pane_key(anchor), 1)
}
/// The fixture panel identity used when a `DesiredInstall` needs one —
/// matches `projected_tabs`' first leaf.
fn panel() -> WorkbenchPanelId {
    WorkbenchPanelId::parse(&id(5)).unwrap()
}

/// The `PaneKey`/`PaneRef` a real state minted for `anchor` — preferred over
/// the fabricated helpers whenever a session exists so panel identity matches
/// the actual workbench leaf.
fn projected_one_tab() -> NativeContextTabs {
    projected_tabs(1)
}

fn projected_tabs(tab_count: usize) -> NativeContextTabs {
    project_native_standalone_tabs(&Snapshot {
        navigation: Navigation {
            projects: Vec::new(),
            standalone_terminal_context_id: WorkbenchTerminalContextId::parse(&id(3)).unwrap(),
        },
        protocol_version: ProtocolVersion,
        revision: WorkbenchRevision::new(1),
        site_id: SiteId::parse(&id(2)).unwrap(),
        terminal_contexts: vec![TerminalContext {
            groups: Vec::new(),
            tabs: (0..tab_count)
                .map(|index| {
                    let offset = u128::try_from(index).unwrap() * 10;
                    let label = (index + 1).to_string();
                    Tab {
                        group_id: None,
                        label: label.clone(),
                        pinned: false,
                        root: PanelNode::Panel {
                            right_click: Default::default(),
                            child_presentations: Vec::new(),
                            panel_id: WorkbenchPanelId::parse(&id(5 + offset)).unwrap(),
                            primary_presentation: Presentation {
                                custom_label: None,
                                binding: Some(PresentationBinding {
                                    terminal_id: TerminalId::parse(&id(7 + offset)).unwrap(),
                                    worker_id: WorkerId::parse(&id(6 + offset)).unwrap(),
                                }),
                                label,
                                presentation_id: TerminalPresentationId::parse(&id(4 + offset))
                                    .unwrap(),
                            },
                        },
                        tab_id: WorkbenchTabId::parse(&id(1 + offset)).unwrap(),
                    }
                })
                .collect(),
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(3)).unwrap(),
        }],
    })
    .unwrap()
}

fn id(value: u128) -> String {
    format!("{value:032x}")
}

fn geometry(columns: u16) -> Geometry {
    Geometry::new(columns, 24, 8, 16).unwrap()
}
