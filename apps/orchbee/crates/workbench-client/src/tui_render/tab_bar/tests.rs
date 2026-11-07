use super::*;
use ratatui::{Terminal, backend::TestBackend};

/// Long names must truncate around the whole bar's center rather than move toward the smaller
/// button. Check the actual painted fill, including wide characters and offset bar coordinates.
#[test]
fn current_fill_stays_centered_and_clear_of_buttons_after_truncation() {
    let theme = Theme::orchbee_dark();
    for width in 20..60 {
        for height in [1, 3] {
            for locale in [Locale::EnUs, Locale::ZhCn] {
                for label in [
                    "PowerShell",
                    "A long terminal name that cannot fit in the mobile bar",
                    "中文长标签页名称：测试省略号与居中位置",
                ] {
                    let bar = Rect::new(3, 1, width, height);
                    let row = bar.y + u16::from(height == 3);
                    let mut terminal =
                        Terminal::new(TestBackend::new(width + 6, height + 2)).unwrap();
                    let view = ClientView::new(width);
                    terminal
                        .draw(|frame| {
                            render(
                                frame,
                                bar,
                                &Context {
                                    view: &view,
                                    locale,
                                    theme: &theme,
                                    current: Some(Current {
                                        label: label.into(),
                                        agent: None,
                                    }),
                                    count: 8,
                                    toggle: true,
                                },
                                &mut HitMapBuilder::new(),
                            );
                        })
                        .unwrap();
                    let buffer = terminal.backend().buffer();
                    let current: Vec<_> = (bar.x..bar.right())
                        .filter(|x| buffer[(*x, row)].bg == theme.color(Role::SurfaceCurrent))
                        .collect();
                    let start = *current.first().unwrap();
                    let end = current.last().unwrap() + 1;
                    let case = format!("{width}x{height} {locale:?} {label}");
                    assert!(
                        (start + end).abs_diff(bar.x + bar.right()) <= 1,
                        "{case}: fill {start}..{end} is off center"
                    );
                    let brackets: Vec<_> = (bar.x..bar.right())
                        .filter(|x| matches!(buffer[(*x, row)].symbol(), "[" | "]"))
                        .collect();
                    assert_eq!(brackets.len(), 4, "{case}");
                    assert!(start >= brackets[1] + 4, "{case}: leading gap");
                    assert!(end + 3 <= brackets[2], "{case}: trailing gap");
                }
            }
        }
    }
}

/// The strip and the current Tab share the same two-cell visual height over the canvas, even
/// when the strip's color happens to equal the sidebar's chrome color.
#[test]
fn touch_track_keeps_its_half_cell_fill_in_both_named_themes() {
    for theme in [Theme::orchbee_light(), Theme::orchbee_dark()] {
        let mut terminal = Terminal::new(TestBackend::new(46, 3)).unwrap();
        let view = ClientView::new(46);
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    &Context {
                        view: &view,
                        locale: Locale::ZhCn,
                        theme: &theme,
                        current: Some(Current {
                            label: "PowerShell".into(),
                            agent: None,
                        }),
                        count: 8,
                        toggle: true,
                    },
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        for (y, symbol) in [(0, "▄"), (2, "▀")] {
            assert_eq!(buffer[(0, y)].symbol(), symbol);
            assert_eq!(buffer[(0, y)].fg, theme.color(Role::SurfaceStrip));
            assert_eq!(buffer[(0, y)].bg, theme.color(Role::SurfaceCanvas));
        }
    }
}
