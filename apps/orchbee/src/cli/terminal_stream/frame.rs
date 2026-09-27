//! Full ANSI viewport projection from admitted cells, never raw terminal output.
use super::super::{Failure, FailureKind, output, terminal_output::ansi};
use base64::Engine;
use orchbee_terminal_protocol::TerminalTextFrame;
use serde_json::{Value, json};

pub(super) fn history(
    window: &orchbee_terminal_protocol::TerminalTextRowWindow,
    columns: u16,
    rows: u16,
) -> Result<Value, Failure> {
    let bytes = render_history(window, columns, rows)?;
    Ok(json!({"kind":"history","encoding":"ansi","full":true,
        "sequence":window.sequence.to_string(),"generation":window.generation.to_string(),"revision":window.revision.to_string(),
        "width":columns,"height":rows,"startRow":window.start_row.to_string(),"totalRows":window.total_rows.to_string(),
        "bytes":base64::engine::general_purpose::STANDARD.encode(bytes)}))
}

pub(super) fn render_history(
    window: &orchbee_terminal_protocol::TerminalTextRowWindow,
    columns: u16,
    rows: u16,
) -> Result<Vec<u8>, Failure> {
    let bytes = output::render_text(|out| {
        out.push("\x1b[0m\x1b[?25l\x1b[2J\x1b[H")?;
        for (line, row) in window.rows.iter().take(usize::from(rows)).enumerate() {
            if row.identity_ascii {
                out.write(format_args!("\x1b[{};1H", line + 1))?;
                let text = row
                    .text
                    .get(..row.text.len().min(usize::from(columns)))
                    .unwrap_or("");
                ansi::write_cell_text(out, text)?;
                continue;
            }
            for (index, cell) in row.mappings.iter().enumerate() {
                if cell.column >= columns {
                    continue;
                }
                let end = row
                    .mappings
                    .get(index + 1)
                    .map_or(row.text.len(), |next| next.byte_offset as usize);
                let text = row.text.get(cell.byte_offset as usize..end).unwrap_or("");
                out.write(format_args!("\x1b[{};{}H", line + 1, cell.column + 1))?;
                if cell.width == 2 && cell.column + 1 >= columns {
                    out.push(" ")?;
                } else if text
                    .chars()
                    .any(crate::terminal_text::is_forbidden_terminal_scalar)
                    || ratatui::text::Span::raw(text).width() != usize::from(cell.width)
                {
                    out.push(if cell.width == 2 { "� " } else { "�" })?;
                } else {
                    ansi::write_cell_text(out, text)?;
                }
            }
        }
        Ok(())
    })
    .map_err(|_| Failure::new(FailureKind::OutputTooLarge))?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err(Failure::new(FailureKind::OutputTooLarge));
    }
    Ok(bytes)
}

pub(super) fn record(frame: &TerminalTextFrame, columns: u16, rows: u16) -> Result<Value, Failure> {
    let bytes = render(frame, columns, rows)?;
    Ok(json!({
        "kind":"frame", "encoding":"ansi", "full":true,
        "sequence":frame.route.sequence.to_string(), "generation":frame.route.generation.to_string(),
        "revision":frame.route.revision.to_string(), "width":columns, "height":rows,
        "sourceWidth":frame.geometry.columns, "sourceHeight":frame.geometry.rows,
        "bytes":base64::engine::general_purpose::STANDARD.encode(bytes),
    }))
}

pub(super) fn render(
    frame: &TerminalTextFrame,
    columns: u16,
    rows: u16,
) -> Result<Vec<u8>, Failure> {
    let bytes = output::render_text(|out| {
        let base = orchbee_terminal_protocol::TerminalTextStyle {
            foreground: frame.foreground,
            background: frame.background,
            underline_color: 0x0100_0000,
            flags: 0,
            underline: 0,
        };
        ansi::write_style(out, base)?;
        out.push("\x1b[?25l\x1b[2J\x1b[H")?;
        for row in frame.rows.iter().take(usize::from(rows)) {
            out.write(format_args!("\x1b[{};1H", row.index + 1))?;
            let mut previous = None;
            for (column, cell) in row.cells.iter().take(usize::from(columns)).enumerate() {
                if cell.width == 2 {
                    continue;
                }
                let mut style = cell.style;
                if style.foreground > 0x00ff_ffff {
                    style.foreground = frame.foreground;
                }
                if style.background > 0x00ff_ffff {
                    style.background = frame.background;
                }
                if previous != Some(style) {
                    ansi::write_style(out, style)?;
                    previous = Some(style);
                }
                let width = if cell.width == 1 { 2 } else { 1 };
                if cell.width == 3 || (width == 2 && column + 1 >= usize::from(columns)) {
                    out.push(" ")?;
                } else if cell.text.is_empty() {
                    out.push(if width == 2 { "  " } else { " " })?;
                } else if cell
                    .text
                    .chars()
                    .any(crate::terminal_text::is_forbidden_terminal_scalar)
                    || ratatui::text::Span::raw(cell.text.as_ref()).width() != width
                {
                    out.push(if width == 2 { "� " } else { "�" })?;
                } else {
                    ansi::write_cell_text(out, &cell.text)?;
                }
            }
        }
        out.push("\x1b[0m")?;
        if frame.cursor.visible && frame.cursor.column < columns && frame.cursor.row < rows {
            out.write(format_args!(
                "\x1b[{};{}H\x1b[?25h",
                frame.cursor.row + 1,
                frame.cursor.column + 1
            ))?;
        }
        Ok(())
    })
    .map_err(|_| Failure::new(FailureKind::OutputTooLarge))?;
    // Reserve base64 and record overhead before allocating the encoded content.
    if bytes.len() > 5 * 1024 * 1024 {
        return Err(Failure::new(FailureKind::OutputTooLarge));
    }
    Ok(bytes)
}
