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
        tab: &str,
        pane: &str,
    ) -> String {
        let location = workspace.or(project);
        match (self, location) {
            (Self::EnUs, Some(location)) => {
                format!("Workspace {location} · Tab {tab} · Panel {pane}")
            }
            (Self::EnUs, None) => format!("Tab {tab} · Panel {pane}"),
            (Self::ZhCn, Some(location)) => {
                format!("工作区 {location} · 标签页 {tab} · 面板 {pane}")
            }
            (Self::ZhCn, None) => format!("标签页 {tab} · 面板 {pane}"),
        }
    }
}
