use super::*;
use crate::client_view::{KeyCode, KeyEvent};
use orchbee_local_runtime::{KeyChord, KeyOverride, KeySettings, KeyTrigger};

/// A port whose copy slot holds the Find bar once Find in terminal runs.
#[derive(Default)]
struct FindPort {
    open: bool,
    forwarded: Vec<u8>,
    keys: Vec<KeyEvent>,
    actions: Vec<ClientAction>,
}

impl TerminalInputPort for FindPort {
    fn surface_input(&mut self, input: crate::terminal_input::SurfaceInput<'_>) -> bool {
        use crate::terminal_input::{SurfaceInput, TextEntry};
        match input {
            SurfaceInput::CopyMode(TextEntry::Key(event)) => {
                self.keys.push(event);
                if event.code == KeyCode::Esc {
                    self.open = false;
                }
                true
            }
            _ => false,
        }
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }

    fn apply_client_action(
        &mut self,
        command: crate::workbench_client::InputAction,
    ) -> Option<ClientActionDisposition> {
        let action = command.intent();
        if action == ClientAction::FindInTerminal {
            self.open = true;
        }
        self.actions.push(action);
        Some(ClientActionDisposition::DismissNavigator)
    }

    fn copy_mode_active(&self) -> bool {
        self.open
    }

    fn find_bar_active(&self) -> bool {
        self.open
    }
}

fn typed(port: &FindPort) -> String {
    port.keys
        .iter()
        .filter_map(|event| match event.code {
            // Enter arrives as CR and decodes as Ctrl+M.
            KeyCode::Char(character) if !event.mods.ctrl => Some(character),
            _ => None,
        })
        .collect()
}

#[test]
fn prefix_f_opens_find_and_the_typed_query_never_reaches_the_pane() {
    let mut port = FindPort::default();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(100);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02f");
    assert_eq!(port.actions, [ClientAction::FindInTerminal]);
    assert!(port.open && view.find_bar_active());

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"error\r");
    assert_eq!(typed(&port), "error");
    assert!(port.forwarded.is_empty());

    // The prefix stays live over the bar; its action runs instead of editing the query.
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02f");
    assert_eq!(
        port.actions,
        [ClientAction::FindInTerminal, ClientAction::FindInTerminal]
    );
    assert_eq!(typed(&port), "error");

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b");
    dispatcher
        .synchronize(
            &mut view,
            &mut port,
            Instant::now() + Duration::from_millis(100),
            &mut |_| Ok::<_, io::Error>(()),
        )
        .unwrap();
    assert!(!port.open && !view.find_bar_active());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"ls");
    assert_eq!(port.forwarded, b"ls");
}

#[test]
fn a_configured_direct_chord_opens_find_through_the_same_route() {
    let keys = KeySettings {
        commands: Vec::new(),
        prefix: KeyChord {
            key: "b".into(),
            ctrl: true,
            alt: false,
            shift: false,
            super_key: false,
        },
        overrides: vec![KeyOverride {
            action: "find",
            triggers: vec![KeyTrigger {
                prefix: false,
                chord: KeyChord {
                    key: "f".into(),
                    ctrl: true,
                    alt: false,
                    shift: true,
                    super_key: false,
                },
            }],
        }],
    };
    let mut view = ClientView::new(100);
    view.configure_keys(&keys);
    let mut port = FindPort::default();
    let mut dispatcher = InputDispatcher::default();
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[102;6u");
    assert_eq!(port.actions, [ClientAction::FindInTerminal]);
    assert!(view.find_bar_active());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"x");
    assert_eq!(typed(&port), "x");
    assert!(port.forwarded.is_empty());
}
