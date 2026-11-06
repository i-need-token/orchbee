//! Localized CLI failure routing and shared diagnostics; does not recover or retry operations.

use super::{Locale, failure_target};
use crate::cli_outcome::Failure;

impl super::CliText {
    /// Localized human diagnostic for one CLI failure. User-controlled data is
    /// escaped before it reaches the terminal.
    pub(crate) fn cli_failure(self, failure: &Failure) -> String {
        use crate::cli_outcome::FailureKind;
        if failure.failure_schema.is_none()
            && failure
                .details
                .as_ref()
                .and_then(|details| details.get("stage"))
                .and_then(|stage| stage.as_str())
                == Some("plugin")
        {
            let reason = failure
                .details
                .as_ref()
                .and_then(|details| details.get("reason"))
                .and_then(|reason| reason.as_str())
                .unwrap_or("");
            if reason == "client-refused" {
                let client = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("clientReason"))
                    .and_then(|reason| reason.as_str())
                    .unwrap_or("");
                return self.cli_plugin_client_refused(client);
            }
            return self.cli_plugin_preparation_failure(reason).to_owned();
        }
        if failure.failure_schema == Some("orchbee.cli.failure.layout-apply")
            && matches!(
                failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("reason"))
                    .and_then(serde_json::Value::as_str),
                Some("invalid-layout" | "layout-version-1-required")
            )
        {
            return match self.locale {
                Locale::EnUs => "Invalid layout input; expected the current orchbee.layout.export version 1 shape.\n".to_owned(),
                Locale::ZhCn => "布局输入无效；需要当前 orchbee.layout.export 版本 1 的结构。\n".to_owned(),
            };
        }
        if failure.failure_schema == Some("orchbee.cli.failure.workbench-query") {
            let reason = failure
                .details
                .as_ref()
                .and_then(|details| details.get("reason"))
                .and_then(|value| value.as_str());
            return self.cli_workbench_query_failure(reason).to_owned();
        }
        if failure.failure_schema == Some("orchbee.cli.failure.client-query") {
            return self.cli_client_query_failure(failure.kind).to_owned();
        }
        if failure.failure_schema == Some("orchbee.cli.failure.client-control")
            && let Some(message) = failure
                .details
                .as_ref()
                .and_then(|details| details.get("reason"))
                .and_then(|reason| reason.as_str())
                .and_then(|reason| self.cli_client_selection_failure(reason))
        {
            return message.to_owned();
        }
        if failure.failure_schema == Some("orchbee.cli.failure.configuration") {
            let field = failure
                .details
                .as_ref()
                .and_then(|details| details.get("field"))
                .and_then(|v| v.as_str())
                .unwrap_or("file");
            let reason = failure
                .details
                .as_ref()
                .and_then(|details| details.get("reason"))
                .and_then(|v| v.as_str())
                .unwrap_or("io-error");
            if (field, reason) == ("APPDATA", "missing") {
                return match self.locale {
                    Locale::EnUs => "APPDATA is not set, so the local Session has no configuration location. Set APPDATA or ORCHBEE_CONFIG_FILE to an absolute path and retry.\n",
                    Locale::ZhCn => "未设置 APPDATA，本地会话没有配置位置。请将 APPDATA 或 ORCHBEE_CONFIG_FILE 设为绝对路径后重试。\n",
                }
                .to_owned();
            }
            if reason.starts_with("source-query-") {
                return match self.locale {
                    Locale::EnUs => format!(
                        "Could not resolve the configured checkout directory ({reason}). Inspect the source with orchbee worktree list.\n"
                    ),
                    Locale::ZhCn => format!(
                        "无法解析配置的检出目录（{reason}）。请使用 orchbee worktree list 检查来源。\n"
                    ),
                };
            }
            return match self.locale {
                Locale::EnUs => format!(
                    "Configuration preparation failed ({field}: {reason}). Check the file with orchbee config check.\n"
                ),
                Locale::ZhCn => format!(
                    "配置准备失败（{field}: {reason}）。请使用 orchbee config check 检查文件。\n"
                ),
            };
        }
        if failure.failure_schema.is_none()
            && failure
                .details
                .as_ref()
                .and_then(|details| details.get("stage"))
                .and_then(|stage| stage.as_str())
                == Some("workspace-creation")
        {
            return self
                .cli_workspace_preparation_failure(failure.kind)
                .to_owned();
        }
        if failure.failure_schema == Some("orchbee.cli.failure.agent-start") {
            return self.cli_agent_start_failure(failure);
        }
        // `server start` failures carry the local-runtime source; the shared
        // query/send wording for these kinds would misstate what a start
        // actually did, so each gets a dedicated start diagnostic.
        let local_runtime = failure
            .source
            .as_ref()
            .and_then(|source| source.get("kind"))
            .and_then(|value| value.as_str())
            == Some("local-runtime");
        // The dedicated `worktree list` failure envelope owns its own
        // wording; the schema marker is the unambiguous discriminator.
        if failure.failure_schema == Some(crate::invocation::grammar::schemas::WORKTREE_FAILURE_ID)
        {
            return self.cli_worktree_failure(failure);
        }
        // Agent read and cancelled failures answer under the dedicated Agent
        // envelope; the schema marker selects their wording the same way.
        if failure.failure_schema == Some(crate::invocation::grammar::schemas::AGENT_FAILURE_ID) {
            return self.cli_agent_failure(failure);
        }
        if failure.failure_schema == Some("orchbee.cli.failure.agent-projection") {
            return self.cli_projection_failure(failure);
        }
        // Launch-profile catalog reads and profile resolutions answer under
        // their dedicated envelope the same way.
        if failure.failure_schema
            == Some(crate::invocation::grammar::schemas::LAUNCH_PROFILE_FAILURE_ID)
        {
            return self.cli_launch_profile_failure(failure);
        }
        if failure.failure_schema == Some("orchbee.cli.failure.agent-names") {
            if failure.kind == FailureKind::NotFound {
                return match self.locale {
                    Locale::EnUs => "No Agent session or name matches the target.\n",
                    Locale::ZhCn => "没有与目标匹配的智能体会话或名称。\n",
                }
                .to_owned();
            }
            if failure
                .details
                .as_ref()
                .and_then(|details| details.get("reason"))
                .and_then(|reason| reason.as_str())
                == Some("launch-pending")
            {
                return match self.locale {
                    Locale::EnUs => "This Agent name or terminal is reserved by a pending launch. Use `orchbee agent names` to inspect its remaining time.\n",
                    Locale::ZhCn => "此智能体名称或终端正被启动预留占用。可用 `orchbee agent names` 查看剩余时间。\n",
                }.to_owned();
            }
        }
        match failure.kind {
            FailureKind::Timeout if local_runtime => match self.locale {
                Locale::EnUs if cfg!(windows) => "Local startup reached its time limit before readiness was confirmed. Run orchbee server diagnose with the same --session selection before retrying.\n".to_owned(),
                Locale::ZhCn if cfg!(windows) => "本地启动达到时限，尚未确认就绪。请用相同 --session 选择运行 orchbee server diagnose，再决定是否重试。\n".to_owned(),
                Locale::EnUs => {
                    "The local session did not become ready within the time limit; a Server may still be starting, or activation state may have been left behind. No recovery was attempted.\n"
                        .to_owned()
                }
                Locale::ZhCn => {
                    "本地会话未在时限内就绪：Server 可能仍在启动，也可能遗留了激活状态。未尝试任何恢复。\n"
                        .to_owned()
                }
            },
            FailureKind::Rejected if local_runtime => {
                let reason = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("reason"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                match self.locale {
                    Locale::EnUs if cfg!(windows) && reason == "recovery-required" => self.locale.startup_failure(crate::local_startup::StartupFailure::RecoveryRequired).to_owned(),
                    Locale::ZhCn if cfg!(windows) && reason == "recovery-required" => self.locale.startup_failure(crate::local_startup::StartupFailure::RecoveryRequired).to_owned(),
                    Locale::EnUs => {
                        format!("The local session refused the start ({reason}); its activation state requires recovery and no repair was attempted.\n")
                    }
                    Locale::ZhCn => {
                        format!("本地会话拒绝了启动（{reason}）：其激活状态需要恢复，未尝试任何修复。\n")
                    }
                }
            }
            FailureKind::Denied if local_runtime => match self.locale {
                Locale::EnUs => {
                    "The local session refused the start; the native development cohort is not permitted.\n"
                        .to_owned()
                }
                Locale::ZhCn => "本地会话拒绝了启动：原生开发 cohort 未获许可。\n".to_owned(),
            },
            FailureKind::ServiceUnavailable if local_runtime => {
                self.cli_server_unavailable(failure)
            }
            FailureKind::ConnectionCapacity if local_runtime => match self.locale {
                Locale::EnUs => {
                    "The local session has no Client Connection capacity for the start probe.\n"
                        .to_owned()
                }
                Locale::ZhCn => "本地会话没有可接纳启动探针的客户端连接 容量。\n".to_owned(),
            },
            FailureKind::TransportLoss if local_runtime => match self.locale {
                Locale::EnUs => {
                    "The connection to the local session was lost during the start admission.\n"
                        .to_owned()
                }
                Locale::ZhCn => "与本地会话的连接在启动准入期间中断。\n".to_owned(),
            },
            FailureKind::InvalidUsage => self.cli_usage(failure.usage.as_deref()),
            FailureKind::InvalidIdentifier => {
                let resource = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("resource"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("project");
                let target = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("target"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                self.cli_invalid_identifier(resource, target)
            }
            FailureKind::NotFound => {
                let (resource, target) = failure_target(failure);
                self.cli_not_found(resource, &target)
            }
            FailureKind::Ambiguous => {
                let (resource, target) = failure_target(failure);
                self.cli_ambiguous(resource, &target, failure)
            }
            FailureKind::Denied => {
                let command = failure
                    .source
                    .as_ref()
                    .and_then(|source| source.get("kind"))
                    .and_then(|value| value.as_str())
                    == Some("workbench-command");
                match (self.locale, command) {
                    (Locale::EnUs, true) => {
                        "The local session denied the command; nothing was applied.\n".to_owned()
                    }
                    (Locale::EnUs, false) => {
                        "The local session denied the read request.\n".to_owned()
                    }
                    (Locale::ZhCn, true) => {
                        "本地会话拒绝了该命令；未应用任何修改。\n".to_owned()
                    }
                    (Locale::ZhCn, false) => "本地会话拒绝了读取请求。\n".to_owned(),
                }
            }
            FailureKind::Rejected => {
                let reason = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("reason"))
                    .and_then(|value| value.as_str())
                    .filter(|reason| !reason.is_empty());
                // A refusal without a stable reason omits the parenthetical instead of printing `()`.
                match (self.locale, reason) {
                    (_, Some("launch-request-too-large")) => {
                        format!("{}\n", self.locale.launch_request_too_large())
                    }
                    (Locale::EnUs, Some(reason)) => {
                        format!("The local session rejected the command ({reason}); nothing was applied.\n")
                    }
                    (Locale::EnUs, None) => {
                        "The local session rejected the command; nothing was applied.\n".to_owned()
                    }
                    (Locale::ZhCn, Some(reason)) => {
                        format!("本地会话拒绝了该命令（{reason}）；未应用任何修改。\n")
                    }
                    (Locale::ZhCn, None) => "本地会话拒绝了该命令；未应用任何修改。\n".to_owned(),
                }
            }
            FailureKind::OutcomeUnknown => match self.locale {
                Locale::EnUs => {
                    "The command's outcome is unknown: the request may or may not have been applied.\nIt was not retried — inspect the Workbench state before deciding whether to repeat it.\n".to_owned()
                }
                Locale::ZhCn => {
                    "命令结果未知：请求可能已应用，也可能未应用。\n该命令未被重试——请先检查 Workbench 状态，再决定是否重复执行。\n".to_owned()
                }
            },
            FailureKind::ServiceUnavailable => self.cli_service_unavailable(failure),
            FailureKind::ConnectionCapacity => match self.locale {
                Locale::EnUs => {
                    "The local session has no Client Connection capacity for this query.\n".to_owned()
                }
                Locale::ZhCn => "本地会话没有可接纳本次查询的客户端连接 容量。\n".to_owned(),
            },
            FailureKind::TransportLoss => {
                let stage = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("stage"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                match (self.locale, stage) {
                    (Locale::EnUs, "command") => {
                        "The connection to the local session failed before the command could be submitted; nothing was sent.\n".to_owned()
                    }
                    (Locale::ZhCn, "command") => {
                        "与本地会话的连接在命令提交前失败；未发送任何内容。\n".to_owned()
                    }
                    (Locale::EnUs, "viewer") => {
                        "The connection to the local session was lost during the terminal read.\n"
                            .to_owned()
                    }
                    (Locale::ZhCn, "viewer") => "与本地会话的连接在终端读取期间中断。\n".to_owned(),
                    (Locale::EnUs, _) => {
                        "The connection to the local session was lost before a snapshot arrived.\n"
                            .to_owned()
                    }
                    (Locale::ZhCn, _) => "与本地会话的连接在快照返回前中断。\n".to_owned(),
                }
            }
            FailureKind::ProtocolInvalid => match self.locale {
                Locale::EnUs => {
                    "The local session answered with an invalid protocol response.\n".to_owned()
                }
                Locale::ZhCn => "本地会话返回了无效的协议响应。\n".to_owned(),
            },
            FailureKind::Incompatible => match self.locale {
                Locale::EnUs => {
                    "The running local session is incompatible with this development cohort.\n"
                        .to_owned()
                }
                Locale::ZhCn => "正在运行的本地会话与当前开发 cohort 不兼容。\n".to_owned(),
            },
            FailureKind::CheckFailed => self.cli_check_failed(failure),
            FailureKind::Timeout => {
                // `stage:"input"` is the `terminal send` source stage: no
                // wait ran and no text was awaited — the diagnosis must say
                // what actually happened. Every other producer stays on the
                // truthful wait wording.
                let input_stage = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("stage"))
                    .and_then(|value| value.as_str())
                    == Some("input");
                match (self.locale, input_stage) {
                    (Locale::EnUs, true) => {
                        "The declared input could not be read within the time limit; nothing was sent.\n"
                            .to_owned()
                    }
                    (Locale::ZhCn, true) => "声明的输入未能在时限内读取；未发送任何内容。\n".to_owned(),
                    (Locale::EnUs, false) => {
                        "The wait deadline elapsed before the text appeared in the terminal.\n"
                            .to_owned()
                    }
                    (Locale::ZhCn, false) => "等待期限内终端中未出现目标文本。\n".to_owned(),
                }
            }
            FailureKind::Cancelled => {
                let input_stage = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("stage"))
                    .and_then(|value| value.as_str())
                    == Some("input");
                match (self.locale, input_stage) {
                    (Locale::EnUs, true) => {
                        "Reading the declared input was cancelled; nothing was sent.\n".to_owned()
                    }
                    (Locale::ZhCn, true) => "声明输入的读取已被取消；未发送任何内容。\n".to_owned(),
                    (Locale::EnUs, false) => {
                        "The wait was cancelled; no match is reported.\n".to_owned()
                    }
                    (Locale::ZhCn, false) => "等待已被取消；未报告任何匹配。\n".to_owned(),
                }
            }
            FailureKind::OutputTooLarge => match self.locale {
                Locale::EnUs => {
                    "The result exceeded the 8 MiB output bound; nothing was written.\n".to_owned()
                }
                Locale::ZhCn => "结果超过 8 MiB 输出上限；未写出任何内容。\n".to_owned(),
            },
            FailureKind::InternalFailure => match self.locale {
                Locale::EnUs => "The CLI hit an internal failure; no data was written.\n".to_owned(),
                Locale::ZhCn => "CLI 发生内部错误；未写出任何数据。\n".to_owned(),
            },
        }
    }

    fn cli_invalid_identifier(self, resource: &str, target: &str) -> String {
        let escaped = crate::cli_text::escape::quoted(target);
        match self.locale {
            Locale::EnUs => {
                format!("Invalid {resource} identifier {escaped}.\n")
            }
            Locale::ZhCn => format!("无效的 {resource} 标识符 {escaped}。\n"),
        }
    }

    fn cli_not_found(self, resource: &str, target: &str) -> String {
        let escaped = crate::cli_text::escape::quoted(target);
        match self.locale {
            Locale::EnUs => format!("No {resource} matches {escaped}.\n"),
            Locale::ZhCn => format!("没有与 {escaped} 匹配的 {resource}。\n"),
        }
    }

    /// The bounded candidate list and the true total: list handles with
    /// labels when the resolver supplied them, otherwise the JSON identifiers.
    fn cli_ambiguous(self, resource: &str, target: &str, failure: &Failure) -> String {
        let escaped = crate::cli_text::escape::quoted(target);
        let details = failure.details.as_ref();
        let listed: Vec<String> = match &failure.candidate_display {
            Some(display) => display.lines.clone(),
            None => details
                .and_then(|details| details.get("candidates"))
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
        };
        let total = details
            .and_then(|details| details.get("totalCount"))
            .and_then(serde_json::Value::as_u64)
            .map_or(listed.len(), |total| {
                usize::try_from(total).unwrap_or(usize::MAX)
            });
        if listed.is_empty() {
            return match self.locale {
                Locale::EnUs => {
                    format!("More than one {resource} matches {escaped}; use its exact ID.\n")
                }
                Locale::ZhCn => format!("多个 {resource} 与 {escaped} 匹配；请使用精确 ID。\n"),
            };
        }
        let more = if total > listed.len() { ", …" } else { "" };
        let list = listed.join(", ");
        match self.locale {
            Locale::EnUs => format!(
                "{total} {resource} candidates match {escaped}: {list}{more}. Use one of these handles or the full ID.\n"
            ),
            Locale::ZhCn => format!(
                "共有 {total} 个 {resource} 与 {escaped} 匹配：{list}{more}。请改用其中一个句柄或完整 ID。\n"
            ),
        }
    }

    fn cli_service_unavailable(self, failure: &Failure) -> String {
        self.cli_service_unavailable_for_selection(
            failure,
            crate::local_activation::selected_session_name(),
        )
    }

    pub(crate) fn cli_service_unavailable_for_selection(
        self,
        failure: &Failure,
        selected_session: Option<&str>,
    ) -> String {
        let stage = failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let reason = failure
            .details
            .as_ref()
            .and_then(|details| details.get("reason"))
            .and_then(|value| value.as_str());
        match (self.locale, stage) {
            // A known directory cause is an input to check, not a transient Session fault.
            (Locale::EnUs, "command") if reason == Some("directory-unavailable") => {
                "The working directory is unavailable: it may not exist or may not be accessible. Check the path; nothing was applied.\n"
                    .to_owned()
            }
            (Locale::ZhCn, "command") if reason == Some("directory-unavailable") => {
                "工作目录不可用：目录可能不存在或无法访问。请检查路径；未应用任何修改。\n".to_owned()
            }
            (Locale::EnUs, "platform") => {
                "This command is unavailable on this platform.\n".to_owned()
            }
            (Locale::ZhCn, "platform") => "当前平台尚不支持此命令。\n".to_owned(),
            (Locale::EnUs, "discovery") | (Locale::ZhCn, "discovery") => {
                self.cli_missing_session(selected_session)
            }
            (Locale::EnUs, "snapshot") => {
                "The local session could not produce a snapshot right now.\n".to_owned()
            }
            (Locale::ZhCn, "snapshot") => "本地会话当前无法产出快照。\n".to_owned(),
            (_, "viewer") => self.cli_viewer_unavailable(reason).to_owned(),
            (Locale::EnUs, "command") => {
                "The local session could not execute the command right now; nothing was applied.\n"
                    .to_owned()
            }
            (Locale::ZhCn, "command") => "本地会话当前无法执行该命令；未应用任何修改。\n".to_owned(),
            (Locale::EnUs, _) => {
                "The local session could not admit this query connection.\n".to_owned()
            }
            (Locale::ZhCn, _) => "本地会话无法接纳本次查询连接。\n".to_owned(),
        }
    }

    /// Viewer-stage unavailability: the typed Host reason stays in the JSON
    /// details; text names the cause in words and one next step.
    fn cli_viewer_unavailable(self, reason: Option<&str>) -> &'static str {
        match (self.locale, reason) {
            (Locale::EnUs, None) => {
                "The terminal could not be observed for reading right now. Retry the command shortly.\n"
            }
            (Locale::ZhCn, None) => "该终端当前无法进行读取观察。请稍后重试该命令。\n",
            (Locale::EnUs, Some("stale-revision" | "stale-generation")) => {
                "The terminal output kept changing, so no consistent copy could be read before the deadline. Retry the read once the output settles.\n"
            }
            (Locale::ZhCn, Some("stale-revision" | "stale-generation")) => {
                "终端输出持续变化，截止时间前未能读取一致的内容。请在输出稳定后重试读取。\n"
            }
            (Locale::EnUs, Some("history-evicted")) => {
                "The requested terminal history was discarded while it was being read. Retry the read with fewer lines.\n"
            }
            (Locale::ZhCn, Some("history-evicted")) => {
                "所请求的终端历史在读取过程中已被丢弃。请减少行数后重试读取。\n"
            }
            (Locale::EnUs, Some(_)) => {
                "The terminal history could not be read right now. Retry the read shortly.\n"
            }
            (Locale::ZhCn, Some(_)) => "该终端的历史当前无法读取。请稍后重试读取。\n",
        }
    }

    pub(crate) fn cli_missing_session(self, selected: Option<&str>) -> String {
        match (self.locale, selected) {
            (Locale::EnUs, Some(name)) => format!(
                "No running local session named `{name}` was found. Start it with: orchbee --session {name} server start.\n"
            ),
            (Locale::ZhCn, Some(name)) => format!(
                "未找到名为 `{name}` 的运行中本地会话。请运行：orchbee --session {name} server start。\n"
            ),
            (Locale::EnUs, None) => "No running local session was found for this user. Start one with: orchbee server start.\n".to_owned(),
            (Locale::ZhCn, None) => "未找到当前用户正在运行的本地会话。请运行：orchbee server start。\n".to_owned(),
        }
    }

    fn cli_check_failed(self, failure: &Failure) -> String {
        let stage = failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        match (self.locale, stage) {
            (Locale::EnUs, "cohort") => self.cli_cohort_diagnostic(
                failure
                    .io_diagnostic
                    .as_ref()
                    .and_then(|observed| observed.missing_artifact),
            ),
            (Locale::ZhCn, "cohort") => self.cli_cohort_diagnostic(
                failure
                    .io_diagnostic
                    .as_ref()
                    .and_then(|observed| observed.missing_artifact),
            ),
            (Locale::EnUs, "platform") => {
                let command = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("command"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("config check");
                format!("{command} is supported on Linux only.\n")
            }
            (Locale::ZhCn, "platform") => {
                let command = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("command"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("config check");
                format!("{command} 仅在 Linux 上受支持。\n")
            }
            (Locale::EnUs, "socket") => {
                "A local session socket failed its safety checks.\n".to_owned()
            }
            (Locale::ZhCn, "socket") => "本地会话套接字未通过安全检查。\n".to_owned(),
            (Locale::EnUs, "runtime-directory") => {
                "The local session runtime directory failed its safety checks.\n".to_owned()
            }
            (Locale::ZhCn, "runtime-directory") => "本地会话运行时目录未通过安全检查。\n".to_owned(),
            (Locale::EnUs, "probe") => {
                "The directory is not a Git repository, or Git does not trust its owner. Run `git status` in that directory to see which.\n".to_owned()
            }
            (Locale::ZhCn, "probe") => {
                "该目录不是 Git 仓库，或 Git 不信任其所有者。请在该目录运行 `git status` 查看原因。\n".to_owned()
            }
            (Locale::EnUs, "list") => "Git could not list the repository's worktrees.\n".to_owned(),
            (Locale::ZhCn, "list") => "Git 无法列出该仓库的 worktree。\n".to_owned(),
            (Locale::EnUs, _) => "The runtime base is missing, invalid, or unsafe.\n".to_owned(),
            (Locale::ZhCn, _) => "运行时基座缺失、无效或不安全。\n".to_owned(),
        }
    }

    pub(crate) fn cli_cohort_diagnostic(self, missing_artifact: Option<&str>) -> String {
        let io_details = if cfg!(windows) {
            match self.locale {
                Locale::EnUs => " Run `orchbee status client --format json` for I/O details.",
                Locale::ZhCn => "运行 `orchbee status client --format json` 查看 I/O 详情。",
            }
        } else {
            ""
        };
        match (self.locale, missing_artifact) {
            (Locale::EnUs, Some(name)) => format!(
                "Required native sibling `{name}` is missing from the development cohort. Rebuild the development cohort or reinstall the complete application.{io_details}\n"
            ),
            (Locale::ZhCn, Some(name)) => format!(
                "原生开发 cohort 缺少必需的同级文件 `{name}`。请重新构建完整 cohort 或重新安装完整应用。{io_details}\n"
            ),
            (Locale::EnUs, None) => format!(
                "The native development cohort could not be validated. Verify that all cohort files are present, readable, and unchanged.{io_details}\n"
            ),
            (Locale::ZhCn, None) => format!(
                "无法验证原生开发 cohort。请确认所有 cohort 文件均存在、可读取且未被修改。{io_details}\n"
            ),
        }
    }

    pub(crate) const fn cli_output_failed(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Writing the result to stdout failed.\n",
            Locale::ZhCn => "向标准输出写入结果失败。\n",
        }
    }

    pub(crate) const fn cli_config_failed(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "One or more configuration checks failed.\n",
            Locale::ZhCn => "一项或多项配置检查失败。\n",
        }
    }

    /// The startup diagnostic when a derived Session socket path cannot fit one
    /// socket address on this platform.
    pub(crate) const fn cli_socket_path_too_long(self) -> &'static str {
        self.locale.socket_path_too_long()
    }
}
