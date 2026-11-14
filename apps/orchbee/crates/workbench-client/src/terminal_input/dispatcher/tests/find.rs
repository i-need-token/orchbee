use super::*;
use crate::client_view::{KeyCode, KeyEvent};
use crate::terminal_copy_mode::EpisodeId;
use crate::terminal_find::{FindControl, FindToken};
use crate::tui_hit_map::{HitLayer, HitMapBuilder, HitTarget, InteractionAvailability};
use orchbee_local_runtime::{KeyChord, KeyOverride, KeySettings, KeyTrigger};
use ratatui::layout::Rect;
use std::num::NonZeroU64;

/// A port whose copy slot holds the Find bar once Find in terminal runs.
#[derive(Default)]
struct FindPort {
    open: bool,
    forwarded: Vec<u8>,
    keys: Vec<KeyEvent>,
    actions: Vec<ClientAction>,
    pointer: Vec<crate::workbench_client::input::FindAction>,
}

impl TerminalInputPort for FindPort {
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<ClientActionDisposition> {
        use crate::workbench_client::input::FindAction;
        let crate::terminal_input::SurfaceAction::Find(action) = action else {
            return None;
        };
        self.pointer.push(action);
        if matches!(
            action,
            FindAction::Close(_)
                | FindAction::Control {
                    control: FindControl::Close,
                    ..
                }
        ) {
            self.open = false;
        }
        Some(ClientActionDisposition::KeepNavigator)
    }

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

#[test]
fn a_release_on_close_closes_the_bar_and_a_press_on_the_content_reaches_no_pane() {
    use crate::workbench_client::input::FindAction;
    let token = FindToken(EpisodeId::new(NonZeroU64::new(1).unwrap()));
    let close = Rect::new(30, 1, 3, 1);
    let content = Rect::new(0, 1, 40, 10);
    for on_content in [false, true] {
        let mut port = FindPort::default();
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(100);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatch_all(&mut dispatcher, &mut view, &mut port, b"f");
        assert!(port.open);
        let mut hits = HitMapBuilder::new();
        hits.record_modal_control(
            HitLayer::Chrome,
            close,
            HitTarget::FindControl {
                token,
                control: FindControl::Close,
            },
            InteractionAvailability::Enabled,
        );
        hits.record(
            HitLayer::Chrome,
            Rect::new(8, 1, 28, 1),
            HitTarget::FindSurface(token),
        );
        hits.record(
            HitLayer::ChromeSurface,
            content,
            HitTarget::FindContent(token),
        );
        dispatcher.publish_hit_map(hits.finish());

        let (column, row) = if on_content { (5, 6) } else { (31, 1) };
        let at = |end: char| format!("[<0;{};{}{end}", column + 1, row + 1);
        dispatch_all(&mut dispatcher, &mut view, &mut port, at('M').as_bytes());
        assert_eq!(
            port.open, !on_content,
            "a control acts on its release, content on press"
        );
        dispatch_all(&mut dispatcher, &mut view, &mut port, at('m').as_bytes());
        assert!(!port.open);
        let expected = if on_content {
            FindAction::Close(token)
        } else {
            FindAction::Control {
                token,
                control: FindControl::Close,
            }
        };
        assert_eq!(port.pointer, [expected]);
        assert!(
            port.forwarded.is_empty(),
            "the program sees neither press nor release"
        );
    }
}
