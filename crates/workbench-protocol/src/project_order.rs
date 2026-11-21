//! Project display order is independent of the canonical identity-sorted registry.
use crate::{ContractError, MAXIMUM_PROJECTS, Navigation, Project};
use std::collections::{HashMap, HashSet};

impl Navigation {
    /// Adds `project` last in the display order. The registry stays in identity order; when the new
    /// identity does not sort last, an implicit order, which is identity order, is written out
    /// first, so the new Project still comes last.
    pub fn add_project(&mut self, project: Project) {
        let at = self
            .projects
            .partition_point(|existing| existing.project_id < project.project_id);
        if at < self.projects.len() && self.project_order.is_empty() {
            self.project_order = self
                .projects
                .iter()
                .map(|existing| existing.project_id.clone())
                .collect();
        }
        if !self.project_order.is_empty() {
            self.project_order.push(project.project_id.clone());
        }
        self.projects.insert(at, project);
    }

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
