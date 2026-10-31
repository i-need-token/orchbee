//! Active-Client topology bridges slow metadata reads without promoting a pending move to authority.
//! The Client state supplies its topology and pending move as a value; this projection never
//! reads that state.
use super::checkpoint::{Checkpoint, ProjectSummary, Row};
use crate::client_context::{ClientContextLocation, ClientContextTarget};
use orchbee_workbench_protocol::{
    MetadataTokens, NavigationConfiguration, WorkbenchRevision, configure_navigation,
};
use std::collections::HashMap;

/// The active Client's canonical topology and its one admitted, reversible sidebar move.
/// Metadata observations and terminal selection remain with their existing owners.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SidebarNavigationProjection {
    pub(crate) snapshot: std::sync::Arc<orchbee_workbench_protocol::Snapshot>,
    pub(crate) pending: Option<NavigationConfiguration>,
}

impl Checkpoint {
    /// Newer observed topology supersedes speculation. Older observations contribute metadata
    /// only by exact identity; Git additionally requires the same canonical directory.
    pub(crate) fn project_navigation(
        source: &SidebarNavigationProjection,
        observed: Option<&Self>,
    ) -> Option<Self> {
        let snapshot = &source.snapshot;
        let observed = observed.filter(|known| known.session_id == snapshot.session_id);
        if let Some(known) = observed
            && (known.revision > snapshot.revision
                || (known.revision == snapshot.revision && source.pending.is_none()))
        {
            return Some(known.clone());
        }
        let pending = source
            .pending
            .as_ref()
            .map(|operation| configure_navigation(&snapshot.navigation, operation))
            .transpose()
            .ok()?;
        let navigation = pending
            .as_ref()
            .and_then(Option::as_ref)
            .unwrap_or(&snapshot.navigation);
        let metadata: HashMap<_, _> = observed
            .into_iter()
            .flat_map(|checkpoint| checkpoint.rows.iter())
            .filter_map(|row| match &row.target.location {
                ClientContextLocation::Workspace { workspace_id, .. } => Some((workspace_id, row)),
                ClientContextLocation::NoWorkspace => None,
            })
            .collect();
        let mut rows = Vec::new();
        for (_, workspace) in navigation.ordered_workspaces().ok()? {
            let target = ClientContextTarget {
                session_id: snapshot.session_id.clone(),
                location: ClientContextLocation::Workspace {
                    workspace_id: workspace.workspace_id.clone(),
                    terminal_context_id: workspace.terminal_context_id.clone(),
                },
            };
            let known = metadata
                .get(&workspace.workspace_id)
                .copied()
                .filter(|row| row.target == target);
            let directory = navigation
                .workspace_directory(&workspace.workspace_id)
                .cloned();
            let git = known
                .filter(|row| row.directory == directory)
                .and_then(|row| row.git.clone());
            rows.push(Row {
                target,
                project_id: workspace.project_id.clone(),
                label: Some(workspace.label.clone()),
                tokens: known.map_or_else(MetadataTokens::default, |row| row.tokens.clone()),
                directory,
                git_stale: git.is_some()
                    && known.is_some_and(|row| {
                        row.git_stale
                            || observed
                                .is_some_and(|checkpoint| checkpoint.revision < snapshot.revision)
                    }),
                git,
            });
        }
        Some(Self {
            session_id: snapshot.session_id.clone(),
            revision: snapshot.revision,
            metadata_revision: observed.map_or(WorkbenchRevision::new(0), |checkpoint| {
                checkpoint.metadata_revision
            }),
            git_requested: observed.is_some_and(|checkpoint| checkpoint.git_requested),
            rows: rows.into(),
            projects: navigation
                .ordered_projects()
                .ok()?
                .into_iter()
                .map(|project| ProjectSummary {
                    project_id: project.project_id.clone(),
                    label: project.label.clone(),
                })
                .collect(),
        })
    }
}
