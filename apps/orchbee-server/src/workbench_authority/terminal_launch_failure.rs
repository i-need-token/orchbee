//! Publishes a known terminal launch failure, or another one-shot Server fact, on the Presentation
//! still bound to that terminal.
use super::WorkbenchAuthority;
use orchbee_workbench_protocol::{
    PanelNode, Presentation, PresentationBinding, PresentationLaunchFailure,
    TerminalPresentationId, WorkbenchProtocolDocument, WorkbenchRevision, serialize_document,
};

impl WorkbenchAuthority {
    /// Advances the projection with one failure fact after the creating request has settled. The
    /// caller holds the Session gate across both steps, so no reader observes the settled failure
    /// without it. A rebinding, removal or exhausted projection budget leaves the request receipt
    /// as the only failure fact; nothing here retries or repeats a launch.
    pub fn record_terminal_launch_failure(
        &mut self,
        presentation_id: &TerminalPresentationId,
        binding: &PresentationBinding,
        executable: Option<&str>,
    ) -> bool {
        self.commit_presentation_fact(presentation_id, binding, |presentation| {
            if presentation.launch_failure.is_some() {
                return false;
            }
            presentation.launch_failure = Some(PresentationLaunchFailure::terminal_create(
                executable.map(str::to_owned),
            ));
            true
        })
    }

    /// Advances the projection by exactly one revision with `apply`'s change to the Presentation
    /// still bound as `binding`. Nothing advances when the authority is retired, the Presentation
    /// is gone or rebound, `apply` declines, or the projection budget refuses the candidate.
    pub(super) fn commit_presentation_fact(
        &mut self,
        presentation_id: &TerminalPresentationId,
        binding: &PresentationBinding,
        apply: impl FnOnce(&mut Presentation) -> bool,
    ) -> bool {
        if self.retired {
            return false;
        }
        let mut candidate = self.projection.snapshot().clone();
        let Some(presentation) = candidate
            .terminal_contexts
            .iter_mut()
            .flat_map(|context| context.tabs.iter_mut())
            .find_map(|tab| presentation_mut(&mut tab.root, presentation_id))
        else {
            return false;
        };
        if presentation.binding.as_ref() != Some(binding) || !apply(presentation) {
            return false;
        }
        let Some(revision) = candidate.revision.value().checked_add(1) else {
            return false;
        };
        candidate.revision = WorkbenchRevision::new(revision);
        let Ok(encoded) = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate))
        else {
            return false;
        };
        let Ok(prepared) = self.projection.prepare_advance(&encoded) else {
            return false;
        };
        self.projection.commit_prepared(prepared);
        true
    }
}

fn presentation_mut<'node>(
    node: &'node mut PanelNode,
    presentation_id: &TerminalPresentationId,
) -> Option<&'node mut Presentation> {
    match node {
        PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        } => std::iter::once(primary_presentation)
            .chain(child_presentations)
            .find(|presentation| &presentation.presentation_id == presentation_id),
        PanelNode::Split { children, .. } => children
            .iter_mut()
            .find_map(|child| presentation_mut(&mut child.node, presentation_id)),
    }
}
