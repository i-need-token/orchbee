//! Localized Agent startup outcomes and provider choices; does not start or retry Agents.

use super::Locale;
use crate::cli::Failure;

impl Locale {
    pub(crate) fn cli_agent_started(self, name: &str, session: &str) -> String {
        match self {
            Self::EnUs => format!(
                "Agent {name} is ready ({session}).\nNext: orchbee agent prompt --name {name} <text>\n"
            ),
            Self::ZhCn => format!(
                "智能体 {name} 已就绪（{session}）。\n下一步：orchbee agent prompt --name {name} <text>\n"
            ),
        }
    }

    /// Local integration preflight refusals: nothing was reserved or sent, so
    /// the copy is the cause plus the one command that resolves it.
    fn cli_agent_start_preflight(self, reason: &str, provider: &str) -> Option<String> {
        let integration = crate::agent_provider::integration_id(provider).unwrap_or(provider);
        let executable = crate::agent_provider::executable(provider).unwrap_or(provider);
        Some(match (self, reason) {
            (Self::EnUs, "provider-not-found") => format!(
                "The {executable} command for the {provider} Agent was not found on PATH; nothing was started. Install it, then retry.\n"
            ),
            (Self::ZhCn, "provider-not-found") => format!(
                "PATH 中找不到 {provider} 智能体的命令 {executable}；未启动任何内容。请先安装再重试。\n"
            ),
            (Self::EnUs, "integration-not-enabled") => format!(
                "The {integration} integration is not enabled, so Orchbee cannot confirm the Agent is ready; nothing was started. Run: orchbee agent integration enable --provider {integration}\n"
            ),
            (Self::ZhCn, "integration-not-enabled") => format!(
                "{integration} 集成尚未启用，Orchbee 无法确认智能体就绪；未启动任何内容。请运行：orchbee agent integration enable --provider {integration}\n"
            ),
            (Self::EnUs, "integration-blocked") => format!(
                "The {integration} integration cannot be managed in its current state; nothing was started. Inspect it with: orchbee agent integration explain --provider {integration}\n"
            ),
            (Self::ZhCn, "integration-blocked") => format!(
                "{integration} 集成当前状态无法管理；未启动任何内容。请运行以下命令查看：orchbee agent integration explain --provider {integration}\n"
            ),
            _ => return None,
        })
    }

    pub(super) fn cli_agent_start_failure(self, failure: &Failure) -> String {
        if matches!(
            failure.usage.as_deref(),
            Some(crate::invocation::UsageReason::CallerContextUnavailable)
        ) {
            return match self {
                Self::EnUs => "Calling terminal context is missing or invalid. Run inside an Orchbee terminal, or pass --panel <id> to agent start. Inspect `orchbee agent start --help`.\n",
                Self::ZhCn => "调用终端上下文缺失或无效。请在 Orchbee 终端内运行，或为 agent start 传入 --panel <id>。查看 `orchbee agent start --help`。\n",
            }.to_owned();
        }
        let field = |key| {
            failure
                .details
                .as_ref()
                .and_then(|details| details.get(key))
                .and_then(|value| value.as_str())
                .unwrap_or("")
        };
        if field("reason") == "startup-failed"
            && let Some(cause) = failure
                .details
                .as_ref()
                .and_then(|details| details.get("cause"))
            && cause["kind"] == "not-found"
        {
            return match self {
                Self::EnUs => "The target panel was not found. Run `orchbee panel list`, then pass an existing panel ID to --panel.\n",
                Self::ZhCn => "未找到目标面板。请运行 `orchbee panel list`，再将现有面板 ID 传给 --panel。\n",
            }.to_owned();
        }
        if let Some(message) = self.cli_agent_start_preflight(field("reason"), field("providerId"))
        {
            return message;
        }
        let message = match (self, field("reason")) {
            (Self::EnUs, "terminal-not-idle") => {
                "The target must be an idle cmd, PowerShell or pwsh terminal with no other running command."
            }
            (Self::ZhCn, "terminal-not-idle") => {
                "目标必须是空闲的 cmd、PowerShell 或 pwsh 终端，且没有其他命令正在运行。"
            }
            (Self::EnUs, "name-in-use" | "presentation-busy") => {
                "The Agent name or terminal is already in use. Inspect `orchbee agent names`."
            }
            (Self::ZhCn, "name-in-use" | "presentation-busy") => {
                "智能体名称或终端已被占用。请用 `orchbee agent names` 查看。"
            }
            (Self::EnUs, "command-too-long") => {
                "The encoded Windows launch command exceeds 8000 characters. Reduce the arguments."
            }
            (Self::ZhCn, "command-too-long") => {
                "编码后的 Windows 启动命令超过 8000 个字符，请缩短参数。"
            }
            (Self::EnUs, "start-deadline") => {
                "The Agent did not confirm readiness before the deadline. Inspect its terminal and provider hooks."
            }
            (Self::ZhCn, "start-deadline") => {
                "智能体未在时限内确认就绪。请检查终端输出和提供方 hook。"
            }
            (Self::EnUs, "agent-blocked") => {
                "The Agent needs attention. Resolve the prompt shown in its terminal."
            }
            (Self::ZhCn, "agent-blocked") => "智能体需要人工处理，请查看其终端中的提示。",
            (Self::EnUs, "agent-exited") => {
                "The Agent exited before readiness was confirmed. Inspect its terminal output."
            }
            (Self::ZhCn, "agent-exited") => "智能体在确认就绪前已退出，请检查其终端输出。",
            (Self::EnUs, "agent-provider-mismatch" | "agent-runtime-ambiguous") => {
                "The running Agent does not uniquely match the requested provider."
            }
            (Self::ZhCn, "agent-provider-mismatch" | "agent-runtime-ambiguous") => {
                "运行中的智能体无法唯一匹配所请求的提供方。"
            }
            (Self::EnUs, "binding-changed") => "The target terminal changed during startup.",
            (Self::ZhCn, "binding-changed") => "目标终端在启动期间发生了变化。",
            (Self::EnUs, "reply-not-confirmed") => {
                "The startup operation's result is unknown. Inspect Agent names and the terminal before retrying."
            }
            (Self::ZhCn, "reply-not-confirmed") => {
                "启动操作的结果尚不确定。重试前请检查智能体名称和终端。"
            }
            (Self::EnUs, "cancelled") => "Agent startup monitoring was cancelled.",
            (Self::ZhCn, "cancelled") => "已取消等待智能体启动。",
            (Self::EnUs, _) => {
                "Agent startup could not be confirmed. Inspect the command result and terminal."
            }
            (Self::ZhCn, _) => "无法确认智能体启动成功，请检查命令结果和终端。",
        };
        let reservation = match (self, field("reservationState")) {
            (Self::EnUs, "pending" | "unknown") => {
                " The name reservation may remain until it expires; if `orchbee agent names` lists it as pending, wait for it to expire before reusing the name."
            }
            (Self::ZhCn, "pending" | "unknown") => {
                " 名称预留可能保留至到期；若 `orchbee agent names` 将其列为待定，请等其到期后再复用该名称。"
            }
            (Self::EnUs, "released") => " The name reservation was released.",
            (Self::ZhCn, "released") => " 名称预留已释放。",
            (Self::EnUs, "completed") => " The Agent name was already committed.",
            (Self::ZhCn, "completed") => " 智能体名称已经绑定。",
            _ => "",
        };
        let submitted = failure
            .details
            .as_ref()
            .and_then(|details| details.get("submission"))
            .is_some_and(|value| !value.is_null());
        let delivery = match (self, submitted) {
            (Self::EnUs, true) => {
                " Input was attempted once; the Agent may still be running. No process was terminated."
            }
            (Self::ZhCn, true) => " 启动输入已尝试提交一次，智能体可能仍在运行；没有终止进程。",
            _ => "",
        };
        format!("{message}{reservation}{delivery}\n")
    }

    pub(super) fn cli_agent_start_provider_choices(self) -> &'static str {
        match self {
            Self::EnUs => {
                "one of pi, claude-code, codex, gemini, cursor, devin, antigravity, cline, omp, mastracode, opencode, github-copilot, kimi, kiro, droid, amp, grok, hermes, kilo, qodercli, qwen-code, letta, maki, muse"
            }
            Self::ZhCn => {
                "以下之一：pi、claude-code、codex、gemini、cursor、devin、antigravity、cline、omp、mastracode、opencode、github-copilot、kimi、kiro、droid、amp、grok、hermes、kilo、qodercli、qwen-code、letta、maki、muse"
            }
        }
    }
}
