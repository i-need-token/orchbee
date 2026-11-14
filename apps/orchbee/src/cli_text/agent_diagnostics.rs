//! Localized Agent diagnostic hints; does not collect diagnostic evidence.

use super::Locale;
use orchbee_agent_observation_protocol::{
    AgentIdentityGrade as Grade, AgentIdentityUnknownReason as Unknown,
    AgentIdentityUnsupportedReason as Unsupported, AgentProcessIdentity as Identity,
};

impl super::CliText {
    /// The Server's process identity result as one value: provider and grade, or the reason.
    pub(crate) fn cli_agent_identity(self, identity: Identity) -> String {
        let zh = self.locale == Locale::ZhCn;
        match identity {
            Identity::Agent { provider, grade } => {
                let provider = provider.native_provider_id();
                match (zh, grade) {
                    (false, Grade::Confirmed) => format!("{provider} Agent, confirmed"),
                    (false, Grade::Recognized) => format!("{provider} Agent, recognized"),
                    (true, Grade::Confirmed) => format!("{provider} 智能体，已确认"),
                    (true, Grade::Recognized) => format!("{provider} 智能体，已识别"),
                }
            }
            Identity::None if zh => "没有智能体".to_owned(),
            Identity::None => "No Agent".to_owned(),
            Identity::Unknown(reason) if zh => format!("未知：{}", self.unknown_reason(reason)),
            Identity::Unknown(reason) => format!("Unknown: {}", self.unknown_reason(reason)),
            Identity::Unsupported(reason) if zh => {
                format!("不支持：{}", self.unsupported_reason(reason))
            }
            Identity::Unsupported(reason) => {
                format!("Unsupported: {}", self.unsupported_reason(reason))
            }
        }
    }

    const fn unknown_reason(self, reason: Unknown) -> &'static str {
        match (self.locale, reason) {
            (Locale::EnUs, Unknown::Settling) => "the Agent is still settling",
            (Locale::ZhCn, Unknown::Settling) => "智能体尚未稳定",
            (Locale::EnUs, Unknown::SampleFailed) => "process sampling is failing",
            (Locale::ZhCn, Unknown::SampleFailed) => "进程采样持续失败",
            (Locale::EnUs, Unknown::SampleLimit) => "the process sample reached its limit",
            (Locale::ZhCn, Unknown::SampleLimit) => "进程采样达到上限",
            (Locale::EnUs, Unknown::CommandLineUnreadable) => "a command line is unreadable",
            (Locale::ZhCn, Unknown::CommandLineUnreadable) => "有命令行无法读取",
            (Locale::EnUs, Unknown::AmbiguousCandidates) => "several Agent candidates",
            (Locale::ZhCn, Unknown::AmbiguousCandidates) => "存在多个智能体候选",
            (Locale::EnUs, Unknown::UnclassifiedArguments) => "arguments are not classified",
            (Locale::ZhCn, Unknown::UnclassifiedArguments) => "参数无法分类",
            (Locale::EnUs, Unknown::UnverifiedProvenance) => "the install source is unverified",
            (Locale::ZhCn, Unknown::UnverifiedProvenance) => "安装来源未经验证",
            (Locale::EnUs, Unknown::DetachedConsole) => "the Agent is detached from the console",
            (Locale::ZhCn, Unknown::DetachedConsole) => "智能体已脱离控制台",
        }
    }

    const fn unsupported_reason(self, reason: Unsupported) -> &'static str {
        match (self.locale, reason) {
            (Locale::EnUs, Unsupported::Platform) => "this platform",
            (Locale::ZhCn, Unsupported::Platform) => "当前平台",
            (Locale::EnUs, Unsupported::RemoteExecution) => "remote execution",
            (Locale::ZhCn, Unsupported::RemoteExecution) => "远程执行",
        }
    }

    pub(crate) fn cli_agent_runtime_hint(self, reason: &str) -> Option<&'static str> {
        match (self.locale, reason) {
            (Locale::EnUs, "agent-provider-unsupported") => Some(
                "This Agent provider has no runtime identification rule. Inspect panel process-info before using terminal send explicitly.",
            ),
            (Locale::ZhCn, "agent-provider-unsupported") => Some(
                "此智能体 provider 尚无运行时识别规则。请先检查 panel process-info，再显式使用 terminal send。",
            ),
            (Locale::EnUs, "agent-not-running") => Some(
                "The terminal no longer contains the expected Agent. Inspect panel process-info and agent get before sending input.",
            ),
            (Locale::ZhCn, "agent-not-running") => {
                Some("终端中已找不到预期智能体。发送前请检查 panel process-info 和 agent get。")
            }
            (Locale::EnUs, "agent-runtime-ambiguous") => Some(
                "Several Agent processes cannot be resolved to one foreground chain. Inspect panel process-info before sending input.",
            ),
            (Locale::ZhCn, "agent-runtime-ambiguous") => {
                Some("多个智能体进程无法确定为一条前台链。发送前请检查 panel process-info。")
            }
            (Locale::EnUs, "agent-identity-unknown") => Some(
                "The terminal's Agent cannot be identified well enough to receive input now. Inspect the identity in panel process-info, or use terminal send for explicit input.",
            ),
            (Locale::ZhCn, "agent-identity-unknown") => Some(
                "目前无法充分确认终端中的智能体身份，暂不发送输入。请查看 panel process-info 中的身份信息，或用 terminal send 显式发送。",
            ),
            (Locale::EnUs, "agent-provider-mismatch") => Some(
                "The running Agent differs from the selected session. Inspect panel process-info and select the intended Agent.",
            ),
            (Locale::ZhCn, "agent-provider-mismatch") => Some(
                "实际运行的智能体与所选会话不同。请检查 panel process-info 并重新选择目标智能体。",
            ),
            (Locale::EnUs, "agent-binding-changed") => Some(
                "The Agent binding changed during preflight. Resolve the target again before sending input.",
            ),
            (Locale::ZhCn, "agent-binding-changed") => {
                Some("预检期间智能体绑定已变化。发送前请重新解析目标。")
            }
            _ => None,
        }
    }
}
