//! Localized projection labels and actionable complete-read/selector failures.
use super::Locale;
use crate::cli::{Failure, FailureKind};
impl Locale {
    pub(crate) fn cli_panel_table_headers(self) -> [&'static str; 6] {
        match self {
            Self::EnUs => ["#", "HANDLE", "TITLE", "AGENT", "STATE", "CWD"],
            Self::ZhCn => ["#", "句柄", "标题", "智能体", "状态", "目录"],
        }
    }
    pub(crate) fn cli_projection_headers(self) -> [&'static str; 6] {
        match self {
            Self::EnUs => ["#", "HANDLE", "AGENT", "STATE", "SOURCE", "CWD"],
            Self::ZhCn => ["#", "句柄", "智能体", "状态", "来源", "目录"],
        }
    }
    pub(crate) fn cli_projection_source(self, value: &str) -> &'static str {
        match (self, value) {
            (Self::EnUs, "canonical") => "Canonical",
            (Self::EnUs, "terminal") => "Terminal",
            (Self::EnUs, _) => "Both",
            (Self::ZhCn, "canonical") => "规范",
            (Self::ZhCn, "terminal") => "终端",
            (Self::ZhCn, _) => "两者",
        }
    }
    pub(crate) fn cli_projection_status(self, value: &str) -> &'static str {
        match (self, value) {
            (Self::EnUs, "idle") => "Idle",
            (Self::EnUs, "working") => "Working",
            (Self::EnUs, "blocked") => "Blocked",
            (Self::EnUs, "done") => "Done",
            (Self::EnUs, "error") => "Error",
            (Self::EnUs, "ended") => "Ended",
            (Self::EnUs, _) => "Unknown",
            (Self::ZhCn, "idle") => "空闲",
            (Self::ZhCn, "working") => "工作中",
            (Self::ZhCn, "blocked") => "受阻",
            (Self::ZhCn, "done") => "完成",
            (Self::ZhCn, "error") => "错误",
            (Self::ZhCn, "ended") => "已结束",
            (Self::ZhCn, _) => "未知",
        }
    }
    pub(crate) fn cli_projection_failure(self, failure: &Failure) -> String {
        let reason = failure
            .details
            .as_ref()
            .and_then(|d| d.get("reason"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let message = match (self, failure.kind, reason) {
            (Self::EnUs, FailureKind::NotFound, _) => {
                "No Agent matches the target. Run orchbee agent list to inspect current targets."
            }
            (Self::ZhCn, FailureKind::NotFound, _) => {
                "没有与目标匹配的智能体。请运行 orchbee agent list 查看当前目标。"
            }
            (Self::EnUs, FailureKind::Ambiguous, _) => {
                "More than one Agent matches. Choose a current candidate below:"
            }
            (Self::ZhCn, FailureKind::Ambiguous, _) => "匹配到多个智能体，请选择下面的当前候选项：",
            (Self::EnUs, _, "changed" | "cursor-restart") => {
                "The Agent list changed or its cursor is obsolete. Restart with orchbee agent list without --cursor."
            }
            (Self::ZhCn, _, "changed" | "cursor-restart") => {
                "智能体列表已变化或游标已失效。请运行不带 --cursor 的 orchbee agent list 重新列举。"
            }
            (Self::EnUs, FailureKind::Cancelled, _) => "The Agent projection read was cancelled.",
            (Self::ZhCn, FailureKind::Cancelled, _) => "智能体投影读取已取消。",
            (Self::EnUs, FailureKind::Timeout, _) => {
                "The complete Agent projection could not be read within 5 seconds."
            }
            (Self::ZhCn, FailureKind::Timeout, _) => "未能在 5 秒内完整读取智能体投影。",
            (Self::EnUs, FailureKind::OutputTooLarge, _) => {
                "The complete Agent projection exceeds its size limit; no partial list was returned."
            }
            (Self::ZhCn, FailureKind::OutputTooLarge, _) => {
                "完整智能体投影超出大小限制，未返回部分列表。"
            }
            (Self::EnUs, FailureKind::Denied, _) => {
                "The local session denied the Agent projection read."
            }
            (Self::ZhCn, FailureKind::Denied, _) => "本地会话拒绝了智能体投影读取。",
            (Self::EnUs, _, _) => {
                "A complete Agent projection is unavailable. Check orchbee status server and run orchbee agent list again."
            }
            (Self::ZhCn, _, _) => {
                "当前无法取得完整智能体投影。请检查 orchbee status server 后重新运行 orchbee agent list。"
            }
        };
        let mut text = format!("{message}\n");
        if let Some(candidates) = failure
            .details
            .as_ref()
            .and_then(|d| d.get("candidates"))
            .and_then(|v| v.as_array())
        {
            for candidate in candidates {
                if let Some(selector) = candidate["selector"].as_str() {
                    text.push_str("  ");
                    text.push_str(&crate::cli::escape::unquoted(selector));
                    if let Some(id) = candidate["sessionId"].as_str() {
                        text.push_str("  --agent-session ");
                        text.push_str(&crate::cli::escape::quoted(id));
                    }
                    if let Some(id) = candidate["panelId"].as_str() {
                        text.push_str("  panel=");
                        text.push_str(&crate::cli::escape::unquoted(id));
                    }
                    text.push('\n');
                }
            }
        }
        text
    }
}
