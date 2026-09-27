//! Pure projection of the exact primary incarnation's already-installed viewport.
use super::*;
use orchbee_workbench_protocol::{
    ClientPanelViewport, ClientScrollMetrics, ClientViewControl, ClientViewControlReason as Reason,
    ClientViewIntent, ClientViewportMode, ClientViewportObservation as Observation,
    ClientViewportPending, ClientViewportSnapshot, ClientViewportSource,
    ClientViewportUnavailable as Unavailable, PanelNode, PresentationBinding, Snapshot,
};

fn binding<'a>(
    node: &'a PanelNode,
    id: &WorkbenchPanelId,
) -> Option<Option<&'a PresentationBinding>> {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } if panel_id == id => Some(primary_presentation.binding.as_ref()),
        PanelNode::Split { children, .. } => {
            children.iter().find_map(|child| binding(&child.node, id))
        }
        _ => None,
    }
}

impl NativeUi {
    pub(in crate::terminal_session) fn read_client_viewports(
        &self,
        command: &ClientViewControl,
        snapshot: &Snapshot,
    ) -> Result<ClientViewportSnapshot, Reason> {
        let ClientViewIntent::Viewports { panel_ids } = &command.intent else {
            return Err(Reason::Unsupported);
        };
        command.validate().map_err(|_| Reason::TargetUnavailable)?;
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        if snapshot.site_id != *state.cache().site_id()
            || snapshot.revision < command.minimum_workbench_revision
        {
            return Err(Reason::ProjectionUnavailable);
        }
        let panels = panel_ids
            .iter()
            .map(|id| {
                let primary = snapshot
                    .terminal_contexts
                    .iter()
                    .flat_map(|context| &context.tabs)
                    .find_map(|tab| binding(&tab.root, id))
                    .ok_or(Reason::TargetUnavailable)?;
                let projection = primary.and_then(|binding| {
                    let pane = state.pane_by_panel(id)?;
                    if pane.binding().terminal_id() != &binding.terminal_id
                        || pane.binding().worker_id() != &binding.worker_id
                    {
                        return None;
                    }
                    self.projections
                        .iter()
                        .find(|projection| projection.key == pane.pane_ref())
                });
                Ok(ClientPanelViewport {
                    panel_id: id.clone(),
                    primary_terminal_id: primary.map(|b| b.terminal_id.clone()),
                    observation: projection.map_or(
                        Observation::Unavailable {
                            reason: Unavailable::NotAttached,
                        },
                        observe,
                    ),
                })
            })
            .collect::<Result<Vec<_>, Reason>>()?;
        let result = ClientViewportSnapshot {
            workbench_revision: snapshot.revision,
            panels,
        };
        let (_, view) = self
            .control_observation(false)
            .ok_or(Reason::ProjectionUnavailable)?;
        if !result.answers(panel_ids, &view, command.minimum_workbench_revision) {
            return Err(Reason::ProjectionUnavailable);
        }
        Ok(result)
    }
}

fn observe(projection: &KeyedProjection) -> Observation {
    if let Some(viewport) = &projection.viewport {
        let phase = match viewport.surface() {
            ViewportSurface::Entering => ClientViewportPending::Entering,
            ViewportSurface::AwaitingWindow => ClientViewportPending::AwaitingWindow,
            ViewportSurface::Recovering => ClientViewportPending::Recovering,
            ViewportSurface::Window(window) => {
                let route = window.route;
                return observed(
                    ClientViewportMode::History,
                    ClientViewportSource {
                        host_epoch: route.host_epoch.to_text(),
                        attachment_id: route.attachment_id.to_text(),
                        generation: route.generation.get(),
                        revision: route.revision,
                        columns: route.columns,
                        alternate_screen: false,
                        total_rows: route.total_rows,
                        start_row: route.start_row,
                    },
                    viewport.geometry().1,
                );
            }
        };
        return Observation::Pending { phase };
    }
    if projection.status == NativeProjectionStatus::FrameAvailable
        && let Some(frame) = projection.value.frame()
    {
        let rows = frame.geometry.rows;
        let scrollback = if frame.alternate_screen {
            0
        } else {
            frame.scrollback_rows
        };
        if let Some(total_rows) = scrollback.checked_add(u64::from(rows)) {
            return observed(
                ClientViewportMode::Live,
                ClientViewportSource {
                    host_epoch: frame.route.host_epoch.to_text(),
                    attachment_id: frame.route.attachment_id.to_text(),
                    generation: frame.route.generation.get(),
                    revision: frame.route.revision,
                    columns: frame.geometry.columns,
                    alternate_screen: frame.alternate_screen,
                    total_rows,
                    start_row: scrollback,
                },
                rows,
            );
        }
    }
    Observation::Unavailable {
        reason: Unavailable::FrameUnavailable,
    }
}
fn observed(mode: ClientViewportMode, source: ClientViewportSource, rows: u16) -> Observation {
    let maximum = source.total_rows.saturating_sub(u64::from(rows));
    Observation::Observed {
        mode,
        scroll: ClientScrollMetrics {
            offset_from_bottom: maximum.saturating_sub(source.start_row),
            max_offset_from_bottom: maximum,
            viewport_rows: rows,
        },
        source,
    }
}
