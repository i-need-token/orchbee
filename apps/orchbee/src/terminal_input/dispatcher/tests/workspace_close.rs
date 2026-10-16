use super::*;
use crate::terminal_session::WorkspaceCloseToken;
use crate::tui_pointer::{HitLayer, HitMapBuilder, HitTarget};
use ratatui::layout::Rect;

struct ClosePort {
    active: bool,
    decisions: Vec<bool>,
    forwarded: Vec<u8>,
}
impl TerminalInputPort for ClosePort {
    fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
        crate::tui_composition::NativeCaptureClaims::default().with_workspace_close(self.active)
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::NativeCaptureOwner,
    ) -> Option<crate::terminal_session::NativeCaptureContext> {
        if owner != crate::tui_composition::NativeCaptureOwner::WorkspaceClose || !self.active {
            return None;
        }
        Some(
            crate::terminal_session::NativeCaptureContext::WorkspaceClose(
                crate::terminal_session::ConfirmationCaptureContext {
                    token: WorkspaceCloseToken::for_test(1),
                    phase: crate::tui_presentation::PresentationPhase::Admitted,
                    readiness: crate::terminal_session::CaptureReadiness::Interactive,
                },
            ),
        )
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }
    fn capture_workspace_confirmation_key_for(
        &mut self,
        token: WorkspaceCloseToken,
        _: ClientKey,
    ) -> bool {
        token == WorkspaceCloseToken::for_test(1) && self.active
    }
    fn click_workspace_close(&mut self, token: WorkspaceCloseToken, confirm: bool) {
        assert_eq!(token, WorkspaceCloseToken::for_test(1));
        assert!(self.active);
        self.active = false;
        self.decisions.push(confirm);
    }
}

fn publish_close_control(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    port: &mut ClosePort,
    confirm: bool,
) {
    dispatcher.pointer.set_capture(true);
    dispatcher
        .synchronize(view, port, Instant::now(), &mut |_| Ok::<(), io::Error>(()))
        .unwrap();
    let mut hits = HitMapBuilder::new();
    hits.start_modal();
    hits.record_modal_control(
        HitLayer::Overlay,
        Rect::new(10, 5, 8, 1),
        HitTarget::WorkspaceClose {
            token: WorkspaceCloseToken::for_test(1),
            confirm,
        },
        crate::tui_pointer::InteractionAvailability::Enabled,
    );
    dispatcher.publish_hit_map(hits.finish());
}

#[test]
fn close_mouse_decision_captures_its_whole_packet_and_split_tail_frame() {
    for confirm in [false, true] {
        let mut port = ClosePort {
            active: true,
            decisions: vec![],
            forwarded: vec![],
        };
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        publish_close_control(&mut dispatcher, &mut view, &mut port, confirm);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;12;6M\x1b[<0;12;6mterminal\x1b[",
        );
        assert_eq!(port.decisions, [confirm]);
        assert!(port.forwarded.is_empty());
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"Anext");
        assert_eq!(port.forwarded, b"next");
    }
}

#[test]
fn backdrop_cancel_owns_split_mouse_decision_and_tail_before_shell_input_resumes() {
    let bytes = b"\x1b[<0;2;2M\x1b[<0;2;2mtail\x02x";
    let release_end = b"\x1b[<0;2;2M\x1b[<0;2;2m".len();
    for split in 0..release_end {
        let mut port = ClosePort {
            active: true,
            decisions: vec![],
            forwarded: vec![],
        };
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        publish_close_control(&mut dispatcher, &mut view, &mut port, false);
        let mut hits = HitMapBuilder::new();
        hits.start_modal();
        hits.record(
            HitLayer::OverlaySurface,
            Rect::new(10, 5, 30, 10),
            HitTarget::OverlaySurface,
        );
        let target = HitTarget::WorkspaceClose {
            token: WorkspaceCloseToken::for_test(1),
            confirm: false,
        };
        hits.record_modal_dismissal(target.clone());
        hits.record_modal_control(
            HitLayer::Overlay,
            Rect::new(36, 5, 3, 1),
            target,
            crate::tui_pointer::InteractionAvailability::Enabled,
        );
        dispatcher.publish_hit_map(hits.finish());
        dispatch_all(&mut dispatcher, &mut view, &mut port, &bytes[..split]);
        // Keep the tail in the release's physical packet. A later independent
        // packet is ordinary input, not indefinitely captured modal history.
        dispatch_all(&mut dispatcher, &mut view, &mut port, &bytes[split..]);
        assert_eq!(port.decisions, [false], "split {split}");
        assert!(port.forwarded.is_empty(), "split {split}");
        assert!(!view.prefix_active());
        port.forwarded.clear();
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"next");
        assert_eq!(port.forwarded, b"next");
    }
}

#[test]
fn split_close_mouse_press_and_release_use_the_same_painted_control() {
    let bytes = b"\x1b[<0;12;6M\x1b[<0;12;6m";
    for split in 0..=bytes.len() {
        let mut port = ClosePort {
            active: true,
            decisions: vec![],
            forwarded: vec![],
        };
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        publish_close_control(&mut dispatcher, &mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, &bytes[..split]);
        dispatch_all(&mut dispatcher, &mut view, &mut port, &bytes[split..]);
        assert_eq!(port.decisions, [true], "split {split}");
        assert!(port.forwarded.is_empty());
    }
}

#[cfg(windows)]
#[test]
fn newer_machine_owner_cannot_hide_or_reuse_cancelled_confirmation_cleanup() {
    for retire_with_new_press in [false, true] {
        let mut port = ClosePort {
            active: true,
            decisions: vec![],
            forwarded: vec![],
        };
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        view.machines.available = true;
        publish_close_control(&mut dispatcher, &mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;12;6M");
        assert!(port.decisions.is_empty());

        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[O\x1b[I");
        port.active = false;
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02M");
        assert!(view.machines.open);

        if retire_with_new_press {
            // A new non-wheel press retires the old cleanup record, then
            // belongs to the newer Machine surface.
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<1;2;2Mtail");
        } else {
            // The exact old release settles before Machine routing and owns
            // its complete packet once.
            dispatch_all(
                &mut dispatcher,
                &mut view,
                &mut port,
                b"\x1b[<0;12;6mtail\x1b[200~paste\x1b[201~",
            );
        }
        assert!(port.forwarded.is_empty());
        assert!(port.decisions.is_empty());

        dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
        assert!(!view.machines.open);
        dispatcher.publish_hit_map(HitMapBuilder::new().finish());
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            b"\x1b[<0;12;6morphan",
        );
        assert_eq!(
            port.forwarded, b"orphan",
            "new press={retire_with_new_press}"
        );
        assert!(port.decisions.is_empty());
    }
}
