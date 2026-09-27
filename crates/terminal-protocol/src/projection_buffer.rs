//! Strict borrowed OBPB views. Canonical wire validation precedes every exposed cell.

use std::num::NonZeroU64;

use crate::{
    AttachmentId, HostEpoch, MAXIMUM_FRAME_CELLS, MAXIMUM_TERMINAL_FRAME_BYTES,
    MAXIMUM_TEXT_CELL_BYTES, ProtocolError, ProtocolErrorCode, TerminalFrameRoute,
    TerminalPixelGeometry, TerminalTextStyle,
};

const HEADER: usize = 320;
const ELEMENT_BYTES: [usize; 16] = [2, 1, 1, 4, 2, 4, 2, 4, 4, 4, 2, 1, 1, 1, 1, 1];

/// Immutable, validated sections borrow the wire record. Repeated string aliases
/// never turn into repeated allocations or repeated UTF-8 scans.
#[derive(Debug)]
pub struct TerminalProjectionBuffer<'a> {
    route: TerminalFrameRoute,
    geometry: TerminalPixelGeometry,
    scrollback_rows: u64,
    theme_revision: u64,
    foreground: u32,
    background: u32,
    ignored_optional_flags: u8,
    sections: [&'a [u8]; 16],
    strings: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalProjectionCell<'a> {
    pub text: &'a str,
    pub hyperlink: Option<&'a str>,
    pub style: TerminalTextStyle,
    pub width: u8,
    pub semantic: u8,
    pub protected: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalProjectionRow<'p, 'a> {
    projection: &'p TerminalProjectionBuffer<'a>,
    offset: usize,
}

impl<'a> TerminalProjectionBuffer<'a> {
    #[must_use]
    pub const fn route(&self) -> TerminalFrameRoute {
        self.route
    }

    #[must_use]
    pub const fn geometry(&self) -> TerminalPixelGeometry {
        self.geometry
    }

    #[must_use]
    pub const fn scrollback_rows(&self) -> u64 {
        self.scrollback_rows
    }

    #[must_use]
    pub const fn theme_revision(&self) -> u64 {
        self.theme_revision
    }

    #[must_use]
    pub const fn foreground(&self) -> u32 {
        self.foreground
    }

    #[must_use]
    pub const fn background(&self) -> u32 {
        self.background
    }

    #[must_use]
    pub const fn ignored_optional_flags(&self) -> u8 {
        self.ignored_optional_flags
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = TerminalProjectionRow<'_, 'a>> {
        (0..self.sections[1].len()).map(|offset| TerminalProjectionRow {
            projection: self,
            offset,
        })
    }

    fn cell(&self, index: usize) -> TerminalProjectionCell<'a> {
        let range = |offsets, lengths, nullable| {
            string_range(
                self.strings,
                u32_at(self.sections[offsets], index * 4),
                u32::from(u16_at(self.sections[lengths], index * 2)),
                nullable,
            )
            .expect("all immutable string ranges were validated")
        };
        TerminalProjectionCell {
            text: range(3, 4, false).expect("graphemes are not nullable"),
            hyperlink: range(5, 6, true),
            style: TerminalTextStyle {
                foreground: u32_at(self.sections[7], index * 4),
                background: u32_at(self.sections[8], index * 4),
                underline_color: u32_at(self.sections[9], index * 4),
                flags: u16_at(self.sections[10], index * 2),
                underline: self.sections[11][index],
            },
            width: self.sections[12][index],
            semantic: self.sections[13][index],
            protected: self.sections[14][index] != 0,
        }
    }
}

impl<'p, 'a> TerminalProjectionRow<'p, 'a> {
    #[must_use]
    pub fn index(self) -> u16 {
        u16_at(self.projection.sections[0], self.offset * 2)
    }

    #[must_use]
    pub fn flags(self) -> u8 {
        self.projection.sections[1][self.offset]
    }

    #[must_use]
    pub fn prompt_class(self) -> u8 {
        self.projection.sections[2][self.offset]
    }

    pub fn cells(self) -> impl ExactSizeIterator<Item = TerminalProjectionCell<'a>> + Clone + 'p {
        let columns = usize::from(self.projection.geometry.columns);
        let start = self.offset * columns;
        (start..start + columns).map(move |index| self.projection.cell(index))
    }
}

pub fn decode_terminal_projection_buffer(
    bytes: &[u8],
) -> Result<TerminalProjectionBuffer<'_>, ProtocolError> {
    if bytes.len() < HEADER {
        return Err(error(ProtocolErrorCode::Truncated));
    }
    if bytes.len() > MAXIMUM_TERMINAL_FRAME_BYTES {
        return Err(error(ProtocolErrorCode::LimitExceeded));
    }
    if &bytes[..4] != b"OBPB" {
        return Err(error(ProtocolErrorCode::InvalidMagic));
    }
    if bytes[4] != 0 || bytes[5] != 0 || bytes[144] != 2 || u16_at(bytes, 12) != 320 {
        return Err(error(ProtocolErrorCode::UnsupportedVersion));
    }
    if u32_at(bytes, 8) as usize != bytes.len() {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    if bytes[7] & 0x80 != 0
        || bytes[105] & !7 != 0
        || bytes[107] != 0
        || u32_at(bytes, 108) & !0x7ffff != 0
        || bytes[153..160].iter().any(|byte| *byte != 0)
        || bytes[288..320].iter().any(|byte| *byte != 0)
    {
        return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
    }
    let geometry = TerminalPixelGeometry {
        columns: u16_at(bytes, 14),
        rows: u16_at(bytes, 16),
        cell_pixel_width: u16_at(bytes, 100),
        cell_pixel_height: u16_at(bytes, 102),
    };
    crate::geometry::validate_geometry(geometry)?;
    let row_count = usize::from(u16_at(bytes, 18));
    let cell_count = u32_at(bytes, 20) as usize;
    let interaction = u32_at(bytes, 108);
    if row_count > usize::from(geometry.rows)
        || cell_count != row_count * usize::from(geometry.columns)
        || cell_count > MAXIMUM_FRAME_CELLS
        || u16_at(bytes, 96) >= geometry.columns
        || u16_at(bytes, 98) >= geometry.rows
        || !(1..=3).contains(&bytes[104])
        || !(1..=2).contains(&bytes[106])
        || ((interaction >> 4) & 15).count_ones() > 1
        || ((interaction >> 8) & 15).count_ones() > 1
        || u32_at(bytes, 112) > 0xffffff
        || u32_at(bytes, 116) > 0xffffff
        || (u32_at(bytes, 120) > 0xffffff && u32_at(bytes, 120) != 0x1000000)
    {
        return Err(error(ProtocolErrorCode::InvalidField));
    }
    let base = u64_at(bytes, 64);
    let revision = u64_at(bytes, 72);
    let base_revision = (base != u64::MAX).then_some(base);
    match (bytes[6], base_revision) {
        (1, None) if row_count == usize::from(geometry.rows) => {}
        (2, Some(base)) if base < revision => {}
        _ => return Err(error(ProtocolErrorCode::InvalidField)),
    }
    let route = TerminalFrameRoute {
        host_epoch: HostEpoch::from_bytes(bytes[24..40].try_into().expect("fixed header"))
            .map_err(|_| error(ProtocolErrorCode::InvalidIdentifier))?,
        attachment_id: AttachmentId::from_bytes(bytes[40..56].try_into().expect("fixed header"))
            .map_err(|_| error(ProtocolErrorCode::InvalidIdentifier))?,
        sequence: NonZeroU64::new(u64_at(bytes, 56))
            .ok_or_else(|| error(ProtocolErrorCode::InvalidField))?,
        generation: NonZeroU64::new(u64_at(bytes, 80))
            .ok_or_else(|| error(ProtocolErrorCode::InvalidField))?,
        revision,
        base_revision,
    };
    let string_length = u32_at(bytes, 140) as usize;
    let mut ranges = [(0, 0); 16];
    let mut cursor = HEADER;
    for (index, width) in ELEMENT_BYTES.into_iter().enumerate() {
        let start = cursor
            .checked_add(width - 1)
            .ok_or_else(|| error(ProtocolErrorCode::ArithmeticOverflow))?
            & !(width - 1);
        let count = match index {
            0..=2 => row_count,
            15 => string_length,
            _ => cell_count,
        };
        let length = count
            .checked_mul(width)
            .ok_or_else(|| error(ProtocolErrorCode::ArithmeticOverflow))?;
        cursor = start
            .checked_add(length)
            .ok_or_else(|| error(ProtocolErrorCode::ArithmeticOverflow))?;
        ranges[index] = (start, length);
    }
    if cursor != bytes.len() {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    let mut sections = [&[][..]; 16];
    cursor = HEADER;
    for (index, (start, length)) in ranges.into_iter().enumerate() {
        if u32_at(bytes, 160 + index * 8) as usize != start
            || u32_at(bytes, 164 + index * 8) as usize != length
            || bytes[cursor..start].iter().any(|byte| *byte != 0)
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        sections[index] = &bytes[start..start + length];
        cursor = start + length;
    }
    let strings =
        std::str::from_utf8(sections[15]).map_err(|_| error(ProtocolErrorCode::InvalidUtf8))?;
    string_range(strings, u32_at(bytes, 124), u32_at(bytes, 128), true)?;
    string_range(strings, u32_at(bytes, 132), u32_at(bytes, 136), true)?;
    let view = TerminalProjectionBuffer {
        route,
        geometry,
        scrollback_rows: u64_at(bytes, 88),
        theme_revision: u64_at(bytes, 145),
        foreground: u32_at(bytes, 112),
        background: u32_at(bytes, 116),
        ignored_optional_flags: bytes[7] & 0x7f,
        sections,
        strings,
    };
    validate_rows(&view)?;
    Ok(view)
}

fn validate_rows(view: &TerminalProjectionBuffer<'_>) -> Result<(), ProtocolError> {
    let mut prior: Option<(u16, u8, bool)> = None;
    for (offset, row) in view.rows().enumerate() {
        let index = row.index();
        let flags = row.flags();
        if flags & !3 != 0 {
            return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
        }
        if row.prompt_class() > 2
            || index >= view.geometry.rows
            || prior.is_some_and(|(previous, _, _)| index <= previous)
            || (view.route.base_revision.is_none() && usize::from(index) != offset)
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        let adjacent = prior.is_some_and(|(previous, _, _)| previous + 1 == index);
        if adjacent && prior.is_some_and(|(_, previous, _)| (previous & 1 != 0) != (flags & 2 != 0))
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        let columns = usize::from(view.geometry.columns);
        let mut previous_width = None;
        for column in 0..columns {
            let cell = offset * columns + column;
            let width = view.sections[12][cell];
            let style_flags = u16_at(view.sections[10], cell * 2);
            if style_flags & !0xff != 0 {
                return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
            }
            if width > 3
                || view.sections[13][cell] > 2
                || view.sections[14][cell] > 1
                || view.sections[11][cell] > 5
                || [7, 8, 9]
                    .into_iter()
                    .any(|section| !valid_color(u32_at(view.sections[section], cell * 4)))
                || (width == 2 && previous_width != Some(1))
                || (previous_width == Some(1) && width != 2)
                || (width == 3 && (column != columns - 1 || flags & 1 == 0))
                || (column == 0
                    && adjacent
                    && prior.is_some_and(|(_, _, spacer)| spacer)
                    && width != 1)
            {
                return Err(error(ProtocolErrorCode::InvalidField));
            }
            for (offsets, lengths, nullable) in [(3, 4, false), (5, 6, true)] {
                string_range(
                    view.strings,
                    u32_at(view.sections[offsets], cell * 4),
                    u32::from(u16_at(view.sections[lengths], cell * 2)),
                    nullable,
                )?;
            }
            previous_width = Some(width);
        }
        if previous_width == Some(1) {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        prior = Some((index, flags, previous_width == Some(3)));
    }
    Ok(())
}

fn string_range(
    text: &str,
    offset: u32,
    length: u32,
    nullable: bool,
) -> Result<Option<&str>, ProtocolError> {
    if offset == u32::MAX {
        return if nullable && length == 0 {
            Ok(None)
        } else {
            Err(error(ProtocolErrorCode::InvalidField))
        };
    }
    let start = offset as usize;
    let length = length as usize;
    if length > MAXIMUM_TEXT_CELL_BYTES {
        return Err(error(ProtocolErrorCode::LimitExceeded));
    }
    if start > text.len() || length > text.len() - start {
        return Err(error(ProtocolErrorCode::InvalidField));
    }
    text.get(start..start + length)
        .map(Some)
        .ok_or_else(|| error(ProtocolErrorCode::InvalidUtf8))
}

fn valid_color(value: u32) -> bool {
    value <= 0xffffff || value == 0x1000000 || value == 0x2000000
}

// Every caller first validates the fixed header or canonical section extent.
fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated extent"),
    )
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated extent"),
    )
}
fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("validated extent"),
    )
}
fn error(code: ProtocolErrorCode) -> ProtocolError {
    ProtocolError::new(code, "invalid OBPB projection buffer")
}
