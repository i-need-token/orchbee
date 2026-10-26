//! Report labels and full-width source sequence values; these confer no process authority.

mod provider;
pub use provider::{AgentReportProvider, normalize_agent_report_label};

use crate::AgentIdentityError;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Full-u64 report sequence, serialized as canonical decimal text rather than a JS number.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct AgentReportSequence(u64);
impl AgentReportSequence {
    /// Constructs a sequence with no truncation to the canonical runtime's safe-integer domain.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    /// Parses only canonical decimal text: no sign, whitespace, leading zero or exponent.
    pub fn parse(value: &str) -> Result<Self, AgentIdentityError> {
        if value.is_empty()
            || value.len() > 20
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(AgentIdentityError::InvalidFormat);
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(|_| AgentIdentityError::InvalidFormat)
    }
    /// The original unsigned sequence; ordering and acceptance belong to the state owner.
    pub const fn get(self) -> u64 {
        self.0
    }
}
impl Serialize for AgentReportSequence {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for AgentReportSequence {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}
