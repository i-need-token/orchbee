//! Transient feedback for the pointer owner's already-validated sidebar destination.
use crate::{
    localization::Locale,
    terminal_text::{sanitize_terminal_text, truncate_display},
    tui_pointer::SidebarReorderDraw,
    tui_theme::{Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
};

pub(crate) fn render_sidebar_reorder_preview(
    frame: &mut Frame<'_>,
    preview: &SidebarReorderDraw,
    locale: Locale,
    theme: &Theme,
) {
    let viewport = frame.area();
    let emphasis = theme
        .style(Role::SurfaceSelection)
        .patch(theme.style(Role::TextPrimary));
    if let Some(area) = preview.grabbed {
        frame.buffer_mut().set_style(
            area.intersection(viewport),
            Style::new().add_modifier(Modifier::DIM),
        );
    }
    if let Some((area, _, _)) = &preview.destination {
        // An underline differentiates the insertion destination when colors collapse.
        frame.buffer_mut().set_style(
            area.intersection(viewport),
            Style::new().add_modifier(Modifier::UNDERLINED),
        );
    }
    let footer = preview.footer.intersection(frame.area());
    let footer_active = preview
        .destination
        .as_ref()
        .is_some_and(|(area, _, _)| *area == preview.footer);
    frame.render_widget(
        Paragraph::new(truncate_display(
            &format!("[ {} ]", locale.sidebar_drop_zone(preview.workspace)),
            usize::from(footer.width),
            theme.glyphs.ellipsis,
        ))
        .style(if footer_active {
            emphasis.add_modifier(Modifier::BOLD)
        } else {
            theme
                .style(Role::SurfaceChrome)
                .patch(theme.style(Role::TextSecondary))
        }),
        footer,
    );
    let area = frame.area();
    if area.height > 0 {
        let hint = locale.sidebar_reorder_hint(
            preview
                .destination
                .as_ref()
                .map(|(_, placement, label)| (*placement, label.as_str())),
        );
        frame.render_widget(
            Paragraph::new(truncate_display(
                &sanitize_terminal_text(&hint),
                usize::from(area.width),
                theme.glyphs.ellipsis,
            ))
            .style(emphasis),
            Rect::new(area.x, area.bottom() - 1, area.width, 1),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tui_pointer::SidebarDropPlacement, tui_theme::ColorLevel};
    use ratatui::{Terminal, backend::TestBackend, style::Color, widgets::Block};

    #[test]
    fn preview_preserves_identity_styles_clips_wide_labels_and_keeps_monochrome_cues() {
        for (width, height) in [(120, 40), (80, 24), (70, 18), (50, 10), (15, 4)] {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                for (theme, foreground) in [
                    (Theme::terminal_native(), Color::Red),
                    (Theme::one_light(), Color::Red),
                    (
                        Theme::one_light().quantize(ColorLevel::Monochrome),
                        Color::Reset,
                    ),
                ] {
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let row_width = width.min(30);
                    let draw = SidebarReorderDraw {
                        grabbed: Some(Rect::new(0, 0, row_width, 1)),
                        destination: Some((
                            Rect::new(0, 1, row_width, 1),
                            SidebarDropPlacement::Before,
                            "项目😀e\u{301}\u{1b}[31m a long identity".into(),
                        )),
                        footer: Rect::new(0, height - 2, row_width, 1),
                        workspace: true,
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
                    for x in 0..row_width {
                        assert_eq!(buffer[(x, 0)].fg, foreground);
                        assert_eq!(buffer[(x, 1)].fg, foreground);
                        assert_eq!(buffer[(x, 0)].bg, theme.color(Role::SurfaceChrome));
                        assert!(buffer[(x, 0)].modifier.contains(Modifier::DIM));
                        assert!(buffer[(x, 1)].modifier.contains(Modifier::UNDERLINED));
                    }
                    assert!(
                        buffer
                            .content
                            .iter()
                            .all(|cell| !cell.symbol().chars().any(char::is_control))
                    );
                    assert!((0..row_width).any(|x| buffer[(x, height - 2)].symbol() == "["));
                }
            }
        }
    }
}
