//! Canonical first-slot proportion, independent of any Client's cell viewport.
use crate::ContractError;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorkbenchSplitRatio(f64);
impl Eq for WorkbenchSplitRatio {}
impl Default for WorkbenchSplitRatio {
    fn default() -> Self {
        Self::HALF
    }
}
impl WorkbenchSplitRatio {
    pub const HALF: Self = Self(0.5);
    pub fn new(value: f64) -> Result<Self, ContractError> {
        if value.is_finite() && (0.1..=0.9).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
    #[must_use]
    pub fn is_half(&self) -> bool {
        *self == Self::HALF
    }
}
impl Serialize for WorkbenchSplitRatio {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.0)
    }
}
impl<'de> Deserialize<'de> for WorkbenchSplitRatio {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f64::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}
