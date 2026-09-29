use super::*;
use crate::terminal_input::TerminalInputUnavailable;
use std::{cell::RefCell, io, rc::Rc};

#[cfg(windows)]
mod configured;
mod workspace_setup;

#[derive(Debug, Eq, PartialEq)]
enum Call {
    Forward(Vec<u8>),
    Focus(bool),
    Mode(ModeRequest),
    Action(ClientAction),
    BoardSelect(orchbee_agent_observation_protocol::AgentSessionId),
}

struct UnavailablePort(Rc<RefCell<Vec<Call>>>);

#[test]
fn context_prefix_across_reads_consumes_only_the_navigation_keys() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let packets: [&[u8]; 3] = [b"[]\x02", b"]\x02", b"[literal"];
    for packet in packets {
        assert_eq!(
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<(), io::Error>(()),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DispatchOutcome::Continue,
        );
    }
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"[]".to_vec()),
            Call::Action(ClientAction::SelectNextContext),
            Call::Action(ClientAction::SelectPreviousContext),
            Call::Forward(b"literal".to_vec()),
        ],
    );
    assert!(!view.prefix_active());
}

impl TerminalInputPort for UnavailablePort {
    fn observe_focus(&mut self, focused: bool) {
        self.0.borrow_mut().push(Call::Focus(focused));
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.0.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.0.borrow_mut().push(Call::Action(action));
        None
    }
}

#[test]
fn failed_mode_write_precedes_and_prevents_native_draft_request_and_packet_tail() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let result = dispatcher.dispatch(
        &mut view,
        b"before\x02,after",
        &mut port,
        Instant::now(),
        |_, _| Ok(()),
        &mut |request| {
            calls.borrow_mut().push(Call::Mode(request.clone()));
            if matches!(request, ModeRequest::BracketedPaste(true)) {
                Err(io::Error::other("partial enable write"))
            } else {
                Ok(())
            }
        },
    );
    assert!(result.is_err());
    assert_eq!(
        *calls.borrow(),
        [
            Call::Mode(ModeRequest::BracketedPaste(false)),
            Call::Forward(b"before".to_vec()),
            Call::Mode(ModeRequest::BracketedPaste(true))
        ]
    );
}

#[test]
fn prefix_then_arrow_sequence_is_one_unbound_event_without_tail_leak() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    assert_eq!(
        dispatcher
            .dispatch(
                &mut view,
                b"\x02\x1b[D",
                &mut port,
                Instant::now(),
                |_, _| Ok::<(), io::Error>(()),
                &mut |_| Ok(()),
            )
            .unwrap(),
        DispatchOutcome::Continue,
    );
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::UnknownPrefix)]);
    assert_eq!(
        view.notice(),
        Some(crate::client_view::Notice::UnknownPrefix)
    );
    assert!(!view.prefix_active());
}

#[test]
fn prefix_then_csi_split_across_reads_is_still_one_event() {
    let patterns: [&[&[u8]]; 3] = [
        &[b"\x02\x1b", b"[D"],
        &[b"\x02", b"\x1b", b"[", b"D"],
        &[b"\x02\x1b[", b"D"],
    ];
    for packets in patterns {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        for &packet in packets {
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<(), io::Error>(()),
                    &mut |_| Ok(()),
                )
                .unwrap();
        }
        assert_eq!(
            *calls.borrow(),
            [Call::Action(ClientAction::UnknownPrefix)],
            "split pattern {packets:?}",
        );
        assert!(!view.prefix_active());
    }
}

#[test]
fn prefix_then_chinese_scalar_is_one_unbound_event_without_continuation_leak() {
    let patterns: [&[&[u8]]; 4] = [
        &[b"\x02\xe4\xb8\xad"],
        &[b"\x02\xe4", b"\xb8\xad"],
        &[b"\x02", b"\xe4", b"\xb8", b"\xad"],
        &[b"\x02\xe4\xb8", b"\xad"],
    ];
    for packets in patterns {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        for &packet in packets {
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<(), io::Error>(()),
                    &mut |_| Ok(()),
                )
                .unwrap();
        }
        assert_eq!(
            *calls.borrow(),
            [Call::Action(ClientAction::UnknownPrefix)],
            "split pattern {packets:?}",
        );
        assert!(!view.prefix_active());
    }
}

#[test]
fn prefix_then_emoji_split_across_reads_is_one_unbound_event() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let packets: [&[u8]; 4] = [b"\x02\xf0", b"\x9f", b"\x98", b"\x80"];
    for packet in packets {
        dispatcher
            .dispatch(
                &mut view,
                packet,
                &mut port,
                Instant::now(),
                |_, _| Ok::<(), io::Error>(()),
                &mut |_| Ok(()),
            )
            .unwrap();
    }
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::UnknownPrefix)]);
    assert!(!view.prefix_active());
}

#[test]
fn lone_escape_is_forwarded_only_after_the_bounded_wait() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let start = Instant::now();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b",
            &mut port,
            start,
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(calls.borrow().is_empty());
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            start + std::time::Duration::from_millis(60),
            &mut |_| Ok::<(), io::Error>(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Forward(b"\x1b".to_vec())]);
}

#[test]
fn alt_modified_key_does_not_match_a_plain_prefix_binding() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher
        .dispatch(
            &mut view,
            b"\x02\x1ba",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::UnknownPrefix)]);
    assert!(!view.prefix_active());
}

#[test]
fn paste_payload_never_enters_prefix_matching_and_forwards_whole() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher
        .dispatch(
            &mut view,
            b"\x02\x1b[200~x\x1b[201~",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(
        *calls.borrow(),
        [Call::Forward(b"\x1b[200~x\x1b[201~".to_vec())]
    );
    assert!(view.prefix_active());
}

#[test]
fn paste_between_ordinary_bytes_forwards_as_one_atomic_run() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher
        .dispatch(
            &mut view,
            b"ab\x1b[200~xy\x1b[201~cd",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(
        *calls.borrow(),
        [Call::Forward(b"ab\x1b[200~xy\x1b[201~cd".to_vec())]
    );
}

fn forwarded(calls: &Rc<RefCell<Vec<Call>>>) -> Vec<u8> {
    calls
        .borrow()
        .iter()
        .flat_map(|call| match call {
            Call::Forward(bytes) => bytes.clone(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn a_utf8_continuation_9b_is_not_a_c1_paste_terminator() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let packets: [&[u8]; 3] = [b"\x1b[200~ab\xc2\x9b201~", b"\x02q", b"\x1b[201~"];
    for packet in packets {
        assert_eq!(
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<(), io::Error>(()),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DispatchOutcome::Continue,
        );
    }
    // The `\x9b` inside a UTF-8 scalar is a continuation byte, not a C1
    // terminator: the run stays paste until `ESC[201~` in its own notation,
    // so the `\x02q` payload never reaches the prefix layer.
    assert_eq!(
        forwarded(&calls),
        b"\x1b[200~ab\xc2\x9b201~\x02q\x1b[201~".to_vec()
    );
}

#[test]
fn a_csi_tail_after_the_byte_cap_stays_owned_until_the_final_byte() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let mut first = b"\x1b[".to_vec();
    first.extend(std::iter::repeat_n(b'0', 62));
    let packets: [&[u8]; 3] = [&first, b"\x02q", b"A"];
    for packet in packets {
        assert_eq!(
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<(), io::Error>(()),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DispatchOutcome::Continue,
        );
    }
    let mut expected = first;
    expected.extend_from_slice(b"\x02qA");
    assert_eq!(forwarded(&calls), expected);
}

#[test]
fn an_ss3_tail_after_the_byte_cap_stays_owned_until_the_final_byte() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let packets: [&[u8]; 3] = [b"\x1bO1234567", b"\x02q", b"A"];
    for packet in packets {
        assert_eq!(
            dispatcher
                .dispatch(
                    &mut view,
                    packet,
                    &mut port,
                    Instant::now(),
                    |_, _| Ok::<(), io::Error>(()),
                    &mut |_| Ok(()),
                )
                .unwrap(),
            DispatchOutcome::Continue,
        );
    }
    assert_eq!(forwarded(&calls), b"\x1bO1234567\x02qA".to_vec());
}

#[test]
fn an_esc_inside_a_csi_aborts_the_sequence_instead_of_leaving_residue() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    for packet in [&b"\x02"[..], b"\x1b[3\x1b[31m"] {
        dispatcher
            .dispatch(
                &mut view,
                packet,
                &mut port,
                Instant::now(),
                |_, _| Ok::<(), io::Error>(()),
                &mut |_| Ok(()),
            )
            .unwrap();
    }
    // The aborted first sequence consumes the armed prefix; the complete
    // second sequence is one non-decision item whose raw forwards whole —
    // never `3`,`1`,`m` as residue key events.
    assert_eq!(
        *calls.borrow(),
        [
            Call::Action(ClientAction::UnknownPrefix),
            Call::Forward(b"\x1b[31m".to_vec()),
        ]
    );
    assert!(!view.prefix_active());
}

#[test]
fn a_truncated_csi_releases_its_bytes_after_the_bounded_wait_then_drains() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let start = Instant::now();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b[12",
            &mut port,
            start,
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(calls.borrow().is_empty());
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            start + std::time::Duration::from_millis(60),
            &mut |_| Ok::<(), io::Error>(()),
        )
        .unwrap();
    // The truncated run is emitted once the bounded wait expires.
    assert_eq!(*calls.borrow(), [Call::Forward(b"\x1b[12".to_vec())]);
    // A late tail of the same frame drains without reaching bindings.
    calls.borrow_mut().clear();
    dispatcher
        .dispatch(
            &mut view,
            b"\x02qA",
            &mut port,
            start + std::time::Duration::from_millis(70),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Forward(b"\x02qA".to_vec())]);
    assert!(!view.prefix_active());
}

#[test]
fn a_paste_past_the_raw_cap_drains_until_the_terminator() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let mut first = b"\x1b[200~".to_vec();
    first.extend(std::iter::repeat_n(b'a', 4 * 1024 * 1024));
    dispatcher
        .dispatch(
            &mut view,
            &first,
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    dispatcher
        .dispatch(
            &mut view,
            b"\x02q\x1b[201~z",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    // The over-cap run drains to its real terminator as bounded
    // non-decision chunks, so the pane still receives every raw byte in
    // order — including the `\x02`/`q` payload — while none of it can reach
    // the binding layer. Only `z` is fresh input after the terminator.
    let forwarded = forwarded(&calls);
    assert_eq!(forwarded.len(), first.len() + 2 + 6 + 1);
    assert_eq!(&forwarded[forwarded.len() - 7..], b"\x1b[201~z");
    assert!(!view.prefix_active());
}

#[test]
fn unavailable_entry_restores_mode_and_preserves_ordinary_batch_order() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    assert_eq!(
        dispatcher
            .dispatch(
                &mut view,
                b"before\x02,after",
                &mut port,
                Instant::now(),
                |_, _| Ok::<_, io::Error>(()),
                &mut |request| {
                    calls.borrow_mut().push(Call::Mode(request.clone()));
                    Ok(())
                },
            )
            .unwrap(),
        DispatchOutcome::Continue
    );
    assert_eq!(
        *calls.borrow(),
        [
            Call::Mode(ModeRequest::BracketedPaste(false)),
            Call::Forward(b"before".to_vec()),
            Call::Mode(ModeRequest::BracketedPaste(true)),
            Call::Action(ClientAction::RequestRenameTab),
            Call::Mode(ModeRequest::BracketedPaste(false)),
            Call::Forward(b"after".to_vec())
        ]
    );
}

#[test]
fn prefixed_m_toggles_pointer_capture_through_the_mode_lease() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // The shell settles the armed state after its mode write; from here
    // the binding releases and re-arms through the same lease.
    dispatcher.set_pointer_capture(&mut view, true);
    for expected in [false, true] {
        dispatcher
            .dispatch(
                &mut view,
                b"\x02m",
                &mut port,
                Instant::now(),
                |_, _| Ok::<_, io::Error>(()),
                &mut |request| {
                    calls.borrow_mut().push(Call::Mode(request.clone()));
                    Ok(())
                },
            )
            .unwrap();
        // The prefix arm itself takes the paste lease; the assertion is
        // that exactly one pointer request of the expected direction ran.
        assert_eq!(
            calls
                .borrow()
                .iter()
                .filter(|call| { matches!(call, Call::Mode(ModeRequest::PointerCapture(_))) })
                .count(),
            1
        );
        assert!(
            calls
                .borrow()
                .contains(&Call::Mode(ModeRequest::PointerCapture(expected)))
        );
        assert_eq!(dispatcher.pointer_capture_enabled(), expected);
        calls.borrow_mut().clear();
    }
}

#[test]
fn workspace_shortcuts_acquire_the_modal_lease_and_leave_exit_bindings_intact() {
    for (key, action) in [
        (b'a', ClientAction::RequestWorkspaceCreate),
        (b'e', ClientAction::RequestWorkspaceDirectory),
        (b'r', ClientAction::RequestWorkspaceRename),
        (b'R', ClientAction::RequestProjectRename),
    ] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut port = UnavailablePort(Rc::clone(&calls));
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        dispatcher
            .dispatch(
                &mut view,
                &[2, key],
                &mut port,
                Instant::now(),
                |_, _| Ok::<_, io::Error>(()),
                &mut |request| {
                    calls.borrow_mut().push(Call::Mode(request.clone()));
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(
            *calls.borrow(),
            [
                Call::Mode(ModeRequest::BracketedPaste(false)),
                Call::Mode(ModeRequest::BracketedPaste(true)),
                Call::Action(action),
                Call::Mode(ModeRequest::BracketedPaste(false))
            ]
        );
        for exit in *b"qd" {
            assert_eq!(
                dispatcher
                    .dispatch(
                        &mut view,
                        &[2, exit],
                        &mut port,
                        Instant::now(),
                        |_, _| Ok::<_, io::Error>(()),
                        &mut |_| Ok(())
                    )
                    .unwrap(),
                DispatchOutcome::Exit
            );
        }
    }
}

fn forwarded_bytes(calls: &Rc<RefCell<Vec<Call>>>) -> Vec<u8> {
    calls
        .borrow()
        .iter()
        .filter_map(|call| match call {
            Call::Forward(bytes) => Some(bytes.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn dispatch_all(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    port: &mut UnavailablePort,
    bytes: &[u8],
) {
    dispatcher
        .dispatch(
            view,
            bytes,
            port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
}

#[test]
fn malformed_mouse_reports_are_consumed_and_never_forwarded_to_the_pane() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // Malformed SGR shape, malformed X10 body, then an ordinary key: the
    // mouse-shaped bytes are pointer protocol — dropped whole, only the
    // real key reaches the pane.
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"k\x1b[<0;0;1M\x1b[M\x20\x20\x29z",
    );
    assert_eq!(forwarded_bytes(&calls), b"kz".to_vec());
}

#[test]
fn aborted_and_overlong_mouse_runs_never_forward_their_bytes() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // An ESC-aborted SGR partial: `\x1b[<0;` drops, the abort's arrow key
    // is client input — an unbound CSI, forwarded as one non-decision.
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;\x1b[Ak");
    assert_eq!(forwarded_bytes(&calls), b"\x1b[Ak".to_vec());
    // An overlong SGR parameter plus its release-final twin drop whole.
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;111111;1M\x1b[<999999;1;1mz",
    );
    assert_eq!(forwarded_bytes(&calls), b"\x1b[Akz".to_vec());
}

#[test]
fn an_x10_abort_and_timeout_drop_without_leaking_and_the_decoder_recovers() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // ESC aborts a two-byte-in X10 body; the reprocessed `ESC[D` is client
    // input — forwarded — but the partial frame's bytes never are.
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[M\x20\x1b[Dk");
    assert_eq!(forwarded_bytes(&calls), b"\x1b[Dk".to_vec());
}

#[test]
fn unknown_csi_and_paste_keep_their_passthrough_alongside_dropped_mouse() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // Unknown non-mouse CSI still passes through in order; a bracketed
    // paste forwards whole; only the malformed mouse bytes vanish.
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"k\x1b[?25h\x1b[<0;0;1M\x1b[200~hi\x1b[201~z",
    );
    assert_eq!(
        forwarded_bytes(&calls),
        b"k\x1b[?25h\x1b[200~hi\x1b[201~z".to_vec()
    );
}

#[test]
fn prefixed_z_issues_the_client_local_zoom_action() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatcher
        .dispatch(
            &mut view,
            b"z",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::ToggleZoom)]);
    assert!(!view.prefix_active());
}

struct LayoutChangedPort(Rc<RefCell<Vec<Call>>>);

impl TerminalInputPort for LayoutChangedPort {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.0.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.0.borrow_mut().push(Call::Action(action));
        Some(SessionActionDisposition::LayoutChanged)
    }
}

#[test]
fn a_layout_changed_disposition_reobserves_layout_and_stales_the_pointer_map() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = LayoutChangedPort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let layout_passes = Rc::new(RefCell::new(0usize));
    let count = Rc::clone(&layout_passes);
    dispatcher
        .dispatch(
            &mut view,
            b"z",
            &mut port,
            Instant::now(),
            |_, _| {
                *count.borrow_mut() += 1;
                Ok::<(), io::Error>(())
            },
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::Action(ClientAction::ToggleZoom)]);
    // The transition itself did not flag a layout change, so the layout pass
    // is attributed to the returned disposition alone.
    assert_eq!(*layout_passes.borrow(), 1);
}

/// A port whose board-ownership flag the test flips directly — the same
/// value the session's painted-verdict flag drives in production — plus
/// a record of the board select calls the pointer path produces.
struct BoardPort {
    calls: Rc<RefCell<Vec<Call>>>,
    owned: Rc<std::cell::Cell<bool>>,
}

impl TerminalInputPort for BoardPort {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.calls.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        self.calls.borrow_mut().push(Call::Action(action));
        None
    }

    fn agent_board_input(&mut self) -> bool {
        self.owned.get()
    }

    fn agent_board_select(
        &mut self,
        session: &orchbee_agent_observation_protocol::AgentSessionId,
    ) -> bool {
        self.calls
            .borrow_mut()
            .push(Call::BoardSelect(session.clone()));
        true
    }
}

#[test]
fn a_board_close_edge_invalidates_the_published_hit_map() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(true));
    let mut port = BoardPort {
        calls: Rc::clone(&calls),
        owned: Rc::clone(&owned),
    };
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let session = orchbee_agent_observation_protocol::AgentSessionId::parse("s-1").unwrap();
    dispatcher.set_pointer_capture(&mut view, true);
    // The open edge settles first so the map published below is the
    // current authority, matching the painted board.
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    let mut hits = crate::tui_pointer::HitMapBuilder::new();
    hits.mark_overlay_active();
    hits.record(
        crate::tui_pointer::HitLayer::Overlay,
        ratatui::layout::Rect::new(0, 0, 10, 1),
        crate::tui_pointer::HitTarget::AgentBoardRow(session.clone()),
    );
    dispatcher.publish_hit_map(hits.finish());
    // Baseline: the press resolves the painted row.
    dispatcher
        .dispatch(
            &mut view,
            b"[<0;1;1M",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::BoardSelect(session.clone())]);
    // The close edge turns the recorded regions into lies about a
    // surface that is no longer painted: the next synchronize drops the
    // whole map and the same press resolves nothing.
    owned.set(false);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    calls.borrow_mut().clear();
    dispatcher
        .dispatch(
            &mut view,
            b"[<0;1;1M",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(
        calls.borrow().is_empty(),
        "the stale row must not resolve after the close edge"
    );
}

#[test]
fn the_board_open_edge_also_invalidates_so_a_click_cannot_miss_the_overlay() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(false));
    let mut port = BoardPort {
        calls: Rc::clone(&calls),
        owned: Rc::clone(&owned),
    };
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    let session = orchbee_agent_observation_protocol::AgentSessionId::parse("s-1").unwrap();
    dispatcher.set_pointer_capture(&mut view, true);
    // A foreground region the last frame recorded under where the board
    // is about to paint — resolving by identity, it still answers while
    // the map is current.
    let mut hits = crate::tui_pointer::HitMapBuilder::new();
    hits.record(
        crate::tui_pointer::HitLayer::Foreground,
        ratatui::layout::Rect::new(0, 0, 10, 1),
        crate::tui_pointer::HitTarget::AgentBoardRow(session.clone()),
    );
    dispatcher.publish_hit_map(hits.finish());
    dispatcher
        .dispatch(
            &mut view,
            b"[<0;1;1M",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*calls.borrow(), [Call::BoardSelect(session.clone())]);
    // Opening the board stales that map: the region it describes now
    // sits under a capturing overlay, so the same press must drop
    // instead of reaching through.
    owned.set(true);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    calls.borrow_mut().clear();
    dispatcher
        .dispatch(
            &mut view,
            b"[<0;1;1M",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(
        calls.borrow().is_empty(),
        "the stale foreground region must not resolve past the open board"
    );
}

#[test]
fn an_open_board_swallows_a_paste_inside_the_armed_prefix() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let owned = Rc::new(std::cell::Cell::new(true));
    let mut port = BoardPort {
        calls: Rc::clone(&calls),
        owned: Rc::clone(&owned),
    };
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    // A real input stream: the chord arms the prefix, then a bracketed
    // paste completes in the same read. The painted board owns the run
    // whole — no bytes forward, no binding decides it, and the prefix
    // stays armed for the next key.
    dispatcher
        .dispatch(
            &mut view,
            b"\x02\x1b[200~payload\x1b[201~",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(
        calls.borrow().is_empty(),
        "the board swallows the paste whole: nothing may reach the pane"
    );
    assert!(view.prefix_active());
    // The arm was never spent: the next key still resolves its binding —
    // `g` is the board's own toggle.
    dispatcher
        .dispatch(
            &mut view,
            b"g",
            &mut port,
            Instant::now(),
            |_, _| Ok::<(), io::Error>(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(
        *calls.borrow(),
        [Call::Action(ClientAction::ToggleAgentBoard)]
    );
    assert!(!view.prefix_active());
}

#[test]
fn focus_metadata_preserves_input_order_and_an_armed_prefix() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut port = UnavailablePort(Rc::clone(&calls));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    for packet in [b"before\x1b[".as_slice(), b"Oafter\x02\x1b[I".as_slice()] {
        dispatcher
            .dispatch(
                &mut view,
                packet,
                &mut port,
                Instant::now(),
                |_, _| Ok::<(), io::Error>(()),
                &mut |_| Ok(()),
            )
            .unwrap();
    }
    assert!(view.prefix_active());
    assert_eq!(
        *calls.borrow(),
        [
            Call::Forward(b"before".to_vec()),
            Call::Focus(false),
            Call::Forward(b"after".to_vec()),
            Call::Focus(true)
        ]
    );
}
