//! Integrated packet capture: Settings input and old frame tails cannot reach a pane.
use super::*;
use crate::{
    client_view::KeyEvent,
    native_settings::{Edit, Editor, Row, View},
    tui_hit_map::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
    },
};
use orchbee_workbench_protocol::{
    SettingsFile, SettingsPlatform, SettingsSnapshot, TerminalDisplayAppearance,
    UserSettingsValues, WorkbenchRevision,
};
use ratatui::layout::Rect;

struct Port {
    editor: Editor,
    /// Edits the page sent to the Server.
    edits: usize,
    calls: Rc<RefCell<Vec<Call>>>,
}

fn snapshot(values: &UserSettingsValues) -> SettingsSnapshot {
    let changes = orchbee_local_runtime::value_changes(&UserSettingsValues::default(), values, &[]);
    SettingsSnapshot {
        revision: WorkbenchRevision::new(1),
        file_path: None,
        file: SettingsFile::Ok {},
        platform: SettingsPlatform::Linux,
        values: changes
            .into_iter()
            .filter_map(|change| match change {
                orchbee_workbench_protocol::SettingChange::Set {
                    key,
                    value: orchbee_workbench_protocol::SettingValue::Text(value),
                } => Some((
                    key,
                    Some(orchbee_workbench_protocol::SettingScalar::Text(value)),
                )),
                _ => None,
            })
            .collect(),
        set: Vec::new(),
        launch_profiles: values.launch_profiles.clone(),
        diagnostics: Default::default(),
    }
}

fn page(port: &Port) -> crate::native_settings::PageView<'_> {
    match port.editor.view() {
        Some(View::Page(page)) => page,
        _ => panic!("the page is open"),
    }
}

#[test]
fn search_query_and_local_return_packets_never_edit_preferences_or_reach_a_pane() {
    let mut port = Port::ready();
    let before = page(&port).values().clone();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let map = render_settings_map(&dispatcher, &mut port, 120, 40);
    dispatcher.publish_hit_map(map);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"/sound");
    let shown = page(&port);
    assert!(shown.searching() && shown.query() == "sound" && shown.values() == &before);
    let map = render_settings_map(&dispatcher, &mut port, 120, 40);
    dispatcher.publish_hit_map(map);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\r");
    let shown = page(&port);
    assert!(!shown.searching() && shown.selected() == Row::Sound && shown.values() == &before);
    let map = render_settings_map(&dispatcher, &mut port, 120, 40);
    dispatcher.publish_hit_map(map);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
    let shown = page(&port);
    assert!(shown.query().is_empty() && shown.values() == &before);
    assert_eq!(port.edits, 0);
    assert!(forwarded_bytes(&port.calls).is_empty());
}

/// Settings stays the owner after Enter, Delete or a move, so every key of one packet applies in
/// order on the state the keys before it left, and none reaches a pane.
#[test]
fn every_key_of_a_settings_packet_applies_in_order() {
    struct Case {
        name: &'static str,
        start: Row,
        /// Keys the file sets; each starts at the value opposite to its default.
        set: &'static [&'static str],
        packet: &'static [u8],
        check: fn(crate::native_settings::PageView<'_>) -> bool,
    }
    let defaults = UserSettingsValues::default();
    let cases: [Case; 3] = [
        Case {
            name: "toggle, three rows down, toggle",
            start: Row::CopyOnSelect,
            set: &[],
            packet: b"\r\x1b[B\x1b[B\x1b[B\r",
            check: |page| {
                page.selected() == Row::TrimAgents
                    && page.waits(Row::CopyOnSelect)
                    && page.waits(Row::TrimAgents)
            },
        },
        Case {
            name: "three rows down, three up, restore",
            start: Row::CopyOnSelect,
            set: &["terminal.copy_on_select"],
            packet: b"\x1b[B\x1b[B\x1b[B\x1b[A\x1b[A\x1b[A\x1b[3~",
            check: |page| {
                page.selected() == Row::CopyOnSelect
                    && page.waits(Row::CopyOnSelect)
                    && page.values().copy_on_select == UserSettingsValues::default().copy_on_select
            },
        },
        Case {
            name: "open a list, move, accept",
            start: Row::Language,
            set: &[],
            packet: b"\r\x1b[B\r",
            check: |page| page.picker().is_none() && page.waits(Row::Language),
        },
    ];
    for case in cases {
        let mut values = defaults.clone();
        if case.set.contains(&"terminal.copy_on_select") {
            values.copy_on_select = !defaults.copy_on_select;
        }
        let mut file = snapshot(&values);
        file.set = case
            .set
            .iter()
            .map(|key| orchbee_workbench_protocol::SettingPath::parse(key).unwrap())
            .collect();
        let mut editor = Editor::default();
        let token = editor.open().unwrap();
        assert!(editor.loaded(token, &file));
        editor.draw(editor.view().unwrap().token());
        editor.edit(Edit::Select(case.start));
        let mut port = Port {
            editor,
            edits: 0,
            calls: Rc::new(RefCell::new(Vec::new())),
        };
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        let map = render_settings_map(&dispatcher, &mut port, 120, 40);
        dispatcher.publish_hit_map(map);
        dispatch_all(&mut dispatcher, &mut view, &mut port, case.packet);
        assert!((case.check)(page(&port)), "{}", case.name);
        assert!(forwarded_bytes(&port.calls).is_empty(), "{}", case.name);
    }
}

impl Port {
    fn ready() -> Self {
        Self::ready_with(&UserSettingsValues::default())
    }

    /// A presented page focused on the copy-on-select toggle, so a click on it is an enabled owner.
    fn ready_with(values: &UserSettingsValues) -> Self {
        let mut editor = Editor::default();
        let token = editor.open().unwrap();
        assert!(editor.loaded(token, &snapshot(values)));
        editor.draw(editor.view().unwrap().token());
        editor.edit(Edit::Select(Row::CopyOnSelect));
        editor.draw(editor.view().unwrap().token());
        Self {
            editor,
            edits: 0,
            calls: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn capture_settings_key_for(
        &mut self,
        expected: crate::native_settings::SettingsInputContext,
        event: KeyEvent,
    ) -> bool {
        if self.editor.input_context() != Some(expected) {
            return false;
        }
        if expected.readiness == crate::native_settings::SettingsInputReadiness::Recovery {
            if matches!(
                crate::native_settings::key_edit(event, false),
                Some(Edit::Cancel)
            ) {
                self.editor.edit(Edit::Cancel);
            }
            return true;
        }
        let edit = self
            .editor
            .view()
            .and_then(|view| crate::native_settings::key_edit_for_view(event, view));
        if let Some(edit) = edit
            && self.editor.edit(edit).is_some()
        {
            self.edits += 1;
        }
        true
    }

    fn capture_settings_paste_for(
        &mut self,
        expected: crate::native_settings::SettingsInputContext,
        text: Option<&str>,
    ) -> bool {
        if self.editor.input_context() != Some(expected) {
            return false;
        }
        if expected.readiness == crate::native_settings::SettingsInputReadiness::Interactive {
            if let Some(text) = text {
                self.editor.paste(text);
            } else {
                self.editor.reject_paste();
            }
        }
        true
    }
}

impl TerminalInputPort for Port {
    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        use crate::terminal_input::{SettingsAction, SurfaceAction};
        match action {
            SurfaceAction::Settings(SettingsAction::Row { token, row }) => {
                if self.editor.click(token, row).is_some() {
                    self.edits += 1;
                }
                None
            }
            SurfaceAction::Settings(SettingsAction::Control { token, control }) => {
                if self.editor.control(token, control).is_some() {
                    self.edits += 1;
                }
                None
            }
            _ => None,
        }
    }

    fn surface_input(&mut self, input: crate::terminal_input::SurfaceInput<'_>) -> bool {
        use crate::terminal_input::{SurfaceInput, TextEntry};
        match input {
            SurfaceInput::Settings {
                expected,
                entry: TextEntry::Key(event),
            } => self.capture_settings_key_for(expected, event),
            SurfaceInput::Settings {
                expected,
                entry: TextEntry::Paste(text),
            } => self.capture_settings_paste_for(expected, text),
            _ => false,
        }
    }

    fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
        crate::tui_composition::CaptureClaims::default()
            .with_settings(self.editor.input_context().is_some())
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::WorkbenchCaptureOwner,
    ) -> Option<crate::workbench_client::CaptureContext> {
        (owner == crate::tui_composition::WorkbenchCaptureOwner::Settings)
            .then(|| self.editor.input_context())
            .flatten()
            .map(crate::workbench_client::CaptureContext::Settings)
    }

    fn apply_client_action(
        &mut self,
        command: crate::workbench_client::InputAction,
    ) -> Option<crate::client_view::ClientActionDisposition> {
        let action = command.intent();
        self.calls.borrow_mut().push(Call::Action(action.clone()));
        if action == ClientAction::OpenSettings {
            self.editor.open().unwrap();
        }
        Some(crate::client_view::ClientActionDisposition::KeepNavigator)
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.calls.borrow_mut().push(Call::Forward(bytes.to_vec()));
        Ok(())
    }

    fn scroll_list(
        &mut self,
        target: &crate::tui_hit_map::ListScrollTarget,
        up: bool,
        lines: usize,
    ) -> Option<crate::tui_hit_map::ListScrollTarget> {
        let &crate::tui_hit_map::ListScrollTarget::Settings(token) = target else {
            return None;
        };
        self.editor
            .scroll(token, up, lines)
            .map(crate::tui_hit_map::ListScrollTarget::Settings)
    }
}

fn publish_settings_row(
    dispatcher: &mut InputDispatcher,
    token: crate::native_settings::Token,
    row: Row,
    child: Option<(Rect, crate::native_settings::Control)>,
) {
    let row_rect = Rect::new(2, 2, 20, 1);
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    hits.start_settings();
    if let Some((rect, control)) = child {
        hits.record_interaction(
            HitLayer::Overlay,
            rect,
            rect,
            InteractionTarget::SettingsControl { token, control },
            spec,
        );
    }
    hits.record_interaction(
        HitLayer::Overlay,
        row_rect,
        row_rect,
        InteractionTarget::SettingsRow { token, row },
        spec,
    );
    dispatcher.publish_hit_map(hits.finish());
}

fn render_settings_map(
    dispatcher: &InputDispatcher,
    port: &mut Port,
    width: u16,
    height: u16,
) -> crate::tui_hit_map::FrameHitMap {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
        .expect("settings test terminal");
    let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    let mut presented = None;
    terminal
        .draw(|frame| {
            let presentation = crate::tui_render::render_settings_for_test(
                frame,
                frame.area(),
                port.editor.view().expect("visible settings"),
                false,
                crate::localization::Locale::EnUs,
                &crate::tui_theme::Theme::terminal_native(),
                &mut hits,
            );
            let crate::tui_presentation::ModalPresentation::Presented(draw) = presentation else {
                panic!("settings geometry is admitted")
            };
            presented = Some(draw);
        })
        .expect("settings render");
    port.editor
        .presented(presented.expect("settings draw receipt"));
    hits.finish()
}

fn capturing(port: &mut Port) -> (InputDispatcher, ClientView) {
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatcher.set_pointer_capture(&mut view, port, true);
    dispatcher
        .synchronize(&mut view, port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    (dispatcher, view)
}

/// A presented page with the custom foreground row's editor open and empty.
fn foreground_editor() -> Port {
    let mut port = Port::ready_with(&UserSettingsValues {
        terminal_display_theme: TerminalDisplayAppearance::Custom,
        ..UserSettingsValues::default()
    });
    port.editor.edit(Edit::Select(Row::Foreground));
    port.editor.draw(port.editor.view().unwrap().token());
    port.editor.edit(Edit::Activate);
    port.editor.edit(Edit::Line(
        crate::terminal_text::line_editor::LineKey::KillToStart,
    ));
    assert_eq!(page(&port).text(), Some(("", 0)));
    port
}

#[test]
fn machine_cover_keeps_retained_settings_recovery_and_owns_its_close_packet_tail() {
    let mut port = foreground_editor();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let _map = render_settings_map(&dispatcher, &mut port, 120, 40);
    view.machines.available = true;
    view.machines.show();
    // The compositor covers Settings and the completed native receipt revokes
    // its whole-surface proof while retaining the editor operation.
    port.editor.not_presented();

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27ux");
    assert!(!view.machines.open);
    assert_eq!(page(&port).text(), Some(("", 0)));

    // Settings is selected again, but stays Recovery until a fresh successful paint;
    // neither the Machine packet tail nor a later key edits.
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"y");
    assert_eq!(page(&port).text(), Some(("", 0)));
    let _map = render_settings_map(&dispatcher, &mut port, 120, 40);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"z");
    assert_eq!(page(&port).text(), Some(("z", 1)));
}

#[test]
fn settings_text_paste_is_bounded_atomic_and_never_forwarded_to_the_terminal() {
    let mut port = foreground_editor();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[200~#1234567\x1b[201~",
    );
    assert!(page(&port).invalid());
    assert_eq!(page(&port).text(), Some(("", 0)));
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[200~#ABCDEF\x1b[201~",
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\r");
    assert_eq!(page(&port).values().custom_foreground, "#abcdef");
    assert!(page(&port).text().is_none());
    assert_eq!(port.edits, 1, "Enter applies the pasted color once");
    assert!(forwarded_bytes(&port.calls).is_empty());
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
    assert_eq!(port.edits, 0);
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"normal");
    assert_eq!(forwarded_bytes(&port.calls), b"normal");
}

#[test]
fn settings_release_requires_an_unbroken_same_target_gesture() {
    let mut port = Port::ready();
    let (mut dispatcher, mut view) = capturing(&mut port);
    let token = port.editor.view().unwrap().token();
    publish_settings_row(&mut dispatcher, token, Row::CopyOnSelect, None);

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<32;30;3M");
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<32;3;3M");
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3m");

    assert_eq!(port.edits, 0);
    assert!(port.editor.visible());
    assert!(forwarded_bytes(&port.calls).is_empty());
}

#[test]
fn settings_same_token_repaint_preserves_release_but_changed_exposure_does_not() {
    let mut port = Port::ready();
    let (mut dispatcher, mut view) = capturing(&mut port);
    let token = port.editor.view().unwrap().token();
    publish_settings_row(&mut dispatcher, token, Row::CopyOnSelect, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    publish_settings_row(&mut dispatcher, token, Row::CopyOnSelect, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3m");
    assert_eq!(port.edits, 1);

    let mut port = Port::ready();
    let (mut dispatcher, mut view) = capturing(&mut port);
    let token = port.editor.view().unwrap().token();
    publish_settings_row(&mut dispatcher, token, Row::CopyOnSelect, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    publish_settings_row(
        &mut dispatcher,
        token,
        Row::CopyOnSelect,
        Some((
            Rect::new(18, 2, 3, 1),
            crate::native_settings::Control::Close,
        )),
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3m");
    assert_eq!(port.edits, 0);
    assert!(port.editor.visible());
}

#[test]
fn settings_missing_row_and_stale_owner_consume_release_without_retargeting() {
    let mut port = Port::ready();
    let (mut dispatcher, mut view) = capturing(&mut port);

    let token = port.editor.view().unwrap().token();
    publish_settings_row(&mut dispatcher, token, Row::CopyOnSelect, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    let mut missing = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    missing.start_settings();
    missing.record(
        HitLayer::OverlaySurface,
        Rect::new(0, 0, 40, 12),
        HitTarget::OverlaySurface,
    );
    dispatcher.publish_hit_map(missing.finish());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3m");
    assert_eq!(port.edits, 0);

    let token = port.editor.view().unwrap().token();
    publish_settings_row(&mut dispatcher, token, Row::CopyOnSelect, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    port.editor.edit(Edit::Previous);
    let next = port.editor.view().unwrap().token();
    assert_ne!(next, token);
    port.editor.draw(next);
    publish_settings_row(&mut dispatcher, next, Row::CopyOnSelect, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3m");
    assert_eq!(port.edits, 0);
    assert!(forwarded_bytes(&port.calls).is_empty());
}

#[test]
fn settings_ctrl_w_edits_the_captured_word_without_applying_or_forwarding() {
    let mut port = Port::ready();
    port.editor.edit(Edit::Select(Row::NewTerminalFolder));
    port.editor.draw(port.editor.view().unwrap().token());
    port.editor.edit(Edit::Activate);
    port.editor.edit(Edit::Line(
        crate::terminal_text::line_editor::LineKey::KillToStart,
    ));
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[200~one  foo.bar\x1b[201~",
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x01\x17");
    assert_eq!(page(&port).text(), Some(("one  foo.bar", 0)));
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x05\x17");
    assert_eq!(page(&port).text(), Some(("one  ", 5)));
    assert_eq!(port.edits, 0);
    assert!(forwarded_bytes(&port.calls).is_empty());
}
