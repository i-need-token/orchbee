use crate::client_view::{ClientView, Notice, Overlay};

use super::{DispatchOutcome, TerminalInputPort, TerminalInputUnavailable, dispatch_input};

#[derive(Default)]
struct RecordingTerminalInput {
    forwarded: Vec<Vec<u8>>,
    unavailable: bool,
}

impl TerminalInputPort for RecordingTerminalInput {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.push(input.to_vec());
        if self.unavailable {
            Err(TerminalInputUnavailable)
        } else {
            Ok(())
        }
    }
}

fn prefix(view: &mut ClientView, input: &mut RecordingTerminalInput) {
    assert_eq!(
        dispatch_input(view, b"\x02", input),
        DispatchOutcome::Continue
    );
}

#[test]
fn forwards_ordinary_control_and_multibyte_input_exactly() {
    let mut view = ClientView::new(120);
    let mut input = RecordingTerminalInput::default();
    let ordinary = b"a\x03\x04\x1b[Ax\xe4\xb8\xad";

    assert_eq!(
        dispatch_input(&mut view, ordinary, &mut input),
        DispatchOutcome::Continue
    );
    assert_eq!(input.forwarded, [ordinary.to_vec()]);
}

#[test]
fn consumes_the_prefix_then_forwards_only_the_literal_control_b_binding() {
    let mut view = ClientView::new(120);
    let mut input = RecordingTerminalInput::default();

    prefix(&mut view, &mut input);
    assert!(input.forwarded.is_empty());
    assert!(view.prefix_active());
    assert_eq!(
        dispatch_input(&mut view, b"\x02", &mut input),
        DispatchOutcome::Continue
    );
    assert_eq!(input.forwarded, [vec![0x02]]);
    assert!(!view.prefix_active());
}

#[test]
fn maps_backend_bindings_to_visible_no_effect_results() {
    let mut view = ClientView::new(120);
    let mut input = RecordingTerminalInput::default();

    assert_eq!(
        dispatch_input(&mut view, b"\x02c", &mut input),
        DispatchOutcome::Continue
    );
    assert!(matches!(view.notice(), Some(Notice::BackendUnavailable(_))));
    assert!(input.forwarded.is_empty());
}

#[test]
fn overlays_never_swallow_ordinary_terminal_input() {
    for command in *b"w?" {
        let mut view = ClientView::new(120);
        let mut input = RecordingTerminalInput::default();
        dispatch_input(&mut view, &[0x02, command], &mut input);
        assert!(matches!(
            view.overlay(),
            Some(Overlay::WorkbenchNavigator | Overlay::ShortcutHelp)
        ));

        dispatch_input(&mut view, b"text\x03\x04\x1b[A", &mut input);
        assert_eq!(input.forwarded, [b"text\x03\x04\x1b[A".to_vec()]);
        assert!(view.overlay().is_some());
    }
}

#[test]
fn reports_an_unavailable_terminal_port_without_losing_exact_bytes() {
    let mut view = ClientView::new(120);
    let mut input = RecordingTerminalInput {
        unavailable: true,
        ..RecordingTerminalInput::default()
    };

    dispatch_input(&mut view, b"hello", &mut input);
    assert_eq!(input.forwarded, [b"hello".to_vec()]);
    assert_eq!(view.notice(), Some(Notice::TerminalInputUnavailable));
}

#[test]
fn splits_only_at_prefix_boundaries_and_preserves_order() {
    let mut view = ClientView::new(120);
    let mut input = RecordingTerminalInput::default();

    dispatch_input(&mut view, b"before\x02\x02after", &mut input);
    assert_eq!(
        input.forwarded,
        [b"before".to_vec(), vec![0x02], b"after".to_vec()]
    );
}

#[test]
fn exits_only_after_the_accepted_prefixed_exit_binding() {
    let mut view = ClientView::new(120);
    let mut input = RecordingTerminalInput::default();

    assert_eq!(
        dispatch_input(&mut view, b"q\x02dignored", &mut input),
        DispatchOutcome::Exit
    );
    assert_eq!(input.forwarded, [b"q".to_vec()]);
}
