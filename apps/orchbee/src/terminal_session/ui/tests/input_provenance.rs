//! Packet provenance through the real `LegacyDecoder -> InputDispatcher -> NativeUi`
//! chain with `NativeSessionState` fixtures and the Tab-close confirmation API.
//! Decoder-only witnesses live in `terminal_input/tests/provenance.rs`.

use super::super::tests::move_membership::ready_ui;
use super::super::*;
use crate::client_view::{ClientAction, ClientKey, ClientView};
use crate::terminal_input::{
    DispatchOutcome, InputDispatcher, TerminalInputPort, TerminalInputUnavailable,
};
use std::{
    io,
    time::{Duration, Instant},
};

struct RecordingPort<'a> {
    ui: &'a mut NativeUi,
    forwarded: Vec<u8>,
}

impl TerminalInputPort for RecordingPort<'_> {
    fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
        self.ui.capture_claims()
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::NativeCaptureOwner,
    ) -> Option<crate::terminal_session::NativeCaptureContext> {
        self.ui.capture_context(owner)
    }

    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(input);
        self.ui
            .forward(input)
            .then_some(())
            .ok_or(TerminalInputUnavailable)
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        Some(self.ui.apply_client_action(action))
    }

    fn capture_tab_confirmation_key_for(
        &mut self,
        token: NativeCloseConfirmationToken,
        key: ClientKey,
    ) -> bool {
        self.ui.capture_tab_confirmation_key(token, key)
    }

    fn capture_panel_confirmation_key_for(
        &mut self,
        token: NativePanelCloseConfirmationToken,
        key: ClientKey,
    ) -> bool {
        self.ui.capture_panel_confirmation_key(token, key)
    }

    fn capture_workspace_confirmation_key_for(
        &mut self,
        token: WorkspaceCloseToken,
        key: ClientKey,
    ) -> bool {
        self.ui.capture_workspace_close_key(token, key)
    }

    fn edit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
        edit: NativeRenameEdit<'_>,
    ) -> bool {
        self.ui.edit_rename_draft(token, edit)
    }

    fn cancel_rename_draft(&mut self, token: NativeRenameDraftToken) {
        self.ui.cancel_rename_draft(token)
    }

    fn submit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
    ) -> Option<SessionActionDisposition> {
        Some(self.ui.submit_rename_draft(token))
    }

    fn edit_setup_draft(&mut self, token: NativeSetupToken, edit: NativeSetupEdit<'_>) -> bool {
        self.ui.edit_setup_draft(token, edit)
    }

    fn cancel_setup_draft(&mut self, token: NativeSetupToken) {
        self.ui.cancel_setup_draft(token)
    }

    fn submit_setup_draft(&mut self, token: NativeSetupToken) -> Option<SessionActionDisposition> {
        Some(self.ui.submit_setup_draft(token))
    }
}

fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut NativeUi,
    bytes: &[u8],
    now: Instant,
    forwarded: &mut Vec<u8>,
) -> DispatchOutcome {
    let mut port = RecordingPort {
        ui,
        forwarded: std::mem::take(forwarded),
    };
    let outcome = dispatcher
        .dispatch(
            view,
            bytes,
            &mut port,
            now,
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    *forwarded = port.forwarded;
    outcome
}

fn synchronize(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut NativeUi,
    now: Instant,
    forwarded: &mut Vec<u8>,
) {
    let mut port = RecordingPort {
        ui,
        forwarded: std::mem::take(forwarded),
    };
    dispatcher
        .synchronize(view, &mut port, now, &mut |_| Ok::<_, io::Error>(()))
        .unwrap();
    *forwarded = port.forwarded;
}

fn armed(ui: &NativeUi) -> bool {
    ui.state
        .as_ref()
        .is_some_and(|state| state.view().close_confirmation().is_some())
}

fn draw_close_prompt(ui: &mut NativeUi) {
    let token = ui
        .state
        .as_ref()
        .unwrap()
        .view()
        .close_confirmation()
        .unwrap()
        .token();
    ui.after_close_draw(NativeDrawReceipt::new(
        Some(ModalPresentation::Presented(token)),
        None,
    ));
}

fn arm_and_draw(ui: &mut NativeUi) {
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestCloseTab),
        SessionActionDisposition::DismissNavigator
    );
    assert!(armed(ui));
    draw_close_prompt(ui);
}

// ---------- seam-level (real dispatcher → NativeUi) ----------

/// Packet-07 boundary witness re-run on the final candidates: a decision-read
/// trailing partial sequence now completes in the next read as an OWNED item
/// and is swallowed — it no longer escapes to the pane.
#[test]
fn decision_read_partial_tail_completes_owned_and_is_swallowed() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut forwarded = Vec::new();
    let t0 = Instant::now();
    arm_and_draw(&mut ui);

    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"y\x1b",
        t0,
        &mut forwarded,
    );
    assert!(!armed(&ui), "drawn y decides the confirmation");
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"[A",
        t0 + Duration::from_millis(5),
        &mut forwarded,
    );
    assert!(
        forwarded.is_empty(),
        "the completed tail stays owned by the decision packet, forwarded={forwarded:?}"
    );
    assert!(
        mailbox.try_recv().unwrap().is_some(),
        "exactly one close queued"
    );
}

/// The next normal packet after a decision decodes freely — the capture does
/// not linger (`Released` ends at the packet boundary, never swallowed whole).
#[test]
fn next_packet_after_decision_decodes_normally() {
    let (mut ui, _mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut forwarded = Vec::new();
    let t0 = Instant::now();
    arm_and_draw(&mut ui);
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"y",
        t0,
        &mut forwarded,
    );
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"q",
        t0 + Duration::from_millis(5),
        &mut forwarded,
    );
    assert_eq!(
        forwarded, b"q",
        "ordinary input after the decision forwards"
    );
}

/// A paste opened inside the decision packet keeps ownership across reads:
/// its payload and terminator are swallowed, never reaching the pane, and a
/// `y` payload byte can never decide anything.
#[test]
fn paste_opened_in_decision_packet_stays_owned_across_reads() {
    let (mut ui, _mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut forwarded = Vec::new();
    let t0 = Instant::now();
    arm_and_draw(&mut ui);
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"y\x1b[200~",
        t0,
        &mut forwarded,
    );
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"yyy",
        t0 + Duration::from_millis(5),
        &mut forwarded,
    );
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"\x1b[201~",
        t0 + Duration::from_millis(10),
        &mut forwarded,
    );
    assert!(
        forwarded.is_empty(),
        "owned paste tail must never reach the pane, forwarded={forwarded:?}"
    );
}

/// Complete CSI/SS3/C1 arrows, focus reports, and SGR mouse bursts must not
/// cancel or confirm an armed close confirmation.
#[test]
fn armed_confirmation_survives_arrow_focus_and_sgr_sequences() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut forwarded = Vec::new();
    let t0 = Instant::now();
    arm_and_draw(&mut ui);

    for bytes in [
        &b"\x1b[A"[..],
        &b"\x1b[B"[..],
        &b"\x1bOA"[..],
        &b"\x9bC"[..],
        &b"\x1b[I"[..],
        &b"\x1b[O"[..],
        &b"\x1b[<0;5;10M"[..],
        &b"\x1b[<0;5;10m"[..],
        &b"\x1b[1;5A"[..],
        &b"\x1b[H"[..],
    ] {
        dispatch(
            &mut dispatcher,
            &mut view,
            &mut ui,
            bytes,
            t0,
            &mut forwarded,
        );
        assert!(armed(&ui), "armed confirmation must survive {bytes:?}");
    }
    assert!(mailbox.try_recv().unwrap().is_none());
}

/// Exact-key + draw gate still holds on the final pair: drawn `y` closes
/// exactly once and the same-read tail is swallowed; arm+decide in one read
/// cannot commit undrawn.
#[test]
fn drawn_y_closes_once_and_arm_plus_decide_cannot_commit() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut forwarded = Vec::new();
    let t0 = Instant::now();
    arm_and_draw(&mut ui);
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"y\x02|",
        t0,
        &mut forwarded,
    );
    assert!(mailbox.try_recv().unwrap().is_some(), "drawn y closes");
    assert!(mailbox.try_recv().unwrap().is_none(), "exactly once");
    assert!(forwarded.is_empty(), "decision tail swallowed");

    let (mut ui2, mailbox2, _c2, _e2) = ready_ui();
    let mut view2 = ClientView::new(120);
    let mut dispatcher2 = InputDispatcher::default();
    let mut forwarded2 = Vec::new();
    assert_eq!(
        ui2.apply_client_action(ClientAction::RequestCloseTab),
        SessionActionDisposition::DismissNavigator
    );
    dispatch(
        &mut dispatcher2,
        &mut view2,
        &mut ui2,
        b"y",
        t0,
        &mut forwarded2,
    );
    assert!(
        mailbox2.try_recv().unwrap().is_none(),
        "undrawn y cannot commit"
    );
}

/// Lone Esc remains a real cancel, resolved by the bounded wait tick.
#[test]
fn lone_esc_cancels_only_after_the_wait_expires() {
    let (mut ui, _mailbox, _clock, _elapsed) = ready_ui();
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut forwarded = Vec::new();
    let t0 = Instant::now();
    arm_and_draw(&mut ui);
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"\x1b",
        t0,
        &mut forwarded,
    );
    assert!(armed(&ui), "held ESC does not cancel inside its wait");
    synchronize(
        &mut dispatcher,
        &mut view,
        &mut ui,
        t0 + Duration::from_millis(60),
        &mut forwarded,
    );
    assert!(!armed(&ui), "ticked lone Esc cancels");
}
