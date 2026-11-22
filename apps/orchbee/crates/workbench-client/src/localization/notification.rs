//! Shared native-surface messages, without CLI grammar or result dependencies.
use super::Locale;
impl Locale {
    pub(crate) fn agent_notification_title(self, agent: &str, request: bool) -> String {
        match (self, request) {
            (Self::EnUs, false) => format!("{agent} finished"),
            (Self::EnUs, true) => format!("{agent} needs attention"),
            (Self::ZhCn, false) => format!("{agent} 已完成"),
            (Self::ZhCn, true) => format!("{agent} 需要处理"),
        }
    }

    pub(crate) fn agent_notification_context(
        self,
        workspace: Option<&str>,
        project: Option<&str>,
        tab: Option<&str>,
        pane: &str,
    ) -> String {
        let location = workspace.or(project);
        let (workspace_word, tab_word, pane_word) = match self {
            Self::EnUs => ("Workspace", "Tab", "Panel"),
            Self::ZhCn => ("工作区", "标签页", "面板"),
        };
        // A Workspace's only Tab whose label was never set goes unnamed, as the sidebar leaves it.
        location
            .map(|location| format!("{workspace_word} {location}"))
            .into_iter()
            .chain(tab.map(|tab| format!("{tab_word} {tab}")))
            .chain(std::iter::once(format!("{pane_word} {pane}")))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}
