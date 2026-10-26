//! Provenance for the active compiled rules; updater state belongs to its actual owner.
use super::nullable;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentManifestSource {
    Bundled,
    LocalOverride,
    RemoteCache,
}
impl AgentManifestSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::LocalOverride => "local-override",
            Self::RemoteCache => "remote-cache",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentManifestWarningKind {
    Unreadable,
    NotRegular,
    TooLarge,
    InvalidUtf8,
    InvalidManifest,
    InvalidVersion,
    OlderThanBundled,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentManifestWarning {
    pub source: AgentManifestSource,
    pub kind: AgentManifestWarningKind,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentDetectionManifest {
    pub source: AgentManifestSource,
    #[serde(deserialize_with = "nullable")]
    pub path: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub cached_remote_version: Option<String>,
    pub local_override_shadowing_remote: bool,
    pub warnings: Vec<AgentManifestWarning>,
}
impl AgentDetectionManifest {
    pub(crate) fn valid(&self) -> bool {
        (match self.source {
            AgentManifestSource::Bundled => self.path.is_none(),
            AgentManifestSource::LocalOverride | AgentManifestSource::RemoteCache => {
                self.path.as_ref().is_some_and(|path| {
                    !path.is_empty() && path.len() <= 32768 && !path.chars().any(char::is_control)
                })
            }
        }) && self
            .cached_remote_version
            .as_ref()
            .is_none_or(|v| numeric_version(v))
            && (self.source != AgentManifestSource::RemoteCache
                || self.cached_remote_version.is_some())
            && (!self.local_override_shadowing_remote || self.cached_remote_version.is_some())
            && self.warnings.len() <= 2
            && self.warnings.iter().enumerate().all(|(i, w)| {
                w.source != AgentManifestSource::Bundled
                    && !self.warnings[..i]
                        .iter()
                        .any(|earlier| earlier.source == w.source)
            })
    }
}
pub(crate) fn numeric_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|b| b.is_ascii_digit())
                && part.parse::<u64>().is_ok()
        })
}
