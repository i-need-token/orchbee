//! Sidebar hierarchy and keyboard focus over the Picker's existing canonical rows.
use super::{
    Choice, Context, KeyCode, KeyEvent, KeyKind, Modifiers, NativeContextTarget, Picker, Row,
    Selection,
};
use crate::{client_view::Binding, terminal_session::NativeContextLocation};
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
}
impl SidebarTarget {
    pub(crate) fn machine(&self) -> &Choice {
        match self {
            Self::Project { machine, .. } | Self::Context { machine, .. } => machine,
        }
    }
    pub(crate) fn site_id(&self) -> &SiteId {
        match self {
            Self::Project { site_id, .. } => site_id,
            Self::Context { target, .. } => &target.site_id,
        }
    }
    pub(crate) fn parent(&self) -> Option<Self> {
        let Self::Context {
            machine,
            target:
                NativeContextTarget {
                    site_id,
                    location: NativeContextLocation::Workspace { project_id, .. },
                },
        } = self
        else {
            return None;
        };
        Some(Self::Project {
            machine: machine.clone(),
            site_id: site_id.clone(),
            project_id: project_id.clone(),
        })
    }
}

pub(crate) enum SidebarRow<'a> {
    Project {
        target: SidebarTarget,
        label: &'a str,
    },
    Context(&'a Row),
}
impl SidebarRow<'_> {
    pub(crate) fn target(&self) -> SidebarTarget {
        match self {
            Self::Project { target, .. } => target.clone(),
            Self::Context(row) => row
                .sidebar_target()
                .expect("a sidebar Context row has its canonical target"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarCommand {
    Previous,
    Next,
    First,
    Last,
    Expand,
    Collapse,
    Activate,
    OpenMenu,
    Help,
    Exit,
}

pub(crate) const SIDEBAR_BINDINGS: &[(Binding, SidebarCommand)] = &[
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
    Activate(Selection),
    OpenMenu(SidebarTarget),
    Help,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Focus {
    target: SidebarTarget,
    position: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct SidebarNavigation {
    active: bool,
    focus: Option<Focus>,
    collapsed: Vec<SidebarTarget>,
}

impl Row {
    pub(crate) fn sidebar_target(&self) -> Option<SidebarTarget> {
        Some(SidebarTarget::Context {
            machine: self.choice.clone(),
            target: self.context.as_ref()?.target.clone(),
        })
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
                    workspace: row.label.clone(),
                    project: match &row.target.location {
                        NativeContextLocation::Workspace { project_id, .. } => checkpoint
                            .projects
                            .iter()
                            .find(|project| &project.project_id == project_id)
                            .map(|project| project.label.clone()),
                        NativeContextLocation::Standalone(_) => None,
                    },
                })
                .collect(),
        )
    }

    pub(crate) fn sidebar_navigation_rows(&self) -> Vec<SidebarRow<'_>> {
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
                        SidebarTarget::Project {
                            machine: machine.choice.clone(),
                            site_id: checkpoint.site_id.clone(),
                            project_id: project.project_id.clone(),
                        },
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
                match &context.target.location {
                    NativeContextLocation::Standalone(_) => entries.push(SidebarRow::Context(row)),
                    NativeContextLocation::Workspace { project_id, .. } => {
                        let key = (context.target.site_id.as_str(), project_id.as_str());
                        children.entry(key).or_insert_with(Vec::new).push(row);
                        if known.insert(key) {
                            projects.push((
                                SidebarTarget::Project {
                                    machine: machine.choice.clone(),
                                    site_id: context.target.site_id.clone(),
                                    project_id: project_id.clone(),
                                },
                                context.project.as_deref().unwrap_or(""),
                            ));
                        }
                    }
                }
            }
            for (target, label) in projects {
                let expanded = self.sidebar_project_expanded(&target);
                entries.push(SidebarRow::Project {
                    target: target.clone(),
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
                    entries.extend(rows.iter().map(|row| SidebarRow::Context(row)));
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
                        &context.target.site_id == site_id && matches!(&context.target.location,
                            NativeContextLocation::Workspace { project_id: parent, .. } if parent == project_id)
                    })
                })
            }
        }
    }
    pub(crate) fn sidebar_project_expanded(&self, target: &SidebarTarget) -> bool {
        !self.navigation.collapsed.contains(target)
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
    pub(super) fn reconcile_sidebar_navigation(&mut self) {
        let collapsed = std::mem::take(&mut self.navigation.collapsed);
        self.navigation.collapsed = collapsed
            .into_iter()
            .filter(|target| self.sidebar_target_exists(target))
            .collect();
        if !self.navigation.active && self.navigation.focus.is_none() {
            return;
        }
        let mut targets: Vec<_> = self
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
        if previous.is_none()
            && let Some(parent) = selected.as_ref().and_then(SidebarTarget::parent)
            && self.navigation.collapsed.contains(&parent)
        {
            self.navigation.collapsed.retain(|target| target != &parent);
            targets = self
                .sidebar_navigation_rows()
                .iter()
                .map(SidebarRow::target)
                .collect();
        }
        let position = selected
            .as_ref()
            .and_then(|target| targets.iter().position(|row| row == target))
            .or_else(|| {
                selected
                    .as_ref()
                    .and_then(SidebarTarget::parent)
                    .and_then(|parent| targets.iter().position(|row| row == &parent))
            })
            .unwrap_or_else(|| {
                previous
                    .as_ref()
                    .map_or(0, |focus| focus.position)
                    .min(targets.len().saturating_sub(1))
            });
        self.navigation.focus = targets
            .get(position)
            .cloned()
            .map(|target| Focus { target, position });
        if self.navigation.focus != previous {
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
            SidebarCommand::Exit => {
                self.leave_sidebar_navigation();
                return SidebarIntent::None;
            }
            SidebarCommand::Help => return SidebarIntent::Help,
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
            SidebarCommand::Expand if matches!(focus.target, SidebarTarget::Project { .. }) => {
                if !self.sidebar_project_expanded(&focus.target) {
                    self.toggle_sidebar_project(&focus.target);
                } else if targets
                    .get(focus.position + 1)
                    .and_then(SidebarTarget::parent)
                    .as_ref()
                    == Some(&focus.target)
                {
                    self.focus_sidebar_position(focus.position + 1);
                }
            }
            SidebarCommand::Collapse => {
                if let Some(parent) = focus.target.parent() {
                    if let Some(position) = targets.iter().position(|target| target == &parent) {
                        self.focus_sidebar_position(position);
                    }
                } else if self.sidebar_project_expanded(&focus.target) {
                    self.toggle_sidebar_project(&focus.target);
                }
            }
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
            },
            SidebarCommand::OpenMenu if self.sidebar_target_exists(&focus.target) => {
                return SidebarIntent::OpenMenu(focus.target);
            }
            _ => {}
        }
        SidebarIntent::None
    }
}
