use super::Locale;
use super::MenuLabel;

/// The scope behind a displayed row, independent of the current terminal or menu candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TabMenuScope {
    Addressed,
    Selected(std::num::NonZeroUsize),
    Group(std::num::NonZeroUsize),
    Other(std::num::NonZeroUsize),
    Trailing(std::num::NonZeroUsize),
    Workspace,
}

impl Locale {
    /// Scope is captured owner data, not a count recovered from translated row copy.
    pub(crate) fn menu_tab_action(
        self,
        action: impl Into<MenuLabel>,
        scope: TabMenuScope,
    ) -> String {
        use MenuLabel::*;
        let action = action.into();
        use TabMenuScope as S;
        let selected = match scope {
            S::Selected(count) => Some(count.get()),
            _ => None,
        };
        match (self, action, selected) {
            (Self::EnUs, DuplicateTab, Some(n)) => format!("Duplicate selected {n} Tabs"),
            (Self::ZhCn, DuplicateTab, Some(n)) => format!("复制所选 {n} 个标签页"),
            (Self::EnUs, PinTab, Some(n)) => format!("Pin selected {n} Tabs"),
            (Self::ZhCn, PinTab, Some(n)) => format!("固定所选 {n} 个标签页"),
            (Self::EnUs, UnpinTab, Some(n)) => format!("Unpin selected {n} Tabs"),
            (Self::ZhCn, UnpinTab, Some(n)) => format!("取消固定所选 {n} 个标签页"),
            (Self::EnUs, CloseSelectedTabs, Some(n)) => format!("Close selected {n} Tabs…"),
            (Self::ZhCn, CloseSelectedTabs, Some(n)) => format!("关闭所选 {n} 个标签页…"),
            (Self::EnUs, ManageGroups, Some(n)) => format!("Add selected {n} Tabs to group"),
            (Self::ZhCn, ManageGroups, Some(n)) => format!("将所选 {n} 个标签页加入分组"),
            (Self::EnUs, RemoveFromGroup, Some(n)) => {
                format!("Remove selected {n} Tabs from groups")
            }
            (Self::ZhCn, RemoveFromGroup, Some(n)) => format!("将所选 {n} 个标签页移出分组"),
            _ => match (self, action, scope) {
                (Self::EnUs, CloseOtherTabs, S::Other(n)) if n.get() == 1 => {
                    "Close 1 other Tab…".into()
                }
                (Self::EnUs, CloseOtherTabs, S::Other(n)) => {
                    format!("Close {} other Tabs…", n.get())
                }
                (Self::ZhCn, CloseOtherTabs, S::Other(n)) => {
                    format!("关闭其他 {} 个标签页…", n.get())
                }
                (Self::EnUs, CloseTrailingTabs, S::Trailing(n)) if n.get() == 1 => {
                    "Close 1 Tab to right…".into()
                }
                (Self::EnUs, CloseTrailingTabs, S::Trailing(n)) => {
                    format!("Close {} Tabs to right…", n.get())
                }
                (Self::ZhCn, CloseTrailingTabs, S::Trailing(n)) => {
                    format!("关闭右侧 {} 个标签页…", n.get())
                }
                (Self::EnUs, CloseGroup, S::Group(n)) if n.get() == 1 => {
                    "Close group (1 Tab)…".into()
                }
                (Self::EnUs, CloseGroup, S::Group(n)) => format!("Close group ({} Tabs)…", n.get()),
                (Self::ZhCn, CloseGroup, S::Group(n)) => {
                    format!("关闭分组（{} 个标签页）…", n.get())
                }
                (Self::EnUs, Ungroup, S::Group(n)) if n.get() == 1 => "Ungroup, keep 1 Tab".into(),
                (Self::EnUs, Ungroup, S::Group(n)) => format!("Ungroup, keep {} Tabs", n.get()),
                (Self::ZhCn, Ungroup, S::Group(n)) => {
                    format!("解散分组，保留 {} 个标签页", n.get())
                }
                _ => self.menu_action(action).to_owned(),
            },
        }
    }

    /// A group menu's title follows the group's name with its Tab count.
    pub(crate) fn menu_group_path(self, scope: TabMenuScope) -> String {
        match (self, scope) {
            (Self::EnUs, TabMenuScope::Group(n)) if n.get() == 1 => "Group (1 Tab)".into(),
            (Self::EnUs, TabMenuScope::Group(n)) => format!("Group ({} Tabs)", n.get()),
            (Self::ZhCn, TabMenuScope::Group(n)) => format!("分组（{} 个标签页）", n.get()),
            (Self::EnUs, _) => "Group".into(),
            (Self::ZhCn, _) => "分组".into(),
        }
    }
}
