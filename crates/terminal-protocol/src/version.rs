//! Defines the wire version value and the supported Terminal Host protocol generation. A
//! generation is encoded in the major component with minor and patch zero. This module does not
//! negotiate a connection, compare peer cohorts, or select a fallback encoding.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

/// Generation 1, the only generation this binding sends or accepts.
pub const CURRENT_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion {
    major: 1,
    minor: 0,
    patch: 0,
};
