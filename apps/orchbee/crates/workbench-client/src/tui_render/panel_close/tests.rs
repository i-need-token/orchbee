use super::{PanelCloseCopy, PanelModalDraw, render_prepared};
use crate::localization::{CloseImpact, Locale};
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_presentation::ModalPresentation;
use crate::tui_theme::Theme;
use crate::workbench_state::PanelCloseConfirmationToken;
use ratatui::{Terminal, backend::TestBackend};

const TOKEN: PanelCloseConfirmationToken = PanelCloseConfirmationToken::for_test(3);

fn copy(locale: Locale) -> PanelCloseCopy {
    PanelCloseCopy {
        target: locale.panel_close_target(2, "git"),
        tab: locale.panel_close_tab("build"),
        dissolves_tab: true,
        consequence: locale.close_warning(CloseImpact {
            running: 1,
            ended: 0,
        }),
    }
}

/// Draws into a fresh window; returns the draw, the dialog text between its side borders without
/// spaces (so a wrapped sentence reads as one), and the action cells.
fn render(width: u16, height: u16, locale: Locale) -> (PanelModalDraw, String, Vec<(u16, bool)>) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut result = None;
    terminal
        .draw(|frame| {
            result = Some(render_prepared(
                frame,
                locale,
                TOKEN,
                &copy(locale),
                &Theme::rgb_test_dark(),
                &mut hits,
            ));
        })
        .unwrap();
    let draw = result.unwrap();
    let map = hits.finish();
    let footer = draw.area.bottom().saturating_sub(2);
    let actions = (0..width)
        .filter_map(|x| match map.hit_target_at(x, footer) {
            Some(HitTarget::PanelClose { token, confirm }) if *token == TOKEN => {
                Some((x, *confirm))
            }
            _ => None,
        })
        .collect();
    let buffer = terminal.backend().buffer();
    let inner = draw.area.inner(ratatui::layout::Margin::new(1, 0));
    let text = inner
        .rows()
        .flat_map(|row| row.columns())
        .map(|cell| buffer[(cell.x, cell.y)].symbol())
        .collect::<String>()
        .replace(' ', "");
    (draw, text, actions)
}

#[test]
fn panel_close_confirmation_is_centered_and_localized_with_the_primary_last() {
    for (locale, title, cancel, primary) in [
        (Locale::EnUs, "Close Panel?", "Cancel", "Close"),
        (Locale::ZhCn, "关闭面板？", "取消", "关闭"),
    ] {
        let (draw, text, actions) = render(100, 30, locale);
        assert!(matches!(draw.prompt, ModalPresentation::Presented(token) if token == TOKEN));
        assert_eq!(draw.area.x, (100 - draw.area.width) / 2);
        assert_eq!(draw.area.y, (30 - draw.area.height) / 2);
        let copy = copy(locale);
        for expected in [
            title,
            cancel,
            primary,
            &copy.target,
            &copy.tab,
            &copy.consequence,
            locale.panel_close_last_panel(),
        ] {
            assert!(
                text.contains(&expected.replace(' ', "")),
                "{locale:?} {expected}"
            );
        }
        // Cancel first, the destructive primary right-aligned after it.
        let first_primary = actions.iter().position(|(_, confirm)| *confirm).unwrap();
        assert!(first_primary > 0 && actions[first_primary..].iter().all(|(_, confirm)| *confirm));
    }
}

#[test]
fn panel_close_confirmation_degrades_in_small_windows_and_is_refused_below_its_minimum() {
    let (draw, text, actions) = render(48, 7, Locale::EnUs);
    assert!(matches!(draw.prompt, ModalPresentation::Presented(_)));
    assert_eq!(draw.area.height, 7);
    // The target and the complete action row stay; the context line goes.
    assert!(text.contains("Terminal2·git"));
    assert!(!text.contains("Tab:build"));
    assert!(actions.iter().any(|(_, confirm)| *confirm));

    let (draw, text, actions) = render(20, 5, Locale::EnUs);
    assert!(matches!(draw.prompt, ModalPresentation::NotPresented));
    assert!(draw.area.is_empty() && actions.is_empty() && text.is_empty());
}
