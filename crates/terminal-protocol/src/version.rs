//! Defines the semantic version value and the supported Terminal Host protocol version. It
//! does not negotiate a connection, compare peer cohorts, or select a fallback encoding.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

pub const CURRENT_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion {
    major: 0,
    minor: 1,
    patch: 0,
};
