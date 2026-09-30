//! Admission and canonical publication of Windows Workspace sidebar presentation.
use super::{
    WindowsSettingsError, WindowsSidebarTokenStyle,
    sidebar_tokens::{RawToken, Token, custom_key},
};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES: usize = 32 * 1024;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsSpaceSidebarSettings {
    rows: Vec<Vec<WindowsSpaceSidebarToken>>,
    row_gap: u16,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WindowsSpaceSidebarToken(Token);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsSpaceSidebarTokenKind<'a> {
    StateIcon,
    StateText,
    Workspace,
    Branch,
    GitStatus,
    Custom(&'a str),
}
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RawSettings {
    rows: Option<Vec<Vec<RawToken>>>,
    row_gap: u16,
}
pub(super) fn error(reason: &'static str) -> WindowsSettingsError {
    WindowsSettingsError::new("ui.sidebar.spaces", reason)
}
fn token_kind(name: &str) -> Result<WindowsSpaceSidebarTokenKind<'_>, WindowsSettingsError> {
    use WindowsSpaceSidebarTokenKind::*;
    Ok(match name {
        "state_icon" => StateIcon,
        "state_text" => StateText,
        "workspace" => Workspace,
        "branch" => Branch,
        "git_status" => GitStatus,
        name => Custom(custom_key(name).map_err(error)?),
    })
}
impl WindowsSpaceSidebarToken {
    fn admit(raw: RawToken) -> Result<Self, WindowsSettingsError> {
        let token = Self(Token::admit(raw).map_err(error)?);
        token.validate()?;
        Ok(token)
    }
    fn validate(&self) -> Result<(), WindowsSettingsError> {
        self.0
            .validate(matches!(
                token_kind(self.0.name())?,
                WindowsSpaceSidebarTokenKind::StateIcon | WindowsSpaceSidebarTokenKind::GitStatus
            ))
            .map_err(error)
    }
    pub fn name(&self) -> &str {
        self.0.name()
    }
    pub fn kind(&self) -> WindowsSpaceSidebarTokenKind<'_> {
        token_kind(self.0.name()).expect("admitted token")
    }
    pub fn style_for_value(&self, value: Option<&str>) -> Option<WindowsSidebarTokenStyle> {
        self.0.style_for_value(value)
    }
}
impl Default for WindowsSpaceSidebarSettings {
    fn default() -> Self {
        Self::parse(
            &toml::from_str::<toml::Value>(
                "rows=[['state_icon','workspace'],['branch','git_status']]",
            )
            .expect("default layout"),
        )
        .expect("default tokens")
    }
}
impl WindowsSpaceSidebarSettings {
    pub(super) fn parse(raw: &toml::Value) -> Result<Self, WindowsSettingsError> {
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
                    .map(|row| {
                        row.into_iter()
                            .map(WindowsSpaceSidebarToken::admit)
                            .collect()
                    })
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
    fn validate(&self) -> Result<(), WindowsSettingsError> {
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
    pub fn rows(&self) -> &[Vec<WindowsSpaceSidebarToken>] {
        &self.rows
    }
    pub fn row_gap(&self) -> u16 {
        self.row_gap
    }
    pub fn needs_git(&self) -> bool {
        self.rows.iter().flatten().any(|token| {
            matches!(
                token.kind(),
                WindowsSpaceSidebarTokenKind::Branch | WindowsSpaceSidebarTokenKind::GitStatus
            )
        })
    }
    pub fn profile(&self) -> String {
        serde_json::to_string(self).expect("finite admitted workspace sidebar settings")
    }
    pub fn from_profile(profile: &str) -> Result<Self, WindowsSettingsError> {
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
