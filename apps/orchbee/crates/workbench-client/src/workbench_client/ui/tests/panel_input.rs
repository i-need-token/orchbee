use super::super::tests::move_membership::ready_ui;
use super::super::tests::publication_hint::ready_split_ui;
use super::super::*;
use crate::client_view::{ClientKey, ClientView};
use crate::terminal_input::{
    DispatchOutcome, InputDispatcher, TerminalInputPort, TerminalInputUnavailable,
};
use crate::workbench_state::{CarrierFailure, PanelIntent};
use orchbee_workbench_protocol::{WorkbenchPanelSplitDirection, WorkbenchSnapshotOutcome};
use std::{
    io,
    time::{Duration, Instant},
};

/// Delegates `TerminalInputPort` onto `WorkbenchClientUi` exactly the way the
/// `WorkbenchClient` production impl does, so these witnesses run the real
/// dispatcher, confirmation capture, modal epoch, and mailbox admission
/// without spawning a session worker.
struct ClientInputPort<'a>(&'a mut WorkbenchClientUi);

impl TerminalInputPort for ClientInputPort<'_> {
    fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
        self.0.capture_claims()
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::WorkbenchCaptureOwner,
    ) -> Option<crate::workbench_client::CaptureContext> {
        self.0.capture_context(owner)
    }

    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.0
            .forward(input)
            .then_some(())
            .ok_or(TerminalInputUnavailable)
    }

    fn apply_client_action(
        &mut self,
        command: crate::workbench_client::InputAction,
    ) -> Option<ClientActionDisposition> {
        Some(self.0.apply_client_action(command))
    }

    fn surface_input(&mut self, input: crate::terminal_input::SurfaceInput<'_>) -> bool {
        use crate::terminal_input::SurfaceInput;
        match input {
            SurfaceInput::TabClose { .. }
            | SurfaceInput::PanelClose { .. }
            | SurfaceInput::WorkspaceClose { .. }
            | SurfaceInput::Rename { .. }
            | SurfaceInput::Setup { .. } => self.0.surface_input(input),
            _ => false,
        }
    }
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{CloseDecision, RenameAction, SetupAction, SurfaceAction};
        match action {
            SurfaceAction::Close(CloseDecision::Tab { .. })
            | SurfaceAction::Close(CloseDecision::Panel { .. })
            | SurfaceAction::Rename(RenameAction::Cancel(_))
            | SurfaceAction::Rename(RenameAction::Submit(_))
            | SurfaceAction::Setup(SetupAction::Cancel(_))
            | SurfaceAction::Setup(SetupAction::Submit(_)) => self.0.surface_action(action),
            _ => None,
        }
    }
}

fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut WorkbenchClientUi,
    bytes: &[u8],
) -> DispatchOutcome {
    dispatch_at(dispatcher, view, ui, bytes, Instant::now())
}

fn dispatch_at(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut WorkbenchClientUi,
    bytes: &[u8],
    now: Instant,
) -> DispatchOutcome {
    let mut port = ClientInputPort(ui);
    dispatcher
        .dispatch(
            view,
            bytes,
            &mut port,
            now,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap()
}

/// The decoder holds a lone `0x1b` for a bounded wait to disambiguate it from
/// Alt chords and CSI/SS3 frames; only an idle boundary past the deadline
/// emits the Esc item. Tests advance the supplied timestamp instead of sleeping.
fn settle(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut WorkbenchClientUi,
    now: Instant,
) {
    let mut port = ClientInputPort(ui);
    dispatcher
        .synchronize(view, &mut port, now, &mut |_| Ok::<(), io::Error>(()))
        .unwrap();
}

fn confirmation_drawn(ui: &WorkbenchClientUi) -> bool {
    ui.state
        .as_ref()
        .is_some_and(|state| state.view().close_confirmation().is_some())
}

fn panel_confirmation_drawn(ui: &WorkbenchClientUi) -> bool {
    ui.state
        .as_ref()
        .is_some_and(|state| state.view().panel_close_confirmation().is_some())
}

fn panel_token(ui: &WorkbenchClientUi) -> PanelCloseConfirmationToken {
    ui.state
        .as_ref()
        .unwrap()
        .view()
        .panel_close_confirmation()
        .unwrap()
        .token()
}

fn draw_panel_prompt(ui: &mut WorkbenchClientUi) {
    let token = panel_token(ui);
    ui.after_close_draw(
        DrawReceipt::default().with_panel_close(Some(ModalPresentation::Presented(token))),
    );
}

#[test]
fn painted_tab_and_panel_buttons_require_an_uninterrupted_release_and_capture_its_tail() {
    use crate::tui_hit_map::{HitMapBuilder, HitTarget};
    use crate::{localization::Locale, tui_theme::Theme};
    use ratatui::{Terminal, backend::TestBackend};
    for (kind, action) in [
        (0, ClientAction::RequestCloseTab),
        (1, ClientAction::ClosePanel),
    ] {
        for cancel in [false, true] {
            for interrupt in [false, true] {
                for locale in [Locale::EnUs, Locale::ZhCn] {
                    for (width, height) in [(80, 24), (120, 40)] {
                        let (mut ui, mailbox, _, _) = ready_ui();
                        let mut view = ClientView::new(width);
                        let mut dispatcher = InputDispatcher::default();
                        dispatcher.set_pointer_capture(
                            &mut view,
                            &mut ClientInputPort(&mut ui),
                            true,
                        );
                        ui.apply_client_action(action.clone());
                        dispatcher
                            .synchronize(
                                &mut view,
                                &mut ClientInputPort(&mut ui),
                                Instant::now(),
                                &mut |_| Ok::<_, io::Error>(()),
                            )
                            .unwrap();
                        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                        let mut hits = HitMapBuilder::new();
                        let mut receipt = None;
                        terminal
                            .draw(|frame| {
                                receipt = Some(crate::tui_render::render_terminal_shell(
                                    frame,
                                    &view,
                                    locale,
                                    Some(ui.view()),
                                    &Theme::terminal_native(),
                                    &mut hits,
                                ));
                            })
                            .unwrap();
                        ui.after_close_draw(receipt.unwrap());
                        let map = hits.finish();
                        let (x, y) = (0..height)
                            .flat_map(|y| (0..width).map(move |x| (x, y)))
                            .find(|&(x, y)| match (kind, map.hit_target_at(x, y)) {
                                (0, Some(HitTarget::TabClose { confirm, .. }))
                                | (1, Some(HitTarget::PanelClose { confirm, .. })) => {
                                    *confirm != cancel
                                }
                                _ => false,
                            })
                            .expect("visible decision button");
                        dispatcher.publish_hit_map(map);
                        dispatch(
                            &mut dispatcher,
                            &mut view,
                            &mut ui,
                            format!("\x1b[<0;{};{}M", x + 1, y + 1).as_bytes(),
                        );
                        assert!(mailbox.try_recv().unwrap().is_none(), "press cannot commit");
                        if interrupt {
                            dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b[O");
                        }
                        dispatch(
                            &mut dispatcher,
                            &mut view,
                            &mut ui,
                            format!("\x1b[<0;{};{}mTAIL\x02X", x + 1, y + 1).as_bytes(),
                        );
                        assert_eq!(
                            mailbox.try_recv().unwrap().is_some(),
                            !cancel && !interrupt,
                            "kind={kind}, cancel={cancel}, interrupt={interrupt}, {width}x{height}, {locale:?}"
                        );
                        assert!(mailbox.try_recv().unwrap().is_none());
                        assert!(!view.prefix_active());
                    }
                }
            }
        }
    }
}

#[test]
fn drawn_close_confirmation_captures_panel_prefix_keys_and_the_cancel_packet_tail() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestCloseTab),
        ClientActionDisposition::DismissNavigator
    );
    assert!(confirmation_drawn(&ui));

    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x02X\x02|\x02H\x02-");
    assert!(confirmation_drawn(&ui));
    assert!(!view.prefix_active());
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "panel prefix keys must not reach the mailbox while the close confirmation owns input"
    );

    // A ready encoded Escape continuation survives a busy frame; the tail of
    // its decision packet stays captured and cannot re-arm the prefix.
    let mut now = Instant::now();
    dispatch_at(&mut dispatcher, &mut view, &mut ui, b"\x1b", now);
    now += Duration::from_millis(60);
    dispatch_at(&mut dispatcher, &mut view, &mut ui, b"[27u\x02X", now);
    assert!(!confirmation_drawn(&ui));
    assert!(!view.prefix_active());
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn open_rename_draft_owns_panel_prefix_bytes_as_draft_text() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestRenameTab),
        ClientActionDisposition::DismissNavigator
    );
    let token = ui.rename_input_context().unwrap().token();
    ui.after_rename_draw(Some(ModalPresentation::Presented(token)));

    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x02X\x02|");
    assert!(!view.prefix_active());
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "panel prefix keys must not reach the mailbox while the rename draft owns input"
    );
    // The captured bytes land in the draft, replacing its selected prefill, instead of
    // reaching the prefix layer.
    assert_eq!(
        ui.state.as_ref().unwrap().rename_draft().unwrap().text(),
        "X|"
    );

    // The draft retains its own submit path once drawn; the queued command is
    // a Rename.
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    let lease = mailbox.try_recv().unwrap().expect("rename command queued");
    assert!(matches!(lease.record(), QueuedCommand::Rename(_)));
}

#[test]
fn close_panel_request_queues_no_command_before_a_drawn_confirmation() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    assert!(
        panel_confirmation_drawn(&ui),
        "ClosePanel arms a confirmation instead of queueing"
    );
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "ClosePanel must not queue a command before a drawn confirmation"
    );

    // A decision key before any matching draw receipt executes nothing.
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    assert!(panel_confirmation_drawn(&ui));
    assert!(mailbox.try_recv().unwrap().is_none());

    // A receipt that carried no panel prompt token is not a drawn prompt.
    ui.after_close_draw(DrawReceipt::default());
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    assert!(panel_confirmation_drawn(&ui));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn lowercase_n_does_not_cancel_a_drawn_tab_close_confirmation() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestCloseTab),
        ClientActionDisposition::DismissNavigator
    );
    let token = ui
        .state
        .as_ref()
        .unwrap()
        .view()
        .close_confirmation()
        .unwrap()
        .token();
    ui.after_close_draw(DrawReceipt::new(
        Some(ModalPresentation::Presented(token)),
        None,
    ));
    dispatch(&mut dispatcher, &mut view, &mut ui, b"n");
    assert!(
        confirmation_drawn(&ui),
        "n is a non-decision key: the drawn confirmation must stay armed"
    );
    let now = Instant::now();
    dispatch_at(&mut dispatcher, &mut view, &mut ui, b"\x1b", now);
    settle(
        &mut dispatcher,
        &mut view,
        &mut ui,
        now + Duration::from_millis(60),
    );
    assert!(!confirmation_drawn(&ui));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn drawn_panel_close_confirmation_and_enter_queues_exactly_one_panel_close() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let state = ui.state.as_ref().unwrap();
    let selected = state.ready().unwrap().selected().clone();
    let tab = &state.cache().tabs()[selected.index()];
    assert_eq!(
        tab.panel_count(),
        1,
        "the fixture selects a single-leaf Tab"
    );
    let only_leaf = tab.panel(0).unwrap().panel_id().clone();

    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    assert!(panel_confirmation_drawn(&ui));
    // A single-leaf close dissolves the whole Tab: the modal must say so.
    assert!(
        ui.state
            .as_ref()
            .unwrap()
            .view()
            .panel_close_confirmation()
            .unwrap()
            .dissolves_tab()
    );
    draw_panel_prompt(&mut ui);
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");

    assert!(!panel_confirmation_drawn(&ui));
    let lease = mailbox.try_recv().unwrap().expect("panel command queued");
    let QueuedCommand::Panel(marker) = lease.record() else {
        panic!("one Panel marker, never a Tab close");
    };
    let operation = marker.operation();
    assert_eq!(operation.intent(), &PanelIntent::Close);
    assert_eq!(operation.panel_id(), &only_leaf);
    assert_eq!(operation.target(), selected.key());
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "exactly one panel close is queued"
    );
}

#[test]
fn split_leaf_panel_close_discloses_a_non_dissolving_impact() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_split_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    // The split fixture leaves scheduled work leased; settle it before counting.
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).expect("finish fixture lease");
    }
    let state = ui.state.as_ref().unwrap();
    let selected = state.ready().unwrap().selected().clone();
    let tab = &state.cache().tabs()[selected.index()];
    assert_eq!(tab.panel_count(), 2, "the fixture selects a split Tab");
    let leaf = tab.panel(0).unwrap().panel_id().clone();

    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    assert!(
        !ui.state
            .as_ref()
            .unwrap()
            .view()
            .panel_close_confirmation()
            .unwrap()
            .dissolves_tab(),
        "closing one of two leaves keeps the Tab alive"
    );
    draw_panel_prompt(&mut ui);
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    let lease = mailbox.try_recv().unwrap().expect("panel command queued");
    let QueuedCommand::Panel(marker) = lease.record() else {
        panic!("one Panel marker");
    };
    assert_eq!(marker.operation().intent(), &PanelIntent::Close);
    assert_eq!(marker.operation().panel_id(), &leaf);
}

#[test]
fn panel_close_confirmation_swallows_non_decision_keys_and_the_cancel_packet_tail() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    draw_panel_prompt(&mut ui);

    // Uppercase Y, lowercase n/N, ordinary keys, and prefix bytes are
    // non-decisions that the armed confirmation swallows.
    dispatch(&mut dispatcher, &mut view, &mut ui, b"yYnNq\x02X\x02x");
    for nondecision in [
        b"\x1b[13;2u".as_slice(),
        b"\x1b[13;5u",
        b"\x1b[200~\r\x1b[201~",
    ] {
        dispatch(&mut dispatcher, &mut view, &mut ui, nondecision);
        assert!(panel_confirmation_drawn(&ui));
        assert!(mailbox.try_recv().unwrap().is_none());
    }
    assert!(panel_confirmation_drawn(&ui));
    assert!(!confirmation_drawn(&ui));
    assert!(!view.prefix_active());
    assert!(mailbox.try_recv().unwrap().is_none());

    // A ready encoded Escape continuation cancels after the busy frame; the
    // rest of that decision packet stays captured and cannot re-arm.
    let mut now = Instant::now();
    dispatch_at(&mut dispatcher, &mut view, &mut ui, b"\x1b", now);
    now += Duration::from_millis(60);
    dispatch_at(&mut dispatcher, &mut view, &mut ui, b"[27u\x02X", now);
    assert!(!panel_confirmation_drawn(&ui));
    assert!(!view.prefix_active());
    assert!(mailbox.try_recv().unwrap().is_none());

    // A later independent packet dispatches normally: X re-arms cleanly.
    now += Duration::from_millis(60);
    dispatch_at(&mut dispatcher, &mut view, &mut ui, b"\x02X", now);
    assert!(panel_confirmation_drawn(&ui));
}

#[test]
fn resize_invalidation_blocks_a_drawn_panel_close_until_redrawn() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    draw_panel_prompt(&mut ui);

    // Resize drops the draw receipt; Enter must not execute against it.
    ui.invalidate_close_display();
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    assert!(panel_confirmation_drawn(&ui));
    assert!(mailbox.try_recv().unwrap().is_none());

    // A fresh matching draw re-arms execution; Esc still cancels instead,
    // resolving after its bounded wait at the next idle boundary.
    draw_panel_prompt(&mut ui);
    let now = Instant::now();
    dispatch_at(&mut dispatcher, &mut view, &mut ui, b"\x1b", now);
    settle(
        &mut dispatcher,
        &mut view,
        &mut ui,
        now + Duration::from_millis(60),
    );
    assert!(!panel_confirmation_drawn(&ui));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn retired_client_drops_the_panel_close_confirmation() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    draw_panel_prompt(&mut ui);
    ui.state.as_mut().unwrap().retire(CarrierFailure::Closed);
    assert!(!panel_confirmation_drawn(&ui));
    assert!(!ui.capture_confirmation_key(ClientKey::Enter));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn carrier_retirement_drops_the_drawn_panel_close_confirmation() {
    let (mut ui, _mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    draw_panel_prompt(&mut ui);
    ui.retire(CarrierFailure::Closed);
    assert!(
        !panel_confirmation_drawn(&ui),
        "carrier retirement drops the armed confirmation outright"
    );
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    assert!(!panel_confirmation_drawn(&ui));
}

#[test]
fn read_only_attachment_admits_panel_operations_while_terminal_input_stays_gated() {
    for (action, check) in [
        (ClientAction::SplitPanel(PanelDirection::Right), true),
        (ClientAction::ClosePanel, true),
        // A single-leaf Tab has no directional neighbor: Move plans no target
        // and admission fails closed without surfacing a notice.
        (ClientAction::MovePanel(PanelDirection::Right), false),
    ] {
        let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
        let state = ui.state.as_ref().unwrap();
        assert!(matches!(
            state.ready().unwrap().mode(),
            ControllerMode::ReadOnly
        ));
        assert!(
            !ui.forward(b"ordinary"),
            "ReadOnly keeps terminal input gated"
        );

        let disposition = ui.apply_client_action(action.clone());
        if check {
            assert_eq!(disposition, ClientActionDisposition::DismissNavigator);
            if matches!(action, ClientAction::ClosePanel) {
                // Close arms a confirmation; only a drawn prompt plus the exact
                // unmodified Enter queues the close.
                assert!(mailbox.try_recv().unwrap().is_none());
                assert!(panel_confirmation_drawn(&ui));
                draw_panel_prompt(&mut ui);
                assert!(ui.capture_confirmation_key(ClientKey::Enter));
            }
            let Some(lease) = mailbox.try_recv().unwrap() else {
                panic!("panel command queued for {action:?}");
            };
            let QueuedCommand::Panel(marker) = lease.record() else {
                panic!("one Panel marker");
            };
            match &action {
                ClientAction::SplitPanel(direction) => {
                    let expected = match direction {
                        PanelDirection::Left => WorkbenchPanelSplitDirection::Left,
                        PanelDirection::Right => WorkbenchPanelSplitDirection::Right,
                        PanelDirection::Up => WorkbenchPanelSplitDirection::Above,
                        PanelDirection::Down => WorkbenchPanelSplitDirection::Below,
                    };
                    assert!(matches!(
                        marker.operation().intent(),
                        PanelIntent::Split { direction: wire, .. } if *wire == expected
                    ));
                }
                _ => assert_eq!(marker.operation().intent(), &PanelIntent::Close),
            }
        } else {
            assert_eq!(disposition, ClientActionDisposition::KeepNavigator);
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
}

/// A projection refresh is a real external mutation: it bumps the baseline
/// revision the armed confirmation froze, so the next captured key settles
/// the stale capture as cancelled with a bounded notice rather than deciding.
#[test]
fn drifted_panel_close_target_settles_cancelled_before_the_next_key() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    let token = ui
        .view()
        .panel_close_confirmation()
        .expect("confirmation is armed")
        .token();
    let state = ui.state.as_mut().unwrap();
    assert!(state.start_pending_refresh(2));
    let prepared = state.prepare_refresh(
        2,
        WorkbenchSnapshotOutcome::Available {
            snapshot: super::super::tests::membership::snapshot(2, 2),
        },
    );
    state.apply_prepared_refresh(prepared, std::time::Instant::now());
    assert!(ui.capture_panel_confirmation_key(token, ClientKey::Enter));
    assert!(!panel_confirmation_drawn(&ui));
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::ConfirmationCancelled)
    );
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "the stale y never reaches the carrier"
    );
}

/// The same stale settlement runs after any applied mutation inside the tick,
/// before the next frame or key is processed.
#[test]
fn tick_settles_a_drifted_panel_close_confirmation() {
    let (mut ui, _mailbox, _clock, _elapsed) = ready_ui();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    let state = ui.state.as_mut().unwrap();
    assert!(state.start_pending_refresh(2));
    let prepared = state.prepare_refresh(
        2,
        WorkbenchSnapshotOutcome::Available {
            snapshot: super::super::tests::membership::snapshot(2, 2),
        },
    );
    state.apply_prepared_refresh(prepared, std::time::Instant::now());
    ui.tick();
    assert!(!panel_confirmation_drawn(&ui));
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::ConfirmationCancelled)
    );
}

/// A new prompt that cannot complete its first paint is refused with the geometry notice; it
/// never becomes an invisible input owner or an explicit-cancellation notice.
#[test]
fn first_unpresentable_panel_close_draw_is_refused() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    ui.after_close_draw(
        DrawReceipt::default().with_panel_close(Some(ModalPresentation::NotPresented)),
    );
    assert!(
        !panel_confirmation_drawn(&ui),
        "a first draw that cannot present the prompt refuses the capture"
    );
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::DialogRefused)
    );
    assert!(!ui.capture_confirmation_key(ClientKey::Enter));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn presented_panel_close_suspends_hidden_input_and_resumes_with_the_same_token() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        ClientActionDisposition::DismissNavigator
    );
    let token = panel_token(&ui);
    draw_panel_prompt(&mut ui);
    ui.after_close_draw(
        DrawReceipt::default().with_panel_close(Some(ModalPresentation::NotPresented)),
    );
    let suspended = ui.view().panel_close_confirmation().unwrap();
    assert_eq!(suspended.token(), token);
    assert!(!suspended.interactive());
    ui.click_panel_close(token, true);
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    assert!(mailbox.try_recv().unwrap().is_none());

    ui.after_close_draw(
        DrawReceipt::default().with_panel_close(Some(ModalPresentation::Presented(token))),
    );
    assert!(ui.view().panel_close_confirmation().unwrap().interactive());
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    assert!(mailbox.try_recv().unwrap().is_some());
}

/// Arming and deciding inside one input packet can never execute: Enter
/// arrives while the confirmation has no drawn receipt yet.
#[test]
fn panel_close_arm_and_decision_in_one_packet_cannot_execute() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x02X\r");
    assert!(
        panel_confirmation_drawn(&ui),
        "the y lands before any draw and stays a swallowed non-decision"
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

/// The Tab-close confirmation settles the same way when its frozen target
/// drifts: cancel with notice, never an armed-but-stale capture.
#[test]
fn drifted_close_target_settles_cancelled_before_the_next_key() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestCloseTab),
        ClientActionDisposition::DismissNavigator
    );
    let state = ui.state.as_mut().unwrap();
    assert!(state.start_pending_refresh(2));
    let prepared = state.prepare_refresh(
        2,
        WorkbenchSnapshotOutcome::Available {
            snapshot: super::super::tests::membership::snapshot(2, 2),
        },
    );
    state.apply_prepared_refresh(prepared, std::time::Instant::now());
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    assert!(!confirmation_drawn(&ui));
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::ConfirmationCancelled)
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

/// A blocked first draw refuses a new Tab-close prompt with the geometry notice.
#[test]
fn first_unpresentable_close_draw_is_refused() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestCloseTab),
        ClientActionDisposition::DismissNavigator
    );
    ui.after_close_draw(DrawReceipt::new(
        Some(ModalPresentation::NotPresented),
        None,
    ));
    assert!(!confirmation_drawn(&ui));
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::DialogRefused)
    );
    assert!(!ui.capture_confirmation_key(ClientKey::Enter));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn presented_tab_close_suspends_hidden_input_and_resumes_with_the_same_token() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestCloseTab),
        ClientActionDisposition::DismissNavigator
    );
    let token = ui.view().close_confirmation().unwrap().token();
    ui.after_close_draw(DrawReceipt::new(
        Some(ModalPresentation::Presented(token)),
        None,
    ));
    ui.after_close_draw(DrawReceipt::new(
        Some(ModalPresentation::NotPresented),
        None,
    ));
    let suspended = ui.view().close_confirmation().unwrap();
    assert_eq!(suspended.token(), token);
    assert!(!suspended.interactive());
    ui.click_tab_close(token, true);
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    assert!(mailbox.try_recv().unwrap().is_none());

    ui.after_close_draw(DrawReceipt::new(
        Some(ModalPresentation::Presented(token)),
        None,
    ));
    assert!(ui.view().close_confirmation().unwrap().interactive());
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\r");
    assert!(mailbox.try_recv().unwrap().is_some());
}

/// A blocked first draw refuses a new rename draft with the geometry notice.
#[test]
fn first_unpresentable_rename_draft_is_refused() {
    let (mut ui, _mailbox, _clock, _elapsed) = ready_ui();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestRenameTab),
        ClientActionDisposition::DismissNavigator
    );
    assert!(ui.rename_input_context().is_some());
    ui.after_rename_draw(Some(ModalPresentation::NotPresented));
    assert!(ui.rename_input_context().is_none());
    assert_eq!(
        ui.view().notice(),
        Some(WorkbenchClientNotice::DialogRefused)
    );
}

#[test]
fn presented_rename_draft_suspends_edits_and_resumes_with_the_same_text() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestRenameTab),
        ClientActionDisposition::DismissNavigator
    );
    let token = ui.rename_input_context().unwrap().token();
    ui.after_rename_draw(Some(ModalPresentation::Presented(token)));
    assert!(ui.edit_rename_draft(token, RenameEdit::Append(" preserved")));
    let text = ui
        .state
        .as_ref()
        .unwrap()
        .rename_draft()
        .unwrap()
        .text()
        .to_owned();

    ui.after_rename_draw(Some(ModalPresentation::NotPresented));
    assert_eq!(ui.rename_input_context().unwrap().token(), token);
    assert!(!ui.edit_rename_draft(token, RenameEdit::Append(" hidden")));
    assert_eq!(
        ui.submit_rename_draft(token),
        ClientActionDisposition::KeepNavigator
    );
    assert_eq!(
        ui.state.as_ref().unwrap().rename_draft().unwrap().text(),
        text
    );
    assert!(mailbox.try_recv().unwrap().is_none());

    ui.after_rename_draw(Some(ModalPresentation::Presented(token)));
    assert!(ui.edit_rename_draft(token, RenameEdit::Append(" resumed")));
    assert!(
        ui.state
            .as_ref()
            .unwrap()
            .rename_draft()
            .unwrap()
            .text()
            .ends_with(" resumed")
    );
}

/// A blocked first setup draw refuses that opening attempt with an exact
/// presentation notice instead of retaining a hidden input owner.
#[test]
fn first_unpresentable_setup_draft_is_refused_with_notice() {
    let (mut ui, _mailbox, _clock, _elapsed) = ready_ui();
    ui.apply_client_action(ClientAction::RequestWorkspaceCreate);
    assert!(ui.view().setup_draft().is_some());
    ui.after_setup_draw(Some(ModalPresentation::NotPresented));
    assert!(ui.view().setup_draft().is_none());
    assert_eq!(
        ui.view().setup_status(),
        Some(SetupStatus::PresentationUnavailable)
    );
}

#[test]
fn open_rename_draft_consumes_editor_keys_and_atomic_paste_without_corruption() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestRenameTab),
        ClientActionDisposition::DismissNavigator
    );
    let token = ui.rename_input_context().unwrap().token();
    ui.after_rename_draw(Some(ModalPresentation::Presented(token)));

    // Unmodified editing keys reach the real draft as consumed LineEditor
    // edits: the cursor moves for the arrows/Home/End, Delete is a no-op at
    // the end, and Ctrl+W word-backspace removes the trailing word. The
    // lease stays armed — none of this cancels or forwards.
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"\x1b[D\x1b[C\x1b[H\x1b[F\x1b[3~\x17\x01\x05",
    );
    let draft_text = |ui: &WorkbenchClientUi| {
        ui.state
            .as_ref()
            .unwrap()
            .rename_draft()
            .unwrap()
            .text()
            .to_owned()
    };
    assert_eq!(draft_text(&ui), "Tab ");
    assert!(ui.rename_input_context().is_some());

    // Modified and unbound sequences are consumed whole: no fault, no cancel,
    // no forward.
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"\x1b[1;5D\x1b[A\x1b[Z",
    );
    assert_eq!(draft_text(&ui), "Tab ");
    assert!(
        !ui.state
            .as_ref()
            .unwrap()
            .rename_draft()
            .unwrap()
            .input_fault()
    );
    assert!(ui.rename_input_context().is_some());
    assert!(mailbox.try_recv().unwrap().is_none());

    // An atomic paste lands in the draft even when it carries bytes that
    // would otherwise be prefix, selection, or submit keys.
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"\x1b[200~\x02x[]\x1b[201~",
    );
    assert_eq!(draft_text(&ui), "Tab \x02x[]");
    assert!(
        !ui.state
            .as_ref()
            .unwrap()
            .rename_draft()
            .unwrap()
            .input_fault()
    );
    assert!(mailbox.try_recv().unwrap().is_none());
}

/// Regression (ORC-362): of two Panels closed in quick succession, the second close confirmed
/// while the first close's status read was in flight was refused as busy and dropped without a
/// word. One Enter on a drawn confirmation submits the close exactly once, or leaves a visible
/// reason.
#[test]
fn a_panel_close_confirmed_while_another_close_reads_is_never_dropped_silently() {
    use crate::exchange_values::workbench::panel_close::WorkbenchPanelCloseAccepted;
    use crate::workbench_client::mailbox::{CommandLease, MutationResult};
    use crate::workbench_state::{CloseOperationKind, CloseResult, PanelResult};
    use orchbee_workbench_protocol::{
        PanelNode, WorkbenchTabCloseAttemptGeneration, WorkbenchTabClosePhase,
        WorkbenchTabCloseReadOutcome, WorkbenchTabCloseStatus,
    };

    // Two Panels; the second is unbound, so the focus can move onto it without an attach.
    let mut source = super::super::tests::publication_hint::split_snapshot(1);
    let PanelNode::Split { children, .. } = &mut source.terminal_contexts[0].tabs[0].root else {
        unreachable!("the split fixture");
    };
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut children[1].node
    else {
        unreachable!("the split fixture's second leaf");
    };
    primary_presentation.binding = None;
    let (mut ui, mailbox, clock, _elapsed) =
        super::super::tests::move_membership::ready_ui_with_snapshot(source.clone());

    // Plays the worker for everything this test does not hold: a Poll finishes unanswered and a
    // publication read finds the Workbench unchanged. Returns the first other command.
    let serve = |ui: &mut WorkbenchClientUi| -> Option<CommandLease> {
        while let Some(lease) = mailbox.try_recv().unwrap() {
            match lease.record() {
                QueuedCommand::Poll(_) => mailbox.finish_lease(lease).unwrap(),
                QueuedCommand::Refresh(marker) => {
                    let (generation, deadline) = (marker.generation(), marker.deadline());
                    assert_eq!(
                        mailbox.mark_refresh_in_flight(generation).unwrap(),
                        MutationResult::Applied
                    );
                    let wait = mailbox
                        .publish_refresh_result_before(
                            generation,
                            WorkbenchSnapshotOutcome::Available {
                                snapshot: source.clone(),
                            },
                            deadline,
                            &clock,
                        )
                        .unwrap();
                    ui.tick();
                    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
                    mailbox.finish_lease(lease).unwrap();
                }
                _ => return Some(lease),
            }
        }
        None
    };
    let confirm_focused_panel_close = |ui: &mut WorkbenchClientUi| {
        assert_eq!(
            ui.apply_client_action(ClientAction::ClosePanel),
            ClientActionDisposition::DismissNavigator
        );
        draw_panel_prompt(ui);
        assert!(ui.capture_confirmation_key(ClientKey::Enter));
    };
    assert!(serve(&mut ui).is_none());

    // The first close goes out, and the Server accepts it: its Panel leaves and the focus moves on.
    confirm_focused_panel_close(&mut ui);
    let lease = serve(&mut ui).expect("the first Panel close");
    let QueuedCommand::Panel(marker) = lease.record() else {
        panic!("one Panel marker");
    };
    let first = marker.operation().clone();
    assert_eq!(first.intent(), &PanelIntent::Close);
    assert_eq!(
        mailbox.mark_panel_in_flight(&first).unwrap(),
        MutationResult::Applied
    );
    let wait = mailbox
        .publish_panel_result_before(
            first.clone(),
            PanelResult::CloseAccepted(WorkbenchPanelCloseAccepted {
                close_request_id: first.request_id().clone(),
                terminal_context_id: first.baseline().terminal_context_id().unwrap().clone(),
                tab_id: first.target().tab_id().clone(),
                panel_id: first.panel_id().clone(),
                accepted_revision: first.baseline().revision(),
            }),
            first.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();
    ui.tick();
    let second_panel = ui.state.as_ref().unwrap().focus_panel_id().unwrap().clone();
    assert_ne!(&second_panel, first.panel_id());

    // The retained first close is read by its operation; that read is in flight.
    let read_lease = serve(&mut ui).expect("the first close's status read");
    let QueuedCommand::Close(marker) = read_lease.record() else {
        panic!("one Close marker");
    };
    let read = marker.operation().clone();
    let CloseOperationKind::Read {
        expected_operation: Some(close_request_id),
        ..
    } = read.kind().clone()
    else {
        panic!("the accepted Panel close is read by its operation");
    };
    assert_eq!(
        mailbox.mark_close_in_flight(&read).unwrap(),
        MutationResult::Applied
    );

    // The second Panel's close is confirmed while that read is still on its way.
    confirm_focused_panel_close(&mut ui);

    // The read answers that the first close is still progressing.
    let wait = mailbox
        .publish_close_result_before(
            read.clone(),
            CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                close_request_id,
                terminal_context_id: read.key().target().terminal_context_id().clone(),
                tab_id: read.key().target().tab_id().clone(),
                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1).unwrap(),
                status: WorkbenchTabCloseStatus::Closing {
                    phase: WorkbenchTabClosePhase::WaitingGrace,
                },
            }),
            read.deadline(),
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(read_lease).unwrap();

    let mut submitted = 0;
    for _ in 0..3 {
        ui.tick();
        while let Some(lease) = serve(&mut ui) {
            if let QueuedCommand::Panel(marker) = lease.record() {
                assert_eq!(marker.operation().intent(), &PanelIntent::Close);
                assert_eq!(marker.operation().panel_id(), &second_panel);
                submitted += 1;
            }
            mailbox.finish_lease(lease).unwrap();
        }
    }
    let reason = ui.view().notice().is_some();
    assert!(
        submitted == 1 || (submitted == 0 && reason),
        "the confirmed second close was submitted {submitted} times with no visible reason"
    );
}

/// Regression (ORC-430): when every Panel of the only Tab was closing and one close stopped, the
/// Panel it showed again took the focus in the Client but not in the mailbox, so the next keystroke
/// or publication retired the Client. A focus that a close settlement moves is the focus input
/// reaches.
#[test]
fn a_panel_a_stopped_close_shows_again_takes_typing_and_keeps_the_client() {
    use crate::exchange_values::workbench::panel_close::WorkbenchPanelCloseAccepted;
    use crate::workbench_client::mailbox::{CommandLease, MutationResult};
    use crate::workbench_state::{CloseOperationKind, CloseResult, PanelResult};
    use orchbee_client_server_protocol::ControllerId;
    use orchbee_workbench_protocol::{
        PanelNode, WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseBlockedReason,
        WorkbenchTabClosePhase, WorkbenchTabCloseReadOutcome, WorkbenchTabCloseStatus,
    };
    use std::sync::atomic::Ordering;

    // One Tab of two Panels; the second is unbound, so the focus can move onto it without an
    // attach.
    let mut source = super::super::tests::publication_hint::split_snapshot(1);
    source.terminal_contexts[0].tabs.truncate(1);
    let PanelNode::Split { children, .. } = &mut source.terminal_contexts[0].tabs[0].root else {
        unreachable!("the split fixture");
    };
    let PanelNode::Panel {
        primary_presentation,
        ..
    } = &mut children[1].node
    else {
        unreachable!("the split fixture's second leaf");
    };
    primary_presentation.binding = None;
    let (mut ui, mailbox, clock, elapsed) =
        super::super::tests::move_membership::ready_ui_with_mode(
            source.clone(),
            ControllerMode::Controlling(ControllerId::parse(&format!("{:032x}", 1)).unwrap()),
        );

    // Plays the worker for everything this test does not hold: a Poll finishes unanswered and a
    // publication read finds the Workbench unchanged. Returns the first other command.
    let serve = |ui: &mut WorkbenchClientUi| -> Option<CommandLease> {
        while let Some(lease) = mailbox.try_recv().unwrap() {
            match lease.record() {
                QueuedCommand::Poll(_) => mailbox.finish_lease(lease).unwrap(),
                QueuedCommand::Refresh(marker) => {
                    let (generation, deadline) = (marker.generation(), marker.deadline());
                    assert_eq!(
                        mailbox.mark_refresh_in_flight(generation).unwrap(),
                        MutationResult::Applied
                    );
                    let wait = mailbox
                        .publish_refresh_result_before(
                            generation,
                            WorkbenchSnapshotOutcome::Available {
                                snapshot: source.clone(),
                            },
                            deadline,
                            &clock,
                        )
                        .unwrap();
                    ui.tick();
                    let _ = mailbox.take_deposited_reply(&wait);
                    mailbox.finish_lease(lease).unwrap();
                }
                _ => return Some(lease),
            }
        }
        None
    };
    // Closes the focused Panel and has the Server accept the close.
    let close_focused_panel = |ui: &mut WorkbenchClientUi| -> WorkbenchPanelId {
        ui.apply_client_action(ClientAction::ClosePanel);
        if panel_confirmation_drawn(ui) {
            draw_panel_prompt(ui);
            assert!(ui.capture_confirmation_key(ClientKey::Enter));
        }
        let lease = serve(ui).expect("the Panel close");
        let QueuedCommand::Panel(marker) = lease.record() else {
            panic!("one Panel marker");
        };
        let operation = marker.operation().clone();
        assert_eq!(
            mailbox.mark_panel_in_flight(&operation).unwrap(),
            MutationResult::Applied
        );
        let wait = mailbox
            .publish_panel_result_before(
                operation.clone(),
                PanelResult::CloseAccepted(WorkbenchPanelCloseAccepted {
                    close_request_id: operation.request_id().clone(),
                    terminal_context_id: operation
                        .baseline()
                        .terminal_context_id()
                        .unwrap()
                        .clone(),
                    tab_id: operation.target().tab_id().clone(),
                    panel_id: operation.panel_id().clone(),
                    accepted_revision: operation.baseline().revision(),
                }),
                operation.deadline(),
                &clock,
            )
            .unwrap();
        ui.tick();
        assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
        mailbox.finish_lease(lease).unwrap();
        ui.tick();
        operation.panel_id().clone()
    };
    // Answers each status read the Client sends with the status `answer` gives the Panel it reads,
    // until `until` has been read.
    let read_until =
        |ui: &mut WorkbenchClientUi,
         until: &WorkbenchPanelId,
         answer: &dyn Fn(&WorkbenchPanelId) -> WorkbenchTabCloseStatus| {
            for _ in 0..8 {
                elapsed.fetch_add(1_000, Ordering::SeqCst);
                ui.tick();
                while let Some(lease) = serve(ui) {
                    let QueuedCommand::Close(marker) = lease.record() else {
                        mailbox.finish_lease(lease).unwrap();
                        continue;
                    };
                    let read = marker.operation().clone();
                    let CloseOperationKind::Read {
                        expected_operation: Some(close_request_id),
                        ..
                    } = read.kind().clone()
                    else {
                        panic!("an accepted Panel close is read by its operation");
                    };
                    let panel = read.key().target().panel_id().unwrap().clone();
                    assert_eq!(
                        mailbox.mark_close_in_flight(&read).unwrap(),
                        MutationResult::Applied
                    );
                    let wait = mailbox
                        .publish_close_result_before(
                            read.clone(),
                            CloseResult::Read(WorkbenchTabCloseReadOutcome::Available {
                                close_request_id,
                                terminal_context_id: read
                                    .key()
                                    .target()
                                    .terminal_context_id()
                                    .clone(),
                                tab_id: read.key().target().tab_id().clone(),
                                attempt_generation: WorkbenchTabCloseAttemptGeneration::new(1)
                                    .unwrap(),
                                status: answer(&panel),
                            }),
                            read.deadline(),
                            &clock,
                        )
                        .unwrap();
                    ui.tick();
                    let _ = mailbox.take_deposited_reply(&wait);
                    mailbox.finish_lease(lease).unwrap();
                    if &panel == until {
                        return;
                    }
                }
            }
            panic!("the Client never read the close of {until:?}");
        };
    let closing = |_: &WorkbenchPanelId| WorkbenchTabCloseStatus::Closing {
        phase: WorkbenchTabClosePhase::WaitingGrace,
    };
    assert!(serve(&mut ui).is_none());

    // The live Panel closes first and the focus moves onto the unbound one, which closes next:
    // every Panel of the only Tab is now closing.
    let live = close_focused_panel(&mut ui);
    read_until(&mut ui, &live, &closing);
    let unbound = close_focused_panel(&mut ui);
    assert_ne!(live, unbound);
    assert!(!ui.retirement_requested());

    // The live Panel's close stops: that Panel shows again and takes the focus.
    let blocked = |panel: &WorkbenchPanelId| {
        if *panel == live {
            WorkbenchTabCloseStatus::CleanupBlocked {
                reason: WorkbenchTabCloseBlockedReason::HostRefused,
                resumable: true,
            }
        } else {
            closing(panel)
        }
    };
    read_until(&mut ui, &live, &blocked);
    assert!(!ui.retirement_requested());
    assert_eq!(ui.state.as_ref().unwrap().focus_panel_id(), Some(&live));

    // Typing reaches the Panel shown again, and a publication keeps the Client connected.
    let typed = ui.forward(b"ls\r");
    assert!(!ui.retirement_requested(), "typing retired the Client");
    assert!(typed, "typing into the focused Panel");
    let lease = serve(&mut ui).expect("the typed input");
    assert!(matches!(
        lease.record(),
        QueuedCommand::Input(tagged) if tagged.key().pane().panel() == &live
    ));
    mailbox.finish_lease(lease).unwrap();
    ui.state.as_mut().unwrap().note_publication_hint();
    ui.tick();
    while let Some(lease) = serve(&mut ui) {
        mailbox.finish_lease(lease).unwrap();
    }
    assert!(
        !ui.retirement_requested(),
        "a publication retired the Client"
    );
}

/// The Tab close confirmation follows the same rule: confirmed while a background publication
/// read holds the slot its submission needs, the close goes out exactly once, or a visible reason
/// shows.
#[test]
fn a_tab_close_confirmed_during_a_background_read_is_never_dropped_silently() {
    use crate::workbench_client::mailbox::MutationResult;
    let (mut ui, mailbox, clock, _elapsed) = ready_ui();
    while let Some(lease) = mailbox.try_recv().unwrap() {
        mailbox.finish_lease(lease).unwrap();
    }
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestCloseTab),
        ClientActionDisposition::DismissNavigator
    );
    let token = ui.view().close_confirmation().unwrap().token();
    ui.after_close_draw(DrawReceipt::new(
        Some(ModalPresentation::Presented(token)),
        None,
    ));

    // Another terminal's publication asks for a read, which goes out under the open prompt.
    ui.state.as_mut().unwrap().note_publication_hint();
    ui.tick();
    let lease = loop {
        let lease = mailbox.try_recv().unwrap().expect("the publication read");
        if matches!(lease.record(), QueuedCommand::Refresh(_)) {
            break lease;
        }
        mailbox.finish_lease(lease).unwrap();
    };
    let QueuedCommand::Refresh(marker) = lease.record() else {
        unreachable!("a Refresh marker");
    };
    let (generation, deadline) = (marker.generation(), marker.deadline());
    assert_eq!(
        mailbox.mark_refresh_in_flight(generation).unwrap(),
        MutationResult::Applied
    );
    assert!(ui.capture_confirmation_key(ClientKey::Enter));

    // The read finds the Workbench unchanged.
    let wait = mailbox
        .publish_refresh_result_before(
            generation,
            WorkbenchSnapshotOutcome::Available {
                snapshot: super::super::tests::membership::snapshot(2, 1),
            },
            deadline,
            &clock,
        )
        .unwrap();
    ui.tick();
    assert!(mailbox.take_deposited_reply(&wait).unwrap().is_some());
    mailbox.finish_lease(lease).unwrap();

    let mut submitted = 0;
    for _ in 0..3 {
        ui.tick();
        while let Some(lease) = mailbox.try_recv().unwrap() {
            if matches!(lease.record(), QueuedCommand::Close(_)) {
                submitted += 1;
            }
            mailbox.finish_lease(lease).unwrap();
        }
    }
    let reason = ui.view().notice().is_some();
    assert!(
        submitted == 1 || (submitted == 0 && reason),
        "the confirmed Tab close was submitted {submitted} times with no visible reason"
    );
}
