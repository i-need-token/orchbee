//! Canonical popup membership is independent of plugin or configured command resolution.
use orchbee_workbench_protocol::{ClientPopupSource, PanelNode};

impl super::WorkbenchAuthority {
    pub fn popup_source_matches(&self, source: &ClientPopupSource) -> bool {
        if self.retired || source.member.validate().is_err() {
            return false;
        }
        let snapshot = self.snapshot();
        snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == source.terminal_context_id)
            .and_then(|context| context.tabs.iter().find(|tab| tab.tab_id == source.tab_id))
            .is_some_and(|tab| contains(&tab.root, source))
    }
}
fn contains(node: &PanelNode, source: &ClientPopupSource) -> bool {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } => {
            *panel_id == source.panel_id
                && std::iter::once(primary_presentation)
                    .chain(child_presentations)
                    .any(|candidate| {
                        &candidate.presentation_id == source.member.presentation_id()
                            && candidate.binding.as_ref() == source.member.binding()
                    })
        }
        PanelNode::Split { children, .. } => {
            children.iter().any(|child| contains(&child.node, source))
        }
    }
}
