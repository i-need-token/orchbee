use std::convert::Infallible;

use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use super::{
    CompactInteractions, NavigatorRowLayout, TabDisplay, compact_token, navigator_row,
    render_compact, render_navigator,
};

#[derive(Clone, Copy)]
struct TestTab<'a> {
    label: &'a str,
    active: bool,
}

#[derive(Clone, Copy)]
enum Surface {
    Compact,
    Navigator,
}

fn rendered(
    width: u16,
    height: u16,
    tabs: &[TestTab<'_>],
    active_position: Option<usize>,
    surface: Surface,
) -> (String, Buffer) {
    rendered_with_focus(width, height, tabs, active_position, None, surface)
}

fn rendered_with_focus(
    width: u16,
    height: u16,
    tabs: &[TestTab<'_>],
    active_position: Option<usize>,
    focused_position: Option<usize>,
    surface: Surface,
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
        match surface {
            Surface::Compact => render_compact(
                frame,
                area,
                tabs.len(),
                active_position,
                tab_at,
                &theme,
                CompactInteractions::new(
                    &mut (),
                    &mut |_, _, _| crate::tui_pointer::ProjectedPointer::Rest,
                    &mut |_, _, _| {},
                ),
            ),
            Surface::Navigator => render_navigator(
                frame,
                area,
                tabs.len(),
                active_position,
                focused_position,
                tab_at,
                &theme,
                &mut (),
                &mut |_, _, _| crate::tui_pointer::ProjectedPointer::Rest,
                &mut |_, _, _| {},
            ),
        }
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
fn metadata_annotations_are_sanitized_clipped_and_share_drawn_hit_geometry() {
    use super::label_with_metadata;
    for pinned in ["Pinned", "固定"] {
        let decorated = [
            label_with_metadata("shell".into(), Some(pinned), None),
            label_with_metadata("中文 shell".into(), None, Some("work\u{001b}[2J\n组")),
        ];
        assert_eq!(decorated[0], format!("{pinned} · shell"));
        for width in [1, 9, 20, 80] {
            let mut terminal = infallible(Terminal::new(TestBackend::new(width, 1)));
            let mut hits = Vec::new();
            infallible(terminal.draw(|frame| {
                render_compact(
                    frame,
                    Rect::new(0, 0, width, 1),
                    2,
                    Some(2),
                    |index| {
                        Some(TabDisplay {
                            position: index + 1,
                            active: index == 1,
                            label: decorated[index].clone(),
                        })
                    },
                    &crate::tui_theme::Theme::rgb_test_dark(),
                    CompactInteractions::new(
                        &mut (),
                        &mut |_, _, _| crate::tui_pointer::ProjectedPointer::Rest,
                        &mut |_, index, rect| {
                            hits.push((index, usize::from(rect.x), usize::from(rect.width)))
                        },
                    ),
                )
            }));
            assert!(
                hits.iter().all(|(index, x, size)| *index < 2
                    && *size > 0
                    && x + size <= usize::from(width))
            );
            assert!(
                hits.windows(2)
                    .all(|pair| pair[0].1 + pair[0].2 < pair[1].1)
            );
            let buffer = terminal.backend().buffer();
            let mut text = String::new();
            let mut x = 0;
            while x < width {
                let symbol = buffer[(x, 0)].symbol();
                text.push_str(symbol);
                x += u16::try_from(crate::terminal_text::display_width(symbol).max(1))
                    .expect("a rendered cell fits the terminal width");
            }
            assert!(!text.contains('\u{001b}') && !text.contains('\n'));
            assert!(
                hits.iter().any(|(index, _, _)| *index == 1),
                "active Tab remains reachable"
            );
            if width == 80 {
                assert!(text.contains(pinned));
                assert!(text.contains(" / 中文 shell"));
            }
            let tab = TabDisplay {
                position: 2,
                active: true,
                label: decorated[1].clone(),
            };
            if let Some(layout) = NavigatorRowLayout::from_width(usize::from(width), 2) {
                let row = navigator_row(&tab, layout, &crate::tui_theme::Theme::rgb_test_dark());
                let text = row
                    .spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>();
                assert!(crate::terminal_text::display_width(&text) <= usize::from(width));
            }
        }
    }
}

#[test]
fn compact_strip_formats_boundaries_without_assuming_single_digit_positions() {
    for count in [0, 1, 9, 10, 256] {
        let labels = (1..=count)
            .map(|position| position.to_string())
            .collect::<Vec<_>>();
        let tabs = labels
            .iter()
            .enumerate()
            .map(|(index, label)| TestTab {
                label,
                active: index + 1 == count,
            })
            .collect::<Vec<_>>();
        let (text, _) = rendered(80, 1, &tabs, (count > 0).then_some(count), Surface::Compact);

        if count == 0 {
            assert_eq!(text, format!("{}\n", " ".repeat(80)));
        } else {
            assert!(text.contains(&format!(" *{count} {count} ")), "{text:?}");
        }
    }
}

#[test]
fn compact_window_starts_at_a_token_boundary_and_keeps_the_active_prefix_visible() {
    let labels = (1..=20)
        .map(|number| number.to_string())
        .collect::<Vec<_>>();
    let tabs = labels
        .iter()
        .enumerate()
        .map(|(index, label)| TestTab {
            label,
            active: index == 14,
        })
        .collect::<Vec<_>>();

    let (text, _) = rendered(8, 1, &tabs, Some(15), Surface::Compact);

    assert!(text.starts_with(" *15 15 "), "{text:?}");
    assert!(!text.contains("14"));
}

#[test]
fn compact_strip_keeps_duplicates_and_sanitizes_only_the_temporary_rendering() {
    let unsafe_label =
        String::from("same\0\x1b\r\n\u{7f}\u{80}\u{85}\u{9f}\u{2028}\u{2029}e\u{301}界");
    let canonical = unsafe_label.clone();
    let tabs = [
        TestTab {
            label: "same",
            active: false,
        },
        TestTab {
            label: "same",
            active: false,
        },
        TestTab {
            label: &unsafe_label,
            active: true,
        },
    ];

    let (text, _) = rendered(80, 1, &tabs, Some(3), Surface::Compact);

    assert!(text.contains("  1 same    2 same "), "{text:?}");
    assert!(text.contains(" *3 same "), "{text:?}");
    assert_eq!(unsafe_label, canonical);
    assert!(!text.trim_end_matches('\n').chars().any(char::is_control));
}

#[test]
fn compact_strip_clips_without_splitting_a_double_width_cell() {
    let tabs = [TestTab {
        label: "界suffix",
        active: true,
    }];

    let (prefix, _) = rendered(4, 1, &tabs, Some(1), Surface::Compact);
    let (clipped, buffer) = rendered(6, 1, &tabs, Some(1), Surface::Compact);

    assert_eq!(prefix, " *1 \n");
    assert_eq!(clipped, " *1 … \n");
    assert_eq!(buffer[(5, 0)].symbol(), " ");
}

#[test]
fn long_active_label_truncates_with_an_ellipsis_after_its_complete_prefix() {
    let label = format!("e\u{301}{}suffix", "界".repeat(256));
    let canonical = label.clone();
    let tabs = [TestTab {
        label: &label,
        active: true,
    }];

    let (text, _) = rendered(9, 1, &tabs, Some(1), Surface::Compact);

    assert_eq!(text, " *1 e\u{301}界 … \n");
    assert_eq!(label, canonical);
}

#[test]
fn long_chinese_label_truncates_on_display_width_with_an_ellipsis() {
    let tabs = [TestTab {
        label: "项目管理器工作区终端会话标签页",
        active: true,
    }];

    let (text, _) = rendered(12, 1, &tabs, Some(1), Surface::Compact);

    // The padded ordinal and suffix leave room for three CJK glyphs and an ellipsis.
    // Wide graphemes occupy a continuation cell that reads back as a space.
    assert_eq!(text, " *1 项 目 管 … \n");
    assert!(!text.replace(' ', "").contains("理"));
}

#[test]
fn navigator_rows_truncate_wide_labels_on_display_width() {
    let tabs = [TestTab {
        label: "项目管理器工作区终端会话标签页",
        active: true,
    }];

    let (text, _) = rendered(14, 1, &tabs, Some(1), Surface::Navigator);

    assert_eq!(text, "  * 1 项 目 管 … \n");
}

#[test]
fn one_column_compact_and_navigator_rectangles_clip_safely() {
    let tabs = [TestTab {
        label: "界\x1b",
        active: true,
    }];

    let (compact, _) = rendered(1, 1, &tabs, Some(1), Surface::Compact);
    let (navigator, _) = rendered(1, 1, &tabs, Some(1), Surface::Navigator);

    assert_eq!(compact, " \n");
    assert_eq!(navigator, " \n");
}

#[test]
fn rendering_is_bounded_by_its_rect_and_marks_the_active_token_style() {
    let mut terminal = infallible(Terminal::new(TestBackend::new(16, 2)));
    let tabs = [
        TestTab {
            label: "a",
            active: false,
        },
        TestTab {
            label: "b",
            active: true,
        },
    ];
    infallible(terminal.draw(|frame| {
        frame
            .buffer_mut()
            .set_string(0, 0, ".".repeat(16), Style::new());
        let tab_at = |index: usize| {
            tabs.get(index).map(|tab| TabDisplay {
                position: index + 1,
                active: tab.active,
                label: tab.label.into(),
            })
        };
        render_compact(
            frame,
            Rect::new(3, 0, 1, 1),
            2,
            Some(2),
            tab_at,
            &crate::tui_theme::Theme::rgb_test_dark(),
            CompactInteractions::new(
                &mut (),
                &mut |_, _, _| crate::tui_pointer::ProjectedPointer::Rest,
                &mut |_, _, _| {},
            ),
        );
    }));
    let bounded = terminal.backend().buffer();
    assert_eq!(bounded[(2, 0)].symbol(), ".");
    assert_eq!(bounded[(3, 0)].symbol(), " ");
    assert_eq!(bounded[(4, 0)].symbol(), ".");

    let (_, styled) = rendered(16, 1, &tabs, Some(2), Surface::Compact);
    assert_eq!(
        styled[(0, 0)].fg,
        crate::tui_theme::Theme::rgb_test_dark().color(crate::tui_theme::Role::TextSecondary)
    );
    let theme = crate::tui_theme::Theme::rgb_test_dark();
    let current = theme.current_context_style(theme.style(crate::tui_theme::Role::TextPrimary));
    assert_eq!(styled[(7, 0)].style().fg, current.fg);
    assert_eq!(
        styled[(7, 0)].bg,
        theme.color(crate::tui_theme::Role::SurfaceCurrent)
    );
    assert!(styled[(7, 0)].modifier.contains(Modifier::BOLD));
}

#[test]
fn token_formatting_is_usize_safe() {
    let tab = TabDisplay {
        position: usize::MAX,
        active: true,
        label: "label".into(),
    };

    let (compact, _) =
        compact_token(&tab, usize::MAX, &crate::tui_theme::Theme::rgb_test_dark()).unwrap();
    let navigator = navigator_row(
        &tab,
        NavigatorRowLayout::from_width(usize::MAX, usize::MAX).unwrap(),
        &crate::tui_theme::Theme::rgb_test_dark(),
    );
    let navigator = navigator
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();

    assert_eq!(compact, format!(" *{} label ", usize::MAX));
    assert_eq!(navigator, format!("  * {} label", usize::MAX));
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

    let (text, _) = rendered(20, 4, &tabs, Some(256), Surface::Navigator);

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

    let (text, _) = rendered(24, 3, &tabs, Some(2), Surface::Navigator);

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

    let (text, buffer) = rendered_with_focus(24, 2, &tabs, Some(1), Some(2), Surface::Navigator);

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
fn compact_pointer_surface_keeps_current_content_and_exact_token_geometry() {
    use crate::{
        tui_pointer::ProjectedPointer,
        tui_theme::{Role, Theme},
    };

    for (pointer, role) in [
        (ProjectedPointer::Hovered, Role::SurfaceHover),
        (ProjectedPointer::Pressed, Role::SurfacePressed),
    ] {
        let theme = Theme::rgb_test_dark();
        let mut terminal = infallible(Terminal::new(TestBackend::new(32, 1)));
        let mut projected = Vec::new();
        let mut recorded = Vec::new();
        infallible(terminal.draw(|frame| {
            render_compact(
                frame,
                frame.area(),
                2,
                Some(1),
                |index| {
                    Some(TabDisplay {
                        position: index + 1,
                        active: index == 0,
                        label: if index == 0 { "current" } else { "other" }.into(),
                    })
                },
                &theme,
                CompactInteractions::new(
                    &mut (),
                    &mut |_, index, rect| {
                        projected.push((index, rect));
                        if index == 0 {
                            pointer
                        } else {
                            ProjectedPointer::Rest
                        }
                    },
                    &mut |_, index, rect| recorded.push((index, rect)),
                ),
            );
        }));

        assert_eq!(recorded, projected);
        let current = recorded[0].1;
        let buffer = terminal.backend().buffer();
        assert_eq!(
            buffer[(current.x + 1, current.y)].symbol(),
            theme.glyphs.active_marker
        );
        assert_eq!(buffer[(current.x + 1, current.y)].bg, theme.color(role));
        assert!(
            buffer[(current.x + 1, current.y)]
                .modifier
                .contains(Modifier::BOLD)
        );
    }
}

#[test]
fn navigator_pointer_surface_keeps_current_and_provisional_tracks_independent() {
    use crate::{
        tui_pointer::ProjectedPointer,
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
            &mut |_, _, _| crate::tui_pointer::ProjectedPointer::Rest,
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
