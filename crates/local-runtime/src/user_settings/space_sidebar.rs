//! Admission and canonical publication of Windows Workspace sidebar presentation.
use super::{
    SettingsError, SidebarTokenStyle,
    sidebar_tokens::{RawToken, Token, custom_key},
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES: usize = 32 * 1024;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceSidebarSettings {
    rows: Vec<Vec<SpaceSidebarToken>>,
    row_gap: u16,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpaceSidebarToken(Token);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpaceSidebarTokenKind<'a> {
    StateIcon,
    StateText,
    Workspace,
    Branch,
    /// The Git branch when one is observed, otherwise the Workspace folder.
    Location,
    GitStatus,
    Custom(&'a str),
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawSettings {
    rows: Option<Vec<Vec<RawToken>>>,
    row_gap: u16,
}
pub(super) fn error(reason: &'static str) -> SettingsError {
    SettingsError::new("appearance.sidebar.workspaces", reason)
}
fn token_kind(name: &str) -> Result<SpaceSidebarTokenKind<'_>, SettingsError> {
    use SpaceSidebarTokenKind::*;
    Ok(match name {
        "state_icon" => StateIcon,
        "state_text" => StateText,
        "workspace" => Workspace,
        "branch" => Branch,
        "location" => Location,
        "git_status" => GitStatus,
        name => Custom(custom_key(name).map_err(error)?),
    })
}
impl SpaceSidebarToken {
    fn admit(raw: RawToken) -> Result<Self, SettingsError> {
        let token = Self(Token::admit(raw).map_err(error)?);
        token.validate()?;
        Ok(token)
    }
    fn validate(&self) -> Result<(), SettingsError> {
        self.0
            .validate(matches!(
                token_kind(self.0.name())?,
                SpaceSidebarTokenKind::StateIcon | SpaceSidebarTokenKind::GitStatus
            ))
            .map_err(error)
    }
    pub fn name(&self) -> &str {
        self.0.name()
    }
    pub fn kind(&self) -> SpaceSidebarTokenKind<'_> {
        token_kind(self.0.name()).expect("admitted token")
    }
    pub fn style_for_value(&self, value: Option<&str>) -> Option<SidebarTokenStyle> {
        self.0.style_for_value(value)
    }
}
impl Default for SpaceSidebarSettings {
    fn default() -> Self {
        Self::parse(
            &toml::from_str::<toml::Value>("rows=[['workspace','git_status'],['location']]")
                .expect("default layout"),
        )
        .expect("default tokens")
    }
}
impl SpaceSidebarSettings {
    pub(super) fn parse(raw: &toml::Value) -> Result<Self, SettingsError> {
        let raw: RawSettings = raw
            .clone()
            .try_into()
            .map_err(|_| error("invalid-layout"))?;
        let rows = match raw.rows {
            Some(rows) => {
                if rows.len() > 16 || rows.iter().any(|row| row.len() > 16) {
                    return Err(error("invalid-layout"));
                }
                rows.into_iter()
                    .map(|row| row.into_iter().map(SpaceSidebarToken::admit).collect())
                    .collect::<Result<_, _>>()?
            }
            None => Self::default().rows,
        };
        let settings = Self {
            rows,
            row_gap: raw.row_gap,
        };
        settings.validate()?;
        Ok(settings)
    }
    fn validate(&self) -> Result<(), SettingsError> {
        if self.rows.len() > 16 || self.rows.iter().any(|row| row.len() > 16) {
            return Err(error("invalid-layout"));
        }
        for token in self.rows.iter().flatten() {
            token.validate()?;
        }
        if self.profile().len() > MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES {
            return Err(error("value-too-long"));
        }
        Ok(())
    }
    pub fn rows(&self) -> &[Vec<SpaceSidebarToken>] {
        &self.rows
    }
    pub fn row_gap(&self) -> u16 {
        self.row_gap
    }
    pub fn needs_git(&self) -> bool {
        self.rows.iter().flatten().any(|token| {
            matches!(
                token.kind(),
                SpaceSidebarTokenKind::Branch
                    | SpaceSidebarTokenKind::Location
                    | SpaceSidebarTokenKind::GitStatus
            )
        })
    }
    pub fn profile(&self) -> String {
        serde_json::to_string(self).expect("finite admitted workspace sidebar settings")
    }
    pub fn from_profile(profile: &str) -> Result<Self, SettingsError> {
        if profile.len() > MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES {
            return Err(error("value-too-long"));
        }
        let settings: Self = serde_json::from_str(profile).map_err(|_| error("invalid-layout"))?;
        settings.validate()?;
        Ok(settings)
    }
}
#[cfg(test)]
mod tests;
