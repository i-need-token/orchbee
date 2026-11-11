//! Integrated packet capture: draft decisions and old frame tails cannot reach a pane.
use super::*;
use crate::{
    client_view::{KeyEvent, Overlay},
    native_settings::{Basis, Edit, Editor, Row},
    tui_hit_map::{
        HitLayer, HitMapBuilder, HitTarget, InteractionSpec, InteractionTarget, PrimaryContract,
    },
};
use ratatui::layout::Rect;
struct Port {
    editor: Editor,
    saves: usize,
    calls: Rc<RefCell<Vec<Call>>>,
}

#[test]
fn search_query_and_local_return_packets_never_edit_preferences_or_reach_a_pane() {
    let mut port = Port::ready();
    let before = match port.editor.view().unwrap() {
        crate::native_settings::View::Draft(draft) => draft.values.clone(),
        _ => panic!("draft"),
    };
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    let map = render_settings_map(&dispatcher, &mut port, 120, 40);
    dispatcher.publish_hit_map(map);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"/sound");
    assert!(
        matches!(port.editor.view(), Some(crate::native_settings::View::Draft(draft)) if draft.searching && draft.query == "sound" && draft.values == &before)
    );
    let map = render_settings_map(&dispatcher, &mut port, 120, 40);
    dispatcher.publish_hit_map(map);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\r");
    assert!(
        matches!(port.editor.view(), Some(crate::native_settings::View::Draft(draft)) if !draft.searching && draft.selected == Row::Sound && draft.values == &before)
    );
    let map = render_settings_map(&dispatcher, &mut port, 120, 40);
    dispatcher.publish_hit_map(map);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
    assert!(
        matches!(port.editor.view(), Some(crate::native_settings::View::Draft(draft)) if draft.query.is_empty() && draft.values == &before)
    );
    assert_eq!(port.saves, 0);
    assert!(forwarded_bytes(&port.calls).is_empty());
}
impl Port {
    fn ready() -> Self {
        Self::ready_with(orchbee_workbench_protocol::UserSettingsValues::default())
    }

    fn ready_with(values: orchbee_workbench_protocol::UserSettingsValues) -> Self {
        let mut editor = Editor::default();
        let token = editor.open().unwrap();
        assert!(editor.loaded(
            token,
            Basis {
                revision: 1,
                values,
            }
        ));
        // Activation is authorized only after this incarnation has actually
        // been presented. Dirty the draft after that boundary so Save is a
        // real enabled owner in pointer-release tests.
        editor.draw(editor.view().unwrap().token());
        editor.edit(Edit::Adjust(true));
        editor.edit(Edit::Select(Row::Save));
        editor.draw(editor.view().unwrap().token());
        assert!(matches!(
            editor.view(),
            Some(crate::native_settings::View::Draft(draft))
                if draft.dirty && draft.selected == Row::Save
        ));
        Self {
            editor,
            saves: 0,
            calls: Rc::new(RefCell::new(Vec::new())),
        }
    }
}
impl Port {
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
            self.saves += 1;
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

    fn click_settings(&mut self, token: crate::native_settings::Token, row: Row) {
        if self.editor.click(token, row).is_some() {
            self.saves += 1;
        }
    }

    fn click_settings_control(
        &mut self,
        token: crate::native_settings::Token,
        control: crate::native_settings::Control,
    ) {
        if self.editor.control(token, control).is_some() {
            self.saves += 1;
        }
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
                self.click_settings(token, row);
                None
            }
            SurfaceAction::Settings(SettingsAction::Control { token, control }) => {
                self.click_settings_control(token, control);
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

fn target_cell(
    map: &crate::tui_hit_map::FrameHitMap,
    target: &HitTarget,
    width: u16,
    height: u16,
) -> (u16, u16) {
    (0..height)
        .flat_map(|row| (0..width).map(move |column| (column, row)))
        .find(|&(column, row)| map.hit_target_at(column, row) == Some(target))
        .unwrap_or_else(|| panic!("target was not painted: {target:?}"))
}

fn primary_packet((column, row): (u16, u16), release: bool) -> Vec<u8> {
    format!(
        "\x1b[<0;{};{}{}",
        column + 1,
        row + 1,
        if release { 'm' } else { 'M' }
    )
    .into_bytes()
}

#[test]
fn settings_confirmation_uses_owner_key_map_and_retains_non_enter_decisions_and_packet_tails() {
    let mut port = Port::ready();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    port.editor.edit(Edit::Select(Row::ResetAll));
    let _map = render_settings_map(&dispatcher, &mut port, 120, 40);
    port.editor.edit(Edit::Activate);
    for packet in [
        b" ".as_slice(),
        b"y",
        b"\x1b[13;5u",
        b"\x1b[13;2u",
        b"\x1b[200~\r\x1b[201~",
    ] {
        let map = render_settings_map(&dispatcher, &mut port, 120, 40);
        dispatcher.publish_hit_map(map);
        let token = port.editor.view().unwrap().token();
        dispatch_all(&mut dispatcher, &mut view, &mut port, packet);
        assert!(
            matches!(port.editor.view(), Some(crate::native_settings::View::Draft(draft))
            if draft.token == token && draft.confirmation == Some(crate::native_settings::Confirmation::ResetAll))
        );
        assert_eq!(port.saves, 0);
        assert!(port.calls.borrow().is_empty());
    }
    let map = render_settings_map(&dispatcher, &mut port, 120, 40);
    dispatcher.publish_hit_map(map);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\rnever-forward-this",
    );
    assert!(
        matches!(port.editor.view(), Some(crate::native_settings::View::Draft(draft))
        if draft.confirmation.is_none() && draft.dirty)
    );
    assert_eq!(port.saves, 0, "confirmation edits only the draft");
    assert!(
        port.calls.borrow().is_empty(),
        "the decision packet cannot reach a pane"
    );
}

#[test]
fn settings_wheel_never_wraps_or_edits_values_and_keyboard_cancels_backlog() {
    const WIDTH: u16 = 80;
    const HEIGHT: u16 = 12;
    let wheel = |cell: (u16, u16), down: bool, count: usize| {
        format!(
            "\x1b[<{};{};{}M",
            if down { 65 } else { 64 },
            cell.0 + 1,
            cell.1 + 1
        )
        .repeat(count)
        .into_bytes()
    };
    let painted_list_cell = |map: &crate::tui_hit_map::FrameHitMap, port: &Port| {
        let crate::native_settings::View::Draft(draft) = port.editor.view().expect("painted draft")
        else {
            panic!("loaded settings draft")
        };
        // Category landmarks share the viewport with settings. Use the painted list owner,
        // including its blank/scrollbar track, rather than indexing a separate settings catalog.
        target_cell(
            map,
            &HitTarget::ListScroll(crate::tui_hit_map::ListScrollTarget::Settings(draft.token)),
            WIDTH,
            HEIGHT,
        )
    };

    let mut port = Port::ready();
    port.editor.edit(Edit::Home);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(WIDTH);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<_, io::Error>(())
        })
        .unwrap();

    let map = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
    let scroll_cell = painted_list_cell(&map, &port);
    dispatcher.publish_hit_map(map);
    let (values, initial_viewport) = match port.editor.view().expect("painted draft") {
        crate::native_settings::View::Draft(draft) => {
            assert_eq!(draft.selected, Row::Language);
            (draft.values.clone(), draft.viewport)
        }
        _ => panic!("loaded settings draft"),
    };
    assert_eq!(initial_viewport, 0);

    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        &wheel(scroll_cell, false, 1),
    );
    assert!(matches!(
        port.editor.view(),
        Some(crate::native_settings::View::Draft(draft))
            if draft.selected == Row::Language
                && draft.viewport == initial_viewport
                && draft.values == &values
    ));

    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        &wheel(scroll_cell, true, 20),
    );
    let scrolled_viewport = match port.editor.view().expect("scrolled draft") {
        crate::native_settings::View::Draft(draft) => {
            assert_eq!(draft.selected, Row::Language);
            assert_eq!(draft.values, &values);
            draft.viewport
        }
        _ => panic!("loaded settings draft"),
    };
    assert!(scrolled_viewport > initial_viewport);

    // The first bounded wheel pass leaves a same-stream remainder. A keyboard
    // decision cancels that remainder before a freshly painted map can admit it.
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[H");
    let map = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
    dispatcher.publish_hit_map(map);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<_, io::Error>(())
        })
        .unwrap();
    assert!(matches!(
        port.editor.view(),
        Some(crate::native_settings::View::Draft(draft))
            if draft.selected == Row::Language
                && draft.viewport == initial_viewport
                && draft.values == &values
    ));

    for _ in 0..32 {
        let map = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
        let scroll_cell = painted_list_cell(&map, &port);
        dispatcher.publish_hit_map(map);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            &wheel(scroll_cell, true, 1),
        );
    }
    let bottom = match port.editor.view().expect("bottom draft") {
        crate::native_settings::View::Draft(draft) => {
            assert_eq!(draft.selected, Row::Language);
            assert_eq!(draft.values, &values);
            draft.viewport
        }
        _ => panic!("loaded settings draft"),
    };
    assert!(bottom > initial_viewport);
    let map = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
    let scroll_cell = painted_list_cell(&map, &port);
    dispatcher.publish_hit_map(map);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        &wheel(scroll_cell, true, 1),
    );
    assert!(matches!(
        port.editor.view(),
        Some(crate::native_settings::View::Draft(draft))
            if draft.selected == Row::Language
                && draft.viewport == bottom
                && draft.values == &values
    ));
    assert_eq!(port.saves, 0);
    assert!(forwarded_bytes(&port.calls).is_empty());
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
            let click = format!("\x1b[<0;3;{height}M\x1b[<0;3;{height}mtail");
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
fn machine_cover_keeps_retained_settings_recovery_and_owns_its_close_packet_tail() {
    let mut port = Port::ready();
    port.editor.edit(Edit::Select(Row::TerminalTheme));
    for _ in 0..2 {
        port.editor.edit(Edit::Adjust(true));
    }
    port.editor.edit(Edit::Select(Row::Foreground));
    port.editor.edit(Edit::Activate);
    port.editor.edit(Edit::Line(
        crate::terminal_text::line_editor::LineKey::KillToStart,
    ));

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
    assert!(matches!(
        port.editor.view(),
        Some(crate::native_settings::View::Draft(draft)) if draft.text == Some(("", 0))
    ));

    // The retained parent is selected again, but stays Recovery until a fresh
    // successful paint; neither the Machine packet tail nor a later key edits.
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"y");
    assert!(matches!(
        port.editor.view(),
        Some(crate::native_settings::View::Draft(draft)) if draft.text == Some(("", 0))
    ));
    let _map = render_settings_map(&dispatcher, &mut port, 120, 40);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"z");
    assert!(matches!(
        port.editor.view(),
        Some(crate::native_settings::View::Draft(draft)) if draft.text == Some(("z", 1))
    ));
}

#[test]
fn settings_save_consumes_repeated_enter_and_paste_without_forwarding_or_replay() {
    const WIDTH: u16 = 120;
    const HEIGHT: u16 = 40;
    let mut port = Port::ready();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(WIDTH);

    port.editor.edit(Edit::Select(Row::Language));
    let map = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
    dispatcher.publish_hit_map(map);
    let before = match port.editor.view().expect("painted draft") {
        crate::native_settings::View::Draft(draft) => draft.values.clone(),
        _ => panic!("loaded settings draft"),
    };
    port.editor.edit(Edit::Adjust(true));
    assert!(matches!(
        port.editor.view(),
        Some(crate::native_settings::View::Draft(draft)) if draft.values != &before
    ));
    port.editor.edit(Edit::Select(Row::Save));
    let map = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
    let token = port.editor.view().expect("painted Save draft").token();
    let save = HitTarget::SettingsRow {
        token,
        row: Row::Save,
    };
    let _save_cell = target_cell(&map, &save, WIDTH, HEIGHT);
    dispatcher.publish_hit_map(map);

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
fn settings_text_paste_is_bounded_atomic_and_never_forwarded_to_the_terminal() {
    let mut port = Port::ready();
    port.editor.edit(Edit::Select(Row::TerminalTheme));
    for _ in 0..2 {
        port.editor.edit(Edit::Adjust(true));
    }
    port.editor.edit(Edit::Select(Row::Foreground));
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
        b"\x1b[200~#1234567\x1b[201~",
    );
    assert!(
        matches!(port.editor.view(), Some(crate::native_settings::View::Draft(draft)) if draft.invalid && draft.text == Some(("",0)))
    );
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[200~#ABCDEF\x1b[201~",
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\rpacket tail");
    assert!(
        matches!(port.editor.view(), Some(crate::native_settings::View::Draft(draft)) if draft.values.custom_foreground == "#abcdef" && draft.text.is_none())
    );
    assert_eq!(port.saves, 0);
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
    publish_settings_row(&mut dispatcher, token, Row::Save, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    assert_eq!(port.saves, 0);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;3;3mignored",
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

#[test]
fn settings_cannot_hide_an_older_modal_release_or_make_it_fence_twice() {
    let mut port = Port {
        editor: Editor::default(),
        saves: 0,
        calls: Rc::new(RefCell::new(Vec::new())),
    };
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(120);
    view.commit(
        view.decide_action(ClientAction::OpenShortcutHelp),
        crate::client_view::ClientActionDisposition::KeepNavigator,
    );
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    let source = view
        .help_draw_candidate()
        .and_then(|candidate| candidate.presented(0, 0))
        .unwrap();
    let mut hits = HitMapBuilder::with_pointer(dispatcher.pointer_projection());
    hits.record_help_draw(source);
    let rect = Rect::new(10, 5, 8, 1);
    hits.record_interaction(
        HitLayer::Overlay,
        rect,
        rect,
        InteractionTarget::Help(crate::client_view::help::DrawnHelpControl::new(
            source,
            crate::client_view::help::HelpControl::Close,
        )),
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
    );
    let source = dispatcher.publish_hit_map_with_help(hits.finish());
    view.settle_help_draw(source);

    // Help only arms on press. A higher owner may replace it before release,
    // but that exact physical release remains cleanup-only.
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;12;6M");
    assert_eq!(view.overlay(), Some(Overlay::ShortcutHelp));
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    // Settings is a prefix-group binding: Ctrl+B, then F4.
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x02\x1bOS");
    assert!(port.editor.visible(), "F4 opens the newer Settings owner");
    assert_eq!(view.overlay(), None);
    port.calls.borrow_mut().clear();

    // Pre-owner cleanup wins even though Settings would otherwise intercept
    // this report and its packet tail.
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;12;6mtail\x1b[200~paste\x1b[201~\x1b[",
    );
    assert!(forwarded_bytes(&port.calls).is_empty());
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"A");
    assert!(forwarded_bytes(&port.calls).is_empty());

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
    assert!(!port.editor.visible());
    dispatcher.publish_hit_map(HitMapBuilder::new().finish());
    port.calls.borrow_mut().clear();
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        b"\x1b[<0;12;6morphan",
    );
    assert_eq!(forwarded_bytes(&port.calls), b"orphan");
}

#[test]
fn settings_real_frame_routes_row_and_adjustment_release_to_the_exact_owner() {
    const WIDTH: u16 = 120;
    const HEIGHT: u16 = 40;
    let mut port = Port::ready();
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(WIDTH);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();

    let token = port.editor.view().expect("draft").token();
    let sound = HitTarget::SettingsRow {
        token,
        row: Row::Sound,
    };
    let first = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
    let sound_cell = target_cell(&first, &sound, WIDTH, HEIGHT);
    dispatcher.publish_hit_map(first);
    let sound_before = match port.editor.view().expect("draft") {
        crate::native_settings::View::Draft(draft) => draft.values.sound_enabled,
        _ => panic!("loaded settings draft"),
    };
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        &primary_packet(sound_cell, false),
    );
    assert_eq!(
        match port.editor.view().expect("draft") {
            crate::native_settings::View::Draft(draft) => draft.values.sound_enabled,
            _ => panic!("loaded settings draft"),
        },
        sound_before,
        "press only arms the painted row"
    );
    let pressed = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
    assert_eq!(
        target_cell(&pressed, &sound, WIDTH, HEIGHT),
        sound_cell,
        "a visual-only pressed repaint keeps the row geometry"
    );
    dispatcher.publish_hit_map(pressed);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        &primary_packet(sound_cell, true),
    );
    assert_eq!(
        match port.editor.view().expect("draft") {
            crate::native_settings::View::Draft(draft) => draft.values.sound_enabled,
            _ => panic!("loaded settings draft"),
        },
        !sound_before
    );

    port.editor.edit(Edit::Select(Row::CompletionThreshold));
    let token = port.editor.view().expect("draft").token();
    let forward = HitTarget::SettingsControl {
        token,
        control: crate::native_settings::Control::Adjust {
            row: Row::CompletionThreshold,
            forward: true,
        },
    };
    let first = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
    let forward_cell = target_cell(&first, &forward, WIDTH, HEIGHT);
    dispatcher.publish_hit_map(first);
    let threshold_before = match port.editor.view().expect("draft") {
        crate::native_settings::View::Draft(draft) => {
            draft.values.completion_reminder_minimum_seconds
        }
        _ => panic!("loaded settings draft"),
    };
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        &primary_packet(forward_cell, false),
    );
    let pressed = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
    assert_eq!(
        target_cell(&pressed, &forward, WIDTH, HEIGHT),
        forward_cell,
        "the frontmost adjustment remains the armed target"
    );
    dispatcher.publish_hit_map(pressed);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        &primary_packet(forward_cell, true),
    );
    let crate::native_settings::View::Draft(draft) = port.editor.view().expect("draft") else {
        panic!("loaded settings draft")
    };
    assert_eq!(
        draft.values.completion_reminder_minimum_seconds,
        threshold_before + 1
    );
    assert_eq!(draft.selected, Row::CompletionThreshold);
    assert!(draft.text.is_none(), "the parent row was not activated");
}

#[test]
fn settings_real_frame_disabled_adjustment_consumes_without_parent_retarget() {
    const WIDTH: u16 = 120;
    const HEIGHT: u16 = 40;
    let values = orchbee_workbench_protocol::UserSettingsValues {
        completion_reminder_minimum_seconds: 1,
        ..Default::default()
    };
    let mut port = Port::ready_with(values);
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(WIDTH);
    dispatcher.set_pointer_capture(&mut view, &mut port, true);
    dispatcher
        .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();

    let token = port.editor.view().expect("draft").token();
    let backward = HitTarget::SettingsControl {
        token,
        control: crate::native_settings::Control::Adjust {
            row: Row::CompletionThreshold,
            forward: false,
        },
    };
    let map = render_settings_map(&dispatcher, &mut port, WIDTH, HEIGHT);
    let backward_cell = target_cell(&map, &backward, WIDTH, HEIGHT);
    dispatcher.publish_hit_map(map);
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        &primary_packet(backward_cell, false),
    );
    dispatch_all(
        &mut dispatcher,
        &mut view,
        &mut port,
        &primary_packet(backward_cell, true),
    );

    let crate::native_settings::View::Draft(draft) = port.editor.view().expect("draft") else {
        panic!("loaded settings draft")
    };
    assert_eq!(draft.values.completion_reminder_minimum_seconds, 1);
    assert_eq!(draft.selected, Row::Save);
    assert!(draft.text.is_none());
    assert_eq!(port.saves, 0);
    assert!(forwarded_bytes(&port.calls).is_empty());
}

#[test]
fn settings_release_requires_an_unbroken_same_target_gesture() {
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
    publish_settings_row(&mut dispatcher, token, Row::Save, None);

    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<32;30;3M");
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<32;3;3M");
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3m");

    assert_eq!(port.saves, 0);
    assert!(port.editor.visible());
    assert!(forwarded_bytes(&port.calls).is_empty());
}

#[test]
fn settings_same_token_repaint_preserves_release_but_changed_exposure_does_not() {
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
    publish_settings_row(&mut dispatcher, token, Row::Save, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    publish_settings_row(&mut dispatcher, token, Row::Save, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3m");
    assert_eq!(port.saves, 1);

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
    publish_settings_row(&mut dispatcher, token, Row::Save, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    publish_settings_row(
        &mut dispatcher,
        token,
        Row::Save,
        Some((
            Rect::new(18, 2, 3, 1),
            crate::native_settings::Control::Close,
        )),
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3m");
    assert_eq!(port.saves, 0);
    assert!(port.editor.visible());
}

#[test]
fn settings_missing_row_and_stale_owner_consume_release_without_retargeting() {
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
    publish_settings_row(&mut dispatcher, token, Row::Save, None);
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
    assert_eq!(port.saves, 0);

    let token = port.editor.view().unwrap().token();
    publish_settings_row(&mut dispatcher, token, Row::Save, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3M");
    port.editor.edit(Edit::Previous);
    let next = port.editor.view().unwrap().token();
    assert_ne!(next, token);
    port.editor.draw(next);
    publish_settings_row(&mut dispatcher, next, Row::Save, None);
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x1b[<0;3;3m");
    assert_eq!(port.saves, 0);
    assert!(forwarded_bytes(&port.calls).is_empty());
}

#[test]
fn settings_ctrl_w_edits_the_captured_word_without_saving_or_forwarding() {
    let mut port = Port::ready();
    port.editor.edit(Edit::Select(Row::LaunchProfiles));
    port.editor.edit(Edit::Activate);
    port.editor.edit(Edit::Select(Row::ProfileName));
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
    assert!(
        matches!(port.editor.view(), Some(crate::native_settings::View::Draft(draft)) if draft.text == Some(("one  foo.bar", 0)))
    );
    dispatch_all(&mut dispatcher, &mut view, &mut port, b"\x05\x17");
    assert!(
        matches!(port.editor.view(), Some(crate::native_settings::View::Draft(draft)) if draft.text == Some(("one  ", 5)))
    );
    assert_eq!(port.saves, 0);
    assert!(forwarded_bytes(&port.calls).is_empty());
}

#[test]
fn settings_backdrop_retains_drafts_children_confirmations_and_inflight_operations() {
    use crate::native_settings::{Confirmation, Control, View};

    for task in 0..13 {
        let mut port = Port::ready();
        if !matches!(task, 0 | 2 | 3 | 7 | 12) {
            port.editor.edit(Edit::Select(Row::Language));
            port.editor.edit(Edit::Adjust(false));
            port.editor.edit(Edit::Select(Row::Save));
            port.editor.draw(port.editor.view().unwrap().token());
        }
        let mut accepted = None;
        match task {
            0 => {} // Dirty root.
            1 => {
                port.editor.edit(Edit::Select(Row::CompletionThreshold));
                port.editor.edit(Edit::Activate);
                port.editor.edit(Edit::Line(
                    crate::terminal_text::line_editor::LineKey::KillToStart,
                ));
                port.editor.paste("17");
            }
            2 | 3 => {
                port.editor.edit(Edit::Select(Row::LaunchProfiles));
                port.editor.edit(Edit::Activate);
                port.editor.edit(Edit::Select(Row::AddProfile));
                port.editor.edit(Edit::Activate);
                port.editor.edit(Edit::Select(Row::ProfileName));
                port.editor.edit(Edit::Activate);
                port.editor.edit(Edit::Line(
                    crate::terminal_text::line_editor::LineKey::KillToStart,
                ));
                port.editor.paste("Temporary command · unsaved");
                port.editor.edit(Edit::Activate);
                port.editor.edit(Edit::Select(Row::CommandType));
                port.editor.edit(Edit::Activate);
                port.editor.edit(Edit::Select(Row::Executable));
                port.editor.edit(Edit::Activate);
                port.editor.paste(if cfg!(windows) {
                    "C:\\tools\\shell.exe"
                } else {
                    "/bin/sh"
                });
                port.editor.edit(Edit::Activate);
                port.editor.edit(Edit::Select(Row::AddArgument));
                port.editor.edit(Edit::Activate);
                port.editor.edit(Edit::Activate);
                port.editor.paste("-NoProfile");
                port.editor.edit(Edit::Activate);
                if task == 3 {
                    port.editor.edit(Edit::Select(Row::ResetProfiles));
                    port.editor.edit(Edit::Activate);
                }
            }
            4..=6 | 8 => {
                port.editor.edit(Edit::Select(if task == 8 {
                    Row::Reload
                } else {
                    Row::ResetAll
                }));
                port.editor.edit(Edit::Activate);
                if task == 6 || task == 8 {
                    port.editor.draw(port.editor.view().unwrap().token());
                    accepted = port
                        .editor
                        .edit(Edit::Activate)
                        .map(|submission| submission.token);
                } else if task == 5 {
                    port.editor.edit(Edit::Cancel);
                    port.editor.edit(Edit::Select(Row::Reload));
                    port.editor.edit(Edit::Activate);
                }
            }
            7 | 12 => {
                accepted = Some(
                    port.editor
                        .edit(Edit::Activate)
                        .expect("accepted Save")
                        .token,
                );
                if task == 12 {
                    assert!(port.editor.failed(
                        accepted.unwrap(),
                        Some(crate::native_settings::Reconciliation::OutcomeUnknown)
                    ));
                }
            }
            9 => {
                port.editor.retire();
                accepted = port.editor.open();
            }
            10 | 11 => {
                port.editor.edit(Edit::Select(Row::LaunchProfiles));
                port.editor.edit(Edit::Activate);
                if task == 11 {
                    port.editor.edit(Edit::Select(Row::ResetProfiles));
                    port.editor.edit(Edit::Activate);
                }
            }
            _ => unreachable!(),
        }
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                Ok::<(), io::Error>(())
            })
            .unwrap();
        let snapshot = |view: View<'_>| match view {
            View::Draft(draft) => Some((
                draft.values.clone(),
                draft.basis_revision,
                draft.selected,
                draft.page,
                draft.profile,
                draft.argument,
                draft.text.map(|(text, cursor)| (text.to_owned(), cursor)),
                draft.confirmation,
            )),
            _ => None,
        };
        let before = port.editor.view().unwrap();
        if matches!(task, 1 | 4 | 5 | 10 | 11) {
            assert!(
                matches!(before, View::Draft(draft) if !draft.dirty),
                "task {task} protects a clean active task"
            );
        }
        assert!(!before.allows_backdrop_close(), "task {task}");
        let token = before.token();
        let retained = snapshot(before);
        let map = render_settings_map(&dispatcher, &mut port, 120, 40);
        dispatcher.publish_hit_map(map);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            &primary_packet((0, 0), false),
        );
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            &primary_packet((0, 0), true),
        );
        let after = port.editor.view().expect("outside click retains Settings");
        assert_eq!(after.token(), token, "task {task}");
        assert_eq!(snapshot(after), retained, "task {task}");
        assert_eq!(port.saves, 0, "outside click cannot submit or replay work");
        assert!(forwarded_bytes(&port.calls).is_empty());

        if task == 7 {
            assert!(port.editor.saving(), "Save remains visible and in flight");
            assert!(port.editor.open().is_none());
            assert!(port.editor.failed(accepted.unwrap(), None));
            assert!(matches!(port.editor.view(), Some(View::Draft(draft)) if draft.dirty));
        } else if task == 8 || task == 9 {
            assert!(matches!(port.editor.view(), Some(View::Loading { .. })));
            assert!(port.editor.loaded(
                accepted.unwrap(),
                Basis {
                    revision: 1,
                    values: Default::default(),
                }
            ));
            assert!(matches!(port.editor.view(), Some(View::Draft(draft)) if !draft.dirty));
        } else if task == 3 || task == 4 || task == 5 {
            let expected = match task {
                3 => Confirmation::ResetProfiles,
                4 => Confirmation::ResetAll,
                _ => Confirmation::Reload,
            };
            assert!(
                matches!(port.editor.view(), Some(View::Draft(draft)) if draft.confirmation == Some(expected))
            );
        }
        let current = port.editor.view().unwrap().token();
        port.editor.control(current, Control::Close);
        assert!(
            !port.editor.visible(),
            "explicit Close still exits task {task}"
        );
    }
}

#[test]
fn settings_clean_backdrop_still_closes_but_a_new_edit_revokes_an_armed_close() {
    for edit_after_press in [false, true] {
        let mut port = Port::ready();
        port.editor.edit(Edit::Select(Row::Language));
        port.editor.edit(Edit::Adjust(false));
        assert!(port.editor.view().unwrap().allows_backdrop_close());
        let mut dispatcher = InputDispatcher::default();
        let mut view = ClientView::new(120);
        dispatcher.set_pointer_capture(&mut view, &mut port, true);
        dispatcher
            .synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                Ok::<(), io::Error>(())
            })
            .unwrap();
        let map = render_settings_map(&dispatcher, &mut port, 120, 40);
        dispatcher.publish_hit_map(map);
        dispatch_all(
            &mut dispatcher,
            &mut view,
            &mut port,
            &primary_packet((0, 0), false),
        );
        if edit_after_press {
            port.editor.edit(Edit::Select(Row::Sound));
            port.editor.edit(Edit::Activate);
            let map = render_settings_map(&dispatcher, &mut port, 120, 40);
            dispatcher.publish_hit_map(map);
        }
        let mut release = primary_packet((0, 0), true);
        release.extend_from_slice(b"must-not-reach-pane");
        dispatch_all(&mut dispatcher, &mut view, &mut port, &release);
        assert_eq!(port.editor.visible(), edit_after_press);
        assert!(forwarded_bytes(&port.calls).is_empty());
        assert_eq!(port.saves, 0);
    }
}
