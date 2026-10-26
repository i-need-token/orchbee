//! Read-only target projection using canonical membership and the observing Client's geometry.
mod adjustment;
mod divider;
use super::NativeUi;
use crate::{
    client_view::SidebarSurface,
    tui_layout::{self, ViewportTier},
    workbench_client::workbench_projection::{
        NativeSimpleTab, project_native_context_tabs, project_preferred_workspace,
    },
};
use orchbee_workbench_protocol::{
    ClientCellRect, ClientPanelArea, ClientPanelLayout, ClientPanelNeighbors, ClientSplitArea,
    ClientSplitRatio, ClientViewControl, ClientViewControlReason as Reason, ClientViewIntent,
    Snapshot,
};
use ratatui::layout::Rect;

impl NativeUi {
    pub(in crate::workbench_client) fn read_client_selections(
        &self,
        command: &ClientViewControl,
        snapshot: &Snapshot,
    ) -> Result<orchbee_workbench_protocol::ClientNavigationSnapshot, Reason> {
        let ClientViewIntent::Selections {
            terminal_context_ids,
        } = &command.intent
        else {
            return Err(Reason::Unsupported);
        };
        command.validate().map_err(|_| Reason::TargetUnavailable)?;
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        if snapshot.site_id != *state.cache().site_id()
            || snapshot.revision < command.minimum_workbench_revision
        {
            return Err(Reason::ProjectionUnavailable);
        }
        let navigation =
            project_preferred_workspace(snapshot).map_err(|_| Reason::ProjectionUnavailable)?;
        let contexts = terminal_context_ids
            .iter()
            .map(|id| {
                let location = &navigation
                    .navigation()
                    .iter()
                    .find(|entry| entry.location.terminal_context_id() == Some(id))
                    .ok_or(Reason::TargetUnavailable)?
                    .location;
                let candidate = project_native_context_tabs(snapshot, location)
                    .map_err(|_| Reason::ProjectionUnavailable)?;
                Ok(orchbee_workbench_protocol::ClientContextSelection {
                    terminal_context_id: id.clone(),
                    active_tab_id: state
                        .query_active_tab_id(&candidate)
                        .map_err(|_| Reason::ProjectionUnavailable)?,
                })
            })
            .collect::<Result<Vec<_>, Reason>>()?;
        let result = orchbee_workbench_protocol::ClientNavigationSnapshot {
            workbench_revision: snapshot.revision,
            contexts,
        };
        let (_, view) = self
            .control_observation(false)
            .ok_or(Reason::ProjectionUnavailable)?;
        if !result.answers(
            terminal_context_ids,
            &view,
            command.minimum_workbench_revision,
        ) {
            return Err(Reason::ProjectionUnavailable);
        }
        Ok(result)
    }

    pub(in crate::workbench_client) fn read_client_layout(
        &self,
        command: &ClientViewControl,
        snapshot: &Snapshot,
        viewport: Rect,
        sidebar: SidebarSurface,
    ) -> Result<ClientPanelLayout, Reason> {
        let (terminal_context_id, tab_id, panel_id) = match &command.intent {
            ClientViewIntent::Layout {
                terminal_context_id,
                tab_id,
                panel_id,
            }
            | ClientViewIntent::FocusDirection {
                terminal_context_id,
                tab_id,
                panel_id,
                ..
            } => (terminal_context_id, tab_id, panel_id),
            _ => return Err(Reason::Unsupported),
        };
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        if snapshot.site_id != *state.cache().site_id()
            || snapshot.revision < command.minimum_workbench_revision
        {
            return Err(Reason::ProjectionUnavailable);
        }
        let navigation =
            project_preferred_workspace(snapshot).map_err(|_| Reason::ProjectionUnavailable)?;
        let location = navigation
            .navigation()
            .iter()
            .find(|entry| entry.location.terminal_context_id() == Some(terminal_context_id))
            .ok_or(Reason::TargetUnavailable)?
            .location
            .clone();
        let candidate = project_native_context_tabs(snapshot, &location)
            .map_err(|_| Reason::ProjectionUnavailable)?;
        let canonical = candidate
            .tab_index(tab_id)
            .and_then(|index| candidate.tabs().get(index))
            .ok_or(Reason::TargetUnavailable)?;
        if canonical.panel_index(panel_id).is_none() {
            return Err(Reason::TargetUnavailable);
        }
        let selected = state.cache().terminal_context_id() == Some(terminal_context_id)
            && state
                .active()
                .is_some_and(|active| active.key().anchor().tab_id() == tab_id);
        let cache = if selected { state.cache() } else { &candidate };
        if cache.revision() < command.minimum_workbench_revision {
            return Err(Reason::ProjectionUnavailable);
        }
        let tab = cache
            .tab_index(tab_id)
            .and_then(|index| cache.tabs().get(index))
            .ok_or(Reason::TargetUnavailable)?;
        if tab.panel_index(panel_id).is_none() {
            return Err(Reason::TargetUnavailable);
        }
        let focused = if selected {
            state.focus_panel_id().cloned()
        } else {
            state.layout_focus_panel_id(&location, tab)
        };
        let active_panel = focused.as_ref().and_then(|id| tab.panel_index(id));
        let zoomed = state.layout_zoomed(tab);
        let (area, panels, splits) = project_geometry(
            viewport,
            sidebar,
            tab,
            active_panel,
            zoomed,
            &|split, first, second| {
                tab.panel_tree()
                    .split_ratio(split, first, second)
                    .unwrap_or_default()
            },
        )?;
        let neighbors = ClientPanelNeighbors::from_panels(panel_id, &panels);
        let source_available = panels
            .iter()
            .any(|panel| &panel.panel_id == panel_id && panel.unzoomed.is_some());
        let result = ClientPanelLayout {
            terminal_context_id: terminal_context_id.clone(),
            tab_id: tab_id.clone(),
            source_panel_id: panel_id.clone(),
            workbench_revision: cache.revision(),
            viewport: cell(viewport),
            area: cell(area),
            selected,
            zoomed,
            focused_panel_id: focused,
            source_available,
            neighbors,
            panels,
            splits,
        };
        result
            .validate()
            .map_err(|_| Reason::ProjectionUnavailable)?;
        Ok(result)
    }
}

fn project_geometry(
    viewport: Rect,
    sidebar: SidebarSurface,
    tab: &NativeSimpleTab,
    active_panel: Option<usize>,
    zoomed: bool,
    ratios: &impl Fn(
        &orchbee_workbench_protocol::WorkbenchSplitId,
        &orchbee_workbench_protocol::WorkbenchPanelSlotId,
        &orchbee_workbench_protocol::WorkbenchPanelSlotId,
    ) -> ClientSplitRatio,
) -> Result<(Rect, Vec<ClientPanelArea>, Vec<ClientSplitArea>), Reason> {
    let areas = tui_layout::project_shell_areas(viewport, sidebar);
    if areas.tier == ViewportTier::TooSmall {
        return Err(Reason::Busy);
    }
    let framed = areas.tier.shows_panel_frame();
    let pane_frame = |zoomed| tui_layout::PaneFrame::of(framed, tab.panel_count(), zoomed);
    let content = tui_layout::native_terminal_area(areas.panel, pane_frame(false));
    let displayed_content = tui_layout::native_terminal_area(areas.panel, pane_frame(zoomed));
    if content.is_empty() {
        return Err(Reason::ProjectionUnavailable);
    }
    let paned = framed && tab.panel_count() > 1;
    let projection = tui_layout::project_panel_tree(
        if paned { content } else { Rect::default() },
        tab.panel_tree(),
        active_panel,
        ratios,
    );
    let fallback = active_panel.unwrap_or(0);
    let mut panels = Vec::with_capacity(tab.panel_count());
    for index in 0..tab.panel_count() {
        let panel = tab.panel(index).ok_or(Reason::ProjectionUnavailable)?;
        let leaf = projection
            .leaves
            .iter()
            .find(|leaf| leaf.panel_index == index);
        let (unzoomed, terminal) = if paned {
            (
                leaf.map(|leaf| cell(leaf.leaf)),
                leaf.map(|leaf| cell(leaf.content)),
            )
        } else if index == fallback {
            (Some(cell(content)), Some(cell(content)))
        } else {
            (None, None)
        };
        let displayed = if zoomed {
            (active_panel == Some(index)).then_some(cell(displayed_content))
        } else {
            terminal
        };
        panels.push(ClientPanelArea {
            panel_id: panel.panel_id().clone(),
            unzoomed,
            terminal,
            displayed,
        });
    }
    let splits = projection
        .splits
        .iter()
        .map(|split| ClientSplitArea {
            split_id: split.split_id.clone(),
            first_slot_id: split.first_slot_id.clone(),
            second_slot_id: split.second_slot_id.clone(),
            orientation: split.orientation,
            ratio: split.ratio,
            area: split.area.map(cell),
            divider: split.divider.map(cell),
        })
        .collect();
    // Zoom changes the displayed pane, not the root containing the unzoomed tree.
    Ok((content, panels, splits))
}

fn cell(rect: Rect) -> ClientCellRect {
    ClientCellRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

#[cfg(test)]
mod tests;
