//! Formats Panel relocation outcomes without selecting or moving a live Panel.
use super::Locale;
use crate::cli::handles;
use serde_json::Value;

/// Names a resource of a kind people address by handle.
type Handle = fn(&str) -> String;

/// A resource by the handle a person types next, other values as they are.
fn shown(value: &str, handle: Option<Handle>) -> String {
    handle.map_or_else(|| value.to_owned(), |handle| handle(value))
}
impl super::CliText {
    pub(crate) fn cli_panel_relocation(self, document: &Value) -> String {
        let document = &document["result"];
        let outcome = match (self.locale, document["outcome"].as_str()) {
            (Locale::EnUs, Some("completed")) => "Panel moved",
            (Locale::ZhCn, Some("completed")) => "面板已迁移",
            (Locale::EnUs, Some("unchanged")) => "Panel unchanged",
            (Locale::ZhCn, Some("unchanged")) => "面板未改变",
            (Locale::EnUs, Some("outcome-unknown")) => "Panel move outcome unknown",
            (Locale::ZhCn, Some("outcome-unknown")) => "面板迁移结果不确定",
            (Locale::EnUs, _) => "Panel move incomplete",
            (Locale::ZhCn, _) => "面板迁移未完整完成",
        };
        let mut text = format!("{outcome}\n");
        if document["reason"].as_str().is_some() {
            text.push_str(match self.locale {
                Locale::EnUs => "The source or target Tab is maximized.\n",
                Locale::ZhCn => "来源或目标标签页处于最大化状态。\n",
            });
        }
        for phase in ["movement"] {
            let result = &document[phase]["result"];
            let identities: [(&str, &str, &str, Option<Handle>); 4] = [
                ("projectId", "Project", "项目", Some(handles::project)),
                ("panelId", "Panel", "面板", Some(handles::panel)),
                (
                    "targetTabId",
                    "Target Tab",
                    "目标标签页",
                    Some(handles::tab),
                ),
                ("revision", "Committed revision", "已提交修订", None),
            ];
            for (key, en, zh, handle) in identities {
                if let Some(value) = result[key].as_str() {
                    let label = match self.locale {
                        Locale::EnUs => en,
                        Locale::ZhCn => zh,
                    };
                    text.push_str(&format!(
                        "{label}: {}\n",
                        crate::cli_text::escape::escape(&shown(value, handle))
                    ));
                }
            }
        }
        for key in ["ratio", "focus"] {
            if !document[key].is_null() {
                text.push_str(&self.cli_client_control(&document[key]));
            }
        }
        let transfer = &document["movement"]["result"]["transfer"];
        let transfers: [(&str, &str, &str, Option<Handle>); 3] = [
            (
                "createdWorkspaceId",
                "Created Workspace",
                "已创建工作区",
                Some(handles::workspace),
            ),
            (
                "createdDirectoryId",
                "Created directory",
                "已创建目录",
                None,
            ),
            (
                "closedSourceWorkspaceId",
                "Removed empty source Workspace",
                "已移除空来源工作区",
                Some(handles::workspace),
            ),
        ];
        for (key, en, zh, handle) in transfers {
            if let Some(value) = transfer[key].as_str() {
                let label = match self.locale {
                    Locale::EnUs => en,
                    Locale::ZhCn => zh,
                };
                text.push_str(&format!(
                    "{label}: {}\n",
                    crate::cli_text::escape::escape(&shown(value, handle))
                ));
            }
        }
        if transfer["closedSourceTab"] == true {
            text.push_str(match self.locale {
                Locale::EnUs => "Removed the empty source Tab.\n",
                Locale::ZhCn => "已移除空来源标签页。\n",
            });
        }
        if let Some(path) = document["directoryPath"].as_str() {
            let label = match self.locale {
                Locale::EnUs => "Workspace directory",
                Locale::ZhCn => "工作区目录",
            };
            text.push_str(&format!(
                "{label}: {}\n",
                crate::cli_text::escape::escape(path)
            ));
        }
        text
    }
}
