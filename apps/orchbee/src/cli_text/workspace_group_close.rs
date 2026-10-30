//! Localized group progress and recovery instructions; untrusted labels never reach the terminal raw.
use crate::{cli_text::escape::escape, localization::Locale};
use serde_json::Value;
impl super::CliText {
    pub(crate) fn cli_workspace_group_close_result(self, result: &Value) -> String {
        let observation = &result["observation"];
        let outcome = &observation["reply"]["outcome"];
        let status = &outcome["status"];
        let state = status["kind"]
            .as_str()
            .or_else(|| outcome["kind"].as_str())
            .or_else(|| observation["kind"].as_str())
            .unwrap_or("unavailable");
        let mut text = format!("{}\n", self.group_close_state(state));
        let labels = match self.locale {
            Locale::EnUs => [
                "Operation",
                "Attempt",
                "Reason",
                "Waiting",
                "Project members",
                "Close",
                "Generation",
            ],
            Locale::ZhCn => ["操作", "尝试", "原因", "等待", "项目成员", "关闭", "代次"],
        };
        let id = result["operationId"].as_str().unwrap_or("");
        text.push_str(&format!("{}: {}\n", labels[0], escape(id)));
        if let Some(attempt) = outcome["attempt"].as_u64() {
            text.push_str(&format!("{}: {attempt}\n", labels[1]));
        }
        if let Some(reason) = status["reason"]
            .as_str()
            .or_else(|| outcome["reason"].as_str())
            .or_else(|| observation["reason"].as_str())
        {
            text.push_str(&format!("{}: {}\n", labels[2], escape(reason)));
        }
        if let Some(wait) = result["wait"].as_str() {
            text.push_str(&format!(
                "{}: {}\n",
                labels[3],
                self.group_close_state(wait)
            ));
        }
        if let Some(members) = outcome["members"].as_array() {
            text.push_str(&format!("{}: {}\n", labels[4], members.len()));
            for member in members {
                let close = &member["close"];
                let state = if close.is_null() {
                    "unsubmitted"
                } else {
                    close["observation"]["status"]["kind"]
                        .as_str()
                        .unwrap_or("unobserved")
                };
                text.push_str(&format!(
                    "  {}  {}  {}\n",
                    escape(member["workspaceId"].as_str().unwrap_or("")),
                    escape(member["label"].as_str().unwrap_or("")),
                    self.group_close_state(state)
                ));
                if let Some(request) = close["requestId"].as_str() {
                    text.push_str(&format!("    {}: {}", labels[5], escape(request)));
                    if let Some(generation) = close["observation"]["attemptGeneration"].as_str() {
                        text.push_str(&format!("  {}: {}", labels[6], escape(generation)));
                    }
                    text.push('\n');
                }
            }
        }
        if state == "pending" || state == "failed" || state == "unknown" {
            text.push_str(&format!(
                "orchbee project close-status --request {}\n",
                escape(id)
            ));
        }
        if status["resumable"].as_bool() == Some(true)
            && let Some(attempt) = outcome["attempt"].as_u64()
        {
            text.push_str(&format!(
                "orchbee project close-resume --request {} --generation {attempt} --confirm\n",
                escape(id)
            ));
        }
        text
    }
    fn group_close_state(self, state: &str) -> &'static str {
        match (self.locale, state) {
            (Locale::EnUs, "pending" | "closing") => "Closing",
            (Locale::ZhCn, "pending" | "closing") => "正在关闭",
            (Locale::EnUs, "closed") => "Closed",
            (Locale::ZhCn, "closed") => "已关闭",
            (Locale::EnUs, "failed") => "Project close-all failed",
            (Locale::ZhCn, "failed") => "项目批量关闭失败",
            (Locale::EnUs, "not-found") => "Operation not recorded",
            (Locale::ZhCn, "not-found") => "未找到操作记录",
            (Locale::EnUs, "conflict") => "Operation identity conflict",
            (Locale::ZhCn, "conflict") => "操作身份冲突",
            (Locale::EnUs, "denied") => "Permission denied",
            (Locale::ZhCn, "denied") => "权限不足",
            (Locale::EnUs, "unknown" | "outcome-unknown") => "Outcome unknown",
            (Locale::ZhCn, "unknown" | "outcome-unknown") => "结果未知",
            (Locale::EnUs, "unsubmitted") => "Not submitted",
            (Locale::ZhCn, "unsubmitted") => "尚未提交",
            (Locale::EnUs, "unobserved") => "Accepted; status not observed",
            (Locale::ZhCn, "unobserved") => "已接纳；尚未观测状态",
            (Locale::EnUs, "cleanup-blocked") => "Cleanup blocked",
            (Locale::ZhCn, "cleanup-blocked") => "清理受阻",
            (Locale::EnUs, "disposed-awaiting-publication") => "Disposed; publication pending",
            (Locale::ZhCn, "disposed-awaiting-publication") => "已处置；等待发布",
            (Locale::EnUs, "detached") => "Detached; operation continues",
            (Locale::ZhCn, "detached") => "已分离；操作继续",
            (Locale::EnUs, "cancelled") => "Wait cancelled",
            (Locale::ZhCn, "cancelled") => "已取消等待",
            (Locale::EnUs, "timed-out") => "Wait timed out",
            (Locale::ZhCn, "timed-out") => "等待超时",
            (Locale::EnUs, "transport-lost") => "Connection lost",
            (Locale::ZhCn, "transport-lost") => "连接中断",
            (Locale::EnUs, "observation-failed") => "Observation failed",
            (Locale::ZhCn, "observation-failed") => "观测失败",
            (Locale::EnUs, _) => "Unavailable",
            (Locale::ZhCn, _) => "不可用",
        }
    }
}
