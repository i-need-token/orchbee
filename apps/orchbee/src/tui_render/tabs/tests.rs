use std::convert::Infallible;

use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use super::{TabDisplay, compact_token, navigator_row, render_compact, render_navigator};

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
                &mut |_, _, _| {},
            ),
            Surface::Navigator => render_navigator(
                frame,
                area,
                tabs.len(),
                active_position,
                None,
                tab_at,
                &theme,
                &mut |_, _| {},
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
                    &mut |index, x, width| hits.push((index, x, width)),
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
            assert!(
                crate::terminal_text::display_width(&navigator_row(
                    &tab,
                    usize::from(width),
                    &crate::tui_theme::Theme::rgb_test_dark()
                )) <= usize::from(width)
            );
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
            assert!(text.contains(&format!("[*{count} {count}]")), "{text:?}");
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

    assert!(text.starts_with("[*15 15]"), "{text:?}");
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

    assert!(text.contains("[ 1 same] [ 2 same]"), "{text:?}");
    assert!(text.contains("[*3 same]"), "{text:?}");
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
    let (clipped, buffer) = rendered(5, 1, &tabs, Some(1), Surface::Compact);

    assert_eq!(prefix, "[*1 \n");
    assert_eq!(clipped, "[*1 ]\n");
    assert_eq!(buffer[(4, 0)].symbol(), "]");
}

#[test]
fn long_active_label_truncates_with_an_ellipsis_after_its_complete_prefix() {
    let label = format!("e\u{301}{}suffix", "界".repeat(256));
    let canonical = label.clone();
    let tabs = [TestTab {
        label: &label,
        active: true,
    }];

    let (text, _) = rendered(8, 1, &tabs, Some(1), Surface::Compact);

    assert_eq!(text, "[*1 e\u{301}…] \n");
    assert_eq!(label, canonical);
}

#[test]
fn long_chinese_label_truncates_on_display_width_with_an_ellipsis() {
    let tabs = [TestTab {
        label: "项目管理器工作区终端会话标签页",
        active: true,
    }];

    let (text, _) = rendered(12, 1, &tabs, Some(1), Surface::Compact);

    // "[*1 " is 4 cells; "…]" reserves 2, leaving a 6-cell label budget (3 CJK).
    // Wide graphemes occupy a continuation cell that reads back as a space.
    assert_eq!(text, "[*1 项 目 管 …]\n");
    assert!(!text.replace(' ', "").contains("理"));
}

#[test]
fn navigator_rows_truncate_wide_labels_on_display_width() {
    let tabs = [TestTab {
        label: "项目管理器工作区终端会话标签页",
        active: true,
    }];

    let (text, _) = rendered(14, 1, &tabs, Some(1), Surface::Navigator);

    assert_eq!(text, "[*1] 项 目 管 理 …\n");
}

#[test]
fn one_column_compact_and_navigator_rectangles_clip_safely() {
    let tabs = [TestTab {
        label: "界\x1b",
        active: true,
    }];

    let (compact, _) = rendered(1, 1, &tabs, Some(1), Surface::Compact);
    let (navigator, _) = rendered(1, 1, &tabs, Some(1), Surface::Navigator);

    assert_eq!(compact, "[\n");
    assert_eq!(navigator, "[\n");
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
            &mut |_, _, _| {},
        );
    }));
    let bounded = terminal.backend().buffer();
    assert_eq!(bounded[(2, 0)].symbol(), ".");
    assert_eq!(bounded[(3, 0)].symbol(), "[");
    assert_eq!(bounded[(4, 0)].symbol(), ".");

    let (_, styled) = rendered(16, 1, &tabs, Some(2), Surface::Compact);
    assert_eq!(
        styled[(0, 0)].fg,
        crate::tui_theme::Theme::rgb_test_dark().color(crate::tui_theme::Role::TextSecondary)
    );
    assert_eq!(
        styled[(7, 0)].fg,
        crate::tui_theme::Theme::rgb_test_dark().color(crate::tui_theme::Role::AccentFocus)
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
    let navigator = navigator_row(&tab, usize::MAX, &crate::tui_theme::Theme::rgb_test_dark());

    assert_eq!(compact, format!("[*{} label]", usize::MAX));
    assert_eq!(navigator, format!("[*{}] label", usize::MAX));
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
    assert!(text.contains("[ 254] 254"), "{text:?}");
    assert!(text.contains("[*256] 256"), "{text:?}");
    assert!(text.contains("256/256"), "{text:?}");
    assert!(!text.contains("[ 253]"), "{text:?}");
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

    assert!(text.contains("[ 1] duplicate"), "{text:?}");
    assert!(text.contains("[*2] duplicate"), "{text:?}");
    assert!(text.contains("[ 3]    "), "{text:?}");
}
