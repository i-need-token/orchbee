use std::time::Instant;

use orchbee_workbench_protocol::{
    WorkbenchProtocolDocument, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseStatus, serialize_document,
};

use super::super::{
    RetainedRequest, WorkbenchAuthority, WorkbenchTerminalMembership, WorkbenchTerminalTab,
};
use super::{CloseRecord, CloseScope};
use crate::workbench_projection::{PreparedProjectionAdvance, WorkbenchProjectionStore};
use orchbee_workbench_protocol::{PanelNode, Presentation, WorkbenchPanelId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkbenchTabClosePublicationError {
    OperationUnavailable,
    NotReady,
    AuthorityRetired,
    CompositionMismatch,
    PreparationFailed,
}

pub struct PreparedWorkbenchTabClosePublication<'a> {
    projection: &'a mut WorkbenchProjectionStore,
    record: &'a mut CloseRecord,
    retired: &'a mut bool,
    prepared: PreparedProjectionAdvance,
    before: WorkbenchTerminalMembership,
    after: WorkbenchTerminalMembership,
    removed_members: Vec<WorkbenchTerminalTab>,
    revision: WorkbenchRevision,
    status: WorkbenchTabCloseStatus,
}

impl PreparedWorkbenchTabClosePublication<'_> {
    #[must_use]
    pub fn before_membership(&self) -> &WorkbenchTerminalMembership {
        &self.before
    }
    #[must_use]
    pub fn after_membership(&self) -> &WorkbenchTerminalMembership {
        &self.after
    }
    #[must_use]
    pub fn exact_removed_members(&self) -> &[WorkbenchTerminalTab] {
        &self.removed_members
    }
    #[must_use]
    pub fn status(&self) -> &WorkbenchTabCloseStatus {
        &self.status
    }
    #[must_use]
    pub const fn revision(&self) -> WorkbenchRevision {
        self.revision
    }

    /// Session composition failure retains known disposal; it cannot become a pre-effect rejection.
    pub fn retire_composition_failure(self) {
        *self.retired = true;
        self.record
            .publication_failed(WorkbenchTabClosePublicationReason::AuthorityRetired);
    }

    /// Commit together with Session's separately prepared reference update under the same gate.
    #[must_use]
    pub fn commit(self) -> WorkbenchTabCloseStatus {
        self.projection.commit_prepared(self.prepared);
        self.record.commit_closed(self.revision);
        self.status
    }
}

impl WorkbenchAuthority {
    pub fn prepare_terminal_tab_close_publication(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        now: Instant,
    ) -> Result<PreparedWorkbenchTabClosePublication<'_>, WorkbenchTabClosePublicationError> {
        if self.worktree_holds_close(close_request_id) {
            return Err(WorkbenchTabClosePublicationError::NotReady);
        }
        let Some(record) = self.close_record_mut(close_request_id) else {
            return Err(WorkbenchTabClosePublicationError::OperationUnavailable);
        };
        if record.next_step(now).next() != super::continuation::CloseContinuationNext::Publication
            || !record.publication_ready()
        {
            return Err(WorkbenchTabClosePublicationError::NotReady);
        }
        let record = self
            .close_record(close_request_id)
            .ok_or(WorkbenchTabClosePublicationError::OperationUnavailable)?;
        if self.retired {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::AuthorityRetired,
            );
        }
        let targets: Vec<_> = record.targets().cloned().collect();
        let scope = record.scope.clone();
        // The scope-level Context identity — never a member — is the
        // publication address; a zero-member Context scope has no first leaf.
        let context_id = record.terminal_context_id.clone();
        let tab_id = record.tab_id().cloned();
        let panel_id = record.panel_id().cloned();
        if self.ledger_bytes < record.reserved_bytes {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::CompositionMismatch,
            );
        }
        if (matches!(scope, CloseScope::Tab | CloseScope::Panel) && tab_id.is_none())
            || (matches!(scope, CloseScope::Panel) && panel_id.is_none())
        {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::CompositionMismatch,
            );
        }
        let before = match self.presentation_membership(&context_id) {
            Ok(membership) => membership,
            Err(_error) => {
                return self.fail_close_publication(
                    close_request_id,
                    WorkbenchTabClosePublicationError::CompositionMismatch,
                );
            }
        };
        // Compare every captured Presentation, not only the Panel's primary.
        // Exact order also detects child reordering while disposal was in flight.
        let actual_members = before.members().iter().filter(|member| match &scope {
            CloseScope::Context { .. } => true,
            CloseScope::Tab => Some(member.tab_id()) == tab_id.as_ref(),
            CloseScope::Panel => {
                Some(member.tab_id()) == tab_id.as_ref()
                    && Some(member.panel_id()) == panel_id.as_ref()
            }
        });
        if before.terminal_context_id() != &context_id
            || !actual_members.eq(targets.iter().map(|target| &target.member))
        {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::CompositionMismatch,
            );
        }
        let mut candidate = self.projection.snapshot().clone();
        let Some(revision) = candidate
            .revision
            .value()
            .checked_add(1)
            .map(WorkbenchRevision::new)
        else {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::CompositionMismatch,
            );
        };
        match &scope {
            // Strict membership proved the exact Tab/Panel/Presentation/binding tuple above.
            CloseScope::Tab => {
                let Some(context) = candidate
                    .terminal_contexts
                    .iter_mut()
                    .find(|context| context.terminal_context_id == context_id)
                else {
                    return self.fail_close_publication(
                        close_request_id,
                        WorkbenchTabClosePublicationError::CompositionMismatch,
                    );
                };
                let Some(index) = context
                    .tabs
                    .iter()
                    .position(|tab| Some(&tab.tab_id) == tab_id.as_ref())
                else {
                    return self.fail_close_publication(
                        close_request_id,
                        WorkbenchTabClosePublicationError::CompositionMismatch,
                    );
                };
                let removed = context.tabs.remove(index);
                // Only the vacated source group belongs to this Close. Unrelated groups,
                // including empty groups created by another Client, remain authoritative.
                context.groups.retain(|group| {
                    removed.group_id.as_ref() != Some(&group.group_id)
                        || context
                            .tabs
                            .iter()
                            .any(|tab| tab.group_id.as_ref() == Some(&group.group_id))
                });
            }
            CloseScope::Panel => {
                let panel_id = panel_id
                    .as_ref()
                    .ok_or(WorkbenchTabClosePublicationError::CompositionMismatch)?;
                let Some(context) = candidate
                    .terminal_contexts
                    .iter_mut()
                    .find(|context| context.terminal_context_id == context_id)
                else {
                    return self.fail_close_publication(
                        close_request_id,
                        WorkbenchTabClosePublicationError::CompositionMismatch,
                    );
                };
                let Some(tab_index) = context
                    .tabs
                    .iter()
                    .position(|tab| Some(&tab.tab_id) == tab_id.as_ref())
                else {
                    return self.fail_close_publication(
                        close_request_id,
                        WorkbenchTabClosePublicationError::CompositionMismatch,
                    );
                };
                let Some(_) = context
                    .tabs
                    .get(tab_index)
                    .and_then(|tab| leaf_presentation(&tab.root, panel_id))
                else {
                    return self.fail_close_publication(
                        close_request_id,
                        WorkbenchTabClosePublicationError::CompositionMismatch,
                    );
                };
                if matches!(&context.tabs[tab_index].root, PanelNode::Panel { .. }) {
                    // Sibling Closes already collapsed the tree; this leaf is the whole Tab.
                    context.tabs.remove(tab_index);
                } else {
                    let tab = &mut context.tabs[tab_index];
                    if !remove_leaf_panel(&mut tab.root, panel_id) {
                        return self.fail_close_publication(
                            close_request_id,
                            WorkbenchTabClosePublicationError::CompositionMismatch,
                        );
                    }
                }
            }
            CloseScope::Context { workspace_id } => {
                // One authoritative projection commit removes the Workspace
                // and its Terminal Context; Project, Directories, checkout,
                // and branch are not touched by construction.
                if candidate
                    .terminal_contexts
                    .iter()
                    .position(|context| context.terminal_context_id == context_id)
                    .is_none()
                {
                    return self.fail_close_publication(
                        close_request_id,
                        WorkbenchTabClosePublicationError::CompositionMismatch,
                    );
                }
                let Some(workspace_index) =
                    candidate
                        .navigation
                        .workspaces
                        .iter()
                        .position(|workspace| {
                            workspace.workspace_id == *workspace_id
                                && workspace.terminal_context_id == context_id
                        })
                else {
                    return self.fail_close_publication(
                        close_request_id,
                        WorkbenchTabClosePublicationError::CompositionMismatch,
                    );
                };
                candidate.navigation.workspaces.remove(workspace_index);
                candidate
                    .navigation
                    .workspace_order
                    .retain(|id| id != workspace_id);
                candidate
                    .terminal_contexts
                    .retain(|context| context.terminal_context_id != context_id);
            }
        }
        candidate.revision = revision;
        let after = if matches!(scope, CloseScope::Context { .. }) {
            WorkbenchTerminalMembership::removed_workspace(context_id.clone())
        } else {
            match super::super::terminal_membership::presentation_membership(
                &candidate,
                &context_id,
            ) {
                Ok(membership) => membership,
                Err(_error) => {
                    return self.fail_close_publication(
                        close_request_id,
                        WorkbenchTabClosePublicationError::CompositionMismatch,
                    );
                }
            }
        };
        let prepared = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate))
            .ok()
            .and_then(|encoded| self.projection.prepare_advance(&encoded).ok());
        let Some(prepared) = prepared else {
            return self.fail_close_publication(
                close_request_id,
                WorkbenchTabClosePublicationError::PreparationFailed,
            );
        };
        let status = WorkbenchTabCloseStatus::Closed { revision };
        let Some(RetainedRequest::TabClose(record)) = self.ledger.get_mut(close_request_id) else {
            return Err(WorkbenchTabClosePublicationError::OperationUnavailable);
        };
        Ok(PreparedWorkbenchTabClosePublication {
            projection: &mut self.projection,
            record,
            retired: &mut self.retired,
            prepared,
            before,
            after,
            removed_members: targets.into_iter().map(|target| target.member).collect(),
            revision,
            status,
        })
    }

    fn fail_close_publication<T>(
        &mut self,
        close_request_id: &WorkbenchRequestId,
        failure: WorkbenchTabClosePublicationError,
    ) -> Result<T, WorkbenchTabClosePublicationError> {
        if matches!(
            failure,
            WorkbenchTabClosePublicationError::CompositionMismatch
        ) {
            self.retired = true;
        }
        let reason = if self.retired {
            WorkbenchTabClosePublicationReason::AuthorityRetired
        } else {
            WorkbenchTabClosePublicationReason::PreparationFailed
        };
        if let Some(record) = self.close_record_mut(close_request_id) {
            record.publication_failed(reason);
        }
        Err(failure)
    }
}

/// Resolves the primary Presentation of the leaf Panel `panel_id`, or `None` when the identity
/// addresses no leaf (a Split identity or a vanished node).
pub(super) fn leaf_presentation<'a>(
    node: &'a PanelNode,
    panel_id: &WorkbenchPanelId,
) -> Option<&'a Presentation> {
    match node {
        PanelNode::Panel {
            right_click: _,
            panel_id: identity,
            primary_presentation,
            ..
        } if identity == panel_id => Some(primary_presentation),
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|slot| leaf_presentation(&slot.node, panel_id)),
        PanelNode::Panel { .. } => None,
    }
}

/// Removes the leaf Panel `panel_id` by replacing its parent Split with the sibling subtree.
/// The caller handles the case where the leaf is the Tab root.
fn remove_leaf_panel(node: &mut PanelNode, panel_id: &WorkbenchPanelId) -> bool {
    let PanelNode::Split { children, .. } = node else {
        return false;
    };
    for index in 0..2 {
        let is_target = matches!(
            &children[index].node,
            PanelNode::Panel {
                panel_id: identity,
                ..
            } if identity == panel_id
        );
        if is_target {
            *node = children[1 - index].node.clone();
            return true;
        }
        if remove_leaf_panel(&mut children[index].node, panel_id) {
            return true;
        }
    }
    false
}
