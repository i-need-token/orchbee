//! The identities File Bar traffic names: the Workspace a request acts in, the Server's call,
//! Client connection and lane numbers on the private link, and the one-time token that admits a
//! started `orchbee-files` process. Each is validated at construction; none grants authority by
//! itself.

use std::{fmt, num::NonZeroU64};

const WORKSPACE_BYTES: usize = 16;
const TOKEN_BYTES: usize = 32;

/// Text that is not the identity's canonical spelling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidIdentity;

impl fmt::Display for InvalidIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid File Bar identity")
    }
}

impl std::error::Error for InvalidIdentity {}

/// A Workspace as the Workbench names it: 32 lowercase hexadecimal digits, never all zero.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkspaceId([u8; WORKSPACE_BYTES]);

impl WorkspaceId {
    /// The Workspace these bytes name, most significant first; `None` for the all-zero value no
    /// Workspace has.
    #[must_use]
    pub fn from_bytes(bytes: [u8; WORKSPACE_BYTES]) -> Option<Self> {
        bytes.iter().any(|byte| *byte != 0).then_some(Self(bytes))
    }

    /// Admits exactly the Workbench spelling: 32 lowercase hexadecimal digits, not all zero.
    ///
    /// # Errors
    ///
    /// [`InvalidIdentity`] for any other text, uppercase digits included.
    pub fn parse(text: &str) -> Result<Self, InvalidIdentity> {
        decode_lowercase_hex::<WORKSPACE_BYTES>(text)
            .and_then(Self::from_bytes)
            .ok_or(InvalidIdentity)
    }

    /// The bytes of this identity, most significant first.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; WORKSPACE_BYTES] {
        &self.0
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_lowercase_hex(&self.0, formatter)
    }
}

macro_rules! link_number {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(NonZeroU64);

        impl $name {
            /// The number `value` names; `None` for zero, which names nothing.
            #[must_use]
            pub const fn new(value: u64) -> Option<Self> {
                match NonZeroU64::new(value) {
                    Some(value) => Some(Self(value)),
                    None => None,
                }
            }

            /// The number's value, never zero.
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0.get()
            }
        }
    };
}

link_number!(
    /// One relayed call on the private link, which the Server numbers and never reuses while the
    /// link lasts.
    CallId
);
link_number!(
    /// One admitted Client connection, which the Server numbers and never reuses while it runs.
    ConnectionId
);
link_number!(
    /// A lane inside one Client connection's multiplex, as that connection numbered it.
    LaneId
);

/// The 256-bit secret a Server hands one `orchbee-files` start, which the started process
/// returns in its hello. Comparing two tokens takes the same time wherever they differ.
#[derive(Clone, Eq)]
pub struct LinkToken([u8; TOKEN_BYTES]);

impl LinkToken {
    /// The token these bytes form.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; TOKEN_BYTES]) -> Self {
        Self(bytes)
    }

    /// Admits exactly 64 lowercase hexadecimal digits, the spelling a start passes on standard
    /// input.
    ///
    /// # Errors
    ///
    /// [`InvalidIdentity`] for any other text.
    pub fn parse(text: &str) -> Result<Self, InvalidIdentity> {
        decode_lowercase_hex::<TOKEN_BYTES>(text)
            .map(Self)
            .ok_or(InvalidIdentity)
    }

    /// The token's bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; TOKEN_BYTES] {
        &self.0
    }

    /// The spelling [`Self::parse`] admits.
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut text = String::with_capacity(TOKEN_BYTES * 2);
        for byte in self.0 {
            text.push(char::from(HEX[usize::from(byte >> 4)]));
            text.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        text
    }
}

impl PartialEq for LinkToken {
    /// Folds every byte before deciding, so the time taken does not reveal where two tokens
    /// first differ.
    fn eq(&self, other: &Self) -> bool {
        let difference = self
            .0
            .iter()
            .zip(other.0.iter())
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            });
        std::hint::black_box(difference) == 0
    }
}

impl fmt::Debug for LinkToken {
    /// Never prints the secret.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LinkToken(..)")
    }
}

const HEX: &[u8; 16] = b"0123456789abcdef";

fn decode_lowercase_hex<const BYTES: usize>(text: &str) -> Option<[u8; BYTES]> {
    let text = text.as_bytes();
    if text.len() != BYTES * 2 {
        return None;
    }
    let digit = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    };
    let mut bytes = [0_u8; BYTES];
    for (byte, pair) in bytes.iter_mut().zip(text.chunks_exact(2)) {
        *byte = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Some(bytes)
}

fn write_lowercase_hex(bytes: &[u8], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}
