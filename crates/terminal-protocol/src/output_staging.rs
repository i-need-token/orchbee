//! Earlier output staged for the next terminal creation on a broker connection. The Host owns
//! admission and application; this binding only frames the bounded request and its receipt.
use crate::{CreationId, ProtocolError, ProtocolErrorCode};

/// The largest staged output, including any divider the caller adds.
pub const MAXIMUM_STAGED_OUTPUT_BYTES: usize = 1024 * 1024;
pub const OUTPUT_STAGING_HEADER_BYTES: usize = 32;
const REQUEST_MAGIC: &[u8; 8] = b"OBSO\0\x01\0\0";
const RECEIPT_MAGIC: &[u8; 8] = b"OBSS\0\x01\0\0";

/// The Host's receipt: the staged creation and the number of bytes it retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputStaged {
    pub creation_id: CreationId,
    pub bytes: u32,
}

fn invalid() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::InvalidField,
        "invalid staged output payload",
    )
}

fn header(
    magic: &[u8; 8],
    creation_id: CreationId,
    length: usize,
) -> Result<Vec<u8>, ProtocolError> {
    let mut out = Vec::with_capacity(OUTPUT_STAGING_HEADER_BYTES + length);
    out.extend_from_slice(magic);
    out.extend_from_slice(creation_id.as_bytes());
    out.extend_from_slice(&u32::try_from(length).map_err(|_| invalid())?.to_be_bytes());
    out.extend_from_slice(&[0; 4]);
    Ok(out)
}

/// Frames `output` for `creation_id`. The Host separately admits only text, CR, LF and SGR.
pub fn encode_stage_terminal_output(
    creation_id: CreationId,
    output: &[u8],
) -> Result<Vec<u8>, ProtocolError> {
    if output.is_empty() || output.len() > MAXIMUM_STAGED_OUTPUT_BYTES {
        return Err(invalid());
    }
    let mut out = header(REQUEST_MAGIC, creation_id, output.len())?;
    out.extend_from_slice(output);
    Ok(out)
}

pub fn encode_terminal_output_staged(value: OutputStaged) -> Result<Vec<u8>, ProtocolError> {
    let mut out = header(RECEIPT_MAGIC, value.creation_id, 0)?;
    out[24..28].copy_from_slice(&value.bytes.to_be_bytes());
    Ok(out)
}

pub fn decode_terminal_output_staged(bytes: &[u8]) -> Result<OutputStaged, ProtocolError> {
    if bytes.len() != OUTPUT_STAGING_HEADER_BYTES
        || &bytes[..8] != RECEIPT_MAGIC
        || bytes[28..32] != [0; 4]
    {
        return Err(invalid());
    }
    let staged = OutputStaged {
        creation_id: CreationId::from_bytes(bytes[8..24].try_into().map_err(|_| invalid())?)
            .map_err(|_| invalid())?,
        bytes: u32::from_be_bytes(bytes[24..28].try_into().map_err(|_| invalid())?),
    };
    if staged.bytes == 0 || staged.bytes as usize > MAXIMUM_STAGED_OUTPUT_BYTES {
        return Err(invalid());
    }
    Ok(staged)
}
