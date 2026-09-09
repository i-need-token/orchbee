use orchbee_workbench_protocol::{PanelNode, PresentationBinding, SiteId};

use super::{WorkbenchAuthority, terminal_membership::visit_panel_presentations};

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
    let mut exposed = false;
    visit_panel_presentations(node, &mut |_, presentation| {
        exposed |= presentation.binding.as_ref() == Some(binding);
    });
    exposed
}

#[cfg(test)]
mod tests;
