use super::super::*;
use crate::{
    client_view::{ClientAction, ClientView, SessionActionDisposition},
    localization::Locale,
    terminal_input::{InputDispatcher, TerminalInputPort, TerminalInputUnavailable},
    terminal_session::{
        NativeCaptureContext, NativeSetupStage, NativeSetupToken, TerminalSessionView,
        WorkspaceCloseToken,
    },
    tui_pointer::{FrameHitMap, HitMapBuilder, HitTarget, ModalControl, MotionObservability},
    tui_theme::{Role, Theme},
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Position, style::Color};
use std::{
    io,
    time::{Duration, Instant},
};

/// A narrow production adapter: every ownership and effect decision remains
/// inside `NativeUi`, just as it does behind `TerminalSession`.
struct NativePort<'a> {
    ui: &'a mut NativeUi,
    forwarded: &'a mut Vec<u8>,
}

impl TerminalInputPort for NativePort<'_> {
    fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
        self.ui.capture_claims()
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::NativeCaptureOwner,
    ) -> Option<NativeCaptureContext> {
        self.ui.capture_context(owner)
    }

    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(input);
        Ok(())
    }

    fn click_workspace_close(&mut self, token: WorkspaceCloseToken, confirm: bool) {
        self.ui.click_workspace_close(token, confirm);
    }

    fn choose_setup_project(&mut self, token: NativeSetupToken, index: usize) {
        self.ui.choose_setup_project(token, index);
    }
}

struct RenderedFrame {
    map: FrameHitMap,
    buffer: Buffer,
    cursor: Option<Position>,
    width: u16,
}

impl RenderedFrame {
    fn background(&self, x: u16, y: u16) -> Color {
        self.buffer[(x, y)].bg
    }
}

struct Harness {
    dispatcher: InputDispatcher,
    view: ClientView,
    forwarded: Vec<u8>,
    now: Instant,
}

impl Harness {
    fn new(width: u16) -> Self {
        Self {
            dispatcher: InputDispatcher::default(),
            view: ClientView::new(width),
            forwarded: Vec::new(),
            now: Instant::now(),
        }
    }

    fn next_instant(&mut self) -> Instant {
        self.now += Duration::from_millis(1);
        self.now
    }

    fn enable_pointer(&mut self, ui: &mut NativeUi) {
        self.dispatcher.set_pointer_capture(
            &mut self.view,
            &mut NativePort {
                ui,
                forwarded: &mut self.forwarded,
            },
            true,
        );
        self.dispatcher
            .set_pointer_motion_observability(MotionObservability::AnyMotion);
        self.synchronize(ui);
    }

    fn synchronize(&mut self, ui: &mut NativeUi) {
        let now = self.next_instant();
        self.dispatcher
            .synchronize(
                &mut self.view,
                &mut NativePort {
                    ui,
                    forwarded: &mut self.forwarded,
                },
                now,
                &mut |_| Ok::<(), io::Error>(()),
            )
            .unwrap();
    }

    fn dispatch(&mut self, ui: &mut NativeUi, input: &[u8]) {
        let now = self.next_instant();
        self.dispatcher
            .dispatch(
                &mut self.view,
                input,
                &mut NativePort {
                    ui,
                    forwarded: &mut self.forwarded,
                },
                now,
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
    }

    fn render(
        &mut self,
        ui: &mut NativeUi,
        width: u16,
        height: u16,
        theme: &Theme,
        capture_name: Option<&str>,
    ) -> RenderedFrame {
        self.render_locale(ui, (width, height), Locale::EnUs, theme, capture_name)
    }

    fn render_locale(
        &mut self,
        ui: &mut NativeUi,
        size: (u16, u16),
        locale: Locale,
        theme: &Theme,
        capture_name: Option<&str>,
    ) -> RenderedFrame {
        let (width, height) = size;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut hits = HitMapBuilder::with_pointer(self.dispatcher.pointer_projection());
        let mut receipt = None;
        terminal
            .draw(|frame| {
                receipt = Some(crate::tui_render::render_terminal_shell(
                    frame,
                    &self.view,
                    locale,
                    Some(TerminalSessionView::Native(ui.view())),
                    theme,
                    &mut hits,
                ));
            })
            .unwrap();
        if let Some(name) = capture_name {
            crate::tui_render::test_capture::capture_screen(&mut terminal, name, locale, theme);
        }
        let buffer = terminal.backend().buffer().clone();
        let cursor = terminal
            .backend()
            .cursor_visible()
            .then(|| terminal.get_cursor_position().unwrap());
        let map = hits.finish();
        self.dispatcher.publish_hit_map(map.clone());
        ui.after_close_draw(receipt.unwrap());
        RenderedFrame {
            map,
            buffer,
            cursor,
            width,
        }
    }
}

fn sgr_motion(x: u16, y: u16) -> Vec<u8> {
    format!("\x1b[<35;{};{}M", x + 1, y + 1).into_bytes()
}

fn sgr_press(x: u16, y: u16) -> Vec<u8> {
    format!("\x1b[<0;{};{}M", x + 1, y + 1).into_bytes()
}

fn sgr_release(x: u16, y: u16) -> Vec<u8> {
    format!("\x1b[<0;{};{}m", x + 1, y + 1).into_bytes()
}

fn workspace_close_cell(
    frame: &RenderedFrame,
    token: WorkspaceCloseToken,
    confirm: bool,
    height: u16,
) -> (u16, u16) {
    (0..height)
        .find_map(|y| {
            (0..frame.width).find_map(|x| {
                matches!(
                    frame.map.hit_target_at(x, y),
                    Some(HitTarget::WorkspaceClose {
                        token: painted,
                        confirm: painted_confirm,
                    }) if *painted == token && *painted_confirm == confirm
                )
                .then_some((x, y))
            })
        })
        .expect("production Workspace close action")
}

fn setup_choice_cell(
    frame: &RenderedFrame,
    token: NativeSetupToken,
    index: usize,
    height: u16,
) -> (u16, u16) {
    (0..height)
        .find_map(|y| {
            (0..frame.width).find_map(|x| {
                matches!(
                    frame.map.hit_target_at(x, y),
                    Some(HitTarget::Setup {
                        token: painted,
                        control: ModalControl::Choice(painted_index),
                    }) if *painted == token && *painted_index == index
                )
                .then_some((x, y))
            })
        })
        .expect("production Setup choice row")
}

#[test]
fn workspace_close_action_roundtrips_hover_release_and_geometry_cancellation() {
    let theme = Theme::rgb_test_dark();

    let (mut ui, mailbox, _, _) = super::workspace_close::ready();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestWorkspaceClose),
        SessionActionDisposition::DismissNavigator
    );
    let token = ui.view().workspace_close_confirmation().unwrap().token();
    let mut harness = Harness::new(120);
    harness.enable_pointer(&mut ui);
    let rest = harness.render(
        &mut ui,
        120,
        40,
        &theme,
        Some("p3-workspace-close-ordinary"),
    );
    let (x, y) = workspace_close_cell(&rest, token, true, 40);

    harness.dispatch(&mut ui, &sgr_motion(x, y));
    assert!(mailbox.try_recv().unwrap().is_none());
    let hovered = harness.render(&mut ui, 120, 40, &theme, Some("p3-workspace-close-hover"));
    assert_eq!(
        hovered.background(x, y),
        theme.color(Role::SurfaceHover),
        "AnyMotion must project the exact rendered action's hover cue"
    );

    harness.dispatch(&mut ui, &sgr_press(x, y));
    assert!(ui.view().workspace_close_confirmation().is_some());
    assert!(mailbox.try_recv().unwrap().is_none(), "press cannot decide");
    let pressed = harness.render(&mut ui, 120, 40, &theme, Some("p3-workspace-close-pressed"));
    assert_eq!(pressed.background(x, y), theme.color(Role::SurfacePressed));
    harness.dispatch(&mut ui, &sgr_release(x, y));
    assert!(ui.view().workspace_close_confirmation().is_none());
    assert!(matches!(
        mailbox.try_recv().unwrap().unwrap().record(),
        QueuedCommand::Refresh(marker) if marker.workspace_close().is_some()
    ));
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(harness.forwarded.is_empty());

    let (mut ui, mailbox, _, _) = super::workspace_close::ready();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestWorkspaceClose),
        SessionActionDisposition::DismissNavigator
    );
    let token = ui.view().workspace_close_confirmation().unwrap().token();
    let mut harness = Harness::new(120);
    harness.enable_pointer(&mut ui);
    let narrow = harness.render(&mut ui, 80, 24, &theme, None);
    let (old_x, old_y) = workspace_close_cell(&narrow, token, true, 24);
    harness.dispatch(&mut ui, &sgr_press(old_x, old_y));
    assert!(mailbox.try_recv().unwrap().is_none());

    let wide = harness.render(&mut ui, 120, 40, &theme, None);
    let (new_x, new_y) = workspace_close_cell(&wide, token, true, 40);
    assert_ne!((old_x, old_y), (new_x, new_y));
    let mut late_release = sgr_release(old_x, old_y);
    late_release.extend_from_slice(b"tail");
    harness.dispatch(&mut ui, &late_release);
    assert!(ui.view().workspace_close_confirmation().is_some());
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(
        harness.forwarded.is_empty(),
        "the cancelled gesture owns its matching release packet once"
    );

    ui.click_workspace_close(token, false);
    harness.synchronize(&mut ui);
    harness.synchronize(&mut ui);
    let _uncovered = harness.render(&mut ui, 120, 40, &theme, None);
    let mut orphan = sgr_release(old_x, old_y);
    orphan.extend_from_slice(b"orphan");
    harness.dispatch(&mut ui, &orphan);
    assert_eq!(harness.forwarded, b"orphan");
}

#[test]
fn setup_project_row_roundtrips_hover_press_and_release_through_native_ui() {
    let theme = Theme::rgb_test_dark();
    let (mut ui, mailbox, _, _) = super::workspace_close::ready();
    assert_eq!(
        ui.apply_client_action(ClientAction::RequestWorkspaceMove),
        SessionActionDisposition::KeepNavigator
    );
    let draft = ui.view().setup_draft().unwrap();
    assert_eq!(draft.stage(), NativeSetupStage::ChooseWorkspaceProject);
    let token = draft.token();
    let choice = draft.project_count() - 1;
    assert_ne!(draft.selected_project(), choice);

    let mut harness = Harness::new(120);
    harness.enable_pointer(&mut ui);
    let rest = harness.render(
        &mut ui,
        120,
        40,
        &theme,
        Some("p3-setup-projects-provisional"),
    );
    let (x, y) = setup_choice_cell(&rest, token, choice, 40);

    harness.dispatch(&mut ui, &sgr_motion(x, y));
    let hovered = harness.render(&mut ui, 120, 40, &theme, Some("p3-setup-projects-hover"));
    assert_eq!(
        hovered.background(x, y),
        theme.color(Role::SurfaceHover),
        "AnyMotion must project the exact rendered Setup row's hover cue"
    );

    harness.dispatch(&mut ui, &sgr_press(x, y));
    let still_picker = ui.view().setup_draft().unwrap();
    assert_eq!(still_picker.token(), token);
    assert_eq!(
        still_picker.stage(),
        NativeSetupStage::ChooseWorkspaceProject
    );
    assert!(mailbox.try_recv().unwrap().is_none(), "press cannot choose");
    let pressed = harness.render(&mut ui, 120, 40, &theme, Some("p3-setup-projects-pressed"));
    assert_eq!(pressed.background(x, y), theme.color(Role::SurfacePressed));

    harness.dispatch(&mut ui, &sgr_release(x, y));
    let advanced = ui.view().setup_draft().unwrap();
    assert_ne!(advanced.token(), token);
    assert_eq!(advanced.stage(), NativeSetupStage::WorkspaceProjectName);
    assert!(mailbox.try_recv().unwrap().is_none());
    assert!(harness.forwarded.is_empty());
}

fn visible_row(buffer: &Buffer, y: u16) -> String {
    let mut line = String::new();
    let mut x = buffer.area.x;
    while x < buffer.area.right() {
        let symbol = buffer[(x, y)].symbol();
        line.push_str(symbol);
        x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
    }
    line
}

fn text_position(buffer: &Buffer, text: &str) -> (u16, u16) {
    (buffer.area.y..buffer.area.bottom())
        .find_map(|y| {
            let row = visible_row(buffer, y);
            let offset = row.find(text)?;
            Some((
                u16::try_from(crate::terminal_text::display_width(&row[..offset])).unwrap(),
                y,
            ))
        })
        .unwrap_or_else(|| panic!("missing popup text {text:?}"))
}

#[test]
fn project_name_popup_keeps_task_field_feedback_and_complete_actions_in_one_hierarchy() {
    use crate::{terminal_session::NativeSetupEdit, tui_theme::ColorLevel};
    use ratatui::style::Modifier;

    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (profile, theme) in [
            ("native", Theme::terminal_native()),
            ("light", Theme::one_light()),
            ("ansi16", Theme::one_dark().quantize(ColorLevel::Ansi16)),
            (
                "monochrome",
                Theme::terminal_native().quantize(ColorLevel::Monochrome),
            ),
        ] {
            for (width, height) in [(120, 40), (80, 24), (60, 18), (48, 11), (15, 4)] {
                for fault in [false, true] {
                    let (mut ui, mailbox, _, _) = super::workspace_close::ready();
                    ui.apply_client_action(ClientAction::RequestWorkspaceMove);
                    let draft = ui.view().setup_draft().unwrap();
                    let (picker_token, choice) = (draft.token(), draft.project_count() - 1);
                    let mut harness = Harness::new(120);
                    harness.render(&mut ui, 120, 40, &theme, None);
                    ui.choose_setup_project(picker_token, choice);
                    let token = ui.view().setup_draft().unwrap().token();
                    harness.render(&mut ui, 120, 40, &theme, None);
                    ui.edit_setup_draft(token, NativeSetupEdit::Paste("p1界"));
                    if fault {
                        ui.edit_setup_draft(token, NativeSetupEdit::Fault);
                    }
                    let capture = format!(
                        "popup-project-name-{locale:?}-{profile}-{width}x{height}-fault-{fault}"
                    );
                    let drawn = harness.render_locale(
                        &mut ui,
                        (width, height),
                        locale,
                        &theme,
                        Some(&capture),
                    );
                    let draft = ui.view().setup_draft().unwrap();
                    assert_eq!(draft.stage(), NativeSetupStage::WorkspaceProjectName);
                    assert_eq!(draft.text(), "p1界");
                    assert!(mailbox.try_recv().unwrap().is_none());
                    if width < 60 {
                        assert!(drawn.cursor.is_none());
                        assert!((0..height).all(|y| (0..width).all(|x| !matches!(
                            drawn.map.hit_target_at(x, y),
                            Some(HitTarget::Setup { .. })
                        ))));
                        continue;
                    }

                    let title = text_position(&drawn.buffer, locale.setup_title(draft.stage()));
                    let context = text_position(
                        &drawn.buffer,
                        &format!("{}: 项目 space 界", locale.shell().workspaces),
                    );
                    let helper = text_position(&drawn.buffer, locale.setup_input_hint(false));
                    let cursor = drawn
                        .cursor
                        .expect("active field keeps its real terminal cursor");
                    let bold = |position| drawn.buffer[position].modifier.contains(Modifier::BOLD);
                    assert!(bold(title));
                    assert!(!bold(context) && !bold(helper));
                    assert_eq!(
                        drawn.buffer[context].fg,
                        theme.style(Role::TextSecondary).fg.unwrap()
                    );
                    assert_eq!(
                        drawn.buffer[helper].fg,
                        theme.style(Role::TextSecondary).fg.unwrap()
                    );
                    assert!(title.1 < context.1 && context.1 < cursor.y && cursor.y < helper.1);
                    assert!(
                        visible_row(&drawn.buffer, cursor.y - 1)
                            .contains(locale.quick_workspace_name())
                    );
                    assert!(visible_row(&drawn.buffer, cursor.y).contains("[ p1界"));

                    let action = text_position(
                        &drawn.buffer,
                        &format!("[ {}  Enter ]", locale.setup_submit_label()),
                    );
                    let actions = visible_row(&drawn.buffer, action.1);
                    assert!(actions.contains(&format!("[ {}  Esc ]", locale.setup_cancel_label())));
                    assert!(!visible_row(&drawn.buffer, action.1 - 1).contains("Esc"));
                    assert!(helper.1 < action.1);
                    assert_eq!(
                        drawn.buffer[(action.0 + 2, action.1)].fg,
                        theme
                            .style(if fault {
                                Role::TextDisabled
                            } else {
                                Role::AccentAction
                            })
                            .fg
                            .unwrap()
                    );
                    if fault {
                        let feedback = text_position(
                            &drawn.buffer,
                            locale.setup_fault().split(['.', '。']).next().unwrap(),
                        );
                        assert_eq!(
                            drawn.buffer[(feedback.0 - 2, feedback.1)].fg,
                            theme.style(Role::StatusError).fg.unwrap()
                        );
                        assert_eq!(
                            drawn.buffer[feedback].fg,
                            theme.style(Role::TextPrimary).fg.unwrap()
                        );
                        assert!(!bold(feedback));
                    }
                }
            }
        }
    }
}
