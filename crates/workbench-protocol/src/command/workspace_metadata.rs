//! Transient Workspace reports have their own revision and never enter the topology ledger.
use crate::{
    ContractError, ProtocolVersion, SiteId, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchWorkspaceId,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, MapAccess, Visitor},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

pub const MAXIMUM_WORKSPACE_METADATA_READ_ROWS: usize = 8;
pub const MAXIMUM_ENCODED_WORKSPACE_METADATA_RESULT_BYTES: usize = 128 * 1024;
pub const MAXIMUM_METADATA_TOKEN_KEYS: usize = 32;
pub const MAXIMUM_METADATA_PATCH_KEYS: usize = 16;
pub const MAXIMUM_METADATA_TTL_MILLISECONDS: u32 = 86_400_000;

macro_rules! text_type {
    ($name:ident, $valid:expr) => {
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);
        impl $name {
            pub fn parse(value: &str) -> Result<Self, ContractError> {
                if ($valid)(value) {
                    Ok(Self(value.to_owned()))
                } else {
                    Err(ContractError::InvalidDocument)
                }
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl TryFrom<String> for $name {
            type Error = ContractError;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(&value)
            }
        }
        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}
text_type!(MetadataSource, |value: &str| !value.is_empty()
    && value.len() <= 80
    && value
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b":._-".contains(&c)));
text_type!(MetadataTokenKey, |value: &str| !value.is_empty()
    && value.len() <= 32
    && value
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)));
text_type!(MetadataTokenValue, |value: &str| !value.is_empty()
    && value.trim() == value
    && value.chars().count() <= 80
    && !value.chars().any(char::is_control));
impl MetadataTokenValue {
    /// User-input normalization; wire readers require the already-canonical result.
    pub fn normalize(value: &str) -> Option<Self> {
        let value: String = value
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(80)
            .collect();
        let value = value.trim();
        (!value.is_empty()).then(|| Self(value.to_owned()))
    }
}

macro_rules! token_map {
    ($(#[$attr:meta])* $name:ident, $value:ty, $minimum:expr, $maximum:expr) => {
        $(#[$attr])*
        #[derive(Clone, Debug, Eq, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(BTreeMap<MetadataTokenKey, $value>);
        impl $name {
            pub fn new(values: BTreeMap<MetadataTokenKey, $value>) -> Result<Self, ContractError> {
                if ($minimum..=$maximum).contains(&values.len()) {
                    Ok(Self(values))
                } else {
                    Err(ContractError::InvalidDocument)
                }
            }
            pub fn as_map(&self) -> &BTreeMap<MetadataTokenKey, $value> {
                &self.0
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct TokenVisitor;
                impl<'de> Visitor<'de> for TokenVisitor {
                    type Value = $name;
                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str("bounded unique metadata token map")
                    }
                    fn visit_map<A: MapAccess<'de>>(
                        self,
                        mut map: A,
                    ) -> Result<Self::Value, A::Error> {
                        let mut values = BTreeMap::new();
                        while let Some((key, value)) =
                            map.next_entry::<MetadataTokenKey, $value>()?
                        {
                            if values.insert(key, value).is_some() || values.len() > $maximum {
                                return Err(de::Error::custom(
                                    "duplicate or excessive metadata tokens",
                                ));
                            }
                        }
                        $name::new(values).map_err(de::Error::custom)
                    }
                }
                deserializer.deserialize_map(TokenVisitor)
            }
        }
    };
}
token_map!(
    MetadataTokenPatch,
    Option<MetadataTokenValue>,
    1,
    MAXIMUM_METADATA_PATCH_KEYS
);
token_map!(
    #[derive(Default)]
    MetadataTokens,
    MetadataTokenValue,
    0,
    MAXIMUM_METADATA_TOKEN_KEYS
);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceMetadataAction {
    Read {
        workspace_ids: Vec<WorkbenchWorkspaceId>,
    },
    Report {
        workspace_id: WorkbenchWorkspaceId,
        source: MetadataSource,
        tokens: MetadataTokenPatch,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sequence: Option<WorkbenchRevision>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ttl_milliseconds: Option<u32>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceMetadataRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub site_id: SiteId,
    pub action: WorkspaceMetadataAction,
}
impl WorkspaceMetadataRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        let valid = match &self.action {
            WorkspaceMetadataAction::Read { workspace_ids } => {
                workspace_ids.len() <= MAXIMUM_WORKSPACE_METADATA_READ_ROWS
                    && workspace_ids.iter().collect::<BTreeSet<_>>().len() == workspace_ids.len()
            }
            WorkspaceMetadataAction::Report {
                ttl_milliseconds, ..
            } => ttl_milliseconds
                .is_none_or(|ttl| (1..=MAXIMUM_METADATA_TTL_MILLISECONDS).contains(&ttl)),
        };
        if valid {
            Ok(())
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceMetadataRow {
    pub workspace_id: WorkbenchWorkspaceId,
    pub tokens: MetadataTokens,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetadataReportDisposition {
    Applied,
    Stale,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceMetadataFailure {
    InvalidRequest,
    #[serde(rename = "session-mismatch")]
    SiteMismatch,
    NotFound,
    Denied,
    Unavailable,
    TokenLimit,
    SequenceSourceLimit,
    ResourceLimit,
    RevisionExhausted,
    DeadlineOverflow,
    ResultTooLarge,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceMetadataOutcome {
    Read {
        workbench_revision: WorkbenchRevision,
        metadata_revision: WorkbenchRevision,
        workspaces: Vec<WorkspaceMetadataRow>,
    },
    Reported {
        workbench_revision: WorkbenchRevision,
        metadata_revision: WorkbenchRevision,
        workspace_id: WorkbenchWorkspaceId,
        disposition: MetadataReportDisposition,
    },
    Failed {
        reason: WorkspaceMetadataFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceMetadataResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    #[serde(rename = "sessionId")]
    pub site_id: SiteId,
    pub outcome: WorkspaceMetadataOutcome,
}
impl WorkspaceMetadataResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        if let WorkspaceMetadataOutcome::Read { workspaces, .. } = &self.outcome
            && (workspaces.len() > MAXIMUM_WORKSPACE_METADATA_READ_ROWS
                || workspaces
                    .iter()
                    .map(|row| &row.workspace_id)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != workspaces.len())
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
    pub fn answers(&self, request: &WorkspaceMetadataRequest) -> bool {
        self.protocol_version == request.protocol_version
            && self.request_id == request.request_id
            && self.site_id == request.site_id
            && match (&request.action, &self.outcome) {
                (_, WorkspaceMetadataOutcome::Failed { .. }) => true,
                (
                    WorkspaceMetadataAction::Report { workspace_id, .. },
                    WorkspaceMetadataOutcome::Reported {
                        workspace_id: actual,
                        ..
                    },
                ) => workspace_id == actual,
                (
                    WorkspaceMetadataAction::Read { workspace_ids },
                    WorkspaceMetadataOutcome::Read { workspaces, .. },
                ) => workspace_ids
                    .iter()
                    .eq(workspaces.iter().map(|row| &row.workspace_id)),
                _ => false,
            }
    }
}
