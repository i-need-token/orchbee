mod provenance;

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
        fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
            crate::tui_composition::NativeCaptureClaims::default().with_tab_close(self.active)
        }

        fn native_capture_context(
            &self,
            owner: crate::tui_composition::NativeCaptureOwner,
        ) -> Option<crate::terminal_session::NativeCaptureContext> {
            if owner != crate::tui_composition::NativeCaptureOwner::TabClose || !self.active {
                return None;
            }
            Some(crate::terminal_session::NativeCaptureContext::TabClose(
                crate::terminal_session::ConfirmationCaptureContext {
                    token: crate::terminal_session::NativeCloseConfirmationToken::for_test(1),
                    phase: crate::tui_presentation::PresentationPhase::Admitted,
                    readiness: crate::terminal_session::CaptureReadiness::Interactive,
                },
            ))
        }

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
        fn capture_tab_confirmation_key_for(
            &mut self,
            token: crate::terminal_session::NativeCloseConfirmationToken,
            key: crate::client_view::ClientKey,
        ) -> bool {
            if token != crate::terminal_session::NativeCloseConfirmationToken::for_test(1) {
                return false;
            }
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
        fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
            crate::tui_composition::NativeCaptureClaims::default().with_tab_close(self.active)
        }

        fn native_capture_context(
            &self,
            owner: crate::tui_composition::NativeCaptureOwner,
        ) -> Option<crate::terminal_session::NativeCaptureContext> {
            if owner != crate::tui_composition::NativeCaptureOwner::TabClose || !self.active {
                return None;
            }
            Some(crate::terminal_session::NativeCaptureContext::TabClose(
                crate::terminal_session::ConfirmationCaptureContext {
                    token: crate::terminal_session::NativeCloseConfirmationToken::for_test(1),
                    phase: crate::tui_presentation::PresentationPhase::Admitted,
                    readiness: crate::terminal_session::CaptureReadiness::Interactive,
                },
            ))
        }

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
        fn capture_tab_confirmation_key_for(
            &mut self,
            token: crate::terminal_session::NativeCloseConfirmationToken,
            key: crate::client_view::ClientKey,
        ) -> bool {
            if token != crate::terminal_session::NativeCloseConfirmationToken::for_test(1) {
                return false;
            }
            if !self.active {
                return false;
            }
            if key == crate::client_view::ClientKey::Enter {
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
    // The decision Enter and a bare ESC share the decision packet; the CSI
    // tail completing in the next read belongs to the captured frame.
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"\r\x1b",
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
        b"\r\x1b[200~",
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
#[cfg(not(windows))]
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
fn shortcut_help_captures_ordinary_input_and_consumes_escape() {
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput::default();
    dispatch(&mut dispatcher, &mut view, b"\x02?", &mut input);
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));

    dispatch(
        &mut dispatcher,
        &mut view,
        b"text\x03\x04\x1b[A",
        &mut input,
    );
    assert!(
        !input
            .recorded
            .iter()
            .any(|call| matches!(call, Recorded::Forward(_))),
        "Help owns ordinary keys, controls and escape sequences"
    );
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));

    dispatch(&mut dispatcher, &mut view, b"\x1b[27u", &mut input);
    assert_eq!(view.overlay(), None);
    assert_eq!(
        input.recorded,
        [
            Recorded::Action(ClientAction::OpenShortcutHelp),
            Recorded::Action(ClientAction::CloseShortcutHelp)
        ],
        "the consumed Escape must never reach the terminal port"
    );
}

#[test]
#[cfg(not(windows))]
fn noncapturing_navigator_preview_preserves_ordinary_terminal_input() {
    let mut view = ClientView::new(120);
    let mut dispatcher = InputDispatcher::default();
    let mut input = RecordingTerminalInput::default();
    dispatch(&mut dispatcher, &mut view, b"\x02w", &mut input);
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));

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
    assert_eq!(view.overlay(), Some(Overlay::WorkbenchNavigator));
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
    dispatch(&mut dispatcher, &mut view, b"y", &mut input);
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

#[cfg(windows)]
#[test]
fn popup_captures_prefix_escape_paste_and_retires_cross_packet_fragments() {
    use orchbee_workbench_protocol::WorkbenchRequestId;
    struct PopupPort {
        id: Option<WorkbenchRequestId>,
        interactive: bool,
        decoded: Vec<Vec<u8>>,
        forwarded: Vec<Vec<u8>>,
        mouse: Vec<WorkbenchRequestId>,
        close_attempts: Vec<WorkbenchRequestId>,
    }
    impl TerminalInputPort for PopupPort {
        fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
            crate::tui_composition::NativeCaptureClaims::default().with_popup(self.id.is_some())
        }

        fn native_capture_context(
            &self,
            owner: crate::tui_composition::NativeCaptureOwner,
        ) -> Option<crate::terminal_session::NativeCaptureContext> {
            if owner != crate::tui_composition::NativeCaptureOwner::Popup {
                return None;
            }
            self.id.clone().map(|id| {
                crate::terminal_session::NativeCaptureContext::Popup(
                    crate::terminal_session::PopupInputContext::for_test(id, self.interactive),
                )
            })
        }

        fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.forwarded.push(bytes.to_vec());
            Ok(())
        }
        fn popup_input(&mut self, id: &WorkbenchRequestId, item: &super::DecodedItem) {
            if self.id.as_ref() == Some(id) {
                self.decoded.push(item.raw.to_vec());
            }
        }
        fn popup_mouse(
            &mut self,
            id: &WorkbenchRequestId,
            _: ratatui::layout::Rect,
            _: super::MouseEvent,
        ) {
            if self.id.as_ref() != Some(id) {
                return;
            }
            self.mouse.push(id.clone());
        }
        fn activate_drawn_popup_close(
            &mut self,
            target: &crate::terminal_session::DrawnPopupClose,
        ) -> bool {
            self.close_attempts.push(target.id().clone());
            if self.id.as_ref() != Some(target.id()) {
                return false;
            }
            self.id = None;
            true
        }
    }
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let mut port = PopupPort {
        id: Some(WorkbenchRequestId::parse(&format!("{:032x}", 1)).unwrap()),
        interactive: true,
        decoded: Vec::new(),
        forwarded: Vec::new(),
        mouse: Vec::new(),
        close_attempts: Vec::new(),
    };
    let now = Instant::now();
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"\x02q\x1b[200~hello\nworld\x1b[201~",
        &mut port,
        now,
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    assert_eq!(
        port.decoded.concat(),
        b"\x02q\x1b[200~hello\nworld\x1b[201~"
    );
    assert!(port.forwarded.is_empty());
    assert!(!view.prefix_active());
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"\x1b",
        &mut port,
        now,
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            now + std::time::Duration::from_millis(100),
            &mut |_| Ok::<_, Infallible>(()),
        )
        .unwrap();
    assert_eq!(port.decoded.last().unwrap(), b"\x1b");
    // A paste from the old popup must never migrate to a replacement.
    let now = now + std::time::Duration::from_millis(200);
    for (index, tail) in [
        b"tail\x1b[201~new".as_slice(),
        b"\xfftail\x1b[201~new".as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        dispatch_input(
            &mut dispatcher,
            &mut view,
            b"\x1b[200~stale",
            &mut port,
            now,
            |_, _| Ok::<_, Infallible>(()),
        )
        .unwrap();
        port.id = Some(WorkbenchRequestId::parse(&format!("{:032x}", index + 2)).unwrap());
        let count = port.decoded.len();
        dispatch_input(&mut dispatcher, &mut view, tail, &mut port, now, |_, _| {
            Ok::<_, Infallible>(())
        })
        .unwrap();
        assert_eq!(port.decoded[count..], [vec![b'n'], vec![b'e'], vec![b'w']]);
    }
    // Closing claims unfinished CSI; only a later ordinary key reaches the Pane.
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"\x1b[",
        &mut port,
        now,
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    port.id = None;
    dispatch_input(&mut dispatcher, &mut view, b"AZ", &mut port, now, |_, _| {
        Ok::<_, Infallible>(())
    })
    .unwrap();
    assert_eq!(port.forwarded.concat(), b"Z");

    // Opening is a recovery capture for keys, but its current painted close
    // control remains an exact pointer authority. A stale map cannot close a
    // replacement, while the current close owns the rest of its read packet.
    use crate::tui_pointer::{HitLayer, HitMapBuilder};
    use ratatui::layout::Rect;
    let stale = WorkbenchRequestId::parse(&format!("{:032x}", 19)).unwrap();
    let current = WorkbenchRequestId::parse(&format!("{:032x}", 20)).unwrap();
    let close_map = |id: WorkbenchRequestId| {
        let mut hits = HitMapBuilder::new();
        hits.start_modal();
        let popup = crate::terminal_session::PopupView {
            id: &id,
            title: "Dashboard",
            projection: None,
            width: None,
            height: None,
            closing: false,
        };
        hits.record_interaction(
            HitLayer::Overlay,
            Rect::new(10, 5, 1, 1),
            Rect::new(10, 5, 1, 1),
            crate::tui_pointer::InteractionTarget::PopupClose(popup.close_target()),
            crate::tui_pointer::InteractionSpec::enabled(
                crate::tui_pointer::PrimaryContract::ActivateOnRelease,
            ),
        );
        hits.finish()
    };
    port.id = Some(current.clone());
    port.interactive = false;
    port.decoded.clear();
    port.forwarded.clear();
    port.mouse.clear();
    port.close_attempts.clear();
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .synchronize(&mut view, &mut port, now, &mut |_| Ok::<_, Infallible>(()))
        .unwrap();

    dispatcher.publish_hit_map(close_map(stale.clone()));
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"\x1b[<0;11;6M\x1b[<0;11;6m",
        &mut port,
        now,
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    assert_eq!(port.id.as_ref(), Some(&current));
    assert!(port.mouse.is_empty());
    assert_eq!(port.close_attempts, [stale]);

    dispatcher.publish_hit_map(close_map(current.clone()));
    port.close_attempts.clear();
    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"\x1b[<0;11;6M\x1b[<0;11;6mtail\x1b[200~paste\x1b[201~",
        &mut port,
        now,
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    assert!(port.mouse.is_empty());
    assert_eq!(port.close_attempts, [current]);
    assert!(port.id.is_none());
    assert!(port.decoded.is_empty());
    assert!(port.forwarded.is_empty());

    dispatch_input(
        &mut dispatcher,
        &mut view,
        b"normal",
        &mut port,
        now,
        |_, _| Ok::<_, Infallible>(()),
    )
    .unwrap();
    assert_eq!(port.forwarded.concat(), b"normal");
}
