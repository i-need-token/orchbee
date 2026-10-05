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
            for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
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
            for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
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
fn ready_status_has_the_reviewed_registry_hints_and_retains_pane_space() {
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
                Locale::EnUs,
                None,
                &Theme::terminal_native(),
            )
        })
        .unwrap();
    assert_eq!(
        visible_row(terminal.backend().buffer(), 80),
        format!("COPY · {}v · y · / · F1 · Esc exit", " ".repeat(48))
    );
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
