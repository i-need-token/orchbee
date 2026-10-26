//! Exact local membership from a live PaneRecord or a stable unbound selection.
use super::{ClientPhaseKind, NativeUi, WorkbenchPanelId};
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
        PanelNode::Panel { .. } => None,
    }
}
impl NativeUi {
    #[cfg(windows)]
    pub(in crate::workbench_client) fn configured_command_source(
        &self,
    ) -> Option<(
        Option<orchbee_workbench_protocol::ClientPopupSource>,
        orchbee_workbench_protocol::ClientViewObservation,
    )> {
        let (_, view) = self.control_observation(false)?;
        if view.phase != orchbee_workbench_protocol::ClientViewPhase::Ready {
            return None;
        }
        let selection = view.selection.as_ref()?;
        if selection.tab_id.is_none() && selection.panel_id.is_none() {
            return Some((None, view));
        }
        let panel_id = selection.panel_id.as_ref()?;
        let tab_id = selection.tab_id.as_ref()?;
        let state = self.state.as_ref()?;
        let active = state.active()?;
        let tab = state.cache().tabs().get(active.index())?;
        let panel = tab
            .panel_index(panel_id)
            .and_then(|index| tab.panel(index))?;
        let selected = state.selected_presentation(panel);
        let binding = selected.binding()?;
        let pane = state.pane_by_panel(panel_id)?;
        if pane.binding().worker_id() != binding.worker_id()
            || pane.binding().terminal_id() != binding.terminal_id()
        {
            return None;
        }
        let key = pane.pane_ref();
        let viewport = self
            .projections
            .iter()
            .find(|projection| projection.key == key)
            .map(super::client_viewport::observe)?;
        let source = orchbee_workbench_protocol::ClientPopupSource {
            terminal_context_id: selection.terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: panel_id.clone(),
            member: ClientPresentationSelection::Bound {
                presentation_id: selected.presentation_id().clone(),
                binding: orchbee_workbench_protocol::PresentationBinding {
                    worker_id: binding.worker_id().clone(),
                    terminal_id: binding.terminal_id().clone(),
                },
                attachment_generation: WorkbenchRevision::new(key.anchor().generation()),
                incarnation: key.incarnation(),
                viewport,
            },
        };
        Some((Some(source), view))
    }
    pub(in crate::workbench_client) fn read_client_presentation(
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
            && (Some(&context.terminal_context_id) != state.cache().terminal_context_id()
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
                ClientPhaseKind::Ready | ClientPhaseKind::SelectedWithoutAttachment
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
