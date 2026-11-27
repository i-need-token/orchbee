//! The scope each Workspace has on one link, as the Server last stated it there. A scope update
//! precedes the first call that names a Workspace and every call after its scope changed; a
//! Workspace the Session no longer has is stated closed, so the files its connections opened stay
//! reachable, and one the link never heard of is refused.

use orchbee_files_protocol::{ScopeUpdate, WorkspaceId, WorkspaceScope};
use std::collections::HashMap;

/// How many closed Workspaces one link remembers; beyond it the oldest statements are forgotten,
/// and a call naming one of them is refused as a Workspace the link never heard of.
const MAXIMUM_CLOSED: usize = 1024;

/// What the Server stated on one link.
#[derive(Debug, Default)]
pub(super) struct Stated {
    workspaces: HashMap<WorkspaceId, WorkspaceScope>,
    /// Closed Workspaces in the order they closed.
    closed: Vec<WorkspaceId>,
}

/// A call names a Workspace that is not the Session's and that the link never heard of.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct Unknown;

impl Stated {
    /// The update that must precede a call naming `workspace`, whose scope is now `current`,
    /// `None` when the Session no longer has it.
    pub(super) fn before_call(
        &mut self,
        workspace: WorkspaceId,
        current: Option<WorkspaceScope>,
    ) -> Result<Option<ScopeUpdate>, Unknown> {
        let scope = match current {
            Some(scope) => scope,
            None if self.workspaces.contains_key(&workspace) => WorkspaceScope::Closed,
            None => return Err(Unknown),
        };
        Ok(self.state(workspace, scope))
    }

    /// The Workspaces this link heard of that are still open; a closed one never reopens.
    pub(super) fn workspaces(&self) -> Vec<WorkspaceId> {
        self.workspaces
            .iter()
            .filter(|(_, scope)| **scope != WorkspaceScope::Closed)
            .map(|(workspace, _)| *workspace)
            .collect()
    }

    /// The updates that bring the stated Workspaces to `current`, given in the order of
    /// [`Self::workspaces`]; `None` names a Workspace the Session no longer has.
    pub(super) fn changes(
        &mut self,
        workspaces: &[WorkspaceId],
        current: Vec<Option<WorkspaceScope>>,
    ) -> Vec<ScopeUpdate> {
        workspaces
            .iter()
            .zip(current)
            .filter_map(|(workspace, scope)| {
                self.state(*workspace, scope.unwrap_or(WorkspaceScope::Closed))
            })
            .collect()
    }

    fn state(&mut self, workspace: WorkspaceId, scope: WorkspaceScope) -> Option<ScopeUpdate> {
        if self.workspaces.get(&workspace) == Some(&scope) {
            return None;
        }
        self.closed.retain(|closed| *closed != workspace);
        if scope == WorkspaceScope::Closed {
            self.closed.push(workspace);
            if self.closed.len() > MAXIMUM_CLOSED {
                let forgotten = self.closed.remove(0);
                self.workspaces.remove(&forgotten);
            }
        }
        self.workspaces.insert(workspace, scope.clone());
        Some(ScopeUpdate { workspace, scope })
    }
}
