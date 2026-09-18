use super::move_membership_tests::ready_ui;
use super::publication_hint_tests::ready_split_ui;
use super::*;
use crate::client_view::{ClientKey, ClientView};
use crate::terminal_input::{
    DispatchOutcome, InputDispatcher, TerminalInputPort, TerminalInputUnavailable,
};
use crate::terminal_session::state::{CarrierFailure, PanelIntent};
use orchbee_workbench_protocol::WorkbenchPanelSplitDirection;
use std::{io, time::Instant};

/// Delegates `TerminalInputPort` onto `NativeUi` exactly the way the
/// `TerminalSession` production impl does, so these witnesses run the real
/// dispatcher, confirmation capture, modal epoch, and mailbox admission
/// without spawning a session worker.
struct NativePort<'a>(&'a mut NativeUi);

impl TerminalInputPort for NativePort<'_> {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.0
            .forward(input)
            .then_some(())
            .ok_or(TerminalInputUnavailable)
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        Some(self.0.apply_client_action(action))
    }

    fn capture_confirmation_key(&mut self, key: ClientKey) -> bool {
        self.0.capture_confirmation_key(key)
    }

    fn rename_input_context(&mut self) -> Option<NativeRenameInputContext> {
        self.0.rename_input_context()
    }

    fn edit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
        edit: NativeRenameEdit<'_>,
    ) -> bool {
        self.0.edit_rename_draft(token, edit)
    }

    fn cancel_rename_draft(&mut self, token: NativeRenameDraftToken) {
        self.0.cancel_rename_draft(token);
    }

    fn submit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
    ) -> Option<SessionActionDisposition> {
        Some(self.0.submit_rename_draft(token))
    }

    fn setup_input_context(&mut self) -> Option<NativeSetupInputContext> {
        self.0.setup_input_context()
    }

    fn edit_setup_draft(&mut self, token: NativeSetupToken, edit: NativeSetupEdit<'_>) -> bool {
        self.0.edit_setup_draft(token, edit)
    }

    fn cancel_setup_draft(&mut self, token: NativeSetupToken) {
        self.0.cancel_setup_draft(token);
    }

    fn submit_setup_draft(&mut self, token: NativeSetupToken) -> Option<SessionActionDisposition> {
        Some(self.0.submit_setup_draft(token))
    }
}

fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut NativeUi,
    bytes: &[u8],
) -> DispatchOutcome {
    let mut port = NativePort(ui);
    dispatcher
        .dispatch(
            view,
            bytes,
            &mut port,
            Instant::now(),
            |_, _| Ok::<_, io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap()
}

fn confirmation_drawn(ui: &NativeUi) -> bool {
    ui.state
        .as_ref()
        .is_some_and(|state| state.view().close_confirmation().is_some())
}

fn panel_confirmation_drawn(ui: &NativeUi) -> bool {
    ui.state
        .as_ref()
        .is_some_and(|state| state.view().panel_close_confirmation().is_some())
}

fn panel_token(ui: &NativeUi) -> NativePanelCloseConfirmationToken {
    ui.state
        .as_ref()
        .unwrap()
        .view()
        .panel_close_confirmation()
        .unwrap()
        .token()
}

fn draw_panel_prompt(ui: &mut NativeUi) {
    let token = panel_token(ui);
    ui.after_close_draw(NativeDrawReceipt::default().with_panel_close(Some(token)));
}

#[test]
fn drawn_close_confirmation_captures_panel_prefix_keys_and_the_cancel_packet_tail() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestCloseTab),
        SessionActionDisposition::DismissNavigator
    );
    assert!(confirmation_drawn(&ui));

    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x02X\x02|\x02H\x02-");
    assert!(confirmation_drawn(&ui));
    assert!(!view.prefix_active());
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "panel prefix keys must not reach the mailbox while the close confirmation owns input"
    );

    // The packet tail stays captured after the answer byte disarms the prompt.
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b\x02X");
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
        SessionActionDisposition::DismissNavigator
    );

    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x02X\x02|");
    assert!(!view.prefix_active());
    assert!(
        mailbox.try_recv().unwrap().is_none(),
        "panel prefix keys must not reach the mailbox while the rename draft owns input"
    );
    // The draft is prefilled with the current label; the captured bytes land
    // inside it instead of reaching the prefix layer.
    assert_eq!(
        ui.state.as_ref().unwrap().rename_draft().unwrap().text(),
        "Tab 0X|"
    );

    // The draft retains its own submit path once drawn; the queued command is
    // a Rename.
    let token = ui.rename_input_context().unwrap().token();
    ui.after_rename_draw(Some(token));
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
        SessionActionDisposition::DismissNavigator
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
    dispatch(&mut dispatcher, &mut view, &mut ui, b"y");
    assert!(panel_confirmation_drawn(&ui));
    assert!(mailbox.try_recv().unwrap().is_none());

    // A receipt that carried no panel prompt token is not a drawn prompt.
    ui.after_close_draw(NativeDrawReceipt::default());
    dispatch(&mut dispatcher, &mut view, &mut ui, b"y");
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
        SessionActionDisposition::DismissNavigator
    );
    let token = ui
        .state
        .as_ref()
        .unwrap()
        .view()
        .close_confirmation()
        .unwrap()
        .token();
    ui.after_close_draw(NativeDrawReceipt::new(Some(token), None));
    dispatch(&mut dispatcher, &mut view, &mut ui, b"n");
    assert!(
        confirmation_drawn(&ui),
        "n is a non-decision key: the drawn confirmation must stay armed"
    );
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b");
    assert!(!confirmation_drawn(&ui));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn drawn_panel_close_confirmation_and_lowercase_y_queue_exactly_one_panel_close() {
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
        SessionActionDisposition::DismissNavigator
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
    dispatch(&mut dispatcher, &mut view, &mut ui, b"y");

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
        SessionActionDisposition::DismissNavigator
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
    dispatch(&mut dispatcher, &mut view, &mut ui, b"y");
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
        SessionActionDisposition::DismissNavigator
    );
    draw_panel_prompt(&mut ui);

    // Uppercase Y, lowercase n/N, ordinary keys, and prefix bytes are
    // non-decisions that the armed confirmation swallows.
    dispatch(&mut dispatcher, &mut view, &mut ui, b"YnNq\x02X\x02x");
    assert!(panel_confirmation_drawn(&ui));
    assert!(!confirmation_drawn(&ui));
    assert!(!view.prefix_active());
    assert!(mailbox.try_recv().unwrap().is_none());

    // Esc cancels; the same packet's tail stays captured and cannot re-arm.
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b\x02X");
    assert!(!panel_confirmation_drawn(&ui));
    assert!(!view.prefix_active());
    assert!(mailbox.try_recv().unwrap().is_none());

    // A later independent packet dispatches normally: X re-arms cleanly.
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x02X");
    assert!(panel_confirmation_drawn(&ui));
}

#[test]
fn resize_invalidation_blocks_a_drawn_panel_close_until_redrawn() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        SessionActionDisposition::DismissNavigator
    );
    draw_panel_prompt(&mut ui);

    // Resize drops the draw receipt; y must not execute against it.
    ui.invalidate_close_display();
    dispatch(&mut dispatcher, &mut view, &mut ui, b"y");
    assert!(panel_confirmation_drawn(&ui));
    assert!(mailbox.try_recv().unwrap().is_none());

    // A fresh matching draw re-arms execution; Esc still cancels instead.
    draw_panel_prompt(&mut ui);
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b");
    assert!(!panel_confirmation_drawn(&ui));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn retired_session_drops_the_panel_close_confirmation() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        SessionActionDisposition::DismissNavigator
    );
    draw_panel_prompt(&mut ui);
    ui.state.as_mut().unwrap().retire(CarrierFailure::Closed);
    assert!(!panel_confirmation_drawn(&ui));
    assert!(!ui.capture_confirmation_key(ClientKey::Plain('y')));
    assert!(mailbox.try_recv().unwrap().is_none());
}

#[test]
fn carrier_retirement_blocks_a_drawn_panel_close_token() {
    let (mut ui, _mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    assert_eq!(
        ui.apply_client_action(ClientAction::ClosePanel),
        SessionActionDisposition::DismissNavigator
    );
    draw_panel_prompt(&mut ui);
    ui.retire(CarrierFailure::Closed);
    dispatch(&mut dispatcher, &mut view, &mut ui, b"y");
    assert!(
        panel_confirmation_drawn(&ui),
        "carrier retirement blocks the drawn token but keeps the modal cancellable"
    );
    dispatch(&mut dispatcher, &mut view, &mut ui, b"\x1b");
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

        let disposition = ui.apply_client_action(action);
        if check {
            assert_eq!(disposition, SessionActionDisposition::DismissNavigator);
            if matches!(action, ClientAction::ClosePanel) {
                // Close arms a confirmation; only a drawn prompt plus the exact
                // lowercase y queues the close.
                assert!(mailbox.try_recv().unwrap().is_none());
                assert!(panel_confirmation_drawn(&ui));
                draw_panel_prompt(&mut ui);
                assert!(ui.capture_confirmation_key(ClientKey::Plain('y')));
            }
            let Some(lease) = mailbox.try_recv().unwrap() else {
                panic!("panel command queued for {action:?}");
            };
            let QueuedCommand::Panel(marker) = lease.record() else {
                panic!("one Panel marker");
            };
            match action {
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
            assert_eq!(disposition, SessionActionDisposition::KeepNavigator);
            assert!(mailbox.try_recv().unwrap().is_none());
        }
    }
}
