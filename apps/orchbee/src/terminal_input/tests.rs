use std::convert::Infallible;
use std::time::Instant;

use crate::client_view::{
    ClientAction, ClientView, Notice, Overlay, SessionActionDisposition, SidebarSurface,
};

use super::{
    DispatchOutcome, InputDispatcher, TerminalInputPort, TerminalInputUnavailable, dispatch_input,
};

#[test]
fn modal_capture_keeps_the_packet_tail_after_cancel_and_preserves_prior_bytes() {
    struct ModalInput {
        active: bool,
        keys: Vec<crate::client_view::ClientKey>,
        forwarded: Vec<Vec<u8>>,
    }
    impl TerminalInputPort for ModalInput {
        fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.forwarded.push(bytes.to_vec());
            Ok(())
        }
        fn apply_client_action(
            &mut self,
            action: ClientAction,
        ) -> Option<SessionActionDisposition> {
            if action == ClientAction::RequestCloseTab {
                self.active = true;
            }
            Some(SessionActionDisposition::KeepNavigator)
        }
        fn capture_confirmation_key(&mut self, key: crate::client_view::ClientKey) -> bool {
            if !self.active {
                return false;
            }
            self.keys.push(key);
            if key == crate::client_view::ClientKey::Plain('n') {
                self.active = false;
            }
            true
        }
    }
    let mut view = ClientView::new(100);
    let mut dispatcher = InputDispatcher::default();
    let mut port = ModalInput {
        active: false,
        keys: Vec::new(),
        forwarded: Vec::new(),
    };
    let result = dispatch_input(
        &mut dispatcher,
        &mut view,
        b"before\x02xnleak\x02q",
        &mut port,
        Instant::now(),
        |_, _| Ok::<_, Infallible>(()),
    );
    assert_eq!(result, Ok(DispatchOutcome::Continue));
    assert_eq!(port.forwarded, [b"before".to_vec()]);
    assert_eq!(port.keys, [crate::client_view::ClientKey::Plain('n')]);
    assert!(!view.prefix_active());
    let result = dispatch_input(
        &mut dispatcher,
        &mut view,
        b"next",
        &mut port,
        Instant::now(),
        |_, _| Ok::<_, Infallible>(()),
    );
    assert_eq!(result, Ok(DispatchOutcome::Continue));
    assert_eq!(port.forwarded, [b"before".to_vec(), b"next".to_vec()]);
}

/// A frame that began inside the captured decision packet stays captured
/// when it completes in a later read — the tail is consumed, never
/// forwarded to the pane.
#[test]
fn a_confirmation_tail_sequence_started_in_the_decision_packet_stays_captured() {
    struct ConfirmTail {
        active: bool,
        forwarded: Vec<Vec<u8>>,
    }
    impl TerminalInputPort for ConfirmTail {
        fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.forwarded.push(bytes.to_vec());
            Ok(())
        }
        fn apply_client_action(
            &mut self,
            action: ClientAction,
        ) -> Option<SessionActionDisposition> {
            if action == ClientAction::RequestCloseTab {
                self.active = true;
            }
            Some(SessionActionDisposition::KeepNavigator)
        }
        fn capture_confirmation_key(&mut self, key: crate::client_view::ClientKey) -> bool {
            if !self.active {
                return false;
            }
            if key == crate::client_view::ClientKey::Plain('y') {
                self.active = false;
            }
            true
        }
    }
    let mut view = ClientView::new(100);
    let mut dispatcher = InputDispatcher::default();
    let mut port = ConfirmTail {
        active: false,
        forwarded: Vec::new(),
    };
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"\x02x",
        &mut port,
        Instant::now(),
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    assert!(port.active);
    // The decision `y` and a bare ESC share the decision packet; the CSI
    // tail completing in the next read belongs to the captured frame.
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"y\x1b",
        &mut port,
        Instant::now(),
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"[A",
        &mut port,
        Instant::now(),
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    assert!(port.forwarded.is_empty());
    // A paste run opened inside the decision packet drains captured too;
    // only genuinely later input reaches the pane.
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"\x02x",
        &mut port,
        Instant::now(),
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    assert!(port.active);
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"y\x1b[200~",
        &mut port,
        Instant::now(),
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"ab\x1b[201~z",
        &mut port,
        Instant::now(),
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    assert_eq!(port.forwarded, [b"z".to_vec()]);
}

#[derive(Debug, Eq, PartialEq)]
enum Recorded {
    Action(ClientAction),
    Forward(Vec<u8>),
    Layout(SidebarSurface),
}

#[derive(Default)]
struct RecordingTerminalInput {
    recorded: Vec<Recorded>,
    unavailable: bool,
    action_disposition: Option<SessionActionDisposition>,
}

impl TerminalInputPort for RecordingTerminalInput {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.recorded.push(Recorded::Forward(input.to_vec()));
        if self.unavailable {
            Err(TerminalInputUnavailable)
        } else {
            Ok(())
        }
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.recorded.push(Recorded::Action(action));
        self.action_disposition
    }
}

fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    bytes: &[u8],
    input: &mut RecordingTerminalInput,
) -> DispatchOutcome {
    match dispatch_input(
        dispatcher,
        view,
        bytes,
        input,
        Instant::now(),
        |_view, _input| Ok::<(), Infallible>(()),
    ) {
        Ok(outcome) => outcome,
        Err(never) => match never {},
    }
}

fn prefix(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    input: &mut RecordingTerminalInput,
) {
    assert_eq!(
        dispatch(dispatcher, view, b"\x02", input),
        DispatchOutcome::Continue
    );
}

#[test]
fn forwards_ordinary_control_and_multibyte_input_exactly() {
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput::default();
    let ordinary = b"a\x03\x04\x1b[Ax\xe4\xb8\xad";

    assert_eq!(
        dispatch(&mut dispatcher, &mut view, ordinary, &mut input),
        DispatchOutcome::Continue
    );
    assert_eq!(input.recorded, [Recorded::Forward(ordinary.to_vec())]);
}

#[test]
fn completes_literal_prefix_action_before_forwarding_its_byte() {
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput {
        action_disposition: Some(SessionActionDisposition::KeepNavigator),
        ..RecordingTerminalInput::default()
    };

    prefix(&mut dispatcher, &mut view, &mut input);
    assert!(input.recorded.is_empty());
    assert!(view.prefix_active());
    assert_eq!(
        dispatch(&mut dispatcher, &mut view, b"\x02", &mut input),
        DispatchOutcome::Continue
    );
    assert_eq!(
        input.recorded,
        [
            Recorded::Action(ClientAction::ForwardLiteralPrefix),
            Recorded::Forward(vec![0x02]),
        ]
    );
    assert!(!view.prefix_active());
}

#[test]
fn unavailable_backend_action_reports_no_effect_after_completed_prefix() {
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput::default();

    assert_eq!(
        dispatch(&mut dispatcher, &mut view, b"\x02c", &mut input),
        DispatchOutcome::Continue
    );
    assert_eq!(input.recorded, [Recorded::Action(ClientAction::CreateTab)]);
    assert!(matches!(view.notice(), Some(Notice::BackendUnavailable(_))));
}

#[test]
fn admitted_backend_action_uses_its_session_disposition() {
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput {
        action_disposition: Some(SessionActionDisposition::KeepNavigator),
        ..RecordingTerminalInput::default()
    };
    dispatch(&mut dispatcher, &mut view, b"\x02w", &mut input);
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));

    input.action_disposition = Some(SessionActionDisposition::DismissNavigator);
    dispatch(&mut dispatcher, &mut view, b"\x02n", &mut input);
    assert_eq!(view.overlay(), None);
    assert_eq!(view.notice(), None);
}

#[test]
fn overlays_never_swallow_ordinary_terminal_input() {
    for command in *b"w?" {
        let mut view = ClientView::new(120);
        let mut dispatcher = InputDispatcher::default();
        let mut input = RecordingTerminalInput::default();
        dispatch(&mut dispatcher, &mut view, &[0x02, command], &mut input);
        assert!(matches!(
            view.overlay(),
            Some(Overlay::WorkbenchNavigator | Overlay::ShortcutHelp)
        ));

        dispatch(
            &mut dispatcher,
            &mut view,
            b"text\x03\x04\x1b[A",
            &mut input,
        );
        assert!(
            input
                .recorded
                .contains(&Recorded::Forward(b"text\x03\x04\x1b[A".to_vec()))
        );
        assert!(view.overlay().is_some());
    }
}

#[test]
fn reports_an_unavailable_terminal_port_without_losing_exact_bytes() {
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput {
        unavailable: true,
        ..RecordingTerminalInput::default()
    };

    dispatch(&mut dispatcher, &mut view, b"hello", &mut input);
    assert_eq!(input.recorded, [Recorded::Forward(b"hello".to_vec())]);
    assert_eq!(view.notice(), Some(Notice::TerminalInputUnavailable));
}

#[test]
fn splits_only_at_prefix_boundaries_and_preserves_order() {
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput {
        action_disposition: Some(SessionActionDisposition::KeepNavigator),
        ..RecordingTerminalInput::default()
    };

    dispatch(
        &mut dispatcher,
        &mut view,
        b"before\x02\x02after",
        &mut input,
    );
    assert_eq!(
        input.recorded,
        [
            Recorded::Forward(b"before".to_vec()),
            Recorded::Action(ClientAction::ForwardLiteralPrefix),
            Recorded::Forward(vec![0x02]),
            Recorded::Forward(b"after".to_vec()),
        ]
    );
}

#[test]
fn observes_layout_between_a_toggle_and_later_effects_in_the_same_batch() {
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput {
        action_disposition: Some(SessionActionDisposition::KeepNavigator),
        ..RecordingTerminalInput::default()
    };

    let result = dispatch_input(
        &mut dispatcher,
        &mut view,
        b"\x02b\x02cbytes",
        &mut input,
        Instant::now(),
        |view, input| {
            input
                .recorded
                .push(Recorded::Layout(view.sidebar_surface()));
            Ok::<(), Infallible>(())
        },
    );
    assert_eq!(result, Ok(DispatchOutcome::Continue));
    assert_eq!(
        input.recorded,
        [
            Recorded::Action(ClientAction::ToggleBusinessSidebar),
            Recorded::Layout(SidebarSurface::Hidden),
            Recorded::Action(ClientAction::CreateTab),
            Recorded::Forward(b"bytes".to_vec()),
        ]
    );
}

#[test]
fn completed_unknown_action_precedes_its_parser_notice() {
    let mut view = ClientView::new(120);
    view.report_terminal_input_unavailable();
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput {
        action_disposition: Some(SessionActionDisposition::KeepNavigator),
        ..RecordingTerminalInput::default()
    };

    prefix(&mut dispatcher, &mut view, &mut input);
    assert_eq!(view.notice(), Some(Notice::TerminalInputUnavailable));
    dispatch(&mut dispatcher, &mut view, b"z", &mut input);
    assert_eq!(
        input.recorded,
        [Recorded::Action(ClientAction::UnknownPrefix)]
    );
    assert_eq!(view.notice(), Some(Notice::UnknownPrefix));
}

#[test]
fn exits_only_after_the_completed_prefixed_exit_action() {
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput {
        action_disposition: Some(SessionActionDisposition::KeepNavigator),
        ..RecordingTerminalInput::default()
    };

    assert_eq!(
        dispatch(&mut dispatcher, &mut view, b"q\x02dignored", &mut input),
        DispatchOutcome::Exit
    );
    assert_eq!(
        input.recorded,
        [
            Recorded::Forward(b"q".to_vec()),
            Recorded::Action(ClientAction::Exit),
        ]
    );
}
