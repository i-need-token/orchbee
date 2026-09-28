//! Committed primary and child memberships define logical report resource lifetime.
use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, Snapshot, TerminalId, WorkbenchPanelId, WorkerId,
};
use std::collections::{BTreeMap, BTreeSet};
pub(super) type BindingKey = (WorkerId, TerminalId);
pub(super) fn key(binding: &PresentationBinding) -> BindingKey {
    (binding.worker_id.clone(), binding.terminal_id.clone())
}
#[derive(Default)]
pub(super) struct ReportMembership {
    pub(super) panels: BTreeMap<WorkbenchPanelId, Option<PresentationBinding>>,
    pub(super) live: BTreeSet<BindingKey>,
}
impl ReportMembership {
    pub(super) fn read(snapshot: &Snapshot) -> Self {
        let mut result = Self::default();
        for context in &snapshot.terminal_contexts {
            for tab in &context.tabs {
                result.include(&tab.root);
            }
        }
        result
    }
    fn include(&mut self, node: &PanelNode) {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                child_presentations,
                ..
            } => {
                self.panels
                    .insert(panel_id.clone(), primary_presentation.binding.clone());
                for p in std::iter::once(primary_presentation).chain(child_presentations) {
                    if let Some(binding) = &p.binding {
                        self.live.insert(key(binding));
                    }
                }
            }
            PanelNode::Split { children, .. } => {
                for child in children.iter() {
                    self.include(&child.node);
                }
            }
        }
    }
}
