//! Strictly decodes one OBKG generation 1 Kitty graphics record, shared by live frames and render
//! windows. Owned pixels are copied once for consumers that keep them; this module never resolves
//! a placement against retained resources, composes images, or paints.

use crate::{ProtocolError, ProtocolErrorCode, wire_constants::GRAPHICS};
use std::{collections::HashMap, sync::Arc};

const MAXIMUM_RECORD_BYTES: usize = 33 * 1024 * 1024;
const MAXIMUM_RESOURCE_BYTES: u64 = 32 * 1024 * 1024;
const MAXIMUM_IMAGES: u16 = 256;
const MAXIMUM_PLACEMENTS: u16 = 1024;

/// A complete resource set, or a placement-only replacement naming retained resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalGraphicsUpdateKind {
    Snapshot,
    Placements,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalGraphicsImageFormat {
    Gray,
    GrayAlpha,
    Rgb,
    Rgba,
}

impl TerminalGraphicsImageFormat {
    /// Bytes per pixel in the decoded row-major image.
    #[must_use]
    pub const fn channels(self) -> u8 {
        match self {
            Self::Gray => 1,
            Self::GrayAlpha => 2,
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }
}

/// One Host-owned decoded image. Consumers share the pixels instead of copying them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalGraphicsImage {
    pub id: u32,
    pub format: TerminalGraphicsImageFormat,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
    /// Exactly `width * height * format.channels()` row-major bytes.
    pub pixels: Arc<Vec<u8>>,
}

impl TerminalGraphicsImage {
    /// The Host's image identity: equal identities carry equal pixels.
    #[must_use]
    pub fn same_resource(&self, other: &Self) -> bool {
        self.id == other.id
            && self.format == other.format
            && self.generation == other.generation
            && self.width == other.width
            && self.height == other.height
    }
}

/// One resolved placement: a source crop drawn into a destination pixel rectangle whose origin
/// is `offset` pixels inside the cell at signed `column` and `row`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalGraphicsPlacement {
    pub image_id: u32,
    pub placement_id: u32,
    pub column: i32,
    pub row: i32,
    pub offset_x: u32,
    pub offset_y: u32,
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub destination_width: u32,
    pub destination_height: u32,
    pub z: i32,
    pub internal_id: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalGraphics {
    pub kind: TerminalGraphicsUpdateKind,
    pub resource_revision: u64,
    pub animation_tick: u64,
    /// Empty in a placement-only update.
    pub images: Vec<TerminalGraphicsImage>,
    pub placements: Vec<TerminalGraphicsPlacement>,
}

impl TerminalGraphics {
    /// No image and no placement: nothing can be visible.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.images.is_empty() && self.placements.is_empty()
    }
    #[must_use]
    pub fn image(&self, id: u32) -> Option<&TerminalGraphicsImage> {
        self.images.iter().find(|image| image.id == id)
    }
}

/// Decodes and copies one complete record; every placement of a snapshot references an included
/// image with an in-bounds crop.
pub fn decode_terminal_graphics(bytes: &[u8]) -> Result<TerminalGraphics, ProtocolError> {
    let parsed = parse(bytes)?;
    Ok(TerminalGraphics {
        kind: parsed.kind,
        resource_revision: parsed.resource_revision,
        animation_tick: parsed.animation_tick,
        images: parsed
            .images
            .into_iter()
            .map(|(image, pixels)| TerminalGraphicsImage {
                id: image.id,
                format: image.format,
                generation: image.generation,
                width: image.width,
                height: image.height,
                pixels: Arc::new(pixels.to_vec()),
            })
            .collect(),
        placements: parsed.placements,
    })
}

/// Validates a complete record without copying pixels; returns its image and placement counts.
pub(crate) fn validate_terminal_graphics(
    bytes: &[u8],
) -> Result<(TerminalGraphicsUpdateKind, u16, u16), ProtocolError> {
    let parsed = parse(bytes)?;
    // Both counts were admitted from u16 header fields.
    Ok((
        parsed.kind,
        u16::try_from(parsed.images.len()).map_err(|_| error(ProtocolErrorCode::LimitExceeded))?,
        u16::try_from(parsed.placements.len())
            .map_err(|_| error(ProtocolErrorCode::LimitExceeded))?,
    ))
}

#[derive(Clone, Copy)]
struct ImageHeader {
    id: u32,
    format: TerminalGraphicsImageFormat,
    generation: u64,
    width: u32,
    height: u32,
}

struct Parsed<'a> {
    kind: TerminalGraphicsUpdateKind,
    resource_revision: u64,
    animation_tick: u64,
    images: Vec<(ImageHeader, &'a [u8])>,
    placements: Vec<TerminalGraphicsPlacement>,
}

fn parse(bytes: &[u8]) -> Result<Parsed<'_>, ProtocolError> {
    if bytes.len() < GRAPHICS.header_bytes {
        return Err(error(ProtocolErrorCode::Truncated));
    }
    if bytes.len() > MAXIMUM_RECORD_BYTES {
        return Err(error(ProtocolErrorCode::LimitExceeded));
    }
    if &bytes[..4] != GRAPHICS.magic {
        return Err(error(ProtocolErrorCode::InvalidMagic));
    }
    let mut reader = Reader(&bytes[4..]);
    if reader.take(3)? != GRAPHICS.version {
        return Err(error(ProtocolErrorCode::UnsupportedVersion));
    }
    let kind = reader.u8()?;
    if reader.u32()? as usize != bytes.len() {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    let resource_revision = reader.u64()?;
    // Monotonic Ghostty animation tick: every unsigned value is valid on the wire.
    let animation_tick = reader.u64()?;
    let image_count = reader.u16()?;
    let placement_count = reader.u16()?;
    let declared_bytes = reader.u32()?;
    if reader.u32()? != 0 {
        return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
    }
    if image_count > MAXIMUM_IMAGES
        || placement_count > MAXIMUM_PLACEMENTS
        || u64::from(declared_bytes) > MAXIMUM_RESOURCE_BYTES
    {
        return Err(error(ProtocolErrorCode::LimitExceeded));
    }
    let kind = match kind {
        1 => TerminalGraphicsUpdateKind::Snapshot,
        2 if image_count == 0 => TerminalGraphicsUpdateKind::Placements,
        _ => return Err(error(ProtocolErrorCode::InvalidField)),
    };
    let mut images = Vec::with_capacity(usize::from(image_count));
    let mut dimensions = HashMap::with_capacity(usize::from(image_count));
    let mut actual_bytes = 0_u64;
    for _ in 0..image_count {
        let id = reader.u32()?;
        let format = reader.u8()?;
        let reserved = reader.take(3)?;
        let generation = reader.u64()?;
        let width = reader.u32()?;
        let height = reader.u32()?;
        let data_length = reader.u32()?;
        if reader.u32()? != 0 || reserved.iter().any(|byte| *byte != 0) {
            return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
        }
        let format = match format {
            1 => TerminalGraphicsImageFormat::Gray,
            2 => TerminalGraphicsImageFormat::GrayAlpha,
            3 => TerminalGraphicsImageFormat::Rgb,
            4 => TerminalGraphicsImageFormat::Rgba,
            _ => return Err(error(ProtocolErrorCode::InvalidField)),
        };
        if id == 0
            || generation == 0
            || !(1..=4096).contains(&width)
            || !(1..=4096).contains(&height)
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        let pixels = reader.take(data_length as usize)?;
        if u64::from(width) * u64::from(height) * u64::from(format.channels())
            != u64::from(data_length)
        {
            return Err(error(ProtocolErrorCode::LengthMismatch));
        }
        if dimensions.insert(id, (width, height)).is_some() {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        actual_bytes += u64::from(data_length);
        if actual_bytes > MAXIMUM_RESOURCE_BYTES {
            return Err(error(ProtocolErrorCode::LimitExceeded));
        }
        images.push((
            ImageHeader {
                id,
                format,
                generation,
                width,
                height,
            },
            pixels,
        ));
    }
    if actual_bytes != u64::from(declared_bytes) {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    if image_count != 0 && resource_revision == 0 {
        return Err(error(ProtocolErrorCode::InvalidField));
    }
    let mut placements = Vec::with_capacity(usize::from(placement_count));
    for _ in 0..placement_count {
        let image_id = reader.u32()?;
        // Zero and duplicate placement IDs are legal.
        let placement_id = reader.u32()?;
        let column = reader.i32()?;
        let row = reader.i32()?;
        let offset_x = reader.u32()?;
        let offset_y = reader.u32()?;
        let source_x = reader.u32()?;
        let source_y = reader.u32()?;
        let source_width = reader.u32()?;
        let source_height = reader.u32()?;
        let destination_width = reader.u32()?;
        let destination_height = reader.u32()?;
        let z = reader.i32()?;
        let flags = reader.u32()?;
        if flags & !1 != 0 {
            return Err(error(ProtocolErrorCode::UnknownRequiredFlag));
        }
        if image_id == 0
            || !(1..=4096).contains(&source_width)
            || !(1..=4096).contains(&source_height)
            || !(1..=65535).contains(&destination_width)
            || !(1..=65535).contains(&destination_height)
        {
            return Err(error(ProtocolErrorCode::InvalidField));
        }
        match (kind, dimensions.get(&image_id)) {
            (TerminalGraphicsUpdateKind::Snapshot, None) => {
                return Err(error(ProtocolErrorCode::InvalidField));
            }
            (_, Some((width, height)))
                if u64::from(source_x) + u64::from(source_width) > u64::from(*width)
                    || u64::from(source_y) + u64::from(source_height) > u64::from(*height) =>
            {
                return Err(error(ProtocolErrorCode::InvalidField));
            }
            _ => {}
        }
        placements.push(TerminalGraphicsPlacement {
            image_id,
            placement_id,
            column,
            row,
            offset_x,
            offset_y,
            source_x,
            source_y,
            source_width,
            source_height,
            destination_width,
            destination_height,
            z,
            internal_id: flags & 1 != 0,
        });
    }
    if !reader.0.is_empty() {
        return Err(error(ProtocolErrorCode::LengthMismatch));
    }
    Ok(Parsed {
        kind,
        resource_revision,
        animation_tick,
        images,
        placements,
    })
}

fn error(code: ProtocolErrorCode) -> ProtocolError {
    ProtocolError::new(code, "invalid OBKG graphics record")
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], ProtocolError> {
        let (value, rest) = self
            .0
            .split_at_checked(length)
            .ok_or_else(|| error(ProtocolErrorCode::Truncated))?;
        self.0 = rest;
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
    fn i32(&mut self) -> Result<i32, ProtocolError> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}
