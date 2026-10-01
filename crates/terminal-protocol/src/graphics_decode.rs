//! Bounded, attachment-scoped PNG decoding. This operation never changes a terminal.
use crate::{AttachmentId, ProtocolError, ProtocolErrorCode, TerminalId};

pub const MAXIMUM_GRAPHICS_PNG_BYTES: usize = 16 * 1024 * 1024;
pub const MAXIMUM_GRAPHICS_RGBA_BYTES: usize = 32 * 1024 * 1024;
pub const GRAPHICS_DECODE_REQUEST_HEADER_BYTES: usize = 40;
pub const GRAPHICS_DECODE_RESULT_HEADER_BYTES: usize = 56;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphicsDecodeRequest<'a> {
    pub attachment_id: AttachmentId,
    pub width: u32,
    pub height: u32,
    pub png: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphicsDecodeResult<'a> {
    pub attachment_id: AttachmentId,
    pub terminal_id: TerminalId,
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

fn invalid() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::InvalidField,
        "invalid graphics decode payload",
    )
}
fn dimensions(width: u32, height: u32) -> Result<usize, ProtocolError> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(invalid());
    }
    let length = u64::from(width) * u64::from(height) * 4;
    if width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || length > MAXIMUM_GRAPHICS_RGBA_BYTES as u64
    {
        return Err(invalid());
    }
    usize::try_from(length).map_err(|_| invalid())
}
fn word(bytes: &[u8], offset: usize) -> Result<u32, ProtocolError> {
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    ))
}
fn header<'a>(
    bytes: &'a [u8],
    magic: &[u8; 4],
    size: usize,
    maximum: usize,
) -> Result<&'a [u8], ProtocolError> {
    if bytes.len() < size
        || bytes.len() - size > maximum
        || &bytes[..4] != magic
        || bytes[4..8] != [0, 1, 0, 0]
        || bytes[size - 4..size] != [0; 4]
    {
        return Err(invalid());
    }
    let length = usize::try_from(word(bytes, size - 8)?).map_err(|_| invalid())?;
    if length == 0 || length != bytes.len() - size {
        return Err(invalid());
    }
    Ok(&bytes[size..])
}
impl GraphicsDecodeRequest<'_> {
    pub fn validate(&self) -> Result<(), ProtocolError> {
        dimensions(self.width, self.height)?;
        if self.png.len() < 33
            || self.png.len() > MAXIMUM_GRAPHICS_PNG_BYTES
            || &self.png[..8] != b"\x89PNG\r\n\x1a\n"
            || word(self.png, 8)? != 13
            || &self.png[12..16] != b"IHDR"
            || word(self.png, 16)? != self.width
            || word(self.png, 20)? != self.height
        {
            return Err(invalid());
        }
        Ok(())
    }
}
pub fn encode_graphics_decode_request(
    value: &GraphicsDecodeRequest<'_>,
) -> Result<Vec<u8>, ProtocolError> {
    value.validate()?;
    let mut out = Vec::with_capacity(GRAPHICS_DECODE_REQUEST_HEADER_BYTES + value.png.len());
    out.extend_from_slice(b"OBPI\0\x01\0\0");
    out.extend_from_slice(value.attachment_id.as_bytes());
    out.extend_from_slice(&value.width.to_be_bytes());
    out.extend_from_slice(&value.height.to_be_bytes());
    out.extend_from_slice(
        &u32::try_from(value.png.len())
            .map_err(|_| invalid())?
            .to_be_bytes(),
    );
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(value.png);
    Ok(out)
}
pub fn decode_graphics_decode_request(
    bytes: &[u8],
) -> Result<GraphicsDecodeRequest<'_>, ProtocolError> {
    let png = header(
        bytes,
        b"OBPI",
        GRAPHICS_DECODE_REQUEST_HEADER_BYTES,
        MAXIMUM_GRAPHICS_PNG_BYTES,
    )?;
    let value = GraphicsDecodeRequest {
        attachment_id: AttachmentId::from_bytes(bytes[8..24].try_into().map_err(|_| invalid())?)
            .map_err(|_| invalid())?,
        width: word(bytes, 24)?,
        height: word(bytes, 28)?,
        png,
    };
    value.validate()?;
    Ok(value)
}
pub fn encode_graphics_decode_result(
    value: &GraphicsDecodeResult<'_>,
) -> Result<Vec<u8>, ProtocolError> {
    if dimensions(value.width, value.height)? != value.rgba.len() {
        return Err(invalid());
    }
    let mut out = Vec::with_capacity(GRAPHICS_DECODE_RESULT_HEADER_BYTES + value.rgba.len());
    out.extend_from_slice(b"OBPX\0\x01\0\0");
    out.extend_from_slice(value.attachment_id.as_bytes());
    out.extend_from_slice(value.terminal_id.as_bytes());
    out.extend_from_slice(&value.width.to_be_bytes());
    out.extend_from_slice(&value.height.to_be_bytes());
    out.extend_from_slice(
        &u32::try_from(value.rgba.len())
            .map_err(|_| invalid())?
            .to_be_bytes(),
    );
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(value.rgba);
    Ok(out)
}
pub fn decode_graphics_decode_result(
    bytes: &[u8],
) -> Result<GraphicsDecodeResult<'_>, ProtocolError> {
    let rgba = header(
        bytes,
        b"OBPX",
        GRAPHICS_DECODE_RESULT_HEADER_BYTES,
        MAXIMUM_GRAPHICS_RGBA_BYTES,
    )?;
    let value = GraphicsDecodeResult {
        attachment_id: AttachmentId::from_bytes(bytes[8..24].try_into().map_err(|_| invalid())?)
            .map_err(|_| invalid())?,
        terminal_id: TerminalId::from_bytes(bytes[24..40].try_into().map_err(|_| invalid())?)
            .map_err(|_| invalid())?,
        width: word(bytes, 40)?,
        height: word(bytes, 44)?,
        rgba,
    };
    if dimensions(value.width, value.height)? != rgba.len() {
        return Err(invalid());
    }
    Ok(value)
}
