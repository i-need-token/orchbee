//! Localized Agent prompt admission and acknowledgement feedback; does not submit prompts.

use super::Locale;

impl Locale {
    #[cfg(any(target_os = "linux", windows))]
    #[cfg(windows)]
    pub(crate) fn cli_terminal_prompt_submitted(self, presentation: &str, matched: bool) -> String {
        match (self, matched) {
            (Self::EnUs, false) => format!("Prompt submitted to terminal {presentation}.\n"),
            (Self::EnUs, true) => {
                format!("Prompt submitted to terminal {presentation}; wait conditions matched.\n")
            }
            (Self::ZhCn, false) => format!("提示词已提交至终端 {presentation}。\n"),
            (Self::ZhCn, true) => format!("提示词已提交至终端 {presentation}；等待条件已满足。\n"),
        }
    }

    pub(crate) fn cli_prompt_submitted(self, session: &str, matched: bool) -> String {
        match (self, matched) {
            (Self::EnUs, false) => {
                format!("Prompt input delivered to the terminal for Agent {session}.\n")
            }
            (Self::EnUs, true) => format!(
                "Prompt input delivered to the terminal for Agent {session}; a wait condition matched.\n"
            ),
            (Self::ZhCn, false) => format!("提示输入已送达智能体 {session} 的终端。\n"),
            (Self::ZhCn, true) => {
                format!("提示输入已送达智能体 {session} 的终端，等待条件已匹配。\n")
            }
        }
    }

    #[cfg(any(target_os = "linux", windows))]
    pub(crate) fn cli_prompt_hint(self, reason: &str) -> Option<&'static str> {
        match (self, reason) {
            (Self::EnUs, "agent-launch-pending") => Some(
                "The Agent is still starting. Wait for agent start to finish before sending a prompt.",
            ),
            (Self::ZhCn, "agent-launch-pending") => {
                Some("智能体仍在启动。请等待 agent start 完成后再提交提示。")
            }
            (Self::EnUs, "agent-blocked") => Some(
                "Agent needs attention. Resolve its pending request before submitting a prompt.",
            ),
            (Self::ZhCn, "agent-blocked") => Some("智能体需要处理待决请求。处理后再提交提示。"),
            (Self::EnUs, "agent-not-ready") => Some(
                "Agent state is not fresh and known. Inspect agent get/explain before submitting.",
            ),
            (Self::ZhCn, "agent-not-ready") => {
                Some("智能体状态不够新鲜或仍未知。提交前请检查 agent get/explain。")
            }
            (Self::EnUs, "prompt-stalled" | "effect-deadline") => Some(
                "Input was accepted but no new Agent activity was observed. Inspect the terminal; do not blindly resend.",
            ),
            (Self::ZhCn, "prompt-stalled" | "effect-deadline") => {
                Some("输入已接受，但尚未观察到新的智能体活动。请检查终端，不要盲目重发。")
            }
            (Self::EnUs, "client-read-failed") => Some(
                "The selected Client could not be read. Inspect the failure and any retained input receipt before retrying.",
            ),
            (Self::ZhCn, "client-read-failed") => {
                Some("无法读取所选客户端。重试前请检查失败信息及保留的输入回执。")
            }
            _ => None,
        }
    }
}
