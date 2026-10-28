//! The immutable styled history surface shared by native TUI and CLI display.
use std::{
    collections::{HashMap, hash_map::Entry},
    sync::Arc,
};

use orchbee_terminal_protocol::{
    CURRENT_PROTOCOL_VERSION, MessageRole, PayloadEncoding, TerminalRenderRowWindow,
    TerminalRowWindowRoute, TerminalTextCell, TerminalTextRow, decode_terminal_render_row_window,
    decode_transport_envelope,
};

#[derive(Debug)]
pub struct ViewportWindow {
    pub route: TerminalRowWindowRoute,
    // Windows CLI streams reproduce captured defaults; themed TUI cells use roles.
    pub foreground: u32,
    pub background: u32,
    pub graphics_present: bool,
    pub rows: Vec<TerminalTextRow>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ViewportWindowFailure {
    Invalid,
    MemoryLimit,
}

/// Validate the entire response, including graphics, before publishing cells.
/// The core separately fences attachment, read sequence, generation and geometry.
pub fn decode_viewport_window(
    record: &[u8],
    maximum_bytes: u32,
) -> Result<ViewportWindow, ViewportWindowFailure> {
    let envelope = decode_transport_envelope(record).map_err(|_| ViewportWindowFailure::Invalid)?;
    if envelope.protocol != CURRENT_PROTOCOL_VERSION
        || envelope.kind != 46
        || envelope.role != MessageRole::Response
        || envelope.optional_message
        || envelope.payload_encoding != PayloadEncoding::RowWindow
        || envelope.correlation_id.is_none()
        || envelope.payload.len() > maximum_bytes as usize
    {
        return Err(ViewportWindowFailure::Invalid);
    }
    let window = decode_terminal_render_row_window(envelope.payload)
        .map_err(|_| ViewportWindowFailure::Invalid)?;
    if envelope.host_epoch != Some(window.route().host_epoch) {
        return Err(ViewportWindowFailure::Invalid);
    }
    materialize(&window, maximum_bytes as usize)
}

fn materialize(
    window: &TerminalRenderRowWindow<'_>,
    maximum_text_bytes: usize,
) -> Result<ViewportWindow, ViewportWindowFailure> {
    let projection = window.projection();
    // Immutable borrowed ranges identify aliases without hashing or allocating
    // the same potentially large grapheme once per cell.
    let mut strings = HashMap::<(usize, usize), Arc<str>>::new();
    let mut remaining = maximum_text_bytes;
    let rows = projection
        .rows()
        .map(|row| {
            Ok(TerminalTextRow {
                index: row.index(),
                flags: row.flags(),
                prompt_class: row.prompt_class(),
                cells: row
                    .cells()
                    .map(|cell| {
                        let text =
                            match strings.entry((cell.text.as_ptr() as usize, cell.text.len())) {
                                Entry::Occupied(entry) => Arc::clone(entry.get()),
                                Entry::Vacant(entry) => {
                                    remaining = remaining
                                        .checked_sub(cell.text.len())
                                        .ok_or(ViewportWindowFailure::MemoryLimit)?;
                                    Arc::clone(entry.insert(Arc::from(cell.text)))
                                }
                            };
                        Ok(TerminalTextCell {
                            text,
                            style: cell.style,
                            width: cell.width,
                        })
                    })
                    .collect::<Result<_, ViewportWindowFailure>>()?,
            })
        })
        .collect::<Result<_, ViewportWindowFailure>>()?;
    Ok(ViewportWindow {
        route: window.route(),
        foreground: projection.foreground(),
        background: projection.background(),
        graphics_present: window.graphics_placement_count() > 0,
        rows,
    })
}

#[cfg(test)]
mod tests;
