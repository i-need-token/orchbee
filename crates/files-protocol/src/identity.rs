//! The identities File Bar traffic names: the Workspace a request acts in, a mutation's request,
//! a credential prompt, a delete's undo stash, a Local History entry, a Client's transfer, its
//! import, export and picker sessions, its text searches and Quick Open file lists, the Server's
//! call, Client connection and lane numbers on the private link, and the one-time token that admits
//! a started `orchbee-files` process. Each is validated at construction; none grants authority by
//! itself.

use std::{fmt, num::NonZeroU64};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::value::{decode_lowercase_hex, lowercase_hex, parse_canonical_u64};

const WIDE_BYTES: usize = 16;
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

macro_rules! wide_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; WIDE_BYTES]);

        impl $name {
            /// The identity these bytes name, most significant first; `None` for the all-zero
            /// value that names nothing.
            #[must_use]
            pub fn from_bytes(bytes: [u8; WIDE_BYTES]) -> Option<Self> {
                bytes.iter().any(|byte| *byte != 0).then_some(Self(bytes))
            }

            /// Admits exactly 32 lowercase hexadecimal digits, not all zero.
            ///
            /// # Errors
            ///
            /// [`InvalidIdentity`] for any other text, uppercase digits included.
            pub fn parse(text: &str) -> Result<Self, InvalidIdentity> {
                decode_lowercase_hex::<WIDE_BYTES>(text)
                    .and_then(Self::from_bytes)
                    .ok_or(InvalidIdentity)
            }

            /// The bytes of this identity, most significant first.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; WIDE_BYTES] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&lowercase_hex(&self.0))
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                Self::parse(&text).map_err(|_| {
                    de::Error::custom("an identity of 32 lowercase hexadecimal digits, not all zero")
                })
            }
        }
    };
}

wide_identity!(
    /// A Workspace as the Workbench names it.
    WorkspaceId
);
wide_identity!(
    /// A mutation's identity, which the Client chooses; a repeated one returns the recorded
    /// outcome without running again.
    RequestId
);
wide_identity!(
    /// One credential, host-key or editor prompt an operation's Git child raised.
    PromptId
);
wide_identity!(
    /// The undo stash a capturing `fs.delete` filled, which `orchbee-files` chooses and
    /// `fs.restore` names.
    StashId
);
wide_identity!(
    /// One Local History entry, which `orchbee-files` chooses when it records the entry and the
    /// `history.*` operations and `fs.read` name.
    HistoryEntryId
);

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
link_number!(
    /// One read or write transfer, which its Client numbers uniquely in its connection. JSON
    /// records spell it in canonical decimal; Put and Chunk records carry it as a u64.
    TransferId
);
link_number!(
    /// One import, export or picker session, which its Client numbers uniquely in its connection
    /// among the sessions of that kind, so it can send the session's next requests before the
    /// first one answers. JSON records spell it in canonical decimal.
    SessionId
);

link_number!(
    /// One text search, which its Client numbers uniquely in its connection. JSON records spell
    /// it in canonical decimal.
    SearchId
);
link_number!(
    /// One Quick Open file list, which its Client numbers uniquely in its connection, as VS Code's
    /// `cacheKey` names one. JSON records spell it in canonical decimal.
    CacheId
);

/// Implements serialization in canonical decimal for a nonzero number that JSON records carry.
macro_rules! decimal_serde {
    ($name:ident) => {
        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(&self.get())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                parse_canonical_u64(&text)
                    .and_then(Self::new)
                    .ok_or_else(|| de::Error::custom("a nonzero canonical unsigned 64-bit decimal"))
            }
        }
    };
}

decimal_serde!(TransferId);
decimal_serde!(SessionId);
decimal_serde!(SearchId);
decimal_serde!(CacheId);

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
        lowercase_hex(&self.0)
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
