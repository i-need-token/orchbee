//! Sidebar hierarchy and keyboard focus over the Picker's existing canonical rows.
use super::{
    Choice, Context, KeyCode, KeyEvent, KeyKind, Modifiers, NativeContextTarget, Picker, Row,
    Selection,
};
use crate::{
    client_view::Binding, terminal_session::NativeContextLocation, tui_layout::SidebarWidthChange,
};
use orchbee_workbench_protocol::{SiteId, WorkbenchProjectId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SidebarTarget {
    Project {
        machine: Choice,
        site_id: SiteId,
        project_id: WorkbenchProjectId,
    },
    Context {
        machine: Choice,
        target: NativeContextTarget,
    },
    Agent {
        machine: Choice,
        target: Box<crate::machine_agents::Target>,
    },
}
impl SidebarTarget {
    pub(crate) fn machine(&self) -> &Choice {
        match self {
            Self::Project { machine, .. }
            | Self::Context { machine, .. }
            | Self::Agent { machine, .. } => machine,
        }
    }
    pub(crate) fn site_id(&self) -> &SiteId {
        match self {
            Self::Project { site_id, .. } => site_id,
            Self::Context { target, .. } => &target.site_id,
            Self::Agent { target, .. } => &target.context.site_id,
        }
    }
}

pub(crate) enum SidebarRow<'a> {
    Project {
        machine: &'a Choice,
        site_id: &'a SiteId,
        project_id: &'a WorkbenchProjectId,
        label: &'a str,
    },
    Context {
        row: &'a Row,
        has_agents: bool,
    },
    Agent {
        row: &'a Row,
        /// Zero-based position in the global eligible Agent projection.
        ordinal: usize,
    },
}
impl SidebarRow<'_> {
    pub(crate) fn target(&self) -> SidebarTarget {
        match self {
            Self::Project {
                machine,
                site_id,
                project_id,
                ..
            } => SidebarTarget::Project {
                machine: (*machine).clone(),
                site_id: (*site_id).clone(),
                project_id: (*project_id).clone(),
            },
            Self::Context { row, .. } => row
                .sidebar_target()
                .expect("a sidebar Context row has its canonical target"),
            Self::Agent { row, .. } => row
                .sidebar_agent_target()
                .expect("a sidebar Agent row has its canonical target"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarCommand {
    NewWorkspace,
    Search,
    Previous,
    Next,
    First,
    Last,
    Expand,
    Collapse,
    Activate,
    OpenMenu,
    Narrower,
    Wider,
    ResetWidth,
    Help,
    Exit,
}

pub(crate) const SIDEBAR_BINDINGS: &[(Binding, SidebarCommand)] = &[
    (
        Binding::new(KeyCode::Char('n'), Modifiers::NONE.with_shift()),
        SidebarCommand::NewWorkspace,
    ),
    (
        Binding::new(KeyCode::Char('/'), Modifiers::NONE),
        SidebarCommand::Search,
    ),
    (
        Binding::new(KeyCode::Up, Modifiers::NONE),
        SidebarCommand::Previous,
    ),
    (
        Binding::new(KeyCode::Down, Modifiers::NONE),
        SidebarCommand::Next,
    ),
    (
        Binding::new(KeyCode::Home, Modifiers::NONE),
        SidebarCommand::First,
    ),
    (
        Binding::new(KeyCode::End, Modifiers::NONE),
        SidebarCommand::Last,
    ),
    (
        Binding::new(KeyCode::Right, Modifiers::NONE),
        SidebarCommand::Expand,
    ),
    (
        Binding::new(KeyCode::Left, Modifiers::NONE),
        SidebarCommand::Collapse,
    ),
    (
        Binding::new(KeyCode::Enter, Modifiers::NONE),
        SidebarCommand::Activate,
    ),
    (
        Binding::new(KeyCode::F(2), Modifiers::NONE),
        SidebarCommand::OpenMenu,
    ),
    (
        Binding::new(KeyCode::Left, Modifiers::NONE.with_shift()),
        SidebarCommand::Narrower,
    ),
    (
        Binding::new(KeyCode::Right, Modifiers::NONE.with_shift()),
        SidebarCommand::Wider,
    ),
    (
        Binding::new(KeyCode::Char('0'), Modifiers::NONE),
        SidebarCommand::ResetWidth,
    ),
    (
        Binding::new(KeyCode::F(1), Modifiers::NONE),
        SidebarCommand::Help,
    ),
    (
        Binding::new(KeyCode::Esc, Modifiers::NONE),
        SidebarCommand::Exit,
    ),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SidebarIntent {
    None,
    NewWorkspace,
    Search,
    Activate(Selection),
    OpenMenu(SidebarTarget),
    Resize(SidebarWidthChange),
    Help,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Focus {
    target: SidebarTarget,
    position: usize,
    // Last accepted ancestry is used only when the focused identity disappears.
    // Existing Workspaces always resolve their possibly changed Project from live metadata.
    ancestors: Vec<SidebarTarget>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct SidebarNavigation {
    active: bool,
    focus: Option<Focus>,
    collapsed: Vec<SidebarTarget>,
    // Every Context is expanded by default; retain independent explicit collapse choices.
    collapsed_agents: Vec<SidebarTarget>,
}

impl Row {
    pub(crate) fn sidebar_target(&self) -> Option<SidebarTarget> {
        Some(SidebarTarget::Context {
            machine: self.choice.clone(),
            target: self.context.as_ref()?.target.clone(),
        })
    }
    pub(crate) fn sidebar_agent_target(&self) -> Option<SidebarTarget> {
        Some(SidebarTarget::Agent {
            machine: self.choice.clone(),
            target: Box::new(self.agent.as_ref()?.target.clone()),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum MachineIdentity<'a> {
    Local,
    Remote(&'a str),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ContextLocationIdentity<'a> {
    Standalone(&'a str),
    Workspace {
        workspace: &'a str,
        terminal: &'a str,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ContextIdentity<'a> {
    machine: MachineIdentity<'a>,
    site: &'a str,
    location: ContextLocationIdentity<'a>,
}

/// Shared join key for Agent rows and counts; Project membership is presentation metadata.
pub(super) fn context_identity<'a>(
    machine: &'a Choice,
    target: &'a NativeContextTarget,
) -> ContextIdentity<'a> {
    ContextIdentity {
        machine: match machine {
            Choice::Local => MachineIdentity::Local,
            Choice::Remote(id) => MachineIdentity::Remote(id.as_str()),
        },
        site: target.site_id.as_str(),
        location: match &target.location {
            NativeContextLocation::Standalone(id) => {
                ContextLocationIdentity::Standalone(id.as_str())
            }
            NativeContextLocation::Workspace {
                workspace_id,
                terminal_context_id,
            } => ContextLocationIdentity::Workspace {
                workspace: workspace_id.as_str(),
                terminal: terminal_context_id.as_str(),
            },
        },
    }
}

fn append_context<'a>(
    navigation: &SidebarNavigation,
    agent_children: &std::collections::BTreeMap<ContextIdentity<'a>, Vec<(usize, &'a Row)>>,
    entries: &mut Vec<SidebarRow<'a>>,
    row: &'a Row,
) {
    let Some(target) = row.sidebar_target() else {
        return;
    };
    let context = row
        .context
        .as_ref()
        .expect("a sidebar Context row has canonical Context data");
    let children = agent_children.get(&context_identity(&row.choice, &context.target));
    entries.push(SidebarRow::Context {
        row,
        has_agents: children.is_some_and(|children| !children.is_empty()),
    });
    if navigation.collapsed_agents.contains(&target) {
        return;
    }
    if let Some(children) = children {
        entries.extend(children.iter().map(|(ordinal, row)| SidebarRow::Agent {
            row,
            ordinal: *ordinal,
        }));
    }
}

impl Picker {
    pub(super) fn checkpoint_contexts(&self, machine: &Choice) -> Option<Vec<Context>> {
        let crate::machine_spaces::Observation::Ready(checkpoint) = &self
            .space_observations
            .iter()
            .find(|(choice, _)| choice == machine)?
            .1
        else {
            return None;
        };
        Some(
            checkpoint
                .rows
                .iter()
                .map(|row| Context {
                    target: row.target.clone(),
                    project_id: row.project_id.clone(),
                    workspace: row.label.clone(),
                    project: row.project_id.as_ref().and_then(|project_id| {
                        checkpoint
                            .projects
                            .iter()
                            .find(|project| &project.project_id == project_id)
                            .map(|project| project.label.clone())
                    }),
                })
                .collect(),
        )
    }

    pub(crate) fn sidebar_navigation_rows(&self) -> Vec<SidebarRow<'_>> {
        // Join the accepted Agent projection to its exact endpoint + Site + Context once.
        // Labels and physical placement ordinals are presentation data, never ownership keys.
        let mut agent_children = std::collections::BTreeMap::new();
        let mut ordinal = 0;
        for row in self.agent_rows.iter().filter(|row| row.enabled) {
            let Some(agent) = row.agent.as_ref() else {
                continue;
            };
            agent_children
                .entry(context_identity(&row.choice, &agent.target.context))
                .or_insert_with(Vec::new)
                .push((ordinal, row));
            ordinal += 1;
        }
        let mut entries = Vec::new();
        for machine in self.rows.iter().filter(|machine| machine.enabled) {
            let mut projects = Vec::new();
            let mut known = std::collections::BTreeSet::new();
            let mut children = std::collections::BTreeMap::new();
            if let Some((_, crate::machine_spaces::Observation::Ready(checkpoint))) = self
                .space_observations
                .iter()
                .find(|(choice, _)| choice == &machine.choice)
            {
                for project in checkpoint.projects.iter() {
                    known.insert((checkpoint.site_id.as_str(), project.project_id.as_str()));
                    projects.push((
                        &machine.choice,
                        &checkpoint.site_id,
                        &project.project_id,
                        project.label.as_str(),
                    ));
                }
            }
            for row in self
                .workspace_rows
                .iter()
                .filter(|row| row.enabled && row.choice == machine.choice)
            {
                let Some(context) = row.context.as_ref() else {
                    continue;
                };
                match &context.project_id {
                    None => append_context(&self.navigation, &agent_children, &mut entries, row),
                    Some(project_id) => {
                        let key = (context.target.site_id.as_str(), project_id.as_str());
                        children.entry(key).or_insert_with(Vec::new).push(row);
                        if known.insert(key) {
                            projects.push((
                                &machine.choice,
                                &context.target.site_id,
                                project_id,
                                context.project.as_deref().unwrap_or(""),
                            ));
                        }
                    }
                }
            }
            for (machine, site_id, project_id, label) in projects {
                let target = SidebarTarget::Project {
                    machine: machine.clone(),
                    site_id: site_id.clone(),
                    project_id: project_id.clone(),
                };
                let expanded = self.sidebar_project_expanded(&target);
                entries.push(SidebarRow::Project {
                    machine,
                    site_id,
                    project_id,
                    label,
                });
                if expanded
                    && let SidebarTarget::Project {
                        site_id,
                        project_id,
                        ..
                    } = &target
                    && let Some(rows) = children.get(&(site_id.as_str(), project_id.as_str()))
                {
                    for row in rows {
                        append_context(&self.navigation, &agent_children, &mut entries, row);
                    }
                }
            }
        }
        entries
    }

    pub(crate) fn sidebar_target_exists(&self, target: &SidebarTarget) -> bool {
        if !self
            .rows
            .iter()
            .any(|row| row.enabled && &row.choice == target.machine())
        {
            return false;
        }
        match target {
            SidebarTarget::Context { machine, target } => self.workspace_rows.iter().any(|row| {
                &row.choice == machine && row.context.as_ref().is_some_and(|context| &context.target == target)
            }),
            SidebarTarget::Project { machine, site_id, project_id } => {
                self.space_observations.iter().any(|(choice, observation)| {
                    choice == machine && matches!(observation,
                        crate::machine_spaces::Observation::Ready(checkpoint)
                            if &checkpoint.site_id == site_id && checkpoint.projects.iter().any(|project| &project.project_id == project_id))
                }) || self.workspace_rows.iter().any(|row| {
                    &row.choice == machine && row.context.as_ref().is_some_and(|context| {
                        &context.target.site_id == site_id && context.project_id.as_ref() == Some(project_id)
                    })
                })
            }
            SidebarTarget::Agent { machine, target } => {
                self.workspace_rows.iter().any(|row| {
                    row.enabled && &row.choice == machine
                        && row.context.as_ref().is_some_and(|context| context.target == target.context)
                }) && self.agent_rows.iter().any(|row| {
                    row.enabled
                        && &row.choice == machine
                        && row.agent.as_ref().is_some_and(|agent| &agent.target == target.as_ref())
                })
            }
        }
    }
    pub(crate) fn sidebar_project_expanded(&self, target: &SidebarTarget) -> bool {
        !self.navigation.collapsed.contains(target)
    }
    /// Resolve organizational ancestry from current metadata, never from terminal identity.
    pub(crate) fn sidebar_parent(&self, target: &SidebarTarget) -> Option<SidebarTarget> {
        match target {
            SidebarTarget::Agent { machine, target } => Some(SidebarTarget::Context {
                machine: machine.clone(),
                target: target.context.clone(),
            }),
            SidebarTarget::Context { machine, target } => {
                let context = self.workspace_rows.iter().find_map(|row| {
                    row.context
                        .as_ref()
                        .filter(|context| &row.choice == machine && &context.target == target)
                })?;
                Some(SidebarTarget::Project {
                    machine: machine.clone(),
                    site_id: target.site_id.clone(),
                    project_id: context.project_id.clone()?,
                })
            }
            SidebarTarget::Project { .. } => None,
        }
    }
    fn sidebar_ancestors(&self, target: &SidebarTarget) -> Vec<SidebarTarget> {
        let mut result = Vec::new();
        let mut parent = self.sidebar_parent(target);
        while let Some(target) = parent {
            parent = self.sidebar_parent(&target);
            result.push(target);
        }
        result
    }
    pub(crate) fn sidebar_context_has_agents(&self, target: &SidebarTarget) -> bool {
        let SidebarTarget::Context { machine, target } = target else {
            return false;
        };
        self.agent_rows.iter().any(|row| {
            row.enabled
                && &row.choice == machine
                && row
                    .agent
                    .as_ref()
                    .is_some_and(|agent| agent.target.context == *target)
        })
    }
    pub(crate) fn sidebar_context_agents_expanded(&self, target: &SidebarTarget) -> bool {
        self.sidebar_target_exists(target)
            && self.sidebar_context_has_agents(target)
            && !self.navigation.collapsed_agents.contains(target)
    }
    fn sidebar_context_visible(&self, target: &SidebarTarget) -> bool {
        matches!(target, SidebarTarget::Context { .. })
            && self.sidebar_target_exists(target)
            && self
                .sidebar_parent(target)
                .is_none_or(|parent| self.sidebar_project_expanded(&parent))
    }
    pub(crate) fn sidebar_navigation_active(&self) -> bool {
        self.navigation.active
    }
    pub(crate) fn sidebar_focused_target(&self) -> Option<&SidebarTarget> {
        self.navigation
            .active
            .then_some(self.navigation.focus.as_ref())
            .flatten()
            .map(|focus| &focus.target)
    }
    pub(crate) fn enter_sidebar_navigation(&mut self) -> bool {
        if !self.available || self.open {
            return false;
        }
        self.navigation.active = true;
        self.reconcile_sidebar_navigation();
        self.spaces.reveal();
        true
    }
    pub(crate) fn leave_sidebar_navigation(&mut self) {
        self.navigation.active = false;
        self.spaces.scroll = None;
    }
    fn focus_sidebar_position(&mut self, index: usize) {
        let targets = self.sidebar_navigation_rows();
        self.navigation.focus = targets.get(index).map(|row| Focus {
            target: row.target(),
            position: index,
            ancestors: self.sidebar_ancestors(&row.target()),
        });
        self.spaces.reveal();
    }
    pub(crate) fn focus_sidebar_target(&mut self, target: &SidebarTarget) -> bool {
        if !self.available || self.open || !self.spaces.visible() {
            return false;
        }
        let Some(position) = self
            .sidebar_navigation_rows()
            .iter()
            .position(|row| &row.target() == target)
        else {
            return false;
        };
        self.navigation.active = true;
        self.focus_sidebar_position(position);
        true
    }
    pub(crate) fn toggle_sidebar_project(&mut self, target: &SidebarTarget) -> bool {
        if !self.available
            || self.open
            || !matches!(target, SidebarTarget::Project { .. })
            || !self.sidebar_target_exists(target)
        {
            return false;
        }
        if let Some(index) = self
            .navigation
            .collapsed
            .iter()
            .position(|held| held == target)
        {
            self.navigation.collapsed.remove(index);
        } else {
            self.navigation.collapsed.push(target.clone());
        }
        self.reconcile_sidebar_navigation();
        self.spaces.reveal();
        true
    }
    pub(crate) fn toggle_sidebar_context_agents(&mut self, target: &SidebarTarget) -> bool {
        if !self.available
            || self.open
            || !self.sidebar_context_visible(target)
            || !self.sidebar_context_has_agents(target)
        {
            return false;
        }
        // A wheel packet is tied to the row geometry of the frame that published it.
        // Preserve the settled manual position while discarding work from that old tree.
        self.spaces.scroll = None;
        if let Some(index) = self
            .navigation
            .collapsed_agents
            .iter()
            .position(|held| held == target)
        {
            self.navigation.collapsed_agents.remove(index);
        } else {
            self.navigation.collapsed_agents.push(target.clone());
        }
        self.reconcile_sidebar_navigation();
        true
    }
    pub(super) fn reconcile_sidebar_navigation(&mut self) {
        let collapsed = std::mem::take(&mut self.navigation.collapsed);
        self.navigation.collapsed = collapsed
            .into_iter()
            .filter(|target| self.sidebar_target_exists(target))
            .collect();
        let collapsed_agents = std::mem::take(&mut self.navigation.collapsed_agents);
        self.navigation.collapsed_agents = collapsed_agents
            .into_iter()
            .filter(|target| self.sidebar_target_exists(target))
            .collect();
        if !self.navigation.active && self.navigation.focus.is_none() {
            return;
        }
        let targets: Vec<_> = self
            .sidebar_navigation_rows()
            .iter()
            .map(SidebarRow::target)
            .collect();
        let previous = self.navigation.focus.clone();
        let selected = previous
            .as_ref()
            .map(|focus| focus.target.clone())
            .or_else(|| {
                self.workspace_rows
                    .iter()
                    .find(|row| self.current_workspace(row))
                    .and_then(Row::sidebar_target)
            });
        let position = selected
            .as_ref()
            .and_then(|target| targets.iter().position(|row| row == target))
            .or_else(|| {
                let selected = selected.as_ref()?;
                let ancestors = if self.sidebar_target_exists(selected) {
                    self.sidebar_ancestors(selected)
                } else {
                    previous
                        .as_ref()
                        .map(|focus| focus.ancestors.clone())
                        .unwrap_or_default()
                };
                ancestors
                    .iter()
                    .find_map(|target| targets.iter().position(|row| row == target))
            })
            .unwrap_or_else(|| {
                previous
                    .as_ref()
                    .map_or(0, |focus| focus.position)
                    .min(targets.len().saturating_sub(1))
            });
        self.navigation.focus = targets.get(position).cloned().map(|target| Focus {
            ancestors: self.sidebar_ancestors(&target),
            target,
            position,
        });
        if self.navigation.active
            && self.navigation.focus.as_ref().map(|focus| &focus.target)
                != previous.as_ref().map(|focus| &focus.target)
        {
            self.spaces.reveal();
        }
    }
    pub(crate) fn sidebar_key(&mut self, key: KeyEvent) -> SidebarIntent {
        if !self.navigation.active || key.kind == KeyKind::Release {
            return SidebarIntent::None;
        }
        let Some((_, command)) = SIDEBAR_BINDINGS
            .iter()
            .find(|(binding, _)| binding.matches(key))
        else {
            return SidebarIntent::None;
        };
        match command {
            SidebarCommand::NewWorkspace => {
                return if key.kind == KeyKind::Press {
                    SidebarIntent::NewWorkspace
                } else {
                    SidebarIntent::None
                };
            }
            SidebarCommand::Search => return SidebarIntent::Search,
            SidebarCommand::Exit => {
                self.leave_sidebar_navigation();
                return SidebarIntent::None;
            }
            SidebarCommand::Help => return SidebarIntent::Help,
            SidebarCommand::Narrower => return SidebarIntent::Resize(SidebarWidthChange::Narrower),
            SidebarCommand::Wider => return SidebarIntent::Resize(SidebarWidthChange::Wider),
            SidebarCommand::ResetWidth => return SidebarIntent::Resize(SidebarWidthChange::Reset),
            _ => {}
        }
        let targets: Vec<_> = self
            .sidebar_navigation_rows()
            .iter()
            .map(SidebarRow::target)
            .collect();
        let Some(focus) = self.navigation.focus.clone() else {
            return SidebarIntent::None;
        };
        match command {
            SidebarCommand::Previous => {
                self.focus_sidebar_position(focus.position.saturating_sub(1))
            }
            SidebarCommand::Next => self
                .focus_sidebar_position((focus.position + 1).min(targets.len().saturating_sub(1))),
            SidebarCommand::First => self.focus_sidebar_position(0),
            SidebarCommand::Last => self.focus_sidebar_position(targets.len().saturating_sub(1)),
            SidebarCommand::Expand => match &focus.target {
                SidebarTarget::Project { .. } => {
                    if !self.sidebar_project_expanded(&focus.target) {
                        self.toggle_sidebar_project(&focus.target);
                    } else if targets
                        .get(focus.position + 1)
                        .and_then(|target| self.sidebar_parent(target))
                        .as_ref()
                        == Some(&focus.target)
                    {
                        self.focus_sidebar_position(focus.position + 1);
                    }
                }
                SidebarTarget::Context { .. } if self.sidebar_context_has_agents(&focus.target) => {
                    if !self.sidebar_context_agents_expanded(&focus.target) {
                        self.toggle_sidebar_context_agents(&focus.target);
                        self.spaces.reveal();
                    } else if targets
                        .get(focus.position + 1)
                        .and_then(|target| self.sidebar_parent(target))
                        .as_ref()
                        == Some(&focus.target)
                    {
                        self.focus_sidebar_position(focus.position + 1);
                    }
                }
                SidebarTarget::Context { .. } | SidebarTarget::Agent { .. } => {}
            },
            SidebarCommand::Collapse => match &focus.target {
                SidebarTarget::Agent { .. } => {
                    let parent = self
                        .sidebar_parent(&focus.target)
                        .expect("an Agent sidebar row has a Context parent");
                    if let Some(position) = targets.iter().position(|target| target == &parent) {
                        self.focus_sidebar_position(position);
                    }
                }
                SidebarTarget::Context { .. }
                    if self.sidebar_context_agents_expanded(&focus.target) =>
                {
                    self.toggle_sidebar_context_agents(&focus.target);
                    self.spaces.reveal();
                }
                SidebarTarget::Context { .. } => {
                    if let Some(parent) = self.sidebar_parent(&focus.target)
                        && let Some(position) = targets.iter().position(|target| target == &parent)
                    {
                        self.focus_sidebar_position(position);
                    }
                }
                SidebarTarget::Project { .. } if self.sidebar_project_expanded(&focus.target) => {
                    self.toggle_sidebar_project(&focus.target);
                }
                SidebarTarget::Project { .. } => {}
            },
            SidebarCommand::Activate => match &focus.target {
                SidebarTarget::Project { .. } => {
                    self.toggle_sidebar_project(&focus.target);
                }
                SidebarTarget::Context { machine, target }
                    if self.sidebar_target_exists(&focus.target) =>
                {
                    return SidebarIntent::Activate(Selection {
                        machine: machine.clone(),
                        context: Some(target.clone().into()),
                    });
                }
                SidebarTarget::Context { .. } => {}
                SidebarTarget::Agent { machine, target }
                    if self.sidebar_target_exists(&focus.target) =>
                {
                    return SidebarIntent::Activate(Selection {
                        machine: machine.clone(),
                        context: Some(crate::terminal_session::NativeStartupTarget::Agent(
                            target.clone(),
                        )),
                    });
                }
                SidebarTarget::Agent { .. } => {}
            },
            SidebarCommand::OpenMenu
                if !matches!(focus.target, SidebarTarget::Agent { .. })
                    && self.sidebar_target_exists(&focus.target) =>
            {
                return SidebarIntent::OpenMenu(focus.target);
            }
            _ => {}
        }
        SidebarIntent::None
    }
}
