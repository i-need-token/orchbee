//! Deterministic menu capture, selection and successful-draw authority.
//! WorkbenchClientUi alone owns target validation and navigation; this unit cannot execute actions.
use crate::client_actions::PrefixAction;
use crate::client_view::{
    Binding, ClientAction, KeyCode, KeyEvent, KeyKind, Modifiers, PanelDirection,
};
use crate::terminal_text::line_editor::{LengthLimit, LineEditor, LineKey};
use crate::tui_presentation::{GeometrySettlement, PresentationPhase};

mod tab_presentation;
pub(crate) use tab_presentation::{TabMenuPresentation, TabMenuScope};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MenuTarget {
    Global,
    /// Creation-only entry over the existing global or exact Project actions.
    Create(Option<orchbee_workbench_protocol::WorkbenchProjectId>),
    TabGroups {
        tab_id: orchbee_workbench_protocol::WorkbenchTabId,
        colors: bool,
    },
    TabGroupChoices(orchbee_workbench_protocol::WorkbenchTabId),
    TabCreate(DrawnTabCreate),
    /// The new-Tab button's saved launch profiles, the default first.
    TabProfiles(DrawnTabCreate),
    Pane(crate::workbench_state::PaneRef),
    Tab(orchbee_workbench_protocol::WorkbenchTabId),
    /// Organization actions address the Project directly; terminal actions require a member.
    Project(orchbee_workbench_protocol::WorkbenchProjectId),
    /// One Agent row's terminal, titled by the name the row shows.
    Agent {
        target: Box<crate::machine_agents::Target>,
        title: String,
    },
    Workspace(crate::workbench_projection::ClientContextLocation),
    WorkspaceProjects(crate::workbench_projection::ClientContextLocation),
    /// One Workspace's Open Worktree submenu: the Git Worktrees other than its own checkout.
    WorkspaceWorktrees(crate::workbench_projection::ClientContextLocation),
    /// One Project's New Workspace submenu: a new Worktree, its folders and its Git Worktrees.
    WorkspaceCreation(orchbee_workbench_protocol::WorkbenchProjectId),
    /// An object entry must still belong to the exact Session snapshot that painted it.
    Drawn {
        source: DrawnMenuSource,
        target: Box<Self>,
    },
}

impl MenuTarget {
    /// Object and creation menus retain context chrome even when entered with the keyboard.
    pub(crate) fn context_menu(&self) -> bool {
        match self {
            Self::Global => false,
            Self::Drawn { target, .. } => target.context_menu(),
            _ => true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrawnMenuSource {
    pub(crate) session: orchbee_workbench_protocol::SessionId,
    pub(crate) revision: orchbee_workbench_protocol::WorkbenchRevision,
}

/// The exact context and snapshot behind a painted tab-strip create control.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrawnTabCreate {
    pub(crate) source: DrawnMenuSource,
    pub(crate) location: crate::workbench_projection::ClientContextLocation,
}

/// Where a context menu opens; a menu without an anchor is centered in the overlay area.
///
/// Both variants name one cell. The renderer resolves the complete trigger containing it from the
/// frame being drawn, so a moved or covered trigger never leaves the menu on stale geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAnchor {
    /// The pressed cell: the menu opens on the row below its trigger, else above it.
    Pointer { column: u16, row: u16 },
    /// A cell of a keyboard-focused sidebar row: the menu opens beside that row.
    Row { column: u16, row: u16 },
}

impl MenuAnchor {
    pub(crate) const fn pointer(self) -> bool {
        matches!(self, Self::Pointer { .. })
    }
}

impl From<(u16, u16)> for MenuAnchor {
    fn from((column, row): (u16, u16)) -> Self {
        Self::Pointer { column, row }
    }
}

pub use crate::client_actions::MenuAction;

pub(crate) use crate::localization::{MenuHeading, MenuRowNote, MenuUnavailable};

/// Copy selection leads; Zoom joins only beside another panel; Take control only while another
/// Client holds the size.
pub(crate) const PANE_ACTIONS: &[MenuAction] = &[
    MenuAction::CopySelection,
    MenuAction::FindInTerminal,
    MenuAction::ClearTerminal,
    MenuAction::RenamePanel,
    MenuAction::SplitPanel(PanelDirection::Right),
    MenuAction::SplitPanel(PanelDirection::Down),
    MenuAction::Zoom,
    MenuAction::Takeover,
    MenuAction::ClosePane,
];
/// Remove Tab from group, Resume closing and Close other Tabs join only when they apply.
pub(crate) const TAB_ACTIONS: &[MenuAction] = &[
    MenuAction::RenameTab,
    MenuAction::PinTab,
    MenuAction::ManageGroups,
    MenuAction::CloseSelectedTabs,
    MenuAction::CloseOtherTabs,
];
/// The Worktree rows wait for Git discovery, which may also add Remove Worktree….
pub(crate) const WORKSPACE_ACTIONS: &[MenuAction] = &[
    MenuAction::WorkspaceProjects,
    MenuAction::RenameWorkspace,
    MenuAction::NewWorktree,
    MenuAction::WorkspaceWorktrees,
    MenuAction::DeleteWorkspace,
];
pub(crate) const PROJECT_ACTIONS: &[MenuAction] = &[
    MenuAction::WorkspaceCreation,
    MenuAction::RenameProject,
    MenuAction::DeleteProject,
];
/// Rename and Close address the Agent's Panel, which the menu focuses before either runs.
pub(crate) const AGENT_ACTIONS: &[MenuAction] = &[MenuAction::RenamePanel, MenuAction::ClosePane];
/// The Tab section acts on the current Tab, keeping selection and movement reachable by keyboard.
pub(crate) const GLOBAL_ACTIONS: &[MenuAction] = &[
    MenuAction::TabCreation,
    MenuAction::NewTabToRight,
    MenuAction::DuplicateTab,
    MenuAction::RenameTab,
    MenuAction::MoveTabPrevious,
    MenuAction::MoveTabNext,
    MenuAction::MoveTabFirst,
    MenuAction::MoveTabLast,
    MenuAction::SelectAllTabs,
    MenuAction::ClearTabSelection,
    MenuAction::CloseTab,
    MenuAction::CloseTrailingTabs,
    MenuAction::LastPanel,
    MenuAction::ClearTerminal,
    MenuAction::RenamePanel,
    MenuAction::SplitPanel(PanelDirection::Up),
    MenuAction::SplitPanel(PanelDirection::Down),
    MenuAction::SplitPanel(PanelDirection::Left),
    MenuAction::SplitPanel(PanelDirection::Right),
    MenuAction::ClosePane,
    MenuAction::Zoom,
    MenuAction::MovePanel(PanelDirection::Up),
    MenuAction::MovePanel(PanelDirection::Down),
    MenuAction::MovePanel(PanelDirection::Left),
    MenuAction::MovePanel(PanelDirection::Right),
    MenuAction::FindInTerminal,
    MenuAction::Copy,
    MenuAction::NewWorkspace,
    MenuAction::WorkspaceAtDirectory,
    MenuAction::NewWorktree,
    MenuAction::OpenWorktree,
    MenuAction::Detach,
    MenuAction::Settings,
    MenuAction::Machines,
    MenuAction::Help,
    MenuAction::Sidebar,
    MenuAction::Navigator,
    MenuAction::AgentBoard,
    MenuAction::StartAgent,
    MenuAction::OpenNotificationTarget,
    MenuAction::ReleasePointer,
];

/// Resolves only bindings whose dispatch target has the same meaning as this menu entry.
///
/// Stable metadata identity is deliberately insufficient here: a prefix action aimed at the
/// current Tab, Workspace, or Pane must not be advertised for an exact background menu target.
pub(crate) fn menu_binding_action(
    target: &crate::action_menu::MenuTarget,
    action: crate::action_menu::MenuAction,
) -> Option<PrefixAction> {
    use crate::action_menu::{MenuAction as M, MenuTarget as T};
    use ClientAction as A;

    if let T::Drawn { target, .. } = target {
        return menu_binding_action(target, action);
    }

    let action = match (target, action) {
        (T::Global | T::Create(None), M::NewWorkspace) => {
            PrefixAction::Client(A::RequestWorkspaceCreate)
        }
        (T::Global | T::Create(None), M::NewWorktree) => {
            PrefixAction::Client(A::RequestWorktreeCreate)
        }
        (T::Global | T::Create(None), M::OpenWorktree) => {
            PrefixAction::Client(A::RequestWorktreeOpen)
        }
        (T::Global, M::SplitPanel(direction)) => PrefixAction::Client(A::SplitPanel(direction)),
        (T::Global, M::ClosePane) => PrefixAction::Client(A::ClosePanel),
        (T::Global, M::CloseTab) => PrefixAction::Client(A::RequestCloseTab),
        (T::Global, M::Zoom) => PrefixAction::Client(A::ToggleZoom),
        (T::Global, M::MovePanel(direction)) => PrefixAction::Client(A::MovePanel(direction)),
        (T::Global, M::RenameTab) => PrefixAction::Client(A::RequestRenameTab),
        (T::Global, M::RenamePanel) => PrefixAction::Client(A::RequestRenamePanel),
        (T::Global, M::ClearTerminal) => PrefixAction::Client(A::ClearTerminal),
        // The Panel menu focuses its Panel first, so the key acts on the same Panel.
        (T::Global | T::Pane(_), M::FindInTerminal) => PrefixAction::Client(A::FindInTerminal),
        (T::Global, M::LastPanel) => PrefixAction::Client(A::FocusLastPanel),
        (T::Global, M::Copy) => PrefixAction::Client(A::EnterCopyMode),
        (T::Global, M::Settings) => PrefixAction::Client(A::OpenSettings),
        (T::Global, M::Machines) => PrefixAction::Client(A::OpenMachines),
        (T::Global, M::Help) => PrefixAction::ShowHelp,
        (T::Global, M::Sidebar) => PrefixAction::SidebarToggle,
        (T::Global, M::Navigator) => PrefixAction::NavigatorToggle,
        (T::Global, M::AgentBoard) => PrefixAction::BoardToggle,
        (T::Global, M::StartAgent) => PrefixAction::Client(A::StartAgent),
        (T::Global, M::OpenNotificationTarget) => PrefixAction::Client(A::OpenNotificationTarget),
        (T::Global, M::ReleasePointer) => PrefixAction::Client(A::TogglePointerCapture),
        (T::Global, M::Detach) => PrefixAction::Client(A::Exit),
        (T::TabCreate(_), M::NewTab) => PrefixAction::Client(A::CreateTab),
        // The key opens the same picker; only its starting place differs.
        (T::TabProfiles(_), M::MoreAgents) => PrefixAction::Client(A::StartAgent),
        _ => return None,
    };
    Some(action)
}
/// One presentation revision of the open menu. Every revision mints a distinct token; an
/// availability refresh keeps the lineage of the revision it refreshes, because it leaves the
/// rows, their copy, the selection and the viewport as they were.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuToken {
    revision: u64,
    lineage: u64,
}

impl MenuToken {
    /// Whether this token is `earlier` or a later revision that only refreshed availability since.
    pub(crate) const fn refreshes(self, earlier: Self) -> bool {
        self.lineage == earlier.lineage && self.revision >= earlier.revision
    }
}

/// The local chrome actions owned by an open menu surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuControl {
    /// Returns from a cascaded submenu to its parent with the opening row as the candidate.
    Back,
    /// Returns from a cascaded submenu to its parent with this other parent row as the candidate.
    ParentRow(usize),
    Close,
}

/// What input the exact retained menu snapshot currently admits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuInputReadiness {
    /// The owner retains capture, but no ordinary key may mutate this surface.
    Recovery,
    /// The whole surface is still the admitted owner while an exact row proof is renewed.
    Continuation,
    /// The selected row has a fresh exact draw and may activate.
    ActivationReady,
}

/// One exact input snapshot for the retained menu surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuInputContext {
    token: MenuToken,
    phase: PresentationPhase,
    readiness: MenuInputReadiness,
}

impl MenuInputContext {
    #[cfg(test)]
    pub(crate) const fn token(self) -> MenuToken {
        self.token
    }

    pub(crate) const fn phase(self) -> PresentationPhase {
        self.phase
    }

    pub(crate) const fn readiness(self) -> MenuInputReadiness {
        self.readiness
    }

    pub(crate) const fn activation_ready(self) -> bool {
        matches!(self.readiness, MenuInputReadiness::ActivationReady)
    }

    pub(crate) const fn recovery_only(mut self) -> Self {
        self.readiness = MenuInputReadiness::Recovery;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MenuDraw {
    pub(crate) token: MenuToken,
    pub(crate) first: usize,
    pub(crate) visible_rows: usize,
    pub(crate) selected: usize,
    /// False while a pointer-opened menu has not yet revealed a candidate.
    pub(crate) candidate_visible: bool,
    /// Result positions map back to the admitted action snapshot, including when filtering.
    pub(crate) result_actions: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MenuSearchChild {
    pub(crate) parent: MenuAction,
    pub(crate) source: MenuTarget,
    pub(crate) actions: Vec<MenuAction>,
    pub(crate) labels: Vec<Option<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MenuDrawProof {
    token: MenuToken,
    first: usize,
    visible_rows: usize,
    selected: usize,
    candidate_visible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MenuSelection {
    Previous,
    Next,
    First,
    Last,
}

#[derive(Clone, Copy)]
pub(crate) struct MenuView<'a> {
    pub(crate) source: &'a MenuTarget,
    pub(crate) context_menu: bool,
    pub(crate) has_parent: bool,
    pub(crate) title: &'a str,
    pub(crate) labels: &'a [Option<String>],
    pub(crate) notes: &'a [Option<MenuRowNote>],
    pub(crate) tabs: Option<&'a TabMenuPresentation>,
    pub(crate) actions: &'a [MenuAction],
    pub(crate) unavailable: &'a [Option<MenuUnavailable>],
    #[cfg(test)]
    pub(crate) result_actions: &'a [usize],
    pub(crate) query: Option<&'a str>,
    /// The query cursor's byte offset; always a grapheme boundary of `query`.
    pub(crate) query_cursor: usize,
    pub(crate) search_children: &'a [MenuSearchChild],
    pub(crate) selected: usize,
    pub(crate) pending_selected_action: Option<&'a MenuAction>,
    pub(crate) first: usize,
    pub(crate) follow_selection: bool,
    pub(crate) token: MenuToken,
    pub(crate) anchor: Option<MenuAnchor>,
    /// The selected row is retained for navigation but painted and admitted as no candidate.
    pub(crate) candidate_hidden: bool,
    pub(crate) transferring: bool,
}

impl MenuView<'_> {
    pub(crate) fn result_separator_before(&self, results: &[usize], position: usize) -> bool {
        position > 0
            && self.query.is_none_or(str::is_empty)
            && separator_before(self.source, self.actions, results[position])
    }

    /// A group heading belongs to the row it introduces, so it is shown even when that row is the
    /// first one visible.
    pub(crate) fn result_heading_before(
        &self,
        results: &[usize],
        position: usize,
    ) -> Option<MenuHeading> {
        self.query
            .is_none_or(str::is_empty)
            .then(|| heading_before(self.source, self.actions, results[position]))
            .flatten()
    }

    pub(crate) fn result_row_height(&self, results: &[usize], first: usize, end: usize) -> usize {
        end.saturating_sub(first)
            + (first + 1..end)
                .filter(|position| self.result_separator_before(results, *position))
                .count()
            + (first..end)
                .filter(|position| self.result_heading_before(results, *position).is_some())
                .count()
    }

    pub(crate) fn first_visible_result_row(
        &self,
        rows: usize,
        results: &[usize],
        selected: usize,
    ) -> usize {
        let rows = rows.max(1);
        if results.is_empty() {
            return 0;
        }
        let selected = selected.min(results.len() - 1);
        let mut maximum = results.len().saturating_sub(1);
        while maximum > 0 && self.result_row_height(results, maximum - 1, results.len()) <= rows {
            maximum -= 1;
        }
        let mut first = self.first.min(maximum);
        if !self.follow_selection {
            return first;
        }
        if selected < first {
            first = selected;
        }
        while first < selected && self.result_row_height(results, first, selected + 1) > rows {
            first += 1;
        }
        first.min(maximum)
    }
}

pub(crate) const MAXIMUM_ACTION_MENU_QUERY_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GlobalMenuGroup {
    Tab,
    Panel,
    Workspace,
    Client,
    Other,
}

fn global_menu_group(action: &MenuAction) -> GlobalMenuGroup {
    match action {
        MenuAction::TabCreation
        | MenuAction::NewTabToRight
        | MenuAction::DuplicateTab
        | MenuAction::RenameTab
        | MenuAction::MoveTabPrevious
        | MenuAction::MoveTabNext
        | MenuAction::MoveTabFirst
        | MenuAction::MoveTabLast
        | MenuAction::SelectAllTabs
        | MenuAction::ClearTabSelection
        | MenuAction::CloseTab
        | MenuAction::CloseTrailingTabs => GlobalMenuGroup::Tab,
        MenuAction::SplitPanel(_)
        | MenuAction::Zoom
        | MenuAction::Copy
        | MenuAction::CopySelection
        | MenuAction::ClosePane
        | MenuAction::MovePanel(_)
        | MenuAction::RenamePanel => GlobalMenuGroup::Panel,
        MenuAction::FindInTerminal | MenuAction::ClearTerminal | MenuAction::LastPanel => {
            GlobalMenuGroup::Panel
        }
        MenuAction::NewWorkspace
        | MenuAction::WorkspaceAtDirectory
        | MenuAction::NewWorktree
        | MenuAction::OpenWorktree => GlobalMenuGroup::Workspace,
        MenuAction::Detach => GlobalMenuGroup::Client,
        _ => GlobalMenuGroup::Other,
    }
}

/// The new-Tab menu's Agents section: its provider rows and "More Agents…".
const fn agent_row(action: &MenuAction) -> bool {
    matches!(
        action,
        MenuAction::StartAgentInTab(_) | MenuAction::MoreAgents
    )
}

fn heading_before(
    source: &MenuTarget,
    actions: &[MenuAction],
    index: usize,
) -> Option<MenuHeading> {
    let previous = index.checked_sub(1).map(|index| &actions[index]);
    if matches!(source, MenuTarget::TabProfiles(_)) {
        return (agent_row(&actions[index]) && !previous.is_some_and(agent_row))
            .then_some(MenuHeading::Agents);
    }
    if !matches!(source, MenuTarget::WorkspaceCreation(_)) {
        return None;
    }
    match &actions[index] {
        MenuAction::WorkspaceInFolder(_)
            if !matches!(previous, Some(MenuAction::WorkspaceInFolder(_))) =>
        {
            Some(MenuHeading::Folders)
        }
        MenuAction::OpenWorktreeAt(_)
            if !matches!(previous, Some(MenuAction::OpenWorktreeAt(_))) =>
        {
            Some(MenuHeading::Worktrees)
        }
        _ => None,
    }
}

fn separator_before(source: &MenuTarget, actions: &[MenuAction], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    let action = &actions[index];
    let previous = &actions[index - 1];
    if matches!(source, MenuTarget::Global) {
        return global_menu_group(previous) != global_menu_group(action);
    }
    if matches!(source, MenuTarget::Workspace(_)) {
        // The Worktree rows form one section; the close and removal rows form the trailing one.
        let worktree = |action: &MenuAction| {
            matches!(
                action,
                MenuAction::NewWorktree | MenuAction::WorkspaceWorktrees
            )
        };
        return worktree(action) && !worktree(previous)
            || matches!(action, MenuAction::DeleteWorkspace)
            || matches!(action, MenuAction::RemoveWorktree)
                && !matches!(previous, MenuAction::DeleteWorkspace);
    }
    match action {
        MenuAction::WorkspaceInFolder(_) => !matches!(previous, MenuAction::WorkspaceInFolder(_)),
        MenuAction::OpenWorktreeAt(_) => !matches!(previous, MenuAction::OpenWorktreeAt(_)),
        // The new-Tab menu's Agents follow its shells as their own section.
        action if agent_row(action) => !agent_row(previous),
        // The pane's copy row is its own group above the split and close commands.
        MenuAction::SplitPanel(_) => matches!(previous, MenuAction::CopySelection),
        // A Tab menu's close section starts before its first close row.
        MenuAction::CloseSelectedTabs => !matches!(previous, MenuAction::ResumeTabClose),
        MenuAction::CreateWorkspaceProject
        | MenuAction::RenameWorkspace
        | MenuAction::DeleteWorkspace
        | MenuAction::DeleteProject
        | MenuAction::RemoveWorktree
        | MenuAction::ManageGroups
        | MenuAction::ResumeTabClose
        | MenuAction::CreateGroup
        | MenuAction::Ungroup
        | MenuAction::ClosePane => true,
        _ => false,
    }
}

struct Menu {
    source: MenuTarget,
    labels: Vec<Option<String>>,
    notes: Vec<Option<MenuRowNote>>,
    tabs: Option<TabMenuPresentation>,
    title: String,
    actions: Vec<MenuAction>,
    unavailable: Vec<Option<MenuUnavailable>>,
    result_actions: Vec<usize>,
    query: Option<LineEditor>,
    search_children: Vec<MenuSearchChild>,
    pending_selected_action: Option<MenuAction>,
    selected: usize,
    first: usize,
    visible_rows: usize,
    follow_selection: bool,
    token: MenuToken,
    drawn: Option<MenuDrawProof>,
    /// The draw an availability refresh revoked, still on screen until the next presented frame.
    refreshed_draw: Option<MenuDrawProof>,
    surface_presented: bool,
    anchor: Option<MenuAnchor>,
    candidate_hidden: bool,
}

/// A parent kept painted, without input, beside its cascaded child.
///
/// It is a frozen projection for drawing only; returning reopens the parent through its owner.
#[derive(Clone, Debug)]
pub(crate) struct RetainedMenu {
    source: MenuTarget,
    title: String,
    labels: Vec<Option<String>>,
    notes: Vec<Option<MenuRowNote>>,
    tabs: Option<TabMenuPresentation>,
    actions: Vec<MenuAction>,
    unavailable: Vec<Option<MenuUnavailable>>,
    #[cfg(test)]
    result_actions: Vec<usize>,
    first: usize,
    open_row: usize,
    token: MenuToken,
    anchor: Option<MenuAnchor>,
}

impl RetainedMenu {
    pub(crate) fn set_title(&mut self, title: &str) {
        title.clone_into(&mut self.title);
    }

    /// The parent as last drawn, with its opening row as the selected row. Its window follows that
    /// row, so a resize can never scroll the row a submenu is level with out of view.
    pub(crate) fn view(&self, has_parent: bool) -> MenuView<'_> {
        MenuView {
            source: &self.source,
            context_menu: true,
            has_parent,
            title: &self.title,
            labels: &self.labels,
            notes: &self.notes,
            tabs: self.tabs.as_ref(),
            actions: &self.actions,
            unavailable: &self.unavailable,
            #[cfg(test)]
            result_actions: &self.result_actions,
            query: None,
            query_cursor: 0,
            search_children: &[],
            selected: self.open_row,
            pending_selected_action: None,
            first: self.first,
            follow_selection: true,
            token: self.token,
            anchor: self.anchor,
            candidate_hidden: false,
            transferring: false,
        }
    }

    /// The action on one painted row. A cascade parent is a context menu without a query, so its
    /// rows are its actions in order.
    pub(crate) fn row_action(&self, row: usize) -> Option<&MenuAction> {
        self.actions.get(row)
    }
}

#[derive(Default)]
pub(crate) struct ActionMenu {
    next: u64,
    menu: Option<Menu>,
    confirmed: Option<MenuAction>,
    /// A click confirmed the action; a child it opens starts without a candidate.
    confirmed_by_pointer: bool,
    presentation: PresentationPhase,
}

impl ActionMenu {
    fn allocate(&mut self) -> Option<MenuToken> {
        self.next = self.next.checked_add(1)?;
        Some(MenuToken {
            revision: self.next,
            lineage: self.next,
        })
    }

    pub(crate) fn select_action(&mut self, action: MenuAction) {
        if self.menu.as_ref().is_some_and(|menu| {
            menu.query
                .as_ref()
                .is_some_and(|query| !query.text().is_empty())
                && menu.actions.contains(&action)
        }) {
            let Some(token) = self.allocate() else {
                self.cancel();
                return;
            };
            let Some(menu) = &mut self.menu else {
                return;
            };
            menu.pending_selected_action = Some(action);
            menu.selected = 0;
            menu.first = 0;
            menu.follow_selection = true;
            menu.candidate_hidden = false;
            menu.visible_rows = 0;
            menu.token = token;
            menu.drawn = None;
            return;
        }
        if let Some(index) = self.menu.as_ref().and_then(|menu| {
            menu.result_actions
                .iter()
                .position(|action_index| menu.actions.get(*action_index) == Some(&action))
        }) {
            self.select_row(index);
        }
    }
    pub(crate) fn key(&mut self, event: KeyEvent) -> bool {
        if event.kind == KeyKind::Release {
            return false;
        }
        if event.code == KeyCode::Esc && event.mods == Modifiers::NONE {
            self.cancel();
            return false;
        }
        let hidden = self.candidate_hidden();
        if Binding::new(KeyCode::Enter, Modifiers::NONE).matches(event) {
            if hidden {
                self.select(MenuSelection::First);
                return false;
            }
            return self.confirm();
        }
        let searchable = self.menu.as_ref().is_some_and(|menu| menu.query.is_some());
        if searchable && let Some(key) = LineKey::for_key(event) {
            let changed = self
                .menu
                .as_mut()
                .and_then(|menu| menu.query.as_mut())
                .is_some_and(|query| query.apply(key) == Ok(true));
            if changed {
                if key.edits_text() {
                    self.query_changed();
                } else {
                    self.query_cursor_moved();
                }
            }
            return false;
        }
        if searchable {
            match (event.mods, event.code) {
                (Modifiers::NONE, KeyCode::Backspace) => {
                    let changed = self
                        .menu
                        .as_mut()
                        .and_then(|menu| menu.query.as_mut())
                        .is_some_and(LineEditor::backspace);
                    if changed {
                        self.query_changed();
                    }
                    return false;
                }
                (mods, KeyCode::Char(character))
                    if (mods == Modifiers::NONE || mods == Modifiers::NONE.with_shift())
                        && !character.is_control() =>
                {
                    let changed = self
                        .menu
                        .as_mut()
                        .and_then(|menu| menu.query.as_mut())
                        .is_some_and(|query| query.insert(&character.to_string()).is_ok());
                    if changed {
                        self.query_changed();
                    }
                    return false;
                }
                (Modifiers::NONE, KeyCode::Space) => {
                    let changed = self
                        .menu
                        .as_mut()
                        .and_then(|menu| menu.query.as_mut())
                        .is_some_and(|query| query.insert(" ").is_ok());
                    if changed {
                        self.query_changed();
                    }
                    return false;
                }
                _ => {}
            }
        }
        if event.mods != Modifiers::NONE {
            return false;
        }
        if searchable && self.menu.as_ref().is_some_and(|menu| menu.drawn.is_none()) {
            return false;
        }
        match event.code {
            KeyCode::Right if hidden => self.select(MenuSelection::First),
            KeyCode::Right
                if self
                    .menu
                    .as_ref()
                    .and_then(|menu| {
                        menu.result_actions
                            .get(menu.selected)
                            .and_then(|index| menu.actions.get(*index))
                    })
                    .is_some_and(MenuAction::submenu) =>
            {
                return self.confirm();
            }
            KeyCode::Up | KeyCode::Char('k') => self.select(MenuSelection::Previous),
            KeyCode::Down | KeyCode::Char('j') => self.select(MenuSelection::Next),
            KeyCode::Home => self.select(MenuSelection::First),
            KeyCode::End => self.select(MenuSelection::Last),
            _ => {}
        }
        false
    }
    pub(crate) fn active(&self) -> bool {
        self.menu.is_some()
    }

    pub(crate) fn input_context(&self) -> Option<MenuInputContext> {
        let menu = self.menu.as_ref()?;
        let exact_selected = self.confirmed.is_none()
            && !menu.candidate_hidden
            && menu.drawn.is_some_and(|draw| {
                draw.token == menu.token
                    && draw.candidate_visible
                    && draw.selected == menu.selected
                    && (draw.first..draw.first.saturating_add(draw.visible_rows))
                        .contains(&menu.selected)
            });
        let readiness = if !self.presentation.interactive() || !menu.surface_presented {
            MenuInputReadiness::Recovery
        } else if exact_selected {
            MenuInputReadiness::ActivationReady
        } else {
            MenuInputReadiness::Continuation
        };
        Some(MenuInputContext {
            token: menu.token,
            phase: self.presentation,
            readiness,
        })
    }

    #[cfg(test)]
    pub(crate) fn open(
        &mut self,
        title: String,
        actions: Vec<MenuAction>,
        anchor: Option<MenuAnchor>,
    ) -> bool {
        self.open_for(MenuTarget::Global, title, actions, anchor)
    }

    pub(crate) fn open_for(
        &mut self,
        source: MenuTarget,
        title: String,
        actions: Vec<MenuAction>,
        anchor: Option<MenuAnchor>,
    ) -> bool {
        if self.active()
            || actions.is_empty()
            || actions.len() > orchbee_workbench_protocol::MAXIMUM_TAB_GROUPS_PER_CONTEXT + 16
        {
            return false;
        }
        let Some(token) = self.allocate() else {
            return false;
        };
        let searchable = matches!(&source, MenuTarget::Global | MenuTarget::TabCreate(_));
        let result_actions = (0..actions.len()).collect();
        self.menu = Some(Menu {
            source,
            title,
            labels: vec![None; actions.len()],
            notes: vec![None; actions.len()],
            tabs: None,
            unavailable: vec![None; actions.len()],
            result_actions,
            query: searchable.then(|| {
                LineEditor::new(
                    String::new(),
                    LengthLimit::Bytes(MAXIMUM_ACTION_MENU_QUERY_BYTES),
                )
            }),
            search_children: Vec::new(),
            pending_selected_action: None,
            actions,
            selected: 0,
            first: 0,
            visible_rows: 0,
            follow_selection: true,
            token,
            drawn: None,
            refreshed_draw: None,
            surface_presented: false,
            anchor,
            candidate_hidden: false,
        });
        self.confirmed = None;
        self.confirmed_by_pointer = false;
        self.presentation = PresentationPhase::Pending;
        true
    }

    /// Starts a menu without a candidate; the first navigation key reveals one.
    pub(crate) fn hide_candidate(&mut self) {
        if self.confirmed.is_some() {
            return;
        }
        let Some(menu) = &mut self.menu else {
            return;
        };
        if menu.candidate_hidden {
            return;
        }
        menu.candidate_hidden = true;
        menu.selected = 0;
        menu.pending_selected_action = None;
        menu.follow_selection = true;
        menu.drawn = None;
        menu.visible_rows = 0;
    }

    pub(crate) fn candidate_hidden(&self) -> bool {
        self.menu.as_ref().is_some_and(|menu| menu.candidate_hidden)
    }

    /// Freezes the current menu for painting beside a child it opens.
    pub(crate) fn retain(&self) -> Option<RetainedMenu> {
        let menu = self.menu.as_ref()?;
        Some(RetainedMenu {
            source: menu.source.clone(),
            title: menu.title.clone(),
            labels: menu.labels.clone(),
            notes: menu.notes.clone(),
            tabs: menu.tabs.clone(),
            actions: menu.actions.clone(),
            unavailable: menu.unavailable.clone(),
            #[cfg(test)]
            result_actions: menu.result_actions.clone(),
            first: menu.first,
            open_row: menu.selected,
            token: menu.token,
            anchor: menu.anchor,
        })
    }

    pub(crate) fn query(&self) -> Option<&str> {
        self.menu.as_ref()?.query.as_ref().map(LineEditor::text)
    }

    pub(crate) fn set_query(&mut self, query: Option<&str>) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        let Some(editor) = &mut menu.query else {
            return;
        };
        if editor.text() == query.unwrap_or_default() {
            return;
        }
        let mut next = LineEditor::new(
            String::new(),
            LengthLimit::Bytes(MAXIMUM_ACTION_MENU_QUERY_BYTES),
        );
        if let Some(query) = query {
            let _ = next.insert(query);
        }
        editor.clear();
        *editor = next;
        self.query_changed();
    }

    pub(crate) fn set_search_children(&mut self, children: Vec<MenuSearchChild>) {
        let Some(menu) = &self.menu else {
            return;
        };
        if menu.search_children == children {
            return;
        }
        if self.confirmed.is_some() {
            self.cancel();
            return;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return;
        };
        let Some(menu) = &mut self.menu else {
            return;
        };
        menu.search_children = children;
        menu.token = token;
        menu.drawn = None;
        menu.visible_rows = 0;
    }

    pub(crate) fn set_labels(&mut self, label: impl Fn(MenuAction) -> Option<String>) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        let labels: Vec<_> = menu
            .actions
            .iter()
            .map(|action| label(action.clone()))
            .collect();
        if menu.labels == labels {
            return;
        }
        if self.confirmed.is_some() {
            self.cancel();
            return;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return;
        };
        let Some(menu) = &mut self.menu else {
            return;
        };
        menu.labels = labels;
        menu.token = token;
        menu.drawn = None;
        menu.visible_rows = 0;
    }

    /// The heading is a display fact under the same successful-draw fence as row copy. Reports
    /// whether it changed.
    pub(crate) fn set_title(&mut self, title: &str) -> bool {
        if self.menu.as_ref().is_none_or(|menu| menu.title == title) {
            return false;
        }
        if self.confirmed.is_some() {
            self.cancel();
            return true;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return true;
        };
        let Some(menu) = &mut self.menu else {
            return false;
        };
        title.clone_into(&mut menu.title);
        menu.token = token;
        menu.drawn = None;
        menu.visible_rows = 0;
        true
    }

    /// Row notes are display facts under the same successful-draw fence as row copy.
    pub(crate) fn set_notes(&mut self, note: impl Fn(&MenuAction) -> Option<MenuRowNote>) {
        let Some(menu) = &self.menu else {
            return;
        };
        let notes: Vec<_> = menu.actions.iter().map(note).collect();
        if menu.notes == notes {
            return;
        }
        if self.confirmed.is_some() {
            self.cancel();
            return;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return;
        };
        let Some(menu) = &mut self.menu else {
            return;
        };
        menu.notes = notes;
        menu.token = token;
        menu.drawn = None;
        menu.visible_rows = 0;
    }

    /// Replaces captured display facts under the same successful-draw fence as row copy.
    pub(crate) fn set_tab_presentation(
        &mut self,
        scope: TabMenuScope,
        applied_color: Option<orchbee_workbench_protocol::TabGroupColor>,
        row: impl Fn(&MenuAction) -> Option<TabMenuScope>,
    ) {
        let Some(menu) = &self.menu else {
            return;
        };
        let Some(rows) = menu.actions.iter().map(row).collect() else {
            self.cancel();
            return;
        };
        let tabs = TabMenuPresentation {
            scope,
            applied_color,
            rows,
        };
        if menu.tabs.as_ref() == Some(&tabs) {
            return;
        }
        if self.confirmed.is_some() {
            self.cancel();
            return;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return;
        };
        let Some(menu) = &mut self.menu else {
            return;
        };
        menu.tabs = Some(tabs);
        menu.token = token;
        menu.drawn = None;
        menu.visible_rows = 0;
    }

    pub(crate) fn view(&self) -> Option<MenuView<'_>> {
        self.menu.as_ref().map(|menu| MenuView {
            source: &menu.source,
            context_menu: menu.source.context_menu(),
            has_parent: false,
            title: &menu.title,
            labels: &menu.labels,
            notes: &menu.notes,
            tabs: menu.tabs.as_ref(),
            actions: &menu.actions,
            unavailable: &menu.unavailable,
            #[cfg(test)]
            result_actions: &menu.result_actions,
            query: menu.query.as_ref().map(LineEditor::text),
            query_cursor: menu.query.as_ref().map_or(0, LineEditor::cursor),
            search_children: &menu.search_children,
            selected: menu.selected,
            pending_selected_action: menu.pending_selected_action.as_ref(),
            first: menu.first,
            follow_selection: menu.follow_selection,
            token: menu.token,
            anchor: menu.anchor,
            candidate_hidden: menu.candidate_hidden,
            transferring: self.confirmed.is_some(),
        })
    }

    /// Commits the exact row window from one completed menu frame.
    pub(crate) fn presented(&mut self, draw: MenuDraw) -> bool {
        let Some(menu) = &self.menu else {
            return false;
        };
        let empty_search_result = menu.query.is_some()
            && draw.result_actions.is_empty()
            && draw.first == 0
            && draw.visible_rows == 0
            && draw.selected == 0;
        if menu.token != draw.token
            || (draw.visible_rows == 0 && !empty_search_result)
            || (!empty_search_result && draw.first >= draw.result_actions.len())
            || (!empty_search_result && draw.selected >= draw.result_actions.len())
            || (draw.selected != menu.selected
                && !menu.pending_selected_action.as_ref().is_some_and(|action| {
                    draw.result_actions
                        .get(draw.selected)
                        .and_then(|index| menu.actions.get(*index))
                        == Some(action)
                })
                && !(menu.selected >= draw.result_actions.len() && draw.selected == 0))
            || draw.candidate_visible == menu.candidate_hidden
            || draw.first.saturating_add(draw.visible_rows) > draw.result_actions.len()
            || draw
                .result_actions
                .iter()
                .any(|index| *index >= menu.actions.len())
            || draw
                .result_actions
                .windows(2)
                .any(|indices| indices[0] >= indices[1])
        {
            self.revoke_exact_draw();
            return false;
        }
        let proof = MenuDrawProof {
            token: draw.token,
            first: draw.first,
            visible_rows: draw.visible_rows,
            selected: draw.selected,
            candidate_visible: draw.candidate_visible,
        };
        let Some(menu) = &mut self.menu else {
            return false;
        };
        menu.result_actions = draw.result_actions;
        menu.selected = draw.selected;
        menu.pending_selected_action = None;
        menu.first = draw.first;
        menu.visible_rows = draw.visible_rows;
        menu.drawn = Some(proof);
        menu.refreshed_draw = None;
        menu.surface_presented = true;
        self.presentation.presented();
        true
    }

    /// Revokes row-level activation while preserving the admitted whole-surface lease.
    fn revoke_exact_draw(&mut self) {
        if let Some(menu) = &mut self.menu {
            menu.drawn = None;
            menu.refreshed_draw = None;
            menu.visible_rows = 0;
        }
    }

    /// Revokes both exact activation and proof that this owner occupied the latest whole frame.
    pub(crate) fn not_presented(&mut self) {
        let rekey = self
            .menu
            .as_ref()
            .is_some_and(|menu| menu.surface_presented || menu.drawn.is_some());
        let token = if rekey {
            let Some(token) = self.allocate() else {
                self.cancel();
                return;
            };
            Some(token)
        } else {
            None
        };
        if let Some(menu) = &mut self.menu {
            if let Some(token) = token {
                menu.token = token;
            }
            menu.drawn = None;
            menu.refreshed_draw = None;
            menu.visible_rows = 0;
            menu.surface_presented = false;
        }
    }

    /// Settles an evaluated geometry block for this retained menu incarnation.
    pub(crate) fn geometry_blocked(&mut self) -> Option<GeometrySettlement> {
        self.menu.as_ref()?;
        self.not_presented();
        let settlement = self.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.cancel();
        }
        Some(settlement)
    }

    pub(crate) fn select(&mut self, selection: MenuSelection) {
        if self.confirmed.is_some() {
            return;
        }
        let Some(menu) = &mut self.menu else {
            return;
        };
        if menu.result_actions.is_empty() {
            return;
        }
        if menu.candidate_hidden {
            // The first key reveals a candidate at the end it names, without moving past it.
            let selected = match selection {
                MenuSelection::Previous | MenuSelection::Last => menu.result_actions.len() - 1,
                MenuSelection::Next | MenuSelection::First => 0,
            };
            menu.follow_selection = false;
            self.select_row(selected);
            return;
        }
        let selected = match selection {
            MenuSelection::Previous => menu.selected.saturating_sub(1),
            MenuSelection::Next => (menu.selected + 1).min(menu.result_actions.len() - 1),
            MenuSelection::First => 0,
            MenuSelection::Last => menu.result_actions.len() - 1,
        };
        self.select_row(selected);
    }

    pub(crate) fn scroll(&mut self, token: MenuToken, up: bool, lines: usize) -> Option<MenuToken> {
        let menu = self.menu.as_ref()?;
        let draw = menu.drawn?;
        if self.confirmed.is_some()
            || !self.presentation.interactive()
            || menu.token != token
            || draw.token != token
        {
            return None;
        }
        // Derive the actual terminal-row budget from the admitted window. Separators mean the same
        // budget can contain a different number of actions at the tail.
        let view = self.view()?;
        let rows = view.result_row_height(
            &menu.result_actions,
            draw.first,
            draw.first.saturating_add(draw.visible_rows),
        );
        let mut maximum = menu.result_actions.len().saturating_sub(1);
        while maximum > 0
            && view.result_row_height(&menu.result_actions, maximum - 1, menu.result_actions.len())
                <= rows
        {
            maximum -= 1;
        }
        let first = if up {
            draw.first.saturating_sub(lines)
        } else {
            draw.first.saturating_add(lines).min(maximum)
        };
        if first == draw.first {
            return Some(token);
        }
        let Some(next) = self.allocate() else {
            self.cancel();
            return None;
        };
        let menu = self.menu.as_mut()?;
        menu.first = first;
        menu.follow_selection = false;
        menu.visible_rows = 0;
        menu.token = next;
        menu.drawn = None;
        Some(next)
    }

    fn select_row(&mut self, selected: usize) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        if selected == menu.selected && menu.follow_selection && !menu.candidate_hidden {
            return;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return;
        };
        let Some(menu) = &mut self.menu else {
            return;
        };
        menu.selected = selected;
        menu.pending_selected_action = None;
        menu.follow_selection = true;
        menu.candidate_hidden = false;
        menu.visible_rows = 0;
        menu.token = token;
        menu.drawn = None;
    }

    /// A cursor-only query edit repaints the query row without changing results or selection.
    fn query_cursor_moved(&mut self) {
        if self.confirmed.is_some() {
            return;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return;
        };
        if let Some(menu) = &mut self.menu {
            menu.token = token;
            menu.drawn = None;
        }
    }

    fn query_changed(&mut self) {
        if self.confirmed.is_some() {
            return;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return;
        };
        let Some(menu) = &mut self.menu else {
            return;
        };
        menu.token = token;
        menu.selected = 0;
        menu.pending_selected_action = None;
        menu.first = 0;
        menu.visible_rows = 0;
        menu.follow_selection = true;
        menu.drawn = None;
        if menu
            .query
            .as_ref()
            .is_some_and(|query| query.text().is_empty())
        {
            menu.result_actions = (0..menu.actions.len()).collect();
        }
    }

    /// Reasons come from the operation owner; changing them revokes the painted proof under a
    /// token of the same lineage. The revoked draw stays recorded only as the frame still on
    /// screen, on which a pointer press may have been made.
    pub(crate) fn refresh_availability(
        &mut self,
        mut query: impl FnMut(MenuAction) -> Option<MenuUnavailable>,
    ) -> bool {
        let Some(menu) = &mut self.menu else {
            return false;
        };
        let unavailable: Vec<_> = menu
            .actions
            .iter()
            .map(|action| query(action.clone()))
            .collect();
        if menu.unavailable == unavailable {
            return false;
        }
        if self.confirmed.is_some() {
            self.cancel();
            return true;
        }
        let Some(token) = self.allocate() else {
            self.cancel();
            return true;
        };
        let Some(menu) = &mut self.menu else {
            return false;
        };
        menu.unavailable = unavailable;
        menu.token = MenuToken {
            lineage: menu.token.lineage,
            ..token
        };
        if let Some(draw) = menu.drawn.take() {
            menu.refreshed_draw = Some(draw);
        }
        menu.visible_rows = 0;
        true
    }

    pub(crate) fn confirm(&mut self) -> bool {
        if self.confirmed.is_some() {
            return false;
        }
        let Some(menu) = &self.menu else {
            return false;
        };
        let Some(action_index) = menu.result_actions.get(menu.selected).copied() else {
            return false;
        };
        if !self
            .input_context()
            .is_some_and(MenuInputContext::activation_ready)
            || menu.unavailable[action_index].is_some()
        {
            return false;
        }
        self.confirmed = Some(menu.actions[action_index].clone());
        self.confirmed_by_pointer = false;
        true
    }

    fn drawn_enabled_row(&self, token: MenuToken, row: usize) -> bool {
        self.menu
            .as_ref()
            .is_some_and(|menu| menu.token == token && self.enabled_row_in(menu.drawn, token, row))
    }

    /// A press and its release are one click when the pressed row is still the same action in
    /// the same place and enabled now: it was pressed under the current token, or under an
    /// earlier one whose draw is still on screen and since which only availability changed.
    fn clicked_enabled_row(&self, token: MenuToken, row: usize) -> bool {
        self.menu.as_ref().is_some_and(|menu| {
            if menu.token == token {
                self.enabled_row_in(menu.drawn, token, row)
            } else {
                menu.token.refreshes(token) && self.enabled_row_in(menu.refreshed_draw, token, row)
            }
        })
    }

    fn enabled_row_in(&self, draw: Option<MenuDrawProof>, token: MenuToken, row: usize) -> bool {
        let (Some(menu), Some(draw)) = (&self.menu, draw) else {
            return false;
        };
        self.confirmed.is_none()
            && self.presentation.interactive()
            && draw.token == token
            && row < menu.result_actions.len()
            && row >= draw.first
            && row < draw.first.saturating_add(draw.visible_rows)
            && menu
                .result_actions
                .get(row)
                .is_some_and(|index| menu.unavailable[*index].is_none())
    }

    /// Movement changes only the provisional menu candidate, never confirms an action.
    /// The painted window remains anchored until explicit keyboard navigation resumes.
    pub(crate) fn hover(&mut self, token: MenuToken, row: usize) -> bool {
        if !self.drawn_enabled_row(token, row) {
            return false;
        }
        let Some(menu) = &mut self.menu else {
            return false;
        };
        if menu.selected == row && !menu.candidate_hidden {
            return false;
        }
        menu.selected = row;
        menu.candidate_hidden = false;
        menu.pending_selected_action = None;
        menu.follow_selection = false;
        // Hover preserves the exact row window so every motion in a packet can update the
        // candidate. Enter still requires a draw whose selected row matches this candidate.
        true
    }

    pub(crate) fn click(&mut self, token: MenuToken, row: usize) -> bool {
        if !self.clicked_enabled_row(token, row) {
            return false;
        }
        self.activate_row(row)
    }

    fn activate_row(&mut self, row: usize) -> bool {
        let Some(menu) = &mut self.menu else {
            return false;
        };
        menu.selected = row;
        menu.candidate_hidden = false;
        self.confirmed = Some(menu.actions[menu.result_actions[row]].clone());
        self.confirmed_by_pointer = true;
        true
    }

    /// A pointer resting on a drawn submenu row opens it as a click would; any other row ignores
    /// the rest.
    pub(crate) fn rest(&mut self, token: MenuToken, row: usize) -> bool {
        let submenu = self.menu.as_ref().is_some_and(|menu| {
            menu.result_actions
                .get(row)
                .is_some_and(|index| menu.actions[*index].submenu())
        });
        submenu && self.drawn_enabled_row(token, row) && self.activate_row(row)
    }

    pub(crate) fn confirmed(&self) -> Option<MenuAction> {
        self.confirmed.clone()
    }

    pub(crate) const fn confirmed_by_pointer(&self) -> bool {
        self.confirmed_by_pointer
    }

    pub(crate) fn take(&mut self) -> Option<MenuAction> {
        let action = self.confirmed.take()?;
        self.menu = None;
        self.confirmed_by_pointer = false;
        self.presentation = PresentationPhase::Pending;
        Some(action)
    }

    pub(crate) fn cancel(&mut self) {
        self.menu = None;
        self.confirmed = None;
        self.confirmed_by_pointer = false;
        self.presentation = PresentationPhase::Pending;
    }
}

#[cfg(test)]
mod tests;
