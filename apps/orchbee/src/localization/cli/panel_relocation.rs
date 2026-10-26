use super::Locale;
#[cfg(windows)]
use serde_json::Value;
impl Locale {
    #[cfg(windows)]
    pub(crate) fn cli_panel_relocation(self, document: &Value) -> String {
        let outcome = match (self, document["outcome"].as_str()) {
            (Self::EnUs, Some("completed")) => "Panel moved",
            (Self::ZhCn, Some("completed")) => "面板已迁移",
            (Self::EnUs, Some("unchanged")) => "Panel unchanged",
            (Self::ZhCn, Some("unchanged")) => "面板未改变",
            (Self::EnUs, Some("outcome-unknown")) => "Panel move outcome unknown",
            (Self::ZhCn, Some("outcome-unknown")) => "面板迁移结果不确定",
            (Self::EnUs, _) => "Panel move incomplete",
            (Self::ZhCn, _) => "面板迁移未完整完成",
        };
        let mut text = format!("{outcome}\n");
        if document["reason"].as_str().is_some() {
            text.push_str(match self {
                Self::EnUs => "The source or target Tab is zoomed.\n",
                Self::ZhCn => "来源或目标标签页处于缩放状态。\n",
            });
        }
        for phase in ["movement"] {
            let result = &document[phase]["result"];
            for (key, en, zh) in [
                ("projectId", "Project", "项目"),
                ("panelId", "Panel", "面板"),
                ("targetTabId", "Target Tab", "目标标签页"),
                ("revision", "Committed revision", "已提交修订"),
            ] {
                if let Some(value) = result[key].as_str() {
                    let label = match self {
                        Self::EnUs => en,
                        Self::ZhCn => zh,
                    };
                    text.push_str(&format!("{label}: {}\n", crate::cli::escape::escape(value)));
                }
            }
        }
        for key in ["ratio", "focus"] {
            if !document[key].is_null() {
                text.push_str(&self.cli_client_control(&document[key]));
            }
        }
        let transfer = &document["movement"]["result"]["transfer"];
        for (key, en, zh) in [
            ("createdWorkspaceId", "Created Workspace", "已创建工作区"),
            ("createdDirectoryId", "Created directory", "已创建目录"),
            (
                "closedSourceWorkspaceId",
                "Removed empty source Workspace",
                "已移除空来源工作区",
            ),
        ] {
            if let Some(value) = transfer[key].as_str() {
                let label = match self {
                    Self::EnUs => en,
                    Self::ZhCn => zh,
                };
                text.push_str(&format!("{label}: {}\n", crate::cli::escape::escape(value)));
            }
        }
        if transfer["closedSourceTab"] == true {
            text.push_str(match self {
                Self::EnUs => "Removed the empty source Tab.\n",
                Self::ZhCn => "已移除空来源标签页。\n",
            });
        }
        if let Some(path) = document["directoryPath"].as_str() {
            let label = match self {
                Self::EnUs => "Workspace directory",
                Self::ZhCn => "工作区目录",
            };
            text.push_str(&format!("{label}: {}\n", crate::cli::escape::escape(path)));
        }
        text
    }
}
