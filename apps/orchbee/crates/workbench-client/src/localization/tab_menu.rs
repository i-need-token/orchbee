use super::Locale;

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
    /// The Mobile bar's button that opens the Tab list, before the Tab count.
    pub(crate) const fn tab_list_label(self) -> &'static str {
        match self {
            Self::EnUs => "Tabs",
            Self::ZhCn => "标签页",
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
