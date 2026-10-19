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

impl Locale {
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
            text.push_str(&match self {
                Self::EnUs => format!("Next: orchbee terminal list --panel {}\n", escape(panel)),
                Self::ZhCn => format!("下一步：orchbee terminal list --panel {}\n", escape(panel)),
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
        match (self, resource) {
            (Self::EnUs, resource) => format!(
                "Closed {resource} {label} ({terminals} {}).\n",
                if terminals == 1 {
                    "terminal"
                } else {
                    "terminals"
                }
            ),
            (Self::ZhCn, "workspace") => format!("已关闭工作区 {label}（{terminals} 个终端）。\n"),
            (Self::ZhCn, _) => format!("已关闭标签页 {label}（{terminals} 个终端）。\n"),
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
        match self {
            Self::EnUs => format!(
                "The close was accepted but had not finished after {timeout_ms} ms; it continues in the background. Check with: {check}\n"
            ),
            Self::ZhCn => {
                format!(
                    "关闭已被接纳，但 {timeout_ms} 毫秒内尚未完成；它会在后台继续。查看进度：{check}\n"
                )
            }
        }
    }

    /// The opening line of a close `--dry-run`.
    pub(crate) fn cli_close_preview(self, resource: &str) -> String {
        match (self, resource) {
            (Self::EnUs, resource) => format!(
                "Dry run: closing this {resource} with --confirm would end these terminals and the processes and Agents running in them.\n"
            ),
            (Self::ZhCn, "workspace") => {
                "试运行：使用 --confirm 关闭该工作区将结束以下终端及其中运行的进程和 Agent。\n"
                    .to_owned()
            }
            (Self::ZhCn, _) => {
                "试运行：使用 --confirm 关闭该标签页将结束以下终端及其中运行的进程和 Agent。\n"
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
        match (self, outcome, verb) {
            (Self::EnUs, "unchanged", _) => {
                format!("No change: the {en} already matched the request.\n")
            }
            (Self::ZhCn, "unchanged", _) => format!("未改变：该{zh}已符合请求。\n"),
            (Self::EnUs, "accepted", _) => {
                format!("Closing {named_en}; the session finishes the close in the background.\n")
            }
            (Self::ZhCn, "accepted", _) => {
                format!("正在关闭{named_zh}；会话会在后台完成关闭。\n")
            }
            (Self::EnUs, "resumed", _) => format!("Resumed closing the {en}.\n"),
            (Self::ZhCn, "resumed", _) => format!("已继续关闭该{zh}。\n"),
            (Self::EnUs, _, "create") => format!("Created {named_en}.\n"),
            (Self::ZhCn, _, "create") => format!("已创建{named_zh}。\n"),
            (Self::EnUs, _, "split") => "Split the panel.\n".to_owned(),
            (Self::ZhCn, _, "split") => "已分裂面板。\n".to_owned(),
            (Self::EnUs, _, "rename") => match &label {
                Some(label) => format!("Renamed the {en} to {label}.\n"),
                None => format!("Renamed the {en}.\n"),
            },
            (Self::ZhCn, _, "rename") => match &label {
                Some(label) => format!("已将该{zh}重命名为 {label}。\n"),
                None => format!("已重命名该{zh}。\n"),
            },
            (Self::EnUs, _, "move" | "move-block" | "relocate") => format!("Moved the {en}.\n"),
            (Self::ZhCn, _, "move" | "move-block" | "relocate") => format!("已移动该{zh}。\n"),
            (Self::EnUs, _, "delete") => format!("Deleted {named_en}.\n"),
            (Self::ZhCn, _, "delete") => format!("已删除{named_zh}。\n"),
            (Self::EnUs, _, "dissolve") => format!("Dissolved the {en}.\n"),
            (Self::ZhCn, _, "dissolve") => format!("已解散该{zh}。\n"),
            (Self::EnUs, ..) => format!("Updated the {en}.\n"),
            (Self::ZhCn, ..) => format!("已更新该{zh}。\n"),
        }
    }

    fn receipt_field(self, key: &str) -> &'static str {
        match (self, key) {
            (Self::EnUs, "projectId") => "Project",
            (Self::ZhCn, "projectId") => "项目",
            (Self::EnUs, "workspaceId") => "Workspace",
            (Self::ZhCn, "workspaceId") => "工作区",
            (Self::EnUs, "workspaceIds") => "Workspaces",
            (Self::ZhCn, "workspaceIds") => "工作区列表",
            (Self::EnUs, "beforeWorkspaceId") => "Before workspace",
            (Self::ZhCn, "beforeWorkspaceId") => "插入位置之后的工作区",
            (Self::EnUs, "createdWorkspaceId") => "New workspace",
            (Self::ZhCn, "createdWorkspaceId") => "新工作区",
            (Self::EnUs, "closedSourceWorkspaceId") => "Closed workspace",
            (Self::ZhCn, "closedSourceWorkspaceId") => "已关闭的工作区",
            (Self::EnUs, "tabId") => "Tab",
            (Self::ZhCn, "tabId") => "标签页",
            (Self::EnUs, "targetTabId") => "Target tab",
            (Self::ZhCn, "targetTabId") => "目标标签页",
            (Self::EnUs, "newPanelId") => "New panel",
            (Self::ZhCn, "newPanelId") => "新面板",
            (Self::EnUs, "panelId") => "Panel",
            (Self::ZhCn, "panelId") => "面板",
            (Self::EnUs, "targetPanelId") => "Target panel",
            (Self::ZhCn, "targetPanelId") => "目标面板",
            (Self::EnUs, "profileId") => "Launch profile",
            (Self::ZhCn, "profileId") => "启动配置",
            (Self::EnUs, "name") => "Name",
            (Self::ZhCn, "name") => "名称",
            (Self::EnUs, "label") => "Label",
            (Self::ZhCn, "label") => "标签",
            (Self::EnUs, "customLabel") => "Custom label",
            (Self::ZhCn, "customLabel") => "自定义标签",
            (Self::EnUs, "direction") => "Direction",
            (Self::ZhCn, "direction") => "方向",
            (Self::EnUs, "insertIndex") => "Position",
            (Self::ZhCn, "insertIndex") => "位置",
            (Self::EnUs, "rightClick") => "Right click",
            (Self::ZhCn, "rightClick") => "右键",
            (Self::EnUs, "path") => "Directory",
            (Self::ZhCn, "path") => "目录",
            (Self::EnUs, _) => "Terminal",
            (Self::ZhCn, _) => "终端",
        }
    }

    const fn receipt_cleared(self) -> &'static str {
        match self {
            Self::EnUs => "cleared",
            Self::ZhCn => "已清除",
        }
    }

    /// The spawn outcome of a created terminal, with its grid when started.
    fn receipt_terminal(self, terminal: &Value) -> String {
        let outcome = terminal["outcome"].as_str().unwrap_or_default();
        let state = match (self, outcome) {
            (Self::EnUs, "started") => "started",
            (Self::ZhCn, "started") => "已启动",
            (Self::EnUs, "pending") => "starting",
            (Self::ZhCn, "pending") => "启动中",
            (Self::EnUs, "unavailable") => "unavailable",
            (Self::ZhCn, "unavailable") => "不可用",
            (Self::EnUs, "failed") => "failed",
            (Self::ZhCn, "failed") => "失败",
            (Self::EnUs, _) => "outcome unknown",
            (Self::ZhCn, _) => "结果未知",
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
