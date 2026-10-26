//! GitHub shorthand and exact managed checkout provenance, independent of display paths.
use super::PluginRegistryError as Error;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginGithubSource {
    pub repository: String,
    pub subdir: Option<String>,
}
impl PluginGithubSource {
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.len() > 1024 {
            return Err(Error::InvalidPath);
        }
        let parts: Vec<_> = value.split('/').collect();
        if parts.len() < 2 || parts.len() > 18 || parts.iter().any(|p| !segment(p)) {
            return Err(Error::InvalidPath);
        }
        Ok(Self {
            repository: parts[..2].join("/"),
            subdir: (parts.len() > 2).then(|| parts[2..].join("/")),
        })
    }
    pub fn display(&self) -> String {
        match &self.subdir {
            Some(subdir) => format!("{}/{subdir}", self.repository),
            None => self.repository.clone(),
        }
    }
    pub fn remote_url(&self) -> String {
        format!("https://github.com/{}.git", self.repository)
    }
    pub fn validate(&self) -> Result<(), Error> {
        if Self::parse(&self.display())? == *self {
            Ok(())
        } else {
            Err(Error::InvalidPath)
        }
    }
}
fn segment(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 120
        || value.starts_with('-')
        || value.ends_with('.')
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return false;
    }
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}
pub fn plugin_git_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && !value.starts_with('-')
        && !value.ends_with('.')
        && !value.contains("..")
        && !value.contains("@{")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginManagedSource {
    pub github: PluginGithubSource,
    pub requested_ref: Option<String>,
    pub resolved_commit: String,
    pub installation_id: String,
}
impl PluginManagedSource {
    pub(super) fn validate(&self) -> Result<(), Error> {
        self.github.validate()?;
        if self
            .requested_ref
            .as_deref()
            .is_some_and(|r| !plugin_git_reference(r))
            || !matches!(self.resolved_commit.len(), 40 | 64)
            || !self
                .resolved_commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || !installation_id(&self.installation_id)
        {
            return Err(Error::InvalidRegistry);
        }
        Ok(())
    }
}
pub(super) fn installation_id(value: &str) -> bool {
    value.len() == 32
        && value.bytes().any(|b| b != b'0')
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
