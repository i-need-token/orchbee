use crate::{
    ClientMessage, ControllerId, MAXIMUM_PASTE_CONTENT_BYTES, PasteContent, ProtocolError,
    ReadMessageError, RequestId, TerminalControlAction, TerminalControlRequest, ViewerId,
};
use std::io::Read;
use std::num::NonZeroU64;

pub const PASTE_RECORD_MAGIC: [u8; 4] = *b"OBCQ";

const HEADER: usize = 12;
const PREFIX: [u8; 8] = *b"OBCQ\0\x01\0\0";
const BODY_PREFIX: usize = 56;
const MAXIMUM_BODY: usize = BODY_PREFIX + MAXIMUM_PASTE_CONTENT_BYTES;

pub(crate) fn encode(request: &TerminalControlRequest) -> Result<Vec<u8>, ProtocolError> {
    let TerminalControlAction::Paste {
        attachment_id,
        controller_id,
        sequence,
        content,
    } = request.action()
    else {
        return Err(ProtocolError::InvalidPayload);
    };
    let length = BODY_PREFIX + content.as_bytes().len();
    let mut bytes = Vec::with_capacity(HEADER + length);
    bytes.extend_from_slice(&PREFIX);
    bytes.extend_from_slice(
        &u32::try_from(length)
            .map_err(|_| ProtocolError::InvalidPayload)?
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&identity_bytes(request.request_id())?);
    bytes.extend_from_slice(&identity_bytes(attachment_id)?);
    bytes.extend_from_slice(&identity_bytes(controller_id)?);
    bytes.extend_from_slice(&sequence.get().to_be_bytes());
    bytes.extend_from_slice(content.as_bytes());
    Ok(bytes)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<ClientMessage, ProtocolError> {
    let header = bytes.get(..HEADER).ok_or(ProtocolError::InvalidPayload)?;
    let length = body_length(header)?;
    if bytes.len() != HEADER + length {
        return Err(ProtocolError::InvalidPayload);
    }
    decode_body(&bytes[HEADER..])
}

pub(crate) fn read(
    reader: &mut impl Read,
    prefix: [u8; 8],
) -> Result<ClientMessage, ReadMessageError> {
    let mut header = [0; HEADER];
    header[..8].copy_from_slice(&prefix);
    reader.read_exact(&mut header[8..])?;
    let length = body_length(&header)?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    decode_body(&body).map_err(Into::into)
}

fn body_length(header: &[u8]) -> Result<usize, ProtocolError> {
    if header[..4] != PREFIX[..4] {
        return Err(ProtocolError::InvalidMagic);
    }
    let version = u16::from_be_bytes([header[4], header[5]]);
    if version != 1 {
        return Err(ProtocolError::IncompatibleFrameVersion { received: version });
    }
    if header[6..8] != [0, 0] {
        return Err(ProtocolError::InvalidPayload);
    }
    let length = usize::try_from(u32::from_be_bytes(
        header[8..12]
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))
    .map_err(|_| ProtocolError::InvalidPayload)?;
    if length <= BODY_PREFIX {
        return Err(ProtocolError::InvalidPayload);
    }
    if length > MAXIMUM_BODY {
        return Err(ProtocolError::PayloadTooLarge {
            actual: length,
            maximum: MAXIMUM_BODY,
        });
    }
    Ok(length)
}

fn decode_body(body: &[u8]) -> Result<ClientMessage, ProtocolError> {
    let request_id = identity::<RequestId>(body.get(..16))?;
    let attachment_id = identity::<ViewerId>(body.get(16..32))?;
    let controller_id = identity::<ControllerId>(body.get(32..48))?;
    let sequence = NonZeroU64::new(u64::from_be_bytes(
        body.get(48..56)
            .ok_or(ProtocolError::InvalidPayload)?
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))
    .ok_or(ProtocolError::InvalidPayload)?;
    let content = PasteContent::new(
        body.get(BODY_PREFIX..)
            .ok_or(ProtocolError::InvalidPayload)?
            .to_vec(),
    )?;
    Ok(ClientMessage::TerminalControl(TerminalControlRequest::new(
        request_id,
        TerminalControlAction::Paste {
            attachment_id,
            controller_id,
            sequence,
            content,
        },
    )))
}

fn identity_bytes<T: Identity>(id: &T) -> Result<[u8; 16], ProtocolError> {
    u128::from_str_radix(id.as_str(), 16)
        .map_err(|_| ProtocolError::InvalidPayload)
        .map(|value| value.to_be_bytes())
}

trait Identity {
    fn as_str(&self) -> &str;
    fn parse(value: &str) -> Result<Self, ProtocolError>
    where
        Self: Sized;
}
macro_rules! identity_adapter {
    ($name:ident) => {
        impl Identity for $name {
            fn as_str(&self) -> &str {
                $name::as_str(self)
            }
            fn parse(value: &str) -> Result<Self, ProtocolError> {
                $name::parse(value).map_err(|_| ProtocolError::InvalidPayload)
            }
        }
    };
}
identity_adapter!(RequestId);
identity_adapter!(ViewerId);
identity_adapter!(ControllerId);

fn identity<T: Identity>(bytes: Option<&[u8]>) -> Result<T, ProtocolError> {
    let raw: [u8; 16] = bytes
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_| ProtocolError::InvalidPayload)?;
    let value = u128::from_be_bytes(raw);
    T::parse(&format!("{value:032x}"))
}
