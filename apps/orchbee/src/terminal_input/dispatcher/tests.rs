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
    for packet in [b"[]\x02".as_slice(), b"]\x02", b"[literal"] {
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
