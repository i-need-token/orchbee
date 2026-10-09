//! Menu adapters for canonical Tab configuration and identity-keyed, Client-local group disclosure.
//! The existing setup lane owns admission, unknown outcomes, publication recovery and selection.
use super::{NativeSessionState, NativeUi, OperationDeadlines, SessionActionDisposition};
use crate::action_menu::{MenuAction, MenuTarget};
use crate::terminal_session::state::workspace_setup::NativeSetupIntent;
use crate::terminal_session::workbench_projection::NativeSimpleTab;
use orchbee_workbench_protocol::{
    TabConfiguration, TabGroupColor, TabGroupMoveDirection, WorkbenchLabel, WorkbenchTabGroupId,
};
#[cfg(test)]
mod tests;

pub(super) fn menu_actions(
    state: &NativeSessionState,
    tab: &NativeSimpleTab,
    submenu: Option<bool>,
) -> Vec<MenuAction> {
    use MenuAction::*;
    match submenu {
        None => crate::action_menu::TAB_ACTIONS
            .iter()
            .map(|action| {
                if *action == PinTab && tab.pinned() {
                    UnpinTab
                } else {
                    *action
                }
            })
            .collect(),
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
                    RenameGroup,
                    ChooseGroupColor,
                    if state.group_collapsed(&group.group_id) {
                        ExpandGroup
                    } else {
                        CollapseGroup
                    },
                    RemoveFromGroup,
                    Ungroup,
                    MoveGroupPrevious,
                    MoveGroupNext,
                ]);
            }
            actions.extend(
                state
                    .cache()
                    .groups()
                    .iter()
                    .enumerate()
                    .filter(|(_, group)| tab.group() != Some(group))
                    .filter_map(|(index, _)| u16::try_from(index).ok().map(JoinGroup)),
            );
            actions
        }
    }
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
        let operation = match action {
            PinTab | UnpinTab => TabConfiguration::SetPinned {
                tab_id,
                pinned: action == PinTab,
            },
            CreateGroup => TabConfiguration::CreateGroup {
                tab_id,
                label: WorkbenchLabel::parse(&tab.display_label(index)).ok()?,
                color: TabGroupColor::Blue,
            },
            JoinGroup(index) => TabConfiguration::SetGroup {
                tab_id,
                group_id: Some(
                    state
                        .cache()
                        .groups()
                        .get(usize::from(index))?
                        .group_id
                        .clone(),
                ),
            },
            RemoveFromGroup => TabConfiguration::SetGroup {
                tab_id,
                group_id: None,
            },
            Ungroup => TabConfiguration::Ungroup {
                group_id: tab.group()?.group_id.clone(),
            },
            RenameGroup | GroupColor(_) => {
                let group = tab.group()?;
                TabConfiguration::UpdateGroup {
                    group_id: group.group_id.clone(),
                    label: WorkbenchLabel::parse(&group.label).ok()?,
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
