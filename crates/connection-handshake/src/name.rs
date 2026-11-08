//! Wire names: 1–64 bytes of `a`–`z`, `0`–`9`, and `-`, compared and ordered bytewise. Each
//! meaning has its own type, so a capability name never stands in for an environment.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::InvalidValue;

const MAXIMUM_NAME_BYTES: usize = 64;

/// Whether `text` follows the name grammar.
pub(crate) fn is_name(text: &str) -> bool {
    (1..=MAXIMUM_NAME_BYTES).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

macro_rules! wire_name {
    ($(#[$documentation:meta])* $name:ident) => {
        $(#[$documentation])*
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Admits `text` when it follows the name grammar.
            ///
            /// # Errors
            ///
            /// Returns [`InvalidValue`] for an empty, overlong, or out-of-grammar name.
            pub fn new(text: impl Into<String>) -> Result<Self, InvalidValue> {
                let text = text.into();
                if is_name(&text) { Ok(Self(text)) } else { Err(InvalidValue) }
            }

            /// The name's bytes.
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
            fn from(name: $name) -> Self {
                name.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

wire_name!(
    /// A capability name; `extensions` member names follow the same grammar.
    CapabilityName
);
wire_name!(
    /// The compiled environment identity of a build, such as `orchbee` or `ob-dev-…`.
    EnvironmentName
);
wire_name!(
    /// The name of a connection, frame format, or delivery, such as `client-server`.
    ContractName
);
wire_name!(
    /// The `type` of a control request, including one this build does not know.
    RequestName
);

/// The release environment's name.
pub const RELEASE_ENVIRONMENT: &str = "orchbee";

impl EnvironmentName {
    /// The release environment `orchbee`.
    #[must_use]
    pub fn release() -> Self {
        Self(RELEASE_ENVIRONMENT.to_owned())
    }

    /// Whether this is the release environment `orchbee`.
    #[must_use]
    pub fn is_release(&self) -> bool {
        self.0 == RELEASE_ENVIRONMENT
    }
}

/// Implements serde for a growable enum through its `as_str` and `from_name`: any name in the
/// grammar decodes, and an unknown one becomes the enum's `Unknown`.
macro_rules! growable_serde {
    ($name:ident) => {
        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<Self, D::Error> {
                let name = <String as ::serde::Deserialize>::deserialize(deserializer)?;
                if $crate::name::is_name(&name) {
                    Ok(Self::from_name(&name))
                } else {
                    Err(<D::Error as ::serde::de::Error>::custom(
                        "value violates the name grammar",
                    ))
                }
            }
        }
    };
}

pub(crate) use growable_serde;

/// The wire name of `value` in a growable enum's table, `unknown` when the table lacks it.
pub(crate) fn growable_name<Value: Copy + PartialEq>(
    names: &[(Value, &'static str)],
    value: Value,
) -> &'static str {
    names
        .iter()
        .find_map(|(candidate, name)| (*candidate == value).then_some(*name))
        .unwrap_or("unknown")
}

/// The value `name` denotes in a growable enum's table, `unknown` for any other name.
pub(crate) fn growable_value<Value: Copy>(
    names: &[(Value, &'static str)],
    name: &str,
    unknown: Value,
) -> Value {
    names
        .iter()
        .find_map(|(value, candidate)| (*candidate == name).then_some(*value))
        .unwrap_or(unknown)
}
