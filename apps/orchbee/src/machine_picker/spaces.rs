//! Workspace projection and identity-checked input over complete read-only checkpoints.
use super::{Choice, Mode, Picker, Row, Selection, SidebarRow, SidebarTarget, SidebarWindow};
use orchbee_workbench_protocol::MetadataState;

/// Complete physical Agent evidence, independent of sidebar disclosure and view filters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SidebarAgentCounts {
    pub(crate) total: usize,
    pub(crate) waiting: usize,
}

impl Picker {
    /// Count canonical Workspaces, including those hidden by a collapsed Project.
    pub(crate) fn sidebar_workspace_count(&self) -> usize {
        self.workspace_rows
            .iter()
            .filter(|row| {
                row.enabled
                    && row.context.as_ref().is_some_and(|context| {
                        matches!(
                            context.target.location,
                            crate::terminal_session::NativeContextLocation::Workspace { .. }
                        )
                    })
            })
            .count()
    }
    /// Counts for an exact Context or Project from the complete attention checkpoint.
    pub(crate) fn sidebar_agent_counts(
        &self,
        target: &SidebarTarget,
    ) -> Option<SidebarAgentCounts> {
        if !self.sidebar_target_exists(target) || matches!(target, SidebarTarget::Agent { .. }) {
            return None;
        }
        let contexts: std::collections::BTreeSet<_> = self
            .workspace_rows
            .iter()
            .filter_map(|row| {
                let context = row.context.as_ref()?;
                let belongs = row.enabled
                    && &row.choice == target.machine()
                    && match target {
                        SidebarTarget::Context { target, .. } => context.target == *target,
                        SidebarTarget::Project {
                            site_id,
                            project_id,
                            ..
                        } => {
                            context.target.site_id == *site_id
                                && context.project_id.as_ref() == Some(project_id)
                        }
                        SidebarTarget::Agent { .. } => false,
                    };
                belongs.then(|| super::navigation::context_identity(&row.choice, &context.target))
            })
            .collect();
        if contexts.is_empty() {
            return Some(SidebarAgentCounts::default());
        }
        let checkpoint = self
            .agent_observations
            .iter()
            .find_map(|(machine, observation)| match observation {
                crate::machine_agents::Observation::Ready(checkpoint)
                    if machine == target.machine() && &checkpoint.site_id == target.site_id() =>
                {
                    Some(checkpoint)
                }
                _ => None,
            })?;
        let observed_contexts = checkpoint
            .contexts
            .iter()
            .map(|context| super::navigation::context_identity(target.machine(), context))
            .collect();
        if !contexts.is_subset(&observed_contexts) {
            return None;
        }
        let mut counts = SidebarAgentCounts::default();
        for agent in checkpoint.rows.iter().filter(|row| {
            contexts.contains(&super::navigation::context_identity(
                target.machine(),
                &row.target.context,
            ))
        }) {
            counts.total += 1;
            counts.waiting += usize::from(agent.display_state() == MetadataState::Blocked);
        }
        Some(counts)
    }
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
        self.rebuild_workspaces(self.selection());
        true
    }
    #[cfg(test)]
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
    pub(crate) fn workspace_state(&self, row: &Row) -> Option<MetadataState> {
        let context = row.context.as_ref()?;
        let Some(crate::machine_agents::Observation::Ready(cp)) = self
            .agent_observations
            .iter()
            .find(|(c, _)| c == &row.choice)
            .map(|(_, o)| o)
        else {
            return None;
        };
        if cp.site_id != context.target.site_id || !cp.contexts.contains(&context.target) {
            return None;
        }
        // These are complete, attention-projected physical rows, before every view/source filter.
        let mut states = cp
            .rows
            .iter()
            .filter(|r| r.target.context == context.target)
            .map(|r| r.display_state());
        let first = states.next()?;
        let rank = |state| match state {
            MetadataState::Blocked => 0,
            MetadataState::Error => 1,
            MetadataState::Done => 2,
            MetadataState::Working => 3,
            MetadataState::Idle => 4,
            MetadataState::Unknown => 5,
        };
        Some(states.fold(first, |best, state| {
            if rank(state) < rank(best) {
                state
            } else {
                best
            }
        }))
    }
    pub(crate) fn workspace_unread(&self, row: &Row) -> bool {
        row.context.as_ref().is_some_and(|context| {
            self.agent_observations.iter().any(|(choice, observation)| {
                *choice == row.choice
                    && matches!(observation, crate::machine_agents::Observation::Ready(cp)
                        if cp.site_id == context.target.site_id
                            && cp.rows.iter().any(|agent| agent.target.context == context.target && !agent.seen))
            })
        })
    }
    pub(crate) fn current_workspace(&self, row: &Row) -> bool {
        row.choice == self.current
            && row
                .context
                .as_ref()
                .is_some_and(|c| self.current_context.as_ref() == Some(&c.target))
    }
    pub(crate) fn space_window_for_row_gaps(
        &self,
        rows: &[SidebarRow<'_>],
        heights: &[u16],
        gaps: &[u16],
        cells: u16,
    ) -> SidebarWindow {
        let current = self
            .sidebar_focused_target()
            .cloned()
            .or_else(|| {
                rows.iter().find_map(|row| match row {
                    SidebarRow::Agent { row, .. }
                        if row
                            .agent
                            .as_ref()
                            .is_some_and(|agent| self.current_agent(&row.choice, agent)) =>
                    {
                        row.sidebar_agent_target()
                    }
                    _ => None,
                })
            })
            .or_else(|| {
                self.workspace_rows
                    .iter()
                    .find(|row| self.current_workspace(row))
                    .and_then(Row::sidebar_target)
            });
        self.spaces.window_with_gaps(
            &heights[..heights.len().min(rows.len())],
            &gaps[..gaps.len().min(rows.len())],
            cells,
            current.and_then(|target| {
                let mut candidate = Some(target);
                while let Some(target) = candidate {
                    if let Some(position) = rows.iter().position(|row| row.target() == target) {
                        return Some(position);
                    }
                    candidate = self.sidebar_parent(&target);
                }
                None
            }),
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
    fn sidebar_workspace_index(&self, selected: &Selection) -> Option<usize> {
        if !self.available || !self.spaces.visible() || self.open {
            return None;
        }
        self.workspace_rows.iter().position(|r| {
            r.enabled
                && r.context.is_some()
                && &r.selection() == selected
                && r.sidebar_target().is_some_and(|target| {
                    self.sidebar_navigation_rows()
                        .iter()
                        .any(|row| row.target() == target)
                })
        })
    }
    pub(crate) fn activate_sidebar_workspace(&mut self, selected: &Selection) -> bool {
        if self.sidebar_workspace_index(selected).is_none()
            || !self.begin_selection(SelectionOrigin::Sidebar)
        {
            return false;
        }
        // The Session decides exact-current after its in-flight selection guards.
        // Selecting the displayed A while B is pending must retain the new A intent.
        self.cancel_sidebar_scroll();
        self.request = Some(selected.clone());
        self.error = None;
        self.invalidate();
        true
    }
}
impl Row {
    pub(crate) fn workspace_label(&self) -> Option<&str> {
        self.context.as_ref()?.workspace.as_deref()
    }
}
