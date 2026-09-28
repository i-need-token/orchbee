//! A Panel's non-departing PaneRecord owns the exact current member and local token.
use super::*;
use orchbee_workbench_protocol::{
    ClientPresentationSelection, ClientPresentationSnapshot, ClientViewControl,
    ClientViewControlReason as Reason, ClientViewIntent, ClientViewportObservation,
    ClientViewportUnavailable, PanelNode, Snapshot, WorkbenchRevision,
};

fn panel<'a>(node: &'a PanelNode, id: &WorkbenchPanelId) -> Option<&'a PanelNode> {
    match node {
        PanelNode::Panel { panel_id, .. } if panel_id == id => Some(node),
        PanelNode::Split { children, .. } => {
            children.iter().find_map(|child| panel(&child.node, id))
        }
        _ => None,
    }
}
impl NativeUi {
    pub(in crate::terminal_session) fn read_client_presentation(
        &self,
        command: &ClientViewControl,
        snapshot: &Snapshot,
    ) -> Result<ClientPresentationSnapshot, Reason> {
        let ClientViewIntent::Presentation { panel_id } = &command.intent else {
            return Err(Reason::Unsupported);
        };
        command.validate().map_err(|_| Reason::TargetUnavailable)?;
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        if snapshot.site_id != *state.cache().site_id()
            || snapshot.revision < command.minimum_workbench_revision
        {
            return Err(Reason::ProjectionUnavailable);
        }
        let Some(PanelNode::Panel {
            primary_presentation,
            child_presentations,
            ..
        }) = snapshot
            .terminal_contexts
            .iter()
            .flat_map(|context| &context.tabs)
            .find_map(|tab| panel(&tab.root, panel_id))
        else {
            return Err(Reason::TargetUnavailable);
        };
        let presentation = state
            .pane_by_panel(panel_id)
            .map(|pane| {
                let member = std::iter::once(primary_presentation)
                    .chain(child_presentations)
                    .find(|member| {
                        member.binding.as_ref().is_some_and(|binding| {
                            pane.binding().terminal_id() == &binding.terminal_id
                                && pane.binding().worker_id() == &binding.worker_id
                        })
                    })
                    .ok_or(Reason::ProjectionUnavailable)?;
                let key = pane.pane_ref();
                let viewport = self
                    .projections
                    .iter()
                    .find(|projection| projection.key == key)
                    .map_or(
                        ClientViewportObservation::Unavailable {
                            reason: ClientViewportUnavailable::FrameUnavailable,
                        },
                        super::client_viewport::observe,
                    );
                Ok(ClientPresentationSelection {
                    presentation_id: member.presentation_id.clone(),
                    binding: member.binding.clone().expect("matched bound member"),
                    attachment_generation: WorkbenchRevision::new(key.anchor().generation()),
                    incarnation: key.incarnation(),
                    viewport,
                })
            })
            .transpose()?;
        let result = ClientPresentationSnapshot {
            workbench_revision: snapshot.revision,
            panel_id: panel_id.clone(),
            presentation,
        };
        let (_, view) = self
            .control_observation(false)
            .ok_or(Reason::ProjectionUnavailable)?;
        if !result.answers(panel_id, &view, command.minimum_workbench_revision) {
            return Err(Reason::ProjectionUnavailable);
        }
        Ok(result)
    }
}
