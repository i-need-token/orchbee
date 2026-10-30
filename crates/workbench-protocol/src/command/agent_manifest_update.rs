//! Remote update facts remain distinct from activation of the cached rules.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentManifestUpdateFailure {
    InvalidSource,
    InvalidCatalog,
    Fetch,
    TooLarge,
    InvalidUtf8,
    InvalidManifest,
    Cache,
    Busy,
    OlderVersion,
    ChangedSameVersion,
    Cancelled,
    Deadline,
    Clock,
    Process,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentManifestUpdateDisposition {
    Updated,
    Current,
    Failed,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentManifestProviderUpdate {
    pub agent: String,
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub attempted_version: Option<String>,
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub cached_version: Option<String>,
    pub result: AgentManifestUpdateDisposition,
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub error: Option<AgentManifestUpdateFailure>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentManifestUpdateReport {
    pub version: u32,
    pub catalog_url: String,
    pub checked_unix_seconds: u64,
    pub agents: Vec<AgentManifestProviderUpdate>,
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub failure: Option<AgentManifestUpdateFailure>,
    pub status_saved: bool,
}

pub fn valid_agent_manifest_catalog_url(value: &str) -> bool {
    if value.len() > 4096
        || !value.is_ascii()
        || value
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        || value.contains(['\\', '#', '?'])
    {
        return false;
    }
    let Some(tail) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let Some((authority, _)) = tail.split_once('/') else {
        return false;
    };
    !authority.is_empty()
        && authority
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'[' | b']'))
}
impl AgentManifestUpdateReport {
    pub fn successful(&self) -> bool {
        self.failure.is_none()
            && self.status_saved
            && self
                .agents
                .iter()
                .all(|r| r.result != AgentManifestUpdateDisposition::Failed)
    }
    pub fn validate(&self) -> Result<(), crate::ContractError> {
        if self.version != 1
            || !valid_agent_manifest_catalog_url(&self.catalog_url)
            || self.checked_unix_seconds > 9_007_199_254_740_991
            || self.agents.len() > 22
            || self.agents.iter().enumerate().any(|(i, row)| {
                row.agent.is_empty()
                    || row.agent.len() > 64
                    || !row
                        .agent
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                    || self.agents[..i].iter().any(|old| old.agent == row.agent)
                    || [&row.attempted_version, &row.cached_version]
                        .iter()
                        .any(|v| {
                            v.as_ref()
                                .is_some_and(|s| !super::panel_agent::manifest::numeric_version(s))
                        })
                    || match row.result {
                        AgentManifestUpdateDisposition::Failed => row.error.is_none(),
                        AgentManifestUpdateDisposition::Updated
                        | AgentManifestUpdateDisposition::Current => {
                            row.error.is_some()
                                || row.cached_version.is_none()
                                || row.attempted_version != row.cached_version
                        }
                    }
            })
        {
            return Err(crate::ContractError::InvalidDocument);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentManifestMaintenance {
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub source_url: Option<String>,
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub source_error: Option<AgentManifestUpdateFailure>,
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub last_update: Option<AgentManifestUpdateReport>,
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub status_error: Option<AgentManifestUpdateFailure>,
    #[serde(
        default,
        deserialize_with = "present_automatic",
        skip_serializing_if = "Option::is_none"
    )]
    pub automatic: Option<AgentManifestAutomatic>,
}
impl AgentManifestMaintenance {
    pub fn validate(&self) -> Result<(), crate::ContractError> {
        // A configured catalog, a rejected configuration, or no configured catalog at all.
        if !matches!((&self.source_url,self.source_error),(Some(url),None) if valid_agent_manifest_catalog_url(url))
            && !matches!(
                (&self.source_url, self.source_error),
                (None, None | Some(AgentManifestUpdateFailure::InvalidSource))
            )
        {
            return Err(crate::ContractError::InvalidDocument);
        }
        if let Some(automatic) = &self.automatic {
            automatic.validate()?;
        }
        if let Some(report) = &self.last_update {
            report.validate()?;
        }
        Ok(())
    }
}

fn present_automatic<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<AgentManifestAutomatic>, D::Error> {
    AgentManifestAutomatic::deserialize(d).map(Some)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentManifestCheckSource {
    Default,
    File,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentManifestAutomatic {
    pub manifest_check: bool,
    pub manifest_check_source: AgentManifestCheckSource,
    pub background_enabled: bool,
    pub release_build: bool,
    pub update_available: bool,
    pub enabled: bool,
    #[serde(deserialize_with = "super::agent_manifests::nullable")]
    pub last_operation: Option<Box<crate::AgentManifestsOperation>>,
}
impl AgentManifestAutomatic {
    pub fn validate(&self) -> Result<(), crate::ContractError> {
        if self.enabled
            != (self.manifest_check
                && self.background_enabled
                && self.release_build
                && self.update_available)
            || (!self.manifest_check
                && self.manifest_check_source == AgentManifestCheckSource::Default)
            || matches!(
                self.last_operation.as_deref(),
                Some(crate::AgentManifestsOperation::Applied { .. })
            )
        {
            return Err(crate::ContractError::InvalidDocument);
        }
        if let Some(operation) = &self.last_operation {
            operation.validate()?;
        }
        Ok(())
    }
}
