//! Composed terminal chrome preserves the final content row across prefix capture.
use super::*;
use crate::{
    client_view::{PrefixInput, SessionActionDisposition},
    localization::Locale,
    terminal_session::TerminalSessionView,
    tui_layout::project_shell_areas,
    tui_pointer::{HitMapBuilder, HitTarget},
    tui_theme::Theme,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[test]
fn prefix_overlays_and_restores_the_last_terminal_row_without_resizing() {
    for (width, height) in [(120, 40), (80, 24), (60, 12), (50, 10)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::one_light()] {
                let (mut ui, _, _, _) = ready_ui();
                let mut view = ClientView::new(width);
                view.machines.available = true;
                let area = Rect::new(0, 0, width, height);
                let areas = project_shell_areas(area, view.sidebar_surface());
                let pane = crate::tui_shell::active_pane_area(
                    &areas,
                    Some(TerminalSessionView::Native(ui.view())),
                )
                .unwrap();
                assert_eq!(
                    pane.bottom(),
                    height,
                    "ordinary content reaches the host bottom"
                );
                let key = install_projection(&mut ui, None, 0);
                let projection = projection_at(&mut ui, &key);
                let mut content = frame(&projection.value, 0, false, pane.height);
                content.geometry.columns = pane.width;
                content.geometry.rows = pane.height;
                content.cursor.row = pane.height - 1;
                projection.value.install_frame_for_test(content);
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                let paint = |terminal: &mut Terminal<TestBackend>, view: &ClientView| {
                    let mut hits = HitMapBuilder::new();
                    terminal
                        .draw(|frame| {
                            crate::tui_render::render_terminal_shell(
                                frame,
                                view,
                                locale,
                                Some(TerminalSessionView::Native(ui.view())),
                                &theme,
                                &mut hits,
                            );
                        })
                        .unwrap();
                    hits.finish()
                };
                let hits = paint(&mut terminal, &view);
                let ordinary = terminal.backend().buffer().clone();
                assert_eq!(ordinary[(pane.x, height - 1)].symbol(), "x");
                assert!(matches!(
                    hits.hit_target_at(pane.x, height - 1),
                    Some(HitTarget::Pane { .. })
                ));
                if areas.sidebar.is_some() {
                    assert!(matches!(
                        hits.hit_target_at(2, height - 1),
                        Some(HitTarget::SettingsButton)
                    ));
                }
                let transition = view.decide(PrefixInput::Key(KeyEvent::press(
                    KeyCode::Char('b'),
                    Modifiers::CTRL,
                )));
                view.commit(transition, SessionActionDisposition::KeepNavigator);
                let hits = paint(&mut terminal, &view);
                assert!(view.prefix_active());
                assert!(matches!(
                    hits.hit_target_at(pane.x, height - 1),
                    Some(HitTarget::OverlaySurface)
                ));
                assert!(
                    !terminal.backend().cursor_visible(),
                    "a covered terminal cursor stays hidden"
                );
                assert_ne!(
                    terminal.backend().buffer()[(pane.x, height - 1)].symbol(),
                    "x"
                );
                assert_eq!(project_shell_areas(area, view.sidebar_surface()), areas);
                let transition = view.decide(PrefixInput::Key(KeyEvent::press(
                    KeyCode::Esc,
                    Modifiers::NONE,
                )));
                view.commit(transition, SessionActionDisposition::KeepNavigator);
                paint(&mut terminal, &view);
                assert_eq!(
                    terminal.backend().buffer(),
                    &ordinary,
                    "the complete terminal and settings row restore"
                );
                assert_eq!(project_shell_areas(area, view.sidebar_surface()), areas);
            }
        }
    }
}
