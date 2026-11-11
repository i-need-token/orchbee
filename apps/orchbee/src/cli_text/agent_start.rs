//! Localized Agent startup outcomes and provider choices; does not start or retry Agents.

use super::Locale;
use crate::cli_outcome::Failure;

impl super::CliText {
    pub(crate) fn cli_agent_started(self, name: &str, session: &str) -> String {
        match self.locale {
            Locale::EnUs => format!(
                "Agent {name} is ready ({session}).\nNext: orchbee agent prompt --name {name} <text>\n"
            ),
            Locale::ZhCn => format!(
                "智能体 {name} 已就绪（{session}）。\n下一步：orchbee agent prompt --name {name} <text>\n"
            ),
        }
    }

    /// The Agent was seen ready without an integration's Session, so `name` was not bound and the
    /// next step addresses its Panel.
    pub(crate) fn cli_agent_observed(self, name: &str, panel: &str) -> String {
        match self.locale {
            Locale::EnUs => format!(
                "The Agent is ready in Panel {panel}. No integration reported its session, so the name {name} was not given to it.\nNext: orchbee agent prompt --panel {panel} <text>\n"
            ),
            Locale::ZhCn => format!(
                "智能体已在面板 {panel} 中就绪。没有集成报告它的会话，因此未把名称 {name} 分配给它。\n下一步：orchbee agent prompt --panel {panel} <text>\n"
            ),
        }
    }

    pub(super) fn cli_agent_start_failure(self, failure: &Failure) -> String {
        if matches!(
            failure.usage.as_deref(),
            Some(crate::invocation::UsageReason::CallerContextUnavailable)
        ) {
            return match self.locale {
                Locale::EnUs => "Calling terminal context is missing or invalid. Run inside an Orchbee terminal, or pass --panel <id> to agent start. Inspect `orchbee agent start --help`.\n",
                Locale::ZhCn => "调用终端上下文缺失或无效。请在 Orchbee 终端内运行，或为 agent start 传入 --panel <id>。查看 `orchbee agent start --help`。\n",
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
            return match self.locale {
                Locale::EnUs => "The target panel was not found. Run `orchbee panel list`, then pass an existing panel ID to --panel.\n",
                Locale::ZhCn => "未找到目标面板。请运行 `orchbee panel list`，再将现有面板 ID 传给 --panel。\n",
            }.to_owned();
        }
        let cause = failure
            .details
            .as_ref()
            .and_then(|details| details.get("cause"))
            .filter(|cause| cause["stage"] == "capability")
            .and_then(|cause| cause["reason"].as_str());
        // A prompt refusal, including the Host's own busy refusal, and a missing capability
        // typed nothing; the attempted-input suffix below would contradict that.
        let nothing_typed = matches!(field("reason"), "prompt-unconfirmed" | "terminal-not-idle")
            || cause.is_some();
        let message = match (self.locale, cause, field("reason"), field("promptState")) {
            (Locale::EnUs, Some("session-too-old"), ..) => {
                "Nothing was typed: the Session is too old to confirm the prompt. Update Orchbee on the Session's machine and restart the Session."
            }
            (Locale::ZhCn, Some("session-too-old"), ..) => {
                "未输入任何内容：会话版本太旧，无法确认提示符。请更新会话所在机器上的 Orchbee，并重启会话。"
            }
            (Locale::EnUs, Some("terminal-too-old"), ..) => {
                "Nothing was typed: the target terminal was started by an older Orchbee. Open a new terminal."
            }
            (Locale::ZhCn, Some("terminal-too-old"), ..) => {
                "未输入任何内容：目标终端由旧版 Orchbee 启动。请新开一个终端。"
            }
            (Locale::EnUs, _, "prompt-unconfirmed", "unmarked") => {
                "Nothing was typed: the target terminal has not shown a marked prompt. Wait for the prompt, or use a PowerShell or cmd terminal started by Orchbee."
            }
            (Locale::ZhCn, _, "prompt-unconfirmed", "unmarked") => {
                "未输入任何内容：目标终端还没有出现带标记的提示符。请等提示符出现，或改用 Orchbee 启动的 PowerShell 或 cmd 终端。"
            }
            (Locale::EnUs, _, "prompt-unconfirmed", _) => {
                "Nothing was typed: the target shell is not waiting at an empty prompt. Finish the running program, or clear the input line and press Enter for a new prompt, then retry. If PROMPT was changed in that terminal, open a new one."
            }
            (Locale::ZhCn, _, "prompt-unconfirmed", _) => {
                "未输入任何内容：目标 shell 没有停在空的提示符上。请先结束正在运行的程序，或清空输入行后按回车获得新的提示符，然后重试。如果在那个终端里改过 PROMPT，请新开一个终端。"
            }
            (locale, _, reason, _) => self.cli_agent_start_reason(locale, reason),
        };
        let reservation = match (self.locale, field("reservationState")) {
            (Locale::EnUs, "pending" | "unknown") => {
                " The name reservation may remain until it expires; if `orchbee agent names` lists it as pending, wait for it to expire before reusing the name."
            }
            (Locale::ZhCn, "pending" | "unknown") => {
                " 名称预留可能保留至到期；若 `orchbee agent names` 将其列为待定，请等其到期后再复用该名称。"
            }
            (Locale::EnUs, "released") => " The name reservation was released.",
            (Locale::ZhCn, "released") => " 名称预留已释放。",
            (Locale::EnUs, "completed") => " The Agent name was already committed.",
            (Locale::ZhCn, "completed") => " 智能体名称已经绑定。",
            _ => "",
        };
        let submitted = failure
            .details
            .as_ref()
            .and_then(|details| details.get("submission"))
            .is_some_and(|value| !value.is_null());
        let delivery = match (self.locale, submitted && !nothing_typed) {
            (Locale::EnUs, true) => {
                " Input was attempted once; the Agent may still be running. No process was terminated."
            }
            (Locale::ZhCn, true) => " 启动输入已尝试提交一次，智能体可能仍在运行；没有终止进程。",
            _ => "",
        };
        format!("{message}{reservation}{delivery}\n")
    }

    fn cli_agent_start_reason(self, locale: Locale, reason: &str) -> &'static str {
        match (locale, reason) {
            (Locale::EnUs, "terminal-not-idle") => {
                "The target must be an idle cmd, PowerShell or pwsh terminal with no other running command."
            }
            (Locale::ZhCn, "terminal-not-idle") => {
                "目标必须是空闲的 cmd、PowerShell 或 pwsh 终端，且没有其他命令正在运行。"
            }
            (Locale::EnUs, "name-in-use" | "presentation-busy") => {
                "The Agent name or terminal is already in use. Inspect `orchbee agent names`."
            }
            (Locale::ZhCn, "name-in-use" | "presentation-busy") => {
                "智能体名称或终端已被占用。请用 `orchbee agent names` 查看。"
            }
            (Locale::EnUs, "command-too-long") => {
                "The encoded Windows launch command exceeds 8000 characters. Reduce the arguments."
            }
            (Locale::ZhCn, "command-too-long") => {
                "编码后的 Windows 启动命令超过 8000 个字符，请缩短参数。"
            }
            (Locale::EnUs, "start-deadline") => {
                "The Agent did not confirm readiness before the deadline; its command may still be running. Inspect its terminal."
            }
            (Locale::ZhCn, "start-deadline") => {
                "智能体未在时限内确认就绪，其命令可能仍在运行。请检查终端输出。"
            }
            (Locale::EnUs, "agent-blocked") => {
                "The Agent needs attention. Resolve the prompt shown in its terminal."
            }
            (Locale::ZhCn, "agent-blocked") => "智能体需要人工处理，请查看其终端中的提示。",
            (Locale::EnUs, "agent-exited") => {
                "The Agent exited before readiness was confirmed. Inspect its terminal output."
            }
            (Locale::ZhCn, "agent-exited") => "智能体在确认就绪前已退出，请检查其终端输出。",
            (Locale::EnUs, "launch-command-failed") => {
                "The launch command failed in PowerShell before the Agent started, for example because the provider was not found. Inspect its terminal output."
            }
            (Locale::ZhCn, "launch-command-failed") => {
                "启动命令在 PowerShell 中执行失败，智能体未能启动，例如找不到提供方程序。请检查其终端输出。"
            }
            (Locale::EnUs, "agent-provider-mismatch" | "agent-runtime-ambiguous") => {
                "The running Agent does not uniquely match the requested provider."
            }
            (Locale::ZhCn, "agent-provider-mismatch" | "agent-runtime-ambiguous") => {
                "运行中的智能体无法唯一匹配所请求的提供方。"
            }
            (Locale::EnUs, "binding-changed") => "The target terminal changed during startup.",
            (Locale::ZhCn, "binding-changed") => "目标终端在启动期间发生了变化。",
            (Locale::EnUs, "reply-not-confirmed") => {
                "The startup operation's result is unknown. Inspect Agent names and the terminal before retrying."
            }
            (Locale::ZhCn, "reply-not-confirmed") => {
                "启动操作的结果尚不确定。重试前请检查智能体名称和终端。"
            }
            (Locale::EnUs, "cancelled") => "Agent startup monitoring was cancelled.",
            (Locale::ZhCn, "cancelled") => "已取消等待智能体启动。",
            (Locale::EnUs, _) => {
                "Agent startup could not be confirmed. Inspect the command result and terminal."
            }
            (Locale::ZhCn, _) => "无法确认智能体启动成功，请检查命令结果和终端。",
        }
    }

    pub(super) fn cli_agent_start_provider_choices(self) -> &'static str {
        match self.locale {
            Locale::EnUs => {
                "one of pi, claude-code, codex, gemini, cursor, devin, antigravity, cline, omp, mastracode, opencode, github-copilot, kimi, kiro, droid, amp, grok, hermes, kilo, qodercli, qwen-code, letta, maki, muse, atomcode, codebuddy, command-code, deveco, mimo, reasonix, snow, minimax-code, mothx, kirocrew, qwenpaw, jcode, openclaw, penguin, prime-agent"
            }
            Locale::ZhCn => {
                "以下之一：pi、claude-code、codex、gemini、cursor、devin、antigravity、cline、omp、mastracode、opencode、github-copilot、kimi、kiro、droid、amp、grok、hermes、kilo、qodercli、qwen-code、letta、maki、muse、atomcode、codebuddy、command-code、deveco、mimo、reasonix、snow、minimax-code、mothx、kirocrew、qwenpaw、jcode、openclaw、penguin、prime-agent"
            }
        }
    }
}
