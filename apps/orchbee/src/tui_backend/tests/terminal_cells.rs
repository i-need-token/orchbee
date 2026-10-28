//! Captured terminal cells painted through the real fixed-viewport backend.

use std::{
    cell::RefCell,
    io::{self, Write},
    rc::Rc,
    sync::Arc,
};

use orchbee_terminal_protocol::TerminalTextRow;
use orchbee_workbench_protocol::{
    ApplicationAppearance as Application, TerminalDisplayAppearance as Appearance,
    UserPreferencesValues,
};
use ratatui::{Terminal, layout::Rect};

use crate::{
    terminal_device::WindowDimensions,
    terminal_lifecycle::SynchronizedUpdateLease,
    tui_backend::FixedViewportBackend,
    tui_render::render_terminal_cells as render,
    tui_theme::{ColorLevel, Theme},
};

#[test]
fn native_backend_keeps_blank_and_existing_input_cells_visible_for_every_terminal_appearance() {
    #[derive(Clone, Default)]
    struct Writer(Rc<RefCell<Vec<u8>>>);
    impl Write for Writer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut source = orchbee_terminal_protocol::decode_terminal_text_frame(include_bytes!(
        "../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .unwrap();
    source.foreground = 0x123456;
    source.background = 0x9abcde;
    source.geometry.columns = 16;
    source.geometry.rows = 2;
    let mut default_style = source.rows[0].cells[0].style;
    default_style.foreground = 0x0100_0000;
    default_style.background = 0x0100_0000;
    default_style.flags = 0;
    source.rows = vec![TerminalTextRow {
        index: 0,
        flags: 0,
        prompt_class: 0,
        cells: "Get-Date"
            .chars()
            .map(|character| orchbee_terminal_protocol::TerminalTextCell {
                text: character.to_string().into(),
                width: 0,
                style: default_style,
            })
            .collect(),
    }];
    source.cursor.visible = true;
    source.cursor.row = 0;
    for appearance in [
        Appearance::Light,
        Appearance::Dark,
        Appearance::Custom,
        Appearance::Host,
    ] {
        let values = UserPreferencesValues {
            application_theme: if appearance == Appearance::Host {
                Application::System
            } else {
                Application::Light
            },
            terminal_display_theme: appearance,
            custom_foreground: "#cccccc".into(),
            custom_background: "#cccccc".into(),
            ..UserPreferencesValues::default()
        };
        let theme = if appearance == Appearance::Host {
            Theme::application_with_host(
                Application::System,
                crate::terminal_host_appearance::HostAppearance::Dark,
                ColorLevel::Truecolor,
            )
        } else {
            Theme::one_dark()
        }
        .with_terminal_preferences(&values);
        let writer = Writer::default();
        let bytes = writer.0.clone();
        let backend = FixedViewportBackend::new(
            writer,
            WindowDimensions {
                columns: 20,
                rows: 4,
                pixel_width: 0,
                pixel_height: 0,
            },
            Arc::new(SynchronizedUpdateLease::new()),
        );
        let mut terminal = Terminal::new(backend).unwrap();
        for (column, glyph) in [(8, " "), (3, "-")] {
            source.cursor.column = column;
            let offset = bytes.borrow().len();
            terminal
                .draw(|surface| render(surface, Rect::new(1, 1, 16, 2), &source, true, &theme))
                .unwrap();
            let output = bytes.borrow();
            let output = std::str::from_utf8(&output[offset..]).unwrap();
            let channel = if appearance == Appearance::Dark {
                255
            } else {
                0
            };
            // The first frame paints the captured Host colors. The later cursor-only redraw
            // intentionally emits only the old and new cursor cells, not the retained row.
            if appearance == Appearance::Host && column == 8 {
                assert!(
                    output.contains("\x1b[38;2;18;52;86m\x1b[48;2;154;188;222mGet-Date"),
                    "{output:?}"
                );
            }
            assert!(
                output.contains(&format!("\x1b[48;2;{channel};{channel};{channel}m{glyph}")),
                "{appearance:?} {output:?}"
            );
            assert!(output.ends_with(&format!("\x1b[2;{}H\x1b[?2026l", column + 2)));
            assert!(!output.contains("\x1b[?25h"));
            assert_eq!(
                terminal.get_cursor_position().unwrap(),
                ratatui::layout::Position::new(column + 1, 1)
            );
        }
    }
}
