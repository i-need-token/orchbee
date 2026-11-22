use super::super::tests::ready::ready_ui;
use super::super::*;
use crate::workbench_client::mailbox::EventDeadline;
use crate::workbench_state::OPERATION_TOTAL;
use crate::workbench_state::control_takeover::{TakeoverEvidence, TakeoverResult};
use crate::{
    client_view::ClientView, localization::Locale, tui_hit_map::HitMapBuilder,
    tui_render::test_capture::capture_screen, tui_theme::Theme,
};
use orchbee_client_server_protocol::ControllerId;
use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, Envelope, MessageRole, PayloadEncoding, decode_terminal_text_frame,
    encode_transport_envelope, inspect_terminal_frame_route,
};
use ratatui::{Terminal, backend::TestBackend};
use std::sync::atomic::Ordering;

const SNAPSHOT: &[u8] =
    include_bytes!("../../../../../../../../schemas/terminal/v1/fixtures/ghostty-surface.bin");

#[test]
fn published_controller_changes_reach_the_client_without_disconnect_or_discarding_its_picture() {
    let (mut ui, mailbox, clock, _) = ready_ui();
    let key = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    let picture = ui.state.as_ref().unwrap().focus_accepted_frame();
    let owner = ControllerMode::Controlling(ControllerId::parse(&"33".repeat(16)).unwrap());
    for (mode, geometry) in [
        (
            owner,
            Some(orchbee_client_server_protocol::Geometry::new(80, 24, 8, 16).unwrap()),
        ),
        (ControllerMode::ReadOnly, None),
    ] {
        mailbox
            .publish_notice_before(
                WorkerEvent::ControllerObserved {
                    key: key.clone(),
                    mode: mode.clone(),
                    geometry,
                },
                clock.now() + OPERATION_TOTAL,
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(
            ui.retirement_failure.is_none(),
            "ownership publication must not disconnect a Client"
        );
        let state = ui.state.as_ref().unwrap();
        assert_eq!(state.controller_mode(), Some(&mode));
        assert_eq!(state.focus_accepted_frame(), picture);
    }
}

fn poll_snapshot(
    ui: &mut WorkbenchClientUi,
    mailbox: &CommandMailbox,
    clock: &LocalOperationClock,
    sequence: u64,
) {
    let key = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    let mut frame = SNAPSHOT.to_vec();
    frame[48..56].copy_from_slice(&sequence.to_le_bytes());
    let route = inspect_terminal_frame_route(&frame).unwrap();
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
    ui.tick();
    let lease = mailbox.try_recv().unwrap().expect("ordinary poll queued");
    let poll = lease.poll_key().expect("ordinary poll");
    let cutoff = clock.now() + OPERATION_TOTAL;
    mailbox
        .publish_poll_result_before(poll.clone(), Some(record), false, cutoff, clock)
        .unwrap();
    ui.tick();
    assert_eq!(
        ui.view().projection_status(),
        ClientProjectionStatus::FrameAvailable
    );
    ui.after_draw();
    assert!(matches!(
        mailbox.take_causal_before(&poll, cutoff, clock).unwrap(),
        Some(CausalSettlement::Feedback(
            TerminalViewerAction::FrameApplied { .. }
        ))
    ));
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::FeedbackAccepted {
                key,
                frame: FrameIdentity::new(poll.serial()),
            },
            EventDeadline::PlacementOnly(cutoff),
            clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
}

#[test]
fn returning_to_a_controlled_terminal_preserves_typing_and_its_current_frame() {
    let (mut ui, mailbox, clock, _) = super::ready::ready_ui_with_mode(
        super::membership::snapshot(2, 1),
        ControllerMode::Controlling(
            ControllerId::parse("00000000000000000000000000000001").unwrap(),
        ),
    );
    ui.observe_focus(true);
    assert!(!ui.drive_focus_control());
    ui.observe_focus(false);
    ui.observe_focus(true);
    assert!(ui.drive_focus_control());
    assert!(
        ui.state.as_ref().unwrap().takeover_input_ready(),
        "returning while already controlling must not fence input"
    );
    assert!(
        ui.forward(b"git st"),
        "input during the ownership observation must be retained"
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("ownership observation");
    };
    let operation = marker.takeover().unwrap().clone();
    assert_eq!(
        mailbox
            .mark_refresh_in_flight(operation.generation)
            .unwrap(),
        MutationResult::Applied
    );
    let evidence = TakeoverEvidence {
        operation: operation.clone(),
        result: TakeoverResult::Unchanged(
            ControllerId::parse("00000000000000000000000000000001").unwrap(),
        ),
    };
    assert!(mailbox.retain_takeover_result(&operation, evidence.result.clone()));
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::ControlTakeoverResult { evidence },
            EventDeadline::PlacementOnly(operation.deadlines.forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
    assert!(ui.state.as_ref().unwrap().takeover_input_ready());
    assert!(
        ui.view().takeover_status().is_none(),
        "the unchanged owner needs no control notice or new frame"
    );
    let input = mailbox.try_recv().unwrap().unwrap();
    assert!(
        matches!(input.record(), QueuedCommand::Input(tagged) if tagged.value().as_bytes() == b"git st")
    );
    mailbox.finish_lease(input).unwrap();
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn keys_in_the_same_packet_as_focus_gain_wait_for_the_activation() {
    let (mut ui, mailbox, _, _) = ready_ui();
    ui.observe_focus(true);
    assert!(
        ui.forward(b"echo immediate\r"),
        "the first packet must not require an intervening UI tick"
    );
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "unsubmitted activation input must not reach an old controller"
    );
    assert!(ui.drive_focus_control());
    assert!(!ui.drive_focus_control());
}

#[test]
fn focus_takeover_keeps_early_keys_until_frame_feedback_then_forwards_the_whole_command() {
    let (mut ui, mailbox, clock, _) = ready_ui();
    let route = inspect_terminal_frame_route(SNAPSHOT).unwrap();
    let frame = decode_terminal_text_frame(SNAPSHOT).unwrap();
    ui.observe_geometry(
        Geometry::new(
            frame.geometry.columns,
            frame.geometry.rows,
            frame.geometry.cell_pixel_width,
            frame.geometry.cell_pixel_height,
        )
        .unwrap(),
    );
    let key = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    ui.projections.push(KeyedProjection::for_test(
        key,
        TerminalProjection::new(route.attachment_id, route.host_epoch),
        ClientProjectionStatus::Empty,
    ));
    ui.observe_focus(false);
    ui.observe_keyboard();
    assert!(ui.forward(b"ec"));
    ui.observe_focus(true);
    assert!(ui.drive_focus_control());
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("focus activation");
    };
    let operation = marker.takeover().unwrap().clone();
    mailbox
        .mark_refresh_in_flight(operation.generation)
        .unwrap();
    let evidence = TakeoverEvidence {
        operation: operation.clone(),
        result: TakeoverResult::Ready(ControllerId::parse(&"77".repeat(16)).unwrap()),
    };
    assert!(mailbox.retain_takeover_result(&operation, evidence.result.clone()));
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::ControlTakeoverResult { evidence },
            EventDeadline::PlacementOnly(operation.deadlines.forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
    assert!(ui.forward(b"ho\r"));
    poll_snapshot(&mut ui, &mailbox, &clock, 1);
    ui.tick();
    let input = mailbox
        .try_recv()
        .unwrap()
        .expect("the accepted activation releases its unsent keys");
    assert!(
        matches!(input.record(), QueuedCommand::Input(tagged) if tagged.value().as_bytes() == b"echo\r"),
        "the complete command must survive the frame gate: {:?}",
        input.record()
    );
}

#[test]
fn an_unknown_focus_transfer_does_not_loop_and_a_later_activation_can_reconcile_it() {
    let (mut ui, mailbox, clock, _) = ready_ui();
    ui.observe_focus(true);
    assert!(ui.drive_focus_control());
    let lease = mailbox.try_recv().unwrap().unwrap();
    let QueuedCommand::Refresh(marker) = lease.record() else {
        panic!("activation");
    };
    let operation = marker.takeover().unwrap().clone();
    mailbox
        .mark_refresh_in_flight(operation.generation)
        .unwrap();
    let evidence = TakeoverEvidence {
        operation: operation.clone(),
        result: TakeoverResult::Unknown,
    };
    assert!(mailbox.retain_takeover_result(&operation, evidence.result.clone()));
    let wait = mailbox
        .publish_event_before(
            WorkerEvent::ControlTakeoverResult { evidence },
            EventDeadline::PlacementOnly(operation.deadlines.forward_cutoff()),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
    assert!(
        !ui.drive_focus_control(),
        "uncertainty must not rearm this activation"
    );
    assert!(!ui.forward(b"uncertain grant"));
    ui.observe_focus(false);
    ui.observe_focus(true);
    assert!(
        ui.drive_focus_control(),
        "a new activation reads Server authority again"
    );
    assert!(!ui.drive_focus_control());
}

/// Two TUIs of one Session: this one's host already reported focus, its focus-out never came,
/// and another Client took the terminal. Raw host bytes through the real dispatcher and the
/// production port still claim the size: a repeated FocusIn, a click, or a typed key, which
/// waits for the claim instead of reaching the old controller.
#[test]
fn a_viewer_with_stale_host_focus_claims_the_size_from_raw_focus_click_or_key() {
    use crate::terminal_input::InputDispatcher;
    use crate::workbench_client::{ClientBackend, WorkbenchClient};
    fn dispatch(
        dispatcher: &mut InputDispatcher,
        view: &mut ClientView,
        client: &mut WorkbenchClient,
        bytes: &[u8],
    ) {
        dispatcher
            .dispatch(
                view,
                bytes,
                client,
                std::time::Instant::now(),
                |_, _| Ok::<bool, std::io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
    }
    for evidence in [
        b"\x1b[I".as_slice(),
        b"\x1b[<0;10;5M\x1b[<0;10;5m".as_slice(),
        b"x".as_slice(),
    ] {
        let (ui, mailbox, clock, _) = ready_ui();
        let (shutdown, _peer) = orchbee_local_ipc::LocalStream::pair().unwrap();
        let mut client = WorkbenchClient {
            position_writer: None,
            position_startup: None,
            configuration: None,
            notifications: None,
            window_title: None,
            terminal_titles: None,
            agent_confirmations: None,
            agent_confirmations_check: None,
            client_control: None,
            backend: ClientBackend::Native(Box::new(ui)),
            task: None,
            shutdown: crate::local_client::test_support::server::shutdown_handle(shutdown),
            retirement_redraw_pending: false,
            origin: None,
            recovery_shutdown: None,
            updater: None,
            local_runtime: None,
        };
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);

        dispatch(&mut dispatcher, &mut view, &mut client, b"\x1b[I");
        let ui = client.backend.native_mut().unwrap();
        assert!(ui.drive_focus_control(), "the first FocusIn claims");
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Refresh(marker) = lease.record() else {
            panic!("focus claim");
        };
        let operation = marker.takeover().unwrap().clone();
        mailbox
            .mark_refresh_in_flight(operation.generation)
            .unwrap();
        let lost = TakeoverEvidence {
            operation: operation.clone(),
            result: TakeoverResult::Lost(ControllerId::parse(&"77".repeat(16)).unwrap()),
        };
        assert!(mailbox.retain_takeover_result(&operation, lost.result.clone()));
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::ControlTakeoverResult { evidence: lost },
                EventDeadline::PlacementOnly(operation.deadlines.forward_cutoff()),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
        mailbox.finish_lease(lease).unwrap();
        assert_eq!(
            ui.state.as_ref().unwrap().controller_mode(),
            Some(&ControllerMode::ReadOnly)
        );
        assert!(!ui.drive_focus_control(), "nothing claims again by itself");

        dispatch(&mut dispatcher, &mut view, &mut client, evidence);
        let ui = client.backend.native_mut().unwrap();
        assert!(ui.drive_focus_control(), "{evidence:?} claims the size");
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Refresh(marker) = lease.record() else {
            panic!("{evidence:?}: the claim comes before any input");
        };
        let claim = marker.takeover().unwrap();
        assert!(claim.automatic);
        assert_eq!(
            Some(&claim.key),
            ui.state.as_ref().unwrap().focus_pane_ref().as_ref()
        );
        assert!(
            mailbox.try_recv().unwrap().is_none(),
            "{evidence:?}: typing waits for the claim"
        );
        assert_eq!(view.notice(), None, "{evidence:?}: no refusal is shown");
    }
}

#[test]
fn focus_activation_queues_one_transfer_and_focus_loss_cancels_unsubmitted_intent() {
    let (mut ui, mailbox, _, _) = ready_ui();
    ui.observe_focus(false);
    assert!(!ui.drive_focus_control());
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.observe_focus(true);
    ui.observe_focus(false);
    assert!(!ui.drive_focus_control());
    assert!(mailbox.try_recv().unwrap().is_none());
    ui.observe_focus(true);
    assert!(ui.drive_focus_control());
    assert!(
        !ui.drive_focus_control(),
        "one activation submits only once"
    );
    let lease = mailbox.try_recv().unwrap().unwrap();
    assert!(
        matches!(lease.record(), QueuedCommand::Refresh(marker) if marker.takeover().is_some())
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn takeover_rearmed_snapshot_restores_typing_paste_and_control_without_reselection_or_replay() {
    let (mut ui, mailbox, clock, _) = ready_ui();
    let route = inspect_terminal_frame_route(SNAPSHOT).unwrap();
    let frame = decode_terminal_text_frame(SNAPSHOT).unwrap();
    ui.observe_geometry(
        Geometry::new(
            frame.geometry.columns,
            frame.geometry.rows,
            frame.geometry.cell_pixel_width,
            frame.geometry.cell_pixel_height,
        )
        .unwrap(),
    );
    let key = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    ui.projections.push(KeyedProjection::for_test(
        key.clone(),
        TerminalProjection::new(route.attachment_id, route.host_epoch),
        ClientProjectionStatus::Empty,
    ));

    // Read-only to controlling, then a self-grant on the same idle Viewer and geometry.
    for sequence in [1, 2] {
        assert_eq!(
            ui.request_takeover(),
            ClientActionDisposition::DismissNavigator
        );
        assert!(!ui.forward(b"never replay this"));
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Refresh(marker) = lease.record() else {
            panic!("takeover barrier");
        };
        let operation = marker.takeover().unwrap().clone();
        assert_eq!(
            mailbox
                .mark_refresh_in_flight(operation.generation)
                .unwrap(),
            MutationResult::Applied
        );
        let evidence = TakeoverEvidence {
            operation: operation.clone(),
            result: TakeoverResult::Ready(ControllerId::parse(&format!("{:032x}", 81)).unwrap()),
        };
        assert!(mailbox.retain_takeover_result(&operation, evidence.result.clone()));
        let wait = mailbox
            .publish_event_before(
                WorkerEvent::ControlTakeoverResult { evidence },
                EventDeadline::PlacementOnly(operation.deadlines.forward_cutoff()),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
        mailbox.finish_lease(lease).unwrap();
        assert_eq!(
            ui.view().takeover_status().unwrap().0,
            crate::workbench_state::control_takeover::TakeoverStatus::WaitingForFrame
        );
        assert!(!ui.forward(b"still waiting"));
        poll_snapshot(&mut ui, &mailbox, &clock, sequence);
        assert_eq!(
            ui.state.as_ref().unwrap().focus_pane_ref(),
            Some(key.clone())
        );
        for bytes in [b"x".as_slice(), b"\x1b[200~paste\x1b[201~", b"\x03"] {
            assert!(ui.forward(bytes));
            let input = mailbox.try_recv().unwrap().unwrap();
            assert!(
                matches!(input.record(), QueuedCommand::Input(tagged) if tagged.key() == &key && tagged.value().as_bytes() == bytes)
            );
            mailbox.finish_lease(input).unwrap();
        }
        assert!(
            mailbox.try_recv().unwrap().is_none(),
            "rejected bytes never reappear"
        );
    }
}

#[test]
fn former_controller_accepts_host_snapshot_geometry_without_local_resize_or_authority_change() {
    let (mut ui, mailbox, clock, _) = super::super::tests::viewport::controlling_ui();
    let route = inspect_terminal_frame_route(SNAPSHOT).unwrap();
    let key = ui.state.as_ref().unwrap().focus_pane_ref().unwrap();
    let mode = ui.state.as_ref().unwrap().controller_mode().cloned();
    let accepted_size = ui.state.as_ref().unwrap().last_accepted_geometry(&key);
    let frame = decode_terminal_text_frame(SNAPSHOT).unwrap();
    assert_ne!(accepted_size.unwrap().columns(), frame.geometry.columns);
    assert_ne!(accepted_size.unwrap().rows(), frame.geometry.rows);
    ui.projections.push(KeyedProjection::for_test(
        key.clone(),
        TerminalProjection::new(route.attachment_id, route.host_epoch),
        ClientProjectionStatus::Empty,
    ));
    poll_snapshot(&mut ui, &mailbox, &clock, 1);
    assert_eq!(
        ui.projection().unwrap().frame().unwrap().geometry,
        frame.geometry
    );
    assert_eq!(ui.state.as_ref().unwrap().controller_mode().cloned(), mode);
    assert_eq!(
        ui.state.as_ref().unwrap().last_accepted_geometry(&key),
        accepted_size
    );
    assert!(ui.state.as_ref().unwrap().pane_attached(&key));
    assert!(ui.state.as_ref().unwrap().invariant_holds());
}

#[test]
fn read_only_input_notice_shows_view_only_in_the_status_line() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::orchbee_light(),
            Theme::orchbee_dark(),
        ] {
            let (ui, _, _, _) = ready_ui();
            let mut view = ClientView::new(120);
            view.report_terminal_input_unavailable();
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        locale,
                        Some(ui.view()),
                        &theme,
                        &mut HitMapBuilder::new(),
                    );
                })
                .unwrap();
            let status: String = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .skip(120 * 39)
                .map(|cell| cell.symbol())
                .collect();
            let readable: String = status.chars().filter(|c| c.is_alphanumeric()).collect();
            let view_only: String = locale
                .view_only()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect();
            assert!(readable.contains(&view_only), "{locale:?}: {status}");
        }
    }
}

#[test]
fn control_takeover_status_renders_both_locales_themes_and_size_tiers() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::orchbee_light(),
            Theme::orchbee_dark(),
        ] {
            for (width, height) in [(80, 24), (120, 40), (60, 16), (24, 8), (8, 3)] {
                let (mut ui, mailbox, _, elapsed) = ready_ui();
                let cursors = mailbox.cursors();
                assert!(!ui.forward(b"typing does not take control"));
                assert_eq!(mailbox.cursors(), cursors);
                assert_eq!(
                    ui.apply_client_action(ClientAction::RequestControlTakeover),
                    ClientActionDisposition::DismissNavigator
                );
                assert!(
                    ui.view().takeover_status().is_none(),
                    "pending indication waits 200ms"
                );
                elapsed.store(200, Ordering::SeqCst);
                ui.tick();
                assert_eq!(
                    ui.view().takeover_status().unwrap().0,
                    crate::workbench_state::control_takeover::TakeoverStatus::Pending
                );
                assert!(!ui.forward(b"no queued replay"));
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal
                    .draw(|frame| {
                        crate::tui_render::render_terminal_shell(
                            frame,
                            &ClientView::new(width),
                            locale,
                            Some(ui.view()),
                            &theme,
                            &mut HitMapBuilder::new(),
                        );
                    })
                    .unwrap();
                if width == 120
                    && height == 40
                    && locale == Locale::EnUs
                    && theme.name == Theme::terminal_native().name
                {
                    capture_screen(
                        &mut terminal,
                        "alignment-status-takeover-pending-120x40-en-terminal-native",
                        locale,
                        &theme,
                    );
                }
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect();
                assert!(!text.contains('\u{1b}'));
                assert!(ui.state.as_ref().unwrap().invariant_holds());
            }
        }
    }
}
