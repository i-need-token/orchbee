use super::publication_hint_tests::starting_split_ui;
use super::*;
use crate::terminal_session::mailbox::EventDeadline;
use orchbee_client_server_protocol::ControllerId;
use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use orchbee_workbench_protocol::WorkbenchPanelId;
use std::sync::atomic::Ordering;

fn publish(
    ui: &mut NativeUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    event: WorkerEvent,
) -> UiReply {
    let wait = mailbox
        .publish_event_before(
            event,
            EventDeadline::Forward(clock.now() + crate::terminal_session::state::OPERATION_TOTAL),
            clock,
        )
        .unwrap();
    ui.tick();
    mailbox.take_deposited_reply(&wait).unwrap().unwrap()
}

/// First-Starting boundary: a multi-leaf observation still syncs membership —
/// the peer record is installed and the convergence debt stays visible — but
/// the ordinary queue stays empty: no `ReconcilePanes`, no `Resize`, neither
/// through the observation entry nor through a fair tick. The startup
/// poll-key allocator's queue-empty/lease-empty guard itself still passes,
/// so the focus leaf's own setup can claim its Poll.
#[test]
fn starting_observation_records_membership_without_queuing_ordinary_work() {
    let (mut ui, mailbox, _clock, _elapsed) = starting_split_ui();
    assert_eq!(
        ui.state.as_ref().unwrap().phase_kind(),
        SessionPhaseKind::Starting
    );
    let focus_panel = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
    let peer_panel = WorkbenchPanelId::parse(&format!("{:032x}", 3_010)).unwrap();
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();

    ui.observe_panes(&[
        (focus_panel.clone(), 0, geometry),
        (peer_panel.clone(), 1, geometry),
    ]);

    let state = ui.state.as_ref().unwrap();
    assert!(state.pane_by_panel(&peer_panel).is_some());
    assert!(state.reconcile_pending());
    assert_eq!(state.phase_kind(), SessionPhaseKind::Starting);
    let focus_ref = state
        .pane_ref_for(state.desired().unwrap())
        .expect("focus pane record");
    assert!(mailbox.allocate_startup_poll_key(&focus_ref).is_ok());
    assert!(mailbox.try_recv().unwrap().is_none());

    ui.tick();
    assert!(mailbox.try_recv().unwrap().is_none());
    assert_eq!(
        ui.state.as_ref().unwrap().phase_kind(),
        SessionPhaseKind::Starting
    );
}

/// Post-Configured but still Starting: the focus leaf is attached and
/// `Controlling`, yet observations carrying fresh geometry queue no ordinary
/// `Resize`. Once Ready installs — after the clock has advanced — the first
/// schedule delivers the deferred reconcile under the deadline minted by the
/// Starting-phase divergence (never renewed), targeting the `PaneRef` the
/// record held when the observation installed it, and carrying the latest
/// observed geometry. The next observation emits exactly one `Resize` that
/// records `last_admitted_geometry`, so an identical repeat queues none.
#[test]
fn configured_starting_defers_ordinary_work_until_ready_then_resumes_once() {
    let (mut ui, mailbox, clock, elapsed) = starting_split_ui();
    let focus_panel = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
    let peer_panel = WorkbenchPanelId::parse(&format!("{:032x}", 3_010)).unwrap();
    let key = ui.state.as_ref().unwrap().desired().unwrap().clone();
    let geometry = Geometry::new(80, 24, 8, 16).unwrap();
    let moved = Geometry::new(100, 30, 8, 16).unwrap();
    let latest = Geometry::new(90, 28, 8, 16).unwrap();
    let mode = ControllerMode::Controlling(ControllerId::parse(&format!("{:032x}", 8)).unwrap());

    // The focus leaf's own setup converges to Controlling while the session
    // stays in Starting.
    let focus_ref = ui.state.as_ref().unwrap().pane_ref_for(&key).unwrap();
    publish(
        &mut ui,
        &mailbox,
        &clock,
        WorkerEvent::Attached {
            key: focus_ref.clone(),
            projection: TerminalProjection::new(
                AttachmentId::parse(&format!("{:032x}", 4_001)).unwrap(),
                HostEpoch::parse(&format!("{:032x}", 4_002)).unwrap(),
            ),
        },
    );
    publish(
        &mut ui,
        &mailbox,
        &clock,
        WorkerEvent::SetupConfigured {
            key: focus_ref.clone(),
            mode: mode.clone(),
            applied_geometry: Some(geometry),
        },
    );
    assert_eq!(
        ui.state.as_ref().unwrap().phase_kind(),
        SessionPhaseKind::Starting
    );

    // Two observations inside the same Starting window each record their
    // leaf geometry — the latest wins — yet neither may queue ordinary work.
    let sampled_at = clock.now();
    ui.observe_panes(&[
        (focus_panel.clone(), 0, moved),
        (peer_panel.clone(), 1, moved),
    ]);
    ui.observe_panes(&[
        (focus_panel.clone(), 0, latest),
        (peer_panel.clone(), 1, latest),
    ]);
    // The peer's incarnation is captured while still in Starting: the
    // post-Ready reconcile must target this same `PaneRef`, not a re-mint.
    let peer_ref = ui
        .state
        .as_ref()
        .unwrap()
        .pane_by_panel(&peer_panel)
        .unwrap()
        .pane_ref();
    assert!(mailbox.allocate_startup_poll_key(&focus_ref).is_ok());
    assert!(mailbox.try_recv().unwrap().is_none());

    // Advance the shared clock before Ready installs: a deadline silently
    // re-minted now would differ from the Starting-phase budget.
    elapsed.store(5_000, Ordering::SeqCst);

    // Finish the focus leaf's own setup so Ready installs.
    assert!(
        ui.state
            .as_mut()
            .unwrap()
            .stage_drawn_frame_t(&key, FrameIdentity::new(1))
    );
    publish(
        &mut ui,
        &mailbox,
        &clock,
        WorkerEvent::FeedbackAccepted {
            key: focus_ref.clone(),
            frame: FrameIdentity::new(1),
        },
    );
    assert!(matches!(
        publish(
            &mut ui,
            &mailbox,
            &clock,
            WorkerEvent::ReadyCandidate {
                key: focus_ref.clone(),
                mode: mode.clone(),
                last_accepted_geometry: Some(geometry),
                frame: FrameIdentity::new(1),
            },
        ),
        UiReply::Ready(crate::terminal_session::state::ReadyDecision::CatchUpResize(actual))
            if actual == latest
    ));
    assert!(ui.state.as_ref().unwrap().ready().is_none());
    assert!(mailbox.try_recv().unwrap().is_none());
    publish(
        &mut ui,
        &mailbox,
        &clock,
        WorkerEvent::SetupConfigured {
            key: focus_ref.clone(),
            mode: mode.clone(),
            applied_geometry: Some(latest),
        },
    );
    assert!(
        ui.state
            .as_mut()
            .unwrap()
            .stage_drawn_frame_t(&key, FrameIdentity::new(2))
    );
    publish(
        &mut ui,
        &mailbox,
        &clock,
        WorkerEvent::FeedbackAccepted {
            key: focus_ref.clone(),
            frame: FrameIdentity::new(2),
        },
    );
    assert!(matches!(
        publish(
            &mut ui,
            &mailbox,
            &clock,
            WorkerEvent::ReadyCandidate {
                key: focus_ref.clone(),
                mode: mode.clone(),
                last_accepted_geometry: Some(latest),
                frame: FrameIdentity::new(2),
            },
        ),
        UiReply::Ready(crate::terminal_session::state::ReadyDecision::Ready)
    ));
    assert!(ui.state.as_ref().unwrap().ready().is_some());

    // The first post-Ready schedule delivers the deferred reconcile under
    // the original deadline minted by the Starting-phase divergence, with
    // the peer's unchanged incarnation and its latest observed geometry.
    ui.tick();
    let lease = mailbox.try_recv().unwrap().expect("deferred reconcile");
    let QueuedCommand::ReconcilePanes(marker) = lease.record() else {
        panic!("deferred work is the pane reconcile");
    };
    assert_eq!(marker.anchor(), key.anchor());
    let target = marker
        .targets()
        .iter()
        .find(|target| target.pane_ref() == &peer_ref)
        .expect("the deferred reconcile must target the peer's Starting-era incarnation");
    assert_eq!(target.geometry(), latest);
    assert_eq!(
        marker.deadlines(),
        OperationDeadlines::from_sample(sampled_at).unwrap()
    );
    mailbox.finish_lease(lease).expect("lease finish");

    // A repeated observation now admits exactly one ordinary Resize on the
    // unchanged focus incarnation carrying the latest geometry.
    let post_ready = Geometry::new(91, 28, 8, 16).unwrap();
    ui.observe_panes(&[
        (focus_panel.clone(), 0, post_ready),
        (peer_panel.clone(), 1, latest),
    ]);
    let mut resizes = Vec::new();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        if let QueuedCommand::Resize(tagged) = lease.record() {
            resizes.push((tagged.key().clone(), *tagged.value()));
        }
        mailbox.finish_lease(lease).expect("lease finish");
    }
    assert_eq!(resizes, vec![(focus_ref, post_ready)]);

    // The identical re-observation is deduplicated through
    // `last_admitted_geometry`: no second Resize is queued.
    ui.observe_panes(&[(focus_panel, 0, post_ready), (peer_panel, 1, latest)]);
    while let Some(lease) = mailbox.try_recv().unwrap() {
        assert!(!matches!(lease.record(), QueuedCommand::Resize(_)));
        mailbox.finish_lease(lease).expect("lease finish");
    }
}

#[test]
fn starting_pane_observation_publishes_focus_geometry_without_background_override() {
    for reverse in [false, true] {
        let (mut ui, mailbox, _clock, _elapsed) = starting_split_ui();
        let focus_panel = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
        let peer_panel = WorkbenchPanelId::parse(&format!("{:032x}", 3_010)).unwrap();
        let focus_geometry = Geometry::new(89, 37, 8, 16).unwrap();
        let peer_geometry = Geometry::new(44, 37, 8, 16).unwrap();
        let mut observed = vec![
            (focus_panel, 0, focus_geometry),
            (peer_panel, 1, peer_geometry),
        ];
        if reverse {
            observed.reverse();
        }
        ui.observe_panes(&observed);
        assert_eq!(mailbox.latest_geometry().unwrap(), focus_geometry);
        assert_eq!(ui.state.as_ref().unwrap().latest_geometry(), focus_geometry);
        assert!(mailbox.try_recv().unwrap().is_none());
        ui.observe_panes(&[]);
        assert_eq!(mailbox.latest_geometry().unwrap(), focus_geometry);
        assert_eq!(ui.state.as_ref().unwrap().latest_geometry(), focus_geometry);
    }
}

#[test]
fn switching_pane_observation_replaces_the_previous_setup_geometry() {
    let (mut ui, mailbox, _clock, _elapsed) = super::move_membership_tests::ready_ui();
    let previous = Geometry::new(89, 37, 8, 16).unwrap();
    let focus = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
    ui.observe_panes(&[(focus, 0, previous)]);
    assert_eq!(mailbox.latest_geometry().unwrap(), previous);
    assert_eq!(
        ui.select_tab(ClientAction::SelectTab(2)),
        SessionActionDisposition::DismissNavigator
    );
    let target = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
    let measured = Geometry::new(79, 31, 8, 16).unwrap();
    ui.observe_panes(&[(target.clone(), 0, measured)]);
    assert_eq!(
        ui.state.as_ref().unwrap().phase_kind(),
        SessionPhaseKind::Switching
    );
    assert_eq!(ui.state.as_ref().unwrap().latest_geometry(), measured);
    assert_eq!(mailbox.latest_geometry().unwrap(), measured);
    ui.observe_panes(&[(target, 0, measured)]);
    assert_eq!(mailbox.latest_geometry().unwrap(), measured);
    assert!(!ui.retirement_requested());
}
