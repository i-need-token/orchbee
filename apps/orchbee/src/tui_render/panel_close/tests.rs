use super::super::modal::ModalAction;
use super::{PreparedContent, confirmation_text, render_content, render_prepared};
use crate::{
    localization::Locale,
    tui_pointer::HitMapBuilder,
    tui_theme::Theme,
    workbench_client::{ModalPresentation, PanelCloseConfirmationToken},
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    text::{Line, Text},
};

fn modal_hints(_locale: Locale) -> [super::super::controls::KeyHint<'static>; 0] {
    []
}

fn modal_actions<'a>(locale: Locale, confirm: &'a str) -> [ModalAction<'a>; 2] {
    [
        ModalAction::new(
            locale.panel_close_cancel_label(),
            super::super::controls::ActionKind::Secondary,
            true,
        )
        .with_shortcut("Esc"),
        ModalAction::new(confirm, super::super::controls::ActionKind::Primary, true)
            .with_shortcut("Enter"),
    ]
}

fn draw(width: u16, height: u16, locale: Locale, lines: Vec<Line<'_>>) -> (bool, String) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let theme = Theme::rgb_test_dark();
    let mut complete = false;
    terminal
        .draw(|frame| {
            let hints = modal_hints(locale);
            complete = render_content(
                frame,
                frame.area(),
                Text::from(lines.clone()),
                &hints,
                modal_actions(locale, locale.panel_close_confirm_label()),
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
    confirmation_text(
        locale,
        2,
        1,
        "fixture",
        "fixture",
        dissolves_tab,
        &Theme::rgb_test_dark(),
    )
    .lines
}

#[test]
fn panel_close_requires_complete_visible_text_and_rejects_a_narrow_draw() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let lines = confirmation_lines(locale, false);
        for (width, height) in [(120, 40), (80, 24)] {
            let (complete, wide) = draw(width, height, locale, lines.clone());
            assert!(complete);
            let target = locale.panel_close_targets(2, 1, "fixture", "fixture");
            assert!(alnum(&wide).contains(&alnum(&target[0])));
            assert!(alnum(&wide).contains(&alnum(&target[1])));
        }
        let (complete, narrow) = draw(22, 8, locale, lines);
        assert!(!complete);
        assert!(!narrow.contains("fixture"));
    }
}

#[test]
fn panel_close_target_positions_survive_duplicate_and_control_labels() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let targets = locale.panel_close_targets(2, 3, "same\x1b[31m\nlabel", "same\x07label");
        assert!(targets[0].starts_with(&format!("{} 2 · ", locale.shell().panel_title)));
        assert!(targets[1].starts_with("Tab 3 · "));
        assert!(
            targets
                .iter()
                .all(|target| !target.chars().any(char::is_control))
        );
    }
}

#[test]
fn panel_close_long_duplicate_labels_fit_at_supported_normal_and_compact_sizes() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let panel_label = "duplicate terminal label with a stable suffix";
        let tab_label = "duplicate Tab label with a stable suffix";
        let text = confirmation_text(
            locale,
            12,
            34,
            panel_label,
            tab_label,
            false,
            &Theme::rgb_test_dark(),
        );
        let targets = locale.panel_close_targets(12, 34, panel_label, tab_label);
        for (width, height) in [(120, 40), (80, 24)] {
            let (complete, rendered) = draw(width, height, locale, text.lines.clone());
            assert!(complete, "{locale:?} at {width}x{height}");
            assert!(alnum(&rendered).contains(&alnum(&targets[0])));
            assert!(alnum(&rendered).contains(&alnum(&targets[1])));
        }
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
            let hints = modal_hints(Locale::EnUs);
            result = render_content(
                frame,
                content,
                Text::from(lines.clone()),
                &hints,
                modal_actions(Locale::EnUs, Locale::EnUs.panel_close_confirm_label()),
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
            let hints = modal_hints(Locale::EnUs);
            small_result = render_content(
                frame,
                tiny,
                Text::from(lines.clone()),
                &hints,
                modal_actions(Locale::EnUs, Locale::EnUs.panel_close_confirm_label()),
                &theme,
            );
        })
        .unwrap();
    assert!(
        !small_result.1,
        "a content area that cannot fit the text reports not complete"
    );
    assert_eq!(small_result.0, Rect::default());
}

fn assert_atomic_refusal(content: Rect, text: Text<'_>, confirm: &str) {
    let width = 40;
    let height = 12;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut result = None;
    terminal
        .draw(|frame| {
            for y in 0..height {
                for x in 0..width {
                    frame.buffer_mut()[(x, y)].set_symbol("q");
                }
            }
            let hints = modal_hints(Locale::EnUs);
            result = Some(render_prepared(
                frame,
                PanelCloseConfirmationToken::for_test(41),
                content,
                PreparedContent {
                    title: Locale::EnUs.panel_close_title(),
                    text: text.clone(),
                    hints: &hints,
                    actions: modal_actions(Locale::EnUs, confirm),
                },
                &Theme::rgb_test_dark(),
                &mut hits,
            ));
        })
        .unwrap();
    let result = result.unwrap();
    assert_eq!(result.area, Rect::default());
    assert!(matches!(result.prompt, ModalPresentation::NotPresented));
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .all(|cell| cell.symbol() == "q"),
        "a refused modal must not clear, paint, or partially copy any cell"
    );
    let hits = hits.finish();
    assert!((0..height).all(|y| (0..width).all(|x| hits.hit_target_at(x, y).is_none())));
}

#[test]
fn refused_modal_is_atomic_for_too_small_partial_oversize_and_clipped_actions() {
    let ordinary = Text::from(vec![
        Line::from("Close panel"),
        Line::from(""),
        Line::from("Warning"),
        Line::from(""),
        Line::from(""),
    ]);
    assert_atomic_refusal(Rect::new(0, 0, 10, 3), ordinary.clone(), "Close");
    assert_atomic_refusal(Rect::new(35, 2, 20, 8), ordinary, "Close");

    let oversized = Text::from(vec![
        Line::from("Close panel"),
        Line::from("界".repeat(2048)),
        Line::from(""),
        Line::from(""),
    ]);
    assert_atomic_refusal(Rect::new(0, 0, 40, 12), oversized, "Close");

    let long_confirm = "Confirm permanently".repeat(8);
    let clipped = Text::from(vec![
        Line::from("Close panel"),
        Line::from(""),
        Line::from(""),
    ]);
    assert_atomic_refusal(Rect::new(0, 0, 40, 12), clipped, long_confirm.as_str());
}
