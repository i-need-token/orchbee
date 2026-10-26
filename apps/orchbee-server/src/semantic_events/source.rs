//! Resolve a committed Panel or child Presentation without borrowing another source's identity.
use orchbee_workbench_protocol::{
    PanelNode, PresentationBinding, SemanticEventSource, SemanticPane, Snapshot, Tab,
    WorkbenchTerminalContextId,
};

#[must_use]
pub fn semantic_pane(snapshot: &Snapshot, source: &SemanticEventSource) -> Option<SemanticPane> {
    for context in &snapshot.terminal_contexts {
        for tab in &context.tabs {
            if let Some(pane) = find(&tab.root, &context.terminal_context_id, tab, source) {
                return Some(pane);
            }
        }
    }
    None
}
/// All committed presentations sharing an admitted physical resource, without collapsing children.
#[must_use]
pub fn semantic_panes_for_binding(
    snapshot: &Snapshot,
    binding: &PresentationBinding,
) -> Vec<SemanticPane> {
    fn visit(
        node: &PanelNode,
        context: &WorkbenchTerminalContextId,
        tab: &Tab,
        binding: &PresentationBinding,
        out: &mut Vec<SemanticPane>,
    ) {
        match node {
            PanelNode::Panel {
                panel_id,
                primary_presentation,
                child_presentations,
                ..
            } => {
                for presentation in std::iter::once(primary_presentation).chain(child_presentations)
                {
                    if presentation.binding.as_ref() == Some(binding) {
                        out.push(SemanticPane {
                            terminal_context_id: context.clone(),
                            tab_id: tab.tab_id.clone(),
                            panel_id: panel_id.clone(),
                            presentation_id: presentation.presentation_id.clone(),
                            binding: Some(binding.clone()),
                            label: presentation.display_label().into(),
                        });
                    }
                }
            }
            PanelNode::Split { children, .. } => {
                for slot in children.iter() {
                    visit(&slot.node, context, tab, binding, out);
                }
            }
        }
    }
    let mut panes = Vec::new();
    for context in &snapshot.terminal_contexts {
        for tab in &context.tabs {
            visit(
                &tab.root,
                &context.terminal_context_id,
                tab,
                binding,
                &mut panes,
            );
        }
    }
    panes
}
fn find(
    node: &PanelNode,
    context: &WorkbenchTerminalContextId,
    tab: &Tab,
    source: &SemanticEventSource,
) -> Option<SemanticPane> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
            ..
        } if panel_id == &source.panel_id => {
            let presentation = match &source.presentation_id {
                None => primary_presentation,
                Some(id) => std::iter::once(primary_presentation)
                    .chain(child_presentations)
                    .find(|presentation| &presentation.presentation_id == id)?,
            };
            Some(SemanticPane {
                terminal_context_id: context.clone(),
                tab_id: tab.tab_id.clone(),
                panel_id: panel_id.clone(),
                presentation_id: presentation.presentation_id.clone(),
                binding: presentation.binding.clone(),
                label: presentation.display_label().into(),
            })
        }
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|slot| find(&slot.node, context, tab, source)),
        PanelNode::Panel { .. } => None,
    }
}
