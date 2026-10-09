//! One local menu intent; existing membership and action owners retain all execution authority.
use super::*;
use crate::action_menu::{
    ActionMenu, DrawnMenuSource, MenuAction, MenuDraw, MenuTarget, MenuToken, MenuUnavailable,
    MenuView,
};
use crate::client_view::KeyEvent;
use crate::terminal_session::workbench_projection::{
    NativeContextLocation, NativeMenuContextSource, NativeSimpleTab,
};
use std::time::{Duration, Instant};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Origin {
    Unavailable,
    Context(Box<ContextOrigin>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContextOrigin {
    site: orchbee_workbench_protocol::SiteId,
    location: NativeContextLocation,
    desired: Option<PaneKey>,
    focus: Option<PaneRef>,
    tab: Option<NativeSimpleTab>,
    source: Option<NativeMenuContextSource>,
}

impl Origin {
    fn capture(state: Option<&NativeSessionState>) -> Self {
        state.map_or(Self::Unavailable, |state| {
            Self::Context(Box::new(ContextOrigin {
                site: state.cache().site_id().clone(),
                location: state.cache().location().clone(),
                desired: state.desired().cloned(),
                focus: state.focus_pane_ref(),
                tab: state
                    .desired()
                    .and_then(|key| state.cache().tab_index(key.anchor().tab_id()))
                    .and_then(|i| state.cache().tabs().get(i))
                    .cloned(),
                source: state.cache().menu_context_source(state.cache().location()),
            }))
        })
    }
}

fn selected_presentation(
    state: &NativeSessionState,
    pane: &PaneRef,
) -> Option<orchbee_workbench_protocol::TerminalPresentationId> {
    let tab = state
        .cache()
        .tabs()
        .get(state.cache().tab_index(pane.anchor().tab_id())?)?;
    let panel = tab.panel(tab.panel_index(pane.pane().panel())?)?;
    Some(state.selected_presentation(panel).presentation_id().clone())
}

#[derive(Clone)]
enum Target {
    Global,
    Pane {
        pane: PaneRef,
        presentation: orchbee_workbench_protocol::TerminalPresentationId,
        source: NativeMenuContextSource,
    },
    Tab {
        location: NativeContextLocation,
        tab: Box<NativeSimpleTab>,
        source: NativeMenuContextSource,
    },
    Workspace {
        location: NativeContextLocation,
        source: NativeMenuContextSource,
    },
    Project {
        project: Box<orchbee_workbench_protocol::Project>,
        context: Option<Box<ProjectContext>>,
    },
}

#[derive(Clone)]
struct ProjectContext {
    location: NativeContextLocation,
    source: NativeMenuContextSource,
}

fn action_unavailable(
    state: Option<&NativeSessionState>,
    target: &Target,
    action: MenuAction,
    pointer_released: bool,
) -> Option<MenuUnavailable> {
    use super::super::state::workspace_setup::DirectoryConfigurationUnavailable;
    if matches!(target, Target::Project { .. }) && action == MenuAction::QuickWorkspace {
        return state
            .is_none_or(|state| !state.quick_workspace_available())
            .then_some(MenuUnavailable::OperationBusy);
    }
    if matches!(target, Target::Project { context: None, .. }) {
        return Some(MenuUnavailable::ProjectWorkspaceRequired);
    }
    if action == MenuAction::ReleasePointer && pointer_released {
        return Some(MenuUnavailable::PointerReleased);
    }
    if action != MenuAction::WorkspaceDirectory {
        return None;
    }
    let Target::Workspace { location, source } = target else {
        return None;
    };
    if matches!(
        source,
        NativeMenuContextSource::Workspace {
            directory: Some(_),
            ..
        }
    ) {
        return Some(MenuUnavailable::DirectoryConfigured);
    }
    let state = state.filter(|state| state.cache().location() == location)?;
    state
        .directory_configuration_unavailable()
        .map(|reason| match reason {
            DirectoryConfigurationUnavailable::Configured => MenuUnavailable::DirectoryConfigured,
            DirectoryConfigurationUnavailable::EmptyRequired => {
                MenuUnavailable::EmptyWorkspaceRequired
            }
            DirectoryConfigurationUnavailable::WorkspaceRequired => {
                MenuUnavailable::WorkspaceRequired
            }
            DirectoryConfigurationUnavailable::Busy => MenuUnavailable::OperationBusy,
        })
}

enum Stage {
    Open,
    Confirmed(Instant),
    Context {
        generation: u64,
        prepared: bool,
        deadline: Instant,
    },
    Tab {
        generation: u64,
        prepared: bool,
        deadline: Instant,
    },
    Wait(Origin, Instant),
    Offered(Origin, Instant),
}

impl Stage {
    fn deadline(&self) -> Option<Instant> {
        match self {
            Self::Open => None,
            Self::Confirmed(deadline)
            | Self::Context { deadline, .. }
            | Self::Tab { deadline, .. }
            | Self::Wait(_, deadline)
            | Self::Offered(_, deadline) => Some(*deadline),
        }
    }
}

/// Refresh and attachment generations belong to separate monotonic domains.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum NavigationKey {
    Context(u64),
    Tab(u64),
}

struct Intent {
    target: Target,
    origin: Origin,
    stage: Stage,
    zoomed: bool,
    pointer_released: bool,
}

#[derive(Default)]
pub(super) struct Coordinator {
    pub(super) menu: ActionMenu,
    intent: Option<Intent>,
    geometry: Option<Geometry>,
}

impl NativeSessionView<'_> {
    pub(crate) fn action_menu(&self) -> Option<MenuView<'_>> {
        self.menu?.view()
    }
    pub(crate) fn action_menu_source(&self) -> Option<DrawnMenuSource> {
        if self.retirement.is_some() {
            return None;
        }
        let state = self.state.as_ref()?;
        Some(state.action_menu_source())
    }
}

impl NativeUi {
    pub(in crate::terminal_session) fn action_menu_active(&self) -> bool {
        self.menus.menu.active()
    }

    pub(super) fn observe_action_menu_geometry(&mut self, geometry: Geometry) {
        if self
            .menus
            .geometry
            .replace(geometry)
            .is_some_and(|old| old != geometry)
        {
            self.fail_action_menu();
        }
    }

    fn menu_carrier_live(&self) -> bool {
        self.retirement_failure.is_none()
            && self.state.as_ref().is_some_and(|state| {
                state.phase_kind() != SessionPhaseKind::Retired && state.retired_failure().is_none()
            })
    }

    pub(in crate::terminal_session) fn cancel_action_menu(&mut self) {
        self.menus.menu.cancel();
        self.menus.intent = None;
    }

    pub(super) fn fail_action_menu(&mut self) {
        if !self.action_menu_active() {
            return;
        }
        self.cancel_action_menu();
        self.command_notice = self
            .clock
            .now()
            .checked_add(std::time::Duration::from_secs(5))
            .map(|until| (NativeSessionNotice::MenuUnavailable, until));
    }

    pub(in crate::terminal_session) fn open_action_menu(
        &mut self,
        target: MenuTarget,
        anchor: Option<(u16, u16)>,
        pointer_released: bool,
    ) -> bool {
        if self.action_menu_active()
            || self.modal_owns_input()
            || self.copy_mode_active()
            || self.popup.captured()
            || self.board.is_open()
        {
            return false;
        }
        if self.menus.geometry.is_none() {
            self.menus.geometry = self.state.as_ref().map(NativeSessionState::latest_geometry);
        }
        if self.menus.geometry.is_none() {
            return false;
        }
        let target = match target {
            MenuTarget::Drawn { source, target } => {
                if !self.menu_carrier_live()
                    || self.state.as_ref().is_none_or(|state| {
                        state.cache().site_id() != &source.site
                            || state.cache().revision() != source.revision
                    })
                {
                    return false;
                }
                *target
            }
            target => target,
        };
        let live = self.menu_carrier_live();
        let state = self.state.as_ref();
        let (target, title, mut actions) = match target {
            MenuTarget::Global => (
                Target::Global,
                String::new(),
                crate::action_menu::GLOBAL_ACTIONS.to_vec(),
            ),
            MenuTarget::Pane(pane) => {
                if !live {
                    return false;
                }
                let Some(state) = state else {
                    return false;
                };
                let Some(source) = state.cache().menu_context_source(state.cache().location())
                else {
                    return false;
                };
                if state
                    .pane_by_panel(pane.pane().panel())
                    .is_none_or(|record| record.pane_ref() != pane)
                {
                    return false;
                }
                let Some(presentation) = selected_presentation(state, &pane) else {
                    return false;
                };
                let Some(tab) = state
                    .cache()
                    .tab_index(pane.anchor().tab_id())
                    .and_then(|i| state.cache().tabs().get(i))
                else {
                    return false;
                };
                let Some(panel) = tab
                    .panel_index(pane.pane().panel())
                    .and_then(|i| tab.panel(i))
                else {
                    return false;
                };
                let title = state
                    .selected_presentation(panel)
                    .display_label()
                    .to_owned();
                (
                    Target::Pane {
                        pane: pane.clone(),
                        presentation,
                        source,
                    },
                    title,
                    crate::action_menu::PANE_ACTIONS.to_vec(),
                )
            }
            MenuTarget::Tab(id) => {
                if !live {
                    return false;
                }
                let Some(state) = state else {
                    return false;
                };
                let Some(source) = state.cache().menu_context_source(state.cache().location())
                else {
                    return false;
                };
                let Some(index) = state.cache().tab_index(&id) else {
                    return false;
                };
                let tab = &state.cache().tabs()[index];
                (
                    Target::Tab {
                        location: state.cache().location().clone(),
                        tab: Box::new(tab.clone()),
                        source,
                    },
                    tab.display_label(index + 1).into_owned(),
                    crate::action_menu::TAB_ACTIONS.to_vec(),
                )
            }
            MenuTarget::Project(id) => {
                if !live {
                    return false;
                }
                let Some(state) = state else {
                    return false;
                };
                let Some(project) = state.cache().projects().iter().find(|p| p.project_id == id)
                else {
                    return false;
                };
                let context = match state.cache().location() {
                    location @ NativeContextLocation::Workspace { project_id, .. }
                        if project_id == &id =>
                    {
                        let Some(source) = state.cache().menu_context_source(location) else {
                            return false;
                        };
                        Some(Box::new(ProjectContext {
                            location: location.clone(),
                            source,
                        }))
                    }
                    _ => None,
                };
                (
                    Target::Project {
                        project: Box::new(project.clone()),
                        context,
                    },
                    project.label.clone(),
                    crate::action_menu::PROJECT_ACTIONS.to_vec(),
                )
            }
            MenuTarget::Workspace(location) => {
                if !live {
                    return false;
                }
                let Some(state) = state else {
                    return false;
                };
                if !matches!(location, NativeContextLocation::Workspace { .. }) {
                    return false;
                }
                let Some(context) = state
                    .cache()
                    .navigation()
                    .iter()
                    .find(|c| c.location == location)
                else {
                    return false;
                };
                let Some(source) = state.cache().menu_context_source(&location) else {
                    return false;
                };
                (
                    Target::Workspace { location, source },
                    context
                        .workspace_label
                        .as_ref()
                        .map_or(String::new(), |l| l.as_str().to_owned()),
                    crate::action_menu::WORKSPACE_ACTIONS.to_vec(),
                )
            }
            MenuTarget::Drawn { .. } => return false,
        };
        if !live {
            actions.retain(|a| {
                matches!(
                    a,
                    MenuAction::Help
                        | MenuAction::Sidebar
                        | MenuAction::ReleasePointer
                        | MenuAction::Detach
                )
            });
        }
        let intent = Intent {
            target,
            origin: Origin::capture(state),
            stage: Stage::Open,
            zoomed: state.is_some_and(NativeSessionState::zoom_active),
            pointer_released,
        };
        if !self.menus.menu.open(title, actions, anchor) {
            return false;
        }
        self.menus.menu.refresh_availability(|action| {
            action_unavailable(state, &intent.target, action, intent.pointer_released)
        });
        if !self.menus.menu.active() {
            return false;
        }
        self.menus.intent = Some(intent);
        true
    }

    pub(in crate::terminal_session) fn capture_action_menu_key(&mut self, event: KeyEvent) {
        self.drive_action_menu();
        if self.menus.menu.key(event) {
            self.confirm_action_menu();
        }
        if !self.action_menu_active() {
            self.menus.intent = None;
        }
    }

    pub(in crate::terminal_session) fn click_action_menu(&mut self, token: MenuToken, row: usize) {
        self.drive_action_menu();
        if self.menus.menu.click(token, row) {
            self.confirm_action_menu();
        }
    }

    fn confirm_action_menu(&mut self) {
        let Some(intent) = &mut self.menus.intent else {
            return;
        };
        if !matches!(intent.stage, Stage::Open) {
            return;
        }
        let Some(deadline) = self.clock.now().checked_add(Duration::from_secs(30)) else {
            self.fail_action_menu();
            return;
        };
        intent.stage = Stage::Confirmed(deadline);
        self.drive_action_menu();
    }

    pub(super) fn after_action_menu_draw(&mut self, draw: Option<ModalPresentation<MenuDraw>>) {
        if !self.action_menu_active() {
            return;
        }
        match draw {
            Some(ModalPresentation::Presented(draw)) => self.menus.menu.draw(Some(draw)),
            _ => self.fail_action_menu(),
        }
    }

    fn target_valid(&self, target: &Target) -> bool {
        let Some(state) = self.state.as_ref() else {
            return matches!(target, Target::Global);
        };
        if !matches!(target, Target::Global) && !self.menu_carrier_live() {
            return false;
        }
        match target {
            Target::Global => true,
            Target::Pane {
                pane,
                presentation,
                source,
            } => {
                state
                    .pane_by_panel(pane.pane().panel())
                    .is_some_and(|record| record.pane_ref() == *pane)
                    && selected_presentation(state, pane).as_ref() == Some(presentation)
                    && state
                        .cache()
                        .menu_context_source(state.cache().location())
                        .as_ref()
                        == Some(source)
            }
            Target::Tab {
                location,
                tab,
                source,
            } => {
                state.cache().location() == location
                    && state.cache().menu_context_source(location).as_ref() == Some(source)
                    && state
                        .cache()
                        .tab_index(tab.tab_id())
                        .and_then(|i| state.cache().tabs().get(i))
                        == Some(tab.as_ref())
            }
            Target::Workspace { location, source } => {
                state.cache().menu_context_source(location).as_ref() == Some(source)
            }
            Target::Project { project, context } => {
                state
                    .cache()
                    .projects()
                    .iter()
                    .find(|candidate| candidate.project_id == project.project_id)
                    == Some(project.as_ref())
                    && context.as_ref().is_none_or(|context| {
                        state
                            .cache()
                            .menu_context_source(&context.location)
                            .as_ref()
                            == Some(&context.source)
                    })
            }
        }
    }

    fn menu_target_selected(&self, target: &Target, action: MenuAction) -> bool {
        match target {
            Target::Global => true,
            Target::Project { .. } if action == MenuAction::QuickWorkspace => true,
            Target::Project { context, .. } => context.as_ref().is_some_and(|context| {
                self.state
                    .as_ref()
                    .is_some_and(|state| state.cache().location() == &context.location)
            }),
            Target::Workspace { location, .. } => self
                .state
                .as_ref()
                .is_some_and(|state| state.cache().location() == location),
            Target::Tab { location, tab, .. } => self.state.as_ref().is_some_and(|state| {
                state.cache().location() == location
                    && state
                        .desired()
                        .is_some_and(|key| key.anchor().tab_id() == tab.tab_id())
            }),
            Target::Pane { pane, .. } => self.state.as_ref().is_some_and(|state| {
                state.focus_pane_ref().as_ref() == Some(pane)
                    && state.desired() == Some(pane.pane())
            }),
        }
    }

    pub(super) fn drive_action_menu(&mut self) -> bool {
        if self.action_menu_active()
            && (self.modal_owns_input()
                || self.copy_mode_active()
                || self.popup.captured()
                || self.board.is_open())
        {
            self.fail_action_menu();
            return true;
        }
        let Some(mut intent) = self.menus.intent.take() else {
            return false;
        };
        let current = Origin::capture(self.state.as_ref());
        let open = matches!(intent.stage, Stage::Open | Stage::Confirmed(_));
        if intent
            .stage
            .deadline()
            .is_some_and(|deadline| self.clock.now() >= deadline)
            || !self.target_valid(&intent.target)
            || (open && current != intent.origin)
        {
            self.fail_action_menu();
            return true;
        }
        let mut changed = false;
        match intent.stage {
            Stage::Open => {
                changed = self.menus.menu.refresh_availability(|action| {
                    action_unavailable(
                        self.state.as_ref(),
                        &intent.target,
                        action,
                        intent.pointer_released,
                    )
                });
            }
            Stage::Confirmed(deadline) => {
                if self.menus.menu.confirmed().is_some_and(|action| {
                    action_unavailable(
                        self.state.as_ref(),
                        &intent.target,
                        action,
                        intent.pointer_released,
                    )
                    .is_some()
                }) {
                    self.fail_action_menu();
                    return true;
                }
                let next = match &intent.target {
                    Target::Global => Some(Stage::Offered(current.clone(), deadline)),
                    Target::Project { .. } => self
                        .menus
                        .menu
                        .confirmed()
                        .filter(|action| self.menu_target_selected(&intent.target, *action))
                        .is_some()
                        .then(|| Stage::Offered(current.clone(), deadline)),
                    Target::Workspace { location, .. }
                        if self
                            .state
                            .as_ref()
                            .is_some_and(|s| s.cache().location() == location) =>
                    {
                        Some(Stage::Offered(current.clone(), deadline))
                    }
                    Target::Workspace { location, .. } => {
                        let index = self.state.as_ref().and_then(|s| {
                            s.cache()
                                .navigation()
                                .iter()
                                .position(|c| &c.location == location)
                        });
                        index
                            .and_then(|i| self.select_context_at_generation(i))
                            .map(|generation| Stage::Context {
                                generation,
                                prepared: false,
                                deadline,
                            })
                    }
                    Target::Tab { tab, .. }
                        if self
                            .state
                            .as_ref()
                            .and_then(|s| s.desired())
                            .is_some_and(|d| d.anchor().tab_id() == tab.tab_id()) =>
                    {
                        Some(Stage::Offered(current.clone(), deadline))
                    }
                    Target::Tab { tab, .. } => {
                        let index = self
                            .state
                            .as_ref()
                            .and_then(|s| s.cache().tab_index(tab.tab_id()));
                        index
                            .and_then(|i| u8::try_from(i + 1).ok())
                            .and_then(|p| self.select_tab_generation(ClientAction::SelectTab(p)))
                            .map(|generation| Stage::Tab {
                                generation,
                                prepared: false,
                                deadline,
                            })
                    }
                    Target::Pane { pane, .. } => {
                        let exact = self.state.as_ref().and_then(|s| s.focus_pane_ref())
                            == Some(pane.clone());
                        (exact
                            || self
                                .move_panel_focus_admitted(pane.pane().panel())
                                .is_some())
                        .then(|| Stage::Offered(Origin::capture(self.state.as_ref()), deadline))
                    }
                };
                let Some(next) = next else {
                    self.fail_action_menu();
                    return true;
                };
                intent.stage = next;
                changed = true;
            }
            Stage::Wait(ref expected, deadline) => {
                if &current != expected {
                    self.fail_action_menu();
                    return true;
                }
                if self.state.as_ref().is_none_or(|s| {
                    matches!(
                        s.phase_kind(),
                        SessionPhaseKind::Empty
                            | SessionPhaseKind::Ready
                            | SessionPhaseKind::SelectedWithoutAttachment
                            | SessionPhaseKind::RetryableAttachUnavailable
                    )
                }) {
                    intent.stage = Stage::Offered(current.clone(), deadline);
                    changed = true;
                }
            }
            Stage::Offered(ref expected, _) if &current != expected => {
                self.fail_action_menu();
                return true;
            }
            _ => {}
        }
        if self.menus.menu.active() {
            self.menus.intent = Some(intent);
        }
        changed
    }

    pub(in crate::terminal_session) fn take_action_menu_action(
        &mut self,
        pointer_released: bool,
    ) -> Option<ClientAction> {
        self.drive_action_menu();
        let intent = self.menus.intent.as_ref()?;
        if !matches!(intent.stage, Stage::Offered(_, _)) {
            return None;
        }
        let action = self.menus.menu.confirmed()?;
        if !self.target_valid(&intent.target)
            || !self.menu_target_selected(&intent.target, action)
            || action_unavailable(
                self.state.as_ref(),
                &intent.target,
                action,
                pointer_released,
            )
            .is_some()
            || (action == MenuAction::Zoom
                && self
                    .state
                    .as_ref()
                    .is_some_and(NativeSessionState::zoom_active)
                    != intent.zoomed)
            || (action == MenuAction::ReleasePointer
                && (pointer_released || pointer_released != intent.pointer_released))
        {
            self.fail_action_menu();
            return None;
        }
        let request = match (&intent.target, action) {
            (Target::Project { project, .. }, MenuAction::QuickWorkspace) => {
                let request = crate::terminal_session::ProjectQuickWorkspaceRequest::capture(
                    self.state.as_ref()?,
                    project,
                    intent.stage.deadline()?,
                )?;
                Some(request)
            }
            _ => None,
        };
        let action = self.menus.menu.take()?;
        self.menus.intent = None;
        Some(match request {
            Some(request) => ClientAction::RequestProjectQuickWorkspace(Box::new(request)),
            None => action.client_action(),
        })
    }

    pub(super) fn menu_navigation_prepared(&mut self, navigation: NavigationKey, accepted: bool) {
        let Some(intent) = &mut self.menus.intent else {
            return;
        };
        match (&mut intent.stage, navigation) {
            (
                Stage::Context {
                    generation: owned,
                    prepared,
                    ..
                },
                NavigationKey::Context(generation),
            )
            | (
                Stage::Tab {
                    generation: owned,
                    prepared,
                    ..
                },
                NavigationKey::Tab(generation),
            ) if *owned == generation => {
                if accepted {
                    *prepared = true;
                } else {
                    self.fail_action_menu();
                }
            }
            _ => {}
        }
    }

    pub(super) fn menu_navigation_installed(&mut self, navigation: NavigationKey) {
        let Some(intent) = &self.menus.intent else {
            return;
        };
        let owned = matches!((&intent.stage,navigation), (Stage::Context { generation: g, prepared:true, .. },NavigationKey::Context(generation)) | (Stage::Tab { generation:g, prepared:true, .. },NavigationKey::Tab(generation)) if *g == generation);
        if !owned {
            return;
        }
        let selected = self
            .state
            .as_ref()
            .is_some_and(|state| match &intent.target {
                Target::Workspace { location, .. } => state.cache().location() == location,
                Target::Tab { location, tab, .. } => {
                    state.cache().location() == location
                        && state
                            .desired()
                            .is_some_and(|d| d.anchor().tab_id() == tab.tab_id())
                }
                _ => false,
            });
        if !selected {
            self.fail_action_menu();
            return;
        }
        let expected = Origin::capture(self.state.as_ref());
        if let Some(intent) = &mut self.menus.intent {
            let Some(deadline) = intent.stage.deadline() else {
                return;
            };
            intent.stage = Stage::Wait(expected, deadline);
        }
    }

    pub(super) fn fence_menu_context_candidate(
        &self,
        prepared: super::super::state::PreparedRefresh,
        generation: u64,
        baseline_revision: orchbee_workbench_protocol::WorkbenchRevision,
    ) -> super::super::state::PreparedRefresh {
        let Some(Intent {
            target: Target::Workspace { location, source },
            stage: Stage::Context {
                generation: owned, ..
            },
            ..
        }) = &self.menus.intent
        else {
            return prepared;
        };
        if *owned != generation {
            return prepared;
        }
        match &prepared {
            super::super::state::PreparedRefresh::Membership {
                prepared: candidate,
                ..
            } if candidate.context_location() == location
                && candidate.menu_context_source().as_ref() == Some(source) =>
            {
                prepared
            }
            super::super::state::PreparedRefresh::Membership { .. } => {
                super::super::state::PreparedRefresh::DefiniteFailure {
                    generation,
                    baseline_revision,
                }
            }
            _ => prepared,
        }
    }

    pub(super) fn menu_switch_candidate_valid(
        &self,
        generation: u64,
        candidate: &super::super::state::PreparedMembershipCandidate,
    ) -> bool {
        let Some(Intent {
            target:
                Target::Tab {
                    location,
                    tab,
                    source,
                },
            stage: Stage::Tab {
                generation: owned, ..
            },
            ..
        }) = &self.menus.intent
        else {
            return true;
        };
        *owned != generation
            || (candidate.context_location() == location
                && candidate.menu_selected_tab_matches(tab)
                && candidate.menu_context_source().as_ref() == Some(source))
    }
}
