//! Workspace projection and identity-checked input over complete read-only checkpoints.
use super::*;
use orchbee_workbench_protocol::MetadataState;
impl Picker {
    pub(crate) fn wants_spaces(&self) -> bool {
        self.spaces.visible() || (self.open && self.mode == Mode::Workspaces)
    }
    pub(crate) fn space_settings(
        &self,
    ) -> &orchbee_local_site_runtime::WindowsSpaceSidebarSettings {
        &self.spaces.settings
    }
    pub(crate) fn configure_space_sidebar(
        &mut self,
        settings: orchbee_local_site_runtime::WindowsSpaceSidebarSettings,
    ) {
        if self.spaces.settings.as_ref() != &settings {
            self.spaces.settings = std::sync::Arc::new(settings);
            self.spaces.reset();
            self.invalidate();
        }
    }
    pub(crate) fn observe_spaces(
        &mut self,
        observations: Vec<(Choice, crate::machine_spaces::Observation)>,
    ) -> bool {
        if self.space_observations == observations {
            return false;
        }
        self.space_observations = observations;
        self.cancel_sidebar_scroll();
        self.invalidate();
        true
    }
    pub(crate) fn space_rows(&self) -> &[Row] {
        &self.workspace_rows
    }
    pub(crate) fn space_checkpoint_row(&self, row: &Row) -> Option<&crate::machine_spaces::Row> {
        let context = row.context.as_ref()?;
        self.space_observations
            .iter()
            .find(|(choice, _)| choice == &row.choice)
            .and_then(|(_, o)| match o {
                crate::machine_spaces::Observation::Ready(cp)
                    if cp.site_id == context.target.site_id =>
                {
                    cp.rows.iter().find(|r| r.target == context.target)
                }
                _ => None,
            })
    }
    pub(crate) fn workspace_state(&self, row: &Row) -> MetadataState {
        let Some(context) = row.context.as_ref() else {
            return MetadataState::Unknown;
        };
        let Some(crate::machine_agents::Observation::Ready(cp)) = self
            .agent_observations
            .iter()
            .find(|(c, _)| c == &row.choice)
            .map(|(_, o)| o)
        else {
            return MetadataState::Unknown;
        };
        if cp.site_id != context.target.site_id || !cp.contexts.contains(&context.target) {
            return MetadataState::Unknown;
        }
        // These are complete, attention-projected physical rows, before every view/source filter.
        let mut states = cp
            .rows
            .iter()
            .filter(|r| r.target.context == context.target)
            .map(|r| r.display_state());
        let Some(first) = states.next() else {
            return MetadataState::Idle;
        };
        let rank = |state| match state {
            MetadataState::Blocked => 0,
            MetadataState::Done => 1,
            MetadataState::Working => 2,
            MetadataState::Idle => 3,
            MetadataState::Unknown => 4,
        };
        states.fold(first, |best, state| {
            if rank(state) < rank(best) {
                state
            } else {
                best
            }
        })
    }
    pub(crate) fn current_workspace(&self, row: &Row) -> bool {
        row.choice == self.current
            && row
                .context
                .as_ref()
                .is_some_and(|c| self.current_context.as_ref() == Some(&c.target))
    }
    pub(crate) fn space_window_for_heights(
        &self,
        heights: &[u16],
        cells: u16,
        gap: u16,
    ) -> SidebarWindow {
        self.spaces.window(
            &heights[..heights.len().min(self.workspace_rows.len())],
            cells,
            gap,
            self.workspace_rows
                .iter()
                .position(|r| self.current_workspace(r)),
        )
    }
    pub(crate) fn queue_space_scroll(&mut self, window: SidebarWindow, up: bool) {
        if self.available && self.spaces.visible() && !self.open {
            self.spaces.queue(window, up);
        }
    }
    pub(crate) fn show_workspaces(&mut self) {
        self.cancel_sidebar_scroll();
        self.show();
        self.set_mode(Mode::Workspaces);
        self.selected = self
            .workspace_rows
            .iter()
            .position(|r| self.current_workspace(r))
            .unwrap_or(0);
    }
    pub(crate) fn activate_sidebar_workspace(&mut self, selected: &Selection) -> bool {
        if !self.available || !self.spaces.visible() || self.open {
            return false;
        }
        let Some(index) = self
            .workspace_rows
            .iter()
            .position(|r| r.enabled && r.context.is_some() && &r.selection() == selected)
        else {
            return false;
        };
        let current = self.current_workspace(&self.workspace_rows[index]);
        self.show_workspaces();
        self.selected = index;
        if current {
            self.open = false;
            self.request = None;
            self.invalidate();
        } else {
            self.request = Some(selected.clone());
        }
        true
    }
}
impl Row {
    pub(crate) fn workspace_label(&self) -> Option<&str> {
        self.context.as_ref()?.workspace.as_deref()
    }
}
