//! The value grammars the records of both machine connections share: names, tokens, machine ids,
//! Session names, paths, ids, counts, times, host keys, the one-shot answer token, and the reset
//! reasons of every channel kind. Each
//! meaning has its own checked type, so a machine id never stands in for a Session name. Whether a
//! value fits the person's catalog or the remote system belongs to the endpoints.

use std::{error::Error, fmt, num::NonZeroU32};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// A value outside its wire grammar or bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidValue;

impl fmt::Display for InvalidValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("value violates its machine connection grammar or bounds")
    }
}

impl Error for InvalidValue {}

/// The largest count or time a record carries, `2^53 - 1`.
pub const MAXIMUM_COUNT: u64 = 9_007_199_254_740_991;

const MAXIMUM_NAME_BYTES: usize = 64;
const MAXIMUM_PATH_BYTES: usize = 4_096;
const MAXIMUM_PROMPT_BYTES: usize = 4_096;
const MAXIMUM_TARGET_BYTES: usize = 1_024;
const MAXIMUM_KEY_FINGERPRINT_BYTES: usize = 128;
const MACHINE_ID_DIGITS: usize = 32;
const ANSWER_TOKEN_BYTES: usize = 16;

/// Whether `text` follows the `OBHS` name grammar.
pub(crate) fn is_name(text: &str) -> bool {
    (1..=MAXIMUM_NAME_BYTES).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_token(text: &str) -> bool {
    (1..=MAXIMUM_NAME_BYTES).contains(&text.len())
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
}

fn is_lowercase_hex(text: &str, digits: usize) -> bool {
    text.len() == digits
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_session_name(text: &str) -> bool {
    (1..=MAXIMUM_NAME_BYTES).contains(&text.len())
        && !text.starts_with('.')
        && !text.ends_with('.')
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_path(text: &str) -> bool {
    (1..=MAXIMUM_PATH_BYTES).contains(&text.len())
        && !text
            .chars()
            .any(|character| character <= '\u{1f}' || character == '\u{7f}')
}

fn is_visible_ascii(text: &str, maximum: usize) -> bool {
    (1..=maximum).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_graphic())
}

macro_rules! text_value {
    ($(#[$documentation:meta])* $name:ident, $check:expr) => {
        $(#[$documentation])*
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Admits `text` when it follows the value's grammar.
            ///
            /// # Errors
            ///
            /// Returns [`InvalidValue`] for text outside the grammar or its byte bounds.
            pub fn new(text: impl Into<String>) -> Result<Self, InvalidValue> {
                let text = text.into();
                let check: fn(&str) -> bool = $check;
                if check(&text) { Ok(Self(text)) } else { Err(InvalidValue) }
            }

            /// The value's text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidValue;

            fn try_from(text: String) -> Result<Self, Self::Error> {
                Self::new(text)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

text_value!(
    /// An `OBHS` name: 1–64 bytes of `a`–`z`, `0`–`9`, and `-`. Unknown record types, channel
    /// kinds, and request names in an `unsupported` reply are names.
    Name,
    is_name
);
text_value!(
    /// A token: 1–64 bytes of `a`–`z`, `0`–`9`, `-`, and `_`, such as a platform or a host key
    /// algorithm.
    Token,
    is_token
);
text_value!(
    /// A machine id: the 32 lowercase hexadecimal digits of a profile `id` in `machines.json`.
    MachineId,
    |text| is_lowercase_hex(text, MACHINE_ID_DIGITS)
);
text_value!(
    /// A Session name: 1–64 bytes of ASCII letters, digits, `.`, `_`, and `-`, neither starting
    /// nor ending in `.`. Whether the remote platform accepts it is the remote helper's check.
    SessionName,
    is_session_name
);
text_value!(
    /// A path: 1–4,096 bytes of UTF-8 without U+0000–U+001F or U+007F. Its meaning depends on the
    /// machine that reported it.
    PathText,
    is_path
);
text_value!(
    /// A login question's text as `ssh` worded it: 1–4,096 bytes of UTF-8. It may span lines; a
    /// client strips control characters before it paints it.
    Prompt,
    |text| (1..=MAXIMUM_PROMPT_BYTES).contains(&text.len())
);
text_value!(
    /// The `target` of a draft Machine's `ssh` connection: 1–1,024 ASCII bytes. The catalog
    /// record's own target rules are the hub's check, which answers `invalid-draft`.
    Target,
    |text| (1..=MAXIMUM_TARGET_BYTES).contains(&text.len()) && text.is_ascii()
);
text_value!(
    /// A host key fingerprint as `ssh` prints it: 1–128 bytes from `!` to `~`.
    KeyFingerprint,
    |text| is_visible_ascii(text, MAXIMUM_KEY_FINGERPRINT_BYTES)
);

impl Name {
    /// The name of a wire literal this crate declares.
    pub(crate) fn literal(text: &'static str) -> Self {
        debug_assert!(is_name(text), "declared wire names follow the name grammar");
        Self(text.to_owned())
    }
}

/// An id: an integer from 1 to 4,294,967,295, such as a request, login, or question id.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct Id(NonZeroU32);

impl Id {
    /// The id 1.
    pub const MIN: Self = Self(NonZeroU32::MIN);

    /// The id `value`, or `None` for 0.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        match NonZeroU32::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// The id's number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl TryFrom<u32> for Id {
    type Error = InvalidValue;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(InvalidValue)
    }
}

impl From<Id> for u32 {
    fn from(id: Id) -> Self {
        id.get()
    }
}

macro_rules! count_value {
    ($(#[$documentation:meta])* $name:ident) => {
        $(#[$documentation])*
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(try_from = "u64", into = "u64")]
        pub struct $name(u64);

        impl $name {
            /// The value `value` when it is at most [`MAXIMUM_COUNT`].
            ///
            /// # Errors
            ///
            /// Returns [`InvalidValue`] above [`MAXIMUM_COUNT`].
            pub const fn new(value: u64) -> Result<Self, InvalidValue> {
                if value <= MAXIMUM_COUNT { Ok(Self(value)) } else { Err(InvalidValue) }
            }

            /// The number.
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl TryFrom<u64> for $name {
            type Error = InvalidValue;

            fn try_from(value: u64) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

count_value!(
    /// A count: an integer from 0 to 9,007,199,254,740,991.
    Count
);
count_value!(
    /// A time: milliseconds since the Unix epoch on the hub's clock, in the count range.
    Time
);

/// A host key: its algorithm token and its fingerprint.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostKey {
    /// The key algorithm, such as `ssh-ed25519`.
    pub algorithm: Token,
    /// The fingerprint `ssh` shows for the key.
    pub fingerprint: KeyFingerprint,
}

/// The one-shot token of a question's answer endpoint: the 16 bytes its 32 lowercase hexadecimal
/// digits spell. It admits one delivery and reveals nothing, and it is never logged: its debug
/// form shows no digit.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct AnswerToken([u8; ANSWER_TOKEN_BYTES]);

impl AnswerToken {
    /// The token of these bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; ANSWER_TOKEN_BYTES]) -> Self {
        Self(bytes)
    }

    /// The token's bytes, as the answer delivery record carries them.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ANSWER_TOKEN_BYTES] {
        &self.0
    }

    fn parse(text: &str) -> Option<Self> {
        fn digit(byte: u8) -> Option<u8> {
            match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                _ => None,
            }
        }
        if text.len() != ANSWER_TOKEN_BYTES * 2 {
            return None;
        }
        let mut bytes = [0; ANSWER_TOKEN_BYTES];
        for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
            let [high, low] = pair else { return None };
            *byte = (digit(*high)? << 4) | digit(*low)?;
        }
        Some(Self(bytes))
    }

    fn spell(&self) -> String {
        use fmt::Write as _;
        let mut text = String::with_capacity(ANSWER_TOKEN_BYTES * 2);
        for byte in self.0 {
            write!(text, "{byte:02x}").expect("writing to a String cannot fail");
        }
        text
    }
}

impl fmt::Debug for AnswerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AnswerToken(..)")
    }
}

impl Serialize for AnswerToken {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.spell())
    }
}

impl<'de> Deserialize<'de> for AnswerToken {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text)
            .ok_or_else(|| de::Error::custom("an answer token is 32 lowercase hexadecimal digits"))
    }
}

/// The JSON literal `true`, for a member whose only value is `true` and which is omitted
/// otherwise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct True;

impl Serialize for True {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for True {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if bool::deserialize(deserializer)? {
            Ok(Self)
        } else {
            Err(de::Error::custom("the member's only value is true"))
        }
    }
}

impl True {
    /// `Some(True)` exactly when `flag` holds.
    pub(crate) fn when(flag: bool) -> Option<Self> {
        flag.then_some(Self)
    }
}

/// Declares an enumeration of wire names with its name table.
macro_rules! name_enum {
    (
        $(#[$documentation:meta])*
        $visibility:vis enum $name:ident {
            $($(#[$variant_documentation:meta])* $variant:ident = $wire:literal),+ $(,)?
        }
    ) => {
        $(#[$documentation])*
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        $visibility enum $name {
            $($(#[$variant_documentation])* $variant),+
        }

        impl $name {
            /// Every value, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The wire name.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            /// The value `name` denotes, if this build knows it.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($wire => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

pub(crate) use name_enum;

name_enum!(
    /// Why a channel was reset; it grows with fallback `unknown`. Each channel kind lists the
    /// shared reasons — `unsupported`, `capacity`, `cancelled`, `stopping`, `unavailable` — and its
    /// own, and reads any other as `unknown`.
    pub enum ResetReason {
        /// The responder does not know the channel kind.
        Unsupported = "unsupported",
        /// The opener already holds 256 open channels.
        Capacity = "capacity",
        /// The opener cancelled.
        Cancelled = "cancelled",
        /// The end is closing.
        Stopping = "stopping",
        /// The responder cannot serve the channel now.
        Unavailable = "unavailable",
        /// Link `session`: the Session's Server is not running.
        SessionNotRunning = "session-not-running",
        /// Link `session` and `ensure-session`: the remote platform refuses the Session name.
        InvalidSession = "invalid-session",
        /// A `route` request packet exceeds its bound.
        TooLarge = "too-large",
        /// Hub: the catalog holds no such Machine.
        NotFound = "not-found",
        /// Hub: the Machine is not `online`.
        NotOnline = "not-online",
        /// Hub: the link was lost before the channel could have caused an effect.
        LinkLost = "link-lost",
        /// A reason this build does not know for the channel.
        Unknown = "unknown",
    }
);

/// The reasons every channel kind of both connections lists.
pub(crate) const SHARED_RESET_REASONS: [ResetReason; 5] = [
    ResetReason::Unsupported,
    ResetReason::Capacity,
    ResetReason::Cancelled,
    ResetReason::Stopping,
    ResetReason::Unavailable,
];

impl ResetReason {
    /// The reason `name` denotes among `known`, `Unknown` otherwise.
    pub(crate) fn read(name: &str, known: &[Self]) -> Self {
        Self::from_name(name)
            .filter(|reason| known.contains(reason))
            .unwrap_or(Self::Unknown)
    }

    /// `self` as a channel that lists `known` reads it.
    pub(crate) fn within(self, known: &[Self]) -> Self {
        if known.contains(&self) {
            self
        } else {
            Self::Unknown
        }
    }
}
