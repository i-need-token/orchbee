//! Localized group progress and recovery instructions; untrusted labels never reach the terminal raw.
use crate::{cli::escape::escape, localization::Locale};
use serde_json::Value;
impl Locale {
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
        let labels = match self {
            Self::EnUs => [
                "Operation",
                "Attempt",
                "Reason",
                "Waiting",
                "Project members",
                "Close",
                "Generation",
            ],
            Self::ZhCn => ["操作", "尝试", "原因", "等待", "项目成员", "关闭", "代次"],
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
        match (self, state) {
            (Self::EnUs, "pending" | "closing") => "Closing",
            (Self::ZhCn, "pending" | "closing") => "正在关闭",
            (Self::EnUs, "closed") => "Closed",
            (Self::ZhCn, "closed") => "已关闭",
            (Self::EnUs, "failed") => "Project close-all failed",
            (Self::ZhCn, "failed") => "项目批量关闭失败",
            (Self::EnUs, "not-found") => "Operation not recorded",
            (Self::ZhCn, "not-found") => "未找到操作记录",
            (Self::EnUs, "conflict") => "Operation identity conflict",
            (Self::ZhCn, "conflict") => "操作身份冲突",
            (Self::EnUs, "denied") => "Permission denied",
            (Self::ZhCn, "denied") => "权限不足",
            (Self::EnUs, "unknown" | "outcome-unknown") => "Outcome unknown",
            (Self::ZhCn, "unknown" | "outcome-unknown") => "结果未知",
            (Self::EnUs, "unsubmitted") => "Not submitted",
            (Self::ZhCn, "unsubmitted") => "尚未提交",
            (Self::EnUs, "unobserved") => "Accepted; status not observed",
            (Self::ZhCn, "unobserved") => "已接纳；尚未观测状态",
            (Self::EnUs, "cleanup-blocked") => "Cleanup blocked",
            (Self::ZhCn, "cleanup-blocked") => "清理受阻",
            (Self::EnUs, "disposed-awaiting-publication") => "Disposed; publication pending",
            (Self::ZhCn, "disposed-awaiting-publication") => "已处置；等待发布",
            (Self::EnUs, "detached") => "Detached; operation continues",
            (Self::ZhCn, "detached") => "已分离；操作继续",
            (Self::EnUs, "cancelled") => "Wait cancelled",
            (Self::ZhCn, "cancelled") => "已取消等待",
            (Self::EnUs, "timed-out") => "Wait timed out",
            (Self::ZhCn, "timed-out") => "等待超时",
            (Self::EnUs, "transport-lost") => "Connection lost",
            (Self::ZhCn, "transport-lost") => "连接中断",
            (Self::EnUs, "observation-failed") => "Observation failed",
            (Self::ZhCn, "observation-failed") => "观测失败",
            (Self::EnUs, _) => "Unavailable",
            (Self::ZhCn, _) => "不可用",
        }
    }
}
