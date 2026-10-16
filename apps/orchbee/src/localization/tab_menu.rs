use super::Locale;
use crate::action_menu::{MenuAction, MenuTarget, TabMenuScope};

impl Locale {
    /// Scope is captured owner data, not a count recovered from translated row copy.
    pub(crate) fn menu_tab_action(self, action: &MenuAction, scope: TabMenuScope) -> String {
        use MenuAction::*;
        use TabMenuScope as S;
        let selected = match scope {
            S::Selected(count) => Some(count.get()),
            _ => None,
        };
        match (self, action, selected) {
            (Self::EnUs, DuplicateTab, Some(n)) => format!("Duplicate selected {n} Tabs"),
            (Self::ZhCn, DuplicateTab, Some(n)) => format!("复制选中的 {n} 个 Tab"),
            (Self::EnUs, PinTab, Some(n)) => format!("Pin selected {n} Tabs"),
            (Self::ZhCn, PinTab, Some(n)) => format!("固定选中的 {n} 个 Tab"),
            (Self::EnUs, UnpinTab, Some(n)) => format!("Unpin selected {n} Tabs"),
            (Self::ZhCn, UnpinTab, Some(n)) => format!("取消固定选中的 {n} 个 Tab"),
            (Self::EnUs, CloseSelectedTabs, Some(n)) => format!("Close selected {n} Tabs…"),
            (Self::ZhCn, CloseSelectedTabs, Some(n)) => format!("关闭选中的 {n} 个 Tab…"),
            (Self::EnUs, ManageGroups, Some(n)) => format!("Add selected {n} Tabs to group"),
            (Self::ZhCn, ManageGroups, Some(n)) => format!("将选中的 {n} 个 Tab 加入分组"),
            (Self::EnUs, CreateGroup, Some(n)) => format!("New group for selected {n} Tabs…"),
            (Self::ZhCn, CreateGroup, Some(n)) => format!("为选中的 {n} 个 Tab 新建分组…"),
            (Self::EnUs, JoinGroup(_), Some(n)) => format!("Move selected {n} Tabs to"),
            (Self::ZhCn, JoinGroup(_), Some(n)) => format!("将选中的 {n} 个 Tab 移入"),
            (Self::EnUs, RemoveFromGroup, Some(n)) => {
                format!("Remove selected {n} Tabs from groups")
            }
            (Self::ZhCn, RemoveFromGroup, Some(n)) => format!("将选中的 {n} 个 Tab 移出分组"),
            _ => match (self, action, scope) {
                (Self::EnUs, CloseOtherTabs, S::Other(n)) if n.get() == 1 => {
                    "Close 1 other Tab…".into()
                }
                (Self::EnUs, CloseOtherTabs, S::Other(n)) => {
                    format!("Close {} other Tabs…", n.get())
                }
                (Self::ZhCn, CloseOtherTabs, S::Other(n)) => {
                    format!("关闭其他 {} 个 Tab…", n.get())
                }
                (Self::EnUs, CloseTrailingTabs, S::Trailing(n)) if n.get() == 1 => {
                    "Close 1 Tab to right…".into()
                }
                (Self::EnUs, CloseTrailingTabs, S::Trailing(n)) => {
                    format!("Close {} Tabs to right…", n.get())
                }
                (Self::ZhCn, CloseTrailingTabs, S::Trailing(n)) => {
                    format!("关闭右侧 {} 个 Tab…", n.get())
                }
                (Self::EnUs, CloseGroup, S::Group(n)) if n.get() == 1 => {
                    "Close group (1 Tab)…".into()
                }
                (Self::EnUs, CloseGroup, S::Group(n)) => format!("Close group ({} Tabs)…", n.get()),
                (Self::ZhCn, CloseGroup, S::Group(n)) => format!("关闭分组（{} 个 Tab）…", n.get()),
                (Self::EnUs, Ungroup, S::Group(n)) if n.get() == 1 => "Ungroup, keep 1 Tab".into(),
                (Self::EnUs, Ungroup, S::Group(n)) => format!("Ungroup, keep {} Tabs", n.get()),
                (Self::ZhCn, Ungroup, S::Group(n)) => format!("解散分组，保留 {} 个 Tab", n.get()),
                _ => self.menu_action(action).to_owned(),
            },
        }
    }

    /// Purposeful scope/path content; generic menu decoration is not part of this heading.
    pub(crate) fn menu_tab_path(
        self,
        target: &MenuTarget,
        scope: TabMenuScope,
        disclosure: &str,
    ) -> String {
        let group = match (self, scope) {
            (Self::EnUs, TabMenuScope::Group(n)) if n.get() == 1 => "Group (1 Tab)".into(),
            (Self::EnUs, TabMenuScope::Group(n)) => format!("Group ({} Tabs)", n.get()),
            (Self::ZhCn, TabMenuScope::Group(n)) => format!("分组（{} 个 Tab）", n.get()),
            (Self::EnUs, _) => "Group".into(),
            (Self::ZhCn, _) => "分组".into(),
        };
        match (self, target) {
            (Self::EnUs, MenuTarget::TabGroups { colors: true, .. }) => {
                format!("{group} {disclosure} Color")
            }
            (Self::ZhCn, MenuTarget::TabGroups { colors: true, .. }) => {
                format!("{group} {disclosure} 颜色")
            }
            (_, MenuTarget::TabGroups { colors: false, .. }) => group,
            (Self::EnUs, MenuTarget::TabGroupChoices(_)) => {
                format!("Tab {disclosure} Add to group")
            }
            (Self::ZhCn, MenuTarget::TabGroupChoices(_)) => format!("Tab {disclosure} 加入分组"),
            (Self::EnUs, MenuTarget::TabTools(_)) => format!("Tab {disclosure} Select / move"),
            (Self::ZhCn, MenuTarget::TabTools(_)) => format!("Tab {disclosure} 选择与移动"),
            _ => match (self, scope) {
                (Self::EnUs, TabMenuScope::Selected(n)) => format!("{} selected Tabs", n.get()),
                (Self::ZhCn, TabMenuScope::Selected(n)) => format!("选中的 {} 个 Tab", n.get()),
                _ => "Tab".into(),
            },
        }
    }
}
