use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de};

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

define_identity!(RequestId);
define_identity!(SessionId);
define_identity!(WorkerId);
define_identity!(TerminalId);
define_identity!(PresentationId);
define_identity!(PanelId);
define_identity!(TabId);
define_identity!(TerminalContextId);
define_identity!(HostEpoch);
define_identity!(HostTerminalId);
define_identity!(ViewerId);

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

#[cfg(test)]
mod tests;

define_identity!(ControllerId);
