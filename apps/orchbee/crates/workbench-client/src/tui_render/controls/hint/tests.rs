use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Paragraph,
};

use crate::tui_theme::{GlyphSet, Role, Theme};

use super::{KeyHint, measure_key_hints, render_key_hints};

fn symbols(terminal: &Terminal<TestBackend>, width: u16) -> String {
    (0..width)
        .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
        .collect()
}

#[test]
fn renders_semantic_key_and_label_spans_and_drops_only_whole_suffix_items() {
    for theme in [
        Theme::orchbee_dark(),
        Theme::orchbee_dark().with_glyphs(GlyphSet::ASCII),
    ] {
        let navigation_key = if theme.glyphs == GlyphSet::ASCII {
            "Up/Down"
        } else {
            "↑/↓"
        };
        let items = [
            KeyHint::new("Esc", "close"),
            KeyHint::new("Enter", "open"),
            KeyHint::new(navigation_key, "navigate"),
        ];
        let visible_width = measure_key_hints(&items[..2], &theme).unwrap();
        assert!(measure_key_hints(&items, &theme).unwrap() > visible_width);
        let visible_width = u16::try_from(visible_width).unwrap();
        let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
        let mut painted = None;
        terminal
            .draw(|frame| {
                painted =
                    render_key_hints(frame, Rect::new(0, 0, visible_width, 1), &items, &theme);
            })
            .unwrap();

        assert_eq!(painted, Some(Rect::new(0, 0, visible_width, 1)));
        assert_eq!(
            symbols(&terminal, visible_width),
            format!("Esc close {} Enter open", theme.glyphs.middle_dot)
        );
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 0)].style().fg, theme.style(Role::HintKey).fg);
        assert_eq!(buffer[(4, 0)].style().fg, theme.style(Role::HintLabel).fg);
        assert!(buffer[(0, 0)].style().add_modifier.contains(Modifier::BOLD));
        assert!(!buffer[(4, 0)].style().add_modifier.contains(Modifier::BOLD));
        assert_eq!(
            terminal.backend().buffer()[(visible_width, 0)].symbol(),
            " "
        );
    }
}

#[test]
fn paints_the_owner_formatted_ascii_key_without_guessing_a_binding() {
    let theme = Theme::orchbee_dark().with_glyphs(GlyphSet::ASCII);
    let items = [KeyHint::new("Up/Down", "navigate")];
    let width = u16::try_from(measure_key_hints(&items, &theme).unwrap()).unwrap();
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            assert_eq!(render_key_hints(frame, area, &items, &theme), Some(area));
        })
        .unwrap();

    assert_eq!(symbols(&terminal, width), "Up/Down navigate");
}

#[test]
fn rejects_invalid_clipped_and_exit_too_wide_lines_before_painting() {
    let theme = Theme::orchbee_dark();
    let cases = [
        (
            Rect::new(0, 0, 9, 1),
            vec![
                KeyHint::new("Esc", "close"),
                KeyHint::new("\x1b[31m", "hidden invalid"),
            ],
        ),
        (Rect::new(0, 0, 3, 1), vec![KeyHint::new("Esc", "close")]),
        (Rect::new(8, 0, 4, 1), vec![KeyHint::new("Esc", "close")]),
    ];
    for (area, items) in cases {
        let mut terminal = Terminal::new(TestBackend::new(10, 1)).unwrap();
        let mut painted = Some(Rect::new(0, 0, 0, 0));
        let mut before = Style::new();
        terminal
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new("unchanged!")
                        .style(Style::new().fg(Color::Green).bg(Color::Magenta)),
                    frame.area(),
                );
                before = frame.buffer_mut()[(0, 0)].style();
                painted = render_key_hints(frame, area, &items, &theme);
            })
            .unwrap();
        assert_eq!(painted, None);
        assert_eq!(symbols(&terminal, 10), "unchanged!");
        assert_eq!(terminal.backend().buffer()[(0, 0)].style(), before);
    }
}
