//! Defines bounded Session launch-profile catalog reads with checked selectors, names,
//! revisions, and result sizes. It does not store a catalog, choose a profile, or authorize
//! the executable carried by an entry.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;

use crate::{
    ContractError, ProtocolVersion, SessionId, WorkbenchRequestId,
    WorkbenchSnapshotUnavailableReason, command::LaunchSelection,
};

/// Maximum entries one per-Session Launch Profile catalog retains.
pub const MAXIMUM_LAUNCH_PROFILE_ENTRIES: usize = 64;
/// Maximum serialized bytes the catalog document occupies, escaping
/// included — measured exactly like `ledger_size` measures requests.
pub const MAXIMUM_LAUNCH_PROFILE_CATALOG_BYTES: usize = 256 * 1024;
/// Maximum candidate ids an `ambiguous` resolution publishes.
pub const MAXIMUM_LAUNCH_PROFILE_CANDIDATES: usize = MAXIMUM_LAUNCH_PROFILE_ENTRIES;
/// The dedicated encoded bound for this response variant, enforced on both
/// parse and serialize beneath the outer frame cap — the catalog document
/// plus a small result envelope.
pub const MAXIMUM_ENCODED_LAUNCH_PROFILE_CATALOG_RESULT_BYTES: usize =
    MAXIMUM_LAUNCH_PROFILE_CATALOG_BYTES + 2048;

const MAXIMUM_LAUNCH_PROFILE_ID_BYTES: usize = 128;
const MAXIMUM_LAUNCH_PROFILE_NAME_BYTES: usize = 512;
const MAXIMUM_LAUNCH_PROFILE_SELECTOR_BYTES: usize = MAXIMUM_LAUNCH_PROFILE_NAME_BYTES;

fn valid_profile_id(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_alphanumeric())
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

/// A minted Launch Profile identity. Opaque text in the shared id grammar;
/// the Server mints `<sessionHex>:<counterHex>` so a selector captured under a
/// dead Session can never alias a different Session's profile.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct LaunchProfileId(String);

impl LaunchProfileId {
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.is_empty()
            || value.len() > MAXIMUM_LAUNCH_PROFILE_ID_BYTES
            || !valid_profile_id(value)
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

impl<'de> Deserialize<'de> for LaunchProfileId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

impl fmt::Display for LaunchProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A required, non-unique catalog name: 1..=512 UTF-8 bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct LaunchProfileName(String);

impl LaunchProfileName {
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.is_empty() || value.len() > MAXIMUM_LAUNCH_PROFILE_NAME_BYTES {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for LaunchProfileName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

/// One selector accepted by get/delete/use: an exact profile id first, then
/// a name whose cardinality decides not-found/single/ambiguous. Bounded by
/// the name bound so every admitted selector can reach either rule.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct LaunchProfileSelector(String);

impl LaunchProfileSelector {
    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.is_empty() || value.len() > MAXIMUM_LAUNCH_PROFILE_SELECTOR_BYTES {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for LaunchProfileSelector {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

/// The catalog-private revision: a distinct counter from the Workbench
/// navigation revision, advanced only by committed catalog mutations and
/// serialized as canonical decimal text like `WorkbenchRevision`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LaunchProfileCatalogRevision(u64);

impl LaunchProfileCatalogRevision {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn parse(value: &str) -> Result<Self, ContractError> {
        if value.is_empty()
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(ContractError::InvalidDocument);
        }
        let revision = value
            .parse::<u64>()
            .map_err(|_error| ContractError::InvalidDocument)?;
        Ok(Self(revision))
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for LaunchProfileCatalogRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl Serialize for LaunchProfileCatalogRevision {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for LaunchProfileCatalogRevision {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(de::Error::custom)
    }
}

/// One catalog entry: the minted identity, the required non-unique name, and
/// the validated explicit launch selection stored at create time.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchProfileEntry {
    pub profile_id: LaunchProfileId,
    pub name: LaunchProfileName,
    pub launch: LaunchSelection,
}

/// `workbench.launch-profile-catalog.read` — the dedicated typed read intent
/// for the per-Session ephemeral catalog. `selector` absent lists the whole
/// catalog; present resolves exactly one entry under the same lock.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchLaunchProfileCatalogRead {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<LaunchProfileSelector>,
}

/// `workbench.launch-profile-catalog.result` — `catalog` and `profile` are
/// honest observations of the same Server resolution; `not-found` and
/// `ambiguous` are typed read states, `ambiguous` carrying at most the
/// bounded candidate id list — never exec/argv summaries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LaunchProfileCatalogOutcome {
    Catalog {
        #[serde(rename = "sessionId")]
        session_id: SessionId,
        catalog_revision: LaunchProfileCatalogRevision,
        entries: Vec<LaunchProfileEntry>,
    },
    Profile {
        #[serde(rename = "sessionId")]
        session_id: SessionId,
        catalog_revision: LaunchProfileCatalogRevision,
        profile: LaunchProfileEntry,
    },
    NotFound {},
    Ambiguous {
        candidate_ids: Vec<LaunchProfileId>,
    },
    Denied {},
    Unavailable {
        reason: WorkbenchSnapshotUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchLaunchProfileCatalogResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub outcome: LaunchProfileCatalogOutcome,
}

impl WorkbenchLaunchProfileCatalogResult {
    /// Structural invariants the encoded contract enforces on parse and
    /// serialize alike: entry and candidate bounds are part of the shape.
    pub fn validate(&self) -> Result<(), ContractError> {
        match &self.outcome {
            LaunchProfileCatalogOutcome::Catalog { entries, .. } => {
                if entries.len() > MAXIMUM_LAUNCH_PROFILE_ENTRIES {
                    return Err(ContractError::InvalidDocument);
                }
            }
            LaunchProfileCatalogOutcome::Ambiguous { candidate_ids } => {
                if candidate_ids.len() > MAXIMUM_LAUNCH_PROFILE_CANDIDATES {
                    return Err(ContractError::InvalidDocument);
                }
            }
            LaunchProfileCatalogOutcome::Profile { .. }
            | LaunchProfileCatalogOutcome::NotFound {}
            | LaunchProfileCatalogOutcome::Denied {}
            | LaunchProfileCatalogOutcome::Unavailable { .. } => {}
        }
        Ok(())
    }
}
