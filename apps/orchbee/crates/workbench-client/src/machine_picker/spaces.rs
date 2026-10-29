//! Workspace projection and identity-checked input over complete read-only checkpoints.
use super::{Choice, Mode, Picker, Row, Selection, SelectionOrigin, SidebarRow, SidebarWindow};
use orchbee_workbench_protocol::MetadataState;

impl Picker {
    /// The active command lane publishes topology independently of metadata polling.
    pub fn observe_native_navigation(
        &mut self,
        machine: Choice,
        navigation: Option<crate::workbench_client::SidebarNavigationProjection>,
    ) -> bool {
        let navigation = navigation.or_else(|| {
            // Carrier loss revokes speculation, not already validated topology. Keep
            // its revision floor until endpoint replacement explicitly forgets it.
            let (owner, known) = self.native_navigation.as_ref()?;
            (owner == &machine).then(|| crate::workbench_client::SidebarNavigationProjection {
                snapshot: std::sync::Arc::clone(&known.snapshot),
                pending: None,
            })
        });
        let next = navigation.map(|navigation| (machine, navigation));
        if self.native_navigation == next {
            return false;
        }
        let selected = self.selection();
        self.native_navigation = next;
        self.rebuild_workspaces(selected);
        true
    }

    fn observed_space_checkpoint(
        &self,
        machine: &Choice,
    ) -> Option<&crate::machine_spaces::Checkpoint> {
        self.space_observations
            .iter()
            .find_map(|(choice, observation)| match observation {
                crate::machine_spaces::Observation::Ready(checkpoint) if choice == machine => {
                    Some(checkpoint.as_ref())
                }
                _ => None,
            })
    }

    pub(super) fn space_checkpoint(
        &self,
        machine: &Choice,
    ) -> Option<&crate::machine_spaces::Checkpoint> {
        self.space_projection
            .as_ref()
            .filter(|(choice, _)| choice == machine)
            .map(|(_, checkpoint)| checkpoint.as_ref())
            .or_else(|| self.observed_space_checkpoint(machine))
    }

    pub(super) fn rebuild_space_projection(&mut self) {
        self.space_projection = self
            .native_navigation
            .as_ref()
            .and_then(|(machine, source)| {
                crate::machine_spaces::Checkpoint::project_navigation(
                    source,
                    self.observed_space_checkpoint(machine),
                )
                .map(|checkpoint| (machine.clone(), std::sync::Arc::new(checkpoint)))
            });
    }

    /// A new foreground Workspace intent returns the viewport to following its current target.
    pub(crate) fn reveal_sidebar_current(&mut self) {
        self.spaces.reveal();
    }

    /// A same-Session canonical topology at this revision supersedes a creation-only projection.
    pub(crate) fn workspace_checkpoint_covers(
        &self,
        session: &orchbee_workbench_protocol::SessionId,
        revision: orchbee_workbench_protocol::WorkbenchRevision,
    ) -> bool {
        self.space_checkpoint(&self.current)
            .is_some_and(|checkpoint| {
                &checkpoint.session_id == session && checkpoint.revision >= revision
            })
    }
    /// The Project, or `Some(None)` for Recents, that current canonical membership gives one
    /// Workspace; `None` while the checkpoint does not place it.
    pub(crate) fn workspace_checkpoint_group(
        &self,
        session: &orchbee_workbench_protocol::SessionId,
        workspace: &orchbee_workbench_protocol::WorkbenchWorkspaceId,
    ) -> Option<Option<&orchbee_workbench_protocol::WorkbenchProjectId>> {
        let checkpoint = self.space_checkpoint(&self.current)?;
        if &checkpoint.session_id != session {
            return None;
        }
        checkpoint.rows.iter().find_map(|row| {
            let crate::workbench_client::ClientContextLocation::Workspace { workspace_id, .. } =
                &row.target.location
            else {
                return None;
            };
            (workspace_id == workspace).then_some(row.project_id.as_ref())
        })
    }
    pub fn wants_spaces(&self) -> bool {
        self.spaces.visible() || (self.open && self.mode == Mode::Workspaces)
    }
    pub fn space_settings(&self) -> &orchbee_local_runtime::SpaceSidebarSettings {
        &self.spaces.settings
    }
    pub(crate) fn configure_space_sidebar(
        &mut self,
        settings: orchbee_local_runtime::SpaceSidebarSettings,
    ) {
        if self.spaces.settings.as_ref() != &settings {
            self.spaces.settings = std::sync::Arc::new(settings);
            self.spaces.reset();
        }
    }
    pub fn observe_spaces(
        &mut self,
        observations: Vec<(Choice, crate::machine_spaces::Observation)>,
    ) -> bool {
        use crate::machine_spaces::Observation;
        if self.space_updates == observations
            && (!observations.is_empty()
                || self.wants_spaces()
                || self.space_observations.is_empty())
        {
            return false;
        }
        self.space_updates = observations.clone();
        let mut stale = Vec::new();
        let mut next = Vec::new();
        for (choice, observation) in observations {
            let known = self
                .space_observations
                .iter()
                .find_map(|(machine, observation)| match observation {
                    Observation::Ready(checkpoint) if machine == &choice => Some(checkpoint),
                    _ => None,
                });
            let observation = match (observation, known) {
                (Observation::Ready(checkpoint), Some(known))
                    if checkpoint.session_id == known.session_id
                        && (checkpoint.revision < known.revision
                            || checkpoint.metadata_revision < known.metadata_revision) =>
                {
                    stale.push(choice.clone());
                    Observation::Ready(known.clone())
                }
                (Observation::Ready(checkpoint), Some(known)) => {
                    Observation::Ready(std::sync::Arc::new(checkpoint.retaining_git(known)))
                }
                (Observation::Loading | Observation::Unavailable, Some(known)) => {
                    stale.push(choice.clone());
                    Observation::Ready(known.clone())
                }
                (observation, _) => observation,
            };
            next.push((choice, observation));
        }
        // A temporarily unreachable source is not a canonical deletion. Removing or replacing its
        // saved endpoint explicitly retires the checkpoint through the catalog owner.
        if self.wants_spaces() {
            for (choice, observation) in &self.space_observations {
                if matches!(observation, Observation::Ready(_))
                    && !next.iter().any(|(machine, _)| machine == choice)
                    && self
                        .rows
                        .iter()
                        .any(|row| row.enabled && &row.choice == choice)
                {
                    next.push((choice.clone(), observation.clone()));
                    stale.push(choice.clone());
                }
            }
        }
        if self.space_observations == next && self.space_stale == stale {
            return false;
        }
        let selected = self.selection();
        self.space_observations = next;
        self.space_stale = stale;
        self.rebuild_workspaces(selected);
        true
    }
    /// Endpoint replacement revokes retained presentation before its replacement can answer.
    pub fn forget_space_observations(&mut self, machine: &Choice) {
        let selected = self.selection();
        if self
            .native_navigation
            .as_ref()
            .is_some_and(|(choice, _)| choice == machine)
        {
            self.native_navigation = None;
        }
        self.space_observations
            .retain(|(choice, _)| choice != machine);
        self.space_updates.retain(|(choice, _)| choice != machine);
        self.space_stale.retain(|choice| choice != machine);
        self.rebuild_workspaces(selected);
    }

    pub(crate) fn space_checkpoint_stale(&self, row: &Row) -> bool {
        self.space_stale.contains(&row.choice)
            || self
                .space_checkpoint_row(row)
                .is_some_and(|row| row.git_stale)
    }

    pub(super) fn workspace_row_available(&self, row: &Row) -> bool {
        row.enabled
            && (row.context.is_none()
                || row.choice == Choice::Local
                || self.rows.iter().any(|machine| {
                    machine.choice == row.choice
                        && matches!(machine.observation, Some(super::Observation::Online(_)))
                }))
    }
    #[cfg(test)]
    pub(crate) fn space_rows(&self) -> &[Row] {
        &self.workspace_rows
    }
    pub(crate) fn space_checkpoint_row(&self, row: &Row) -> Option<&crate::machine_spaces::Row> {
        let context = row.context.as_ref()?;
        self.space_checkpoint(&row.choice)
            .filter(|checkpoint| checkpoint.session_id == context.target.session_id)
            .and_then(|checkpoint| checkpoint.rows.iter().find(|r| r.target == context.target))
    }
    pub(crate) fn workspace_state(&self, row: &Row) -> Option<MetadataState> {
        self.workspace_agent_states(row).first().copied()
    }
    /// Every Agent state in one Workspace, most urgent first; empty without a complete checkpoint.
    pub(crate) fn workspace_agent_states(&self, row: &Row) -> Vec<MetadataState> {
        let Some(context) = row.context.as_ref() else {
            return Vec::new();
        };
        let Some(crate::machine_agents::Observation::Ready(cp)) = self
            .agent_observations
            .iter()
            .find(|(c, _)| c == &row.choice)
            .map(|(_, o)| o)
        else {
            return Vec::new();
        };
        if cp.session_id != context.target.session_id || !cp.contexts.contains(&context.target) {
            return Vec::new();
        }
        // These are complete, attention-projected physical rows, before every view/source filter.
        let mut states = cp
            .rows
            .iter()
            .filter(|r| r.target.context == context.target)
            .map(crate::machine_agents::Row::display_state)
            .collect::<Vec<_>>();
        states.sort_by_key(|state| urgency(*state));
        states
    }
    /// The attached machine's Agents in one Tab, summarized like a collapsed Workspace row.
    pub(crate) fn tab_agent(
        &self,
        tab: &orchbee_workbench_protocol::WorkbenchTabId,
    ) -> Option<TabAgent> {
        let Some(crate::machine_agents::Observation::Ready(cp)) = self
            .agent_observations
            .iter()
            .find(|(c, _)| c == &self.current)
            .map(|(_, o)| o)
        else {
            return None;
        };
        let rows = || cp.rows.iter().filter(|r| &r.target.tab == tab);
        Some(TabAgent {
            state: most_urgent(rows().map(crate::machine_agents::Row::display_state))?,
            unread: rows().any(|r| !r.seen),
        })
    }
    pub(crate) fn workspace_unread(&self, row: &Row) -> bool {
        row.context.as_ref().is_some_and(|context| {
            self.agent_observations.iter().any(|(choice, observation)| {
                *choice == row.choice
                    && matches!(observation, crate::machine_agents::Observation::Ready(cp)
                        if cp.session_id == context.target.session_id
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
    #[cfg(test)]
    pub(crate) fn space_window_for_row_gaps(
        &self,
        rows: &[SidebarRow<'_>],
        heights: &[u16],
        gaps: &[u16],
        cells: u16,
    ) -> SidebarWindow {
        self.space_window_with_creation(
            rows,
            &heights[..heights.len().min(rows.len())],
            &gaps[..gaps.len().min(rows.len())],
            cells,
            None,
            None,
        )
    }

    /// Measures an inert creation slot in its eventual canonical order, using
    /// the same viewport anchor as the Workspace that will replace it.
    pub(crate) fn space_window_with_creation(
        &self,
        rows: &[SidebarRow<'_>],
        heights: &[u16],
        gaps: &[u16],
        cells: u16,
        insertion: Option<usize>,
        creation_current: Option<usize>,
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
        let current = current.and_then(|target| {
            let mut candidate = Some(target);
            while let Some(target) = candidate {
                if let Some(position) = rows.iter().position(|row| row.target() == target) {
                    return Some(
                        position + usize::from(insertion.is_some_and(|slot| position >= slot)),
                    );
                }
                candidate = self.sidebar_parent(&target);
            }
            None
        });
        self.spaces.window_with_gaps(
            heights,
            gaps,
            cells,
            creation_current.or(current),
            insertion,
        )
    }
    pub(crate) fn queue_space_scroll(&mut self, window: SidebarWindow, up: bool) {
        if self.available && self.spaces.visible() && !self.open {
            self.spaces.queue(window, up);
        }
    }
    pub(crate) fn show_workspaces(&mut self) {
        self.cancel_sidebar_scroll();
        self.workspace_query.clear();
        self.filter_workspaces(None);
        self.show();
        self.set_mode(Mode::Workspaces);
        self.selected = self
            .workspace_matches
            .iter()
            .position(|r| self.current_workspace(r))
            .unwrap_or(0);
    }
    fn sidebar_workspace_index(&self, selected: &Selection) -> Option<usize> {
        if !self.available || !self.spaces.visible() || self.open {
            return None;
        }
        self.workspace_rows.iter().position(|r| {
            self.workspace_row_available(r)
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

        true
    }
}
impl Row {
    pub(crate) fn workspace_label(&self) -> Option<&str> {
        self.context.as_ref()?.workspace.as_deref()
    }
}

/// One Tab's Agent cue: the most urgent sidebar state and whether any Agent is unread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TabAgent {
    pub(crate) state: MetadataState,
    pub(crate) unread: bool,
}

/// Blocked, error, done, working, then idle: the order a person needs to act on them.
pub(crate) const fn urgency(state: MetadataState) -> u8 {
    match state {
        MetadataState::Blocked => 0,
        MetadataState::Error => 1,
        MetadataState::Done => 2,
        MetadataState::Working => 3,
        MetadataState::Idle => 4,
        MetadataState::Unknown => 5,
    }
}

pub(crate) fn most_urgent(states: impl Iterator<Item = MetadataState>) -> Option<MetadataState> {
    states.min_by_key(|state| urgency(*state))
}
