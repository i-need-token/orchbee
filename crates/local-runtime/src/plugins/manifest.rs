//! Parses and normalizes the bounded closed plugin manifest, checking identifiers, literal
//! commands, platform lists, and display declarations. It does not load plugin code, resolve
//! an executable, or perform a declared action.

use super::PluginRegistryError as Error;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MANIFEST_LIMIT: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginPlatform {
    Windows,
    Macos,
    Linux,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCommandDefinition {
    #[serde(default)]
    pub platforms: Option<Vec<PluginPlatform>>,
    pub command: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginActionDefinition {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub contexts: Vec<PluginActionContext>,
    #[serde(default)]
    pub platforms: Option<Vec<PluginPlatform>>,
    pub command: Vec<String>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginActionContext {
    Global,
    Workspace,
    Tab,
    #[serde(rename = "panel")]
    Pane,
    Selection,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginEventDefinition {
    pub on: String,
    #[serde(default)]
    pub platforms: Option<Vec<PluginPlatform>>,
    pub command: Vec<String>,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginPanePlacement {
    #[default]
    Overlay,
    Popup,
    Split,
    Tab,
    Zoomed,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PluginPopupSize {
    Cells(u16),
    Percentage(String),
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPaneDefinition {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub platforms: Option<Vec<PluginPlatform>>,
    #[serde(default)]
    pub placement: PluginPanePlacement,
    #[serde(default)]
    pub width: Option<PluginPopupSize>,
    #[serde(default)]
    pub height: Option<PluginPopupSize>,
    pub command: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginLinkHandler {
    pub id: String,
    pub title: String,
    pub pattern: String,
    pub action: String,
    #[serde(default)]
    pub platforms: Option<Vec<PluginPlatform>>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub min_orchbee_version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub platforms: Option<Vec<PluginPlatform>>,
    #[serde(default)]
    pub build: Vec<PluginCommandDefinition>,
    #[serde(default)]
    pub startup: Vec<PluginCommandDefinition>,
    #[serde(default)]
    pub actions: Vec<PluginActionDefinition>,
    #[serde(default)]
    pub events: Vec<PluginEventDefinition>,
    #[serde(default, rename = "panels")]
    pub panes: Vec<PluginPaneDefinition>,
    #[serde(default)]
    pub link_handlers: Vec<PluginLinkHandler>,
}
pub fn plugin_identifier(value: &str) -> Result<String, Error> {
    identifier(value, true)
}
fn identifier(value: &str, dots: bool) -> Result<String, Error> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 120
        || !value.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b':' | b'_' | b'-') || (dots && b == b'.')
        })
    {
        return Err(Error::InvalidIdentifier);
    }
    Ok(value.into())
}
fn text(value: &mut String, maximum: usize) -> Result<(), Error> {
    *value = value.trim().to_owned();
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(Error::InvalidManifest);
    }
    Ok(())
}
fn optional(value: &mut Option<String>) -> Result<(), Error> {
    if let Some(value) = value {
        text(value, 4096)?;
    }
    Ok(())
}
fn platforms(value: &mut Option<Vec<PluginPlatform>>) -> Result<(), Error> {
    if let Some(value) = value {
        if value.is_empty() || value.len() > 3 {
            return Err(Error::InvalidManifest);
        }
        value.sort();
        value.dedup();
    }
    Ok(())
}
fn command(argv: &[String]) -> Result<(), Error> {
    if argv.is_empty()
        || argv.len() > 128
        || argv
            .iter()
            .any(|v| v.is_empty() || v.len() > 4096 || v.contains('\0'))
    {
        return Err(Error::InvalidManifest);
    }
    Ok(())
}
fn size(value: &Option<PluginPopupSize>) -> Result<(), Error> {
    match value {
        None => Ok(()),
        Some(PluginPopupSize::Cells(1..=4096)) => Ok(()),
        Some(PluginPopupSize::Percentage(value))
            if value
                .strip_suffix('%')
                .and_then(|v| v.parse::<u8>().ok())
                .is_some_and(|n| (1..=100).contains(&n) && *value == format!("{n}%")) =>
        {
            Ok(())
        }
        _ => Err(Error::InvalidManifest),
    }
}
impl PluginManifest {
    pub fn parse(source: &str) -> Result<Self, Error> {
        if source.len() > MANIFEST_LIMIT {
            return Err(Error::LimitExceeded);
        }
        let mut manifest: Self = toml::from_str(source).map_err(|_| Error::InvalidManifest)?;
        manifest.normalize()?;
        Ok(manifest)
    }
    pub fn supports_windows(&self, item: &Option<Vec<PluginPlatform>>) -> bool {
        item.as_ref()
            .or(self.platforms.as_ref())
            .is_none_or(|v| v.contains(&PluginPlatform::Windows))
    }
    pub(super) fn normalize(&mut self) -> Result<(), Error> {
        self.id = plugin_identifier(&self.id)?;
        text(&mut self.name, 256)?;
        text(&mut self.version, 120)?;
        optional(&mut self.description)?;
        if let Some(required) = &mut self.min_orchbee_version {
            text(required, 120)?;
            let numbers = required
                .split('.')
                .map(str::parse::<u64>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| Error::InvalidManifest)?;
            if numbers.len() != 3 || numbers.as_slice() > [0, 1, 0].as_slice() {
                return Err(Error::InvalidManifest);
            }
        }
        platforms(&mut self.platforms)?;
        if [
            self.build.len(),
            self.startup.len(),
            self.actions.len(),
            self.events.len(),
            self.panes.len(),
            self.link_handlers.len(),
        ]
        .iter()
        .any(|n| *n > 64)
        {
            return Err(Error::LimitExceeded);
        }
        for entry in self.build.iter_mut().chain(&mut self.startup) {
            platforms(&mut entry.platforms)?;
            command(&entry.command)?;
        }
        let mut ids = BTreeSet::new();
        for action in &mut self.actions {
            action.id = identifier(&action.id, false)?;
            if !ids.insert(action.id.clone()) || action.contexts.len() > 5 {
                return Err(Error::InvalidManifest);
            }
            text(&mut action.title, 256)?;
            optional(&mut action.description)?;
            platforms(&mut action.platforms)?;
            command(&action.command)?;
        }
        self.actions.sort_by(|a, b| a.id.cmp(&b.id));
        for event in &mut self.events {
            text(&mut event.on, 120)?;
            platforms(&mut event.platforms)?;
            command(&event.command)?;
        }
        let mut pane_ids = BTreeSet::new();
        for pane in &mut self.panes {
            pane.id = identifier(&pane.id, false)?;
            if !pane_ids.insert(pane.id.clone()) {
                return Err(Error::InvalidManifest);
            }
            text(&mut pane.title, 256)?;
            optional(&mut pane.description)?;
            platforms(&mut pane.platforms)?;
            command(&pane.command)?;
            size(&pane.width)?;
            size(&pane.height)?;
            if pane.placement != PluginPanePlacement::Popup
                && (pane.width.is_some() || pane.height.is_some())
            {
                return Err(Error::InvalidManifest);
            }
        }
        self.panes.sort_by(|a, b| a.id.cmp(&b.id));
        let mut handlers = BTreeSet::new();
        for handler in &mut self.link_handlers {
            handler.id = identifier(&handler.id, false)?;
            handler.action = identifier(&handler.action, false)?;
            if !handlers.insert(handler.id.clone()) || !ids.contains(&handler.action) {
                return Err(Error::InvalidManifest);
            }
            text(&mut handler.title, 256)?;
            text(&mut handler.pattern, 4096)?;
            platforms(&mut handler.platforms)?;
        }
        if serde_json::to_vec(self)
            .map_err(|_| Error::InvalidManifest)?
            .len()
            > MANIFEST_LIMIT
        {
            return Err(Error::LimitExceeded);
        }
        Ok(())
    }
}
