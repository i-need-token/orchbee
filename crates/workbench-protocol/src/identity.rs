use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

const IDENTITY_TEXT_LENGTH: usize = 32;
const ZERO_IDENTITY: &str = "00000000000000000000000000000000";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    InvalidFormat,
    ZeroSentinel,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => formatter
                .write_str("identity must contain exactly 32 lowercase hexadecimal characters"),
            Self::ZeroSentinel => formatter.write_str("identity must not use the zero sentinel"),
        }
    }
}

impl std::error::Error for IdentityError {}

macro_rules! define_identity {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, IdentityError> {
                validate_identity(value)?;
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

define_identity!(SiteId);
define_identity!(WorkbenchRequestId);
define_identity!(WorkbenchProjectId);
define_identity!(WorkbenchWorkspaceId);
define_identity!(WorkbenchTerminalContextId);
define_identity!(WorkbenchTabGroupId);
define_identity!(WorkbenchTabId);
define_identity!(WorkbenchPanelId);
define_identity!(WorkbenchSplitId);
define_identity!(WorkbenchPanelSlotId);
define_identity!(TerminalPresentationId);
define_identity!(WorkerId);
define_identity!(TerminalId);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WorkbenchRevision(u64);

impl WorkbenchRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn parse(value: &str) -> Result<Self, RevisionError> {
        if value.is_empty()
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(RevisionError);
        }
        let revision = value.parse::<u64>().map_err(|_error| RevisionError)?;
        Ok(Self(revision))
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for WorkbenchRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl Serialize for WorkbenchRevision {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for WorkbenchRevision {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevisionError;

impl fmt::Display for RevisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("revision must be canonical unsigned 64-bit decimal text")
    }
}

impl std::error::Error for RevisionError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolVersion;

impl Serialize for ProtocolVersion {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        serializer.serialize_u8(1)
    }
}

impl<'de> Deserialize<'de> for ProtocolVersion {
    fn deserialize<DeserializerType>(
        deserializer: DeserializerType,
    ) -> Result<Self, DeserializerType::Error>
    where
        DeserializerType: Deserializer<'de>,
    {
        // The schema governs decoded numeric value, including JSON 1.0 and 1e0.
        let value = f64::deserialize(deserializer)?;
        if value == 1.0 {
            Ok(Self)
        } else {
            Err(de::Error::custom("unsupported Workbench protocol version"))
        }
    }
}

fn validate_identity(value: &str) -> Result<(), IdentityError> {
    if value.len() != IDENTITY_TEXT_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(IdentityError::InvalidFormat);
    }
    if value == ZERO_IDENTITY {
        return Err(IdentityError::ZeroSentinel);
    }
    Ok(())
}
