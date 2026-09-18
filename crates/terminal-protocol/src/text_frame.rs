//! Bounded OBTF text projection. Optional extension contents are not interpreted.
use crate::{
    MAXIMUM_TEXT_CELL_BYTES, ProtocolError, ProtocolErrorCode, TerminalFrameRoute,
    TerminalPixelGeometry, inspect_terminal_frame_route,
};
use std::sync::Arc;

mod rows;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTextStyle {
    pub foreground: u32,
    pub background: u32,
    pub underline_color: u32,
    pub flags: u16,
    pub underline: u8,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTextCell {
    pub text: Arc<str>,
    pub style: TerminalTextStyle,
    /// OBTF width code: narrow 0, wide 1, spacer tail 2, spacer head 3.
    pub width: u8,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTextRow {
    pub index: u16,
    pub flags: u8,
    pub cells: Vec<TerminalTextCell>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTextCursor {
    pub column: u16,
    pub row: u16,
    pub visible: bool,
}
#[derive(Debug)]
pub struct TerminalTextFrame {
    pub route: TerminalFrameRoute,
    pub geometry: TerminalPixelGeometry,
    pub cursor: TerminalTextCursor,
    pub alternate_screen: bool,
    pub foreground: u32,
    pub background: u32,
    pub theme_revision: u64,
    pub rows: Vec<TerminalTextRow>,
}

/// Decode text tables, validating their complete structure before returning any row.
/// Optional extensions are size-checked and skipped; this is not a graphics decoder.
pub fn decode_terminal_text_frame(bytes: &[u8]) -> Result<TerminalTextFrame, ProtocolError> {
    let route = inspect_terminal_frame_route(bytes)?;
    let mut header = Reader::new(bytes);
    header.take(88)?;
    let columns = header.u16()?;
    let row_count = header.u16()?;
    let cursor_column = header.u16()?;
    let cursor_row = header.u16()?;
    let cursor_shape = header.u8()?;
    let cursor_flags = header.u8()?;
    let screen = header.u8()?;
    let reserved = header.u8()?;
    let interaction = header.u32()?;
    let foreground = header.u32()?;
    let background = header.u32()?;
    let cursor_color = header.u32()?;
    let title = header.u32()?;
    let directory = header.u32()?;
    let strings_count = header.u32()?;
    let styles_count = header.u16()?;
    let encoded_rows = header.u16()?;
    let extensions_count = header.u16()?;
    let reserved_tables = header.u16()?;
    let geometry = TerminalPixelGeometry {
        columns,
        rows: row_count,
        cell_pixel_width: header.u16()?,
        cell_pixel_height: header.u16()?,
    };
    let cell_count = header.u32()?;
    header.u8()?;
    let theme_revision = header.u64()?;
    if reserved != 0
        || reserved_tables != 0
        || cursor_flags & !7 != 0
        || header.take(7)?.iter().any(|x| *x != 0)
    {
        return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
    }
    crate::geometry::validate_geometry(geometry)?;
    if cursor_column >= columns
        || cursor_row >= row_count
        || !(1..=3).contains(&cursor_shape)
        || !(1..=2).contains(&screen)
        || interaction & !0x3ffff != 0
        || ((interaction >> 4) & 15).count_ones() > 1
        || ((interaction >> 8) & 15).count_ones() > 1
        || foreground > 0xffffff
        || background > 0xffffff
        || (cursor_color > 0xffffff && cursor_color != 0x1000000)
        || (title != u32::MAX && title >= strings_count)
        || (directory != u32::MAX && directory >= strings_count)
    {
        return Err(error(ProtocolErrorCode::InvalidField));
    }
    if strings_count > 4096
        || styles_count > 1024
        || encoded_rows > 4096
        || cell_count > 262144
        || extensions_count > 64
    {
        return Err(error(ProtocolErrorCode::LimitExceeded));
    }
    if (route.base_revision.is_none() && encoded_rows != row_count)
        || cell_count != u32::from(encoded_rows) * u32::from(columns)
    {
        return Err(error(ProtocolErrorCode::InvalidField));
    }
    let mut strings: Vec<Arc<str>> = Vec::new();
    for _ in 0..strings_count {
        let length = header.u32()?;
        if length as usize > MAXIMUM_TEXT_CELL_BYTES {
            return Err(error(ProtocolErrorCode::LimitExceeded));
        }
        let text = std::str::from_utf8(header.take(length as usize)?)
            .map_err(|_| error(ProtocolErrorCode::InvalidUtf8))?;
        strings.push(Arc::from(text));
    }
    let mut styles = Vec::new();
    for _ in 0..styles_count {
        let foreground = header.u32()?;
        let background = header.u32()?;
        let underline_color = header.u32()?;
        let link = header.u32()?;
        let flags = header.u16()?;
        let underline = header.u8()?;
        if header.u8()? != 0 || flags & !0xff != 0 {
            return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
        }
        if ![foreground, background, underline_color]
            .into_iter()
            .all(valid_color)
            || underline > 5
            || (link != u32::MAX && link >= strings_count)
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        styles.push(TerminalTextStyle {
            foreground,
            background,
            underline_color,
            flags,
            underline,
        });
    }
    let rows = rows::read(
        &mut header,
        geometry,
        encoded_rows,
        route.base_revision.is_none(),
        &strings,
        &styles,
    )?;
    let mut extension_bytes = 0_u64;
    for _ in 0..extensions_count {
        header.u16()?;
        if header.u16()? & 0x8000 != 0 {
            return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
        }
        let length = header.u32()?;
        extension_bytes += u64::from(length);
        if extension_bytes > 33 * 1024 * 1024 {
            return Err(error(ProtocolErrorCode::LimitExceeded));
        }
        header.take(length as usize)?;
    }
    if !header.bytes.is_empty() {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    Ok(TerminalTextFrame {
        route,
        geometry,
        cursor: TerminalTextCursor {
            column: cursor_column,
            row: cursor_row,
            visible: cursor_flags & 1 != 0,
        },
        alternate_screen: screen == 2,
        foreground,
        background,
        theme_revision,
        rows,
    })
}
fn valid_color(value: u32) -> bool {
    value <= 0xffffff || value == 0x1000000 || value == 0x2000000
}
fn error(code: ProtocolErrorCode) -> ProtocolError {
    ProtocolError::new(code, "invalid OBTF text projection")
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
