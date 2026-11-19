//! Localized Agent startup outcomes and provider choices; does not start or retry Agents.

use super::Locale;
use crate::cli_outcome::Failure;
use crate::invocation::AgentStartPlace;
use std::sync::LazyLock;

impl super::CliText {
    pub(crate) fn cli_agent_started(
        self,
        session: &str,
        place: AgentStartPlace,
        panel: &str,
    ) -> String {
        let at = self.place_phrase(place, panel);
        match self.locale {
            Locale::EnUs => format!(
                "The Agent is ready {at}, session {session}.\nNext: orchbee agent prompt --panel {panel} <text>\n"
            ),
            Locale::ZhCn => format!(
                "智能体已在{at}中就绪，会话 {session}。\n下一步：orchbee agent prompt --panel {panel} <text>\n"
            ),
        }
    }

    /// A focus that did not switch the Client to the new Panel. The Agent runs there, so the start
    /// stays a success, and the text names the command that focuses the Panel.
    pub(crate) fn cli_agent_focus_missed(self, panel: &str) -> String {
        match self.locale {
            Locale::EnUs => format!(
                "The Agent runs in the new Panel, but focus did not switch to it. To switch, run: orchbee agent focus --panel {panel}\n"
            ),
            Locale::ZhCn => format!(
                "智能体在新面板中运行，但聚焦未切换到该面板。切换请运行：orchbee agent focus --panel {panel}\n"
            ),
        }
    }

    /// The Agent was seen ready on screen, so the next step addresses its Panel.
    pub(crate) fn cli_agent_observed(self, place: AgentStartPlace, panel: &str) -> String {
        let at = self.place_phrase(place, panel);
        match self.locale {
            Locale::EnUs => format!(
                "The Agent is ready {at}.\nNext: orchbee agent prompt --panel {panel} <text>\n"
            ),
            Locale::ZhCn => format!(
                "智能体已在{at}中就绪。\n下一步：orchbee agent prompt --panel {panel} <text>\n"
            ),
        }
    }

    /// A start without a name typed the command in `panel` and passed the early check, or the
    /// Panel's new shell runs the Agent as its first command; nothing waited for it to become
    /// ready.
    pub(crate) fn cli_agent_sent(
        self,
        product: &str,
        place: AgentStartPlace,
        panel: &str,
    ) -> String {
        let at = self.place_phrase(place, panel);
        match self.locale {
            Locale::EnUs => format!(
                "Started {product} {at}.\nNext: orchbee agent wait --panel {panel} --until idle\n"
            ),
            Locale::ZhCn => format!(
                "已在{at}中启动 {product}。\n下一步：orchbee agent wait --panel {panel} --until idle\n"
            ),
        }
    }

    /// Where a start ran, as its outcome line names it: English reads "in …", Chinese fills
    /// "在…中".
    fn place_phrase(self, place: AgentStartPlace, panel: &str) -> String {
        match (self.locale, place) {
            (Locale::EnUs, AgentStartPlace::NewTab) => format!("in a new Tab (Panel {panel})"),
            (Locale::EnUs, AgentStartPlace::SplitRight) => {
                format!("in a new Panel on the right (Panel {panel})")
            }
            (Locale::EnUs, AgentStartPlace::SplitDown) => {
                format!("in a new Panel below (Panel {panel})")
            }
            (Locale::EnUs, AgentStartPlace::Here) => format!("in Panel {panel}"),
            (Locale::ZhCn, AgentStartPlace::NewTab) => format!("新标签页（面板 {panel}）"),
            (Locale::ZhCn, AgentStartPlace::SplitRight) => format!("右侧新面板（面板 {panel}）"),
            (Locale::ZhCn, AgentStartPlace::SplitDown) => format!("下方新面板（面板 {panel}）"),
            (Locale::ZhCn, AgentStartPlace::Here) => format!("面板 {panel} "),
        }
    }

    /// A failed creation of the new Tab or Panel, or of its shell; nothing was typed.
    fn cli_agent_place_failed(self, failure: &Failure, place: &str, created: bool) -> String {
        let tab = place == "new-tab";
        let reason = failure
            .details
            .as_ref()
            .and_then(|details| details.get("cause"))
            .and_then(|cause| cause["reason"].as_str());
        let unknown = failure.kind == crate::cli_outcome::FailureKind::OutcomeUnknown;
        let message = match (self.locale, tab, created, unknown) {
            (Locale::EnUs, true, false, true) => {
                "Whether the new Tab was created is unknown; inspect `orchbee panel list` before retrying."
            }
            (Locale::EnUs, false, false, true) => {
                "Whether the new Panel was created is unknown; inspect `orchbee panel list` before retrying."
            }
            (Locale::ZhCn, true, false, true) => {
                "无法确认新标签页是否已创建；重试前请运行 `orchbee panel list` 查看。"
            }
            (Locale::ZhCn, false, false, true) => {
                "无法确认新面板是否已创建；重试前请运行 `orchbee panel list` 查看。"
            }
            (Locale::EnUs, true, false, false) => {
                "The new Tab could not be created; nothing was started."
            }
            (Locale::EnUs, false, false, false) => {
                "The new Panel could not be created; nothing was started."
            }
            (Locale::ZhCn, true, false, false) => "无法创建新标签页，未启动任何内容。",
            (Locale::ZhCn, false, false, false) => "无法创建新面板，未启动任何内容。",
            (Locale::EnUs, true, true, _) => {
                "The new Tab's shell did not start; nothing was started."
            }
            (Locale::EnUs, false, true, _) => {
                "The new Panel's shell did not start; nothing was started."
            }
            (Locale::ZhCn, true, true, _) => "新标签页的 shell 未能启动，未启动任何内容。",
            (Locale::ZhCn, false, true, _) => "新面板的 shell 未能启动，未启动任何内容。",
        };
        match (self.locale, reason.filter(|_| !unknown && !created)) {
            (Locale::EnUs, Some(reason)) => format!("{message} Reason: {reason}."),
            (Locale::ZhCn, Some(reason)) => format!("{message}原因：{reason}。"),
            (_, None) => message.to_owned(),
        }
    }

    /// The Panel this invocation created stays when a later step fails.
    fn cli_agent_created_panel(self, place: &str, panel: &str) -> String {
        match (self.locale, place == "new-tab") {
            (Locale::EnUs, true) => format!(" The new Tab stays open as Panel {panel}."),
            (Locale::EnUs, false) => format!(" The new Panel stays open as Panel {panel}."),
            (Locale::ZhCn, true) => format!(" 新标签页保留为面板 {panel}。"),
            (Locale::ZhCn, false) => format!(" 新面板保留为面板 {panel}。"),
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
        let created = match field("createdPanelId") {
            "" => String::new(),
            panel => {
                self.cli_agent_created_panel(field("place"), &crate::cli::handles::panel(panel))
            }
        };
        if field("reason") == "place-failed" {
            let message = self.cli_agent_place_failed(failure, field("place"), !created.is_empty());
            return format!("{message}{created}\n");
        }
        if field("reason") == "startup-failed"
            && let Some(cause) = failure
                .details
                .as_ref()
                .and_then(|details| details.get("cause"))
            && cause["kind"] == "not-found"
        {
            let message = match self.locale {
                Locale::EnUs => {
                    "The target panel was not found. Run `orchbee panel list`, then pass an existing panel ID to --panel."
                }
                Locale::ZhCn => {
                    "未找到目标面板。请运行 `orchbee panel list`，再将现有面板 ID 传给 --panel。"
                }
            };
            return format!("{message}{created}\n");
        }
        let cause = failure
            .details
            .as_ref()
            .and_then(|details| details.get("cause"))
            .filter(|cause| cause["stage"] == "capability")
            .and_then(|cause| cause["reason"].as_str());
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
            (Locale::EnUs, _, "prompt-unconfirmed", "unmarked") if cfg!(windows) => {
                "Nothing was typed: the target terminal has not shown a marked prompt. Wait for the prompt, or use a PowerShell or cmd terminal started by Orchbee."
            }
            (Locale::EnUs, _, "prompt-unconfirmed", "unmarked") => {
                "Nothing was typed: the target terminal has not shown a marked prompt. Wait for the prompt, or use a zsh or bash terminal started by Orchbee."
            }
            (Locale::ZhCn, _, "prompt-unconfirmed", "unmarked") if cfg!(windows) => {
                "未输入任何内容：目标终端还没有出现带标记的提示符。请等提示符出现，或改用 Orchbee 启动的 PowerShell 或 cmd 终端。"
            }
            (Locale::ZhCn, _, "prompt-unconfirmed", "unmarked") => {
                "未输入任何内容：目标终端还没有出现带标记的提示符。请等提示符出现，或改用 Orchbee 启动的 zsh 或 bash 终端。"
            }
            (Locale::EnUs, _, "prompt-unconfirmed", _) => {
                "Nothing was typed: the target shell is not waiting at an empty prompt. Finish the running program, or clear the input line and press Enter for a new prompt, then retry. If PROMPT was changed in that terminal, open a new one."
            }
            (Locale::ZhCn, _, "prompt-unconfirmed", _) => {
                "未输入任何内容：目标 shell 没有停在空的提示符上。请先结束正在运行的程序，或清空输入行后按回车获得新的提示符，然后重试。如果在那个终端里改过 PROMPT，请新开一个终端。"
            }
            (locale, _, reason, _) => self.cli_agent_start_reason(locale, reason),
        };
        let submitted = failure
            .details
            .as_ref()
            .and_then(|details| details.get("submission"))
            .is_some_and(|value| !value.is_null());
        // A start that submitted its line may have left the Agent running, so its failure says
        // so whether or not the Agent was named.
        let delivery = match (self.locale, submitted && !nothing_typed) {
            (Locale::EnUs, true) => {
                " Input was attempted once; the Agent may still be running. No process was terminated."
            }
            (Locale::ZhCn, true) => " 启动输入已尝试提交一次，智能体可能仍在运行；没有终止进程。",
            _ => "",
        };
        format!("{message}{delivery}{created}\n")
    }

    fn cli_agent_start_reason(self, locale: Locale, reason: &str) -> &'static str {
        match (locale, reason) {
            (Locale::EnUs, "terminal-not-idle") if cfg!(windows) => {
                "The target must be an idle cmd, PowerShell or pwsh terminal with no other running command."
            }
            (Locale::EnUs, "terminal-not-idle") => {
                "The target must be an idle zsh or bash terminal with no other running command."
            }
            (Locale::ZhCn, "terminal-not-idle") if cfg!(windows) => {
                "目标必须是空闲的 cmd、PowerShell 或 pwsh 终端，且没有其他命令正在运行。"
            }
            (Locale::ZhCn, "terminal-not-idle") => {
                "目标必须是空闲的 zsh 或 bash 终端，且没有其他命令正在运行。"
            }
            (Locale::EnUs, "name-in-use" | "presentation-busy") => {
                "The Agent name or terminal is already in use. Inspect `orchbee agent list`."
            }
            (Locale::ZhCn, "name-in-use" | "presentation-busy") => {
                "智能体名称或终端已被占用。请用 `orchbee agent list` 查看。"
            }
            (Locale::EnUs, "own-panel") => {
                "--here cannot type into the Panel this command runs in. Use --new-tab, or pass --panel <id> for another Panel."
            }
            (Locale::ZhCn, "own-panel") => {
                "--here 不能在运行本命令的面板中输入。请改用 --new-tab，或用 --panel <id> 指定另一个面板。"
            }
            (Locale::EnUs, "command-too-long") if cfg!(windows) => {
                "The encoded Windows launch command exceeds 8000 characters. Reduce the arguments."
            }
            (Locale::EnUs, "command-too-long") => {
                "The launch command exceeds 8000 characters. Reduce the arguments."
            }
            (Locale::ZhCn, "command-too-long") if cfg!(windows) => {
                "编码后的 Windows 启动命令超过 8000 个字符，请缩短参数。"
            }
            (Locale::ZhCn, "command-too-long") => "启动命令超过 8000 个字符，请缩短参数。",
            (Locale::EnUs, "start-deadline") => {
                "Agent startup did not finish before the deadline; its command may still be running. Inspect its terminal."
            }
            (Locale::ZhCn, "start-deadline") => {
                "智能体启动未在时限内完成，其命令可能仍在运行。请检查终端输出。"
            }
            (Locale::EnUs, "agent-blocked") => {
                "The Agent needs attention. Resolve the prompt shown in its terminal."
            }
            (Locale::ZhCn, "agent-blocked") => "智能体需要人工处理，请查看其终端中的提示。",
            (Locale::EnUs, "agent-exited") => {
                "The Agent exited during startup. Inspect its terminal output."
            }
            (Locale::ZhCn, "agent-exited") => "智能体在启动期间退出，请检查其终端输出。",
            (Locale::EnUs, "command-not-found") => {
                "Command not found: the shell came back to its prompt and no Agent process appeared. Inspect its terminal output."
            }
            (Locale::ZhCn, "command-not-found") => {
                "找不到命令：shell 已回到提示符，且没有出现智能体进程。请检查其终端输出。"
            }
            (Locale::EnUs, "launch-command-failed") if cfg!(windows) => {
                "The launch command failed in PowerShell before the Agent started, for example because the provider was not found. Inspect its terminal output."
            }
            (Locale::EnUs, "launch-command-failed") => {
                "The launch command failed in the shell before the Agent started, for example because the provider was not found. Inspect its terminal output."
            }
            (Locale::ZhCn, "launch-command-failed") if cfg!(windows) => {
                "启动命令在 PowerShell 中执行失败，智能体未能启动，例如找不到提供方程序。请检查其终端输出。"
            }
            (Locale::ZhCn, "launch-command-failed") => {
                "启动命令在 shell 中执行失败，智能体未能启动，例如找不到提供方程序。请检查其终端输出。"
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
                "The startup operation's result is unknown. Inspect the Agent list and the terminal before retrying."
            }
            (Locale::ZhCn, "reply-not-confirmed") => {
                "启动操作的结果尚不确定。重试前请检查智能体列表和终端。"
            }
            (Locale::EnUs, "cancelled") => "Agent startup monitoring was cancelled.",
            (Locale::ZhCn, "cancelled") => "已取消等待智能体启动。",
            (Locale::EnUs, _) => {
                "Agent startup could not be confirmed. Inspect the command result and terminal."
            }
            (Locale::ZhCn, _) => "无法确认智能体启动成功，请检查命令结果和终端。",
        }
    }

    /// Every provider id a start takes, each beside the product name it stands for.
    pub(super) fn cli_agent_start_provider_choices(self) -> &'static str {
        static EN_US: LazyLock<String> =
            LazyLock::new(|| format!("one of {}", provider_list("{id} ({product})", ", ")));
        static ZH_CN: LazyLock<String> =
            LazyLock::new(|| format!("以下之一：{}", provider_list("{id}（{product}）", "、")));
        match self.locale {
            Locale::EnUs => EN_US.as_str(),
            Locale::ZhCn => ZH_CN.as_str(),
        }
    }
}

fn provider_list(entry: &str, separator: &str) -> String {
    crate::agent_provider::choices()
        .map(|choice| {
            entry
                .replace("{id}", choice.id)
                .replace("{product}", choice.product)
        })
        .collect::<Vec<_>>()
        .join(separator)
}
