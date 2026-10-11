//! Integrated packet capture: draft decisions and old frame tails cannot reach a pane.
use super::*;
use crate::{
    client_view::KeyEvent,
    native_settings::{Basis, Edit, Editor, Row, Values},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
};
use orchbee_workbench_protocol::{ConfigurationThemeName, NativeSettingsSource, WorkbenchRevision};
use ratatui::layout::Rect;
struct Port {
    editor: Editor,
    saves: usize,
    calls: Rc<RefCell<Vec<Call>>>,
}
impl Port {
    fn ready() -> Self {
        let mut editor = Editor::default();
        let token = editor.open().unwrap();
        assert!(editor.loaded(
            token,
            Basis {
                revision: WorkbenchRevision::new(1),
                source: NativeSettingsSource::parse("missing").unwrap(),
                values: Values {
                    theme: ConfigurationThemeName::TerminalNative,
                    reminders: true,
                    sound: false
                }
            }
        ));
        editor.edit(Edit::Adjust(true));
        editor.edit(Edit::Select(Row::Save));
        editor.draw(editor.view().unwrap().token());
        Self {
            editor,
            saves: 0,
            calls: Rc::new(RefCell::new(Vec::new())),
        }
    }
}
impl TerminalInputPort for Port {
    fn apply_client_action(
        &mut self,
        action: ClientAction,
    ) -> Option<crate::client_view::SessionActionDisposition> {
        self.calls.borrow_mut().push(Call::Action(action.clone()));
        if action == ClientAction::OpenSettings {
            self.editor.open().unwrap();
        }
        Some(crate::client_view::SessionActionDisposition::KeepNavigator)
    }
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.calls.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }
    fn settings_active(&self) -> bool {
        self.editor.visible()
    }
    fn capture_settings_key(&mut self, event: KeyEvent) {
        let edit = crate::native_settings::key_edit(event);
        if let Some(edit) = edit
            && self.editor.edit(edit).is_some()
        {
            self.saves += 1;
        }
    }
    fn click_settings(&mut self, token: crate::native_settings::Token, row: Row) {
        if self.editor.click(token, row).is_some() {
            self.saves += 1;
        }
    }
}
#[test]
fn settings_footer_click_and_prefix_binding_open_the_same_editor_and_capture_the_packet() {
    for (width, height) in [(120, 40), (80, 24), (60, 12)] {
        for keyboard in [false, true] {
            let mut port = Port::ready();
            port.editor = Editor::default();
            let mut dispatcher = InputDispatcher::default();
            let mut view = ClientView::new(width);
            view.machines.available = true;
            dispatcher.set_pointer_capture(&mut view, &mut port, true);
            if width < 96 {
                dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02B");
                assert!(view.machines.sidebar_navigation_active());
                port.calls.borrow_mut().clear();
            }
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
            let mut hits = HitMapBuilder::new();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        crate::localization::Locale::EnUs,
                        None,
                        &crate::tui_theme::Theme::terminal_native(),
                        &mut hits,
                    );
                })
                .unwrap();
            dispatcher.publish_hit_map(hits.finish());
            let click = format!("\x1b[<0;3;{height}Mtail");
            dispatch_all(
                &mut dispatcher,
                &mut view,
                &mut port,
                if keyboard {
                    b"\x02\x1bOStail"
                } else {
                    click.as_bytes()
                },
            );
            assert!(port.editor.visible());
            assert_eq!(view.overlay(), None);
            assert!(!view.machines.sidebar_navigation_active());
            assert_eq!(
                *port.calls.borrow(),
                vec![Call::Action(ClientAction::OpenSettings)]
            );
            assert!(forwarded_bytes(&port.calls).is_empty());
            assert_eq!(port.saves, 0);
            let mut settings_hits = HitMapBuilder::new();
            settings_hits.start_settings();
            dispatcher.publish_hit_map(settings_hits.finish());
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
            assert!(
                !port.editor.visible(),
                "{width}x{height}, keyboard={keyboard}"
            );
            dispatcher.publish_hit_map(HitMapBuilder::new().finish());
            dispatch_all(&mut dispatcher, &mut view, &mut port, b"after settings");
            assert_eq!(forwarded_bytes(&port.calls), b"after settings");
        }
    }
}

#[test]
fn settings_save_consumes_repeated_enter_and_paste_without_forwarding_or_replay() {
    let mut port = Port::ready();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\rignored\r\x1b[200~paste\x1b[201~",
    );
    assert_eq!(port.saves, 1);
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\r");
    assert_eq!(port.saves, 1);
    assert!(forwarded_bytes(&port.calls).is_empty());
    let token = port.editor.view().unwrap().token();
    assert!(port.editor.complete(token));
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"stale\r");
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"normal");
    assert_eq!(forwarded_bytes(&port.calls), b"normal");
}
#[test]
fn settings_cancellation_keeps_fragmented_paste_owned_across_the_new_frame() {
    let mut port = Port::ready();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[200~literal");
    port.editor.edit(Edit::Cancel);
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"text\x1b[201~");
    assert!(forwarded_bytes(&port.calls).is_empty());
    assert_eq!(port.saves, 0);
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"normal");
    assert_eq!(forwarded_bytes(&port.calls), b"normal");
}
#[test]
fn settings_save_click_and_its_late_release_are_consumed_once() {
    let mut port = Port::ready();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    let token = port.editor.view().unwrap().token();
    let mut hits = HitMapBuilder::new();
    hits.start_settings();
    hits.record(
        HitLayer::Overlay,
        Rect::new(2, 2, 20, 1),
        HitTarget::SettingsRow {
            token,
            row: Row::Save,
        },
    );
    dispatcher.publish_hit_map(hits.finish());
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;3;3Mignored",
    );
    assert_eq!(port.saves, 1);
    assert!(port.editor.complete(token));
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3mnormal");
    assert_eq!(port.saves, 1);
    assert_eq!(forwarded_bytes(&port.calls), b"normal");
}
