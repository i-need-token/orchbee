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
