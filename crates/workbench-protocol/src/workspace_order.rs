//! Display order is a complete permutation, independent of identity-sorted ownership arrays.
use crate::{
    ContractError, MAXIMUM_TERMINAL_CONTEXTS, Navigation, Project, WorkbenchWorkspaceId, Workspace,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::collections::{HashMap, HashSet};

pub const MAXIMUM_WORKSPACE_ORDER_LENGTH: usize = MAXIMUM_TERMINAL_CONTEXTS;
pub const MAXIMUM_ENCODED_WORKSPACE_ORDER_BYTES: usize = 48 * 1024;

/// A nonempty, bounded, duplicate-free sequence of exact Workspace identities.
/// Membership and before-target admission belong to the authoritative snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WorkbenchWorkspaceList(Vec<WorkbenchWorkspaceId>);

impl WorkbenchWorkspaceList {
    pub fn new(ids: Vec<WorkbenchWorkspaceId>) -> Result<Self, ContractError> {
        if ids.is_empty()
            || ids.len() > MAXIMUM_WORKSPACE_ORDER_LENGTH
            || ids.iter().collect::<HashSet<_>>().len() != ids.len()
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(ids))
    }

    #[must_use]
    pub fn ids(&self) -> &[WorkbenchWorkspaceId] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for WorkbenchWorkspaceList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Vec::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// Insertion boundary in the original global Workspace order, including its end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchWorkspaceInsertIndex(usize);

impl WorkbenchWorkspaceInsertIndex {
    pub fn new(value: usize) -> Result<Self, ContractError> {
        if value > MAXIMUM_WORKSPACE_ORDER_LENGTH {
            return Err(ContractError::InvalidDocument);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn value(self) -> usize {
        self.0
    }
}

impl Serialize for WorkbenchWorkspaceInsertIndex {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0 as u64)
    }
}

impl<'de> Deserialize<'de> for WorkbenchWorkspaceInsertIndex {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = super::command::Binary64SafeInteger::deserialize(deserializer)?;
        Self::new(usize::try_from(value.0).map_err(de::Error::custom)?).map_err(de::Error::custom)
    }
}

impl Navigation {
    /// Resolve display order and optional grouping without using containment as identity.
    /// Empty ordering uses canonical Workspace order. Invalid permutations never fall back.
    pub fn ordered_workspaces(&self) -> Result<Vec<(Option<&Project>, &Workspace)>, ContractError> {
        let members: Vec<_> = self
            .workspaces
            .iter()
            .map(|workspace| {
                let project = workspace
                    .project_id
                    .as_ref()
                    .map(|id| self.project(id).ok_or(ContractError::InvalidDocument))
                    .transpose()?;
                Ok((project, workspace))
            })
            .collect::<Result<_, ContractError>>()?;
        if self.workspace_order.is_empty() {
            return Ok(members);
        }
        if self.workspace_order.len() > MAXIMUM_TERMINAL_CONTEXTS
            || self.workspace_order.len() != members.len()
        {
            return Err(ContractError::InvalidDocument);
        }
        let by_id: HashMap<_, _> = members
            .iter()
            .map(|(project, workspace)| (&workspace.workspace_id, (*project, *workspace)))
            .collect();
        if by_id.len() != members.len() {
            return Err(ContractError::InvalidDocument);
        }
        let mut seen = HashSet::new();
        self.workspace_order
            .iter()
            .map(|id| {
                if !seen.insert(id) {
                    return Err(ContractError::InvalidDocument);
                }
                by_id.get(id).copied().ok_or(ContractError::InvalidDocument)
            })
            .collect()
    }
}
