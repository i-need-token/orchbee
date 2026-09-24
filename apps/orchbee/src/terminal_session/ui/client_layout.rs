//! Read-only target projection using canonical membership and the observing Client's geometry.
use super::NativeUi;
use crate::{
    client_view::SidebarSurface,
    terminal_session::workbench_projection::{
        NativeSimpleTab, project_native_context_tabs, project_native_standalone_tabs,
    },
    tui_layout::{self, ViewportTier},
};
use orchbee_workbench_protocol::{
    ClientCellRect, ClientPanelArea, ClientPanelLayout, ClientPanelNeighbors, ClientSplitArea,
    ClientSplitRatio, ClientViewControl, ClientViewControlReason as Reason, ClientViewIntent,
    Snapshot,
};
use ratatui::layout::Rect;

impl NativeUi {
    pub(in crate::terminal_session) fn read_client_layout(
        &self,
        command: &ClientViewControl,
        snapshot: &Snapshot,
        viewport: Rect,
        sidebar: SidebarSurface,
    ) -> Result<ClientPanelLayout, Reason> {
        let ClientViewIntent::Layout {
            terminal_context_id,
            tab_id,
            panel_id,
        } = &command.intent
        else {
            return Err(Reason::Unsupported);
        };
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        if snapshot.site_id != *state.cache().site_id()
            || snapshot.revision < command.minimum_workbench_revision
        {
            return Err(Reason::ProjectionUnavailable);
        }
        let navigation =
            project_native_standalone_tabs(snapshot).map_err(|_| Reason::ProjectionUnavailable)?;
        let location = navigation
            .navigation()
            .iter()
            .find(|entry| entry.location.terminal_context_id() == terminal_context_id)
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
        let selected = state.cache().terminal_context_id() == terminal_context_id
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
        let focused = selected.then(|| state.focus_panel_id().cloned()).flatten();
        let active_panel = focused.as_ref().and_then(|id| tab.panel_index(id));
        let zoomed = state.layout_zoomed(tab);
        let (area, panels, splits) =
            project_geometry(viewport, sidebar, tab, active_panel, zoomed)?;
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
) -> Result<(Rect, Vec<ClientPanelArea>, Vec<ClientSplitArea>), Reason> {
    let areas = tui_layout::project_shell_areas(viewport, sidebar);
    if areas.tier == ViewportTier::TooSmall {
        return Err(Reason::Busy);
    }
    let framed = areas.tier.shows_panel_frame();
    let content = tui_layout::terminal_area(areas.panel, framed);
    if content.is_empty() {
        return Err(Reason::ProjectionUnavailable);
    }
    let paned = framed && tab.panel_count() > 1;
    let projection = tui_layout::project_panel_tree(
        if paned { content } else { Rect::default() },
        tab.panel_tree(),
        active_panel,
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
            (active_panel == Some(index)).then_some(cell(content))
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
    let ratio = ClientSplitRatio::new(0.5).map_err(|_| Reason::ProjectionUnavailable)?;
    let splits = projection
        .splits
        .iter()
        .map(|split| ClientSplitArea {
            split_id: split.split_id.clone(),
            first_slot_id: split.first_slot_id.clone(),
            second_slot_id: split.second_slot_id.clone(),
            orientation: split.orientation,
            ratio,
            area: split.area.map(cell),
            divider: split.divider.map(cell),
        })
        .collect();
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
