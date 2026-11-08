//! Version and build identity a peer reports: the product version, the source commit and cohort
//! identity, and the environment with its development fingerprint. These serve status, the
//! environment check, and fingerprint comparison; no generation decision reads them.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::{EnvironmentName, InvalidValue};

const MAXIMUM_PRODUCT_VERSION_BYTES: usize = 64;

/// A product version for status and diagnostics: 1–64 bytes from `!` to `~`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProductVersion(String);

impl ProductVersion {
    /// Admits `text` when it is 1–64 visible ASCII bytes.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidValue`] for an empty or overlong text, a space, or a byte outside
    /// visible ASCII.
    pub fn new(text: impl Into<String>) -> Result<Self, InvalidValue> {
        let text = text.into();
        let valid = (1..=MAXIMUM_PRODUCT_VERSION_BYTES).contains(&text.len())
            && text.bytes().all(|byte| byte.is_ascii_graphic());
        if valid {
            Ok(Self(text))
        } else {
            Err(InvalidValue)
        }
    }

    /// The version text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ProductVersion {
    type Error = InvalidValue;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl From<ProductVersion> for String {
    fn from(version: ProductVersion) -> Self {
        version.0
    }
}

impl fmt::Display for ProductVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn decode_lowercase_hex<const BYTES: usize>(text: &str) -> Option<[u8; BYTES]> {
    fn digit(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        }
    }
    if text.len() != BYTES * 2 {
        return None;
    }
    let mut bytes = [0; BYTES];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let [high, low] = pair else { return None };
        *byte = (digit(*high)? << 4) | digit(*low)?;
    }
    Some(bytes)
}

fn write_lowercase_hex(bytes: &[u8], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    bytes
        .iter()
        .try_for_each(|byte| write!(formatter, "{byte:02x}"))
}

macro_rules! hex_identity {
    ($(#[$documentation:meta])* $name:ident, $bytes:literal) => {
        $(#[$documentation])*
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name([u8; $bytes]);

        impl $name {
            /// Wraps raw digest bytes.
            #[must_use]
            pub const fn from_bytes(bytes: [u8; $bytes]) -> Self {
                Self(bytes)
            }

            /// Admits exactly the lowercase hexadecimal encoding.
            ///
            /// # Errors
            ///
            /// Returns [`InvalidValue`] for a wrong length or any other character, including an
            /// uppercase digit.
            pub fn parse(text: &str) -> Result<Self, InvalidValue> {
                decode_lowercase_hex(text).map(Self).ok_or(InvalidValue)
            }

            /// The raw digest bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; $bytes] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_lowercase_hex(&self.0, formatter)
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
                Self::parse(&text).map_err(|_| de::Error::custom(concat!("invalid ", stringify!($name))))
            }
        }
    };
}

hex_identity!(
    /// The 40-hex source commit release packaging passes as `ORCHBEE_SOURCE_COMMIT`.
    SourceCommit,
    20
);
hex_identity!(
    /// The 64-hex identity of one installation's cohort.
    CohortIdentity,
    32
);
hex_identity!(
    /// A development protocol fingerprint, 64 lowercase hex digits on the wire.
    Fingerprint,
    32
);

/// Build identity for diagnostics; both members are optional and only exact pairs compare it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Build {
    /// The source commit, when release packaging supplied one.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub commit: Option<SourceCommit>,
    /// The cohort identity; Terminal Host sends none.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub cohort: Option<CohortIdentity>,
}

/// Deserializes a member that may be omitted but never `null`.
pub(crate) fn present<'de, D, Value>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
    Value: Deserialize<'de>,
{
    Value::deserialize(deserializer).map(Some)
}

/// A build's environment and, exactly outside `orchbee`, its protocol fingerprint for one
/// connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildEnvironment {
    name: EnvironmentName,
    fingerprint: Option<Fingerprint>,
}

impl BuildEnvironment {
    /// The release environment `orchbee`, which negotiates generation ranges.
    #[must_use]
    pub fn release() -> Self {
        Self {
            name: EnvironmentName::release(),
            fingerprint: None,
        }
    }

    /// A development or test environment, which admits only an equal fingerprint.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidValue`] when `name` is `orchbee`.
    pub fn development(
        name: EnvironmentName,
        fingerprint: Fingerprint,
    ) -> Result<Self, InvalidValue> {
        if name.is_release() {
            return Err(InvalidValue);
        }
        Ok(Self {
            name,
            fingerprint: Some(fingerprint),
        })
    }

    /// A build's identity: release for `orchbee`, otherwise `name` with `fingerprint`, the build's
    /// development fingerprint for the connection.
    #[must_use]
    pub fn for_build(name: EnvironmentName, fingerprint: Fingerprint) -> Self {
        let fingerprint = (!name.is_release()).then_some(fingerprint);
        Self { name, fingerprint }
    }

    /// The environment name, compared bytewise with a peer's.
    #[must_use]
    pub fn name(&self) -> &EnvironmentName {
        &self.name
    }

    /// The fingerprint, present exactly when the environment is not `orchbee`.
    #[must_use]
    pub fn fingerprint(&self) -> Option<&Fingerprint> {
        self.fingerprint.as_ref()
    }

    /// Joins decoded members, enforcing the presence rule.
    pub(crate) fn from_wire(
        name: EnvironmentName,
        fingerprint: Option<Fingerprint>,
    ) -> Result<Self, InvalidValue> {
        match fingerprint {
            None if name.is_release() => Ok(Self::release()),
            Some(fingerprint) => Self::development(name, fingerprint),
            None => Err(InvalidValue),
        }
    }
}
