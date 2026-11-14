//! Menu adapters for canonical Tab configuration and identity-keyed, Client-local group disclosure.
//! The existing setup lane owns admission, unknown outcomes, publication recovery and selection.
use super::{ClientActionDisposition, OperationDeadlines, WorkbenchClientState, WorkbenchClientUi};
use crate::action_menu::{MenuAction, MenuTarget, MenuUnavailable, TabMenuScope};
use crate::workbench_projection::ClientTab;
use crate::workbench_state::workspace_setup::SetupIntent;
use crate::workbench_state::wrapped_tab_move;
use orchbee_workbench_protocol::{
    TabConfiguration, TabGroupColor, WorkbenchTabGroupId, WorkbenchTabGroupLabel, WorkbenchTabId,
    WorkbenchTabIdSet, WorkbenchTabMoveDirection, configure_tabs,
};
use std::num::NonZeroUsize;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(super) enum MenuPage {
    Group,
    Colors,
    Choices,
}

pub(super) fn menu_actions(
    state: &WorkbenchClientState,
    tab: &ClientTab,
    submenu: Option<MenuPage>,
) -> Vec<MenuAction> {
    use MenuAction::*;
    let selected = state.selected_tab_ids();
    let targets: Vec<_> = state
        .cache()
        .tabs()
        .iter()
        .filter(|candidate| {
            if selected.contains(tab.tab_id()) {
                selected.contains(candidate.tab_id())
            } else {
                candidate.tab_id() == tab.tab_id()
            }
        })
        .collect();
    match submenu {
        None => {
            let mut actions = Vec::new();
            for action in crate::action_menu::TAB_ACTIONS {
                if *action == CloseOtherTabs
                    && close_tab_ids(state, tab.tab_id(), action.clone())
                        .is_some_and(|ids| ids.is_empty())
                {
                    continue;
                }
                if *action == CloseSelectedTabs && state.tab_close_resume_available(tab.tab_id()) {
                    actions.push(ResumeTabClose);
                }
                actions.push(
                    if *action == PinTab && targets.iter().all(|tab| tab.pinned()) {
                        UnpinTab
                    } else {
                        action.clone()
                    },
                );
                if *action == ManageGroups && targets.iter().any(|tab| tab.group().is_some()) {
                    actions.push(RemoveFromGroup);
                }
            }
            actions
        }
        Some(MenuPage::Colors) if tab.group().is_some() => [
            TabGroupColor::Blue,
            TabGroupColor::Cyan,
            TabGroupColor::Green,
            TabGroupColor::Grey,
            TabGroupColor::Orange,
            TabGroupColor::Pink,
            TabGroupColor::Purple,
            TabGroupColor::Red,
            TabGroupColor::Yellow,
        ]
        .into_iter()
        .map(GroupColor)
        .collect(),
        Some(MenuPage::Colors) => Vec::new(),
        // A header click folds the group and dragging moves it, so neither is a row.
        Some(MenuPage::Group) if tab.group().is_some() => vec![
            NewTabInGroup,
            RenameGroup,
            ChooseGroupColor,
            Ungroup,
            CloseGroup,
        ],
        Some(MenuPage::Group) => Vec::new(),
        Some(MenuPage::Choices) => {
            let mut actions: Vec<_> = state
                .cache()
                .groups()
                .iter()
                .enumerate()
                .filter(|(_, group)| tab.group() != Some(*group))
                .filter_map(|(index, _)| u16::try_from(index).ok().map(JoinGroup))
                .collect();
            actions.push(CreateGroup);
            actions
        }
    }
}

/// The Tab that the global menu's Tab rows address, as execution resolves it without a target.
pub(super) fn current_tab(state: &WorkbenchClientState) -> Option<&ClientTab> {
    state.cache().tabs().get(state.active()?.index())
}

/// The direction a global Move Tab row moves its Tab, or `None` for any other action.
pub(super) const fn tab_move_direction(action: &MenuAction) -> Option<WorkbenchTabMoveDirection> {
    match action {
        MenuAction::MoveTabPrevious => Some(WorkbenchTabMoveDirection::Previous),
        MenuAction::MoveTabNext => Some(WorkbenchTabMoveDirection::Next),
        MenuAction::MoveTabFirst => Some(WorkbenchTabMoveDirection::First),
        MenuAction::MoveTabLast => Some(WorkbenchTabMoveDirection::Last),
        _ => None,
    }
}

/// The move a Move Tab row requests for the Tab at `index`, or why the Server's own configuration
/// rule would leave the Tabs unchanged. Previous and Next wrap within the Tabs pinned as this one,
/// so they refuse only a Tab with none to pass. The row, its move and the `<` and `>` keys all ask
/// here, so an enabled row always moves the Tab or takes it out of its group, and a key with nothing
/// to move stays silent. A move the local Workspace cannot judge stays available for the Server.
pub(super) fn tab_move(
    state: &WorkbenchClientState,
    index: usize,
    requested: WorkbenchTabMoveDirection,
) -> Option<Result<TabConfiguration, MenuUnavailable>> {
    let tab_id = state.cache().tabs().get(index)?.tab_id().clone();
    let context = state.cache().canonical_context();
    let direction = context
        .and_then(|context| {
            let pinned = context.tabs.iter().find(|tab| tab.tab_id == tab_id)?.pinned;
            let peers = context.tabs.iter().filter(|tab| tab.pinned == pinned);
            let position = peers.clone().position(|tab| tab.tab_id == tab_id)?;
            Some(wrapped_tab_move(requested, position, peers.count()))
        })
        .unwrap_or(requested);
    let adjacent = if direction == WorkbenchTabMoveDirection::Previous {
        index.checked_sub(1)
    } else {
        index.checked_add(1)
    };
    let operation = TabConfiguration::MoveTab {
        tab_id: tab_id.clone(),
        direction,
        adjacent_group_collapsed: adjacent
            .and_then(|index| state.cache().tabs().get(index))
            .and_then(ClientTab::group)
            .is_some_and(|group| state.group_collapsed(&group.group_id)),
    };
    let Some(context) = context else {
        return Some(Ok(operation));
    };
    if !matches!(configure_tabs(context, &operation, None), Ok(None)) {
        return Some(Ok(operation));
    }
    // A Tab moves only among Tabs pinned as it is; past those, only an ungrouped Tab already at
    // the start or end stays put on Move to start or Move to end.
    let pinned = context
        .tabs
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .is_some_and(|tab| tab.pinned);
    let peers = context
        .tabs
        .iter()
        .filter(|tab| tab.pinned == pinned)
        .count();
    Some(Err(if context.tabs.len() == 1 {
        MenuUnavailable::OnlyOneTab
    } else if peers == 1 && pinned {
        MenuUnavailable::NoOtherPinnedTab
    } else if peers == 1 {
        MenuUnavailable::NoOtherUnpinnedTab
    } else if requested == WorkbenchTabMoveDirection::First {
        MenuUnavailable::TabAlreadyFirst
    } else {
        MenuUnavailable::TabAlreadyLast
    }))
}

fn configuration_scope(state: &WorkbenchClientState, target: &WorkbenchTabId) -> TabMenuScope {
    match NonZeroUsize::new(configuration_tab_ids(state, target).len()) {
        Some(count) if count.get() > 1 => TabMenuScope::Selected(count),
        _ => TabMenuScope::Addressed,
    }
}

fn group_scope(state: &WorkbenchClientState, tab: &ClientTab) -> Option<TabMenuScope> {
    NonZeroUsize::new(close_tab_ids(state, tab.tab_id(), MenuAction::CloseGroup)?.len())
        .map(TabMenuScope::Group)
}

pub(super) fn menu_scope(
    state: &WorkbenchClientState,
    tab: &ClientTab,
    page: Option<MenuPage>,
) -> Option<TabMenuScope> {
    match page {
        Some(MenuPage::Group | MenuPage::Colors) => group_scope(state, tab),
        _ => Some(configuration_scope(state, tab.tab_id())),
    }
}

/// Project the same exact owner-resolved set that execution will revalidate; zero is not a scope.
pub(super) fn menu_row(
    state: &WorkbenchClientState,
    tab: &ClientTab,
    action: &MenuAction,
) -> Option<TabMenuScope> {
    use MenuAction::*;
    let scope = match action {
        CloseSelectedTabs | CloseOtherTabs | CloseTrailingTabs | CloseGroup => {
            let count =
                NonZeroUsize::new(close_tab_ids(state, tab.tab_id(), action.clone())?.len())?;
            match action {
                CloseSelectedTabs if count.get() == 1 => TabMenuScope::Addressed,
                CloseSelectedTabs => TabMenuScope::Selected(count),
                CloseOtherTabs => TabMenuScope::Other(count),
                CloseTrailingTabs => TabMenuScope::Trailing(count),
                _ => TabMenuScope::Group(count),
            }
        }
        DuplicateTab | PinTab | UnpinTab | ManageGroups | CreateGroup | JoinGroup(_)
        | RemoveFromGroup => configuration_scope(state, tab.tab_id()),
        RenameGroup | ChooseGroupColor | GroupColor(_) | NewTabInGroup | Ungroup => {
            group_scope(state, tab)?
        }
        SelectAllTabs | ClearTabSelection => TabMenuScope::Workspace,
        _ => TabMenuScope::Addressed,
    };
    Some(scope)
}

pub(super) fn configuration_tab_ids(
    state: &WorkbenchClientState,
    target: &WorkbenchTabId,
) -> Vec<WorkbenchTabId> {
    let selected = state.selected_tab_ids();
    if selected.contains(target) {
        selected
    } else {
        vec![target.clone()]
    }
}

pub(super) fn close_tab_ids(
    state: &WorkbenchClientState,
    target: &WorkbenchTabId,
    action: MenuAction,
) -> Option<Vec<WorkbenchTabId>> {
    use crate::workbench_state::{CloseSetKind, close_set_targets};
    let kind = match action {
        MenuAction::CloseSelectedTabs => CloseSetKind::Selected,
        MenuAction::CloseOtherTabs => CloseSetKind::Other,
        MenuAction::CloseTrailingTabs => CloseSetKind::Trailing,
        MenuAction::CloseGroup => CloseSetKind::Group,
        _ => return None,
    };
    Some(close_set_targets(
        &state.cache().canonical_context()?.tabs,
        &state.selected_tab_ids(),
        target,
        kind,
    ))
}

impl WorkbenchClientUi {
    pub(in crate::workbench_client) fn tab_drag_source(
        &mut self,
    ) -> Option<crate::workbench_client::input::TabDragSource> {
        if self.retirement_failure.is_some()
            || self.action_menu_active()
            || self.modal_owns_input()
            || self.copy_mode_active()
            || self.popup.captured()
            || self.board.is_open()
        {
            return None;
        }
        let state = self.state.as_ref()?;
        if state.setup_blocks_structure() || !state.setup_authority_available() {
            return None;
        }
        let selected = state.selected_tab_ids();
        Some(crate::workbench_client::input::TabDragSource {
            drawn: self.view().tab_create_target()?,
            tabs: state
                .cache()
                .tabs()
                .iter()
                .map(|tab| crate::workbench_client::input::TabDragTab {
                    id: tab.tab_id().clone(),
                    pinned: tab.pinned(),
                    group: tab.group().map(|group| group.group_id.clone()),
                    selected: selected.contains(tab.tab_id()),
                    collapsed: tab
                        .group()
                        .is_some_and(|group| state.group_collapsed(&group.group_id)),
                })
                .collect(),
        })
    }

    pub(in crate::workbench_client) fn request_drawn_tab_rename(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        tab: &WorkbenchTabId,
    ) -> Option<ClientActionDisposition> {
        if self.tab_drag_source()?.drawn != *source
            || self.state.as_ref()?.active()?.key().anchor().tab_id() != tab
        {
            return None;
        }
        self.request_rename_draft()?;
        Some(ClientActionDisposition::DismissNavigator)
    }

    pub(in crate::workbench_client) fn request_drawn_tab_close(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        tab: &WorkbenchTabId,
    ) {
        if self.tab_drag_source().is_some_and(|current| {
            current.drawn == *source && current.tabs.iter().any(|item| &item.id == tab)
        }) {
            let _admitted = self.request_close_set(std::slice::from_ref(tab));
        }
    }

    pub(in crate::workbench_client) fn page_drawn_tab_strip(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        start: &crate::workbench_state::TabStripStart,
    ) {
        if self
            .tab_drag_source()
            .is_some_and(|current| current.drawn == *source)
            && let Some(state) = self.state.as_mut()
        {
            state.scroll_tab_strip(start.clone());
        }
    }

    pub(in crate::workbench_client) fn follow_drawn_tab_strip(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        tab: &WorkbenchTabId,
    ) {
        if self
            .tab_drag_source()
            .is_some_and(|current| current.drawn == *source)
            && let Some(state) = self.state.as_mut()
        {
            state.follow_tab_strip_scroll(tab);
        }
    }

    pub(in crate::workbench_client) fn commit_tab_drag(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        operation: TabConfiguration,
    ) {
        let admitted = (|| {
            if self
                .tab_drag_source()
                .as_ref()
                .map(|current| &current.drawn)
                != Some(source)
            {
                return None;
            }
            let deadlines = OperationDeadlines::from_sample(self.clock.now())?;
            self.mailbox
                .as_ref()?
                .try_enqueue_tab_configuration_for(
                    self.state.as_mut()?,
                    operation,
                    deadlines,
                    &self.clock,
                )
                .ok()?;
            Some(())
        })();
        if admitted.is_none() {
            self.command_notice.set(Some((
                super::WorkbenchClientNotice::MenuUnavailable,
                self.clock.now() + std::time::Duration::from_secs(3),
            )));
        }
    }
    pub(super) fn toggle_tab_group(
        &mut self,
        group: &WorkbenchTabGroupId,
    ) -> ClientActionDisposition {
        if let Some(state) = &mut self.state {
            state.toggle_tab_group(group);
        }
        ClientActionDisposition::KeepNavigator
    }

    pub(super) fn apply_tab_menu(
        &mut self,
        action: MenuAction,
        target: Option<&orchbee_workbench_protocol::WorkbenchTabId>,
    ) -> ClientActionDisposition {
        use orchbee_workbench_protocol::TabCreationPlacement;
        if let MenuAction::NewTabWithProfile(profile) = &action {
            return self.create_tab_with_profile(profile);
        }
        if action == MenuAction::DuplicateTab {
            let target = target.cloned().or_else(|| {
                self.state
                    .as_ref()?
                    .active()
                    .map(|active| active.key().anchor().tab_id().clone())
            });
            if let Some(target) = target {
                return self.duplicate_tabs(&target);
            }
        }
        if matches!(
            action,
            MenuAction::NewTabInGroup | MenuAction::NewTabToRight | MenuAction::DuplicateTab
        ) {
            let placement = self.state.as_ref().and_then(|state| {
                let index = target
                    .and_then(|id| state.cache().tab_index(id))
                    .or_else(|| {
                        target
                            .is_none()
                            .then(|| state.active().map(|active| active.index()))
                            .flatten()
                    })?;
                let tab = state.cache().tabs().get(index)?;
                if action == MenuAction::NewTabToRight {
                    Some(TabCreationPlacement::AfterTab {
                        tab_id: tab.tab_id().clone(),
                    })
                } else if action == MenuAction::DuplicateTab {
                    Some(TabCreationPlacement::Duplicate {
                        tab_id: tab.tab_id().clone(),
                    })
                } else {
                    Some(TabCreationPlacement::InGroup {
                        group_id: tab.group()?.group_id.clone(),
                    })
                }
            });
            if let Some(placement) = placement {
                return self.create_tab_placed(Some(placement));
            }
            self.command_notice.set(Some((
                super::WorkbenchClientNotice::CreateNotCreated,
                self.clock.now() + std::time::Duration::from_secs(3),
            )));
            return ClientActionDisposition::KeepNavigator;
        }
        if self.configure_tab_menu(action, target).is_none() {
            self.command_notice.set(Some((
                super::WorkbenchClientNotice::LocalBusy,
                self.clock.now() + std::time::Duration::from_secs(3),
            )));
        }
        ClientActionDisposition::KeepNavigator
    }

    fn configure_tab_menu(
        &mut self,
        action: MenuAction,
        target: Option<&orchbee_workbench_protocol::WorkbenchTabId>,
    ) -> Option<()> {
        use MenuAction::*;
        if self.retirement_failure.is_some() {
            return None;
        }
        let state = self.state.as_ref()?;
        let index = match target {
            Some(id) => state.cache().tab_index(id)?,
            None => state.active()?.index(),
        };
        let tab = state.cache().tabs().get(index)?;
        let tab_id = tab.tab_id().clone();
        if action == ResumeTabClose {
            if !state.tab_close_resume_available(&tab_id) {
                return None;
            }
            return match self.request_tab_close_resume(&tab_id) {
                ClientActionDisposition::DismissNavigator => Some(()),
                _ => None,
            };
        }
        if let Some(ids) = close_tab_ids(state, &tab_id, action.clone()) {
            return self.request_close_set(&ids);
        }
        let selection_change = match action {
            SelectTab => Some(crate::workbench_state::TabSelectionChange::Toggle),
            SelectAllTabs => Some(crate::workbench_state::TabSelectionChange::All),
            ClearTabSelection => Some(crate::workbench_state::TabSelectionChange::Clear),
            _ => None,
        };
        if let Some(change) = selection_change {
            return self.state.as_mut()?.mark_tabs(&tab_id, change);
        }
        if matches!(action, ManageGroups | ChooseGroupColor) {
            let target = if action == ManageGroups {
                MenuTarget::TabGroupChoices(tab_id)
            } else {
                MenuTarget::TabGroups {
                    tab_id,
                    colors: true,
                }
            };
            return self.open_action_menu(target, None, false).then_some(());
        }
        let selected = WorkbenchTabIdSet::try_from(configuration_tab_ids(state, &tab_id)).ok()?;
        let operation = match action {
            PinTab | UnpinTab => TabConfiguration::SetTabsPinned {
                tab_ids: selected,
                pinned: action == PinTab,
            },
            // The name field starts empty: Enter there creates the unnamed group its help offers.
            CreateGroup => TabConfiguration::CreateGroupForTabs {
                tab_ids: selected,
                label: WorkbenchTabGroupLabel::parse("").ok()?,
                color: TabGroupColor::Blue,
            },
            JoinGroup(index) => TabConfiguration::SetTabsGroup {
                tab_ids: selected,
                group_id: Some(
                    state
                        .cache()
                        .groups()
                        .get(usize::from(index))?
                        .group_id
                        .clone(),
                ),
            },
            RemoveFromGroup => TabConfiguration::SetTabsGroup {
                tab_ids: selected,
                group_id: None,
            },
            Ungroup => TabConfiguration::Ungroup {
                group_id: tab.group()?.group_id.clone(),
            },
            RenameGroup | GroupColor(_) => {
                let group = tab.group()?;
                TabConfiguration::UpdateGroup {
                    group_id: group.group_id.clone(),
                    label: WorkbenchTabGroupLabel::parse(&group.label).ok()?,
                    color: if let GroupColor(color) = action {
                        color
                    } else {
                        group.color
                    },
                }
            }
            MoveTabPrevious | MoveTabNext | MoveTabFirst | MoveTabLast => {
                // Nothing to move: the row is disabled by the same rule, and a key stays silent.
                match tab_move(state, index, tab_move_direction(&action)?)? {
                    Ok(operation) => operation,
                    Err(_) => return Some(()),
                }
            }
            _ => return None,
        };
        if matches!(action, CreateGroup | RenameGroup) {
            self.request_setup_draft(SetupIntent::TabGroupName(Box::new(operation)))?;
        } else {
            let deadlines = OperationDeadlines::from_sample(self.clock.now())?;
            self.mailbox
                .as_ref()?
                .try_enqueue_tab_configuration_for(
                    self.state.as_mut()?,
                    operation,
                    deadlines,
                    &self.clock,
                )
                .ok()?;
        }
        Some(())
    }
}
