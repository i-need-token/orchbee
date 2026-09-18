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
                frame.area(),
                locale.panel_close_resize_hint(),
                Text::from(lines.clone()),
                &theme,
            )
            .1;
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

/// The modal must live inside the offered content area at a finite preferred
/// size — never spread across the whole frame — and refuse when the content
/// area cannot present the complete text.
#[test]
fn modal_stays_inside_content_area_at_finite_size_and_refuses_too_small() {
    let lines = confirmation_lines(Locale::EnUs, false);
    let theme = Theme::rgb_test_dark();
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    let content = ratatui::layout::Rect::new(20, 5, 80, 20);
    let mut result = (ratatui::layout::Rect::default(), false);
    terminal
        .draw(|frame| {
            result = render_content(
                frame,
                content,
                Locale::EnUs.panel_close_resize_hint(),
                Text::from(lines.clone()),
                &theme,
            );
        })
        .unwrap();
    let (area, complete) = result;
    assert!(complete, "80x20 content area presents the modal");
    assert!(area.x >= content.x && area.y >= content.y);
    assert!(area.x + area.width <= content.x + content.width);
    assert!(area.y + area.height <= content.y + content.height);
    assert!(
        area.width < content.width && area.height < content.height,
        "preferred size is finite, not the whole content area: {area:?}"
    );

    let mut small = Terminal::new(TestBackend::new(120, 30)).unwrap();
    let tiny = ratatui::layout::Rect::new(40, 10, 24, 6);
    let mut small_result = (ratatui::layout::Rect::default(), true);
    small
        .draw(|frame| {
            small_result = render_content(
                frame,
                tiny,
                Locale::EnUs.panel_close_resize_hint(),
                Text::from(lines.clone()),
                &theme,
            );
        })
        .unwrap();
    assert!(
        !small_result.1,
        "a content area that cannot fit the text reports not complete"
    );
}
