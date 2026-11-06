//! Workspace projection and identity-checked input over complete read-only checkpoints.
use super::{
    Choice, Mode, Picker, Row, Selection, SelectionOrigin, SidebarRow, SidebarTarget, SidebarWindow,
};
use orchbee_workbench_protocol::MetadataState;
use std::time::{Duration, Instant};

/// How long a row may hold what it settled on while it waits for Git. An answer normally takes about
/// 200 ms; a read that a commit interrupted is retried after 500 ms and takes as long again, so this
/// covers both. It stays far below the five-second budget of a Git read, because a hung read must not
/// freeze a row that already knows its new name and folder.
pub(crate) const SPACE_HOLD_BOUND: Duration = Duration::from_secs(1);

/// One row's hold and when it gives up. Checkpoints stay clock-free, so the Picker keeps this beside
/// them. A further directory change does not restart it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SpaceHold {
    choice: Choice,
    target: crate::client_context::ClientContextTarget,
    deadline: Instant,
    expired: bool,
}

impl Picker {
    /// The active command lane publishes topology independently of metadata polling.
    pub fn observe_native_navigation(
        &mut self,
        machine: Choice,
        navigation: Option<
            crate::machine_spaces::navigation_projection::SidebarNavigationProjection,
        >,
    ) -> bool {
        let navigation = navigation.or_else(|| {
            // Carrier loss revokes speculation, not already validated topology. Keep
            // its revision floor until endpoint replacement explicitly forgets it.
            let (owner, known) = self.native_navigation.as_ref()?;
            (owner == &machine).then(|| {
                crate::machine_spaces::navigation_projection::SidebarNavigationProjection {
                    snapshot: std::sync::Arc::clone(&known.snapshot),
                    pending: None,
                    leaving: known.leaving.clone(),
                }
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
    ) -> Option<&crate::machine_spaces::checkpoint::Checkpoint> {
        self.space_observations
            .iter()
            .find_map(|(choice, observation)| match observation {
                crate::machine_spaces::checkpoint::Observation::Ready(checkpoint)
                    if choice == machine =>
                {
                    Some(checkpoint.as_ref())
                }
                _ => None,
            })
    }

    pub(super) fn space_checkpoint(
        &self,
        machine: &Choice,
    ) -> Option<&crate::machine_spaces::checkpoint::Checkpoint> {
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
            .and_then(|(machine, native)| {
                // A drag's order shows only over the exact topology it moves, and never over an
                // admitted move, which the native projection shows itself.
                let source = self
                    .reorder_preview
                    .as_ref()
                    .filter(|preview| {
                        native.pending.is_none() && preview.snapshot == native.snapshot
                    })
                    .unwrap_or(native);
                crate::machine_spaces::checkpoint::Checkpoint::project_navigation(
                    source,
                    self.observed_space_checkpoint(machine),
                )
                .map(|checkpoint| (machine.clone(), std::sync::Arc::new(checkpoint)))
            });
    }

    /// Shows a sidebar drag's order, its canonical baseline with the move it shows, in place of
    /// the native one, around the entry it holds; `None` restores the native order. The window
    /// keeps its top row index while rows move under the pointer.
    pub(crate) fn preview_sidebar_reorder(
        &mut self,
        order: Option<crate::machine_spaces::navigation_projection::SidebarNavigationProjection>,
        held: Option<SidebarTarget>,
    ) -> bool {
        if self.reorder_preview == order && self.reorder_held == held {
            return false;
        }
        self.reorder_held = held;
        if order.is_some() {
            self.spaces.hold();
        }
        let selected = self.selection();
        let scroll = self.spaces.scroll;
        self.reorder_preview = order;
        self.rebuild_workspaces(selected);
        self.spaces.scroll = scroll;
        if self.reorder_preview.is_none() {
            self.spaces.release();
        }
        true
    }

    /// The Workspace a drag holds becomes current once the drag ends, dropped or cancelled. The
    /// sidebar keeps showing it as current until the switch settles, so the fill never returns
    /// to the previous Workspace in between.
    pub(crate) fn switch_to_held_workspace(&mut self, held: &SidebarTarget) -> bool {
        let Some(selection) = held.selection() else {
            return false;
        };
        if !self.activate_sidebar_workspace(&selection) {
            return false;
        }
        self.held_switch = Some(held.clone());
        true
    }

    /// The Workspace the sidebar shows as current in place of the displayed one: the one a drag
    /// holds, then the one a finished drag is still switching to.
    pub(crate) fn sidebar_current_override(&self) -> Option<&SidebarTarget> {
        self.reorder_held
            .as_ref()
            .filter(|held| matches!(held, SidebarTarget::Context { .. }))
            .or_else(|| {
                self.held_switch
                    .as_ref()
                    .filter(|_| self.direct_selection_pending())
            })
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
            let crate::workbench_projection::ClientContextLocation::Workspace {
                workspace_id, ..
            } = &row.target.location
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
        observations: Vec<(Choice, crate::machine_spaces::checkpoint::Observation)>,
    ) -> bool {
        use crate::machine_spaces::checkpoint::Observation;
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

    /// What a row keeps presenting while the Git facts of its new directory are unknown. No answer
    /// is coming for a source whose whole read keeps failing or a view that shows nothing from Git,
    /// and a row past its bound shows what it knows.
    pub(crate) fn space_held_row(&self, row: &Row) -> Option<&crate::machine_spaces::HeldRow> {
        let observed = self.space_checkpoint_row(row)?;
        observed.held.as_ref().filter(|_| {
            self.spaces.settings.needs_git()
                && !self.space_stale.contains(&row.choice)
                && !self.space_holds.iter().any(|hold| {
                    hold.expired && hold.choice == row.choice && hold.target == observed.target
                })
        })
    }

    /// Starts the bound of every new hold and ends the holds past it. Returns whether a row now
    /// paints something else.
    pub fn advance_space_holds(&mut self, now: Instant) -> bool {
        use crate::machine_spaces::checkpoint::Observation;
        let mut holds = Vec::new();
        let mut expired = false;
        for (choice, _) in self
            .space_observations
            .iter()
            .filter(|(_, observation)| matches!(observation, Observation::Ready(_)))
        {
            let rows = self
                .space_checkpoint(choice)
                .map_or(&[][..], |checkpoint| &checkpoint.rows);
            for row in rows.iter().filter(|row| row.held.is_some()) {
                let known = self
                    .space_holds
                    .iter()
                    .find(|hold| &hold.choice == choice && hold.target == row.target);
                let deadline = known.map_or(now + SPACE_HOLD_BOUND, |hold| hold.deadline);
                let past = now >= deadline;
                expired |= past && !known.is_some_and(|hold| hold.expired);
                holds.push(SpaceHold {
                    choice: choice.clone(),
                    target: row.target.clone(),
                    deadline,
                    expired: past,
                });
            }
        }
        self.space_holds = holds;
        expired
    }

    /// Workspaces whose current directory has no Git answer yet, for one read ahead of the sampling
    /// cooldown. Each Workspace and directory asks once: a repeated failure waits for sampling, and
    /// a machine before its first checkpoint reads everything anyway.
    pub fn take_space_git_demand(
        &mut self,
    ) -> Vec<(
        Choice,
        std::collections::BTreeSet<orchbee_workbench_protocol::WorkbenchWorkspaceId>,
    )> {
        use crate::machine_spaces::checkpoint::Observation;
        use crate::workbench_projection::ClientContextLocation;
        let mut unanswered = Vec::new();
        let mut demand = Vec::new();
        for (choice, _) in self
            .space_observations
            .iter()
            .filter(|(_, observation)| matches!(observation, Observation::Ready(_)))
        {
            let mut workspaces = std::collections::BTreeSet::new();
            let rows = self
                .space_checkpoint(choice)
                .map_or(&[][..], |checkpoint| &checkpoint.rows);
            for row in rows.iter().filter(|row| row.git.is_none()) {
                let (ClientContextLocation::Workspace { workspace_id, .. }, Some(path)) =
                    (&row.target.location, row.source_path())
                else {
                    continue;
                };
                let asked = (choice.clone(), workspace_id.clone(), path.clone());
                if !self.space_git_demanded.contains(&asked) {
                    workspaces.insert(workspace_id.clone());
                }
                unanswered.push(asked);
            }
            if !workspaces.is_empty() {
                demand.push((choice.clone(), workspaces));
            }
        }
        self.space_git_demanded = unanswered;
        demand
    }

    pub(crate) fn workspace_row_available(&self, row: &Row) -> bool {
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
    pub(crate) fn space_checkpoint_row(
        &self,
        row: &Row,
    ) -> Option<&crate::machine_spaces::checkpoint::Row> {
        let context = row.context.as_ref()?;
        self.space_checkpoint(&row.choice)
            .filter(|checkpoint| checkpoint.session_id == context.target.session_id)
            .and_then(|checkpoint| checkpoint.rows.iter().find(|r| r.target == context.target))
    }
    /// Where the drawn Workspace row that `target` names stands, while it shows a branch for it. A
    /// row without one proves nothing: the Server answers a folder outside Git and a detached HEAD
    /// alike, with no branch.
    pub(crate) fn sidebar_git_directory(
        &self,
        target: &crate::action_menu::MenuTarget,
    ) -> Option<orchbee_workbench_protocol::WorkbenchDirectoryPath> {
        use crate::action_menu::MenuTarget;
        let MenuTarget::Drawn { source, target } = target else {
            return None;
        };
        let MenuTarget::Workspace(location) = target.as_ref() else {
            return None;
        };
        self.space_checkpoint(&self.current)
            .filter(|checkpoint| checkpoint.session_id == source.session)?
            .rows
            .iter()
            .find(|row| &row.target.location == location)
            .filter(|row| {
                !row.git_stale && row.git.as_ref().is_some_and(|git| git.branch.is_some())
            })?
            .source_path()
            .cloned()
    }

    pub(crate) fn workspace_state(&self, row: &Row) -> Option<MetadataState> {
        self.workspace_agent_states(row).first().copied()
    }
    /// Every Agent state in one Workspace, most urgent first; empty without a complete checkpoint.
    pub(crate) fn workspace_agent_states(&self, row: &Row) -> Vec<MetadataState> {
        self.workspace_agent_rows(row)
            .into_iter()
            .map(crate::machine_agents::checkpoint::Row::display_state)
            .collect()
    }
    /// Every Agent in one Workspace, most urgent state first, as the Workspace's status line lists
    /// them; empty without a complete checkpoint.
    pub(crate) fn workspace_agent_rows(
        &self,
        row: &Row,
    ) -> Vec<&crate::machine_agents::checkpoint::Row> {
        let Some(context) = row.context.as_ref() else {
            return Vec::new();
        };
        let Some(crate::machine_agents::checkpoint::Observation::Ready(cp)) = self
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
        let mut agents = cp
            .rows
            .iter()
            .filter(|r| r.target.context == context.target)
            .collect::<Vec<_>>();
        agents.sort_by_key(|agent| urgency(agent.display_state()));
        agents
    }
    /// The attached machine's Agents in one Tab, summarized like a collapsed Workspace row.
    pub(crate) fn tab_agent(
        &self,
        tab: &orchbee_workbench_protocol::WorkbenchTabId,
    ) -> Option<TabAgent> {
        let Some(crate::machine_agents::checkpoint::Observation::Ready(cp)) = self
            .agent_observations
            .iter()
            .find(|(c, _)| c == &self.current)
            .map(|(_, o)| o)
        else {
            return None;
        };
        let rows = || cp.rows.iter().filter(|r| &r.target.tab == tab);
        Some(TabAgent {
            state: most_urgent(rows().map(crate::machine_agents::checkpoint::Row::display_state))?,
            unread: rows().any(|r| !r.seen),
        })
    }
    pub(crate) fn workspace_unread(
        &self,
        row: &Row,
        painted: impl Fn(&crate::machine_agents::Row) -> bool,
    ) -> bool {
        row.context.as_ref().is_some_and(|context| {
            self.agent_observations.iter().any(|(choice, observation)| {
                *choice == row.choice
                    && matches!(observation, crate::machine_agents::checkpoint::Observation::Ready(cp)
                        if cp.session_id == context.target.session_id
                            && cp.rows.iter().any(|agent| agent.target.context == context.target && !agent.seen && !painted(agent)))
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
