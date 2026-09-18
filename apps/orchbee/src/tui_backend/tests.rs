use ratatui::{backend::Backend, buffer::Cell};

use crate::terminal_device::WindowDimensions;

use super::{FixedViewportBackend, ResizableBackend};

#[test]
fn reports_only_its_explicitly_supplied_cell_and_pixel_viewport() -> std::io::Result<()> {
    let initial = WindowDimensions {
        columns: 120,
        pixel_height: 640,
        pixel_width: 960,
        rows: 32,
    };
    let mut backend = FixedViewportBackend::new(Vec::<u8>::new(), initial);

    assert_eq!(backend.size()?.width, 120);
    assert_eq!(backend.size()?.height, 32);
    assert_eq!(backend.window_size()?.pixels.width, 960);
    assert_eq!(backend.window_size()?.pixels.height, 640);

    backend.set_window_dimensions(WindowDimensions {
        columns: 70,
        pixel_height: 480,
        pixel_width: 700,
        rows: 24,
    });
    assert_eq!(backend.size()?.width, 70);
    assert_eq!(backend.window_size()?.columns_rows.height, 24);
    Ok(())
}

#[test]
fn backend_implementation_never_delegates_dimension_or_cursor_queries() {
    let source = include_str!("../tui_backend.rs");

    for forbidden in [
        "CrosstermBackend",
        "inner.size(",
        "inner.window_size(",
        "inner.get_cursor_position(",
        "terminal::size",
        "terminal::window_size",
    ] {
        assert!(
            !source.contains(forbidden),
            "fixed backend contains forbidden global query path: {forbidden}"
        );
    }
}

#[test]
fn sanitizes_untrusted_cell_symbols_at_the_final_print_boundary()
-> Result<(), Box<dyn std::error::Error>> {
    let dimensions = WindowDimensions {
        columns: 1,
        pixel_height: 16,
        pixel_width: 8,
        rows: 1,
    };
    let mut backend = FixedViewportBackend::new(Vec::<u8>::new(), dimensions);
    let payload = "safe\x1b]52;c;payload\x07\r\n\u{85}\u{2028}\u{2029}界e\u{301}";
    let mut cell = Cell::default();
    cell.set_symbol(payload);

    backend.draw(std::iter::once((0, 0, &cell)))?;

    let output = std::str::from_utf8(&backend.writer)?;
    assert!(output.contains("\x1b["));
    assert!(!output.contains("\x1b]52;c;payload"));
    for forbidden in ['\x07', '\r', '\n', '\u{85}', '\u{2028}', '\u{2029}'] {
        assert!(!output.contains(forbidden));
    }
    assert_eq!(output.matches('\u{fffd}').count(), 0);
    assert!(output.contains("界e\u{301}"));
    assert_eq!(cell.symbol(), payload);
    Ok(())
}
