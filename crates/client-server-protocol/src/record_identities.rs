use crate::ProtocolError;

/// Shared 16-byte identity codec for the dedicated binary record frames
/// (`OBCQ` paste, `OBSR` viewer record, `OBSN` send sequence): every identity
/// is the u128 behind its 32-lowercase-hex text form.
pub(crate) fn identity_bytes<T: Identity>(id: &T) -> Result<[u8; 16], ProtocolError> {
    u128::from_str_radix(id.as_str(), 16)
        .map_err(|_| ProtocolError::InvalidPayload)
        .map(|value| value.to_be_bytes())
}

pub(crate) fn identity<T: Identity>(bytes: Option<&[u8]>) -> Result<T, ProtocolError> {
    let raw: [u8; 16] = bytes
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_| ProtocolError::InvalidPayload)?;
    let value = u128::from_be_bytes(raw);
    T::parse(&format!("{value:032x}"))
}

pub(crate) trait Identity {
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

use crate::{ControllerId, RequestId, SiteId, TerminalId, ViewerId, WorkerId};
identity_adapter!(RequestId);
identity_adapter!(SiteId);
identity_adapter!(WorkerId);
identity_adapter!(TerminalId);
identity_adapter!(ViewerId);
identity_adapter!(ControllerId);
