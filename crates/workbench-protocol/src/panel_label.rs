//! Canonical Presentation overrides and normalized Panel label updates.

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{ContractError, MAXIMUM_LABEL_LENGTH};

/// Stored overrides are nonempty, bounded and already normalized. The creation
/// fallback remains a separate immutable Presentation field.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct PanelCustomLabel(String);

impl PanelCustomLabel {
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.is_empty() || value.trim() != value || value.chars().count() > MAXIMUM_LABEL_LENGTH
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for PanelCustomLabel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Wire input can be omitted/null/blank to clear. Semantic fingerprints retain
/// the normalized update, not differences in surrounding whitespace.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct PanelLabelUpdate(Option<PanelCustomLabel>);

impl PanelLabelUpdate {
    pub fn parse(value: Option<&str>) -> Result<Self, ContractError> {
        let value = value.unwrap_or_default();
        if value.chars().count() > MAXIMUM_LABEL_LENGTH {
            return Err(ContractError::InvalidDocument);
        }
        let trimmed = value.trim();
        if trimmed.is_empty() {
            Ok(Self::default())
        } else {
            Ok(Self(Some(PanelCustomLabel::parse(trimmed)?)))
        }
    }

    #[must_use]
    pub const fn custom_label(&self) -> Option<&PanelCustomLabel> {
        self.0.as_ref()
    }
}

impl<'de> Deserialize<'de> for PanelLabelUpdate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Option::<String>::deserialize(deserializer)?;
        Self::parse(value.as_deref()).map_err(de::Error::custom)
    }
}

// `default` handles an absent stored field; when present it must be a label,
// never null. Keeping one absent spelling preserves legacy snapshot encoding.
pub(crate) fn deserialize_present<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<PanelCustomLabel>, D::Error> {
    PanelCustomLabel::deserialize(deserializer).map(Some)
}
