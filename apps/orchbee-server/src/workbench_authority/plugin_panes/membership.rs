//! Canonical reservation owns plugin membership; display metadata never registers it.
use super::super::{WorkbenchAuthority, report_membership::ReportMembership};
use orchbee_workbench_protocol::{
    PanelNode, PluginOutcome, PluginPaneInfo, PluginPaneSelector, Presentation,
    PresentationBinding, Snapshot, TerminalPresentationId, WorkbenchPanelId,
    WorkbenchRejectionReason,
};
use std::collections::BTreeMap;
const MAXIMUM_MEMBERS: usize = 4096;

#[derive(Clone, Default)]
pub(in crate::workbench_authority) struct Membership(
    BTreeMap<TerminalPresentationId, (PresentationBinding, PluginPaneSelector)>,
);
impl Membership {
    fn retain(&mut self, snapshot: &Snapshot) {
        let members = ReportMembership::read(snapshot);
        self.0.retain(|id, (binding, _)| {
            members
                .presentations
                .get(id.as_str())
                .and_then(Option::as_ref)
                == Some(binding)
        });
    }
    pub(in crate::workbench_authority) fn prepare(
        &self,
        snapshot: &Snapshot,
        id: &TerminalPresentationId,
        binding: &PresentationBinding,
        selector: PluginPaneSelector,
    ) -> Result<Self, WorkbenchRejectionReason> {
        let members = ReportMembership::read(snapshot);
        if members
            .presentations
            .get(id.as_str())
            .and_then(Option::as_ref)
            != Some(binding)
        {
            return Err(WorkbenchRejectionReason::PanelUnavailable);
        }
        let mut next = self.clone();
        next.retain(snapshot);
        if next.0.len() >= MAXIMUM_MEMBERS || next.0.contains_key(id) {
            return Err(WorkbenchRejectionReason::ResourceLimit);
        }
        next.0.insert(id.clone(), (binding.clone(), selector));
        Ok(next)
    }
}

impl WorkbenchAuthority {
    /// Read membership at the same canonical revision used by a later fenced close.
    pub fn plugin_pane(&mut self, panel_id: &WorkbenchPanelId) -> PluginOutcome {
        if self.retired {
            return PluginOutcome::Failed {
                reason: orchbee_workbench_protocol::PluginFailure::Unavailable,
            };
        }
        let snapshot = self.projection.snapshot();
        self.plugin_panes.retain(snapshot);
        for context in &snapshot.terminal_contexts {
            for tab in &context.tabs {
                if let Some(presentation) = primary(&tab.root, panel_id)
                    && let Some((binding, selector)) =
                        self.plugin_panes.0.get(&presentation.presentation_id)
                    && presentation.binding.as_ref() == Some(binding)
                {
                    return PluginOutcome::PaneFound {
                        revision: snapshot.revision,
                        pane: Box::new(PluginPaneInfo {
                            selector: selector.clone(),
                            terminal_context_id: context.terminal_context_id.clone(),
                            tab_id: tab.tab_id.clone(),
                            panel_id: panel_id.clone(),
                            presentation_id: presentation.presentation_id.clone(),
                            binding: binding.clone(),
                        }),
                    };
                }
            }
        }
        PluginOutcome::Failed {
            reason: orchbee_workbench_protocol::PluginFailure::NotFound,
        }
    }
}
fn primary<'a>(node: &'a PanelNode, target: &WorkbenchPanelId) -> Option<&'a Presentation> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } => (panel_id == target).then_some(primary_presentation),
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|child| primary(&child.node, target)),
    }
}
