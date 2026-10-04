//! Revalidate a painted divider against the selected cache before release-only CAS handoff.
use super::{NativeUi, Reason};
use crate::{
    client_view::SidebarSurface,
    tui_layout::{DrawnDivider, project_panel_tree, project_shell_areas, terminal_area},
};
use orchbee_workbench_protocol::{
    ClientLayoutAdjustment, ClientLayoutChange, ClientSplitChange, ClientSplitRatio,
    ClientViewObservation, ClientViewPhase, SiteId,
};
use ratatui::layout::Rect;

impl NativeUi {
    pub(in crate::terminal_session) fn prepare_divider_adjustment(
        &self,
        target: &DrawnDivider,
        ratio: ClientSplitRatio,
        viewport: Option<(Rect, SidebarSurface)>,
    ) -> Result<(SiteId, ClientLayoutAdjustment, ClientViewObservation), Reason> {
        let (site, before) = self
            .control_observation(false)
            .ok_or(Reason::ProjectionUnavailable)?;
        if before.phase == ClientViewPhase::Unavailable || self.retirement_failure.is_some() {
            return Err(Reason::ProjectionUnavailable);
        }
        if before.phase != ClientViewPhase::Ready {
            return Err(Reason::Busy);
        }
        if self.view().divider_source().as_ref() != Some(&target.source) {
            return Err(Reason::StaleView);
        }
        let (viewport, sidebar) = viewport.ok_or(Reason::ProjectionUnavailable)?;
        if viewport != target.viewport {
            return Err(Reason::StaleView);
        }
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        if state.zoom_active() {
            return Err(Reason::StaleView);
        }
        let areas = project_shell_areas(viewport, sidebar);
        if !areas.tier.shows_panel_frame() {
            return Err(Reason::ProjectionUnavailable);
        }
        let tab = state
            .active()
            .and_then(|active| state.cache().tabs().get(active.index()))
            .ok_or(Reason::TargetUnavailable)?;
        let projection = project_panel_tree(
            terminal_area(areas.panel, true),
            tab.panel_tree(),
            tab.panel_index(&target.source.focus),
            &|split, first, second| {
                tab.panel_tree()
                    .split_ratio(split, first, second)
                    .unwrap_or_default()
            },
        );
        if !projection.splits.iter().any(|split| {
            DrawnDivider::new(target.source.clone(), viewport, split).as_ref() == Some(target)
        }) {
            return Err(Reason::StaleView);
        }
        Ok((
            site,
            ClientLayoutAdjustment {
                terminal_context_id: target.source.context.clone(),
                tab_id: target.source.tab.clone(),
                workbench_revision: target.source.revision,
                selected: true,
                change: ClientLayoutChange::Split {
                    split: ClientSplitChange {
                        split_id: target.split.clone(),
                        first_slot_id: target.first.clone(),
                        second_slot_id: target.second.clone(),
                        previous_ratio: target.ratio,
                        ratio,
                    },
                },
            },
            before,
        ))
    }
}
