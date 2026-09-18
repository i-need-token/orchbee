use super::*;
use crate::terminal_input::TerminalInputUnavailable;
use std::{cell::RefCell, io, rc::Rc};

mod workspace_setup;

#[derive(Debug, Eq, PartialEq)]
enum Call {
    Forward(Vec<u8>),
    Mode(bool),
    Action(ClientAction),
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
        &mut |enabled| {
            calls.borrow_mut().push(Call::Mode(enabled));
            if enabled {
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
            Call::Mode(false),
            Call::Forward(b"before".to_vec()),
            Call::Mode(true)
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
                &mut |enabled| {
                    calls.borrow_mut().push(Call::Mode(enabled));
                    Ok(())
                },
            )
            .unwrap(),
        DispatchOutcome::Continue
    );
    assert_eq!(
        *calls.borrow(),
        [
            Call::Mode(false),
            Call::Forward(b"before".to_vec()),
            Call::Mode(true),
            Call::Action(ClientAction::RequestRenameTab),
            Call::Mode(false),
            Call::Forward(b"after".to_vec())
        ]
    );
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
                &mut |enabled| {
                    calls.borrow_mut().push(Call::Mode(enabled));
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(
            *calls.borrow(),
            [
                Call::Mode(false),
                Call::Mode(true),
                Call::Action(action),
                Call::Mode(false)
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
