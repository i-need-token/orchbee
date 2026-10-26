//! Localized CLI failure routing and shared diagnostics; does not recover or retry operations.

use super::{Locale, failure_target};
use crate::cli::Failure;

impl Locale {
    /// Localized human diagnostic for one CLI failure. User-controlled data is
    /// escaped before it reaches the terminal.
    pub(crate) fn cli_failure(self, failure: &Failure) -> String {
        use crate::cli::FailureKind;
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
            return match self {
                Self::EnUs => "Invalid layout input; expected the current orchbee.layout.export version 1 shape.\n".to_owned(),
                Self::ZhCn => "布局输入无效；需要当前 orchbee.layout.export 版本 1 的结构。\n".to_owned(),
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
            if reason.starts_with("source-query-") {
                return match self {
                    Self::EnUs => format!(
                        "Could not resolve the configured checkout directory ({reason}). Inspect the source with orchbee worktree list.\n"
                    ),
                    Self::ZhCn => format!(
                        "无法解析配置的检出目录（{reason}）。请使用 orchbee worktree list 检查来源。\n"
                    ),
                };
            }
            return match self {
                Self::EnUs => format!(
                    "Configuration preparation failed ({field}: {reason}). Check the file with orchbee config check.\n"
                ),
                Self::ZhCn => format!(
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
        if failure.failure_schema == Some(crate::cli::schema::WORKTREE_FAILURE_ID) {
            return self.cli_worktree_failure(failure);
        }
        // Agent read and cancelled failures answer under the dedicated Agent
        // envelope; the schema marker selects their wording the same way.
        if failure.failure_schema == Some(crate::cli::schema::AGENT_FAILURE_ID) {
            return self.cli_agent_failure(failure);
        }
        if failure.failure_schema == Some("orchbee.cli.failure.agent-projection") {
            return self.cli_projection_failure(failure);
        }
        // Launch-profile catalog reads and profile resolutions answer under
        // their dedicated envelope the same way.
        if failure.failure_schema == Some(crate::cli::schema::LAUNCH_PROFILE_FAILURE_ID) {
            return self.cli_launch_profile_failure(failure);
        }
        if failure.failure_schema == Some("orchbee.cli.failure.agent-names") {
            if failure.kind == FailureKind::NotFound {
                return match self {
                    Self::EnUs => "No Agent session or name matches the target.\n",
                    Self::ZhCn => "没有与目标匹配的智能体会话或名称。\n",
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
                return match self {
                    Self::EnUs => "This Agent name or terminal is reserved by a pending launch. Use `orchbee agent names` to inspect its remaining time.\n",
                    Self::ZhCn => "此智能体名称或终端正被启动预留占用。可用 `orchbee agent names` 查看剩余时间。\n",
                }.to_owned();
            }
        }
        match failure.kind {
            FailureKind::Timeout if local_runtime => match self {
                Self::EnUs if cfg!(windows) => "Local startup reached its time limit before readiness was confirmed. Run orchbee server diagnose with the same --session selection before retrying.\n".to_owned(),
                Self::ZhCn if cfg!(windows) => "本地启动达到时限，尚未确认就绪。请用相同 --session 选择运行 orchbee server diagnose，再决定是否重试。\n".to_owned(),
                Self::EnUs => {
                    "The local session did not become ready within the time limit; a Server may still be starting, or activation state may have been left behind. No recovery was attempted.\n"
                        .to_owned()
                }
                Self::ZhCn => {
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
                match self {
                    Self::EnUs if cfg!(windows) && reason == "recovery-required" => self.startup_failure(crate::local_startup::StartupFailure::RecoveryRequired).to_owned(),
                    Self::ZhCn if cfg!(windows) && reason == "recovery-required" => self.startup_failure(crate::local_startup::StartupFailure::RecoveryRequired).to_owned(),
                    Self::EnUs => {
                        format!("The local session refused the start ({reason}); its activation state requires recovery and no repair was attempted.\n")
                    }
                    Self::ZhCn => {
                        format!("本地会话拒绝了启动（{reason}）：其激活状态需要恢复，未尝试任何修复。\n")
                    }
                }
            }
            FailureKind::Denied if local_runtime => match self {
                Self::EnUs => {
                    "The local session refused the start; the native development cohort is not permitted.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地会话拒绝了启动：原生开发 cohort 未获许可。\n".to_owned(),
            },
            FailureKind::ServiceUnavailable if local_runtime => {
                self.cli_server_unavailable(failure)
            }
            FailureKind::ConnectionCapacity if local_runtime => match self {
                Self::EnUs => {
                    "The local session has no Client Connection capacity for the start probe.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地会话没有可接纳启动探针的客户端连接 容量。\n".to_owned(),
            },
            FailureKind::TransportLoss if local_runtime => match self {
                Self::EnUs => {
                    "The connection to the local session was lost during the start admission.\n"
                        .to_owned()
                }
                Self::ZhCn => "与本地会话的连接在启动准入期间中断。\n".to_owned(),
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
                match (self, command) {
                    (Self::EnUs, true) => {
                        "The local session denied the command; nothing was applied.\n".to_owned()
                    }
                    (Self::EnUs, false) => {
                        "The local session denied the read request.\n".to_owned()
                    }
                    (Self::ZhCn, true) => {
                        "本地会话拒绝了该命令；未应用任何修改。\n".to_owned()
                    }
                    (Self::ZhCn, false) => "本地会话拒绝了读取请求。\n".to_owned(),
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
                match (self, reason) {
                    (Self::EnUs, Some(reason)) => {
                        format!("The local session rejected the command ({reason}); nothing was applied.\n")
                    }
                    (Self::EnUs, None) => {
                        "The local session rejected the command; nothing was applied.\n".to_owned()
                    }
                    (Self::ZhCn, Some(reason)) => {
                        format!("本地会话拒绝了该命令（{reason}）；未应用任何修改。\n")
                    }
                    (Self::ZhCn, None) => "本地会话拒绝了该命令；未应用任何修改。\n".to_owned(),
                }
            }
            FailureKind::OutcomeUnknown => match self {
                Self::EnUs => {
                    "The command's outcome is unknown: the request may or may not have been applied.\nIt was not retried — inspect the Workbench state before deciding whether to repeat it.\n".to_owned()
                }
                Self::ZhCn => {
                    "命令结果未知：请求可能已应用，也可能未应用。\n该命令未被重试——请先检查 Workbench 状态，再决定是否重复执行。\n".to_owned()
                }
            },
            FailureKind::ServiceUnavailable => self.cli_service_unavailable(failure),
            FailureKind::ConnectionCapacity => match self {
                Self::EnUs => {
                    "The local session has no Client Connection capacity for this query.\n".to_owned()
                }
                Self::ZhCn => "本地会话没有可接纳本次查询的客户端连接 容量。\n".to_owned(),
            },
            FailureKind::TransportLoss => {
                let stage = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("stage"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                match (self, stage) {
                    (Self::EnUs, "command") => {
                        "The connection to the local session failed before the command could be submitted; nothing was sent.\n".to_owned()
                    }
                    (Self::ZhCn, "command") => {
                        "与本地会话的连接在命令提交前失败；未发送任何内容。\n".to_owned()
                    }
                    (Self::EnUs, "viewer") => {
                        "The connection to the local session was lost during the terminal read.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, "viewer") => "与本地会话的连接在终端读取期间中断。\n".to_owned(),
                    (Self::EnUs, _) => {
                        "The connection to the local session was lost before a snapshot arrived.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, _) => "与本地会话的连接在快照返回前中断。\n".to_owned(),
                }
            }
            FailureKind::ProtocolInvalid => match self {
                Self::EnUs => {
                    "The local session answered with an invalid protocol response.\n".to_owned()
                }
                Self::ZhCn => "本地会话返回了无效的协议响应。\n".to_owned(),
            },
            FailureKind::Incompatible => match self {
                Self::EnUs => {
                    "The running local session is incompatible with this development cohort.\n"
                        .to_owned()
                }
                Self::ZhCn => "正在运行的本地会话与当前开发 cohort 不兼容。\n".to_owned(),
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
                match (self, input_stage) {
                    (Self::EnUs, true) => {
                        "The declared input could not be read within the time limit; nothing was sent.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, true) => "声明的输入未能在时限内读取；未发送任何内容。\n".to_owned(),
                    (Self::EnUs, false) => {
                        "The wait deadline elapsed before the text appeared in the terminal.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, false) => "等待期限内终端中未出现目标文本。\n".to_owned(),
                }
            }
            FailureKind::Cancelled => {
                let input_stage = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("stage"))
                    .and_then(|value| value.as_str())
                    == Some("input");
                match (self, input_stage) {
                    (Self::EnUs, true) => {
                        "Reading the declared input was cancelled; nothing was sent.\n".to_owned()
                    }
                    (Self::ZhCn, true) => "声明输入的读取已被取消；未发送任何内容。\n".to_owned(),
                    (Self::EnUs, false) => {
                        "The wait was cancelled; no match is reported.\n".to_owned()
                    }
                    (Self::ZhCn, false) => "等待已被取消；未报告任何匹配。\n".to_owned(),
                }
            }
            FailureKind::OutputTooLarge => match self {
                Self::EnUs => {
                    "The result exceeded the 8 MiB output bound; nothing was written.\n".to_owned()
                }
                Self::ZhCn => "结果超过 8 MiB 输出上限；未写出任何内容。\n".to_owned(),
            },
            FailureKind::InternalFailure => match self {
                Self::EnUs => "The CLI hit an internal failure; no data was written.\n".to_owned(),
                Self::ZhCn => "CLI 发生内部错误；未写出任何数据。\n".to_owned(),
            },
        }
    }

    fn cli_invalid_identifier(self, resource: &str, target: &str) -> String {
        let escaped = crate::cli::escape::quoted(target);
        match self {
            Self::EnUs => {
                format!("Invalid {resource} identifier {escaped}.\n")
            }
            Self::ZhCn => format!("无效的 {resource} 标识符 {escaped}。\n"),
        }
    }

    fn cli_not_found(self, resource: &str, target: &str) -> String {
        let escaped = crate::cli::escape::quoted(target);
        match self {
            Self::EnUs => format!("No {resource} matches {escaped}.\n"),
            Self::ZhCn => format!("没有与 {escaped} 匹配的 {resource}。\n"),
        }
    }

    /// The bounded candidate list and the true total: list handles with
    /// labels when the resolver supplied them, otherwise the JSON identifiers.
    fn cli_ambiguous(self, resource: &str, target: &str, failure: &Failure) -> String {
        let escaped = crate::cli::escape::quoted(target);
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
            return match self {
                Self::EnUs => {
                    format!("More than one {resource} matches {escaped}; use its exact ID.\n")
                }
                Self::ZhCn => format!("多个 {resource} 与 {escaped} 匹配；请使用精确 ID。\n"),
            };
        }
        let more = if total > listed.len() { ", …" } else { "" };
        let list = listed.join(", ");
        match self {
            Self::EnUs => format!(
                "{total} {resource} candidates match {escaped}: {list}{more}. Use one of these handles or the full ID.\n"
            ),
            Self::ZhCn => format!(
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
        match (self, stage) {
            // A known directory cause is an input to check, not a transient Session fault.
            (Self::EnUs, "command") if reason == Some("directory-unavailable") => {
                "The working directory is unavailable: it may not exist or may not be accessible. Check the path; nothing was applied.\n"
                    .to_owned()
            }
            (Self::ZhCn, "command") if reason == Some("directory-unavailable") => {
                "工作目录不可用：目录可能不存在或无法访问。请检查路径；未应用任何修改。\n".to_owned()
            }
            (Self::EnUs, "platform") => {
                "This command is unavailable on this platform.\n".to_owned()
            }
            (Self::ZhCn, "platform") => "当前平台尚不支持此命令。\n".to_owned(),
            (Self::EnUs, "discovery") | (Self::ZhCn, "discovery") => {
                self.cli_missing_session(selected_session)
            }
            (Self::EnUs, "snapshot") => {
                "The local session could not produce a snapshot right now.\n".to_owned()
            }
            (Self::ZhCn, "snapshot") => "本地会话当前无法产出快照。\n".to_owned(),
            (_, "viewer") => self.cli_viewer_unavailable(reason).to_owned(),
            (Self::EnUs, "command") => {
                "The local session could not execute the command right now; nothing was applied.\n"
                    .to_owned()
            }
            (Self::ZhCn, "command") => "本地会话当前无法执行该命令；未应用任何修改。\n".to_owned(),
            (Self::EnUs, _) => {
                "The local session could not admit this query connection.\n".to_owned()
            }
            (Self::ZhCn, _) => "本地会话无法接纳本次查询连接。\n".to_owned(),
        }
    }

    /// Viewer-stage unavailability: the typed Host reason stays in the JSON
    /// details; text names the cause in words and one next step.
    fn cli_viewer_unavailable(self, reason: Option<&str>) -> &'static str {
        match (self, reason) {
            (Self::EnUs, None) => {
                "The terminal could not be observed for reading right now. Retry the command shortly.\n"
            }
            (Self::ZhCn, None) => "该终端当前无法进行读取观察。请稍后重试该命令。\n",
            (Self::EnUs, Some("stale-revision" | "stale-generation")) => {
                "The terminal output kept changing, so no consistent copy could be read before the deadline. Retry the read once the output settles.\n"
            }
            (Self::ZhCn, Some("stale-revision" | "stale-generation")) => {
                "终端输出持续变化，截止时间前未能读取一致的内容。请在输出稳定后重试读取。\n"
            }
            (Self::EnUs, Some("history-evicted")) => {
                "The requested terminal history was discarded while it was being read. Retry the read with fewer lines.\n"
            }
            (Self::ZhCn, Some("history-evicted")) => {
                "所请求的终端历史在读取过程中已被丢弃。请减少行数后重试读取。\n"
            }
            (Self::EnUs, Some(_)) => {
                "The terminal history could not be read right now. Retry the read shortly.\n"
            }
            (Self::ZhCn, Some(_)) => "该终端的历史当前无法读取。请稍后重试读取。\n",
        }
    }

    pub(crate) fn cli_missing_session(self, selected: Option<&str>) -> String {
        match (self, selected) {
            (Self::EnUs, Some(name)) => format!(
                "No running local session named `{name}` was found. Start it with: orchbee --session {name} server start.\n"
            ),
            (Self::ZhCn, Some(name)) => format!(
                "未找到名为 `{name}` 的运行中本地会话。请运行：orchbee --session {name} server start。\n"
            ),
            (Self::EnUs, None) => "No running local session was found for this user. Start one with: orchbee server start.\n".to_owned(),
            (Self::ZhCn, None) => "未找到当前用户正在运行的本地会话。请运行：orchbee server start。\n".to_owned(),
        }
    }

    fn cli_check_failed(self, failure: &Failure) -> String {
        let stage = failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        match (self, stage) {
            (Self::EnUs, "cohort") => self.cli_cohort_diagnostic(
                failure
                    .io_diagnostic
                    .as_ref()
                    .and_then(|observed| observed.missing_artifact),
            ),
            (Self::ZhCn, "cohort") => self.cli_cohort_diagnostic(
                failure
                    .io_diagnostic
                    .as_ref()
                    .and_then(|observed| observed.missing_artifact),
            ),
            (Self::EnUs, "platform") => {
                let command = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("command"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("config check");
                format!("{command} is supported on Linux only.\n")
            }
            (Self::ZhCn, "platform") => {
                let command = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("command"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("config check");
                format!("{command} 仅在 Linux 上受支持。\n")
            }
            (Self::EnUs, "socket") => {
                "A local session socket failed its safety checks.\n".to_owned()
            }
            (Self::ZhCn, "socket") => "本地会话套接字未通过安全检查。\n".to_owned(),
            (Self::EnUs, "runtime-directory") => {
                "The local session runtime directory failed its safety checks.\n".to_owned()
            }
            (Self::ZhCn, "runtime-directory") => "本地会话运行时目录未通过安全检查。\n".to_owned(),
            (Self::EnUs, "probe") => {
                "The directory is not a Git repository, or Git does not trust its owner. Run `git status` in that directory to see which.\n".to_owned()
            }
            (Self::ZhCn, "probe") => {
                "该目录不是 Git 仓库，或 Git 不信任其所有者。请在该目录运行 `git status` 查看原因。\n".to_owned()
            }
            (Self::EnUs, "list") => "Git could not list the repository's worktrees.\n".to_owned(),
            (Self::ZhCn, "list") => "Git 无法列出该仓库的 worktree。\n".to_owned(),
            (Self::EnUs, _) => "The runtime base is missing, invalid, or unsafe.\n".to_owned(),
            (Self::ZhCn, _) => "运行时基座缺失、无效或不安全。\n".to_owned(),
        }
    }

    pub(crate) fn cli_cohort_diagnostic(self, missing_artifact: Option<&str>) -> String {
        let io_details = if cfg!(windows) {
            match self {
                Self::EnUs => " Run `orchbee status client --format json` for I/O details.",
                Self::ZhCn => "运行 `orchbee status client --format json` 查看 I/O 详情。",
            }
        } else {
            ""
        };
        match (self, missing_artifact) {
            (Self::EnUs, Some(name)) => format!(
                "Required native sibling `{name}` is missing from the development cohort. Rebuild the development cohort or reinstall the complete application.{io_details}\n"
            ),
            (Self::ZhCn, Some(name)) => format!(
                "原生开发 cohort 缺少必需的同级文件 `{name}`。请重新构建完整 cohort 或重新安装完整应用。{io_details}\n"
            ),
            (Self::EnUs, None) => format!(
                "The native development cohort could not be validated. Verify that all cohort files are present, readable, and unchanged.{io_details}\n"
            ),
            (Self::ZhCn, None) => format!(
                "无法验证原生开发 cohort。请确认所有 cohort 文件均存在、可读取且未被修改。{io_details}\n"
            ),
        }
    }

    pub(crate) const fn cli_output_failed(self) -> &'static str {
        match self {
            Self::EnUs => "Writing the result to stdout failed.\n",
            Self::ZhCn => "向标准输出写入结果失败。\n",
        }
    }

    pub(crate) const fn cli_config_failed(self) -> &'static str {
        match self {
            Self::EnUs => "One or more configuration checks failed.\n",
            Self::ZhCn => "一项或多项配置检查失败。\n",
        }
    }

    /// The startup diagnostic when a derived Session socket path cannot fit one
    /// socket address on this platform.
    pub(crate) const fn cli_socket_path_too_long(self) -> &'static str {
        match self {
            Self::EnUs => {
                "A derived local session socket path is too long for this platform's socket address limit. Set ORCHBEE_RUNTIME_BASE to a shorter absolute directory.\n"
            }
            Self::ZhCn => {
                "派生出的本地会话套接字路径超出本平台的套接字地址长度限制。请将 ORCHBEE_RUNTIME_BASE 设为更短的绝对目录。\n"
            }
        }
    }
}
