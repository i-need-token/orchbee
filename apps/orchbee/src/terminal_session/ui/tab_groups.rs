//! Menu adapters for canonical Tab configuration and identity-keyed, Client-local group disclosure.
//! The existing setup lane owns admission, unknown outcomes, publication recovery and selection.
use super::{NativeSessionState, NativeUi, OperationDeadlines, SessionActionDisposition};
use crate::action_menu::{MenuAction, MenuTarget};
use crate::terminal_session::state::workspace_setup::NativeSetupIntent;
use crate::terminal_session::workbench_projection::NativeSimpleTab;
use orchbee_workbench_protocol::{
    TabConfiguration, TabGroupColor, TabGroupMoveDirection, WorkbenchTabGroupId,
    WorkbenchTabGroupLabel, WorkbenchTabId, WorkbenchTabIdSet, WorkbenchTabMoveDirection,
};
#[cfg(test)]
mod tests;

pub(super) fn menu_actions(
    state: &NativeSessionState,
    tab: &NativeSimpleTab,
    submenu: Option<bool>,
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
                if *action == CloseSelectedTabs && state.tab_close_resume_available(tab.tab_id()) {
                    actions.push(ResumeTabClose);
                }
                actions.push(
                    if *action == PinTab && targets.iter().all(|tab| tab.pinned()) {
                        UnpinTab
                    } else if *action == SelectTab && selected.contains(tab.tab_id()) {
                        DeselectTab
                    } else {
                        *action
                    },
                );
            }
            actions
        }
        Some(true) => [
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
        Some(false) => {
            let mut actions = vec![CreateGroup];
            if let Some(group) = tab.group() {
                actions.extend([
                    NewTabInGroup,
                    RenameGroup,
                    ChooseGroupColor,
                    if state.group_collapsed(&group.group_id) {
                        ExpandGroup
                    } else {
                        CollapseGroup
                    },
                    Ungroup,
                    MoveGroupPrevious,
                    MoveGroupNext,
                    CloseGroup,
                ]);
            }
            if targets.iter().any(|tab| tab.group().is_some()) {
                actions.push(RemoveFromGroup);
            }
            actions.extend(
                state
                    .cache()
                    .groups()
                    .iter()
                    .enumerate()
                    .filter(|(_, group)| targets.iter().any(|tab| tab.group() != Some(*group)))
                    .filter_map(|(index, _)| u16::try_from(index).ok().map(JoinGroup)),
            );
            actions
        }
    }
}

pub(super) fn configuration_tab_ids(
    state: &NativeSessionState,
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
    state: &NativeSessionState,
    target: &WorkbenchTabId,
    action: MenuAction,
) -> Option<Vec<WorkbenchTabId>> {
    use super::super::state::{CloseSetKind, close_set_targets};
    let kind = match action {
        MenuAction::CloseSelectedTabs => CloseSetKind::Selected,
        MenuAction::CloseOtherTabs => CloseSetKind::Other,
        MenuAction::CloseTrailingTabs => CloseSetKind::Trailing,
        MenuAction::CloseGroup => CloseSetKind::Group,
        _ => return None,
    };
    Some(close_set_targets(
        &state.cache().canonical_context().tabs,
        &state.selected_tab_ids(),
        target,
        kind,
    ))
}

impl NativeUi {
    pub(super) fn toggle_tab_group(
        &mut self,
        group: &WorkbenchTabGroupId,
    ) -> SessionActionDisposition {
        if let Some(state) = &mut self.state {
            state.toggle_tab_group(group);
        }
        SessionActionDisposition::KeepNavigator
    }

    pub(super) fn apply_tab_menu(
        &mut self,
        action: MenuAction,
        target: Option<&orchbee_workbench_protocol::WorkbenchTabId>,
    ) -> SessionActionDisposition {
        use orchbee_workbench_protocol::TabCreationPlacement;
        if matches!(action, MenuAction::NewGroupWithTab) {
            return self.create_tab_placed(Some(TabCreationPlacement::NewGroup {}));
        }
        if matches!(action, MenuAction::NewTabInGroup | MenuAction::DuplicateTab) {
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
                if action == MenuAction::DuplicateTab {
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
            self.command_notice = Some((
                super::NativeSessionNotice::CreateNotCreated,
                self.clock.now() + std::time::Duration::from_secs(3),
            ));
            return SessionActionDisposition::KeepNavigator;
        }
        if self.configure_tab_menu(action, target).is_none() {
            self.command_notice = Some((
                super::NativeSessionNotice::LocalBusy,
                self.clock.now() + std::time::Duration::from_secs(3),
            ));
        }
        SessionActionDisposition::KeepNavigator
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
                SessionActionDisposition::DismissNavigator => Some(()),
                _ => None,
            };
        }
        if let Some(ids) = close_tab_ids(state, &tab_id, action) {
            return self.request_close_set(&ids);
        }
        let selection_change = match action {
            SelectTab | DeselectTab => Some(super::super::state::TabSelectionChange::Toggle),
            SelectTabRange => Some(super::super::state::TabSelectionChange::Range),
            SelectAllTabs => Some(super::super::state::TabSelectionChange::All),
            ClearTabSelection => Some(super::super::state::TabSelectionChange::Clear),
            _ => None,
        };
        if let Some(change) = selection_change {
            return self.state.as_mut()?.mark_tabs(&tab_id, change);
        }
        if matches!(action, ManageGroups | ChooseGroupColor) {
            return self
                .open_action_menu(
                    MenuTarget::TabGroups {
                        tab_id,
                        colors: action == ChooseGroupColor,
                    },
                    None,
                    false,
                )
                .then_some(());
        }
        if matches!(action, CollapseGroup | ExpandGroup) {
            let id = tab.group()?.group_id.clone();
            self.toggle_tab_group(&id);
            return Some(());
        }
        let selected = WorkbenchTabIdSet::try_from(configuration_tab_ids(state, &tab_id)).ok()?;
        let operation = match action {
            PinTab | UnpinTab => TabConfiguration::SetTabsPinned {
                tab_ids: selected,
                pinned: action == PinTab,
            },
            CreateGroup => TabConfiguration::CreateGroupForTabs {
                tab_ids: selected,
                label: WorkbenchTabGroupLabel::parse(&tab.display_label(index)).ok()?,
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
            MoveGroupPrevious | MoveGroupNext => TabConfiguration::MoveGroup {
                group_id: tab.group()?.group_id.clone(),
                direction: if action == MoveGroupPrevious {
                    TabGroupMoveDirection::Previous
                } else {
                    TabGroupMoveDirection::Next
                },
            },
            MoveTabPrevious | MoveTabNext | MoveTabFirst | MoveTabLast => {
                let adjacent = if action == MoveTabPrevious {
                    index.checked_sub(1)
                } else {
                    index.checked_add(1)
                };
                TabConfiguration::MoveTab {
                    tab_id,
                    direction: match action {
                        MoveTabPrevious => WorkbenchTabMoveDirection::Previous,
                        MoveTabNext => WorkbenchTabMoveDirection::Next,
                        MoveTabFirst => WorkbenchTabMoveDirection::First,
                        _ => WorkbenchTabMoveDirection::Last,
                    },
                    adjacent_group_collapsed: adjacent
                        .and_then(|index| state.cache().tabs().get(index))
                        .and_then(NativeSimpleTab::group)
                        .is_some_and(|group| state.group_collapsed(&group.group_id)),
                }
            }
            _ => return None,
        };
        if matches!(action, CreateGroup | RenameGroup) {
            self.request_setup_draft(NativeSetupIntent::TabGroupName(Box::new(operation)))?;
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
