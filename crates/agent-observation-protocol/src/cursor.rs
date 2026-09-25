use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de};

/// Exact minted cursor length: `v1.` + base64url(44-byte payload, unpadded, 59
/// characters) + `.` + base64url(32-byte tag, 43 characters).
pub const AGENT_LIST_CURSOR_CHARACTERS: usize = 106;
/// The agreed bound keeps one character of slack over the minted length.
pub const AGENT_LIST_CURSOR_MAXIMUM_CHARACTERS: usize = 107;
pub const AGENT_CURSOR_PAYLOAD_BYTES: usize = 44;
pub const AGENT_CURSOR_TAG_BYTES: usize = 32;

const PAYLOAD_SEGMENT_CHARACTERS: usize = 59;
const TAG_SEGMENT_CHARACTERS: usize = 43;

/// The opaque `v1.<payload>.<tag>` page cursor. Parsing checks only the wire
/// shape — charset and segment lengths — never authenticity; tag verification
/// is the Server's private concern and never enters this crate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct AgentListCursor(String);

impl AgentListCursor {
    pub fn parse(value: &str) -> Result<Self, AgentCursorError> {
        let bytes = value.as_bytes();
        if bytes.len() != AGENT_LIST_CURSOR_CHARACTERS {
            return Err(AgentCursorError::InvalidShape);
        }
        let valid = bytes[..3] == *b"v1."
            && bytes[3..3 + PAYLOAD_SEGMENT_CHARACTERS]
                .iter()
                .all(|byte| is_base64url(*byte))
            && bytes[3 + PAYLOAD_SEGMENT_CHARACTERS] == b'.'
            && bytes[4 + PAYLOAD_SEGMENT_CHARACTERS..]
                .iter()
                .all(|byte| is_base64url(*byte));
        if !valid {
            return Err(AgentCursorError::InvalidShape);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The unpadded base64url payload segment (59 characters, 44 decoded bytes).
    #[must_use]
    pub fn payload_segment(&self) -> &str {
        &self.0[3..3 + PAYLOAD_SEGMENT_CHARACTERS]
    }

    /// The unpadded base64url tag segment (43 characters, 32 decoded bytes).
    #[must_use]
    pub fn tag_segment(&self) -> &str {
        debug_assert_eq!(
            self.0[4 + PAYLOAD_SEGMENT_CHARACTERS..].len(),
            TAG_SEGMENT_CHARACTERS
        );
        &self.0[4 + PAYLOAD_SEGMENT_CHARACTERS..]
    }
}

impl<'de> Deserialize<'de> for AgentListCursor {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

impl fmt::Display for AgentListCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentCursorError {
    InvalidShape,
}

impl fmt::Display for AgentCursorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("cursor must be the minted v1.<59>.<43> base64url shape")
    }
}

impl std::error::Error for AgentCursorError {}

fn is_base64url(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

/// The decoded 44-byte cursor payload, big-endian. Field extraction is
/// verbatim; canonical re-encode equality, tag verification, and every
/// semantic rule (version, operation, flags, reserved zeros, instance match,
/// revision freshness, position) are enforced by the Server's private cursor
/// verifier — this crate only defines the byte layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentCursorPayload {
    pub version: u8,
    /// `1` = list; `2` = bounded changes with a separate MAC domain.
    pub operation: u8,
    /// Bit 0 records `includeEnded`; all other bits are reserved and must be
    /// zero in version 1.
    pub flags: u16,
    /// Random nonzero `u64` minted per canonical runtime instance.
    pub instance: u64,
    /// Reserved target identity; all zero in version 1.
    pub target: u128,
    /// The canonical runtime revision the page was minted against.
    pub revision: u64,
    /// Index into the filtered, canonically ordered session sequence.
    pub position: u64,
}

impl AgentCursorPayload {
    pub const VERSION: u8 = 1;
    pub const LIST_OPERATION: u8 = 1;
    pub const FLAG_INCLUDE_ENDED: u16 = 1;

    #[must_use]
    pub fn encode(&self) -> [u8; AGENT_CURSOR_PAYLOAD_BYTES] {
        let mut bytes = [0_u8; AGENT_CURSOR_PAYLOAD_BYTES];
        bytes[0] = self.version;
        bytes[1] = self.operation;
        bytes[2..4].copy_from_slice(&self.flags.to_be_bytes());
        bytes[4..12].copy_from_slice(&self.instance.to_be_bytes());
        bytes[12..28].copy_from_slice(&self.target.to_be_bytes());
        bytes[28..36].copy_from_slice(&self.revision.to_be_bytes());
        bytes[36..44].copy_from_slice(&self.position.to_be_bytes());
        bytes
    }

    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let bytes: &[u8; AGENT_CURSOR_PAYLOAD_BYTES] = bytes.try_into().ok()?;
        Some(Self {
            version: bytes[0],
            operation: bytes[1],
            flags: u16::from_be_bytes([bytes[2], bytes[3]]),
            instance: u64::from_be_bytes(bytes[4..12].try_into().ok()?),
            target: u128::from_be_bytes(bytes[12..28].try_into().ok()?),
            revision: u64::from_be_bytes(bytes[28..36].try_into().ok()?),
            position: u64::from_be_bytes(bytes[36..44].try_into().ok()?),
        })
    }
}
