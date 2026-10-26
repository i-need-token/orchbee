//! The human projection shared by every Workbench mutation receipt: one
//! localized sentence, then aligned rows naming each created or affected
//! resource. Revisions, request IDs and other internal identities stay in
//! the JSON document.
use super::Locale;
use crate::cli::escape::{display_width, escape, quoted, unquoted};
use serde_json::Value;

/// Public receipt members in display order. Every other result member is an
/// internal identity or ordering detail that only `--format json` reports.
const FIELDS: [&str; 18] = [
    "projectId",
    "workspaceId",
    "workspaceIds",
    "beforeWorkspaceId",
    "tabId",
    "targetTabId",
    "newPanelId",
    "panelId",
    "targetPanelId",
    "profileId",
    "name",
    "label",
    "customLabel",
    "direction",
    "insertIndex",
    "rightClick",
    "path",
    "terminal",
];

impl super::CliText {
    /// Render one mutation `result` object.
    pub(crate) fn cli_receipt(self, result: &Value) -> String {
        self.cli_receipt_with(result, None)
    }

    /// Render one mutation `result` object, optionally replacing the first
    /// line with a fact the CLI observed beyond the receipt.
    pub(crate) fn cli_receipt_with(self, result: &Value, summary: Option<String>) -> String {
        let mut text = summary.unwrap_or_else(|| self.cli_receipt_summary(result));
        let mut rows: Vec<(&'static str, String)> = Vec::new();
        for key in FIELDS {
            let Some(value) = result.get(key) else {
                continue;
            };
            let text = match (key, value) {
                ("projectId", Value::Null) => self.cli_ungrouped().to_owned(),
                ("customLabel", Value::Null) => self.receipt_cleared().to_owned(),
                ("label" | "name" | "customLabel", Value::String(value)) => quoted(value),
                ("terminal", terminal) => self.receipt_terminal(terminal),
                ("workspaceIds", Value::Array(ids)) => ids
                    .iter()
                    .filter_map(Value::as_str)
                    .map(unquoted)
                    .collect::<Vec<_>>()
                    .join(" "),
                (_, Value::String(value)) => unquoted(value),
                (_, Value::Number(value)) => value.to_string(),
                _ => continue,
            };
            rows.push((self.receipt_field(key), text));
        }
        let transfer = &result["transfer"];
        for (key, label) in [
            (
                "createdWorkspaceId",
                self.receipt_field("createdWorkspaceId"),
            ),
            (
                "closedSourceWorkspaceId",
                self.receipt_field("closedSourceWorkspaceId"),
            ),
        ] {
            if let Some(id) = transfer[key].as_str() {
                rows.push((label, unquoted(id)));
            }
        }
        let width = rows
            .iter()
            .map(|(label, _)| display_width(label))
            .max()
            .unwrap_or(0);
        for (label, value) in rows {
            text.push_str(label);
            text.push_str(&" ".repeat(width - display_width(label) + 2));
            text.push_str(&value);
            text.push('\n');
        }
        let launched = matches!(
            result["terminal"]["outcome"].as_str(),
            Some("started" | "pending")
        );
        if launched
            && let Some(panel) = result["newPanelId"]
                .as_str()
                .or_else(|| result["panelId"].as_str())
        {
            text.push_str(&match self.locale {
                Locale::EnUs => format!("Next: orchbee terminal list --panel {}\n", escape(panel)),
                Locale::ZhCn => {
                    format!("下一步：orchbee terminal list --panel {}\n", escape(panel))
                }
            });
        }
        // The resource stays committed; point at the launch inputs instead of a retry.
        if result["terminal"]["outcome"] == "failed" {
            text.push_str(match self.locale {
                Locale::EnUs => {
                    "Next: the terminal did not start. Check the executable or launch profile and its working directory, then close this Tab or Panel or create another.\n"
                }
                Locale::ZhCn => {
                    "下一步：终端未启动。请检查可执行文件或启动配置及其工作目录，然后关闭此标签页或面板，或另行创建。\n"
                }
            });
        }
        text
    }

    /// A close the CLI waited for until the member disappeared.
    pub(crate) fn cli_close_finished(
        self,
        resource: &str,
        label: &str,
        terminals: usize,
    ) -> String {
        let label = quoted(label);
        match (self.locale, resource) {
            (Locale::EnUs, resource) => format!(
                "Closed {resource} {label} ({terminals} {}).\n",
                if terminals == 1 {
                    "terminal"
                } else {
                    "terminals"
                }
            ),
            (Locale::ZhCn, "workspace") => {
                format!("已关闭工作区 {label}（{terminals} 个终端）。\n")
            }
            (Locale::ZhCn, _) => format!("已关闭标签页 {label}（{terminals} 个终端）。\n"),
        }
    }

    /// What to run when a close outlived the command's wait.
    pub(crate) fn cli_close_wait_hint(self, workspace: bool, id: &str, timeout_ms: u32) -> String {
        let check = if workspace {
            format!(
                "orchbee workspace close-status --workspace {}",
                unquoted(id)
            )
        } else {
            "orchbee tab list".to_owned()
        };
        match self.locale {
            Locale::EnUs => format!(
                "The close was accepted but had not finished after {timeout_ms} ms; it continues in the background. Check with: {check}\n"
            ),
            Locale::ZhCn => {
                format!(
                    "关闭已被接纳，但 {timeout_ms} 毫秒内尚未完成；它会在后台继续。查看进度：{check}\n"
                )
            }
        }
    }

    /// The opening line of a close `--dry-run`.
    pub(crate) fn cli_close_preview(self, resource: &str) -> String {
        match (self.locale, resource) {
            (Locale::EnUs, resource) => format!(
                "Dry run: closing this {resource} with --confirm would end these terminals and the processes and Agents running in them.\n"
            ),
            (Locale::ZhCn, "workspace") => {
                "试运行：使用 --confirm 关闭该工作区将结束以下终端及其中运行的进程和智能体。\n"
                    .to_owned()
            }
            (Locale::ZhCn, _) => {
                "试运行：使用 --confirm 关闭该标签页将结束以下终端及其中运行的进程和智能体。\n"
                    .to_owned()
            }
        }
    }

    /// The first receipt line: what happened, to which kind of resource.
    fn cli_receipt_summary(self, result: &Value) -> String {
        let action = result["action"].as_str().unwrap_or_default();
        let outcome = result["outcome"].as_str().unwrap_or_default();
        let (resource, verb) = action.split_once('.').unwrap_or((action, ""));
        let (en, zh) = match resource {
            "project" => ("project", "项目"),
            "workspace" => ("workspace", "工作区"),
            "tab" => ("tab", "标签页"),
            "panel" => ("panel", "面板"),
            "launch-profile" => ("launch profile", "启动配置"),
            _ => ("resource", "资源"),
        };
        let label = result["label"]
            .as_str()
            .or_else(|| result["name"].as_str())
            .map(quoted);
        let named_en = label
            .as_ref()
            .map_or_else(|| en.to_owned(), |label| format!("{en} {label}"));
        let named_zh = label
            .as_ref()
            .map_or_else(|| zh.to_owned(), |label| format!("{zh} {label}"));
        match (self.locale, outcome, verb) {
            (Locale::EnUs, "unchanged", _) => {
                format!("No change: the {en} already matched the request.\n")
            }
            (Locale::ZhCn, "unchanged", _) => format!("未改变：该{zh}已符合请求。\n"),
            (Locale::EnUs, "accepted", _) => {
                format!("Closing {named_en}; the session finishes the close in the background.\n")
            }
            (Locale::ZhCn, "accepted", _) => {
                format!("正在关闭{named_zh}；会话会在后台完成关闭。\n")
            }
            (Locale::EnUs, "resumed", _) => format!("Resumed closing the {en}.\n"),
            (Locale::ZhCn, "resumed", _) => format!("已继续关闭该{zh}。\n"),
            (Locale::EnUs, _, "create") => format!("Created {named_en}.\n"),
            (Locale::ZhCn, _, "create") => format!("已创建{named_zh}。\n"),
            (Locale::EnUs, _, "split") => "Split the panel.\n".to_owned(),
            (Locale::ZhCn, _, "split") => "已分裂面板。\n".to_owned(),
            (Locale::EnUs, _, "rename") => match &label {
                Some(label) => format!("Renamed the {en} to {label}.\n"),
                None => format!("Renamed the {en}.\n"),
            },
            (Locale::ZhCn, _, "rename") => match &label {
                Some(label) => format!("已将该{zh}重命名为 {label}。\n"),
                None => format!("已重命名该{zh}。\n"),
            },
            (Locale::EnUs, _, "move" | "move-block" | "relocate") => format!("Moved the {en}.\n"),
            (Locale::ZhCn, _, "move" | "move-block" | "relocate") => format!("已移动该{zh}。\n"),
            (Locale::EnUs, _, "delete") => format!("Deleted {named_en}.\n"),
            (Locale::ZhCn, _, "delete") => format!("已删除{named_zh}。\n"),
            (Locale::EnUs, _, "dissolve") => format!("Dissolved the {en}.\n"),
            (Locale::ZhCn, _, "dissolve") => format!("已解散该{zh}。\n"),
            (Locale::EnUs, ..) => format!("Updated the {en}.\n"),
            (Locale::ZhCn, ..) => format!("已更新该{zh}。\n"),
        }
    }

    fn receipt_field(self, key: &str) -> &'static str {
        match (self.locale, key) {
            (Locale::EnUs, "projectId") => "Project",
            (Locale::ZhCn, "projectId") => "项目",
            (Locale::EnUs, "workspaceId") => "Workspace",
            (Locale::ZhCn, "workspaceId") => "工作区",
            (Locale::EnUs, "workspaceIds") => "Workspaces",
            (Locale::ZhCn, "workspaceIds") => "工作区列表",
            (Locale::EnUs, "beforeWorkspaceId") => "Before workspace",
            (Locale::ZhCn, "beforeWorkspaceId") => "插入位置之后的工作区",
            (Locale::EnUs, "createdWorkspaceId") => "New workspace",
            (Locale::ZhCn, "createdWorkspaceId") => "新工作区",
            (Locale::EnUs, "closedSourceWorkspaceId") => "Closed workspace",
            (Locale::ZhCn, "closedSourceWorkspaceId") => "已关闭的工作区",
            (Locale::EnUs, "tabId") => "Tab",
            (Locale::ZhCn, "tabId") => "标签页",
            (Locale::EnUs, "targetTabId") => "Target tab",
            (Locale::ZhCn, "targetTabId") => "目标标签页",
            (Locale::EnUs, "newPanelId") => "New panel",
            (Locale::ZhCn, "newPanelId") => "新面板",
            (Locale::EnUs, "panelId") => "Panel",
            (Locale::ZhCn, "panelId") => "面板",
            (Locale::EnUs, "targetPanelId") => "Target panel",
            (Locale::ZhCn, "targetPanelId") => "目标面板",
            (Locale::EnUs, "profileId") => "Launch profile",
            (Locale::ZhCn, "profileId") => "启动配置",
            (Locale::EnUs, "name") => "Name",
            (Locale::ZhCn, "name") => "名称",
            (Locale::EnUs, "label") => "Label",
            (Locale::ZhCn, "label") => "标签",
            (Locale::EnUs, "customLabel") => "Custom label",
            (Locale::ZhCn, "customLabel") => "自定义标签",
            (Locale::EnUs, "direction") => "Direction",
            (Locale::ZhCn, "direction") => "方向",
            (Locale::EnUs, "insertIndex") => "Position",
            (Locale::ZhCn, "insertIndex") => "位置",
            (Locale::EnUs, "rightClick") => "Right click",
            (Locale::ZhCn, "rightClick") => "右键",
            (Locale::EnUs, "path") => "Directory",
            (Locale::ZhCn, "path") => "目录",
            (Locale::EnUs, _) => "Terminal",
            (Locale::ZhCn, _) => "终端",
        }
    }

    const fn receipt_cleared(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "cleared",
            Locale::ZhCn => "已清除",
        }
    }

    /// The spawn outcome of a created terminal, with its grid when started.
    fn receipt_terminal(self, terminal: &Value) -> String {
        let outcome = terminal["outcome"].as_str().unwrap_or_default();
        let state = match (self.locale, outcome) {
            (Locale::EnUs, "started") => "started",
            (Locale::ZhCn, "started") => "已启动",
            (Locale::EnUs, "pending") => "starting",
            (Locale::ZhCn, "pending") => "启动中",
            (Locale::EnUs, "unavailable") => "unavailable",
            (Locale::ZhCn, "unavailable") => "不可用",
            (Locale::EnUs, "failed") => "failed",
            (Locale::ZhCn, "failed") => "失败",
            (Locale::EnUs, _) => "outcome unknown",
            (Locale::ZhCn, _) => "结果未知",
        };
        let geometry = &terminal["geometry"];
        match (
            geometry["columns"].as_u64(),
            geometry["rows"].as_u64(),
            terminal["reason"].as_str(),
        ) {
            (Some(columns), Some(rows), _) => format!("{state} ({columns}x{rows})"),
            (.., Some(reason)) => format!("{state} ({})", escape(reason)),
            _ => state.to_owned(),
        }
    }
}
