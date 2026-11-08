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

    /// `agent doctor` hint text by hint id — observation statements only;
    /// every hint cites the facts it was derived from, never a remedy.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_agent_doctor_hint(self, id: &str) -> &'static str {
        match (self.locale, id) {
            (Locale::EnUs, "hook-conflict") => {
                "conflicting managed-hook declarations were observed for"
            }
            (Locale::ZhCn, "hook-conflict") => "观察到以下提供方存在相互矛盾的托管钩子声明：",
            (Locale::EnUs, "hook-other-environment") => {
                "the managed hook belongs to another Orchbee environment, which alone changes it, for"
            }
            (Locale::ZhCn, "hook-other-environment") => {
                "以下提供方的托管钩子属于另一个 Orchbee 环境，只有该环境能修改它："
            }
            (Locale::EnUs, "hook-unknown") => {
                "managed-hook declarations could not be fully inspected for"
            }
            (Locale::ZhCn, "hook-unknown") => "以下提供方的托管钩子声明无法完整检查：",
            (Locale::EnUs, "hook-disabled") => {
                "a managed hook is declared but explicitly disabled for"
            }
            (Locale::ZhCn, "hook-disabled") => "以下提供方声明了托管钩子但被显式禁用：",
            (Locale::EnUs, "hook-update-required") => "an outdated managed hook was observed for",
            (Locale::ZhCn, "hook-update-required") => "观察到以下提供方的托管钩子需要更新：",
            (Locale::EnUs, "hook-unsupported") => {
                "the hook state needs the provider's own command surface, which offline inspection does not run, for"
            }
            (Locale::ZhCn, "hook-unsupported") => {
                "以下提供方的钩子状态需要其自身命令面（离线检查不执行命令）："
            }
            (Locale::EnUs, "hook-present-executable-absent") => {
                "a managed hook is declared but the provider executable is absent from PATH for"
            }
            (Locale::ZhCn, "hook-present-executable-absent") => {
                "以下提供方已声明托管钩子但 PATH 中不存在其可执行文件："
            }
            (Locale::EnUs, "hook-absent-executable-present") => {
                "the provider executable is on PATH but no managed hook is declared for"
            }
            (Locale::ZhCn, "hook-absent-executable-present") => {
                "以下提供方的可执行文件在 PATH 中但未声明托管钩子："
            }
            (Locale::EnUs, "node-absent") => {
                "the hook payload requires node, which is absent from PATH, for"
            }
            (Locale::ZhCn, "node-absent") => {
                "以下提供方的钩子负载需要 node，但 PATH 中不存在 node："
            }
            (Locale::EnUs, "endpoint-absent") => {
                "the observation endpoint socket is absent; hooks deliver only while the managed session is running."
            }
            (Locale::ZhCn, "endpoint-absent") => {
                "观察端点套接字不存在；钩子仅在托管会话运行时投递。"
            }
            (Locale::EnUs, "windows-endpoint-not-inspected") => {
                "Windows endpoint delivery is not inspected by this offline command. Managed hooks and modules support the native transport; configuration and capsule presence do not prove a working Agent connection."
            }
            (Locale::ZhCn, "windows-endpoint-not-inspected") => {
                "此离线命令不检查 Windows 端点投递。托管 hook 和模块支持原生传输；检测到配置和胶囊不证明智能体已接通。"
            }
            (Locale::EnUs, "windows-hook-transport-unavailable") => {
                "The shipped Agent hooks cannot yet deliver events on Windows. Configuration and capsule presence do not establish a working Agent connection."
            }
            (Locale::ZhCn, "windows-hook-transport-unavailable") => {
                "随附的智能体 hook 尚不能在 Windows 投递事件。检测到配置和胶囊不代表智能体已接通。"
            }
            (Locale::EnUs, "endpoint-anomaly") => {
                "the observation endpoint path exists but is not admitted by the consumers' ownership and type checks."
            }
            (Locale::ZhCn, "endpoint-anomaly") => {
                "观察端点路径存在但未通过消费者的所有权与类型检查。"
            }
            (Locale::EnUs, "env-override-invalid") => {
                "ORCHBEE_AGENT_OBSERVATION_SOCKET is set but is not an absolute path."
            }
            (Locale::ZhCn, "env-override-invalid") => {
                "ORCHBEE_AGENT_OBSERVATION_SOCKET 已设置但不是绝对路径。"
            }
            (Locale::EnUs, "root-anomaly") => {
                "an asset root contradicts its own manifest declaration:"
            }
            (Locale::ZhCn, "root-anomaly") => "某个资产根与其自身清单声明矛盾：",
            (Locale::EnUs, "root-uninspectable") => "an asset root could not be fully inspected:",
            (Locale::ZhCn, "root-uninspectable") => "某个资产根无法完整检查：",
            (Locale::EnUs, "root-stale") => {
                "an Agent entry still runs the Capsule folder all environments shared before each kept its own; only release Orchbee changes it, with agent integration enable or disable:"
            }
            (Locale::ZhCn, "root-stale") => {
                "某个智能体条目仍在运行各环境改用各自胶囊之前共用的胶囊文件夹；只有正式版 Orchbee 能用 agent integration enable 或 disable 修改它："
            }
            (Locale::EnUs, "partial-document") => {
                "the document is partial: the diagnostic budget exhausted before every fact was probed."
            }
            (Locale::ZhCn, "partial-document") => "文档不完整：诊断预算在探测完全部事实前已耗尽。",
            (Locale::EnUs, _) => "additional facts were observed.",
            (Locale::ZhCn, _) => "观察到其他事实。",
        }
    }

    /// Localized `agent doctor` hint sentence: the `id` statement followed by
    /// the cited `subjects` (provider ids or root paths), joined with the
    /// locale's enumeration separator. The machine document always uses the
    /// canonical `en-US` form; text output re-renders through this method.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_agent_doctor_hint_text(self, id: &str, subjects: &[String]) -> String {
        let base = self.cli_agent_doctor_hint(id);
        if subjects.is_empty() {
            return base.to_owned();
        }
        match self.locale {
            Locale::EnUs => format!("{base} {}", subjects.join(", ")),
            Locale::ZhCn => format!("{base}{}", subjects.join("、")),
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
