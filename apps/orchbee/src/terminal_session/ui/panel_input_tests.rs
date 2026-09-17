use super::move_membership_tests::ready_ui;
use super::*;
use crate::client_view::{ClientKey, ClientView};
use crate::terminal_input::{
    DispatchOutcome, InputDispatcher, TerminalInputPort, TerminalInputUnavailable,
};
use crate::terminal_session::state::PanelIntent;
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
    dispatch(&mut dispatcher, &mut view, &mut ui, b"n\x02X");
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
fn close_panel_on_a_tabs_only_leaf_queues_the_close_without_a_drawn_confirmation() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
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
    // X arms no confirmation: contrast with RequestCloseTab.
    assert!(!confirmation_drawn(&ui));
    let lease = mailbox.try_recv().unwrap().expect("panel command queued");
    let QueuedCommand::Panel(marker) = lease.record() else {
        panic!("one Panel marker");
    };
    let operation = marker.operation();
    assert_eq!(operation.intent(), &PanelIntent::Close);
    assert_eq!(operation.panel_id(), &only_leaf);
    assert_eq!(operation.target(), selected.key());
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
        let queued = mailbox.try_recv().unwrap();
        if check {
            assert_eq!(disposition, SessionActionDisposition::DismissNavigator);
            let Some(lease) = queued else {
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
            assert!(queued.is_none());
        }
    }
}
