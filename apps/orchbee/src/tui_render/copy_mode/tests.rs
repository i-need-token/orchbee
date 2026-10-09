use super::*;
use crate::terminal_copy_mode::{CopyModeCursor, CopyModeSearchView, CopyModeStatus};
use ratatui::{
    Terminal,
    backend::{Backend, TestBackend},
    layout::Position,
};

#[test]
fn status_preserves_mode_help_and_search_caret_across_supported_geometry_and_themes() {
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
                            )
                        })
                        .unwrap();
                    let text = visible_row(terminal.backend().buffer(), width);
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
                        );
                    })
                    .unwrap();
                let caret = terminal.backend_mut().get_cursor_position().unwrap();
                assert!(caret.x < width);
                assert_eq!(caret.y, 0);
            }
        }
    }
}

/// A wide glyph's continuation cell is not a second printed space.
fn visible_row(buffer: &ratatui::buffer::Buffer, width: u16) -> String {
    let mut column = 0;
    let mut text = String::new();
    while column < width {
        let symbol = buffer[(column, 0)].symbol();
        text.push_str(symbol);
        column += u16::try_from(display_width(symbol).max(1)).unwrap();
    }
    text
}

#[test]
fn a_rejected_search_paste_keeps_the_draft_caret_and_visible_warning() {
    for width in [40, 70, 80, 96, 120] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::terminal_native(),
                Theme::rgb_test_dark(),
                Theme::one_light(),
                Theme::one_dark(),
            ] {
                let warning = locale.native_notice(
                    crate::terminal_session::NativeSessionNotice::CopyModeSearchLimit,
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
                        )
                    })
                    .unwrap();
                let text = visible_row(terminal.backend().buffer(), width);
                assert!(text.contains("/kept"), "{width}: {text}");
                assert!(text.contains(warning_prefix), "{width}: {text}");
                assert!(text.contains("Esc"), "{width}: {text}");
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
fn ready_status_chooses_the_widest_hint_set_that_fits_the_remaining_width() {
    let cases: &[(u16, Locale, &str)] = &[
        (
            80,
            Locale::EnUs,
            "arrows · v select · y copy · / search · F1 · Esc exit",
        ),
        (
            60,
            Locale::EnUs,
            "arrows · v select · y copy · / search · F1 · Esc exit",
        ),
        (59, Locale::EnUs, "v · y · / · F1 · Esc exit"),
        (32, Locale::EnUs, "v · y · / · F1 · Esc exit"),
        (31, Locale::EnUs, "Esc exit"),
        (
            80,
            Locale::ZhCn,
            "方向键 · v 选择 · y 复制 · / 搜索 · F1 · Esc 退出",
        ),
        (
            56,
            Locale::ZhCn,
            "方向键 · v 选择 · y 复制 · / 搜索 · F1 · Esc 退出",
        ),
        (55, Locale::ZhCn, "v · y · / · F1 · Esc 退出"),
        (32, Locale::ZhCn, "v · y · / · F1 · Esc 退出"),
        (31, Locale::ZhCn, "Esc 退出"),
    ];
    for &(width, locale, hint) in cases {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_status(
                    frame,
                    frame.area(),
                    CopyModeView {
                        status: CopyModeStatus::Ready,
                        cursor: None,
                        selecting: false,
                        search: None,
                    },
                    locale,
                    None,
                    &Theme::terminal_native(),
                )
            })
            .unwrap();
        let label = if locale == Locale::EnUs {
            "COPY"
        } else {
            "复制"
        };
        assert_eq!(
            visible_row(terminal.backend().buffer(), width),
            format!(
                "{label} · {}{hint}",
                " ".repeat(usize::from(width) - display_width(label) - 3 - display_width(hint))
            ),
            "{width}"
        );
    }
}

#[test]
fn ready_status_reserves_the_notice_before_choosing_a_hint_set() {
    for (width, locale, notice, hint) in [
        (
            80_u16,
            Locale::EnUs,
            "painted source drifted",
            "v · y · / · F1 · Esc exit",
        ),
        (
            80,
            Locale::ZhCn,
            "绘制来源已漂移",
            "方向键 · v 选择 · y 复制 · / 搜索 · F1 · Esc 退出",
        ),
        (
            80,
            Locale::ZhCn,
            "绘制来源内容已经漂移不可用",
            "v · y · / · F1 · Esc 退出",
        ),
        (40, Locale::EnUs, "painted source drifted", "Esc exit"),
        (40, Locale::ZhCn, "绘制来源已漂移", "Esc 退出"),
    ] {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_status(
                    frame,
                    frame.area(),
                    CopyModeView {
                        status: CopyModeStatus::Ready,
                        cursor: None,
                        selecting: false,
                        search: None,
                    },
                    locale,
                    Some(notice),
                    &Theme::terminal_native(),
                )
            })
            .unwrap();
        let text = visible_row(terminal.backend().buffer(), width);
        assert!(text.contains(notice), "{width} {notice}: {text}");
        assert!(text.contains(hint), "{width} {notice}: {text}");
    }
}

#[test]
fn saturated_status_content_keeps_one_blank_seam_before_the_hint() {
    let theme = Theme::terminal_native();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        // A saturating notice truncates with '…' one cell ahead of the exit hint.
        let notice = "a".repeat(240);
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_status(
                    frame,
                    frame.area(),
                    CopyModeView {
                        status: CopyModeStatus::Ready,
                        cursor: None,
                        selecting: false,
                        search: None,
                    },
                    locale,
                    Some(&notice),
                    &theme,
                )
            })
            .unwrap();
        let text = visible_row(terminal.backend().buffer(), 80);
        assert!(
            text.ends_with(&format!("… {}", locale.copy_mode_exit_hint())),
            "{locale:?}: {text}"
        );
        // A fully scrolled search draft pins the caret at the window's last
        // cell; a blank seam still precedes the hint.
        let draft = format!("{}zz", "中文e\u{301}🧪".repeat(32));
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_status(
                    frame,
                    frame.area(),
                    CopyModeView {
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
                )
            })
            .unwrap();
        let caret = terminal.backend_mut().get_cursor_position().unwrap();
        let buffer = terminal.backend().buffer();
        let hint_start =
            80_u16 - u16::try_from(display_width(locale.copy_mode_hint(true, true))).unwrap();
        assert_eq!(caret.x, hint_start - 2, "{locale:?} caret at draft edge");
        assert_eq!(
            buffer[(caret.x - 1, 0)].symbol(),
            "z",
            "{locale:?} draft tail"
        );
        assert_eq!(
            buffer[(hint_start - 1, 0)].symbol(),
            " ",
            "{locale:?} seam cell"
        );
        // A saturating warning hint keeps the same seam against the draft.
        let warning_notice = "b".repeat(240);
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal
            .draw(|frame| {
                render_status(
                    frame,
                    frame.area(),
                    CopyModeView {
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
                    Some(&warning_notice),
                    &theme,
                )
            })
            .unwrap();
        let caret = terminal.backend_mut().get_cursor_position().unwrap();
        let buffer = terminal.backend().buffer();
        let warning_fg = theme.style(Role::StatusWarning).fg.unwrap();
        let warning_start = (0..80_u16)
            .find(|x| buffer[(*x, 0)].fg == warning_fg)
            .expect("warning hint is drawn");
        assert_eq!(
            buffer[(caret.x - 1, 0)].symbol(),
            "z",
            "{locale:?} draft tail"
        );
        assert!(caret.x < warning_start, "{locale:?} caret before hint");
        assert_eq!(
            buffer[(warning_start - 1, 0)].symbol(),
            " ",
            "{locale:?} seam cell"
        );
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
