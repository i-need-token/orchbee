use std::{
    io::{self, Write},
    sync::Arc,
};

use crossterm::style::{Attribute, SetAttribute};
use ratatui::{
    Terminal,
    backend::Backend,
    buffer::Cell,
    style::{Color, Modifier},
};

use crate::{
    terminal_device::WindowDimensions,
    terminal_lifecycle::{
        SYNCHRONIZED_UPDATE_BEGIN, SYNCHRONIZED_UPDATE_END, SynchronizedUpdateLease,
        write_ansi_command,
    },
};

use super::{
    FixedViewportBackend, ResizableBackend, RestorationGatedWriter, write_modifier_transition,
};

mod cursor;

#[test]
fn resolved_cell_colors_keep_ansi_indices_rgb_and_resets() -> io::Result<()> {
    let mut cells = Vec::new();
    let mut expected = Vec::new();
    for (color, foreground, background) in [
        (Color::Black, "38;5;0", "48;5;0"),
        (Color::Red, "38;5;1", "48;5;1"),
        (Color::Green, "38;5;2", "48;5;2"),
        (Color::Yellow, "38;5;3", "48;5;3"),
        (Color::Blue, "38;5;4", "48;5;4"),
        (Color::Magenta, "38;5;5", "48;5;5"),
        (Color::Cyan, "38;5;6", "48;5;6"),
        (Color::Gray, "38;5;7", "48;5;7"),
        (Color::DarkGray, "38;5;8", "48;5;8"),
        (Color::LightRed, "38;5;9", "48;5;9"),
        (Color::LightGreen, "38;5;10", "48;5;10"),
        (Color::LightYellow, "38;5;11", "48;5;11"),
        (Color::LightBlue, "38;5;12", "48;5;12"),
        (Color::LightMagenta, "38;5;13", "48;5;13"),
        (Color::LightCyan, "38;5;14", "48;5;14"),
        (Color::White, "38;5;15", "48;5;15"),
        (Color::Indexed(173), "38;5;173", "48;5;173"),
        (Color::Rgb(18, 52, 86), "38;2;18;52;86", "48;2;18;52;86"),
        (Color::Reset, "39", "49"),
    ] {
        let mut cell = Cell::new("x");
        cell.fg = color;
        cell.bg = color;
        cells.push(cell);
        expected.push(format!("\x1b[{foreground}m\x1b[{background}mx"));
    }
    let dimensions = WindowDimensions {
        columns: u16::try_from(cells.len()).unwrap(),
        ..TEST_DIMENSIONS
    };
    let mut backend = new_test_backend(Vec::new(), dimensions);
    backend.draw(
        cells
            .iter()
            .enumerate()
            .map(|(x, cell)| (u16::try_from(x).unwrap(), 0, cell)),
    )?;
    backend.flush()?;
    let output = std::str::from_utf8(&backend.writer).unwrap();
    let mut remaining = output;
    for sequence in expected {
        let (_, after) = remaining
            .split_once(&sequence)
            .unwrap_or_else(|| panic!("missing resolved color {sequence:?} in {output:?}"));
        remaining = after;
    }
    assert!(!output.contains("\x1b[m"), "{output:?}");
    Ok(())
}

#[test]
fn resolved_cell_colors_do_not_consult_the_process_no_color_policy() {
    for no_color in [Some("1"), None] {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "tui_backend::tests::resolved_cell_colors_keep_ansi_indices_rgb_and_resets",
            "--nocapture",
        ]);
        if let Some(value) = no_color {
            command.env("NO_COLOR", value);
        } else {
            command.env_remove("NO_COLOR");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "NO_COLOR={no_color:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "the exact child color witness must run: {:?}",
            output,
        );
    }
}

fn new_test_backend<W: Write>(writer: W, dimensions: WindowDimensions) -> FixedViewportBackend<W> {
    FixedViewportBackend::new(writer, dimensions, Arc::new(SynchronizedUpdateLease::new()))
}

#[test]
fn reports_only_its_explicitly_supplied_cell_and_pixel_viewport() -> std::io::Result<()> {
    let initial = WindowDimensions {
        columns: 120,
        pixel_height: 640,
        pixel_width: 960,
        rows: 32,
    };
    let mut backend = new_test_backend(Vec::<u8>::new(), initial);

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
    let mut backend = new_test_backend(Vec::<u8>::new(), dimensions);
    let payload = "safe\x1b]52;c;payload\x07\r\n\u{85}\u{2028}\u{2029}界e\u{301}";
    let mut cell = Cell::default();
    cell.set_symbol(payload);

    backend.draw(std::iter::once((0, 0, &cell)))?;

    let output = std::str::from_utf8(&backend.writer)?;
    assert!(output.contains("\x1b["));
    assert!(!output.contains("\x1b]52;c;payload"));
    assert!(output.contains("safe"));
    for forbidden in ['\x07', '\r', '\n', '\u{85}', '\u{2028}', '\u{2029}'] {
        assert!(!output.contains(forbidden));
    }
    // Forbidden scalars and whole sequences are dropped at the print
    // boundary — never echoed back as replacement-glyph noise.
    assert_eq!(output.matches('\u{fffd}').count(), 0);
    assert!(output.contains("界e\u{301}"));
    assert_eq!(cell.symbol(), payload);
    Ok(())
}

#[test]
fn dense_same_style_cells_are_encoded_as_one_cursor_run() -> io::Result<()> {
    let dimensions = WindowDimensions {
        columns: 4,
        pixel_height: 16,
        pixel_width: 32,
        rows: 1,
    };
    let mut cells = [
        Cell::default(),
        Cell::default(),
        Cell::default(),
        Cell::default(),
    ];
    for (cell, symbol) in cells.iter_mut().zip(["a", "b", "c", "d"]) {
        cell.set_symbol(symbol);
        cell.fg = Color::Red;
        cell.bg = Color::Blue;
        cell.modifier = Modifier::BOLD;
    }
    let mut backend = new_test_backend(Vec::<u8>::new(), dimensions);

    backend.draw(
        cells
            .iter()
            .enumerate()
            .map(|(x, cell)| (x as u16, 0, cell)),
    )?;
    backend.flush()?;

    let output = &backend.writer;
    assert_eq!(marker_positions(output, b"\x1b[1;1H").len(), 1);
    for redundant in [b"\x1b[1;2H".as_slice(), b"\x1b[1;3H", b"\x1b[1;4H"] {
        assert!(marker_positions(output, redundant).is_empty());
    }
    assert!(output.windows(4).any(|window| window == b"abcd"));
    assert!(
        output.len() < 96,
        "four cells should not carry four cursor and style prefixes: {} bytes",
        output.len()
    );
    Ok(())
}

#[test]
fn cursor_run_tracks_emitted_wide_symbols_and_real_gaps() -> io::Result<()> {
    let dimensions = WindowDimensions {
        columns: 5,
        pixel_height: 16,
        pixel_width: 40,
        rows: 1,
    };
    let mut wide = Cell::default();
    wide.set_symbol("界");
    let mut adjacent = Cell::default();
    adjacent.set_symbol("b");
    let mut gapped = Cell::default();
    gapped.set_symbol("c");
    let mut backend = new_test_backend(Vec::<u8>::new(), dimensions);

    backend.draw([(0, 0, &wide), (2, 0, &adjacent), (4, 0, &gapped)].into_iter())?;

    assert_eq!(marker_positions(&backend.writer, b"\x1b[1;1H").len(), 1);
    assert!(
        marker_positions(&backend.writer, b"\x1b[1;3H").is_empty(),
        "the terminal is already at column 3 after the wide glyph"
    );
    assert_eq!(marker_positions(&backend.writer, b"\x1b[1;5H").len(), 1);
    Ok(())
}

#[test]
fn captured_width_and_complex_symbols_never_guess_the_next_host_cursor_position() -> io::Result<()>
{
    use ratatui::buffer::CellDiffOption;
    use std::num::NonZeroU16;
    let dimensions = WindowDimensions {
        columns: 12,
        rows: 2,
        pixel_width: 0,
        pixel_height: 0,
    };
    for symbol in [
        "\u{1f1e8}",
        "\u{1f1e8}\u{1f1f3}",
        "\u{1f469}\u{200d}\u{1f4bb}",
        "e\u{301}",
    ] {
        let mut glyph = Cell::default();
        glyph
            .set_symbol(symbol)
            .set_diff_option(CellDiffOption::ForcedWidth(NonZeroU16::new(2).unwrap()));
        let mut suffix = Cell::default();
        suffix.set_symbol("|");
        let mut backend = new_test_backend(Vec::new(), dimensions);
        backend.draw([(0, 0, &glyph), (2, 0, &suffix)].into_iter())?;
        let output = std::str::from_utf8(&backend.writer).unwrap();
        assert!(
            output.contains(&format!("{symbol}\x1b[1;3H|")),
            "{output:?}"
        );
    }
    Ok(())
}

fn attribute_bytes(attribute: Attribute) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_ansi_command(&mut bytes, &SetAttribute(attribute)).unwrap();
    bytes
}

#[test]
fn removing_one_blink_kind_reapplies_the_other_after_shared_reset() -> io::Result<()> {
    let mut bytes = Vec::new();
    write_modifier_transition(
        &mut bytes,
        Modifier::SLOW_BLINK | Modifier::RAPID_BLINK,
        Modifier::SLOW_BLINK,
    )?;

    let reset = attribute_bytes(Attribute::NoBlink);
    let slow = attribute_bytes(Attribute::SlowBlink);
    let rapid = attribute_bytes(Attribute::RapidBlink);
    let reset_at = marker_positions(&bytes, &reset);
    let slow_at = marker_positions(&bytes, &slow);
    assert_eq!(reset_at.len(), 1);
    assert_eq!(slow_at.len(), 1);
    assert!(reset_at[0] < slow_at[0]);
    assert!(marker_positions(&bytes, &rapid).is_empty());
    Ok(())
}

#[test]
fn every_draw_reestablishes_style_and_cursor_baselines_inside_one_frame() -> io::Result<()> {
    let mut first = Cell::default();
    first.set_symbol("a");
    first.fg = Color::Red;
    first.modifier = Modifier::BOLD;
    let mut second = Cell::default();
    second.set_symbol("b");
    let mut backend = new_test_backend(Vec::<u8>::new(), TEST_DIMENSIONS);

    backend.draw(std::iter::once((0, 0, &first)))?;
    backend.draw(std::iter::once((1, 0, &second)))?;
    backend.flush()?;

    let hide = b"\x1b[?25l";
    assert_eq!(
        marker_positions(&backend.writer, SYNCHRONIZED_UPDATE_BEGIN).len(),
        1
    );
    assert_eq!(marker_positions(&backend.writer, hide).len(), 2);
    assert_eq!(
        marker_positions(&backend.writer, SYNCHRONIZED_UPDATE_END).len(),
        1
    );
    Ok(())
}

#[test]
fn ratatui_paints_its_cursor_without_exposing_the_host_cursor() -> io::Result<()> {
    let backend = new_test_backend(Vec::<u8>::new(), TEST_DIMENSIONS);
    let mut terminal = Terminal::new(backend)?;

    for symbol in ["a", "b"] {
        terminal.draw(|frame| {
            frame.buffer_mut()[(0, 0)].set_symbol(symbol);
            frame.set_cursor_position((1, 1));
        })?;
    }

    let output = &terminal.backend().writer;
    let hides = marker_positions(output, b"\x1b[?25l");
    let shows = marker_positions(output, b"\x1b[?25h");
    let ends = marker_positions(output, SYNCHRONIZED_UPDATE_END);
    assert_eq!(hides.len(), 2);
    assert!(shows.is_empty());
    assert_eq!(ends.len(), 2);
    for index in 0..2 {
        assert!(hides[index] < ends[index]);
    }
    let (position, cell) = terminal.backend().painted_cursor.as_ref().unwrap();
    assert_eq!(*position, ratatui::layout::Position::new(1, 1));
    assert_eq!(cell.symbol(), " ");
    assert!(output.windows(4).any(|bytes| bytes == b"\x1b[7m"));
    Ok(())
}

struct ScriptedWriter {
    buffer: Vec<u8>,
    fail_writes_from: Option<usize>,
    flush_calls: usize,
    flush_error_kind: Option<io::ErrorKind>,
    write_calls: usize,
    write_error_kind: io::ErrorKind,
}

impl ScriptedWriter {
    fn new() -> Self {
        Self {
            buffer: Vec::new(),
            fail_writes_from: None,
            flush_calls: 0,
            flush_error_kind: None,
            write_calls: 0,
            write_error_kind: io::ErrorKind::BrokenPipe,
        }
    }
}

impl Write for ScriptedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let index = self.write_calls;
        self.write_calls += 1;
        if self.fail_writes_from.is_some_and(|from| index >= from) {
            return Err(io::Error::new(
                self.write_error_kind,
                "injected write failure",
            ));
        }
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_calls += 1;
        match self.flush_error_kind {
            Some(kind) => Err(io::Error::new(kind, "injected flush failure")),
            None => Ok(()),
        }
    }
}

const TEST_DIMENSIONS: WindowDimensions = WindowDimensions {
    columns: 4,
    pixel_height: 32,
    pixel_width: 16,
    rows: 2,
};

fn marker_positions(output: &[u8], marker: &[u8]) -> Vec<usize> {
    output
        .windows(marker.len())
        .enumerate()
        .filter_map(|(index, window)| (window == marker).then_some(index))
        .collect()
}

#[test]
fn wraps_each_frame_diff_and_cursor_in_one_synchronized_pair_and_one_flush() -> io::Result<()> {
    let mut backend = new_test_backend(ScriptedWriter::new(), TEST_DIMENSIONS);
    let mut cell = Cell::default();
    cell.set_symbol("a");

    for _frame in 0..2 {
        backend.draw(std::iter::once((0, 0, &cell)))?;
        backend.hide_cursor()?;
        backend.set_cursor_position((1, 1))?;
        backend.show_cursor()?;
        backend.flush()?;
    }

    let output = &backend.writer.buffer;
    let begins = marker_positions(output, SYNCHRONIZED_UPDATE_BEGIN);
    let ends = marker_positions(output, SYNCHRONIZED_UPDATE_END);
    assert_eq!(begins.len(), 2);
    assert_eq!(ends.len(), 2);
    assert!(begins[0] < ends[0] && ends[0] < begins[1] && begins[1] < ends[1]);
    assert!(output.ends_with(SYNCHRONIZED_UPDATE_END));
    assert_eq!(backend.writer.flush_calls, 2);
    Ok(())
}

#[test]
fn a_failed_begin_still_owes_the_host_an_end_attempt() {
    let mut writer = ScriptedWriter::new();
    writer.fail_writes_from = Some(0);
    let mut backend = new_test_backend(writer, TEST_DIMENSIONS);
    let mut cell = Cell::default();
    cell.set_symbol("a");

    let error = backend
        .draw(std::iter::once((0, 0, &cell)))
        .expect_err("begin failure propagates");
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert!(backend.synchronized_update.frame_open());

    let writes_before_flush = backend.writer.write_calls;
    backend.writer.fail_writes_from = None;
    backend.writer.flush_error_kind = Some(io::ErrorKind::PermissionDenied);
    let error = backend.flush().expect_err("flush failure propagates");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(backend.writer.write_calls, writes_before_flush + 1);
    assert!(backend.writer.buffer.ends_with(SYNCHRONIZED_UPDATE_END));
    assert!(
        backend.synchronized_update.frame_open(),
        "an unconfirmed end keeps the pair pending"
    );

    backend.writer.flush_error_kind = None;
    backend.flush().expect("the recovery end attempt succeeds");
    assert!(!backend.synchronized_update.frame_open());
}

#[test]
fn a_failed_mid_frame_write_still_ends_the_open_pair() {
    let mut writer = ScriptedWriter::new();
    writer.fail_writes_from = Some(1);
    let mut backend = new_test_backend(writer, TEST_DIMENSIONS);
    let mut cell = Cell::default();
    cell.set_symbol("a");

    assert!(backend.draw(std::iter::once((0, 0, &cell))).is_err());
    assert!(backend.synchronized_update.frame_open());

    backend.writer.fail_writes_from = None;
    backend.flush().expect("end attempt succeeds");
    assert!(backend.writer.buffer.ends_with(SYNCHRONIZED_UPDATE_END));
    assert_eq!(backend.writer.flush_calls, 1);
}

#[test]
fn a_failed_flush_still_attempts_the_end_marker() {
    let mut writer = ScriptedWriter::new();
    writer.flush_error_kind = Some(io::ErrorKind::BrokenPipe);
    let mut backend = new_test_backend(writer, TEST_DIMENSIONS);
    let mut cell = Cell::default();
    cell.set_symbol("a");

    backend
        .draw(std::iter::once((0, 0, &cell)))
        .expect("draw succeeds");
    let error = backend.flush().expect_err("flush failure propagates");
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert!(backend.writer.buffer.ends_with(SYNCHRONIZED_UPDATE_END));
    assert!(
        backend.synchronized_update.frame_open(),
        "an end marker whose flush failed is unconfirmed"
    );
    assert_eq!(backend.writer.flush_calls, 1);

    backend.writer.flush_error_kind = None;
    backend.flush().expect("the recovery end attempt succeeds");
    assert_eq!(
        marker_positions(&backend.writer.buffer, SYNCHRONIZED_UPDATE_END).len(),
        2
    );
    assert!(!backend.synchronized_update.frame_open());
}

#[test]
fn a_failed_end_marker_is_reported_before_a_flush_failure() {
    let mut backend = new_test_backend(ScriptedWriter::new(), TEST_DIMENSIONS);
    let mut cell = Cell::default();
    cell.set_symbol("a");
    backend
        .draw(std::iter::once((0, 0, &cell)))
        .expect("draw succeeds");

    backend.writer.fail_writes_from = Some(backend.writer.write_calls);
    backend.writer.write_error_kind = io::ErrorKind::PermissionDenied;
    backend.writer.flush_error_kind = Some(io::ErrorKind::BrokenPipe);
    let error = backend.flush().expect_err("the first failure wins");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(backend.writer.flush_calls, 1);
    assert!(
        backend.synchronized_update.frame_open(),
        "a failed end write keeps the pair pending"
    );

    backend.writer.fail_writes_from = None;
    backend.writer.flush_error_kind = None;
    backend.flush().expect("the recovery end attempt succeeds");
    assert!(backend.writer.buffer.ends_with(SYNCHRONIZED_UPDATE_END));
    assert!(!backend.synchronized_update.frame_open());
}

#[test]
fn an_unconfirmed_end_keeps_the_pair_pending_for_recovery() {
    let mut backend = new_test_backend(ScriptedWriter::new(), TEST_DIMENSIONS);
    let mut cell = Cell::default();
    cell.set_symbol("a");
    backend
        .draw(std::iter::once((0, 0, &cell)))
        .expect("draw succeeds");

    backend.writer.fail_writes_from = Some(backend.writer.write_calls);
    backend.writer.flush_error_kind = Some(io::ErrorKind::BrokenPipe);
    assert!(backend.flush().is_err());
    assert!(
        backend.synchronized_update.frame_open(),
        "an unconfirmed end must keep the pair pending for lifecycle recovery"
    );

    backend.writer.fail_writes_from = None;
    backend.writer.flush_error_kind = None;
    backend.flush().expect("the recovery end attempt succeeds");
    assert!(!backend.synchronized_update.frame_open());
}

#[test]
fn a_flush_without_an_open_frame_emits_no_end_marker() -> io::Result<()> {
    let mut backend = new_test_backend(ScriptedWriter::new(), TEST_DIMENSIONS);

    backend.flush()?;

    assert!(backend.writer.buffer.is_empty());
    assert_eq!(backend.writer.flush_calls, 1);
    Ok(())
}

#[test]
fn a_restoration_gated_writer_discards_frame_bytes_once_recovery_starts() {
    let synchronized_update = Arc::new(SynchronizedUpdateLease::new());
    let mut writer =
        RestorationGatedWriter::new(Vec::<u8>::new(), Arc::clone(&synchronized_update));

    writer.write_all(b"live frame").expect("pre-restore write");
    writer.flush().expect("pre-restore flush");
    assert_eq!(writer.inner, b"live frame");

    synchronized_update.block_frame_output();
    writer
        .write_all(b"stale frame")
        .expect("blocked writes are absorbed");
    writer.flush().expect("blocked flush is a no-op");
    assert_eq!(
        writer.inner, b"live frame",
        "no late buffered frame reaches the restored terminal"
    );
}

#[test]
fn a_buffer_drop_flush_above_a_blocked_gate_emits_nothing() {
    let synchronized_update = Arc::new(SynchronizedUpdateLease::new());
    let sink = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let sink_for_writer = Arc::clone(&sink);
    {
        let mut writer = io::BufWriter::with_capacity(
            64,
            RestorationGatedWriter::new(
                MutexWriter(sink_for_writer),
                Arc::clone(&synchronized_update),
            ),
        );
        writer.write_all(b"pending frame").expect("buffered write");
        assert!(
            sink.lock().map(|bytes| bytes.is_empty()).unwrap_or(false),
            "the frame stays buffered before any flush"
        );
        synchronized_update.block_frame_output();
    }
    assert!(
        sink.lock().map(|bytes| bytes.is_empty()).unwrap_or(false),
        "the drop-time buffer flush is absorbed by the blocked gate"
    );
}

struct MutexWriter(Arc<std::sync::Mutex<Vec<u8>>>);

impl Write for MutexWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self.0.lock() {
            Ok(mut buffer) => {
                buffer.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            Err(_) => Err(io::Error::other("mutex sink was poisoned")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn synchronized_output_sequences_have_a_single_lifecycle_owner() {
    let lifecycle = include_str!("../terminal_lifecycle.rs");
    for required in ["2026h", "2026l"] {
        assert!(
            lifecycle.contains(required),
            "terminal lifecycle must own the synchronized update marker: {required}"
        );
    }
    for source in [
        include_str!("../tui_backend.rs"),
        include_str!("../tui_render.rs"),
        include_str!("../tui_shell.rs"),
    ] {
        for forbidden in ["2026h", "2026l"] {
            assert!(
                !source.contains(forbidden),
                "non-lifecycle source emits a synchronized update marker: {forbidden}"
            );
        }
    }
}
