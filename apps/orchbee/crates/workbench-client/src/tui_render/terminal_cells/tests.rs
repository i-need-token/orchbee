use super::*;
use crate::tui_theme::ThemeTestAccess as _;
use crate::{
    localization::Locale,
    terminal_viewport::ViewportWindow,
    tui_theme::{ColorLevel, Role},
    workbench_client::ViewportSurface,
};
use orchbee_terminal_protocol::{RowWindowDetail, TerminalRowWindowRoute};
use ratatui::{Terminal, backend::TestBackend};
use std::sync::Arc;

#[test]
fn one_retained_palette_cell_recolors_per_client_and_terminal_colors_emit_indexes() {
    let mut source = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .unwrap();
    source.rows.truncate(1);
    source.rows[0].cells.truncate(1);
    let cell = &mut source.rows[0].cells[0];
    cell.text = "A".into();
    cell.width = 0;
    cell.style.flags = 1 | 4 | 16;
    cell.style.foreground = orchbee_terminal_protocol::COLOR_PALETTE | 1;
    cell.style.background = orchbee_terminal_protocol::COLOR_PALETTE | 15;
    let palettes = [
        orchbee_color_themes::select(orchbee_color_themes::Polarity::Dark, "gruvbox-dark"),
        orchbee_color_themes::select(orchbee_color_themes::Polarity::Light, "gruvbox-light"),
    ];
    let mut painted = Vec::new();
    for palette in palettes {
        let mut terminal = Terminal::new(TestBackend::new(2, 1)).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), &source, false, &Theme::named(palette)))
            .unwrap();
        let rendered = &terminal.backend().buffer()[(0, 0)];
        let rgb = |index: usize| {
            let color = palette.ansi()[index].value();
            Color::Rgb(color.red, color.green, color.blue)
        };
        assert_eq!((rendered.fg, rendered.bg), (rgb(1), rgb(15)));
        assert!(
            rendered
                .modifier
                .contains(Modifier::BOLD | Modifier::DIM | Modifier::REVERSED)
        );
        painted.push(rendered.fg);
    }
    assert_ne!(painted[0], painted[1]);
    for level in [ColorLevel::Truecolor, ColorLevel::Ansi256] {
        let mut terminal = Terminal::new(TestBackend::new(2, 1)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    &source,
                    false,
                    &Theme::terminal_native().quantize(level),
                )
            })
            .unwrap();
        let rendered = &terminal.backend().buffer()[(0, 0)];
        assert_eq!(
            (rendered.fg, rendered.bg),
            (Color::Indexed(1), Color::Indexed(15))
        );
    }
    let reduced = Theme::terminal_native().quantize(ColorLevel::Ansi16);
    assert!(!matches!(
        reduced.terminal_cell_color(orchbee_terminal_protocol::COLOR_PALETTE | 1, Color::Reset),
        Color::Rgb(..) | Color::Indexed(_)
    ));
}

fn row_text(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
    let mut text = String::new();
    let mut x = buffer.area.x;
    while x < buffer.area.right() {
        let cell = &buffer[(x, y)];
        text.push_str(cell.symbol());
        x += cell.cell_width().max(1);
    }
    text
}

#[test]
fn a_smaller_live_grid_keeps_terminal_background_across_the_entire_pane() {
    let mut source = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .unwrap();
    source.geometry.columns = 2;
    source.geometry.rows = 1;
    source.rows.truncate(1);
    source.rows[0].cells.truncate(2);
    let theme = Theme::orchbee_dark().with_terminal_preferences(
        &orchbee_workbench_protocol::UserSettingsValues {
            application_theme: orchbee_workbench_protocol::ApplicationAppearance::Dark,
            terminal_display_theme: orchbee_workbench_protocol::TerminalDisplayAppearance::Custom,
            custom_foreground: "#abcdef".into(),
            custom_background: "#102030".into(),
            ..Default::default()
        },
    );
    let view = LiveView::new(&source, 8, 4);
    let mut terminal = Terminal::new(TestBackend::new(8, 4)).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_live(frame, area, &source, view, false, &theme);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    for y in 0..4 {
        for x in 0..8 {
            if y != 0 || x >= 2 {
                assert_eq!(buffer[(x, y)].bg, theme.terminal_defaults().1);
            }
        }
    }
}

#[test]
fn captured_regional_indicator_cells_survive_paint_and_clear_their_entire_footprint() {
    use ratatui::buffer::{Buffer, CellDiffOption, CellWidth};
    let mut frame = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .unwrap();
    let mut style = frame.rows[0].cells[0].style;
    style.background = 0x112233;
    let cell = |text: &str, width| orchbee_terminal_protocol::TerminalTextCell {
        text: text.into(),
        width,
        style,
    };
    frame.rows = vec![TerminalTextRow {
        index: 0,
        flags: 0,
        prompt_class: 0,
        cells: vec![
            cell("|", 0),
            cell("\u{1f1e8}", 1),
            cell("", 2),
            cell("\u{1f1f3}", 1),
            cell("", 2),
            cell("|", 0),
        ],
    }];
    let mut terminal = Terminal::new(TestBackend::new(8, 2)).unwrap();
    terminal
        .draw(|surface| {
            render(
                surface,
                Rect::new(0, 0, 6, 2),
                &frame,
                false,
                &Theme::terminal_native(),
            )
        })
        .unwrap();
    let painted = terminal.backend().buffer().clone();
    assert_eq!(painted[(1, 0)].symbol(), "\u{1f1e8}");
    assert_eq!(painted[(3, 0)].symbol(), "\u{1f1f3}");
    assert_eq!(painted[(1, 0)].cell_width(), 2);
    assert!(matches!(
        painted[(1, 0)].diff_option,
        CellDiffOption::ForcedWidth(_)
    ));
    assert_eq!(painted[(5, 0)].symbol(), "|");
    let cleared = Buffer::empty(Rect::new(0, 0, 8, 2));
    let cleared_columns: Vec<_> = painted
        .diff_iter(&cleared)
        .filter(|(_, y, _)| *y == 0)
        .map(|(x, _, _)| x)
        .collect();
    for column in 0..6 {
        assert!(cleared_columns.contains(&column), "{cleared_columns:?}");
    }
}

#[test]
fn crop_header_preserves_occupied_cells_without_moving_live_content_or_cursor() {
    let mut source = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .unwrap();
    source.geometry.columns = 104;
    source.geometry.rows = 36;
    source.alternate_screen = false;
    source.interaction = orchbee_terminal_protocol::TerminalInteraction::default();
    source.cursor.visible = true;
    source.cursor.column = 0;
    source.cursor.row = 35;
    let style = source.rows[0].cells[0].style;
    source.rows = vec![TerminalTextRow {
        index: 35,
        flags: 0,
        prompt_class: 0,
        cells: vec![orchbee_terminal_protocol::TerminalTextCell {
            text: ">".into(),
            width: 0,
            style,
        }],
    }];
    for (columns, rows) in [(40, 10), (80, 24)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let area = Rect::new(0, 0, columns, rows);
            let occupied = Rect::new(columns - 12, 0, 12, 1);
            let crop = LiveView::new(&source, columns, rows);
            let mut terminal = Terminal::new(TestBackend::new(columns, rows)).unwrap();
            terminal
                .draw(|surface| {
                    surface.buffer_mut().set_string(
                        occupied.x,
                        occupied.y,
                        "############",
                        Style::new(),
                    );
                    if let Some(cursor) = super::super::viewport::render_live(
                        surface,
                        area,
                        &source,
                        crop,
                        true,
                        Some(occupied),
                        locale,
                        &Theme::terminal_native(),
                    ) {
                        surface.set_cursor_position(cursor);
                    }
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            for column in occupied.x..occupied.right() {
                assert_eq!(buffer[(column, 0)].symbol(), "#");
            }
            let prompt_y = crop.content(area).y + source.cursor.row - crop.row;
            assert_eq!(buffer[(0, prompt_y)].symbol(), ">");
            assert_eq!(
                terminal.get_cursor_position().unwrap(),
                ratatui::layout::Position::new(0, prompt_y),
            );
        }
    }
}

#[test]
fn clipped_live_view_renders_latest_rows_cursor_and_complete_localized_hint() {
    let mut source = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .unwrap();
    source.geometry.rows = 36;
    source.geometry.columns = 104;
    source.alternate_screen = false;
    source.interaction = orchbee_terminal_protocol::TerminalInteraction::default();
    source.cursor.column = 0;
    source.cursor.row = 35;
    source.cursor.visible = true;
    let style = source.rows[0].cells[0].style;
    source.rows = (0..36)
        .map(|index| TerminalTextRow {
            index,
            flags: 0,
            prompt_class: 0,
            cells: vec![orchbee_terminal_protocol::TerminalTextCell {
                text: if index == 35 { ">" } else { "x" }.into(),
                width: 0,
                style,
            }],
        })
        .collect();
    for theme in [Theme::terminal_native(), Theme::orchbee_dark()] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for (columns, rows) in [(80, 24), (120, 40), (60, 12), (40, 10), (19, 4)] {
                let mut terminal = Terminal::new(TestBackend::new(columns, rows)).unwrap();
                let crop = LiveView::new(&source, columns, rows);
                terminal
                    .draw(|surface| {
                        if let Some(cursor) = super::super::viewport::render_live(
                            surface,
                            Rect::new(0, 0, columns, rows),
                            &source,
                            crop,
                            true,
                            None,
                            locale,
                            &theme,
                        ) {
                            surface.set_cursor_position(cursor);
                        }
                    })
                    .unwrap();
                let content = crop.content(Rect::new(0, 0, columns, rows));
                let prompt_y = content.y + 35 - crop.row;
                assert_eq!(terminal.backend().buffer()[(0, prompt_y)].symbol(), ">");
                assert_eq!(
                    terminal.get_cursor_position().unwrap(),
                    ratatui::layout::Position::new(0, prompt_y)
                );
                if crop.clipped {
                    let text = row_text(terminal.backend().buffer(), 0);
                    assert!(
                        text.contains(locale.terminal_live_crop())
                            || text.contains(locale.terminal_live_crop_compact()),
                        "{text}"
                    );
                    assert!(prompt_y > 0);
                }
            }
        }
    }
}

#[test]
fn cropping_into_a_wide_tail_erases_old_cells_without_painting_half_a_glyph() {
    let mut source = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .unwrap();
    source.cursor.visible = true;
    source.cursor.column = 5;
    source.cursor.row = 0;
    let crop = LiveView::new(&source, 4, 4);
    assert_eq!(crop.column, 2);
    let mut terminal = Terminal::new(TestBackend::new(4, 4)).unwrap();
    terminal
        .draw(|frame| {
            frame.buffer_mut().set_string(0, 1, "XXXX", Style::new());
        })
        .unwrap();
    terminal
        .draw(|frame| {
            assert_eq!(
                render_live(
                    frame,
                    Rect::new(0, 0, 4, 4),
                    &source,
                    crop,
                    false,
                    &Theme::orchbee_light(),
                ),
                None
            );
        })
        .unwrap();
    let content = crop.content(Rect::new(0, 0, 4, 4));
    assert_eq!(terminal.backend().buffer()[(0, content.y)].symbol(), " ");
    assert_eq!(
        terminal.backend().buffer()[(1, content.y)].symbol(),
        "e\u{301}"
    );
}

#[test]
fn shared_tail_paints_retained_rows_and_moves_the_cursor_with_its_live_row() {
    let mut live = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
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
        graphics: None,
        rows,
    };
    let mut terminal = Terminal::new(TestBackend::new(10, 5)).unwrap();
    terminal
        .draw(|surface| {
            if let Some(cursor) = render_tail(
                surface,
                Rect::new(1, 1, 6, 3),
                &live,
                &window,
                2,
                true,
                &Theme::orchbee_dark(),
            ) {
                surface.set_cursor_position(cursor);
            }
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
        ApplicationAppearance as Application, TerminalDisplayAppearance as Appearance,
        UserSettingsValues,
    };
    let mut frame = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .unwrap();
    frame.foreground = 0xabcdef;
    frame.background = 0x102030;
    frame.rows[0].cells[0].style.foreground = 0x0100_0000;
    frame.rows[0].cells[0].style.background = 0x0100_0000;
    frame.rows[0].cells[1].style.foreground = 0x112233;
    frame.rows[0].cells[1].style.background = 0x445566;
    for (application, appearance, foreground, background) in [
        (
            Application::System,
            Appearance::Host,
            Color::Rgb(0xab, 0xcd, 0xef),
            Color::Rgb(0x10, 0x20, 0x30),
        ),
        (
            Application::Light,
            Appearance::Theme,
            Color::Rgb(20, 20, 20),
            Color::Rgb(252, 252, 252),
        ),
        (
            Application::Dark,
            Appearance::Theme,
            Color::Rgb(212, 212, 212),
            Color::Rgb(30, 30, 30),
        ),
        (
            Application::Light,
            Appearance::Custom,
            Color::Rgb(1, 2, 3),
            Color::Rgb(4, 5, 6),
        ),
    ] {
        let values = UserSettingsValues {
            application_theme: application,
            terminal_display_theme: appearance,
            custom_foreground: "#010203".into(),
            custom_background: "#040506".into(),
            ..UserSettingsValues::default()
        };
        // The painted theme is the slot the same preferences select.
        let chrome = if application == Application::Light {
            Theme::orchbee_light()
        } else {
            Theme::orchbee_dark()
        };
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
        "../../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
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
        graphics: None,
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
        Theme::orchbee_light(),
        Theme::orchbee_dark(),
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
                            ViewportSurface::Window { window: &window },
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
        "../../../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
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
