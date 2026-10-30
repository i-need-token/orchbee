//! Packet provenance through the real `LegacyDecoder -> InputDispatcher -> WorkbenchClientUi`
//! chain with `WorkbenchClientState` fixtures and the Tab-close confirmation API.
//! Decoder-only witnesses live in `terminal_input/tests/provenance.rs`.

use super::super::tests::move_membership::ready_ui;
use super::super::*;
use crate::client_view::{ClientAction, ClientView};
use crate::terminal_input::{
    DispatchOutcome, InputDispatcher, TerminalInputPort, TerminalInputUnavailable,
};
use std::{
    io,
    time::{Duration, Instant},
};

struct RecordingPort<'a> {
    ui: &'a mut WorkbenchClientUi,
    forwarded: Vec<u8>,
}

impl TerminalInputPort for RecordingPort<'_> {
    fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
        self.ui.capture_claims()
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::WorkbenchCaptureOwner,
    ) -> Option<crate::workbench_client::CaptureContext> {
        self.ui.capture_context(owner)
    }

    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(input);
        self.ui
            .forward(input)
            .then_some(())
            .ok_or(TerminalInputUnavailable)
    }

    fn apply_client_action(
        &mut self,
        command: crate::workbench_client::InputAction,
    ) -> Option<ClientActionDisposition> {
        Some(self.ui.apply_client_action(command))
    }

    fn surface_input(&mut self, input: crate::terminal_input::SurfaceInput<'_>) -> bool {
        use crate::terminal_input::SurfaceInput;
        match input {
            SurfaceInput::TabClose { .. }
            | SurfaceInput::PanelClose { .. }
            | SurfaceInput::WorkspaceClose { .. }
            | SurfaceInput::Rename { .. }
            | SurfaceInput::Setup { .. } => self.ui.surface_input(input),
            _ => false,
        }
    }
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{RenameAction, SetupAction, SurfaceAction};
        match action {
            SurfaceAction::Rename(RenameAction::Cancel(_))
            | SurfaceAction::Rename(RenameAction::Submit(_))
            | SurfaceAction::Setup(SetupAction::Cancel(_))
            | SurfaceAction::Setup(SetupAction::Submit(_)) => self.ui.surface_action(action),
            _ => None,
        }
    }
}

fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    ui: &mut WorkbenchClientUi,
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
    ui: &mut WorkbenchClientUi,
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

fn armed(ui: &WorkbenchClientUi) -> bool {
    ui.state
        .as_ref()
        .is_some_and(|state| state.view().close_confirmation().is_some())
}

fn draw_close_prompt(ui: &mut WorkbenchClientUi) {
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
}

fn arm_and_draw(ui: &mut WorkbenchClientUi) {
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestCloseTab),
        ClientActionDisposition::DismissNavigator
    );
    assert!(armed(ui));
    draw_close_prompt(ui);
}

// ---------- seam-level (real dispatcher → WorkbenchClientUi) ----------

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
        b"\r\x1b",
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
        b"\r",
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
/// Enter payload byte can never decide anything.
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
        b"\r\x1b[200~",
        t0,
        &mut forwarded,
    );
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"\ryy",
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

/// Exact-key + draw gate still holds on the final pair: drawn Enter closes
/// exactly once and the same-read tail is swallowed; arm+decide in one read
/// cannot commit undrawn.
#[test]
fn drawn_enter_closes_once_and_arm_plus_decide_cannot_commit() {
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
        b"\r\x02|",
        t0,
        &mut forwarded,
    );
    assert!(mailbox.try_recv().unwrap().is_some(), "drawn Enter closes");
    assert!(mailbox.try_recv().unwrap().is_none(), "exactly once");
    assert!(forwarded.is_empty(), "decision tail swallowed");

    let (mut ui2, mailbox2, _c2, _e2) = ready_ui();
    let mut view2 = ClientView::new(120);
    let mut dispatcher2 = InputDispatcher::default();
    let mut forwarded2 = Vec::new();
    assert_eq!(
        ui2.apply_client_action(ClientAction::RequestCloseTab),
        ClientActionDisposition::DismissNavigator
    );
    dispatch(
        &mut dispatcher2,
        &mut view2,
        &mut ui2,
        b"\r",
        t0,
        &mut forwarded2,
    );
    assert!(
        mailbox2.try_recv().unwrap().is_none(),
        "undrawn Enter cannot commit"
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

#[test]
fn supplementary_paste_reaches_the_exact_tab_rename_request_at_every_utf8_split() {
    let label = "A中𠮷🚀B";
    let paste = format!("\x1b[200~{label}\x1b[201~").into_bytes();
    for split in 0..=paste.len() {
        let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
        let token = ui.request_rename_draft().unwrap();
        ui.after_rename_draw(Some(ModalPresentation::Presented(token)));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        let mut forwarded = Vec::new();
        let now = Instant::now();
        for packet in [&b"\x15"[..], &paste[..split], &paste[split..]] {
            dispatch(
                &mut dispatcher,
                &mut view,
                &mut ui,
                packet,
                now,
                &mut forwarded,
            );
        }
        assert_eq!(
            ui.state.as_ref().unwrap().rename_draft().unwrap().text(),
            label
        );
        // Backspace deletes the emoji as one grapheme, then ordinary typed
        // supplementary text restores it without touching the extended Han.
        for packet in [&b"\x7f\x7f"[..], "🚀B".as_bytes(), &b"\r"[..]] {
            dispatch(
                &mut dispatcher,
                &mut view,
                &mut ui,
                packet,
                now,
                &mut forwarded,
            );
        }
        assert!(forwarded.is_empty());
        let lease = mailbox.try_recv().unwrap().unwrap();
        let QueuedCommand::Rename(marker) = lease.record() else {
            panic!("one exact rename request");
        };
        assert_eq!(marker.operation().label().as_str(), label);
    }
}

#[test]
fn cancelling_a_supplementary_rename_preserves_the_original_label_and_pane_input() {
    let (mut ui, mailbox, _clock, _elapsed) = ready_ui();
    let original = ui.state.as_ref().unwrap().cache().clone();
    let token = ui.request_rename_draft().unwrap();
    ui.after_rename_draw(Some(ModalPresentation::Presented(token)));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let mut forwarded = Vec::new();
    let now = Instant::now();
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        "\x15\x1b[200~🚀输入𠮷检查\x1b[201~".as_bytes(),
        now,
        &mut forwarded,
    );
    assert_eq!(
        ui.state.as_ref().unwrap().rename_draft().unwrap().text(),
        "🚀输入𠮷检查"
    );
    dispatch(
        &mut dispatcher,
        &mut view,
        &mut ui,
        b"\x1b",
        now,
        &mut forwarded,
    );
    synchronize(
        &mut dispatcher,
        &mut view,
        &mut ui,
        now + Duration::from_millis(60),
        &mut forwarded,
    );
    assert!(ui.state.as_ref().unwrap().rename_draft().is_none());
    assert_eq!(ui.state.as_ref().unwrap().cache(), &original);
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(forwarded.is_empty());
}
