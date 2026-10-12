use super::*;
use crate::{
    localization::Locale,
    terminal_session::{NativeViewportSurface, terminal_viewport::ViewportWindow},
    tui_theme::{ColorLevel, Role},
};
use orchbee_terminal_protocol::{RowWindowDetail, TerminalRowWindowRoute};
use ratatui::{Terminal, backend::TestBackend};
use std::sync::Arc;

#[test]
fn shared_tail_paints_retained_rows_and_moves_the_cursor_with_its_live_row() {
    let mut live = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    ))
    .unwrap();
    live.cursor = orchbee_terminal_protocol::TerminalTextCursor {
        column: 1,
        row: 0,
        visible: true,
    };
    let mut rows = live.rows.clone();
    rows[0].cells[0].text = "H".into();
    let window = ViewportWindow {
        route: TerminalRowWindowRoute {
            attachment_id: live.route.attachment_id,
            host_epoch: live.route.host_epoch,
            sequence: live.route.sequence,
            revision: live.route.revision,
            generation: live.route.generation,
            total_rows: 4,
            start_row: 0,
            columns: 6,
            row_count: 2,
            detail: RowWindowDetail::Render,
        },
        foreground: live.foreground,
        background: live.background,
        graphics_present: false,
        rows,
    };
    let mut terminal = Terminal::new(TestBackend::new(10, 5)).unwrap();
    terminal
        .draw(|surface| {
            render_tail(
                surface,
                Rect::new(1, 1, 6, 3),
                &live,
                &window,
                2,
                true,
                &Theme::one_dark(),
            )
        })
        .unwrap();
    assert_eq!(terminal.backend().buffer()[(1, 1)].symbol(), "H");
    assert_eq!(
        terminal.get_cursor_position().unwrap(),
        ratatui::layout::Position::new(2, 3)
    );
}

#[test]
fn shared_terminal_colors_change_defaults_without_changing_chrome_or_explicit_program_colors() {
    use orchbee_workbench_protocol::{
        TerminalDisplayAppearance as Appearance, UserPreferencesValues,
    };
    let mut frame = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    ))
    .unwrap();
    frame.foreground = 0xabcdef;
    frame.background = 0x102030;
    frame.rows[0].cells[0].style.foreground = 0x0100_0000;
    frame.rows[0].cells[0].style.background = 0x0100_0000;
    frame.rows[0].cells[1].style.foreground = 0x112233;
    frame.rows[0].cells[1].style.background = 0x445566;
    for (appearance, foreground, background) in [
        (
            Appearance::Host,
            Color::Rgb(0xab, 0xcd, 0xef),
            Color::Rgb(0x10, 0x20, 0x30),
        ),
        (
            Appearance::Light,
            Color::Rgb(20, 20, 20),
            Color::Rgb(252, 252, 252),
        ),
        (
            Appearance::Dark,
            Color::Rgb(212, 212, 212),
            Color::Rgb(30, 30, 30),
        ),
        (Appearance::Custom, Color::Rgb(1, 2, 3), Color::Rgb(4, 5, 6)),
    ] {
        let values = UserPreferencesValues {
            terminal_display_theme: appearance,
            custom_foreground: "#010203".into(),
            custom_background: "#040506".into(),
            ..UserPreferencesValues::default()
        };
        let chrome = Theme::one_dark();
        let theme = chrome.with_terminal_preferences(&values);
        assert_eq!(
            theme.style(Role::SurfaceChrome),
            chrome.style(Role::SurfaceChrome)
        );
        let mut terminal = Terminal::new(TestBackend::new(10, 4)).unwrap();
        terminal
            .draw(|surface| super::render(surface, Rect::new(1, 1, 6, 2), &frame, false, &theme))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(
            (buffer[(1, 1)].fg, buffer[(1, 1)].bg),
            (foreground, background)
        );
        assert_eq!(
            (buffer[(2, 1)].fg, buffer[(2, 1)].bg),
            (Color::Rgb(0x11, 0x22, 0x33), Color::Rgb(0x44, 0x55, 0x66))
        );
    }
}

#[test]
fn default_markers_follow_the_theme_without_recoloring_explicit_cells()
-> Result<(), Box<dyn std::error::Error>> {
    let mut frame = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    ))?;
    // The Worker launch profile supplies these light RGB defaults even inside a dark host.
    frame.foreground = 0x14_14_14;
    frame.background = 0xfc_fc_fc;
    let default = 0x0100_0000;
    frame.rows = vec![TerminalTextRow {
        prompt_class: 0,
        index: 1,
        flags: 0,
        cells: [
            ("D", default, default, 0),
            ("E", frame.foreground, frame.background, 0),
            ("B", default, frame.background, 0),
            ("F", frame.foreground, default, 0),
            ("I", default, default, 16),
        ]
        .into_iter()
        .map(
            |(text, foreground, background, flags)| orchbee_terminal_protocol::TerminalTextCell {
                text: text.into(),
                width: 0,
                style: TerminalTextStyle {
                    foreground,
                    background,
                    underline_color: default,
                    flags,
                    underline: 0,
                },
            },
        )
        .collect(),
    }];
    let window = ViewportWindow {
        graphics_present: false,
        route: TerminalRowWindowRoute {
            attachment_id: frame.route.attachment_id,
            host_epoch: frame.route.host_epoch,
            sequence: frame.route.sequence,
            revision: frame.route.revision,
            generation: frame.route.generation,
            total_rows: 2,
            start_row: 0,
            columns: 20,
            row_count: 2,
            detail: RowWindowDetail::Render,
        },
        foreground: frame.foreground,
        background: frame.background,
        rows: frame.rows.clone(),
    };
    for theme in [
        Theme::terminal_native(),
        Theme::one_light(),
        Theme::one_dark(),
    ] {
        for level in [
            ColorLevel::Monochrome,
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::Truecolor,
        ] {
            let theme = theme.quantize(level);
            let foreground = theme.color(Role::TextPrimary);
            let background = theme.color(Role::SurfaceCanvas);
            for retained in [false, true] {
                let mut terminal = Terminal::new(TestBackend::new(22, 6))?;
                let area = Rect::new(1, 1, 20, 4);
                terminal.draw(|surface| {
                    if retained {
                        super::super::viewport::render(
                            surface,
                            area,
                            NativeViewportSurface::Window {
                                window: &window,
                                position: None,
                            },
                            None,
                            false,
                            Locale::EnUs,
                            &theme,
                        );
                    } else {
                        super::render(surface, area, &frame, false, &theme);
                    }
                })?;
                let buffer = terminal.backend().buffer();
                for (column, expected) in [
                    (1, (foreground, background)),
                    (2, (Color::Rgb(20, 20, 20), Color::Rgb(252, 252, 252))),
                    (3, (foreground, Color::Rgb(252, 252, 252))),
                    (4, (Color::Rgb(20, 20, 20), background)),
                    (5, (foreground, background)),
                    (6, (foreground, background)),
                ] {
                    let cell = &buffer[(column, 2)];
                    assert_eq!(
                        (cell.fg, cell.bg),
                        expected,
                        "{} {level:?} retained={retained}",
                        theme.name
                    );
                }
                assert!(buffer[(5, 2)].modifier.contains(Modifier::REVERSED));
                assert_eq!(buffer[(6, 2)].symbol(), " ");
                assert_eq!(
                    (buffer[(6, 3)].fg, buffer[(6, 3)].bg),
                    (foreground, background)
                );
                assert_eq!(buffer[(0, 0)], ratatui::buffer::Cell::default());
            }
            if theme.name == "terminal-native" || level == ColorLevel::Monochrome {
                // Reset delegates to either host polarity without guessing or probing it.
                assert_eq!((foreground, background), (Color::Reset, Color::Reset));
            }
        }
    }
    Ok(())
}

#[test]
fn renders_unicode_width_styles_and_cursor_without_emitting_control_text()
-> Result<(), Box<dyn std::error::Error>> {
    let mut frame = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../packages/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin"
    ))?;
    let mut terminal = Terminal::new(TestBackend::new(10, 4))?;
    let area = Rect::new(1, 1, 6, 2);
    let theme = Theme::terminal_native();
    terminal.draw(|surface| super::render(surface, area, &frame, true, &theme))?;
    assert_eq!(terminal.backend().buffer()[(1, 1)].symbol(), "A");
    assert_eq!(terminal.backend().buffer()[(2, 1)].symbol(), "界");
    assert_eq!(terminal.backend().buffer()[(4, 1)].symbol(), "e\u{301}");
    for forbidden in ["\x1b]52;c;clipboard\x07", "\u{85}", "\u{2028}", "\u{2029}"] {
        frame.rows[0].cells[0].text = Arc::from(forbidden);
        terminal.draw(|surface| super::render(surface, area, &frame, false, &theme))?;
        assert_eq!(terminal.backend().buffer()[(1, 1)].symbol(), "�");
    }
    assert!(
        !terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol().contains('\x1b'))
    );
    Ok(())
}
