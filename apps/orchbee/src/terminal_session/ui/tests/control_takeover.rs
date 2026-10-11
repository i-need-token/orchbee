use super::super::tests::move_membership::ready_ui;
use super::super::*;
use crate::{
    client_view::ClientView, localization::Locale, terminal_session::TerminalSessionView,
    tui_pointer::HitMapBuilder, tui_theme::Theme,
};
use ratatui::{Terminal, backend::TestBackend};
use std::sync::atomic::Ordering;

#[test]
fn read_only_input_notice_preserves_the_takeover_hint_in_the_status_line() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::one_light(),
            Theme::one_dark(),
        ] {
            let (ui, _, _, _) = ready_ui();
            let mut view = ClientView::new(120);
            view.report_terminal_input_unavailable();
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
            terminal
                .draw(|frame| {
                    crate::tui_render::render_terminal_shell(
                        frame,
                        &view,
                        locale,
                        Some(TerminalSessionView::Native(ui.view())),
                        &theme,
                        &mut HitMapBuilder::new(),
                    );
                })
                .unwrap();
            let status: String = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .skip(120 * 39)
                .map(|cell| cell.symbol())
                .collect();
            let readable: String = status.chars().filter(|c| c.is_alphanumeric()).collect();
            assert!(
                readable.contains(match locale {
                    Locale::EnUs => "viewonly",
                    Locale::ZhCn => "仅查看",
                }),
                "{locale:?}: {status}"
            );
            #[cfg(windows)]
            assert!(readable.contains("CtrlBt"), "{locale:?}: {status}");
            #[cfg(not(windows))]
            assert!(!readable.contains("CtrlBt"), "{locale:?}: {status}");
        }
    }
}

#[test]
fn control_takeover_status_renders_both_locales_themes_and_size_tiers() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::rgb_test_dark(),
            Theme::one_light(),
            Theme::one_dark(),
        ] {
            for (width, height) in [(80, 24), (120, 40), (60, 16), (24, 8), (8, 3)] {
                let (mut ui, mailbox, _, elapsed) = ready_ui();
                let cursors = mailbox.cursors();
                assert!(!ui.forward(b"typing does not take control"));
                assert_eq!(mailbox.cursors(), cursors);
                assert_eq!(
                    ui.apply_client_action(ClientAction::RequestControlTakeover),
                    SessionActionDisposition::DismissNavigator
                );
                assert!(
                    ui.view().takeover_status().is_none(),
                    "pending indication waits 200ms"
                );
                elapsed.store(200, Ordering::SeqCst);
                ui.tick();
                assert_eq!(
                    ui.view().takeover_status().unwrap().0,
                    crate::terminal_session::TakeoverStatus::Pending
                );
                assert!(!ui.forward(b"no queued replay"));
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal
                    .draw(|frame| {
                        crate::tui_render::render_terminal_shell(
                            frame,
                            &ClientView::new(width),
                            locale,
                            Some(TerminalSessionView::Native(ui.view())),
                            &theme,
                            &mut HitMapBuilder::new(),
                        );
                    })
                    .unwrap();
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect();
                if width == 120 {
                    let readable: String = text.chars().filter(|c| c.is_alphanumeric()).collect();
                    assert!(
                        readable.contains(match locale {
                            Locale::EnUs => "Takingcontrol",
                            Locale::ZhCn => "正在接管",
                        }),
                        "{locale:?}: {text}"
                    );
                }
                assert!(!text.contains('\u{1b}'));
                assert!(ui.state.as_ref().unwrap().invariant_holds());
            }
        }
    }
}
