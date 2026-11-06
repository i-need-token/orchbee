use super::*;
use crate::{
    tui_hit_map::sidebar_reorder::Placement as SidebarDropPlacement, tui_theme::ColorLevel,
};
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier, Style},
    widgets::Block,
};

fn row_text(buffer: &ratatui::buffer::Buffer, area: Rect) -> String {
    let mut text = String::new();
    let mut x = area.x;
    while x < area.right() {
        let symbol = buffer[(x, area.y)].symbol();
        text.push_str(symbol);
        x += u16::try_from(crate::terminal_text::display_width(symbol).max(1)).unwrap();
    }
    text
}

#[test]
fn drag_feedback_replaces_the_previous_notice_and_draws_the_footer_only_for_projects() {
    for (width, height) in [(120, 40), (80, 24), (70, 18), (50, 10), (15, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [Theme::terminal_native(), Theme::orchbee_light()] {
                for workspace in [false, true] {
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let footer = Rect::new(0, height - 2, width.min(30), 1);
                    let status = Rect::new(0, height - 1, width, 1);
                    let destination = (SidebarDropPlacement::Before, "Workspace 5".to_owned());
                    let hint = locale.sidebar_reorder_hint(
                        workspace,
                        Some((destination.0.into(), destination.1.as_str())),
                    );
                    let preview = SidebarReorderDraw {
                        destination: Some(destination),
                        footer,
                        workspace,
                    };
                    terminal
                        .draw(|frame| {
                            for y in [footer.y, status.y] {
                                frame.render_widget(
                                    Paragraph::new("x".repeat(usize::from(width))).style(
                                        Style::new().fg(Color::Red).add_modifier(Modifier::ITALIC),
                                    ),
                                    Rect::new(0, y, width, 1),
                                );
                            }
                            render_sidebar_reorder_preview(frame, &preview, locale, &theme);
                        })
                        .unwrap();
                    let buffer = terminal.backend().buffer();
                    assert_eq!(
                        row_text(buffer, status).trim_end(),
                        truncate_display(
                            &sanitize_terminal_text(&hint),
                            usize::from(width),
                            theme.glyphs.ellipsis
                        )
                    );
                    if workspace {
                        // A Workspace drag has no drop target below its list, so the footer keeps
                        // the rows the sidebar painted.
                        assert_eq!(
                            row_text(buffer, footer),
                            "x".repeat(usize::from(footer.width))
                        );
                    } else {
                        let zone = format!("[ {} ]", locale.sidebar_drop_zone());
                        assert_eq!(
                            row_text(buffer, footer).trim_end(),
                            truncate_display(
                                &zone,
                                usize::from(footer.width),
                                theme.glyphs.ellipsis
                            )
                        );
                    }
                    for x in status.x..status.right() {
                        assert!(!buffer[(x, status.y)].modifier.contains(Modifier::ITALIC));
                    }
                    for x in footer.right()..width {
                        assert_eq!(
                            buffer[(x, footer.y)].symbol(),
                            "x",
                            "the preview owns only its declared footer extent"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn preview_preserves_identity_styles_clips_wide_labels_and_keeps_monochrome_cues() {
    for (width, height) in [(120, 40), (80, 24), (70, 18), (50, 10), (15, 4)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for (theme, foreground) in [
                (Theme::terminal_native(), Color::Red),
                (Theme::orchbee_light(), Color::Red),
                (
                    Theme::orchbee_light().quantize(ColorLevel::Monochrome),
                    Color::Reset,
                ),
            ] {
                for workspace in [false, true] {
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let row_width = width.min(30);
                    let draw = SidebarReorderDraw {
                        destination: Some((
                            SidebarDropPlacement::Before,
                            "项目😀e\u{301}\u{1b}[31m a long identity".into(),
                        )),
                        footer: Rect::new(0, height - 2, row_width, 1),
                        workspace,
                    };
                    terminal
                        .draw(|frame| {
                            for y in [0, 1] {
                                frame.render_widget(
                                    Block::new().style(
                                        Style::new()
                                            .fg(foreground)
                                            .bg(theme.color(Role::SurfaceChrome)),
                                    ),
                                    Rect::new(0, y, row_width, 1),
                                );
                            }
                            render_sidebar_reorder_preview(frame, &draw, locale, &theme);
                        })
                        .unwrap();
                    let buffer = terminal.backend().buffer();
                    // The sidebar paints the move and the held entry in place; the preview restyles
                    // no list line.
                    for x in 0..row_width {
                        for y in [0, 1] {
                            assert_eq!(buffer[(x, y)].fg, foreground);
                            assert_eq!(buffer[(x, y)].bg, theme.color(Role::SurfaceChrome));
                            assert!(buffer[(x, y)].modifier.is_empty());
                        }
                    }
                    assert!(
                        buffer
                            .content
                            .iter()
                            .all(|cell| !cell.symbol().chars().any(char::is_control))
                    );
                    // The bracketed drop target is the non-color cue of a Project drag only.
                    if !workspace {
                        assert!((0..row_width).any(|x| buffer[(x, height - 2)].symbol() == "["));
                    }
                }
            }
        }
    }
}
