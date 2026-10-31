//! Sidebar hierarchy and keyboard focus over the Picker's existing canonical rows.
use super::{
    Choice, ClientContextTarget, Context, KeyCode, KeyEvent, KeyKind, Modifiers, Picker, Row,
    Selection,
};
use crate::{
    client_view::{Binding, KeyLabelGlyphs, LabelStyle, format_binding_with_glyphs},
    sidebar_width::SidebarWidthChange,
    workbench_client::ClientContextLocation,
};
use orchbee_workbench_protocol::{SessionId, WorkbenchProjectId};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SidebarTarget {
    Project {
        machine: Choice,
        session_id: SessionId,
        project_id: WorkbenchProjectId,
    },
    /// The heading over one Session's Workspaces outside any Project; it folds but never activates.
    Recents {
        machine: Choice,
        session_id: SessionId,
    },
    Context {
        machine: Choice,
        target: ClientContextTarget,
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
            | Self::Recents { machine, .. }
            | Self::Context { machine, .. }
            | Self::Agent { machine, .. } => machine,
        }
    }
    pub(crate) fn session_id(&self) -> &SessionId {
        match self {
            Self::Project { session_id, .. } | Self::Recents { session_id, .. } => session_id,
            Self::Context { target, .. } => &target.session_id,
            Self::Agent { target, .. } => &target.context.session_id,
        }
    }

    /// Project and Recents headings group rows; they fold and never become current.
    pub(crate) const fn is_group(&self) -> bool {
        matches!(self, Self::Project { .. } | Self::Recents { .. })
    }

    pub(crate) fn selection(&self) -> Option<Selection> {
        match self {
            Self::Project { .. } | Self::Recents { .. } => None,
            Self::Context { machine, target } => Some(Selection {
                machine: machine.clone(),
                context: Some(target.clone().into()),
            }),
            Self::Agent { machine, target } => Some(Selection {
                machine: machine.clone(),
                context: Some(crate::workbench_client::ClientStartupTarget::Agent(
                    target.clone(),
                )),
            }),
        }
    }
}

/// An action associated with a painted Sidebar row; Menu is a secondary-button
/// and keyboard capability, without a separate visible control extent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarPart {
    Primary,
    Disclosure,
    Menu,
    Create,
    /// Pointer-revealed close request for one exact Workspace.
    Close,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SidebarCatalogRow {
    target: SidebarTarget,
    parent: Option<SidebarTarget>,
    disclosure: bool,
    menu: bool,
    create: bool,
    close: bool,
}

/// Value-semantic action projection for one successfully painted Sidebar frame.
///
/// Presentation-only labels, statuses, theme roles, keyboard focus, pointer state and scroll
/// position are deliberately absent. Topology and action eligibility remain so an armed frame is
/// retired when its semantic catalog changes.
#[derive(Clone, Debug, Eq, PartialEq)]
struct SidebarCatalog {
    admitted: bool,
    settings_admitted: bool,
    rows: Vec<SidebarCatalogRow>,
}

impl SidebarCatalog {
    fn has_part(&self, target: &SidebarTarget, part: SidebarPart) -> bool {
        self.rows.iter().any(|row| {
            row.target == *target
                && match part {
                    SidebarPart::Primary => true,
                    SidebarPart::Disclosure => row.disclosure,
                    SidebarPart::Menu => row.menu,
                    SidebarPart::Create => row.create,
                    SidebarPart::Close => row.close,
                }
        })
    }

    fn admits(&self, target: &SidebarTarget, part: SidebarPart) -> bool {
        self.admitted && self.has_part(target, part)
    }
}

/// One immutable Sidebar source shared by every target painted in the same frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SidebarSource(Arc<SidebarCatalog>);

impl SidebarSource {
    /// The sidebar and native snapshot arrive through separate read lanes. A current
    /// native revision alone cannot authorize a drag against older grouping/order.
    pub(crate) fn matches_navigation(
        &self,
        machine: &Choice,
        session: &SessionId,
        navigation: &orchbee_workbench_protocol::Navigation,
    ) -> bool {
        let rows = self
            .0
            .rows
            .iter()
            .filter(|row| row.target.machine() == machine && row.target.session_id() == session)
            .collect::<Vec<_>>();
        let projects = rows
            .iter()
            .filter_map(|row| match &row.target {
                SidebarTarget::Project { project_id, .. } => Some(project_id),
                _ => None,
            })
            .collect::<Vec<_>>();
        let Ok(ordered) = navigation.ordered_projects() else {
            return false;
        };
        if projects
            != ordered
                .iter()
                .map(|project| &project.project_id)
                .collect::<Vec<_>>()
        {
            return false;
        }
        let workspaces = navigation
            .workspaces
            .iter()
            .map(|workspace| (&workspace.workspace_id, workspace))
            .collect::<std::collections::HashMap<_, _>>();
        let mut shown = std::collections::HashMap::<
            Option<&WorkbenchProjectId>,
            Vec<&orchbee_workbench_protocol::WorkbenchWorkspaceId>,
        >::new();
        for row in &rows {
            if let SidebarTarget::Context { target, .. } = &row.target
                && let ClientContextLocation::Workspace { workspace_id, .. } = &target.location
            {
                let Some(workspace) = workspaces.get(workspace_id) else {
                    return false;
                };
                shown
                    .entry(workspace.project_id.as_ref())
                    .or_default()
                    .push(workspace_id);
            }
        }
        let visible = shown
            .values()
            .flatten()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let Ok(ordered) = navigation.ordered_workspaces() else {
            return false;
        };
        let mut expected = std::collections::HashMap::<
            Option<&WorkbenchProjectId>,
            Vec<&orchbee_workbench_protocol::WorkbenchWorkspaceId>,
        >::new();
        for (_, workspace) in ordered {
            if visible.contains(&workspace.workspace_id) {
                expected
                    .entry(workspace.project_id.as_ref())
                    .or_default()
                    .push(&workspace.workspace_id);
            }
        }
        if shown != expected {
            return false;
        }
        rows.into_iter().all(|row| match &row.target {
            SidebarTarget::Context { target, .. } => match &target.location {
                ClientContextLocation::NoWorkspace => false,
                ClientContextLocation::Workspace {
                    workspace_id,
                    terminal_context_id,
                } => workspaces.get(workspace_id).is_some_and(|workspace| {
                    &workspace.terminal_context_id == terminal_context_id
                        && match (&row.parent, &workspace.project_id) {
                            (
                                Some(SidebarTarget::Recents {
                                    machine: parent_machine,
                                    session_id: parent_session,
                                }),
                                None,
                            ) => parent_machine == machine && parent_session == session,
                            (
                                Some(SidebarTarget::Project {
                                    machine: parent_machine,
                                    session_id: parent_session,
                                    project_id: parent,
                                }),
                                Some(project),
                            ) => {
                                parent_machine == machine
                                    && parent_session == session
                                    && parent == project
                            }
                            _ => false,
                        }
                }),
            },
            SidebarTarget::Project { .. }
            | SidebarTarget::Recents { .. }
            | SidebarTarget::Agent { .. } => true,
        })
    }

    /// Returns whether this exact semantic catalog contains one row part.
    ///
    /// Layout preflight and painting use the same answer so rows reserve
    /// auxiliary cells only for controls that semantically exist. Transient
    /// surface admission remains independent and cannot move row geometry.
    pub(crate) fn has_part(&self, target: &SidebarTarget, part: SidebarPart) -> bool {
        self.0.has_part(target, part)
    }

    pub(crate) fn drawn(
        &self,
        target: SidebarTarget,
        part: SidebarPart,
        window: SidebarWindow,
    ) -> Option<DrawnSidebarTarget> {
        self.0.admits(&target, part).then(|| DrawnSidebarTarget {
            source: self.clone(),
            target,
            part,
            window,
        })
    }
}

/// Row window of one painted Sidebar or Picker list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SidebarWindow {
    pub(crate) top: usize,
    pub(crate) capacity: usize,
    pub(crate) total: usize,
    pub(crate) maximum: usize,
    /// An inert provisional row participates in geometry, but owns no canonical identity.
    pub(crate) insertion: Option<usize>,
}
impl SidebarWindow {
    pub(crate) fn end(self) -> usize {
        self.top.saturating_add(self.capacity).min(self.total)
    }
}

/// Exact semantic target and window geometry admitted by one Sidebar frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrawnSidebarTarget {
    source: SidebarSource,
    target: SidebarTarget,
    part: SidebarPart,
    window: SidebarWindow,
}

/// Stable identity of a fixed action on the painted Sidebar surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarFixedAction {
    NewWorkspace,
    Search,
    Settings,
    Browse,
}

/// A fixed Sidebar action bound to the same immutable source as the tree frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrawnSidebarFixedTarget {
    source: SidebarSource,
    action: SidebarFixedAction,
    scope: Option<Selection>,
}

impl DrawnSidebarFixedTarget {
    pub(crate) const fn action(&self) -> SidebarFixedAction {
        self.action
    }
}

/// Existing effect selected by a live, exact fixed-action release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SidebarFixedActivation {
    NewWorkspace,
    Search,
    Settings,
    Browse,
}

impl DrawnSidebarTarget {
    pub(crate) const fn part(&self) -> SidebarPart {
        self.part
    }

    pub(crate) fn target(&self) -> &SidebarTarget {
        &self.target
    }

    pub(crate) const fn window(&self) -> SidebarWindow {
        self.window
    }

    pub(crate) fn selection(&self) -> Option<Selection> {
        self.target.selection()
    }
}

/// Existing Picker transition selected by an accepted ordinary row release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SidebarActivation {
    /// Folds or opens a Project or Recents heading.
    ToggleGroup(SidebarTarget),
    ToggleContext(SidebarTarget),
    Activate(Selection),
    CloseWorkspace(orchbee_workbench_protocol::WorkbenchWorkspaceId),
}

pub(crate) enum SidebarRow<'a> {
    Project {
        machine: &'a Choice,
        session_id: &'a SessionId,
        project_id: &'a WorkbenchProjectId,
        label: &'a str,
    },
    Recents {
        machine: &'a Choice,
        session_id: &'a SessionId,
    },
    Context {
        row: &'a Row,
        has_agents: bool,
    },
    Agent {
        row: &'a Row,
    },
}
impl SidebarRow<'_> {
    pub(crate) fn target(&self) -> SidebarTarget {
        match self {
            Self::Project {
                machine,
                session_id,
                project_id,
                ..
            } => SidebarTarget::Project {
                machine: (*machine).clone(),
                session_id: (*session_id).clone(),
                project_id: (*project_id).clone(),
            },
            Self::Recents {
                machine,
                session_id,
            } => SidebarTarget::Recents {
                machine: (*machine).clone(),
                session_id: (*session_id).clone(),
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
    MoveUp,
    MoveDown,
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
impl From<SidebarCommand> for crate::localization::SidebarActionLabel {
    fn from(action: SidebarCommand) -> Self {
        match action {
            SidebarCommand::MoveUp => Self::MoveUp,
            SidebarCommand::MoveDown => Self::MoveDown,
            SidebarCommand::NewWorkspace => Self::NewWorkspace,
            SidebarCommand::Search => Self::Search,
            SidebarCommand::Previous => Self::Previous,
            SidebarCommand::Next => Self::Next,
            SidebarCommand::First => Self::First,
            SidebarCommand::Last => Self::Last,
            SidebarCommand::Expand => Self::Expand,
            SidebarCommand::Collapse => Self::Collapse,
            SidebarCommand::Activate => Self::Activate,
            SidebarCommand::OpenMenu => Self::OpenMenu,
            SidebarCommand::Narrower => Self::Narrower,
            SidebarCommand::Wider => Self::Wider,
            SidebarCommand::ResetWidth => Self::ResetWidth,
            SidebarCommand::Help => Self::Help,
            SidebarCommand::Exit => Self::Exit,
        }
    }
}

pub(crate) const SIDEBAR_BINDINGS: &[(Binding, SidebarCommand)] = &[
    (
        Binding::new(KeyCode::Up, Modifiers::CTRL),
        SidebarCommand::MoveUp,
    ),
    (
        Binding::new(KeyCode::Down, Modifiers::CTRL),
        SidebarCommand::MoveDown,
    ),
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

pub(crate) fn sidebar_binding_label(
    command: SidebarCommand,
    glyphs: KeyLabelGlyphs,
) -> Option<String> {
    SIDEBAR_BINDINGS
        .iter()
        .find(|(_, candidate)| *candidate == command)
        .map(|(binding, _)| format_binding_with_glyphs(binding, LabelStyle::for_host(), glyphs))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SidebarIntent {
    None,
    NewWorkspace,
    Search,
    Activate(Selection),
    OpenMenu(SidebarTarget),
    Reorder { target: SidebarTarget, up: bool },
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
    // Folded Project and Recents headings; every group starts expanded.
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
    NoWorkspace,
    Workspace {
        workspace: &'a str,
        terminal: &'a str,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ContextIdentity<'a> {
    machine: MachineIdentity<'a>,
    session: &'a str,
    location: ContextLocationIdentity<'a>,
}

/// Shared join key for Agent rows and counts; Project membership is presentation metadata.
pub(super) fn context_identity<'a>(
    machine: &'a Choice,
    target: &'a ClientContextTarget,
) -> ContextIdentity<'a> {
    ContextIdentity {
        machine: match machine {
            Choice::Local => MachineIdentity::Local,
            Choice::Remote(id) => MachineIdentity::Remote(id.as_str()),
        },
        session: target.session_id.as_str(),
        location: match &target.location {
            ClientContextLocation::NoWorkspace => ContextLocationIdentity::NoWorkspace,
            ClientContextLocation::Workspace {
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
    agent_children: &std::collections::BTreeMap<ContextIdentity<'a>, Vec<&'a Row>>,
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
        entries.extend(children.iter().map(|row| SidebarRow::Agent { row }));
    }
}

impl Picker {
    pub(super) fn checkpoint_contexts(&self, machine: &Choice) -> Option<Vec<Context>> {
        let checkpoint = self.space_checkpoint(machine)?;
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
        // Join the accepted Agent projection to its exact endpoint + Session + Context once.
        // Labels and physical placement are presentation data, never ownership keys.
        let mut agent_children = std::collections::BTreeMap::new();
        for row in self.agent_rows.iter().filter(|row| row.enabled) {
            let Some(agent) = row.agent.as_ref() else {
                continue;
            };
            agent_children
                .entry(context_identity(&row.choice, &agent.target.context))
                .or_insert_with(Vec::new)
                .push(row);
        }
        let mut entries = Vec::new();
        for machine in self.rows.iter().filter(|machine| machine.enabled) {
            let mut projects = Vec::new();
            let mut ungrouped = Vec::new();
            let mut known = std::collections::BTreeSet::new();
            let mut children = std::collections::BTreeMap::new();
            if let Some(checkpoint) = self.space_checkpoint(&machine.choice) {
                for project in checkpoint.projects.iter() {
                    known.insert((checkpoint.session_id.as_str(), project.project_id.as_str()));
                    projects.push((
                        &machine.choice,
                        &checkpoint.session_id,
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
                    None => {
                        ungrouped.push(row);
                    }
                    Some(project_id) => {
                        let key = (context.target.session_id.as_str(), project_id.as_str());
                        children.entry(key).or_insert_with(Vec::new).push(row);
                        if known.insert(key) {
                            projects.push((
                                &machine.choice,
                                &context.target.session_id,
                                project_id,
                                context.project.as_deref().unwrap_or(""),
                            ));
                        }
                    }
                }
            }
            for (machine, session_id, project_id, label) in projects {
                let target = SidebarTarget::Project {
                    machine: machine.clone(),
                    session_id: session_id.clone(),
                    project_id: project_id.clone(),
                };
                let expanded = self.sidebar_group_expanded(&target);
                entries.push(SidebarRow::Project {
                    machine,
                    session_id,
                    project_id,
                    label,
                });
                if expanded
                    && let SidebarTarget::Project {
                        session_id,
                        project_id,
                        ..
                    } = &target
                    && let Some(rows) = children.get(&(session_id.as_str(), project_id.as_str()))
                {
                    for row in rows {
                        append_context(&self.navigation, &agent_children, &mut entries, row);
                    }
                }
            }
            // Workspaces outside any Project always come last, under one Recents heading per Session.
            let mut sessions = Vec::new();
            for row in &ungrouped {
                let session_id = &row
                    .context
                    .as_ref()
                    .expect("an ungrouped sidebar row has Context data")
                    .target
                    .session_id;
                if !sessions.contains(&session_id) {
                    sessions.push(session_id);
                }
            }
            for session_id in sessions {
                let heading = SidebarRow::Recents {
                    machine: &machine.choice,
                    session_id,
                };
                let expanded = self.sidebar_group_expanded(&heading.target());
                entries.push(heading);
                if !expanded {
                    continue;
                }
                for row in ungrouped.iter().filter(|row| {
                    row.context
                        .as_ref()
                        .is_some_and(|context| &context.target.session_id == session_id)
                }) {
                    append_context(&self.navigation, &agent_children, &mut entries, row);
                }
            }
        }
        entries
    }

    /// Captures only the live semantics that can change a Sidebar target's release result.
    /// Renderers call this once per candidate frame and clone its `Arc` into painted targets.
    pub(crate) fn sidebar_source(&self) -> SidebarSource {
        let admitted = self.available && self.spaces.visible() && !self.open;
        let menu_session = self
            .current_context
            .as_ref()
            .map(|context| &context.session_id);
        let rows = self
            .sidebar_navigation_rows()
            .into_iter()
            .map(|row| {
                let target = row.target();
                let menu = matches!(
                    &target,
                    SidebarTarget::Project { .. } | SidebarTarget::Context { .. }
                ) && target.machine() == &self.current
                    && menu_session == Some(target.session_id());
                SidebarCatalogRow {
                    parent: self.sidebar_parent(&target),
                    disclosure: match row {
                        SidebarRow::Project { .. } | SidebarRow::Recents { .. } => true,
                        SidebarRow::Context { has_agents, .. } => has_agents,
                        SidebarRow::Agent { .. } => false,
                    },
                    create: menu && matches!(&target, SidebarTarget::Project { .. }),
                    // Session Close requires the endpoint menu capability.
                    close: menu
                        && matches!(
                            &target,
                            SidebarTarget::Context {
                                target: ClientContextTarget {
                                    location: ClientContextLocation::Workspace { .. },
                                    ..
                                },
                                ..
                            }
                        ),
                    menu,
                    target,
                }
            })
            .collect();
        SidebarSource(Arc::new(SidebarCatalog {
            admitted,
            // Settings is global chrome. It remains a valid semantic action even when
            // the machine-backed tree is unavailable; the shell decides whether this
            // particular Sidebar surface is paintable.
            settings_admitted: true,
            rows,
        }))
    }

    fn sidebar_fixed_scope(&self, action: SidebarFixedAction) -> Option<Selection> {
        matches!(
            action,
            SidebarFixedAction::NewWorkspace | SidebarFixedAction::Browse
        )
        .then(|| Selection {
            machine: self.current.clone(),
            context: self.current_context.clone().map(Into::into),
        })
    }

    /// Binds a fixed action to this frame's source and the action's current effect scope.
    pub(crate) fn drawn_sidebar_fixed(
        &self,
        source: &SidebarSource,
        action: SidebarFixedAction,
    ) -> DrawnSidebarFixedTarget {
        DrawnSidebarFixedTarget {
            source: source.clone(),
            action,
            scope: self.sidebar_fixed_scope(action),
        }
    }

    /// Rebuilds the pure action projection once at release and selects an existing owner effect.
    pub(crate) fn sidebar_activation(
        &self,
        drawn: &DrawnSidebarTarget,
    ) -> Option<SidebarActivation> {
        let live = self.sidebar_source();
        if live != drawn.source.clone() || !live.0.admits(&drawn.target, drawn.part) {
            return None;
        }
        match (&drawn.target, drawn.part) {
            (
                SidebarTarget::Project { .. } | SidebarTarget::Recents { .. },
                SidebarPart::Primary | SidebarPart::Disclosure,
            ) => Some(SidebarActivation::ToggleGroup(drawn.target.clone())),
            (SidebarTarget::Context { .. }, SidebarPart::Disclosure) => {
                Some(SidebarActivation::ToggleContext(drawn.target.clone()))
            }
            (SidebarTarget::Context { .. } | SidebarTarget::Agent { .. }, SidebarPart::Primary) => {
                drawn.target.selection().map(SidebarActivation::Activate)
            }
            (
                SidebarTarget::Context {
                    target:
                        ClientContextTarget {
                            location: ClientContextLocation::Workspace { workspace_id, .. },
                            ..
                        },
                    ..
                },
                SidebarPart::Close,
            ) => Some(SidebarActivation::CloseWorkspace(workspace_id.clone())),
            (_, SidebarPart::Menu | SidebarPart::Create | SidebarPart::Close) => None,
            (SidebarTarget::Agent { .. }, SidebarPart::Disclosure) => None,
        }
    }

    /// Rebuilds the semantic source once and authorizes no stale fixed-action effect.
    pub(crate) fn sidebar_fixed_activation(
        &self,
        drawn: &DrawnSidebarFixedTarget,
    ) -> Option<SidebarFixedActivation> {
        let live = self.sidebar_source();
        if live != drawn.source || self.sidebar_fixed_scope(drawn.action) != drawn.scope {
            return None;
        }
        match drawn.action {
            SidebarFixedAction::NewWorkspace if live.0.admitted => {
                Some(SidebarFixedActivation::NewWorkspace)
            }
            SidebarFixedAction::Search if live.0.admitted => Some(SidebarFixedActivation::Search),
            SidebarFixedAction::Settings if live.0.settings_admitted => {
                Some(SidebarFixedActivation::Settings)
            }
            SidebarFixedAction::Browse if live.0.admitted => Some(SidebarFixedActivation::Browse),
            SidebarFixedAction::NewWorkspace
            | SidebarFixedAction::Search
            | SidebarFixedAction::Settings
            | SidebarFixedAction::Browse => None,
        }
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
                &row.choice == machine
                    && row
                        .context
                        .as_ref()
                        .is_some_and(|context| &context.target == target)
            }),
            SidebarTarget::Project {
                machine,
                session_id,
                project_id,
            } => {
                self.space_checkpoint(machine).is_some_and(|checkpoint| {
                    &checkpoint.session_id == session_id
                        && checkpoint
                            .projects
                            .iter()
                            .any(|project| &project.project_id == project_id)
                }) || self.workspace_rows.iter().any(|row| {
                    &row.choice == machine
                        && row.context.as_ref().is_some_and(|context| {
                            &context.target.session_id == session_id
                                && context.project_id.as_ref() == Some(project_id)
                        })
                })
            }
            SidebarTarget::Recents {
                machine,
                session_id,
            } => self.workspace_rows.iter().any(|row| {
                &row.choice == machine
                    && row.context.as_ref().is_some_and(|context| {
                        &context.target.session_id == session_id && context.project_id.is_none()
                    })
            }),
            SidebarTarget::Agent { machine, target } => {
                self.workspace_rows.iter().any(|row| {
                    row.enabled
                        && &row.choice == machine
                        && row
                            .context
                            .as_ref()
                            .is_some_and(|context| context.target == target.context)
                }) && self.agent_rows.iter().any(|row| {
                    row.enabled
                        && &row.choice == machine
                        && row
                            .agent
                            .as_ref()
                            .is_some_and(|agent| &agent.target == target.as_ref())
                })
            }
        }
    }
    /// Whether a Project or Recents heading shows its Workspaces.
    pub(crate) fn sidebar_group_expanded(&self, target: &SidebarTarget) -> bool {
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
                Some(match &context.project_id {
                    Some(project_id) => SidebarTarget::Project {
                        machine: machine.clone(),
                        session_id: target.session_id.clone(),
                        project_id: project_id.clone(),
                    },
                    None => SidebarTarget::Recents {
                        machine: machine.clone(),
                        session_id: target.session_id.clone(),
                    },
                })
            }
            SidebarTarget::Project { .. } | SidebarTarget::Recents { .. } => None,
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
                .is_none_or(|parent| self.sidebar_group_expanded(&parent))
    }
    pub fn sidebar_navigation_active(&self) -> bool {
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
    #[cfg(any(test, feature = "test-support"))]
    pub fn focus_sidebar_target(&mut self, target: &SidebarTarget) -> bool {
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
    pub(crate) fn toggle_sidebar_group(&mut self, target: &SidebarTarget) -> bool {
        if !self.available || self.open || !target.is_group() || !self.sidebar_target_exists(target)
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
        let targets: Vec<_> = self
            .sidebar_navigation_rows()
            .iter()
            .map(SidebarRow::target)
            .collect();
        self.spaces.reconcile(&targets);
        if !self.navigation.active && self.navigation.focus.is_none() {
            return;
        }
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
            SidebarCommand::MoveUp | SidebarCommand::MoveDown => {
                // Recents is a fixed trailing heading, never a reorder source.
                if key.kind == KeyKind::Press
                    && !matches!(focus.target, SidebarTarget::Recents { .. })
                {
                    return SidebarIntent::Reorder {
                        target: focus.target,
                        up: *command == SidebarCommand::MoveUp,
                    };
                }
            }
            SidebarCommand::Previous => {
                self.focus_sidebar_position(focus.position.saturating_sub(1))
            }
            SidebarCommand::Next => self
                .focus_sidebar_position((focus.position + 1).min(targets.len().saturating_sub(1))),
            SidebarCommand::First => self.focus_sidebar_position(0),
            SidebarCommand::Last => self.focus_sidebar_position(targets.len().saturating_sub(1)),
            SidebarCommand::Expand => match &focus.target {
                SidebarTarget::Project { .. } | SidebarTarget::Recents { .. } => {
                    if !self.sidebar_group_expanded(&focus.target) {
                        self.toggle_sidebar_group(&focus.target);
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
                SidebarTarget::Project { .. } | SidebarTarget::Recents { .. }
                    if self.sidebar_group_expanded(&focus.target) =>
                {
                    self.toggle_sidebar_group(&focus.target);
                }
                SidebarTarget::Project { .. } | SidebarTarget::Recents { .. } => {}
            },
            SidebarCommand::Activate => match &focus.target {
                SidebarTarget::Project { .. } | SidebarTarget::Recents { .. } => {
                    self.toggle_sidebar_group(&focus.target);
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
                        context: Some(crate::workbench_client::ClientStartupTarget::Agent(
                            target.clone(),
                        )),
                    });
                }
                SidebarTarget::Agent { .. } => {}
            },
            SidebarCommand::OpenMenu
                if matches!(
                    focus.target,
                    SidebarTarget::Project { .. } | SidebarTarget::Context { .. }
                ) && self.sidebar_target_exists(&focus.target) =>
            {
                return SidebarIntent::OpenMenu(focus.target);
            }
            _ => {}
        }
        SidebarIntent::None
    }
}
