use orchbee_workbench_protocol::{PanelNode, PresentationBinding, SiteId};

use super::WorkbenchAuthority;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkbenchBindingExposure {
    Exposed,
    NotExposed,
    Unavailable,
}

impl WorkbenchAuthority {
    #[must_use]
    pub fn terminal_binding_exposure(
        &self,
        site_id: &SiteId,
        binding: &PresentationBinding,
    ) -> WorkbenchBindingExposure {
        if self.retired {
            return WorkbenchBindingExposure::Unavailable;
        }
        let snapshot = self.projection.snapshot();
        if &snapshot.site_id != site_id {
            return WorkbenchBindingExposure::NotExposed;
        }
        if snapshot.terminal_contexts.iter().any(|context| {
            context
                .tabs
                .iter()
                .any(|tab| panel_exposes_binding(&tab.root, binding))
        }) {
            WorkbenchBindingExposure::Exposed
        } else {
            WorkbenchBindingExposure::NotExposed
        }
    }
}

fn panel_exposes_binding(node: &PanelNode, binding: &PresentationBinding) -> bool {
    match node {
        PanelNode::Panel {
            child_presentations,
            primary_presentation,
            ..
        } => {
            primary_presentation.binding.as_ref() == Some(binding)
                || child_presentations
                    .iter()
                    .any(|presentation| presentation.binding.as_ref() == Some(binding))
        }
        PanelNode::Split { children, .. } => children
            .iter()
            .any(|child| panel_exposes_binding(&child.node, binding)),
    }
}

#[cfg(test)]
mod tests;
