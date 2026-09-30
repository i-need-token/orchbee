//! Admission and canonical publication of Windows Agent sidebar presentation.
mod rules;
use super::WindowsSettingsError;
use rules::{RawRule, Rule};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAXIMUM_AGENT_SIDEBAR_PROFILE_BYTES: usize = 32 * 1024;
type Rows = Vec<Vec<WindowsAgentSidebarToken>>;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsAgentSidebarSettings {
    rows: Option<Rows>,
    rows_by_agent: BTreeMap<String, Rows>,
    row_gap: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsSidebarTokenStyle {
    pub fg: Option<[u8; 3]>,
    pub bold: Option<bool>,
    pub dim: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsAgentSidebarToken {
    name: String,
    style: WindowsSidebarTokenStyle,
    rules: Vec<Rule>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowsAgentSidebarTokenKind<'a> {
    StateIcon,
    StateText,
    Machine,
    Workspace,
    Tab,
    Pane,
    Agent,
    Custom(&'a str),
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RawSettings {
    rows: Option<Vec<Vec<RawToken>>>,
    rows_by_agent: BTreeMap<String, Vec<Vec<RawToken>>>,
    row_gap: u16,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum RawToken {
    Plain(String),
    Styled(RawStyled),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStyled {
    token: String,
    fg: Option<String>,
    bold: Option<bool>,
    dim: Option<bool>,
    #[serde(default)]
    rules: Vec<RawRule>,
}

pub(super) fn error(reason: &'static str) -> WindowsSettingsError {
    WindowsSettingsError::new("ui.sidebar.agents", reason)
}
pub(super) fn color(value: Option<String>) -> Result<Option<[u8; 3]>, WindowsSettingsError> {
    value
        .map(|value| {
            let hex = value.strip_prefix('#').ok_or(error("invalid-color"))?;
            if !matches!(hex.len(), 3 | 6) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(error("invalid-color"));
            }
            let hex = if hex.len() == 3 {
                hex.chars().flat_map(|c| [c, c]).collect::<String>()
            } else {
                hex.into()
            };
            let component = |offset| {
                u8::from_str_radix(&hex[offset..offset + 2], 16).map_err(|_| error("invalid-color"))
            };
            Ok([component(0)?, component(2)?, component(4)?])
        })
        .transpose()
}
fn token_kind(name: &str) -> Result<WindowsAgentSidebarTokenKind<'_>, WindowsSettingsError> {
    use WindowsAgentSidebarTokenKind::*;
    Ok(match name {
        "state_icon" => StateIcon,
        "state_text" => StateText,
        "machine" => Machine,
        "workspace" => Workspace,
        "tab" => Tab,
        "pane" => Pane,
        "agent" => Agent,
        "terminal_title" | "terminal_title_stripped" => return Err(error("unsupported-token")),
        name => {
            let key = name.strip_prefix('$').ok_or(error("invalid-token"))?;
            if key.is_empty()
                || key.len() > 32
                || !key
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            {
                return Err(error("invalid-token"));
            }
            Custom(key)
        }
    })
}
impl WindowsAgentSidebarToken {
    fn admit(raw: RawToken) -> Result<Self, WindowsSettingsError> {
        let raw = match raw {
            RawToken::Plain(token) => RawStyled {
                token,
                fg: None,
                bold: None,
                dim: None,
                rules: Vec::new(),
            },
            RawToken::Styled(raw) => raw,
        };
        let token = Self {
            name: raw.token,
            style: WindowsSidebarTokenStyle {
                fg: color(raw.fg)?,
                bold: raw.bold,
                dim: raw.dim,
            },
            rules: raw
                .rules
                .into_iter()
                .map(Rule::admit)
                .collect::<Result<_, _>>()?,
        };
        token.validate()?;
        Ok(token)
    }
    fn validate(&self) -> Result<(), WindowsSettingsError> {
        let kind = token_kind(&self.name)?;
        if self.rules.len() > 16
            || (kind == WindowsAgentSidebarTokenKind::StateIcon && !self.rules.is_empty())
        {
            return Err(error("invalid-rules"));
        }
        for rule in &self.rules {
            rule.validate()?;
        }
        Ok(())
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn kind(&self) -> WindowsAgentSidebarTokenKind<'_> {
        token_kind(&self.name).expect("admitted token")
    }
    /// Rules inspect the original value, before display sanitization. Icons have no text value.
    pub fn style_for_value(&self, value: Option<&str>) -> Option<WindowsSidebarTokenStyle> {
        if let Some(value) = value {
            for rule in &self.rules {
                if rule.matches(value) {
                    return rule.style(self.style);
                }
            }
        }
        Some(self.style)
    }
}
fn rows(raw: Vec<Vec<RawToken>>) -> Result<Rows, WindowsSettingsError> {
    if raw.len() > 16 || raw.iter().any(|r| r.len() > 16) {
        return Err(error("invalid-layout"));
    }
    raw.into_iter()
        .map(|r| r.into_iter().map(WindowsAgentSidebarToken::admit).collect())
        .collect()
}
impl WindowsAgentSidebarSettings {
    pub(super) fn parse(raw: &toml::Value) -> Result<Self, WindowsSettingsError> {
        let raw: RawSettings = raw
            .clone()
            .try_into()
            .map_err(|_| error("invalid-layout"))?;
        let settings = Self {
            rows: raw.rows.map(rows).transpose()?,
            rows_by_agent: raw
                .rows_by_agent
                .into_iter()
                .map(|(id, raw)| Ok((id, rows(raw)?)))
                .collect::<Result<_, WindowsSettingsError>>()?,
            row_gap: raw.row_gap,
        };
        settings.validate()?;
        Ok(settings)
    }
    fn validate(&self) -> Result<(), WindowsSettingsError> {
        if self.rows_by_agent.len() > 64
            || self.rows_by_agent.keys().any(|id| {
                id.is_empty()
                    || id.len() > 64
                    || !id.starts_with(|c: char| c.is_ascii_lowercase())
                    || !id.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_')
                    })
            })
        {
            return Err(error("invalid-agent"));
        }
        for rows in self.rows.iter().chain(self.rows_by_agent.values()) {
            if rows.len() > 16 || rows.iter().any(|r| r.len() > 16) {
                return Err(error("invalid-layout"));
            }
            for token in rows.iter().flatten() {
                token.validate()?;
            }
        }
        if self.profile().len() > MAXIMUM_AGENT_SIDEBAR_PROFILE_BYTES {
            return Err(error("value-too-long"));
        }
        Ok(())
    }
    pub fn row_gap(&self) -> u16 {
        self.row_gap
    }
    /// Physical label takes precedence over its canonical report alias. Presentation text is never an ID.
    pub fn rows_for_agent(
        &self,
        id: &str,
        canonical: Option<&str>,
    ) -> Option<&[Vec<WindowsAgentSidebarToken>]> {
        self.rows_by_agent
            .get(id)
            .or_else(|| canonical.and_then(|id| self.rows_by_agent.get(id)))
            .or(self.rows.as_ref())
            .map(Vec::as_slice)
    }
    pub fn profile(&self) -> String {
        serde_json::to_string(self).expect("finite admitted sidebar settings")
    }
    pub fn from_profile(profile: &str) -> Result<Self, WindowsSettingsError> {
        if profile.len() > MAXIMUM_AGENT_SIDEBAR_PROFILE_BYTES {
            return Err(error("value-too-long"));
        }
        let settings: Self = serde_json::from_str(profile).map_err(|_| error("invalid-layout"))?;
        settings.validate()?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests;
