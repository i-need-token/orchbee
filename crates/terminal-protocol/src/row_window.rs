//! Shared bounded OBRW routing with distinct strict text and render decoders.
//! Each decoder validates the complete record and rejects the other detail.
use crate::{
    AttachmentId, HostEpoch, MAXIMUM_FRAME_CELLS, MAXIMUM_FRAME_COLUMNS, MAXIMUM_FRAME_ROWS,
    MAXIMUM_ROW_WINDOW_BYTES, MAXIMUM_TEXT_CELL_BYTES, ProtocolError, ProtocolErrorCode,
    RowWindowDetail,
};
use std::num::NonZeroU64;

mod render;
pub use render::{TerminalRenderRowWindow, decode_terminal_render_row_window};

pub const TERMINAL_ROW_WINDOW_HEADER_BYTES: usize = 112;

const TERMINAL_GRAPHICS_HEADER_BYTES: u64 = 40;
const DETAIL_RENDER: u8 = 1;
const DETAIL_TEXT: u8 = 2;
const ROW_FLAG_WRAP: u8 = 0x01;
const ROW_FLAG_WRAP_CONTINUATION: u8 = 0x02;
const ROW_FLAG_IDENTITY_ASCII: u8 = 0x04;
const ROW_FLAGS_MASK: u8 = ROW_FLAG_WRAP | ROW_FLAG_WRAP_CONTINUATION | ROW_FLAG_IDENTITY_ASCII;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalRowPromptClass {
    None,
    Prompt,
    PromptContinuation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTextCellMapping {
    pub byte_offset: u32,
    pub utf16_offset: u32,
    pub column: u16,
    pub width: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTextWindowRow {
    pub index: u64,
    pub prompt_class: TerminalRowPromptClass,
    pub wrap: bool,
    pub wrap_continuation: bool,
    /// Implicit per-cell mapping: when set, `mappings` is empty and cell index `i`
    /// maps to byte offset `i`, UTF-16 offset `i`, column `i`, width 1.
    pub identity_ascii: bool,
    pub text: Box<str>,
    pub mappings: Vec<TerminalTextCellMapping>,
}

#[derive(Debug)]
pub struct TerminalTextRowWindow {
    pub attachment_id: AttachmentId,
    pub host_epoch: HostEpoch,
    pub sequence: NonZeroU64,
    pub revision: u64,
    pub generation: NonZeroU64,
    pub total_rows: u64,
    pub start_row: u64,
    pub columns: u16,
    pub ignored_optional_flags: u8,
    pub rows: Vec<TerminalTextWindowRow>,
}

/// Routing and scope facts of a complete OBRW record. The same header validation
/// as `decode_terminal_text_row_window` runs first; row payloads are not decoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalRowWindowRoute {
    pub attachment_id: AttachmentId,
    pub host_epoch: HostEpoch,
    pub sequence: NonZeroU64,
    pub revision: u64,
    pub generation: NonZeroU64,
    pub total_rows: u64,
    pub start_row: u64,
    pub columns: u16,
    pub row_count: u16,
    pub detail: RowWindowDetail,
}

enum Detail {
    Render,
    Text,
}

struct Header {
    columns: u16,
    detail: Detail,
    entry_count: u32,
    row_count: u16,
    sequence: u64,
    revision: u64,
    generation: u64,
    total_rows: u64,
    start_row: u64,
    host_epoch: HostEpoch,
    attachment_id: AttachmentId,
    ignored_optional_flags: u8,
    projection_length: u32,
}

/// Decode one complete text-detail row window, validating the whole structure before
/// returning any row. A render window is rejected after full header validation without
/// interpreting its payload; this function never returns a partially decoded window.
pub fn decode_terminal_text_row_window(
    bytes: &[u8],
) -> Result<TerminalTextRowWindow, ProtocolError> {
    let header = decode_header(bytes)?;
    if matches!(header.detail, Detail::Render) {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidField,
            "OBRW render detail is outside the text row-window decoder",
        ));
    }
    let body = &bytes[TERMINAL_ROW_WINDOW_HEADER_BYTES..];
    let rows = decode_text_rows(
        body,
        header.row_count,
        header.columns,
        header.entry_count,
        header.start_row,
    )?;
    Ok(TerminalTextRowWindow {
        attachment_id: header.attachment_id,
        host_epoch: header.host_epoch,
        sequence: NonZeroU64::new(header.sequence).ok_or_else(field_error)?,
        revision: header.revision,
        generation: NonZeroU64::new(header.generation).ok_or_else(field_error)?,
        total_rows: header.total_rows,
        start_row: header.start_row,
        columns: header.columns,
        ignored_optional_flags: header.ignored_optional_flags,
        rows,
    })
}

/// Validate a complete OBRW record's fixed structure and return its routing
/// facts without decoding rows. Render and text windows both inspect.
pub fn inspect_terminal_row_window_route(
    bytes: &[u8],
) -> Result<TerminalRowWindowRoute, ProtocolError> {
    decode_header(bytes)?.route()
}

impl Header {
    fn route(&self) -> Result<TerminalRowWindowRoute, ProtocolError> {
        Ok(TerminalRowWindowRoute {
            attachment_id: self.attachment_id,
            host_epoch: self.host_epoch,
            sequence: NonZeroU64::new(self.sequence).ok_or_else(field_error)?,
            revision: self.revision,
            generation: NonZeroU64::new(self.generation).ok_or_else(field_error)?,
            total_rows: self.total_rows,
            start_row: self.start_row,
            columns: self.columns,
            row_count: self.row_count,
            detail: match self.detail {
                Detail::Render => RowWindowDetail::Render,
                Detail::Text => RowWindowDetail::Text,
            },
        })
    }
}

fn decode_header(bytes: &[u8]) -> Result<Header, ProtocolError> {
    if bytes.len() < TERMINAL_ROW_WINDOW_HEADER_BYTES {
        return Err(error(ProtocolErrorCode::Truncated));
    }
    if bytes.len() > MAXIMUM_ROW_WINDOW_BYTES {
        return Err(error(ProtocolErrorCode::LimitExceeded));
    }
    if !bytes.starts_with(b"OBRW") {
        return Err(error(ProtocolErrorCode::InvalidMagic));
    }
    let mut reader = Reader::new(&bytes[4..TERMINAL_ROW_WINDOW_HEADER_BYTES]);
    let format_major = reader.u8()?;
    let format_minor = reader.u8()?;
    let detail_code = reader.u8()?;
    let flags = reader.u8()?;
    let total_length = reader.u32()?;
    let header_length = reader.u16()?;
    let columns = reader.u16()?;
    let host_epoch_bytes = reader.array::<16>()?;
    let attachment_id_bytes = reader.array::<16>()?;
    let sequence = reader.u64()?;
    let revision = reader.u64()?;
    let generation = reader.u64()?;
    let total_rows = reader.u64()?;
    let start_row = reader.u64()?;
    let row_count = reader.u16()?;
    let reserved = reader.u16()?;
    let entry_count = reader.u32()?;
    let body_length = reader.u32()?;
    let format_patch = reader.u8()?;
    let projection_length = reader.u32()?;
    let graphics_length = reader.u32()?;
    let reserved_tail = reader.array::<3>()?;
    if format_major != 0 || format_minor != 0 || format_patch != 3 || header_length != 112 {
        return Err(error(ProtocolErrorCode::UnsupportedVersion));
    }
    if u64::from(total_length) != bytes.len() as u64
        || u64::from(body_length) != (bytes.len() - TERMINAL_ROW_WINDOW_HEADER_BYTES) as u64
    {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    if reserved != 0 || reserved_tail.iter().any(|byte| *byte != 0) || flags & 0x80 != 0 {
        return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
    }
    if columns == 0
        || usize::from(columns) > MAXIMUM_FRAME_COLUMNS
        || row_count == 0
        || usize::from(row_count) > MAXIMUM_FRAME_ROWS
        || u64::from(entry_count) > MAXIMUM_FRAME_CELLS as u64
        || sequence == 0
        || generation == 0
        || start_row > total_rows
        || u64::from(row_count) > total_rows - start_row
    {
        return Err(field_error());
    }
    let detail = match detail_code {
        DETAIL_RENDER => Detail::Render,
        DETAIL_TEXT => Detail::Text,
        _ => return Err(field_error()),
    };
    let body_length_u64 = u64::from(body_length);
    match detail {
        Detail::Render => {
            if projection_length == 0
                || u64::from(graphics_length) < TERMINAL_GRAPHICS_HEADER_BYTES
                || u64::from(projection_length) + u64::from(graphics_length) != body_length_u64
            {
                return Err(error(ProtocolErrorCode::LengthMismatch));
            }
        }
        Detail::Text => {
            if projection_length != 0 || graphics_length != 0 {
                return Err(error(ProtocolErrorCode::LengthMismatch));
            }
        }
    }
    let host_epoch = HostEpoch::from_bytes(host_epoch_bytes)
        .map_err(|_| error(ProtocolErrorCode::InvalidIdentifier))?;
    let attachment_id = AttachmentId::from_bytes(attachment_id_bytes)
        .map_err(|_| error(ProtocolErrorCode::InvalidIdentifier))?;
    Ok(Header {
        columns,
        detail,
        entry_count,
        row_count,
        sequence,
        revision,
        generation,
        total_rows,
        start_row,
        host_epoch,
        attachment_id,
        ignored_optional_flags: flags & 0x7f,
        projection_length,
    })
}

fn decode_text_rows(
    body: &[u8],
    row_count: u16,
    columns: u16,
    entry_count: u32,
    start_row: u64,
) -> Result<Vec<TerminalTextWindowRow>, ProtocolError> {
    let mut reader = Reader::new(body);
    let mut rows: Vec<TerminalTextWindowRow> = Vec::with_capacity(usize::from(row_count));
    let mut decoded_mappings = 0_usize;
    for row_offset in 0..u64::from(row_count) {
        let row_flags = reader.u8()?;
        let prompt_class_code = reader.u8()?;
        let reserved = reader.u16()?;
        let text_length = reader.u32()?;
        let mapping_count = reader.u32()?;
        if reserved != 0 || row_flags & !ROW_FLAGS_MASK != 0 {
            return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
        }
        let identity_ascii = row_flags & ROW_FLAG_IDENTITY_ASCII != 0;
        if mapping_count > u32::from(columns) {
            return Err(error(ProtocolErrorCode::LimitExceeded));
        }
        let text_bytes = reader.take(text_length as usize)?;
        let text =
            std::str::from_utf8(text_bytes).map_err(|_| error(ProtocolErrorCode::InvalidUtf8))?;
        if identity_ascii
            && (mapping_count != 0
                || text_bytes.len() > usize::from(columns)
                || !text_bytes.is_ascii())
        {
            return Err(field_error());
        }
        let logical_mapping_count = if identity_ascii {
            text_bytes.len()
        } else {
            mapping_count as usize
        };
        if !identity_ascii && !text_bytes.is_empty() && mapping_count == 0 {
            return Err(field_error());
        }
        if logical_mapping_count > MAXIMUM_FRAME_CELLS - decoded_mappings {
            return Err(error(ProtocolErrorCode::LimitExceeded));
        }
        let utf16_length = if identity_ascii {
            // Identity rows carry no mapping entries, so no UTF-16 offset is
            // ever compared against this value.
            0_u64
        } else {
            text.chars().map(|char| char.len_utf16() as u64).sum()
        };
        let mut mappings = Vec::with_capacity(mapping_count as usize);
        let mut previous_byte_offset: Option<u32> = None;
        let mut previous_utf16_offset: Option<u32> = None;
        let mut previous_column_end = 0_u32;
        let mut position = TextPosition::default();
        for mapping_offset in 0..mapping_count {
            let byte_offset = reader.u32()?;
            let utf16_offset = reader.u32()?;
            let column = reader.u16()?;
            let width = reader.u8()?;
            let mapping_reserved = reader.u8()?;
            if mapping_reserved != 0 {
                return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
            }
            if byte_offset as usize >= text_bytes.len()
                || u64::from(utf16_offset) >= utf16_length
                || u32::from(column) + u32::from(width) > u32::from(columns)
                || (width != 1 && width != 2)
                || (mapping_offset == 0 && (byte_offset != 0 || utf16_offset != 0))
                || !position.advance_to(text, byte_offset, utf16_offset)
                || previous_byte_offset.is_some_and(|previous| byte_offset <= previous)
                || previous_utf16_offset.is_some_and(|previous| utf16_offset <= previous)
                || u32::from(column) < previous_column_end
            {
                return Err(field_error());
            }
            if previous_byte_offset
                .is_some_and(|previous| (byte_offset - previous) as usize > MAXIMUM_TEXT_CELL_BYTES)
            {
                return Err(error(ProtocolErrorCode::LimitExceeded));
            }
            mappings.push(TerminalTextCellMapping {
                byte_offset,
                utf16_offset,
                column,
                width,
            });
            previous_byte_offset = Some(byte_offset);
            previous_utf16_offset = Some(utf16_offset);
            previous_column_end = u32::from(column) + u32::from(width);
        }
        if let Some(last) = mappings.last()
            && text_bytes.len() - last.byte_offset as usize > MAXIMUM_TEXT_CELL_BYTES
        {
            return Err(error(ProtocolErrorCode::LimitExceeded));
        }
        decoded_mappings += logical_mapping_count;
        let prompt_class = match prompt_class_code {
            0 => TerminalRowPromptClass::None,
            1 => TerminalRowPromptClass::Prompt,
            2 => TerminalRowPromptClass::PromptContinuation,
            _ => return Err(field_error()),
        };
        let wrap = row_flags & ROW_FLAG_WRAP != 0;
        let wrap_continuation = row_flags & ROW_FLAG_WRAP_CONTINUATION != 0;
        if let Some(prior) = rows.last()
            && prior.wrap != wrap_continuation
        {
            return Err(field_error());
        }
        rows.push(TerminalTextWindowRow {
            index: start_row + row_offset,
            prompt_class,
            wrap,
            wrap_continuation,
            identity_ascii,
            text: text.into(),
            mappings,
        });
    }
    if !reader.bytes.is_empty() || decoded_mappings != entry_count as usize {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    Ok(rows)
}

#[derive(Default)]
struct TextPosition {
    byte_offset: usize,
    utf16_offset: u64,
}

impl TextPosition {
    /// Advance over already-validated UTF-8 to the target offsets, requiring the byte
    /// target to land on a scalar boundary and the UTF-16 count to match exactly.
    fn advance_to(&mut self, text: &str, target_byte: u32, target_utf16: u32) -> bool {
        let target_byte = target_byte as usize;
        let target_utf16 = u64::from(target_utf16);
        while self.byte_offset < target_byte {
            let Some(scalar) = text[self.byte_offset..].chars().next() else {
                return false;
            };
            self.byte_offset += scalar.len_utf8();
            self.utf16_offset += scalar.len_utf16() as u64;
        }
        self.byte_offset == target_byte && self.utf16_offset == target_utf16
    }
}

fn field_error() -> ProtocolError {
    error(ProtocolErrorCode::InvalidField)
}

fn error(code: ProtocolErrorCode) -> ProtocolError {
    ProtocolError::new(code, "invalid OBRW row window")
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
    fn take(&mut self, length: usize) -> Result<&'a [u8], ProtocolError> {
        let (value, rest) = self
            .bytes
            .split_at_checked(length)
            .ok_or_else(|| error(ProtocolErrorCode::Truncated))?;
        self.bytes = rest;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProtocolError> {
        self.take(N)?
            .try_into()
            .map_err(|_| error(ProtocolErrorCode::Truncated))
    }
    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(u8::from_le_bytes(self.array()?))
    }
    fn u16(&mut self) -> Result<u16, ProtocolError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}
