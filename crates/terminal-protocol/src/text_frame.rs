//! Bounded OBTF text projection and typed progress; other optional extension contents are skipped.
use crate::{
    MAXIMUM_TEXT_CELL_BYTES, ProtocolError, ProtocolErrorCode, TerminalFrameRoute,
    TerminalPixelGeometry, inspect_terminal_frame_route,
};
use std::sync::Arc;

mod completion;
pub use completion::TerminalCommandCompletion;
mod progress;
mod rows;
mod sgr;
pub use progress::{TerminalProgress, TerminalProgressValue};
pub use sgr::{TerminalSgrStyle, terminal_sgr_style};

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
    pub prompt_class: u8,
    pub cells: Vec<TerminalTextCell>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalTextCursor {
    pub column: u16,
    pub row: u16,
    pub visible: bool,
}
#[derive(Clone, Debug)]
pub struct TerminalTextFrame {
    pub route: TerminalFrameRoute,
    pub geometry: TerminalPixelGeometry,
    pub cursor: TerminalTextCursor,
    pub alternate_screen: bool,
    /// The validated `captured_interaction` bitfield carried by this frame.
    /// Delta frames replace the whole value, so it always describes the
    /// terminal modes in force at this frame — never a merged history.
    pub interaction: TerminalInteraction,
    pub foreground: u32,
    pub background: u32,
    pub theme_revision: u64,
    /// Retained scrollback row count captured at frame time; zero means no history.
    pub scrollback_rows: u64,
    /// Raw nullable metadata captured in this frame, including explicit empty strings.
    pub title: Option<Arc<str>>,
    pub current_directory: Option<Arc<str>>,
    /// Presence of the validated current progress extension; absence means removed progress on the wire.
    pub progress: Option<TerminalProgress>,
    /// Latest OSC 133 command-completion fact, scoped by the physical terminal identity.
    pub command_completion: Option<TerminalCommandCompletion>,
    /// None leaves delta graphics unchanged. False identifies an exact empty OBKG update;
    /// other optional graphics payloads remain opaque and conservatively protect presentation.
    pub graphics_present: Option<bool>,
    pub rows: Vec<TerminalTextRow>,
}

/// The wire `captured_interaction` word after the decoder's own validation
/// (unknown bits rejected, tracking and format groups each exclusive). The
/// bit layout stays private; consumers use the named queries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalInteraction(u32);

impl TerminalInteraction {
    /// Mutually exclusive mouse-tracking mode (wire bits 4-7).
    pub const fn mouse_tracking(self) -> MouseTracking {
        match (self.0 >> 4) & 15 {
            1 => MouseTracking::X10,
            2 => MouseTracking::Normal,
            4 => MouseTracking::Button,
            8 => MouseTracking::Any,
            _ => MouseTracking::None,
        }
    }

    /// Mutually exclusive mouse report encoding (wire bits 8-11). `Default`
    /// is the traditional three-byte `ESC[M` form — no format bit set.
    pub const fn mouse_format(self) -> MouseFormat {
        match (self.0 >> 8) & 15 {
            1 => MouseFormat::Utf8,
            2 => MouseFormat::Sgr,
            4 => MouseFormat::Urxvt,
            8 => MouseFormat::SgrPixels,
            _ => MouseFormat::Default,
        }
    }

    /// Bracketed paste (wire bit 2), captured with the same screen revision.
    pub const fn bracketed_paste(self) -> bool {
        self.0 & (1 << 2) != 0
    }

    /// DEC 1007 alternate-screen wheel-to-cursor mode (wire bit 18).
    pub const fn alternate_scroll(self) -> bool {
        self.0 & (1 << 18) != 0
    }

    /// Focus reporting (wire bit 3), captured with the same screen revision.
    pub const fn focus_reporting(self) -> bool {
        self.0 & (1 << 3) != 0
    }

    /// Application cursor keys (wire bit 0): when set, cursor-key output
    /// uses SS3 (`ESC O`) instead of CSI (`ESC [`).
    pub const fn application_cursor_keys(self) -> bool {
        self.0 & 1 != 0
    }
}

/// Mutually exclusive DEC mouse-tracking modes (wire bits 4-7).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseTracking {
    /// No mouse tracking: the pane never receives mouse reports.
    None,
    /// X10: button presses only.
    X10,
    /// Normal tracking: presses and releases, no motion.
    Normal,
    /// Button-event tracking: presses, releases, and drags while a button
    /// is held.
    Button,
    /// Any-event tracking: all motion, with or without a button.
    Any,
}

/// Mutually exclusive mouse report encodings (wire bits 8-11).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseFormat {
    /// Traditional three-byte `ESC[M` report (no format bit set).
    Default,
    /// Extended three-byte report with UTF-8 encoded values.
    Utf8,
    /// SGR `ESC[<Cb;Cx;Cy` report (`M`/`m` final).
    Sgr,
    /// URXVT `ESC[Cb;Cx;CyM` numeric report.
    Urxvt,
    /// SGR report in pixel coordinates.
    SgrPixels,
}

/// Decode text tables, validating their complete structure before returning any row.
/// Optional extensions are size-checked and skipped; this is not a graphics decoder.
pub fn decode_terminal_text_frame(bytes: &[u8]) -> Result<TerminalTextFrame, ProtocolError> {
    let route = inspect_terminal_frame_route(bytes)?;
    let scrollback_rows = u64::from_le_bytes(
        bytes[80..88]
            .try_into()
            .map_err(|_| error(ProtocolErrorCode::Truncated))?,
    );
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
        || interaction & !0x7ffff != 0
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
    let mut progress = None;
    let mut command_completion = None;
    let mut graphics_present = None;
    for _ in 0..extensions_count {
        let kind = header.u16()?;
        if header.u16()? & 0x8000 != 0 {
            return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
        }
        let length = header.u32()?;
        extension_bytes += u64::from(length);
        if extension_bytes > 33 * 1024 * 1024 {
            return Err(error(ProtocolErrorCode::LimitExceeded));
        }
        let bytes = header.take(length as usize)?;
        if kind == 2 {
            if graphics_present.is_some() {
                return Err(error(ProtocolErrorCode::InvalidField));
            }
            graphics_present = Some(
                !(bytes.len() == 40
                    && &bytes[..7] == b"OBKG\0\0\x02"
                    && matches!(bytes[7], 1 | 2)
                    && bytes[8..12] == 40_u32.to_le_bytes()
                    && bytes[28..].iter().all(|byte| *byte == 0)),
            );
        }
        if kind == 3 {
            if progress.is_some() {
                return Err(error(ProtocolErrorCode::InvalidField));
            }
            progress = Some(progress::decode(bytes)?);
        }
        if kind == 4 {
            if command_completion.is_some() {
                return Err(error(ProtocolErrorCode::InvalidField));
            }
            command_completion = Some(completion::decode(bytes)?);
        }
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
        interaction: TerminalInteraction(interaction),
        foreground,
        background,
        theme_revision,
        scrollback_rows,
        title: (title != u32::MAX).then(|| strings[title as usize].clone()),
        current_directory: (directory != u32::MAX).then(|| strings[directory as usize].clone()),
        progress,
        command_completion,
        graphics_present,
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
