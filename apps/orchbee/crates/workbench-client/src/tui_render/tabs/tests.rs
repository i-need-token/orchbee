use std::convert::Infallible;

use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, style::Style};

use super::{TabDisplay, render_navigator};

#[derive(Clone, Copy)]
struct TestTab<'a> {
    label: &'a str,
    active: bool,
}

fn rendered(
    width: u16,
    height: u16,
    tabs: &[TestTab<'_>],
    active_position: Option<usize>,
) -> (String, Buffer) {
    rendered_with_focus(width, height, tabs, active_position, None)
}

fn rendered_with_focus(
    width: u16,
    height: u16,
    tabs: &[TestTab<'_>],
    active_position: Option<usize>,
    focused_position: Option<usize>,
) -> (String, Buffer) {
    let mut terminal = infallible(Terminal::new(TestBackend::new(width, height)));
    infallible(terminal.draw(|frame| {
        let tab_at = |index: usize| {
            tabs.get(index).map(|tab| TabDisplay {
                position: index.saturating_add(1),
                active: tab.active,
                label: tab.label.into(),
            })
        };
        let area = Rect::new(0, 0, width, height);
        let theme = crate::tui_theme::Theme::rgb_test_dark();
        render_navigator(
            frame,
            area,
            tabs.len(),
            active_position,
            focused_position,
            tab_at,
            &theme,
            &mut (),
            &mut |_, _, _| crate::tui_hit_map::ProjectedPointer::Rest,
            &mut |_, _, _| {},
        );
    }));
    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..height {
        for x in 0..width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    (text, buffer)
}

fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

#[test]
fn navigator_rows_truncate_wide_labels_on_display_width() {
    let tabs = [TestTab {
        label: "项目管理器工作区终端会话标签页",
        active: true,
    }];

    let (text, _) = rendered(14, 1, &tabs, Some(1));

    assert_eq!(text, "  * 1 项 目 管 … \n");
}

#[test]
fn one_column_navigator_rectangles_clip_safely() {
    let tabs = [TestTab {
        label: "界\x1b",
        active: true,
    }];

    let (navigator, _) = rendered(1, 1, &tabs, Some(1));

    assert_eq!(navigator, " \n");
}

#[test]
fn navigator_scrolls_to_keep_an_active_row_after_many_tabs_visible() {
    let labels = (1..=256)
        .map(|number| number.to_string())
        .collect::<Vec<_>>();
    let tabs = labels
        .iter()
        .enumerate()
        .map(|(index, label)| TestTab {
            label,
            active: index == 255,
        })
        .collect::<Vec<_>>();

    let (text, _) = rendered(20, 4, &tabs, Some(256));

    // The clipped window keeps its last row for the current/total position.
    assert!(text.contains("    254 254"), "{text:?}");
    assert!(text.contains("  * 256 256"), "{text:?}");
    assert!(text.contains("256/256"), "{text:?}");
    assert!(!text.contains("    253 "), "{text:?}");
}

#[test]
fn navigator_rows_use_exact_markers_and_preserve_duplicate_and_whitespace_labels() {
    let tabs = [
        TestTab {
            label: "duplicate",
            active: false,
        },
        TestTab {
            label: "duplicate",
            active: true,
        },
        TestTab {
            label: "   ",
            active: false,
        },
    ];

    let (text, _) = rendered(24, 3, &tabs, Some(2));

    assert!(text.contains("    1 duplicate"), "{text:?}");
    assert!(text.contains("  * 2 duplicate"), "{text:?}");
    assert!(text.contains("    3    "), "{text:?}");
}

#[test]
fn navigator_keeps_current_and_provisional_focus_as_independent_tracks() {
    let tabs = [
        TestTab {
            label: "current",
            active: true,
        },
        TestTab {
            label: "focused",
            active: false,
        },
    ];
    let theme = crate::tui_theme::Theme::rgb_test_dark();

    let (text, buffer) = rendered_with_focus(24, 2, &tabs, Some(1), Some(2));

    assert!(text.contains("  * 1 current"), "{text:?}");
    assert!(text.contains("›   2 focused"), "{text:?}");
    assert_eq!(buffer[(2, 0)].symbol(), theme.glyphs.active_marker);
    assert_eq!(buffer[(0, 1)].symbol(), theme.glyphs.navigation_marker);
    assert_eq!(buffer[(2, 1)].symbol(), " ");
    assert_eq!(
        buffer[(6, 0)].bg,
        theme.color(crate::tui_theme::Role::SurfaceCurrent)
    );
    assert_eq!(
        buffer[(6, 1)].bg,
        theme.color(crate::tui_theme::Role::SurfaceSelection)
    );
}

#[test]
fn navigator_pointer_surface_keeps_current_and_provisional_tracks_independent() {
    use crate::{
        tui_hit_map::ProjectedPointer,
        tui_theme::{Role, Theme},
    };

    let theme = Theme::rgb_test_dark();
    let mut terminal = infallible(Terminal::new(TestBackend::new(24, 2)));
    let mut rows = Vec::new();
    infallible(terminal.draw(|frame| {
        render_navigator(
            frame,
            frame.area(),
            2,
            Some(1),
            Some(2),
            |index| {
                Some(TabDisplay {
                    position: index + 1,
                    active: index == 0,
                    label: if index == 0 { "current" } else { "focused" }.into(),
                })
            },
            &theme,
            &mut (),
            &mut |_, index, _| {
                if index == 0 {
                    ProjectedPointer::Hovered
                } else {
                    ProjectedPointer::Pressed
                }
            },
            &mut |_, index, rect| rows.push((index, rect)),
        );
    }));

    assert_eq!(rows.len(), 2);
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(2, 0)].symbol(), theme.glyphs.active_marker);
    assert_eq!(buffer[(0, 1)].symbol(), theme.glyphs.navigation_marker);
    assert_eq!(buffer[(6, 0)].bg, theme.color(Role::SurfaceHover));
    assert_eq!(buffer[(6, 1)].bg, theme.color(Role::SurfacePressed));
}

#[test]
fn navigator_preflights_the_complete_visible_source_before_paint_and_hits() {
    let mut terminal = infallible(Terminal::new(TestBackend::new(24, 2)));
    let mut rows = Vec::new();

    infallible(terminal.draw(|frame| {
        for y in 0..2 {
            frame
                .buffer_mut()
                .set_string(0, y, ".".repeat(24), Style::new());
        }
        render_navigator(
            frame,
            frame.area(),
            2,
            Some(1),
            Some(1),
            |index| {
                (index == 0).then(|| TabDisplay {
                    position: 1,
                    active: true,
                    label: "only-row".into(),
                })
            },
            &crate::tui_theme::Theme::rgb_test_dark(),
            &mut (),
            &mut |_, _, _| crate::tui_hit_map::ProjectedPointer::Rest,
            &mut |_, index, rect| rows.push((index, rect)),
        );
    }));

    assert!(rows.is_empty());
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .all(|cell| cell.symbol() == ".")
    );
}
