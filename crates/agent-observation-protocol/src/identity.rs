use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

const OPAQUE_IDENTITY_MAXIMUM: usize = 128;
const IDENTITY_TEXT_LENGTH: usize = 32;
const ZERO_IDENTITY: &str = "00000000000000000000000000000000";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentIdentityError {
    InvalidFormat,
    ZeroSentinel,
}

impl fmt::Display for AgentIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => {
                formatter.write_str("identity does not match its declared wire shape")
            }
            Self::ZeroSentinel => formatter.write_str("identity must not use the zero sentinel"),
        }
    }
}

impl std::error::Error for AgentIdentityError {}

macro_rules! define_opaque_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, AgentIdentityError> {
                validate_opaque_identity(value)?;
                Ok(Self(value.to_owned()))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
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

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

define_opaque_identity!(AgentSessionId);
impl std::borrow::Borrow<str> for AgentSessionId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
define_opaque_identity!(AgentTurnId);
define_opaque_identity!(AgentAttentionRequestId);
define_opaque_identity!(AgentEventId);
define_opaque_identity!(AgentExecutionLocationId);
define_opaque_identity!(AgentAttachmentGeneration);

macro_rules! define_hex_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, AgentIdentityError> {
                validate_hex_identity(value)?;
                Ok(Self(value.to_owned()))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
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

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

define_hex_identity!(ObapRequestId);
// A Server-minted Agent start run; managed PowerShell echoes it in its command markers and
// exports it to the launched command only.
define_hex_identity!(AgentRunToken);
define_hex_identity!(AgentPresentationId);
define_hex_identity!(AgentHostEpoch);
define_hex_identity!(AgentHostTerminalId);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AgentProviderId(String);

impl AgentProviderId {
    pub fn parse(value: &str) -> Result<Self, AgentIdentityError> {
        let bytes = value.as_bytes();
        let valid = !bytes.is_empty()
            && bytes.len() <= 64
            && bytes.iter().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
            })
            && bytes[0].is_ascii_alphanumeric()
            && bytes[bytes.len() - 1].is_ascii_alphanumeric();
        if !valid {
            return Err(AgentIdentityError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for AgentProviderId {
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

impl fmt::Display for AgentProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The Agent session alias charset: ASCII `[a-z][a-z0-9_-]{0,31}` — the typed
/// constructor is the single admission point for the names sidecar, on the
/// wire and inside the canonical cell alike.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AgentName(String);

impl AgentName {
    pub fn parse(value: &str) -> Result<Self, AgentIdentityError> {
        let bytes = value.as_bytes();
        let valid = !bytes.is_empty()
            && bytes.len() <= 32
            && bytes[0].is_ascii_lowercase()
            && bytes.iter().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            });
        if !valid {
            return Err(AgentIdentityError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for AgentName {
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

impl fmt::Display for AgentName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::borrow::Borrow<str> for AgentName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

fn validate_opaque_identity(value: &str) -> Result<(), AgentIdentityError> {
    let valid = !value.is_empty()
        && value.len() <= OPAQUE_IDENTITY_MAXIMUM
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(AgentIdentityError::InvalidFormat)
    }
}

fn validate_hex_identity(value: &str) -> Result<(), AgentIdentityError> {
    if value.len() != IDENTITY_TEXT_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(AgentIdentityError::InvalidFormat);
    }
    if value == ZERO_IDENTITY {
        return Err(AgentIdentityError::ZeroSentinel);
    }
    Ok(())
}

/// A nonnegative safe integer (`0..=2^53-1`) decoded from a JSON number, shared
/// by revision, generation, sequence, and count fields. Serialization emits
/// the plain JSON number; bounds beyond `u64` or the safe-integer ceiling are
/// rejected at decode and re-checked by contract validation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AgentSafeInteger(u64);

pub const MAXIMUM_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

impl AgentSafeInteger {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl Serialize for AgentSafeInteger {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        serializer.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for AgentSafeInteger {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        deserializer.deserialize_any(SafeIntegerVisitor)
    }
}

struct SafeIntegerVisitor;

impl<'de> serde::de::Visitor<'de> for SafeIntegerVisitor {
    type Value = AgentSafeInteger;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a nonnegative safe integer")
    }

    fn visit_u64<Error>(self, value: u64) -> Result<Self::Value, Error>
    where
        Error: de::Error,
    {
        if value > MAXIMUM_SAFE_INTEGER {
            return Err(Error::custom("integer exceeds the safe-integer bound"));
        }
        Ok(AgentSafeInteger(value))
    }

    fn visit_i64<Error>(self, value: i64) -> Result<Self::Value, Error>
    where
        Error: de::Error,
    {
        if value < 0 {
            return Err(Error::custom("integer must be nonnegative"));
        }
        self.visit_u64(value as u64)
    }

    fn visit_f64<Error>(self, value: f64) -> Result<Self::Value, Error>
    where
        Error: de::Error,
    {
        // The schema governs the decoded numeric value, including JSON 1.0
        // and 1e0 spellings; canonical encoding emits an integer literal.
        if value.fract() != 0.0
            || value < 0.0
            || value > MAXIMUM_SAFE_INTEGER as f64
            || !value.is_finite()
        {
            return Err(Error::custom(
                "number must normalize to a nonnegative safe integer",
            ));
        }
        self.visit_u64(value as u64)
    }
}
