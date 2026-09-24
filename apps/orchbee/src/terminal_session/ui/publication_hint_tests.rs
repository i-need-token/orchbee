use super::membership_tests::snapshot;
use super::move_membership_tests::ready_ui;
use super::*;
use crate::local_client::test_server::adjustable_operation_clock;
use crate::terminal_projection::TerminalProjection;
use crate::terminal_session::mailbox::EventDeadline;
use crate::terminal_session::workbench_projection::project_native_standalone_tabs;
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use orchbee_workbench_protocol::{
    PanelNode, PanelOrientation, PanelSlot, Presentation, PresentationBinding, Snapshot,
    TerminalId, TerminalPresentationId, WorkbenchPanelId, WorkbenchPanelSlotId, WorkbenchRevision,
    WorkbenchSnapshotOutcome, WorkbenchSplitId, WorkerId,
};
use std::sync::atomic::AtomicU64;

/// Runs one ordinary poll exactly the way the executor does: the queued Poll is
/// leased, its result is published carrying the hint the publication probe
/// observed, the UI applies it, and the causal settlement the UI deposits is
/// taken so the next poll can publish.
/// Resolve the exact incarnation the mailbox admitted for `anchor` — the UI
/// seeds the pane set from real state, so fabricated keys are never admitted.
fn pane_ref(mailbox: &CommandMailbox, anchor: &AttachmentKey) -> PaneRef {
    mailbox.pane_ref_for_anchor(anchor).expect("admitted pane")
}

fn run_poll(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    key: &AttachmentKey,
    publication_hint: bool,
) {
    mailbox
        .try_enqueue(QueuedCommand::Poll(Tagged::new(pane_ref(mailbox, key), ())))
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

    run_poll(&mut ui, &mailbox, &clock, key.anchor(), true);

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
    run_poll(&mut ui, &mailbox, &clock, key.anchor(), false);
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
        .try_enqueue(QueuedCommand::Poll(Tagged::new(
            pane_ref(&mailbox, key.anchor()),
            (),
        )))
        .expect("Poll admission");
    let poll_lease = mailbox.try_recv().unwrap().expect("queued Poll");
    let poll_key = poll_lease
        .poll_key()
        .expect("Poll lease carries a poll key");
    for _ in 0..8 {
        let input = ControlInput::new(b"x".to_vec()).unwrap();
        mailbox
            .try_enqueue(QueuedCommand::Input(Tagged::new(
                pane_ref(&mailbox, key.anchor()),
                input,
            )))
            .expect("Input admission");
    }
    assert!(
        mailbox
            .try_enqueue(QueuedCommand::Input(Tagged::new(
                pane_ref(&mailbox, key.anchor()),
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

    run_poll(&mut ui, &mailbox, &clock, key.anchor(), true);
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
        .allocate_setup_poll_key(&lease, &pane_ref(&mailbox, key.anchor()))
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

    run_poll(&mut ui, &mailbox, &clock, key.anchor(), true);
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
    run_poll(&mut ui, &mailbox, &clock, key.anchor(), true);
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

/// A parked debt is also re-armed by the real manual refresh entry point: the
/// explicit `OpenWorkbenchNavigator` action admits an ordinary refresh that
/// adopts the parked demand into its flight, and the install settles it.
#[test]
fn parked_debt_rearms_on_explicit_manual_refresh() {
    let (mut ui, mailbox, clock, _elapsed) = ready_ui();
    let key = ui.state.as_ref().unwrap().desired().unwrap().clone();

    run_poll(&mut ui, &mailbox, &clock, key.anchor(), true);
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
    assert_eq!(
        ui.state.as_ref().unwrap().publication_debt_kind(),
        crate::terminal_session::state::PublicationDebt::Parked
    );

    // The parked demand does not drain on its own.
    ui.tick();
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(
        ui.state.as_ref().unwrap().publication_debt_kind(),
        crate::terminal_session::state::PublicationDebt::Parked
    );

    // The real manual refresh entry point re-arms and adopts the parked debt.
    ui.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    assert_eq!(
        ui.state.as_ref().unwrap().publication_debt_kind(),
        crate::terminal_session::state::PublicationDebt::InFlight { late: false }
    );
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
    assert!(!ui.state.as_ref().unwrap().publication_debt_pending());
    assert!(mailbox.try_recv().unwrap().is_none());
}

/// A hint landing during a clean manual refresh flight is not adopted by that
/// flight: it is captured as Pending demand, a definite refusal of the manual
/// flight parks nothing (only the adopted flight's debt parks), and the very
/// next fair tick drains the surviving demand into exactly one follow-up
/// refresh.
#[test]
fn manual_clean_flight_hint_stays_pending_through_refusal_and_drains_once() {
    let (mut ui, mailbox, clock, _elapsed) = ready_ui();
    let key = ui.state.as_ref().unwrap().desired().unwrap().clone();

    // The real manual refresh entry point admits a clean flight: no debt exists,
    // so nothing is adopted.
    ui.apply_client_action(ClientAction::OpenWorkbenchNavigator);
    let lease = mailbox.try_recv().unwrap().expect("manual Refresh queued");
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("one Refresh marker");
    };
    let first_generation = marker.generation();
    assert_eq!(
        ui.state.as_ref().unwrap().publication_debt_kind(),
        crate::terminal_session::state::PublicationDebt::Clean
    );
    assert_eq!(
        mailbox.mark_refresh_in_flight(marker.generation()).unwrap(),
        MutationResult::Applied
    );

    // A publication hint lands while the unassociated manual flight is live. The
    // executor-side setup Poll derived from the refresh lease publishes it, the
    // fair tick captures the latch, and the demand pends: it is not adopted by
    // the running flight.
    let cutoff = clock.now() + crate::terminal_session::state::OPERATION_TOTAL;
    let crate::terminal_session::mailbox::SetupPollKey::Allocated(setup_key) = mailbox
        .allocate_setup_poll_key(&lease, &pane_ref(&mailbox, key.anchor()))
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
        crate::terminal_session::state::PublicationDebt::Pending
    );
    assert!(matches!(
        mailbox
            .take_causal_before(&setup_key, cutoff, &clock)
            .expect("causal take"),
        Some(CausalSettlement::NoFeedback)
    ));

    // The manual flight is definitely refused. Parking applies only to the debt
    // a flight adopted; this flight adopted nothing, so the pending demand
    // survives the refusal and the same fair tick drains it once.
    let wait = mailbox
        .publish_refresh_result_before(
            marker.generation(),
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Unavailable {
                reason:
                    orchbee_workbench_protocol::WorkbenchSnapshotUnavailableReason::ServiceUnavailable,
            },
            marker.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).expect("lease finish");

    // Exactly one follow-up refresh was admitted, under a fresh generation.
    assert_eq!(
        ui.state.as_ref().unwrap().publication_debt_kind(),
        crate::terminal_session::state::PublicationDebt::InFlight { late: false }
    );
    let follow_up = mailbox.try_recv().unwrap().expect("one follow-up Refresh");
    let QueuedCommand::Refresh(follow_up_marker) = follow_up.record() else {
        panic!("one Refresh marker");
    };
    assert_ne!(follow_up_marker.generation(), first_generation);
    assert!(mailbox.try_recv().unwrap().is_none());

    // The follow-up installs the folded publication and settles the debt.
    assert_eq!(
        mailbox
            .mark_refresh_in_flight(follow_up_marker.generation())
            .unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_refresh_result_before(
            follow_up_marker.generation(),
            orchbee_workbench_protocol::WorkbenchSnapshotOutcome::Available {
                snapshot: snapshot(2, 2),
            },
            follow_up_marker.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(follow_up).expect("lease finish");
    assert_eq!(
        ui.state.as_ref().unwrap().cache().revision(),
        WorkbenchRevision::new(2)
    );
    assert!(!ui.state.as_ref().unwrap().publication_debt_pending());
    assert!(mailbox.pending_refresh_stage().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
}

pub(in crate::terminal_session) fn split_snapshot(revision: u64) -> Snapshot {
    let mut baseline = snapshot(2, revision);
    let original_root = baseline.terminal_contexts[0].tabs[0].root.clone();
    baseline.terminal_contexts[0].tabs[0].root = PanelNode::Split {
        children: Box::new([
            PanelSlot {
                node: original_root,
                slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", 3_001)).unwrap(),
            },
            PanelSlot {
                node: PanelNode::Panel {
                    child_presentations: Vec::new(),
                    panel_id: WorkbenchPanelId::parse(&format!("{:032x}", 3_010)).unwrap(),
                    primary_presentation: Presentation {
                        binding: Some(PresentationBinding {
                            terminal_id: TerminalId::parse(&format!("{:032x}", 3_012)).unwrap(),
                            worker_id: WorkerId::parse(&format!("{:032x}", 3_011)).unwrap(),
                        }),
                        label: "peer".into(),
                        presentation_id: TerminalPresentationId::parse(&format!("{:032x}", 3_013))
                            .unwrap(),
                    },
                },
                slot_id: WorkbenchPanelSlotId::parse(&format!("{:032x}", 3_014)).unwrap(),
            },
        ]),
        orientation: PanelOrientation::Horizontal,
        split_id: WorkbenchSplitId::parse(&format!("{:032x}", 3_020)).unwrap(),
    };
    baseline
}

/// Builds a UI over one tab split into two horizontal panels that stops right
/// after the Bootstrap reply: the session is still in its first `Starting`
/// phase and the focus leaf's own setup pipeline has not converged yet.
pub(super) fn starting_split_ui() -> (
    NativeUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    let split = split_snapshot(1);
    let elapsed = Arc::new(AtomicU64::new(0));
    let clock = adjustable_operation_clock(Arc::clone(&elapsed));
    let mailbox = Arc::new(CommandMailbox::new(
        10,
        Geometry::new(80, 24, 8, 16).unwrap(),
    ));
    let mut ui = NativeUi::new(Arc::clone(&mailbox), clock.clone());
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Bootstrap {
                cache: project_native_standalone_tabs(&split).unwrap(),
            },
            EventDeadline::Startup(clock.now() + crate::terminal_session::state::OPERATION_TOTAL),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    (ui, mailbox, clock, elapsed)
}

/// Builds a ready UI over one tab split into two horizontal panels so panel
/// focus has a real target: the MovePanelFocus switch moves `desired` to a fresh
/// generation key without clearing the UI's observed Poll serial.
pub(in crate::terminal_session) fn ready_split_ui() -> (
    NativeUi,
    Arc<CommandMailbox>,
    LocalOperationClock,
    Arc<AtomicU64>,
) {
    let (mut ui, mailbox, clock, elapsed) = starting_split_ui();
    let key = ui.state.as_ref().unwrap().desired().unwrap().clone();

    // Drive the real setup event chain so `ui.projection` is genuinely installed:
    // `schedule_poll` only admits an ordinary Poll while a projection for the
    // desired attachment is live, and only the `Attached` event installs it.
    let publish = |ui: &mut NativeUi, event: WorkerEvent| {
        let wait = mailbox
            .publish_event_before(
                event,
                EventDeadline::Forward(
                    clock.now() + crate::terminal_session::state::OPERATION_TOTAL,
                ),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    };
    let key_ref = ui.state.as_ref().unwrap().pane_ref_for(&key).unwrap();
    publish(
        &mut ui,
        WorkerEvent::Attached {
            key: key_ref.clone(),
            projection: TerminalProjection::new(
                AttachmentId::parse(&format!("{:032x}", 4_001)).unwrap(),
                HostEpoch::parse(&format!("{:032x}", 4_002)).unwrap(),
            ),
        },
    );
    let key_ref = ui.state.as_ref().unwrap().pane_ref_for(&key).unwrap();
    publish(
        &mut ui,
        WorkerEvent::SetupConfigured {
            key: key_ref.clone(),
            mode: ControllerMode::ReadOnly,
            applied_geometry: None,
        },
    );
    // The drawn frame is renderer-local; only its acceptance arrives as an event.
    assert!(
        ui.state
            .as_mut()
            .unwrap()
            .stage_drawn_frame_t(&key, FrameIdentity::new(1))
    );
    let key_ref = ui.state.as_ref().unwrap().pane_ref_for(&key).unwrap();
    publish(
        &mut ui,
        WorkerEvent::FeedbackAccepted {
            key: key_ref.clone(),
            frame: FrameIdentity::new(1),
        },
    );
    let key_ref = ui.state.as_ref().unwrap().pane_ref_for(&key).unwrap();
    publish(
        &mut ui,
        WorkerEvent::ReadyCandidate {
            key: key_ref.clone(),
            mode: ControllerMode::ReadOnly,
            last_accepted_geometry: None,
            frame: FrameIdentity::new(1),
        },
    );
    assert!(ui.state.as_ref().unwrap().ready().is_some());
    assert!(ui.projection().is_some());
    (ui, mailbox, clock, elapsed)
}

#[test]
fn explicit_refresh_install_uses_transaction_clock_and_publication_does_not_reopen() {
    use crate::terminal_session::state::{OPERATION_TOTAL, PaneFault};
    use std::sync::atomic::Ordering;

    for explicit in [false, true] {
        let (mut ui, mailbox, clock, elapsed) = ready_split_ui();
        // Finish the genuine focus poll queued by Ready before installing the
        // expired fixture. Preserve its causal protocol, not just its queue slot.
        let initial_poll = mailbox.try_recv().unwrap().expect("Ready focus poll");
        let poll_key = initial_poll.poll_key().expect("ordinary focus poll");
        let cutoff = clock.now() + OPERATION_TOTAL;
        assert_eq!(
            mailbox
                .publish_poll_result_before(poll_key.clone(), None, false, cutoff, &clock)
                .unwrap(),
            MutationResult::Applied
        );
        ui.tick();
        assert!(matches!(
            mailbox
                .take_causal_before(&poll_key, cutoff, &clock)
                .unwrap(),
            Some(CausalSettlement::NoFeedback)
        ));
        mailbox.finish_lease(initial_poll).unwrap();
        let geometry = Geometry::new(80, 24, 8, 16).unwrap();
        let focus = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
        let peer = WorkbenchPanelId::parse(&format!("{:032x}", 3_010)).unwrap();
        let observed = [(focus, 0, geometry), (peer.clone(), 1, geometry)];
        // Establish a dead episode in the canonical owner without pretending
        // the unattached peer ever obtained a viewer or grant.
        let state = ui.state.as_mut().unwrap();
        assert!(state.sync_pane_set(&observed, clock.now()));
        elapsed.store(51_000, Ordering::SeqCst);
        assert!(state.reconcile_targets(clock.now()).is_none());
        assert_eq!(
            state.pane_by_panel(&peer).unwrap().fault(),
            Some(PaneFault::ConvergenceExpired)
        );

        if explicit {
            ui.apply_client_action(ClientAction::OpenWorkbenchNavigator);
        } else {
            ui.state.as_mut().unwrap().note_publication_hint();
            ui.tick();
        }
        assert!(mailbox.pending_refresh_stage().is_some());
        assert_eq!(
            ui.state
                .as_ref()
                .unwrap()
                .pane_by_panel(&peer)
                .unwrap()
                .fault(),
            Some(PaneFault::ConvergenceExpired)
        );

        // Acceptance happens after admission, at the controlled reply clock.
        // A system Instant::now() would mint an already-expired budget here.
        elapsed.store(52_000, Ordering::SeqCst);
        let installed_at = clock.now();
        settle_queued_refresh(
            &mut ui,
            &mailbox,
            &clock,
            WorkbenchSnapshotOutcome::Available {
                snapshot: split_snapshot(2),
            },
        );
        assert!(mailbox.is_closed().is_none());
        let state = ui.state.as_mut().unwrap();
        if explicit {
            assert_eq!(state.pane_by_panel(&peer).unwrap().fault(), None);
            let (_, _, deadlines) = state.reconcile_targets(installed_at).unwrap();
            assert_eq!(deadlines.total(), installed_at + OPERATION_TOTAL);
        } else {
            assert_eq!(
                state.pane_by_panel(&peer).unwrap().fault(),
                Some(PaneFault::ConvergenceExpired)
            );
            assert!(state.reconcile_targets(installed_at).is_none());
        }
        assert!(state.invariant_holds());
    }
}

/// The exact stall chain: a queued pre-entry Poll on K1, a tab-select switch to
/// K2 that abandons nothing queued but leaves the old Poll to finish silently
/// against the moved `desired`, a scripted non-installing Denied outcome that
/// legally restores the K1 anchor and Ready, then a fair NativeUi tick that
/// reconciles the stale observation and admits exactly one fresh Poll under a
/// serial the abandoned one never owned.
///
/// Same-tab panel focus is a pure local move under pane collections (it never
/// mints a fresh anchor), so the abandoning switch is driven by SelectTab.
#[test]
fn panel_focus_silent_poll_finish_restores_anchor_and_readmits_one_fresh_poll() {
    let (mut ui, mailbox, clock, _elapsed) = ready_split_ui();
    let anchor = ui.state.as_ref().unwrap().desired().unwrap().clone();

    // A fair tick schedules the ordinary K1 Poll; it stays queued pre-entry.
    ui.tick();
    let observed = ui
        .ordinary_poll_serial
        .expect("the K1 Poll is admitted and observed");
    assert!(mailbox.ordinary_poll_continues(observed).unwrap());

    // Selecting the second tab admits a Switch to a fresh generation key; the
    // queued K1 Poll record survives admission untouched while the UI drops its
    // observed serial — the old anchor's projections are hidden until a proven
    // restore.
    ui.apply_client_action(ClientAction::SelectTab(2));
    assert_eq!(
        ui.ordinary_poll_serial, None,
        "the switch drops the old anchor's Poll observation"
    );

    // The executor leases the stale K1 Poll and finishes it silently: its
    // attachment is no longer the desired key, so run_poll returns before any
    // read or publish — no PollResult event and no switch-discard record, the
    // exact pre-entry staleness path the real executor takes.
    let stale_poll = mailbox.try_recv().unwrap().expect("queued K1 Poll");
    let stale_key = stale_poll.poll_key().expect("ordinary Poll key");
    assert_eq!(stale_key.attachment(), anchor.anchor());
    mailbox.finish_lease(stale_poll).expect("lease finish");
    assert!(!mailbox.ordinary_poll_continues(observed).unwrap());
    assert_eq!(ui.ordinary_poll_serial, None);

    // The scripted non-installing Denied outcome legally restores the K1 anchor.
    let switch = mailbox.try_recv().unwrap().expect("Switch marker");
    let QueuedCommand::Switch(switch_marker) = switch.record() else {
        panic!("one Switch marker");
    };
    let target = switch_marker.target().clone();
    let permit = mailbox
        .authorize_switch_snapshot(&switch, &target, &clock)
        .unwrap()
        .expect("snapshot authorization");
    mailbox.answer_switch_snapshot(permit).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::SwitchSnapshotResult {
                marker: target.selected().key().clone(),
                outcome: WorkbenchSnapshotOutcome::Denied {},
            },
            EventDeadline::PlacementOnly(target.deadlines().forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(matches!(
        mailbox.take_deposited_reply(&wait).unwrap(),
        Some(UiReply::Switch(settlement))
            if matches!(settlement.decision(), RefreshDecision::Continue)
    ));
    mailbox.finish_lease(switch).expect("lease finish");

    // The anchor is legally restored: desired is the original K1 key and the
    // session is Ready again.
    assert_eq!(ui.state.as_ref().unwrap().desired(), Some(&anchor));
    assert!(ui.state.as_ref().unwrap().ready().is_some());
    assert!(ui.state.as_ref().unwrap().invariant_holds());

    // One fair tick reconciles the stale observed serial against canonical
    // occupancy and admits exactly one fresh Poll; the abandoned serial is never
    // reused.
    let fresh = mailbox
        .try_recv()
        .unwrap()
        .expect("one fresh Poll must be admitted after the anchor restore");
    let QueuedCommand::Poll(tagged) = fresh.record() else {
        panic!("the readmitted command is an ordinary Poll");
    };
    assert_eq!(tagged.key(), &anchor);
    assert_ne!(fresh.serial(), observed);
    assert!(mailbox.ordinary_poll_continues(fresh.serial()).unwrap());
    assert!(!mailbox.ordinary_poll_continues(observed).unwrap());
    assert_eq!(ui.ordinary_poll_serial, Some(fresh.serial()));
    assert!(mailbox.try_recv().unwrap().is_none());
    mailbox.finish_lease(fresh).expect("lease finish");
}

/// B3 counterexample: a pending draw whose pane left the desired membership
/// before the draw drained must settle `NoFeedback`. The causal settlement
/// claims this draw's proof — forwarding `FrameApplied` for a frame no live
/// leaf staged lets the worker report a frame that never reached the screen,
/// and the departing incarnation stays admitted until the reconcile lands,
/// so the lie is observable on the wire.
#[test]
fn departed_pane_pending_draw_settles_no_feedback_not_frame_applied() {
    use orchbee_terminal_protocol::{
        CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding,
        encode_transport_envelope, inspect_terminal_frame_route,
    };

    let (mut ui, mailbox, clock, _elapsed) = ready_split_ui();
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    let cutoff = clock.now() + crate::terminal_session::state::OPERATION_TOTAL;
    let focus_panel = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
    let peer_panel = WorkbenchPanelId::parse(&format!("{:032x}", 3_010)).unwrap();

    // The leaf-geometry report registers the peer leaf's record and admits
    // both incarnations through the reconcile snapshot it schedules.
    ui.observe_panes(&[
        (focus_panel.clone(), 0, geometry),
        (peer_panel.clone(), 1, geometry),
    ]);
    let pane_b = ui
        .state
        .as_ref()
        .unwrap()
        .pane_by_panel(&peer_panel)
        .expect("peer leaf record")
        .pane_ref();

    // The peer's viewer attaches through the real event path, installing a
    // projection keyed to the exact incarnation and matching the fixture
    // frame's baked route.
    let mut frame = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin"
    ))
    .to_vec();
    frame[48..56].copy_from_slice(&1u64.to_le_bytes());
    frame[56..64].copy_from_slice(&u64::MAX.to_le_bytes());
    frame[64..72].copy_from_slice(&2u64.to_le_bytes());
    let route = inspect_terminal_frame_route(&frame).unwrap();
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::Attached {
                key: pane_b.clone(),
                projection: TerminalProjection::new(route.attachment_id, route.host_epoch),
            },
            EventDeadline::Forward(cutoff),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());

    // A real setup-poll key for the peer, minted under the reconcile lease
    // the observation queued — the same path the executor's background
    // attach/poll pipeline takes.
    let lease = loop {
        let lease = mailbox.try_recv().unwrap().expect("queued commands");
        if matches!(lease.record(), QueuedCommand::ReconcilePanes(_)) {
            break lease;
        }
        mailbox.finish_lease(lease).expect("lease finish");
    };
    let crate::terminal_session::mailbox::SetupPollKey::Allocated(poll_key) = mailbox
        .allocate_setup_poll_key(&lease, &pane_b)
        .expect("setup poll key")
    else {
        panic!("the admitted peer incarnation allocates a poll key")
    };
    let record = encode_transport_envelope(&Envelope {
        protocol: CURRENT_PROTOCOL_VERSION,
        role: MessageRole::Event,
        kind: 41,
        payload_encoding: PayloadEncoding::TerminalFrame,
        host_epoch: Some(route.host_epoch),
        correlation_id: None,
        optional_message: false,
        payload: &frame,
    })
    .unwrap();
    assert_eq!(
        mailbox
            .publish_poll_result_before(poll_key.clone(), Some(record), false, cutoff, &clock)
            .expect("PollResult publication"),
        MutationResult::Applied
    );
    ui.tick();
    assert!(
        ui.startup_draw_ready(),
        "the peer's frame is a pending draw"
    );

    // The leaf vanishes before the draw drains: the record turns departing
    // while its incarnation is still admitted — the reconcile that abandons
    // the causal slot is a separate step that has not landed yet.
    ui.state
        .as_mut()
        .unwrap()
        .sync_pane_set(&[(focus_panel, 0, geometry)], clock.now());
    ui.after_draw();

    // The frame never reached the screen, so the settlement is NoFeedback —
    // forwarding the poll-time FrameApplied would ack an undrawn frame.
    assert_eq!(
        mailbox
            .take_causal_before(&poll_key, cutoff, &clock)
            .expect("causal take"),
        Some(CausalSettlement::NoFeedback)
    );
    mailbox.finish_lease(lease).expect("lease finish");
}

/// Extend the ready split fixture through the peer's actual attachment/setup intake.
pub(in crate::terminal_session) fn attach_split_peer(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
) {
    let state = ui.state.as_ref().unwrap();
    let focus = state.focus_panel_id().unwrap().clone();
    let peer = state.cache().tabs()[0].panel(1).unwrap().panel_id().clone();
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    ui.observe_panes(&[(focus, 0, geometry), (peer.clone(), 1, geometry)]);
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).unwrap();
    }
    let key = ui
        .state
        .as_ref()
        .unwrap()
        .pane_by_panel(&peer)
        .unwrap()
        .pane_ref()
        .clone();
    for event in [
        WorkerEvent::Attached {
            key: key.clone(),
            projection: TerminalProjection::new(
                AttachmentId::parse(&format!("{:032x}", 4011)).unwrap(),
                HostEpoch::parse(&format!("{:032x}", 4012)).unwrap(),
            ),
        },
        WorkerEvent::SetupConfigured {
            key: key.clone(),
            mode: ControllerMode::ReadOnly,
            applied_geometry: None,
        },
    ] {
        let wait = mailbox
            .publish_event_before(
                event,
                EventDeadline::Forward(
                    clock.now() + crate::terminal_session::state::OPERATION_TOTAL,
                ),
                clock,
            )
            .unwrap();
        ui.tick();
        assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    }
    assert!(ui.state.as_ref().unwrap().panel_viewer_live(&peer));
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).unwrap();
    }
}

#[test]
fn remote_panel_focus_refuses_a_faulted_peer_instead_of_waiting_or_reattaching() {
    use crate::terminal_session::client_control::{ControlStep, StepProgress};
    use orchbee_workbench_protocol::{
        ClientViewControl, ClientViewControlReason, ClientViewIntent, ClientViewSequence,
        WorkbenchClientId, WorkbenchRequestId,
    };
    let (mut ui, mailbox, clock, _elapsed) = ready_split_ui();
    attach_split_peer(&mut ui, &mailbox, &clock);
    let state = ui.state.as_ref().unwrap();
    let tab = &state.cache().tabs()[0];
    let peer = tab.panel(1).unwrap().panel_id().clone();
    let key = state.pane_by_panel(&peer).unwrap().pane_ref();
    let command = ClientViewControl {
        operation_id: WorkbenchRequestId::parse(&format!("{:032x}", 9001)).unwrap(),
        client_id: WorkbenchClientId::parse(&format!("{:032x}", 9002)).unwrap(),
        expected_view_revision: ClientViewSequence::new(1).unwrap(),
        minimum_workbench_revision: state.cache().revision(),
        intent: ClientViewIntent::Panel {
            terminal_context_id: state.cache().terminal_context_id().clone(),
            tab_id: tab.tab_id().clone(),
            panel_id: peer.clone(),
        },
    };
    mailbox
        .publish_notice_before(
            WorkerEvent::PaneFailed {
                key,
                fault: crate::terminal_session::state::PaneFault::ViewerLost,
            },
            clock.now() + crate::terminal_session::state::OPERATION_TOTAL,
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(!ui.state.as_ref().unwrap().panel_viewer_live(&peer));
    let before = ui.control_observation(false);
    let mut progress = StepProgress::default();
    assert!(matches!(
        ui.control_step(&command, &mut progress),
        ControlStep::Failed(ClientViewControlReason::ProjectionUnavailable)
    ));
    assert!(!progress.had_effect);
    assert_eq!(ui.control_observation(false), before);
}
