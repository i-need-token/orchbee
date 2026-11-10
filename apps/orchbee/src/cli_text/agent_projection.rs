//! Localized projection labels and actionable complete-read/selector failures.
use super::Locale;
use crate::cli_outcome::{Failure, FailureKind};
impl super::CliText {
    pub(crate) fn cli_panel_table_headers(self) -> [&'static str; 6] {
        match self.locale {
            Locale::EnUs => ["#", "HANDLE", "TITLE", "AGENT", "STATE", "CWD"],
            Locale::ZhCn => ["#", "句柄", "标题", "智能体", "状态", "目录"],
        }
    }
    pub(crate) fn cli_projection_headers(self) -> [&'static str; 6] {
        match self.locale {
            Locale::EnUs => ["#", "HANDLE", "AGENT", "STATE", "SOURCE", "CWD"],
            Locale::ZhCn => ["#", "句柄", "智能体", "状态", "来源", "目录"],
        }
    }
    pub(crate) fn cli_projection_source(self, value: &str) -> &'static str {
        match (self.locale, value) {
            (Locale::EnUs, "canonical") => "Canonical",
            (Locale::EnUs, "terminal") => "Terminal",
            (Locale::EnUs, _) => "Both",
            (Locale::ZhCn, "canonical") => "规范",
            (Locale::ZhCn, "terminal") => "终端",
            (Locale::ZhCn, _) => "两者",
        }
    }
    pub(crate) fn cli_projection_status(self, value: &str) -> &'static str {
        match (self.locale, value) {
            (Locale::EnUs, "idle") => "Idle",
            (Locale::EnUs, "working") => "Working",
            (Locale::EnUs, "blocked") => "Blocked",
            (Locale::EnUs, "done") => "Done",
            (Locale::EnUs, "error") => "Error",
            (Locale::EnUs, "ended") => "Ended",
            (Locale::EnUs, _) => "Unknown",
            (Locale::ZhCn, "idle") => "空闲",
            (Locale::ZhCn, "working") => "工作中",
            (Locale::ZhCn, "blocked") => "受阻",
            (Locale::ZhCn, "done") => "完成",
            (Locale::ZhCn, "error") => "错误",
            (Locale::ZhCn, "ended") => "已结束",
            (Locale::ZhCn, _) => "未知",
        }
    }
    pub(crate) fn cli_projection_failure(self, failure: &Failure) -> String {
        let reason = failure
            .details
            .as_ref()
            .and_then(|d| d.get("reason"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let message = match (self.locale, failure.kind, reason) {
            (Locale::EnUs, FailureKind::NotFound, _) => {
                "No Agent matches the target. Run orchbee agent list to inspect current targets."
            }
            (Locale::ZhCn, FailureKind::NotFound, _) => {
                "没有与目标匹配的智能体。请运行 orchbee agent list 查看当前目标。"
            }
            (Locale::EnUs, FailureKind::Ambiguous, _) => {
                "More than one Agent matches. Choose a current candidate below:"
            }
            (Locale::ZhCn, FailureKind::Ambiguous, _) => {
                "匹配到多个智能体，请选择下面的当前候选项："
            }
            (Locale::EnUs, _, "changed") => {
                "The Agent list changed while it was being read. Run the command again; orchbee agent list shows the current Agents."
            }
            (Locale::ZhCn, _, "changed") => {
                "读取期间智能体列表发生了变化。请重新运行该命令；orchbee agent list 可查看当前智能体。"
            }
            (Locale::EnUs, _, "cursor-restart") => {
                "The Agent list changed or its cursor is obsolete. Restart with orchbee agent list without --cursor."
            }
            (Locale::ZhCn, _, "cursor-restart") => {
                "智能体列表已变化或游标已失效。请运行不带 --cursor 的 orchbee agent list 重新列举。"
            }
            (Locale::EnUs, FailureKind::Cancelled, _) => "The Agent projection read was cancelled.",
            (Locale::ZhCn, FailureKind::Cancelled, _) => "智能体投影读取已取消。",
            (Locale::EnUs, FailureKind::Timeout, _) => {
                "The complete Agent projection could not be read within 5 seconds."
            }
            (Locale::ZhCn, FailureKind::Timeout, _) => "未能在 5 秒内完整读取智能体投影。",
            (Locale::EnUs, FailureKind::OutputTooLarge, _) => {
                "The complete Agent projection exceeds its size limit; no partial list was returned."
            }
            (Locale::ZhCn, FailureKind::OutputTooLarge, _) => {
                "完整智能体投影超出大小限制，未返回部分列表。"
            }
            (Locale::EnUs, FailureKind::Denied, _) => {
                "The local session denied the Agent projection read."
            }
            (Locale::ZhCn, FailureKind::Denied, _) => "本地会话拒绝了智能体投影读取。",
            (Locale::EnUs, _, _) => {
                "A complete Agent projection is unavailable. Check orchbee status server and run orchbee agent list again."
            }
            (Locale::ZhCn, _, _) => {
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
                    text.push_str(&crate::cli_text::escape::unquoted(selector));
                    if let Some(id) = candidate["sessionId"].as_str() {
                        text.push_str("  --agent-session ");
                        text.push_str(&crate::cli_text::escape::quoted(id));
                    }
                    if let Some(id) = candidate["panelId"].as_str() {
                        text.push_str("  panel=");
                        text.push_str(&crate::cli_text::escape::unquoted(id));
                    }
                    text.push('\n');
                }
            }
        }
        text
    }
}
