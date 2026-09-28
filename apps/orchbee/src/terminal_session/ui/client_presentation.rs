//! Exact local membership from a live PaneRecord or a stable unbound selection.
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
        let Some((
            context,
            tab,
            PanelNode::Panel {
                primary_presentation,
                child_presentations,
                ..
            },
        )) = snapshot.terminal_contexts.iter().find_map(|context| {
            context
                .tabs
                .iter()
                .find_map(|tab| panel(&tab.root, panel_id).map(|panel| (context, tab, panel)))
        })
        else {
            return Err(Reason::TargetUnavailable);
        };
        if (state.pane_by_panel(panel_id).is_some() || state.panel_is_unbound(panel_id))
            && (context.terminal_context_id != *state.cache().terminal_context_id()
                || state
                    .active()
                    .is_none_or(|active| active.key().anchor().tab_id() != &tab.tab_id))
        {
            return Err(Reason::ProjectionUnavailable);
        }
        let mut presentation = state
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
                Ok(ClientPresentationSelection::Bound {
                    presentation_id: member.presentation_id.clone(),
                    binding: member.binding.clone().expect("matched bound member"),
                    attachment_generation: WorkbenchRevision::new(key.anchor().generation()),
                    incarnation: key.incarnation(),
                    viewport,
                })
            })
            .transpose()?;
        if presentation.is_none()
            && matches!(
                state.phase_kind(),
                SessionPhaseKind::Ready | SessionPhaseKind::SelectedWithoutAttachment
            )
            && let Some(active) = state.active()
            && let Some(local_panel) = state
                .cache()
                .tabs()
                .get(active.index())
                .and_then(|tab| tab.panel_index(panel_id).and_then(|index| tab.panel(index)))
        {
            let local = state.selected_presentation(local_panel);
            if local.binding().is_none() {
                let canonical = std::iter::once(primary_presentation)
                    .chain(child_presentations)
                    .find(|member| &member.presentation_id == local.presentation_id())
                    .ok_or(Reason::ProjectionUnavailable)?;
                if canonical.binding.is_some() {
                    return Err(Reason::ProjectionUnavailable);
                }
                presentation = Some(ClientPresentationSelection::Unbound {
                    presentation_id: canonical.presentation_id.clone(),
                    selection_generation: WorkbenchRevision::new(
                        active.key().anchor().generation(),
                    ),
                });
            }
        }
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
