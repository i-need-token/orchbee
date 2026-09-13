use super::membership_tests::snapshot;
use super::move_membership_tests::ready_ui;
use super::*;
use orchbee_workbench_protocol::WorkbenchRevision;

/// Runs one ordinary poll exactly the way the executor does: the queued Poll is
/// leased, its result is published carrying the hint the publication probe
/// observed, the UI applies it, and the causal settlement the UI deposits is
/// taken so the next poll can publish.
fn run_poll(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    key: &AttachmentKey,
    publication_hint: bool,
) {
    mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(key.clone(), ())))
        .expect("Poll admission");
    let lease = mailbox.try_recv().unwrap().expect("queued Poll");
    let poll_key = lease.poll_key().expect("Poll lease carries a poll key");
    let cutoff = clock.now() + crate::terminal_session::state::OPERATION_TOTAL;
    assert_eq!(
        mailbox
            .publish_poll_result_before(poll_key.clone(), None, publication_hint, cutoff, clock)
            .expect("PollResult publication"),
        MutationResult::Applied
    );
    ui.tick();
    assert!(matches!(
        mailbox
            .take_causal_before(&poll_key, cutoff, clock)
            .expect("causal take"),
        Some(CausalSettlement::NoFeedback)
    ));
    mailbox.finish_lease(lease).expect("lease finish");
}

/// Receives the queued Refresh the debt drain admitted, publishes its snapshot
/// outcome, and lets the UI settle the event and release the lease.
fn settle_queued_refresh(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    outcome: orchbee_workbench_protocol::WorkbenchSnapshotOutcome,
) {
    let lease = mailbox.try_recv().unwrap().expect("Refresh queued");
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one Refresh marker");
    };
    assert_eq!(
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_refresh_result_before(marker.generation(), outcome, marker.deadline(), clock)
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
}

/// A hint arriving while a local draft blocks refresh admission is captured into
/// canonical debt: the refused drain stays pending, and the first fair tick after
/// the draft closes installs the folded publication without a new hint.
#[test]
fn busy_refused_publication_hint_survives_until_admission_reopens() {
    let (mut ui, mailbox, clock, _elapsed) = ready_ui();
    let key = ui.state.as_ref().unwrap().desired().unwrap().clone();

    // A local rename draft makes phase_allows_refresh() false, so the drain
    // reducer reports Busy and the debt must stay pending.
    assert!(ui.state.as_mut().unwrap().open_rename_draft().is_some());

    run_poll(&mut ui, &mailbox, &clock, &key, true);

    // The hint is captured into debt even though refresh admission was refused.
    assert_eq!(mailbox.pending_refresh_stage(), None);
    assert_eq!(
        ui.state.as_ref().unwrap().pending_refresh_generation(),
        None
    );
    assert!(ui.state.as_ref().unwrap().publication_debt_pending());
    assert!(mailbox.is_closed().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());

    // Cancelling the draft reopens admission; the same fair tick drains the debt.
    ui.state.as_mut().unwrap().cancel_rename_draft();
    assert!(ui.state.as_ref().unwrap().phase_allows_refresh());
    ui.tick();
    assert!(mailbox.pending_refresh_stage().is_some());
    assert!(!ui.state.as_ref().unwrap().publication_debt_pending());

    // The refresh installs the snapshot carrying the folded publication.
    settle_queued_refresh(
        &mut ui,
        &mailbox,
        &clock,
        orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(2, 2),
        },
    );
    assert_eq!(
        ui.state.as_ref().unwrap().cache().revision(),
        WorkbenchRevision::new(2)
    );

    // A follow-up probe against the same revision reports no new hint, and no
    // further refresh demand remains.
    run_poll(&mut ui, &mailbox, &clock, &key, false);
    assert_eq!(mailbox.pending_refresh_stage(), None);
    assert!(!ui.state.as_ref().unwrap().publication_debt_pending());
    assert_eq!(
        ui.state.as_ref().unwrap().cache().revision(),
        WorkbenchRevision::new(2)
    );
}

/// A hint arriving while the command queue is at capacity is captured into debt
/// instead of being dropped: once queued commands drain, a later fair tick admits
/// the owed refresh and the folded publication installs.
#[test]
fn full_refused_publication_hint_survives_until_queue_drains() {
    let (mut ui, mailbox, clock, _elapsed) = ready_ui();
    let key = ui.state.as_ref().unwrap().desired().unwrap().clone();

    // The executor holds the poll lease while the UI keeps admitting commands,
    // so the queue is at capacity when the latch is captured.
    mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(key.clone(), ())))
        .expect("Poll admission");
    let poll_lease = mailbox.try_recv().unwrap().expect("queued Poll");
    let poll_key = poll_lease
        .poll_key()
        .expect("Poll lease carries a poll key");
    for _ in 0..8 {
        let input = ControlInput::new(b"x".to_vec()).unwrap();
        mailbox
            .try_enqueue(QueuedCommand::Input(Tagged::new(key.clone(), input)))
            .expect("Input admission");
    }
    assert!(
        mailbox
            .try_enqueue(QueuedCommand::Input(Tagged::new(
                key.clone(),
                ControlInput::new(b"x".to_vec()).unwrap()
            )))
            .is_err(),
        "queue must be at capacity"
    );

    let cutoff = clock.now() + crate::terminal_session::state::OPERATION_TOTAL;
    assert_eq!(
        mailbox
            .publish_poll_result_before(poll_key.clone(), None, true, cutoff, &clock)
            .expect("PollResult publication"),
        MutationResult::Applied
    );
    ui.tick();

    // The hint became debt even though refresh admission reported Full.
    assert_eq!(mailbox.pending_refresh_stage(), None);
    assert!(ui.state.as_ref().unwrap().publication_debt_pending());
    assert!(mailbox.is_closed().is_none());
    assert!(matches!(
        mailbox
            .take_causal_before(&poll_key, cutoff, &clock)
            .expect("causal take"),
        Some(CausalSettlement::NoFeedback)
    ));
    mailbox.finish_lease(poll_lease).expect("lease finish");

    // The queued commands drain without producing a settlement; the debt stays
    // pending until a fair tick can admit the refresh.
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).expect("lease finish");
    }
    ui.tick();
    assert!(mailbox.pending_refresh_stage().is_some());
    assert!(!ui.state.as_ref().unwrap().publication_debt_pending());

    settle_queued_refresh(
        &mut ui,
        &mailbox,
        &clock,
        orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(2, 2),
        },
    );
    assert_eq!(
        ui.state.as_ref().unwrap().cache().revision(),
        WorkbenchRevision::new(2)
    );
}

/// A second hint arriving during the live debt flight becomes late demand: the
/// settled install leaves exactly one pending demand, which a later tick drains
/// into exactly one follow-up refresh.
#[test]
fn hint_during_debt_flight_leaves_exactly_one_follow_up_demand() {
    let (mut ui, mailbox, clock, _elapsed) = ready_ui();
    let key = ui.state.as_ref().unwrap().desired().unwrap().clone();

    run_poll(&mut ui, &mailbox, &clock, &key, true);
    assert!(mailbox.pending_refresh_stage().is_some());
    let lease = mailbox.try_recv().unwrap().expect("Refresh queued");
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one Refresh marker");
    };
    assert_eq!(
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap(),
        MutationResult::Applied
    );

    // The flight is live but its result has not installed: a second hint
    // arriving now is late demand, not a second debt. The executor-side setup
    // poll derived from the refresh lease can still publish while the flight is
    // live, exactly as the setup path does in production.
    let cutoff = clock.now() + crate::terminal_session::state::OPERATION_TOTAL;
    let crate::terminal_session::mailbox::SetupPollKey::Allocated(setup_key) = mailbox
        .allocate_setup_poll_key(&lease, &key)
        .expect("setup poll key")
    else {
        panic!("desired still current")
    };
    assert_eq!(
        mailbox
            .publish_poll_result_before(setup_key.clone(), None, true, cutoff, &clock)
            .expect("PollResult publication"),
        MutationResult::Applied
    );
    ui.tick();
    assert_eq!(
        ui.state.as_ref().unwrap().publication_debt_kind(),
        crate::terminal_session::state::PublicationDebt::InFlight { late: true }
    );
    assert!(matches!(
        mailbox
            .take_causal_before(&setup_key, cutoff, &clock)
            .expect("causal take"),
        Some(CausalSettlement::NoFeedback)
    ));

    let wait = mailbox
        .publish_refresh_result_before(
            marker.generation(),
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot(2, 2),
            },
            marker.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).expect("lease finish");
    assert_eq!(
        ui.state.as_ref().unwrap().cache().revision(),
        WorkbenchRevision::new(2)
    );
    // The late demand surfaced as exactly one follow-up flight: the same fair
    // tick that settled the install already admitted it.
    assert!(mailbox.pending_refresh_stage().is_some());
    assert_eq!(
        ui.state.as_ref().unwrap().publication_debt_kind(),
        crate::terminal_session::state::PublicationDebt::InFlight { late: false }
    );

    // The follow-up flight installs the late publication and settles the debt.
    settle_queued_refresh(
        &mut ui,
        &mailbox,
        &clock,
        orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(2, 3),
        },
    );
    assert_eq!(
        ui.state.as_ref().unwrap().cache().revision(),
        WorkbenchRevision::new(3)
    );
    assert!(!ui.state.as_ref().unwrap().publication_debt_pending());
    assert!(mailbox.pending_refresh_stage().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
}

/// A definite refresh refusal parks the debt: no hot-loop retry follows, and only
/// an accepted new hint or an explicit refresh re-arms it.
#[test]
fn definite_refusal_parks_debt_until_hint_or_manual_refresh_rearms() {
    let (mut ui, mailbox, clock, _elapsed) = ready_ui();
    let key = ui.state.as_ref().unwrap().desired().unwrap().clone();

    run_poll(&mut ui, &mailbox, &clock, &key, true);
    assert!(mailbox.pending_refresh_stage().is_some());
    settle_queued_refresh(
        &mut ui,
        &mailbox,
        &clock,
        orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Unavailable {
            reason:
                orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
        },
    );
    assert_eq!(mailbox.pending_refresh_stage(), None);
    assert!(!ui.state.as_ref().unwrap().publication_debt_pending());
    assert_eq!(
        ui.state.as_ref().unwrap().cache().revision(),
        WorkbenchRevision::new(1)
    );

    // No hot-loop retry: fair ticks leave the parked demand untouched.
    ui.tick();
    ui.tick();
    assert!(mailbox.pending_refresh_stage().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());

    // An accepted hint re-arms the debt and the same fair tick drains it.
    run_poll(&mut ui, &mailbox, &clock, &key, true);
    assert!(mailbox.pending_refresh_stage().is_some());
    settle_queued_refresh(
        &mut ui,
        &mailbox,
        &clock,
        orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
            snapshot: snapshot(2, 2),
        },
    );
    assert_eq!(
        ui.state.as_ref().unwrap().cache().revision(),
        WorkbenchRevision::new(2)
    );
}
