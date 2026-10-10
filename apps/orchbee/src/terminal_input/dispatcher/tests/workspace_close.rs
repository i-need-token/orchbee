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
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }
    fn capture_confirmation_key(&mut self, _: ClientKey) -> bool {
        self.active
    }
    fn click_workspace_close(&mut self, token: WorkspaceCloseToken, confirm: bool) {
        assert_eq!(token, WorkspaceCloseToken::for_test(1));
        assert!(self.active);
        self.active = false;
        self.decisions.push(confirm);
    }
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
        let mut hits = HitMapBuilder::new();
        hits.start_modal();
        hits.record(
            HitLayer::Overlay,
            Rect::new(10, 5, 8, 1),
            HitTarget::WorkspaceClose {
                token: WorkspaceCloseToken::for_test(1),
                confirm,
            },
        );
        dispatcher.pointer.set_capture(true);
        dispatcher.publish_hit_map(hits.finish());
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
        let mut hits = HitMapBuilder::new();
        hits.start_modal();
        hits.record(
            HitLayer::Overlay,
            Rect::new(10, 5, 8, 1),
            HitTarget::WorkspaceClose {
                token: WorkspaceCloseToken::for_test(1),
                confirm: true,
            },
        );
        dispatcher.pointer.set_capture(true);
        dispatcher.publish_hit_map(hits.finish());
        dispatch_all(&mut dispatcher, &mut view, &mut port, &bytes[..split]);
        dispatch_all(&mut dispatcher, &mut view, &mut port, &bytes[split..]);
        assert_eq!(port.decisions, [true], "split {split}");
        assert!(port.forwarded.is_empty());
    }
}
