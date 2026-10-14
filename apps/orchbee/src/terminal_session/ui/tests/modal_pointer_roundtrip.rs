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
use ratatui::{Terminal, backend::TestBackend, style::Color};
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
    backgrounds: Vec<Color>,
    width: u16,
}

impl RenderedFrame {
    fn background(&self, x: u16, y: u16) -> Color {
        self.backgrounds[usize::from(y) * usize::from(self.width) + usize::from(x)]
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
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut hits = HitMapBuilder::with_pointer(self.dispatcher.pointer_projection());
        let mut receipt = None;
        terminal
            .draw(|frame| {
                receipt = Some(crate::tui_render::render_terminal_shell(
                    frame,
                    &self.view,
                    Locale::EnUs,
                    Some(TerminalSessionView::Native(ui.view())),
                    theme,
                    &mut hits,
                ));
            })
            .unwrap();
        if let Some(name) = capture_name {
            crate::tui_render::test_capture::capture_screen(
                &mut terminal,
                name,
                Locale::EnUs,
                theme,
            );
        }
        let backgrounds = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.bg)
            .collect();
        let map = hits.finish();
        self.dispatcher.publish_hit_map(map.clone());
        ui.after_close_draw(receipt.unwrap());
        RenderedFrame {
            map,
            backgrounds,
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
