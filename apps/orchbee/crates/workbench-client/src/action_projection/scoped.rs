//! Titles that follow the object or the count a menu row acts on.

use crate::client_actions::MenuAction;
use crate::localization::{Locale, TabMenuScope};

impl MenuAction {
    /// Scope is captured owner data, not a count recovered from translated row copy.
    pub(crate) fn scoped_title(&self, locale: Locale, scope: TabMenuScope) -> String {
        use MenuAction::*;
        use TabMenuScope as S;
        let selected = match scope {
            S::Selected(count) => Some(count.get()),
            _ => None,
        };
        match (locale, self, selected) {
            (Locale::EnUs, DuplicateTab, Some(n)) => format!("Duplicate selected {n} Tabs"),
            (Locale::ZhCn, DuplicateTab, Some(n)) => format!("复制所选 {n} 个标签页"),
            (Locale::EnUs, PinTab, Some(n)) => format!("Pin selected {n} Tabs"),
            (Locale::ZhCn, PinTab, Some(n)) => format!("固定所选 {n} 个标签页"),
            (Locale::EnUs, UnpinTab, Some(n)) => format!("Unpin selected {n} Tabs"),
            (Locale::ZhCn, UnpinTab, Some(n)) => format!("取消固定所选 {n} 个标签页"),
            (Locale::EnUs, CloseSelectedTabs, Some(n)) => format!("Close selected {n} Tabs…"),
            (Locale::ZhCn, CloseSelectedTabs, Some(n)) => format!("关闭所选 {n} 个标签页…"),
            (Locale::EnUs, ManageGroups, Some(n)) => format!("Add selected {n} Tabs to group"),
            (Locale::ZhCn, ManageGroups, Some(n)) => format!("将所选 {n} 个标签页加入分组"),
            (Locale::EnUs, RemoveFromGroup, Some(n)) => {
                format!("Remove selected {n} Tabs from groups")
            }
            (Locale::ZhCn, RemoveFromGroup, Some(n)) => format!("将所选 {n} 个标签页移出分组"),
            _ => match (locale, self, scope) {
                (Locale::EnUs, CloseOtherTabs, S::Other(n)) if n.get() == 1 => {
                    "Close 1 other Tab…".into()
                }
                (Locale::EnUs, CloseOtherTabs, S::Other(n)) => {
                    format!("Close {} other Tabs…", n.get())
                }
                (Locale::ZhCn, CloseOtherTabs, S::Other(n)) => {
                    format!("关闭其他 {} 个标签页…", n.get())
                }
                (Locale::EnUs, CloseTrailingTabs, S::Trailing(n)) if n.get() == 1 => {
                    "Close 1 Tab to right…".into()
                }
                (Locale::EnUs, CloseTrailingTabs, S::Trailing(n)) => {
                    format!("Close {} Tabs to right…", n.get())
                }
                (Locale::ZhCn, CloseTrailingTabs, S::Trailing(n)) => {
                    format!("关闭右侧 {} 个标签页…", n.get())
                }
                (Locale::EnUs, CloseGroup, S::Group(n)) if n.get() == 1 => {
                    "Close group (1 Tab)…".into()
                }
                (Locale::EnUs, CloseGroup, S::Group(n)) => {
                    format!("Close group ({} Tabs)…", n.get())
                }
                (Locale::ZhCn, CloseGroup, S::Group(n)) => {
                    format!("关闭分组（{} 个标签页）…", n.get())
                }
                (Locale::EnUs, Ungroup, S::Group(n)) if n.get() == 1 => {
                    "Ungroup, keep 1 Tab".into()
                }
                (Locale::EnUs, Ungroup, S::Group(n)) => format!("Ungroup, keep {} Tabs", n.get()),
                (Locale::ZhCn, Ungroup, S::Group(n)) => {
                    format!("取消分组，保留 {} 个标签页", n.get())
                }
                _ => self.title(locale).to_owned(),
            },
        }
    }

    /// A context menu's frame title already names the object it opened on, so a row acting on that
    /// object alone keeps only its verb. Rows naming another object or a counted set keep their
    /// full labels, as do the global menu, dialog titles and shortcut help.
    pub(crate) fn object_title(&self, locale: Locale, scope: Option<TabMenuScope>) -> String {
        use MenuAction::*;
        let own_object = matches!(
            scope,
            None | Some(TabMenuScope::Addressed | TabMenuScope::Group(_))
        );
        let verb = match (locale, self) {
            _ if !own_object => None,
            (
                Locale::EnUs,
                RenameTab | RenamePanel | RenameWorkspace | RenameProject | RenameGroup,
            ) => Some("Rename"),
            (
                Locale::ZhCn,
                RenameTab | RenamePanel | RenameWorkspace | RenameProject | RenameGroup,
            ) => Some("重命名"),
            (
                Locale::EnUs,
                CloseSelectedTabs | ClosePane | CloseGroup | CloseWorkspace | DeleteWorkspace,
            ) => Some("Close"),
            (
                Locale::ZhCn,
                CloseSelectedTabs | ClosePane | CloseGroup | CloseWorkspace | DeleteWorkspace,
            ) => Some("关闭"),
            (Locale::EnUs, DeleteProject) => Some("Ungroup"),
            (Locale::ZhCn, DeleteProject) => Some("取消分组"),
            (Locale::EnUs, PinTab) => Some("Pin"),
            (Locale::ZhCn, PinTab) => Some("固定"),
            (Locale::EnUs, UnpinTab) => Some("Unpin"),
            (Locale::ZhCn, UnpinTab) => Some("取消固定"),
            (Locale::EnUs, ManageGroups) => Some("Add to group"),
            (Locale::ZhCn, ManageGroups) => Some("加入分组"),
            (Locale::EnUs, RemoveFromGroup) => Some("Remove from group"),
            (Locale::ZhCn, RemoveFromGroup) => Some("移出分组"),
            (Locale::EnUs, Zoom) => Some("Maximize / Restore"),
            (Locale::ZhCn, Zoom) => Some("最大化 / 还原"),
            _ => None,
        };
        match (verb, scope) {
            (Some(verb), _) => verb.to_owned(),
            (None, Some(scope)) => self.scoped_title(locale, scope),
            (None, None) => self.title(locale).to_owned(),
        }
    }

    /// The title with the Agent product its `{agent}` names.
    pub(crate) fn title_naming(&self, locale: Locale, agent: &str) -> String {
        self.title(locale).replace(
            "{agent}",
            &crate::terminal_text::sanitize_terminal_text(agent),
        )
    }
}
