use super::*;
use crate::terminal_input::TerminalInputUnavailable;
use std::{cell::RefCell, io, rc::Rc};

#[derive(Debug, Eq, PartialEq)]
enum Call {
    Forward(Vec<u8>),
    Mode(bool),
    Action(ClientAction),
}

struct UnavailablePort(Rc<RefCell<Vec<Call>>>);

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
