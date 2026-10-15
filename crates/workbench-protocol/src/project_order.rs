//! Project display order is independent of the canonical identity-sorted registry.
use crate::{ContractError, MAXIMUM_PROJECTS, Navigation, Project};
use std::collections::{HashMap, HashSet};

impl Navigation {
    /// Empty ordering retains ID order. Invalid explicit permutations never fall back.
    pub fn ordered_projects(&self) -> Result<Vec<&Project>, ContractError> {
        if self.projects.len() > MAXIMUM_PROJECTS {
            return Err(ContractError::InvalidDocument);
        }
        let by_id: HashMap<_, _> = self
            .projects
            .iter()
            .map(|project| (&project.project_id, project))
            .collect();
        if by_id.len() != self.projects.len() {
            return Err(ContractError::InvalidDocument);
        }
        if self.project_order.is_empty() {
            return Ok(self.projects.iter().collect());
        }
        if self.project_order.len() != self.projects.len() {
            return Err(ContractError::InvalidDocument);
        }
        let mut seen = HashSet::new();
        self.project_order
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
