//! Root-compositor evidence: one painted owner and one exact draw settlement per frame.

use super::super::*;
use crate::agent_board::AgentBoardEpisodeToken;
use crate::client_view::{
    Binding, ClientView, KeyCode, KeyEvent, LabelStyle, Modifiers, PrefixDesc, format_binding,
};
use crate::localization::{Locale, RecoveryVerb};
use crate::native_settings::Basis;
use crate::tui_hit_map::{FrameHitMap, HitMapBuilder, HitTarget, ListScrollTarget};
use crate::tui_presentation::PresentationPhase;
use crate::tui_theme::Role;
use crate::tui_theme::Theme;
use crate::workbench_client::DrawReceipt;
use orchbee_workbench_protocol::UserPreferencesValues;
use ratatui::layout::Rect;
use ratatui::{Terminal, backend::TestBackend};

const WIDTH: u16 = 120;
const FULL_HEIGHT: u16 = 40;
const REFUSED_HEIGHT: u16 = 8;

struct RenderedFrame {
    terminal: Terminal<TestBackend>,
    hits: FrameHitMap,
    receipt: DrawReceipt,
    width: u16,
    height: u16,
}

impl RenderedFrame {
    fn text(&self) -> String {
        self.terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn has_hit(&self, mut predicate: impl FnMut(&HitTarget) -> bool) -> bool {
        for y in 0..self.height {
            for x in 0..self.width {
                if let Some(target) = self.hits.hit_target_at(x, y)
                    && predicate(target)
                {
                    return true;
                }
            }
        }
        false
    }
}

fn render(
    ui: &WorkbenchClientUi,
    shell: &ClientView,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> RenderedFrame {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut receipt = DrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                shell,
                locale,
                Some(ui.view()),
                theme,
                &mut hits,
            );
        })
        .unwrap();
    RenderedFrame {
        terminal,
        hits: hits.finish(),
        receipt,
        width,
        height,
    }
}

fn board_hit(target: &HitTarget) -> bool {
    matches!(
        target,
        HitTarget::AgentBoardRow(_)
            | HitTarget::AgentBoardControl(_)
            | HitTarget::ListScroll(ListScrollTarget::AgentBoard(_))
    )
}

fn admit_board(
    ui: &mut WorkbenchClientUi,
    shell: &ClientView,
    locale: Locale,
    theme: &Theme,
) -> AgentBoardEpisodeToken {
    ui.board.open();
    let episode = ui.board.episode();
    let frame = render(ui, shell, WIDTH, FULL_HEIGHT, locale, theme);
    assert!(frame.has_hit(board_hit), "the Board must actually paint");
    ui.after_close_draw(frame.receipt);
    let context = ui.board.input_context().expect("open Board");
    assert_eq!(context.episode(), episode);
    assert_eq!(context.presentation(), PresentationPhase::Admitted);
    assert!(context.interactive());
    episode
}

fn open_loaded_settings(ui: &mut WorkbenchClientUi) {
    let token = ui.settings.editor.open().expect("Settings opens");
    assert!(ui.settings.editor.loaded(
        token,
        Basis {
            source: "composition-test".into(),
            values: UserPreferencesValues::default(),
            claimed: Default::default(),
        },
    ));
    assert_eq!(
        ui.view().settings_presentation(),
        Some(PresentationPhase::Pending)
    );
}

#[test]
fn settings_centers_on_the_host_independently_of_sidebar_visibility_and_width() {
    use crate::{client_view::ClientAction, tui_layout::SidebarWidth};

    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(120, 40), (120, 30), (100, 24), (80, 24)] {
            for sidebar_columns in [24, 30, 60] {
                for visible in [false, true] {
                    let (mut ui, _, _, _) = super::move_membership::ready_ui();
                    let mut shell = ClientView::new(width);
                    shell.machines.available = true;
                    shell.machines.observe_agent_sidebar(true);
                    let preferred = SidebarWidth::new(sidebar_columns).unwrap();
                    if preferred != shell.machines.sidebar_width() {
                        assert!(
                            shell
                                .machines
                                .set_sidebar_width(SidebarWidth::DEFAULT, preferred)
                        );
                    }
                    let initially_visible = !matches!(
                        shell.sidebar_surface(),
                        crate::client_view::SidebarSurface::Hidden
                    );
                    if initially_visible != visible {
                        let transition = shell.decide_action(ClientAction::ToggleBusinessSidebar);
                        shell.commit(
                            transition,
                            crate::client_view::ClientActionDisposition::KeepNavigator,
                        );
                    }
                    assert_eq!(
                        !matches!(
                            shell.sidebar_surface(),
                            crate::client_view::SidebarSurface::Hidden
                        ),
                        visible
                    );
                    open_loaded_settings(&mut ui);
                    let theme = Theme::orchbee_light();
                    let frame = render(&ui, &shell, width, height, locale, &theme);
                    assert!(
                        frame.has_hit(|target| matches!(target, HitTarget::SettingsRow { .. }))
                    );
                    let buffer = frame.terminal.backend().buffer();
                    // The modal backdrop also blocks the whole host. Measure the painted
                    // neutral overlay frame, not that intentionally larger input-capture surface.
                    let surface = (0..height)
                        .flat_map(|y| (0..width).map(move |x| (x, y)))
                        .filter(|&(x, y)| {
                            buffer[(x, y)].fg == theme.color(Role::FrameUnfocused)
                                && buffer[(x, y)].bg == theme.color(Role::SurfaceOverlay)
                                && buffer[(x, y)].symbol() != " "
                        })
                        .collect::<Vec<_>>();
                    let expected_width = width.min(92);
                    // The owning Settings contract prefers 92×30, clamped to the whole host; the
                    // status reserves no row.
                    let expected_height = height.min(30);
                    let left = (width - expected_width) / 2;
                    let top = (height - expected_height) / 2;
                    assert_eq!(surface.iter().map(|cell| cell.0).min(), Some(left));
                    assert_eq!(
                        surface.iter().map(|cell| cell.0).max(),
                        Some(left + expected_width - 1)
                    );
                    assert_eq!(surface.iter().map(|cell| cell.1).min(), Some(top));
                    assert_eq!(
                        surface.iter().map(|cell| cell.1).max(),
                        Some(top + expected_height - 1)
                    );
                    for x in left + expected_width - 6..left + expected_width - 3 {
                        assert!(matches!(
                            frame.hits.hit_target_at(x, top),
                            Some(HitTarget::SettingsControl {
                                control: crate::native_settings::Control::Close,
                                ..
                            })
                        ));
                    }
                }
            }
        }
    }
}

#[test]
fn settings_is_the_only_painted_owner_when_it_coexists_with_the_board() {
    let (mut ui, _, _, _) = super::move_membership::ready_ui();
    let shell = ClientView::new(WIDTH);
    let theme = Theme::rgb_test_dark();
    let episode = admit_board(&mut ui, &shell, Locale::EnUs, &theme);
    open_loaded_settings(&mut ui);

    let frame = render(&ui, &shell, WIDTH, FULL_HEIGHT, Locale::EnUs, &theme);
    assert!(frame.has_hit(|target| matches!(
        target,
        HitTarget::SettingsRow { .. } | HitTarget::SettingsControl { .. }
    )));
    assert!(!frame.has_hit(board_hit));
    let text = frame.text();
    assert!(text.contains(Locale::EnUs.settings_title()));
    assert!(!text.contains(Locale::EnUs.shell().board.title));

    ui.after_close_draw(frame.receipt);
    assert_eq!(
        ui.view().settings_presentation(),
        Some(PresentationPhase::Admitted)
    );
    let board = ui.board.input_context().expect("covered Board is retained");
    assert_eq!(board.episode(), episode);
    assert_eq!(board.presentation(), PresentationPhase::Admitted);
    assert!(
        !board.interactive(),
        "a missing Board receipt revokes only its current-frame proof"
    );
}

#[test]
fn panel_close_confirmation_outranks_and_is_painted_instead_of_the_board() {
    let (mut ui, _, _, _) = super::move_membership::ready_ui();
    let shell = ClientView::new(WIDTH);
    let theme = Theme::rgb_test_dark();
    let episode = admit_board(&mut ui, &shell, Locale::EnUs, &theme);
    assert_eq!(
        ui.request_panel_close_confirmation(),
        crate::client_view::ClientActionDisposition::DismissNavigator
    );

    let frame = render(&ui, &shell, WIDTH, FULL_HEIGHT, Locale::EnUs, &theme);
    assert!(frame.has_hit(|target| matches!(target, HitTarget::PanelClose { .. })));
    assert!(!frame.has_hit(board_hit));
    assert!(!frame.text().contains(Locale::EnUs.shell().board.title));

    ui.after_close_draw(frame.receipt);
    assert!(
        ui.view()
            .panel_close_confirmation()
            .expect("painted confirmation")
            .interactive()
    );
    let board = ui.board.input_context().expect("covered Board is retained");
    assert_eq!(board.episode(), episode);
    assert_eq!(board.presentation(), PresentationPhase::Admitted);
    assert!(!board.interactive());
}

#[test]
fn machine_capture_suppresses_native_proof_hits_and_remembered_sidebar_focus() {
    let (mut ui, _, _, _) = super::move_membership::ready_ui();
    let theme = Theme::rgb_test_dark();
    let ordinary_shell = ClientView::new(WIDTH);
    let episode = admit_board(&mut ui, &ordinary_shell, Locale::EnUs, &theme);

    let mut shell = ClientView::new(WIDTH);
    shell.machines.available = true;
    assert!(shell.machines.enter_sidebar_navigation());
    shell.machines.open = true;
    assert!(shell.machines.sidebar_navigation_active());
    let frame = render(&ui, &shell, WIDTH, FULL_HEIGHT, Locale::EnUs, &theme);
    assert!(!frame.has_hit(board_hit));
    assert!(!frame.text().contains(Locale::EnUs.shell().board.title));
    assert!(!frame.has_hit(|target| matches!(
        target,
        HitTarget::MachineGroupRow { .. }
            | HitTarget::MachineWorkspaceRow { .. }
            | HitTarget::MachineAgentRow { .. }
            | HitTarget::MachineWorkspaceSurface(_)
            | HitTarget::SidebarDisclosure(_)
            | HitTarget::SidebarMenu(_)
            | HitTarget::CreateMenu(_)
            | HitTarget::SidebarResize(_)
            | HitTarget::NewWorkspaceButton
            | HitTarget::SearchButton
            | HitTarget::SettingsButton
            | HitTarget::SidebarScrollbar { .. }
    )));
    let sidebar = crate::tui_layout::project_shell_areas(
        Rect::new(0, 0, WIDTH, FULL_HEIGHT),
        shell.sidebar_surface(),
    )
    .sidebar
    .expect("wide sidebar");
    let border = &frame.terminal.backend().buffer()[(
        sidebar.area.right() - 1,
        sidebar.area.y + sidebar.area.height / 2,
    )];
    assert_eq!(border.fg, theme.color(Role::FrameDivider));
    assert_ne!(border.fg, theme.color(Role::AccentFocus));

    ui.after_close_draw(frame.receipt);
    let board = ui.board.input_context().expect("covered Board is retained");
    assert_eq!(board.episode(), episode);
    assert_eq!(board.presentation(), PresentationPhase::Admitted);
    assert!(!board.interactive());
    assert!(shell.machines.sidebar_navigation_active());
}

#[test]
fn first_geometry_refusal_and_admitted_suspension_have_distinct_local_recovery() {
    let (mut ui, _, _, _) = super::move_membership::ready_ui();
    let shell = ClientView::new(WIDTH);
    let original_shell = shell.clone();
    let locale = Locale::EnUs;
    let theme = Theme::rgb_test_dark();
    let escape = format_binding(
        &Binding::new(KeyCode::Esc, Modifiers::NONE),
        LabelStyle::for_host(),
    );
    let app_exit = shell
        .bindings()
        .labels(PrefixDesc::Exit)
        .into_iter()
        .next()
        .expect("effective application-exit binding");

    ui.board.open();
    let first = render(&ui, &shell, WIDTH, REFUSED_HEIGHT, locale, &theme);
    assert!(!first.has_hit(board_hit));
    let text = first.text();
    assert!(text.contains(locale.presentation_notice(false)));
    assert!(!text.contains(&escape));
    assert!(!text.contains(&app_exit));
    ui.after_close_draw(first.receipt);
    assert!(
        !ui.board.is_open(),
        "the first unpaintable episode is refused"
    );

    let episode = admit_board(&mut ui, &shell, locale, &theme);
    let suspended = render(&ui, &shell, WIDTH, REFUSED_HEIGHT, locale, &theme);
    assert!(!suspended.has_hit(board_hit));
    let text = suspended.text();
    assert!(text.contains(locale.presentation_notice(true)));
    assert!(text.contains(&escape));
    assert!(text.contains(locale.recovery_verb(RecoveryVerb::Close)));
    assert!(!text.contains(&app_exit));
    ui.after_close_draw(suspended.receipt);

    let board = ui
        .board
        .input_context()
        .expect("admitted Board is retained");
    assert_eq!(board.episode(), episode);
    assert_eq!(board.presentation(), PresentationPhase::Suspended);
    assert!(!board.interactive());
    assert!(ui.capture_agent_board_key(episode, &KeyEvent::press(KeyCode::Esc, Modifiers::NONE),));
    assert!(
        !ui.board.is_open(),
        "the recovery key closes only the Board"
    );
    assert_eq!(shell, original_shell, "local recovery never emits app Exit");
}
