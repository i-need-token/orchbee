use super::*;
use crate::terminal_copy_mode::{CopyModeCursor, CopyModeSearchView, CopyModeStatus, EpisodeId};
use ratatui::{
    Terminal,
    backend::{Backend, TestBackend},
    layout::Position,
};
use std::num::NonZeroU64;

#[test]
fn status_preserves_mode_discovery_and_search_caret_across_supported_geometry_and_themes() {
    for width in [40, 70, 80, 96, 120] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::rgb_test_dark(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
                for phase in [
                    CopyModeStatus::Entering,
                    CopyModeStatus::Ready,
                    CopyModeStatus::Selecting,
                    CopyModeStatus::Moving,
                    CopyModeStatus::Querying,
                    CopyModeStatus::Exporting,
                    CopyModeStatus::Clipboard,
                ] {
                    let mode = CopyModeView {
                        pointer: None,
                        status: phase,
                        cursor: None,
                        selecting: true,
                        search: None,
                    };
                    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
                    terminal
                        .draw(|frame| {
                            render_status(
                                frame,
                                frame.area(),
                                mode,
                                locale,
                                Some("failure"),
                                &theme,
                                true,
                            )
                        })
                        .unwrap();
                    let text = visible_row(terminal.backend().buffer(), width, 0);
                    assert!(
                        text.contains(if locale == Locale::EnUs {
                            "COPY"
                        } else {
                            "复制"
                        }),
                        "{width}: {text}"
                    );
                    assert!(text.contains("Esc"), "{width}: {text}");
                    assert_eq!(
                        terminal.backend().buffer()[(0, 0)].fg,
                        theme.style(Role::StatusInfo).fg.unwrap()
                    );
                }
                let draft = "中文e\u{301}🧪".repeat(32);
                let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
                terminal
                    .draw(|frame| {
                        render_status(
                            frame,
                            frame.area(),
                            CopyModeView {
                                pointer: None,
                                status: CopyModeStatus::Ready,
                                cursor: None,
                                selecting: false,
                                search: Some(CopyModeSearchView {
                                    direction: CopySearchDirection::Backward,
                                    draft: &draft,
                                    cursor: draft.len(),
                                }),
                            },
                            locale,
                            None,
                            &theme,
                            true,
                        );
                    })
                    .unwrap();
                let text = visible_row(terminal.backend().buffer(), width, 0);
                assert!(text.contains("?"));
                if width >= 80 {
                    assert!(text.contains("Ctrl+r"));
                    assert!(text.contains(
                        locale.copy_hint_label(crate::client_view::CopyHintLabel::Reverse)
                    ));
                }
                let caret = terminal.backend_mut().get_cursor_position().unwrap();
                assert!(caret.x < width);
                assert_eq!(caret.y, 0);
            }
        }
    }
}

/// A wide glyph's continuation cell is not a second printed space.
fn visible_row(buffer: &ratatui::buffer::Buffer, width: u16, row: u16) -> String {
    let mut column = 0;
    let mut text = String::new();
    while column < width {
        let symbol = buffer[(column, row)].symbol();
        text.push_str(symbol);
        column += u16::try_from(display_width(symbol).max(1)).unwrap();
    }
    text
}

fn visible_screen(buffer: &ratatui::buffer::Buffer, width: u16, height: u16) -> String {
    (0..height)
        .map(|row| visible_row(buffer, width, row))
        .collect::<Vec<_>>()
        .join("\n")
}

fn display_column_of(text: &str, needle: &str) -> u16 {
    let byte = text.find(needle).expect("visible text contains needle");
    u16::try_from(display_width(&text[..byte])).expect("rendered row width fits u16")
}

#[test]
fn a_rejected_search_paste_keeps_the_draft_caret_warning_and_exit_path() {
    for width in [40, 70, 80, 96, 120] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::rgb_test_dark(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
                let warning = locale.native_notice(
                    crate::workbench_client::WorkbenchClientNotice::CopyModeSearchLimit,
                );
                let warning_prefix = if locale == Locale::EnUs {
                    "Rejected"
                } else {
                    "拒绝搜索"
                };
                let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
                terminal
                    .draw(|frame| {
                        render_status(
                            frame,
                            frame.area(),
                            CopyModeView {
                                pointer: None,
                                status: CopyModeStatus::Ready,
                                cursor: None,
                                selecting: true,
                                search: Some(CopyModeSearchView {
                                    direction: CopySearchDirection::Forward,
                                    draft: "kept",
                                    cursor: 4,
                                }),
                            },
                            locale,
                            Some(&warning),
                            &theme,
                            true,
                        )
                    })
                    .unwrap();
                let text = visible_row(terminal.backend().buffer(), width, 0);
                assert!(text.contains("/kept"), "{width}: {text}");
                assert!(text.contains(warning_prefix), "{width}: {text}");
                assert!(text.contains("Esc"), "{width}: {text}");
                let marker = display_column_of(&text, theme.glyphs.status_bullet);
                let prose = display_column_of(&text, warning_prefix);
                assert_eq!(
                    terminal.backend().buffer()[(marker, 0)].fg,
                    theme.color(Role::StatusWarning)
                );
                assert_eq!(
                    terminal.backend().buffer()[(prose, 0)].fg,
                    theme.color(Role::TextPrimary)
                );
                let caret = terminal.backend_mut().get_cursor_position().unwrap();
                assert_eq!(
                    terminal.backend().buffer()[(caret.x - 1, caret.y)].symbol(),
                    "t"
                );
                assert!(caret.x < width);
            }
        }
    }
}

#[test]
fn footer_drops_only_complete_owner_ordered_key_hints() {
    let theme = Theme::terminal_native();
    for (width, locale, expected, absent) in [
        (16, Locale::EnUs, &["Esc Exit"][..], &["y Copy"][..]),
        (
            40,
            Locale::EnUs,
            &["Esc Exit", "y Copy", "v Select"][..],
            &["/ Search"][..],
        ),
        (
            120,
            Locale::EnUs,
            &["Esc Exit", "y Copy", "v Select", "/ Search", "F1 Help"][..],
            &[][..],
        ),
        (16, Locale::ZhCn, &["Esc 退出"][..], &["y 复制"][..]),
        (
            40,
            Locale::ZhCn,
            &["Esc 退出", "y 复制", "v 选择"][..],
            &["/ 搜索"][..],
        ),
        (
            120,
            Locale::ZhCn,
            &["Esc 退出", "y 复制", "v 选择", "/ 搜索", "F1 帮助"][..],
            &[][..],
        ),
    ] {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_status(
                    frame,
                    frame.area(),
                    CopyModeView {
                        pointer: None,
                        status: CopyModeStatus::Ready,
                        cursor: None,
                        selecting: false,
                        search: None,
                    },
                    locale,
                    None,
                    &theme,
                    true,
                )
            })
            .unwrap();
        let text = visible_row(terminal.backend().buffer(), width, 0);
        for item in expected {
            assert!(text.contains(item), "{width}: missing {item:?} in {text:?}");
        }
        for item in absent {
            assert!(
                !text.contains(item),
                "{width}: unexpected {item:?} in {text:?}"
            );
        }
        assert!(!text.ends_with('·'), "{width}: {text:?}");
    }

    let terminal = {
        let mut terminal = Terminal::new(TestBackend::new(40, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_status(
                    frame,
                    frame.area(),
                    CopyModeView {
                        pointer: None,
                        status: CopyModeStatus::Ready,
                        cursor: None,
                        selecting: false,
                        search: None,
                    },
                    Locale::EnUs,
                    None,
                    &theme,
                    true,
                )
            })
            .unwrap();
        terminal
    };
    let text = visible_row(terminal.backend().buffer(), 40, 0);
    let escape = u16::try_from(text.find("Esc").unwrap()).unwrap();
    assert_eq!(
        terminal.backend().buffer()[(escape, 0)].fg,
        theme.style(Role::HintKey).fg.unwrap()
    );
    assert_eq!(
        terminal.backend().buffer()[(escape + 4, 0)].fg,
        theme.style(Role::HintLabel).fg.unwrap()
    );
}

#[test]
fn status_reserves_complete_notice_content_before_atomic_hints() {
    let theme = Theme::terminal_native();
    for (width, locale, notice) in [
        (80, Locale::EnUs, "painted source drifted"),
        (80, Locale::ZhCn, "绘制来源已漂移"),
        (40, Locale::EnUs, "painted source drifted"),
        (40, Locale::ZhCn, "绘制来源已漂移"),
    ] {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_status(
                    frame,
                    frame.area(),
                    CopyModeView {
                        pointer: None,
                        status: CopyModeStatus::Ready,
                        cursor: None,
                        selecting: false,
                        search: None,
                    },
                    locale,
                    Some(notice),
                    &theme,
                    true,
                )
            })
            .unwrap();
        let text = visible_row(terminal.backend().buffer(), width, 0);
        assert!(text.contains(notice), "{width} {locale:?}: {text}");
        assert!(text.contains("Esc"), "{width} {locale:?}: {text}");
    }
}

#[test]
fn saturated_notice_and_search_draft_keep_a_blank_seam_before_atomic_hints() {
    let theme = Theme::terminal_native();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let exit = format!(
            "Esc {}",
            locale.copy_hint_label(crate::client_view::CopyHintLabel::Exit)
        );
        let notice = "a".repeat(240);
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_status(
                    frame,
                    frame.area(),
                    CopyModeView {
                        pointer: None,
                        status: CopyModeStatus::Ready,
                        cursor: None,
                        selecting: false,
                        search: None,
                    },
                    locale,
                    Some(&notice),
                    &theme,
                    true,
                )
            })
            .unwrap();
        let text = visible_row(terminal.backend().buffer(), 80, 0);
        let hint_start = display_column_of(&text, &exit);
        assert_eq!(
            terminal.backend().buffer()[(hint_start - 1, 0)].symbol(),
            " "
        );
        assert_eq!(
            terminal.backend().buffer()[(hint_start - 2, 0)].symbol(),
            "…"
        );

        let draft = format!("{}zz", "中文e\u{301}🧪".repeat(32));
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_status(
                    frame,
                    frame.area(),
                    CopyModeView {
                        pointer: None,
                        status: CopyModeStatus::Ready,
                        cursor: None,
                        selecting: false,
                        search: Some(CopyModeSearchView {
                            direction: CopySearchDirection::Forward,
                            draft: &draft,
                            cursor: draft.len(),
                        }),
                    },
                    locale,
                    None,
                    &theme,
                    true,
                )
            })
            .unwrap();
        let caret = terminal.backend_mut().get_cursor_position().unwrap();
        let text = visible_row(terminal.backend().buffer(), 80, 0);
        let hint_start = display_column_of(&text, &exit);
        assert_eq!(terminal.backend().buffer()[(caret.x - 1, 0)].symbol(), "z");
        assert!(caret.x < hint_start);
        assert_eq!(
            terminal.backend().buffer()[(hint_start - 1, 0)].symbol(),
            " "
        );
    }
}

fn toolbar_mode<'a>(
    status: CopyModeStatus,
    selecting: bool,
    search: Option<CopyModeSearchView<'a>>,
) -> CopyModeView<'a> {
    let mut mode = CopyModeView::entering(EpisodeId::new(NonZeroU64::new(1).unwrap()));
    mode.status = status;
    mode.selecting = selecting;
    mode.search = search;
    mode
}

fn map_contains_control(
    map: &crate::tui_pointer::FrameHitMap,
    width: u16,
    height: u16,
    token: crate::terminal_copy_mode::CopyModeToken,
    control: CopyControl,
) -> bool {
    (0..height).any(|row| {
        (0..width).any(|column| {
            matches!(
                map.hit_target_at(column, row),
                Some(HitTarget::CopyControl {
                    token: painted,
                    control: painted_control,
                }) if *painted == token && *painted_control == control
            )
        })
    })
}

#[test]
fn toolbar_uses_action_boolean_and_icon_grammars_with_typed_targets() {
    let width = 120;
    let height = 8;
    let theme = Theme::terminal_native();
    for (locale, selecting) in [(Locale::EnUs, false), (Locale::ZhCn, true)] {
        let mode = toolbar_mode(CopyModeStatus::Ready, selecting, None);
        let token = mode.pointer.unwrap();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                render_controls(
                    frame,
                    &ClientView::new(width),
                    mode,
                    locale,
                    &theme,
                    &mut hits,
                )
            })
            .unwrap();
        let text = visible_screen(terminal.backend().buffer(), width, height);
        assert!(text.contains(if locale == Locale::EnUs {
            "[ Copy ]"
        } else {
            "[ 复制 ]"
        }));
        assert!(text.contains(if selecting {
            "[x] 选择"
        } else {
            "[ ] Select"
        }));
        assert!(text.contains("[<]"));
        assert!(text.contains("[>]"));
        assert!(text.contains("[x]"));

        let map = hits.finish();
        for control in [
            CopyControl::Copy,
            CopyControl::ToggleSelection,
            CopyControl::Search,
            CopyControl::RepeatSearch { reverse: true },
            CopyControl::RepeatSearch { reverse: false },
            CopyControl::Help,
            CopyControl::Exit,
        ] {
            assert!(
                map_contains_control(&map, width, height, token, control),
                "missing target {control:?}: {text}"
            );
        }
    }
}

#[test]
fn search_toolbar_keeps_direction_as_static_state_and_reverse_as_a_labeled_action() {
    let width = 120;
    let height = 8;
    let theme = Theme::terminal_native();
    for direction in [CopySearchDirection::Forward, CopySearchDirection::Backward] {
        let mode = toolbar_mode(
            CopyModeStatus::Ready,
            false,
            Some(CopyModeSearchView {
                direction,
                draft: "needle",
                cursor: 6,
            }),
        );
        let token = mode.pointer.unwrap();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut hits = HitMapBuilder::new();
        terminal
            .draw(|frame| {
                render_controls(
                    frame,
                    &ClientView::new(width),
                    mode,
                    Locale::ZhCn,
                    &theme,
                    &mut hits,
                )
            })
            .unwrap();
        let text = visible_screen(terminal.backend().buffer(), width, height);
        assert!(text.contains("[ 查找 ]"));
        assert!(text.contains("[ 返回 ]"));
        assert!(text.contains("[ 反转 ]"));
        assert!(text.contains(match direction {
            CopySearchDirection::Forward => "> [ 反转 ]",
            CopySearchDirection::Backward => "< [ 反转 ]",
        }));
        let map = hits.finish();
        for control in [
            CopyControl::SubmitSearch,
            CopyControl::CancelSearch,
            CopyControl::ReverseSearch,
            CopyControl::Exit,
        ] {
            assert!(map_contains_control(&map, width, height, token, control));
        }
    }
}

#[test]
fn disabled_copy_controls_keep_their_shape_and_checked_value() {
    let width = 120;
    let height = 8;
    let theme = Theme::one_dark();
    let mode = toolbar_mode(CopyModeStatus::Moving, true, None);
    let token = mode.pointer.unwrap();
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| {
            render_controls(
                frame,
                &ClientView::new(width),
                mode,
                Locale::EnUs,
                &theme,
                &mut hits,
            )
        })
        .unwrap();
    let text = visible_screen(terminal.backend().buffer(), width, height);
    assert!(text.contains("[ Copy ]"));
    assert!(text.contains("[x] Select"));
    assert!(text.contains("[<]"));
    assert!(text.contains("[>]"));
    assert!(
        text.contains("[x]"),
        "Exit stays available while work is pending"
    );
    let map = hits.finish();
    for control in [
        CopyControl::Copy,
        CopyControl::ToggleSelection,
        CopyControl::Search,
        CopyControl::RepeatSearch { reverse: true },
        CopyControl::RepeatSearch { reverse: false },
        CopyControl::Help,
        CopyControl::Exit,
    ] {
        assert!(map_contains_control(&map, width, height, token, control));
    }
}

#[test]
fn copy_cursor_uses_the_whole_content_rect_and_search_owns_the_caret() {
    let theme = Theme::terminal_native();
    let area = Rect::new(3, 2, 20, 4);
    for row in [0, 3] {
        let mut terminal = Terminal::new(TestBackend::new(30, 10)).unwrap();
        terminal
            .draw(|frame| {
                frame.buffer_mut().set_string(
                    area.x,
                    area.y + row,
                    "kept text",
                    theme.style(Role::TextPrimary),
                );
                paint_cursor(
                    frame,
                    area,
                    CopyModeView {
                        pointer: None,
                        status: CopyModeStatus::Ready,
                        cursor: Some(CopyModeCursor { column: 0, row }),
                        selecting: true,
                        search: None,
                    },
                    &theme,
                );
            })
            .unwrap();
        assert_eq!(
            terminal.backend_mut().get_cursor_position().unwrap(),
            Position::new(3, 2 + row)
        );
        assert_eq!(terminal.backend().buffer()[(3, 2 + row)].symbol(), "k");
    }
    let (window, caret) = search_window("中e\u{301}X", "中e\u{301}".len(), 3);
    assert_eq!(window, "e\u{301}X");
    assert_eq!(caret, 1);
}
