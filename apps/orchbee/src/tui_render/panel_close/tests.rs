use super::render_content;
use crate::{localization::Locale, tui_theme::Theme};
use ratatui::{
    Terminal,
    backend::TestBackend,
    text::{Line, Text},
};

fn draw(width: u16, height: u16, locale: Locale, lines: Vec<Line<'_>>) -> (bool, String) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let theme = Theme::rgb_test_dark();
    let mut complete = false;
    terminal
        .draw(|frame| {
            complete = render_content(
                frame,
                locale.panel_close_resize_hint(),
                Text::from(lines.clone()),
                frame.area(),
                &theme,
            )
            .map(|(_, complete)| complete)
            .unwrap_or(false);
        })
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    (complete, text)
}

fn confirmation_lines(locale: Locale, dissolves_tab: bool) -> Vec<Line<'static>> {
    vec![
        Line::from(locale.panel_close_title()),
        Line::from(""),
        Line::from("editor · Tab 0"),
        Line::from(""),
        Line::from(locale.panel_close_warning(dissolves_tab)),
        Line::from(""),
        Line::from(locale.panel_close_actions()),
    ]
}

#[test]
fn panel_close_requires_complete_visible_text_and_rejects_a_narrow_draw() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let lines = confirmation_lines(locale, false);
        let (complete, wide) = draw(120, 24, locale, lines.clone());
        assert!(complete);
        assert!(wide.replace(' ', "").contains("editor·Tab0"));
        let (complete, narrow) = draw(22, 8, locale, lines);
        assert!(!complete);
        assert!(!narrow.replace(' ', "").contains("editor·Tab0"));
    }
}

/// Borders, padding, and wrap breaks split the flat buffer text; comparing on
/// the alphanumeric run keeps the whole rendered sentence contiguous.
fn alnum(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).collect()
}

#[test]
fn panel_close_discloses_the_last_leaf_tab_dissolution_in_both_locales() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (complete, leaf) = draw(120, 24, locale, confirmation_lines(locale, false));
        assert!(complete);
        let (complete, last) = draw(120, 24, locale, confirmation_lines(locale, true));
        assert!(complete);
        let dissolving = alnum(locale.panel_close_warning(true));
        let shared = alnum(locale.panel_close_warning(false));
        assert!(alnum(&last).contains(&dissolving));
        assert!(!alnum(&leaf).contains(&dissolving));
        assert!(alnum(&leaf).contains(&shared));
    }
}

#[test]
fn panel_close_actions_name_only_the_exact_decision_keys() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let (complete, text) = draw(120, 24, locale, confirmation_lines(locale, false));
        assert!(complete);
        assert!(text.contains("Esc"));
        assert!(
            !text.contains("n /"),
            "the removed n-cancel hint must not linger: {text}"
        );
    }
}
