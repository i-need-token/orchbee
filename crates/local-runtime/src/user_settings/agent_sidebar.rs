//! Admission and canonical publication of Windows Agent sidebar presentation.
use super::SettingsError;
use super::sidebar_tokens::{RawToken, Token, custom_key};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAXIMUM_AGENT_SIDEBAR_PROFILE_BYTES: usize = 32 * 1024;
type Rows = Vec<Vec<AgentSidebarToken>>;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSidebarSettings {
    rows: Option<Rows>,
    rows_by_agent: BTreeMap<String, Rows>,
    row_gap: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentSidebarToken(Token);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentSidebarTokenKind<'a> {
    StateIcon,
    StateText,
    Machine,
    Workspace,
    Tab,
    Pane,
    Agent,
    TerminalTitle,
    TerminalTitleStripped,
    Custom(&'a str),
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RawSettings {
    rows: Option<Vec<Vec<RawToken>>>,
    rows_by_agent: BTreeMap<String, Vec<Vec<RawToken>>>,
    row_gap: u16,
}
pub(super) fn error(reason: &'static str) -> SettingsError {
    SettingsError::new("appearance.sidebar.agents", reason)
}
fn token_kind(name: &str) -> Result<AgentSidebarTokenKind<'_>, SettingsError> {
    use AgentSidebarTokenKind::*;
    Ok(match name {
        "state_icon" => StateIcon,
        "state_text" => StateText,
        "machine" => Machine,
        "workspace" => Workspace,
        "tab" => Tab,
        "pane" => Pane,
        "agent" => Agent,
        "terminal_title" => TerminalTitle,
        "terminal_title_stripped" => TerminalTitleStripped,
        name => {
            let key = custom_key(name).map_err(error)?;
            Custom(key)
        }
    })
}
impl AgentSidebarToken {
    fn admit(raw: RawToken) -> Result<Self, SettingsError> {
        let token = Self(Token::admit(raw).map_err(error)?);
        token.validate()?;
        Ok(token)
    }
    fn validate(&self) -> Result<(), SettingsError> {
        let kind = token_kind(self.0.name())?;
        self.0
            .validate(kind == AgentSidebarTokenKind::StateIcon)
            .map_err(error)
    }
    pub fn name(&self) -> &str {
        self.0.name()
    }
    pub fn kind(&self) -> AgentSidebarTokenKind<'_> {
        token_kind(self.0.name()).expect("admitted token")
    }
    pub fn style_for_value(&self, value: Option<&str>) -> Option<super::SidebarTokenStyle> {
        self.0.style_for_value(value)
    }
}

fn rows(raw: Vec<Vec<RawToken>>) -> Result<Rows, SettingsError> {
    if raw.len() > 16 || raw.iter().any(|r| r.len() > 16) {
        return Err(error("invalid-layout"));
    }
    raw.into_iter()
        .map(|r| r.into_iter().map(AgentSidebarToken::admit).collect())
        .collect()
}
impl AgentSidebarSettings {
    pub(super) fn parse(raw: &toml::Value) -> Result<Self, SettingsError> {
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
                .collect::<Result<_, SettingsError>>()?,
            row_gap: raw.row_gap,
        };
        settings.validate()?;
        Ok(settings)
    }
    fn validate(&self) -> Result<(), SettingsError> {
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
    ) -> Option<&[Vec<AgentSidebarToken>]> {
        self.rows_by_agent
            .get(id)
            .or_else(|| canonical.and_then(|id| self.rows_by_agent.get(id)))
            .or(self.rows.as_ref())
            .map(Vec::as_slice)
    }
    pub fn profile(&self) -> String {
        serde_json::to_string(self).expect("finite admitted sidebar settings")
    }
    pub fn from_profile(profile: &str) -> Result<Self, SettingsError> {
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
