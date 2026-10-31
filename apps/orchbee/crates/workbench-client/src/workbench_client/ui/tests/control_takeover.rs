use super::super::tests::move_membership::ready_ui;
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
        assert_eq!(
            ui.view().takeover_status().unwrap().0,
            crate::workbench_state::control_takeover::TakeoverStatus::Ready
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
fn read_only_input_notice_preserves_the_takeover_hint_in_the_status_line() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::one_light(),
            Theme::one_dark(),
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
            assert!(
                readable.contains(match locale {
                    Locale::EnUs => "viewonly",
                    Locale::ZhCn => "仅查看",
                }),
                "{locale:?}: {status}"
            );
            assert!(readable.contains("Ctrlbt"), "{locale:?}: {status}");
        }
    }
}

#[test]
fn control_takeover_status_renders_both_locales_themes_and_size_tiers() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::one_light(),
            Theme::one_dark(),
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
                if width == 120 {
                    let readable: String = text.chars().filter(|c| c.is_alphanumeric()).collect();
                    assert!(
                        readable.contains(match locale {
                            Locale::EnUs => "Takingcontrol",
                            Locale::ZhCn => "正在接管",
                        }),
                        "{locale:?}: {text}"
                    );
                }
                assert!(!text.contains('\u{1b}'));
                assert!(ui.state.as_ref().unwrap().invariant_holds());
            }
        }
    }
}
