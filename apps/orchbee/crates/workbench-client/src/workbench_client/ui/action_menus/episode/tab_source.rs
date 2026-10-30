//! Exact ordered inputs omitted by the selected-member/focus origin proof.
//!
//! Relative Close, group membership and configuration resolve against these canonical facts.
//! A peer change retires the captured menu before input or handoff; a later CloseSet decision
//! remains independently owned by its existing complete-set confirmation.

use crate::workbench_client::workbench_projection::ClientTab;
use orchbee_workbench_protocol::{WorkbenchTabGroupId, WorkbenchTabId};

#[derive(Clone)]
pub(in crate::workbench_client::ui::action_menus) struct TabResolverSource(Vec<Member>);

#[derive(Clone)]
struct Member {
    tab: WorkbenchTabId,
    pinned: bool,
    group: Option<WorkbenchTabGroupId>,
}

impl TabResolverSource {
    pub(in crate::workbench_client::ui::action_menus) fn capture(tabs: &[ClientTab]) -> Self {
        Self(
            tabs.iter()
                .map(|tab| Member {
                    tab: tab.tab_id().clone(),
                    pinned: tab.pinned(),
                    group: tab.group().map(|group| group.group_id.clone()),
                })
                .collect(),
        )
    }

    pub(in crate::workbench_client::ui::action_menus) fn matches(
        &self,
        tabs: &[ClientTab],
    ) -> bool {
        self.0.len() == tabs.len()
            && self.0.iter().zip(tabs).all(|(captured, current)| {
                &captured.tab == current.tab_id()
                    && captured.pinned == current.pinned()
                    && captured.group.as_ref() == current.group().map(|group| &group.group_id)
            })
    }
}
