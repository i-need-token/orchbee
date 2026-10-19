//! Localized strings for the read-only CLI query surface. Machine envelope
//! fields stay English; these strings are the human stderr/text-render layer.

mod agent_view_spec;
mod client_control;
mod client_layout;
mod help;
mod help_arguments;
mod help_options;
pub(crate) use help::{HelpDetails, HelpText};
#[cfg(windows)]
mod config_import;
#[cfg(windows)]
mod config_reset;
#[cfg(windows)]
mod layouts;
#[cfg(windows)]
mod named_sessions;
mod organization;
mod panel_agent;
mod panel_metadata;
#[cfg(windows)]
mod panel_relocation;
#[cfg(windows)]
mod panel_terminal_creation;
mod runtime_status;
#[cfg(windows)]
mod schema_export;
mod workspace_metadata;
use super::Locale;
use crate::cli::Failure;
#[cfg(windows)]
mod terminal_clear;
#[cfg(windows)]
mod terminal_copy;
#[cfg(windows)]
mod terminal_editor;
#[cfg(windows)]
mod terminal_links;
#[cfg(windows)]
mod workspace_creation;
mod workspace_group_close;
mod worktree;

impl Locale {
    #[cfg(windows)]
    pub(crate) fn cli_terminal_selection(
        self,
        terminal: &str,
        revision: u64,
        text: &str,
    ) -> String {
        let text = crate::cli::escape::quoted(text);
        match self {
            Self::EnUs => {
                format!("Terminal {terminal}, content revision {revision}\nSelection: {text}\n")
            }
            Self::ZhCn => format!("终端 {terminal}，内容修订 {revision}\n选区：{text}\n"),
        }
    }
    pub(crate) const fn cli_report_presentation_label(self) -> &'static str {
        match self {
            Self::EnUs => "Presentation",
            Self::ZhCn => "终端成员",
        }
    }
    /// Localized human diagnostic for one CLI failure. User-controlled data is
    /// escaped before it reaches the terminal.
    pub(crate) fn cli_failure(self, failure: &Failure) -> String {
        use crate::cli::FailureKind;
        if failure.failure_schema.is_none()
            && failure.version == 3
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
        #[cfg(windows)]
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
        #[cfg(windows)]
        if failure.version == 2
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
        #[cfg(windows)]
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
                    Self::ZhCn => "没有与目标匹配的 Agent 会话或名称。\n",
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
                    Self::ZhCn => "此 Agent 名称或终端正被启动预留占用。可用 `orchbee agent names` 查看剩余时间。\n",
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
                self.cli_site_unavailable(failure)
            }
            FailureKind::SessionCapacity if local_runtime => match self {
                Self::EnUs => {
                    "The local session has no Client Session capacity for the start probe.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地会话没有可接纳启动探针的 Client Session 容量。\n".to_owned(),
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
                self.cli_ambiguous(resource, &target)
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
                    .unwrap_or("");
                match self {
                    Self::EnUs => {
                        format!("The local session rejected the command ({reason}); nothing was applied.\n")
                    }
                    Self::ZhCn => {
                        format!("本地会话拒绝了该命令（{reason}）；未应用任何修改。\n")
                    }
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
            FailureKind::SessionCapacity => match self {
                Self::EnUs => {
                    "The local session has no Client Session capacity for this query.\n".to_owned()
                }
                Self::ZhCn => "本地会话没有可接纳本次查询的 Client Session 容量。\n".to_owned(),
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

    /// A successful input submission, without claiming command completion.
    pub(crate) fn cli_send_success(self, terminal_id: &str) -> String {
        match self {
            Self::EnUs => format!("Sent to terminal {terminal_id}.\n"),
            Self::ZhCn => format!("已发送至终端 {terminal_id}。\n"),
        }
    }

    /// The stderr line a typed `terminal send` answer carries on a nonzero
    /// exit: what the outcome axis settled, and — whenever resending could
    /// type the same input twice — an explicit do-not-resend warning. The
    /// send envelope on stdout still reports both axes verbatim.
    pub(crate) fn cli_send_stderr(
        self,
        verdict: crate::cli::send::SendVerdict,
        reason: Option<&'static str>,
        cleanup_unconfirmed: bool,
    ) -> String {
        use crate::cli::send::SendVerdict;
        let cleanup_note = match (self, cleanup_unconfirmed) {
            (_, false) => "",
            (Self::EnUs, true) => " Release of the temporary input channel is unconfirmed.",
            (Self::ZhCn, true) => " 临时输入通道的释放状态未确认。",
        };
        match verdict {
            SendVerdict::Quiet => String::new(),
            SendVerdict::AcceptedCleanupUnconfirmed => match self {
                Self::EnUs => {
                    "The terminal accepted the input, but release of the temporary input channel could not be confirmed. Do not resend — the input may already have taken effect.\n".to_owned()
                }
                Self::ZhCn => {
                    "终端已接受该输入，但临时输入通道的释放无法确认。请勿重发——输入可能已生效。\n".to_owned()
                }
            },
            SendVerdict::Rejected => match self {
                Self::EnUs => format!(
                    "The local session rejected the input ({}); nothing was sent.{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
                Self::ZhCn => format!(
                    "本地会话拒绝了该输入（{}）；未发送任何内容。{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
            },
            SendVerdict::PartialAccepted => match self {
                Self::EnUs => format!(
                    "The send stopped partway ({}): the receipt's recordsAccepted prefix was typed; the stopped record and everything after it was not sent. Do not resend the whole sequence — the accepted prefix may already have taken effect.{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
                Self::ZhCn => format!(
                    "发送中途停止（{}）：回执中 recordsAccepted 前缀的输入已键入；被停下的记录及其后的内容未发送。请勿重发整个序列——已接受的前缀可能已生效。{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
            },
            SendVerdict::Unavailable => match self {
                Self::EnUs => format!(
                    "The input could not be submitted ({}); nothing was sent.{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
                Self::ZhCn => format!(
                    "该输入未能提交（{}）；未发送任何内容。{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
            },
            SendVerdict::OutcomeUnknown => match self {
                Self::EnUs => format!(
                    "The send outcome is unknown: the input may or may not have reached the terminal. It was not retried — inspect the terminal before deciding whether to resend.{cleanup_note}\n"
                ),
                Self::ZhCn => format!(
                    "发送结果未知：输入可能已到达终端，也可能未到达。未自动重试——请先检查终端，再决定是否重发。{cleanup_note}\n"
                ),
            },
        }
    }

    /// `worktree list` failure diagnostics, keyed by the details the typed
    /// outcome mapping produced. Every line states what the listing observed.
    fn cli_worktree_failure(self, failure: &Failure) -> String {
        use crate::cli::FailureKind;
        let detail = |name: &str| {
            failure
                .details
                .as_ref()
                .and_then(|details| details.get(name))
                .and_then(|value| value.as_str())
                .unwrap_or("")
        };
        match failure.kind {
            FailureKind::NotFound => {
                let (_resource, target) = failure_target(failure);
                match self {
                    Self::EnUs => {
                        format!("No directory with ID {target} is registered to the project.\n")
                    }
                    Self::ZhCn => format!("该项目未注册 ID 为 {target} 的目录。\n"),
                }
            }
            FailureKind::Denied => match self {
                Self::EnUs => "The local session denied the worktree listing.\n".to_owned(),
                Self::ZhCn => "本地会话拒绝了该 worktree 列举。\n".to_owned(),
            },
            FailureKind::ServiceUnavailable => {
                let reason = detail("reason");
                match (self, reason) {
                    (Self::EnUs, "job-dead") => {
                        "The directory's worktree listing job is no longer alive.\n".to_owned()
                    }
                    (Self::ZhCn, "job-dead") => {
                        "该目录的 worktree 列举作业已不存在。\n".to_owned()
                    }
                    (Self::EnUs, "job-unknown") => {
                        "The directory's worktree listing job is unknown to the Worker.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, "job-unknown") => {
                        "Worker 不认识该目录的 worktree 列举作业。\n".to_owned()
                    }
                    (Self::EnUs, "worker-unavailable") => {
                        "The directory's Worker is not available for the worktree listing right now.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, "worker-unavailable") => {
                        "该目录的 Worker 当前无法进行 worktree 列举。\n".to_owned()
                    }
                    (Self::EnUs, _) => {
                        "The local session could not run the worktree listing right now.\n".to_owned()
                    }
                    (Self::ZhCn, _) => "本地会话当前无法执行 worktree 列举。\n".to_owned(),
                }
            }
            FailureKind::CheckFailed => {
                let stage = detail("stage");
                match (self, stage) {
                    (Self::EnUs, "parse") => {
                        "The directory's Git worktree listing was malformed.\n".to_owned()
                    }
                    (Self::ZhCn, "parse") => {
                        "该目录的 Git worktree 列表输出格式无效。\n".to_owned()
                    }
                    (Self::EnUs, stage) => {
                        format!("The directory's Git worktree {stage} stage failed.\n")
                    }
                    (Self::ZhCn, stage) => {
                        format!("该目录的 Git worktree {stage} 阶段失败。\n")
                    }
                }
            }
            FailureKind::Timeout => match self {
                Self::EnUs => {
                    "The worktree listing did not complete within the time limit.\n".to_owned()
                }
                Self::ZhCn => "worktree 列举未在时限内完成。\n".to_owned(),
            },
            FailureKind::OutputTooLarge => {
                let origin = detail("origin");
                match self {
                    Self::EnUs => {
                        format!("The worktree listing exceeded its output bound ({origin}).\n")
                    }
                    Self::ZhCn => format!("worktree 列举超出其输出上限（{origin}）。\n"),
                }
            }
            FailureKind::TransportLoss => match self {
                Self::EnUs => {
                    "The connection to the local session was lost during the worktree listing.\n"
                        .to_owned()
                }
                Self::ZhCn => "与本地会话的连接在 worktree 列举期间中断。\n".to_owned(),
            },
            FailureKind::ProtocolInvalid => match self {
                Self::EnUs => {
                    "The local session answered the worktree listing with an invalid protocol response.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地会话对 worktree 列举返回了无效的协议响应。\n".to_owned(),
            },
            _ => match self {
                Self::EnUs => "The CLI hit an internal failure; no data was written.\n".to_owned(),
                Self::ZhCn => "CLI 发生内部错误；未写出任何数据。\n".to_owned(),
            },
        }
    }

    /// `launch-profile` failure diagnostics, keyed by the details the typed
    /// catalog outcome mapping produced. Every line states what the catalog
    /// resolution observed — the CLI never resolved the selector itself.
    fn cli_launch_profile_failure(self, failure: &Failure) -> String {
        use crate::cli::FailureKind;
        let detail = |name: &str| {
            failure
                .details
                .as_ref()
                .and_then(|details| details.get(name))
                .and_then(|value| value.as_str())
                .unwrap_or("")
        };
        match failure.kind {
            FailureKind::NotFound => {
                let (_resource, target) = failure_target(failure);
                let escaped = crate::cli::escape::quoted(&target);
                match self {
                    Self::EnUs => {
                        format!("No launch profile matches {escaped}.\n")
                    }
                    Self::ZhCn => format!("没有与 {escaped} 匹配的启动配置。\n"),
                }
            }
            FailureKind::Ambiguous => {
                let (_resource, target) = failure_target(failure);
                let escaped = crate::cli::escape::quoted(&target);
                match self {
                    Self::EnUs => {
                        format!(
                            "More than one launch profile matches {escaped}; use its exact profile ID.\n"
                        )
                    }
                    Self::ZhCn => {
                        format!("多个启动配置与 {escaped} 匹配；请使用精确的 profile ID。\n")
                    }
                }
            }
            FailureKind::Denied => match self {
                Self::EnUs => "The local session denied the launch profile request.\n".to_owned(),
                Self::ZhCn => "本地会话拒绝了该启动配置请求。\n".to_owned(),
            },
            FailureKind::ServiceUnavailable => match self {
                Self::EnUs => {
                    "The local session could not answer the launch profile request right now.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地会话当前无法应答该启动配置请求。\n".to_owned(),
            },
            FailureKind::Timeout => match self {
                Self::EnUs => {
                    "The launch profile request did not complete within the time limit.\n"
                        .to_owned()
                }
                Self::ZhCn => "启动配置请求未在时限内完成。\n".to_owned(),
            },
            FailureKind::TransportLoss => match self {
                Self::EnUs => {
                    "The connection to the local session was lost during the launch profile request.\n"
                        .to_owned()
                }
                Self::ZhCn => "与本地会话的连接在启动配置请求期间中断。\n".to_owned(),
            },
            FailureKind::ProtocolInvalid => {
                let reason = detail("reason");
                match (self, reason) {
                    (Self::EnUs, "correlation-mismatch") => {
                        "The local session answered a different launch profile request.\n".to_owned()
                    }
                    (Self::ZhCn, "correlation-mismatch") => {
                        "本地会话应答了另一个启动配置请求。\n".to_owned()
                    }
                    (Self::EnUs, _) => {
                        "The local session answered the launch profile request with an invalid protocol response.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, _) => {
                        "本地会话对启动配置请求返回了无效的协议响应。\n".to_owned()
                    }
                }
            }
            _ => match self {
                Self::EnUs => "The CLI hit an internal failure; no data was written.\n".to_owned(),
                Self::ZhCn => "CLI 发生内部错误；未写出任何数据。\n".to_owned(),
            },
        }
    }

    /// The localized `invalid-usage` stderr diagnostic: one concrete cause
    /// plus a help hint that always names a real path. `None` and the
    /// `Invalid` fallback keep the original generic text.
    fn cli_usage(self, usage: Option<&crate::invocation::UsageReason>) -> String {
        use crate::invocation::{ExpectedValue, SendSourceCause, UsageContext, UsageReason};
        let Some(reason) = usage else {
            return self.usage_failure().to_owned();
        };
        let echo = |value: &str| crate::cli::escape::diagnostic_token(value);
        let context = |context: UsageContext| match context {
            UsageContext::Root => "orchbee".to_owned(),
            UsageContext::Path(path) => format!("orchbee {path}"),
            UsageContext::Literal(literal) => literal.to_owned(),
        };
        let hint = |context: UsageContext| match context {
            UsageContext::Path(path) => format!("orchbee {path} --help"),
            UsageContext::Root | UsageContext::Literal(_) => "orchbee --help".to_owned(),
        };
        let expected = |expected: ExpectedValue| match (self, expected) {
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::TerminalViewportPoint) => {
                "viewport ROW:COL (row/column 0..65535)"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::TerminalViewportPoint) => "视口 ROW:COL（行／列 0..65535）",
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::TerminalSelectionPoint) => {
                "ROW:COL (row 0..4294967295, column 0..65535)"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::TerminalSelectionPoint) => {
                "ROW:COL（行 0..4294967295，列 0..65535）"
            }
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::TerminalSelectionSpan) => {
                "a selection spanning at most 4096 rows"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::TerminalSelectionSpan) => "最多跨 4096 行的选区",
            #[cfg(windows)]
            (_, ExpectedValue::TerminalContentRevision) => "0..18446744073709551615",
            #[cfg(windows)]
            (_, ExpectedValue::TerminalContentGeneration) => "1..18446744073709551615",
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::TerminalCopyMotion) => {
                "one of the ten motions listed by help terminal copy-motion"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::TerminalCopyMotion) => {
                "help terminal copy-motion 列出的十种移动之一"
            }
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::TerminalCopyQuery) => {
                "a literal query of at most 4096 UTF-8 bytes"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::TerminalCopyQuery) => "最多 4096 UTF-8 字节的字面量查询",
            #[cfg(windows)]
            (_, ExpectedValue::TerminalCopyDirection) => "forward|backward",
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::TerminalCopyPrevious) => {
                "ROW:COL-ROW:COL (inclusive buffer endpoints)"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::TerminalCopyPrevious) => {
                "ROW:COL-ROW:COL（包含缓冲区端点）"
            }

            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::PluginActionId) => {
                "an action ID or plugin.action ID (up to 241 ASCII characters)"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::PluginActionId) => {
                "action ID 或 plugin.action ID（最多 241 个 ASCII 字符）"
            }
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::GraphicsSpecification) => {
                "a closed graphics spec JSON object, @regular-file path, or - for noninteractive stdin"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::GraphicsSpecification) => {
                "封闭图形 spec JSON 对象、@普通文件路径，或用于非交互 stdin 的 -"
            }
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::EventsSpecification) => {
                "a valid event name, closed filter/cursor JSON, or the documented bounded event option"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::EventsSpecification) => {
                "有效事件名、封闭 filter/cursor JSON，或帮助中声明的有界事件参数"
            }
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::ConfiguredCommandId) => {
                "an installed opaque command ID from command list"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::ConfiguredCommandId) => {
                "command list 显示的已安装不透明命令 ID"
            }
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::PluginContext) => {
                "a bounded plugin context JSON object (16 KiB)"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::PluginContext) => "有界插件上下文 JSON 对象（16 KiB）",
            #[cfg(windows)]
            (_, ExpectedValue::PluginLogLimit) => "1..200",
            #[cfg(windows)]
            (_, ExpectedValue::PluginPopupSize) => "0..65535 | 1%..100%",
            #[cfg(windows)]
            (_, ExpectedValue::PluginPanePlacement) => "tab|split|zoomed|overlay|popup",
            #[cfg(windows)]
            (_, ExpectedValue::PluginPaneDirection) => "right|down",
            #[cfg(windows)]
            (_, ExpectedValue::PluginGithubSource) => "owner/repo[/subdir]",
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::PluginGitReference) => "a Git branch, tag or commit",
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::PluginGitReference) => "Git 分支、标签或提交",

            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::PluginId) => {
                "1-120 ASCII letters, digits, colon, dot, underscore or hyphen"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::PluginId) => {
                "1-120 个 ASCII 字母、数字、冒号、点、下划线或连字符"
            }
            (Self::EnUs, ExpectedValue::WindowTitle) => {
                "nonempty title after removing controls and trimming"
            }
            (Self::ZhCn, ExpectedValue::WindowTitle) => "移除控制字符并 trim 后非空的标题",
            (Self::EnUs, ExpectedValue::NotificationTitle) => "a nonempty notification title",
            (Self::ZhCn, ExpectedValue::NotificationTitle) => "非空通知标题",
            (_, ExpectedValue::NotificationPosition) => {
                "top-left, top-right, bottom-left, bottom-right"
            }
            (_, ExpectedValue::NotificationSound) => "none, done, request",
            (Self::EnUs, ExpectedValue::ReportAgent) => "a nonempty Agent label",
            (Self::ZhCn, ExpectedValue::ReportAgent) => "非空 Agent 标签",
            (Self::EnUs, ExpectedValue::ReportState) => {
                "idle, working, blocked, done, error, or unknown"
            }
            (Self::ZhCn, ExpectedValue::ReportState) => {
                "idle、working、blocked、done、error 或 unknown"
            }
            (Self::EnUs, ExpectedValue::ReportSource) => "a nonempty report source",
            (Self::ZhCn, ExpectedValue::ReportSource) => "非空上报来源",
            (Self::EnUs, ExpectedValue::ReportSize) => {
                "an Agent report within the 16 KiB request limit"
            }
            (Self::ZhCn, ExpectedValue::ReportSize) => "完整请求不超过 16 KiB 的 Agent 上报",
            (Self::EnUs, ExpectedValue::MetadataAgent) => {
                "a nonempty Agent label, at most 80 Unicode scalars without control characters"
            }
            (Self::ZhCn, ExpectedValue::MetadataAgent) => {
                "非空 Agent 标签，至多 80 个 Unicode 标量且不含控制字符"
            }
            (Self::EnUs, ExpectedValue::MetadataStateLabel) => {
                "STATE=TEXT with nonempty text; state is idle, working, blocked, done, error or unknown"
            }
            (Self::ZhCn, ExpectedValue::MetadataStateLabel) => {
                "状态=非空文本；状态为 idle、working、blocked、done、error 或 unknown"
            }
            (Self::EnUs, ExpectedValue::MetadataPresentation) => {
                "at least one metadata change, without setting and clearing the same field"
            }
            (Self::ZhCn, ExpectedValue::MetadataPresentation) => {
                "至少一个元数据变更，且不能同时设置和清除同一字段"
            }
            (Self::EnUs, ExpectedValue::MetadataSource) => {
                "1–80 ASCII letters, digits, colon, dot, underscore or hyphen"
            }
            (Self::ZhCn, ExpectedValue::MetadataSource) => {
                "1–80 个 ASCII 字母、数字、冒号、点、下划线或连字符"
            }
            (Self::EnUs, ExpectedValue::MetadataToken) => {
                "a 1–32 character ASCII token key (letters/digits/_/-), with =VALUE for --token"
            }
            (Self::ZhCn, ExpectedValue::MetadataToken) => {
                "1–32 个 ASCII 字母／数字／下划线／连字符组成的键；--token 需使用 键=值"
            }
            (Self::EnUs, ExpectedValue::MetadataPatchSize) => {
                "at most 16 distinct token keys per report"
            }
            (Self::ZhCn, ExpectedValue::MetadataPatchSize) => "每次上报至多 16 个不同 token 键",
            (Self::EnUs, ExpectedValue::MetadataSequence) => "an unsigned 64-bit decimal sequence",
            (Self::ZhCn, ExpectedValue::MetadataSequence) => "无符号 64 位十进制序号",
            (Self::EnUs, ExpectedValue::MetadataTtl) => "a TTL from 1 to 86400000 milliseconds",
            (Self::ZhCn, ExpectedValue::MetadataTtl) => "1–86400000 毫秒的 TTL",

            (Self::EnUs, ExpectedValue::OutputFilePath) => {
                "a literal file path of 1–4096 UTF-8 bytes"
            }
            (Self::ZhCn, ExpectedValue::OutputFilePath) => "1–4096 UTF-8 字节的字面文件路径",
            (Self::EnUs, ExpectedValue::PresentationId) => {
                "a nonzero 32-character lowercase hexadecimal Presentation ID"
            }
            (Self::ZhCn, ExpectedValue::PresentationId) => {
                "非零的 32 位小写十六进制 Presentation 标识"
            }
            (Self::EnUs, ExpectedValue::ClientId) => {
                "a nonzero 32-character lowercase hexadecimal Client ID"
            }
            (Self::ZhCn, ExpectedValue::ClientId) => "非零的 32 位小写十六进制 Client 标识",
            (Self::EnUs, ExpectedValue::ClientOperationId) => {
                "a nonzero 32-character lowercase hexadecimal Client control operation ID"
            }
            (Self::ZhCn, ExpectedValue::ClientOperationId) => {
                "非零的 32 位小写十六进制 Client 控制操作标识"
            }
            (Self::EnUs, ExpectedValue::WorktreeReference) => {
                "a nonempty Git reference of at most 1024 UTF-8 bytes without a leading dash or control characters"
            }
            (Self::ZhCn, ExpectedValue::WorktreeReference) => {
                "至多 1024 UTF-8 字节、无前导短横线或控制字符的非空 Git 引用"
            }
            (Self::EnUs, ExpectedValue::WorktreeOperationId) => {
                "a nonzero 32-character lowercase hexadecimal operation ID"
            }
            (Self::ZhCn, ExpectedValue::WorktreeOperationId) => "非零的 32 位小写十六进制操作标识",
            (Self::EnUs, ExpectedValue::WorktreeAttempt) => "a decimal attempt number from 1 to 64",
            (Self::ZhCn, ExpectedValue::WorktreeAttempt) => "1–64 的十进制尝试序号",
            (Self::EnUs, ExpectedValue::SiteStopId) => {
                "a nonzero 32-character lowercase hexadecimal stop request ID"
            }
            (Self::ZhCn, ExpectedValue::SiteStopId) => "非零的 32 位小写十六进制停止请求标识",
            (Self::EnUs, ExpectedValue::Format) => "'text' or 'json'",
            (Self::ZhCn, ExpectedValue::Format) => "'text' 或 'json'",
            (Self::EnUs, ExpectedValue::RightClick) => "'orchbee' or 'panel'",
            (Self::ZhCn, ExpectedValue::RightClick) => "'orchbee' 或 'panel'",
            (Self::EnUs, ExpectedValue::PanelLabel) => {
                "at most 512 Unicode characters; empty text clears the title"
            }
            (Self::ZhCn, ExpectedValue::PanelLabel) => "最多 512 个 Unicode 字符；空文本清除标题",
            (Self::EnUs, ExpectedValue::Label) => "a nonempty label of at most 512 characters",
            (Self::ZhCn, ExpectedValue::Label) => "1–512 个字符的非空标签",
            (Self::EnUs, ExpectedValue::Dimension) => "a decimal integer from 0 to 65535",
            (Self::ZhCn, ExpectedValue::Dimension) => "0–65535 的十进制整数",
            (Self::EnUs, ExpectedValue::WorkspaceInsertIndex) => {
                "an integer insertion boundary from 0 to 1023"
            }
            (Self::ZhCn, ExpectedValue::WorkspaceInsertIndex) => "0 到 1023 的整数插入边界",
            (Self::EnUs, ExpectedValue::WorkspaceBlockSize) => "1 to 1023 Workspace targets",
            (Self::ZhCn, ExpectedValue::WorkspaceBlockSize) => "1 到 1023 个 Workspace 目标",
            (Self::EnUs, ExpectedValue::TabInsertIndex) => {
                "an integer from 0 to 256 (boundary in the original Tab order)"
            }
            (Self::ZhCn, ExpectedValue::TabInsertIndex) => {
                "0 到 256 的整数（原始 Tab 顺序中的插入边界）"
            }
            (Self::EnUs, ExpectedValue::TabDirection) => "'previous', 'next', 'first', or 'last'",
            (Self::ZhCn, ExpectedValue::TabDirection) => "'previous'、'next'、'first' 或 'last'",
            (Self::EnUs, ExpectedValue::FiniteNumber) => "a finite number",
            (Self::ZhCn, ExpectedValue::FiniteNumber) => "有限数值",
            (Self::EnUs, ExpectedValue::SplitPath) => {
                "'root' or a dot-separated binary path of at most 16 steps (for example 0.1)"
            }
            (Self::ZhCn, ExpectedValue::SplitPath) => {
                "'root' 或至多 16 步的点分二元路径（例如 0.1）"
            }
            (Self::EnUs, ExpectedValue::LayoutDirection) => "'left', 'right', 'up', or 'down'",
            (Self::ZhCn, ExpectedValue::LayoutDirection) => "'left'、'right'、'up' 或 'down'",
            (Self::EnUs, ExpectedValue::PanelDirection) => "'above', 'below', 'left', or 'right'",
            (Self::ZhCn, ExpectedValue::PanelDirection) => "'above'、'below'、'left' 或 'right'",
            (Self::EnUs, ExpectedValue::Shell) => {
                "'bash', 'zsh', 'fish', 'powershell', or 'elvish'"
            }
            (Self::ZhCn, ExpectedValue::Shell) => "'bash'、'zsh'、'fish'、'powershell' 或 'elvish'",
            (Self::EnUs, ExpectedValue::HistoryRows) => "a decimal integer from 1 to 1000",
            (Self::ZhCn, ExpectedValue::HistoryRows) => "1–1000 的十进制整数",
            (Self::EnUs, ExpectedValue::DirectoryPath) => {
                "a literal absolute path of at most 4,096 UTF-8 bytes"
            }
            (Self::ZhCn, ExpectedValue::DirectoryPath) => "不超过 4,096 UTF-8 字节的字面绝对路径",
            (Self::EnUs, ExpectedValue::WorktreePath) => {
                "an absolute path of at most 4,096 UTF-8 bytes; Windows also accepts relative paths and ~"
            }
            (Self::ZhCn, ExpectedValue::WorktreePath) => {
                "不超过 4,096 UTF-8 字节的绝对路径；Windows 也接受相对路径及 ~"
            }
            (Self::EnUs, ExpectedValue::MatchText) => "nonempty literal text of at most 4096 bytes",
            (Self::ZhCn, ExpectedValue::MatchText) => "至多 4096 字节的非空字面文本",
            (Self::EnUs, ExpectedValue::RegexPattern) => {
                "a nonempty regular expression of at most 1024 bytes"
            }
            (Self::ZhCn, ExpectedValue::RegexPattern) => "至多 1024 字节的非空正则表达式",
            (Self::EnUs, ExpectedValue::OutputSource) => {
                "visible, recent, recent-unwrapped, or detection"
            }
            (Self::ZhCn, ExpectedValue::OutputSource) => {
                "visible、recent、recent-unwrapped 或 detection"
            }
            (Self::EnUs, ExpectedValue::OutputLines) => {
                "an unsigned decimal integer from 0 to 4294967295 (clamped to 1000)"
            }
            (Self::ZhCn, ExpectedValue::OutputLines) => {
                "0–4294967295 的无符号十进制整数（钳制到 1000）"
            }
            (Self::EnUs, ExpectedValue::TailLines) => "a decimal integer from 1 to 65535",
            (Self::ZhCn, ExpectedValue::TailLines) => "1–65535 的十进制整数",
            (Self::EnUs, ExpectedValue::AgentStartTimeout) => {
                "a decimal integer from 3001 to 300000"
            }
            (Self::ZhCn, ExpectedValue::AgentStartTimeout) => "3001–300000 的十进制整数",
            (Self::EnUs, ExpectedValue::AgentStartProvider) => {
                "a canonical interactive Agent provider id (for example codex or claude-code)"
            }
            (Self::ZhCn, ExpectedValue::AgentStartProvider) => {
                "交互式 Agent 的规范提供方 ID（如 codex 或 claude-code）"
            }
            (Self::EnUs, ExpectedValue::AgentStartArgument) => {
                "an argument without control characters"
            }
            (Self::ZhCn, ExpectedValue::AgentStartArgument) => "不含控制字符的参数",
            (Self::EnUs, ExpectedValue::WaitTimeout) => "a decimal integer from 1 to 3600000",
            (Self::ZhCn, ExpectedValue::WaitTimeout) => "1–3600000 的十进制整数",
            (Self::EnUs, ExpectedValue::SendText) => {
                "nonempty text of at most 4096 bytes including the '--enter' CR"
            }
            (Self::ZhCn, ExpectedValue::SendText) => {
                "非空文本，计入 '--enter' 的回车后不超过 4096 字节"
            }
            (Self::EnUs, ExpectedValue::SendKey) => "one of the accepted '--key' names",
            (Self::ZhCn, ExpectedValue::SendKey) => "'--key' 接受的按键名之一",
            #[cfg(windows)]
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::MachineId) => {
                "a nonzero 32-character lowercase hexadecimal machine profile ID"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::MachineId) => "非零、32 字符的小写十六进制机器配置 ID",
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::MachineLabel) => {
                "a nonblank label of at most 128 UTF-8 bytes without control characters"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::MachineLabel) => {
                "非空白、不含控制字符且最多 128 UTF-8 字节的标签"
            }
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::SshTarget) => {
                "an SSH alias, [user@]host, or ssh://[user@]host[:port] without a password"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::SshTarget) => {
                "不含密码的 SSH 别名、[user@]host 或 ssh://[user@]host[:port]"
            }
            #[cfg(windows)]
            (Self::EnUs, ExpectedValue::SiteName) => {
                "1–64 ASCII letters, digits, '.', '_' or '-'; excluding '.' and '..'"
            }
            #[cfg(windows)]
            (Self::ZhCn, ExpectedValue::SiteName) => {
                "1–64 个 ASCII 字母、数字、'.'、'_' 或 '-'；排除 '.' 和 '..'"
            }
            (Self::EnUs, ExpectedValue::MouseMode) => "'off' or 'on'",
            (Self::ZhCn, ExpectedValue::MouseMode) => "'off' 或 'on'",
            (Self::EnUs, ExpectedValue::DirectoryId) => {
                "a nonzero 32-character lowercase hexadecimal directory ID"
            }
            (Self::ZhCn, ExpectedValue::DirectoryId) => "非零的 32 位小写十六进制目录 ID",
            (Self::EnUs, ExpectedValue::WorkspaceId) => {
                "a nonzero 32-character lowercase hexadecimal workspace ID"
            }
            (Self::ZhCn, ExpectedValue::WorkspaceId) => "非零的 32 位小写十六进制 workspace ID",
            (Self::EnUs, ExpectedValue::ManifestOperationId) => {
                "a nonzero 32-character lowercase hexadecimal operation ID"
            }
            (Self::ZhCn, ExpectedValue::ManifestOperationId) => "非零的 32 位小写十六进制操作 ID",
            (Self::EnUs, ExpectedValue::CloseRequestId) => {
                "the close operation's own nonzero 32-character lowercase hexadecimal request ID"
            }
            (Self::ZhCn, ExpectedValue::CloseRequestId) => {
                "该 close 操作自身的非零 32 位小写十六进制 request ID"
            }
            (Self::EnUs, ExpectedValue::CloseGeneration) => {
                "a nonzero decimal attempt generation without a leading zero"
            }
            (Self::ZhCn, ExpectedValue::CloseGeneration) => {
                "无前导零的非零十进制 attempt generation"
            }
            (Self::EnUs, ExpectedValue::Executable) => "a nonempty absolute executable path",
            (Self::ZhCn, ExpectedValue::Executable) => "非空的绝对可执行文件路径",
            (Self::EnUs, ExpectedValue::LaunchEnvironment) => {
                "NAME=VALUE; names use ASCII letters, digits and underscores (1..128 bytes, no leading digit); at most 32 entries and 1,024 UTF-8 bytes total; no NUL, case-insensitive duplicates, ORCHBEE_*, TERM, COLORTERM or TERM_PROGRAM"
            }
            (Self::ZhCn, ExpectedValue::LaunchEnvironment) => {
                "NAME=VALUE；名称使用 ASCII 字母、数字和下划线（1..128 字节，不能以数字开头）；至多 32 项且合计不超过 1,024 UTF-8 字节；不能包含 NUL、忽略大小写后重复的名称、ORCHBEE_*、TERM、COLORTERM 或 TERM_PROGRAM"
            }
            (Self::EnUs, ExpectedValue::LaunchArguments) => {
                "at most 128 arguments totaling at most 2,048 UTF-8 bytes with '--exec'"
            }
            (Self::ZhCn, ExpectedValue::LaunchArguments) => {
                "配合 '--exec' 使用，至多 128 个参数且合计不超过 2,048 UTF-8 字节"
            }
            (Self::EnUs, ExpectedValue::AgentName) => "a name matching [a-z][a-z0-9_-]{0,31}",
            (Self::ZhCn, ExpectedValue::AgentName) => "符合 [a-z][a-z0-9_-]{0,31} 的名称",
            (Self::EnUs, ExpectedValue::AgentSessionId) => {
                "an agent session identity (1–128 ASCII letters, digits, '.', '_', ':', '-')"
            }
            (Self::ZhCn, ExpectedValue::AgentSessionId) => {
                "一个 Agent 会话身份（1–128 个 ASCII 字母、数字、'.'、'_'、':'、'-'）"
            }
            (Self::EnUs, ExpectedValue::AgentListLimit) => "a decimal integer from 1 to 100",
            (Self::ZhCn, ExpectedValue::AgentListLimit) => "1–100 的十进制整数",
            (Self::EnUs, ExpectedValue::AgentCursor) => {
                "a 'v1.<payload>.<tag>' list cursor from a previous page"
            }
            (Self::ZhCn, ExpectedValue::AgentCursor) => {
                "上一页返回的 'v1.<payload>.<tag>' 列表游标"
            }
            (Self::EnUs, ExpectedValue::AgentViewSpec) => "a bounded Agent view JSON specification",
            (Self::ZhCn, ExpectedValue::AgentViewSpec) => "有界的 Agent 视图 JSON 规格",
            (Self::EnUs, ExpectedValue::AgentViewSource) => {
                "a source of 1–120 ASCII letters, digits or :._-"
            }
            (Self::ZhCn, ExpectedValue::AgentViewSource) => {
                "1–120 个 ASCII 字母数字或 :._- 字符的来源"
            }
            (Self::EnUs, ExpectedValue::AgentWaitUntil) => {
                "one of the fourteen 'agent wait --until' conditions"
            }
            (Self::ZhCn, ExpectedValue::AgentWaitUntil) => {
                "'agent wait --until' 的十四个条件名之一"
            }
            (Self::EnUs, ExpectedValue::AgentAfterRevision) => {
                "a decimal integer from 0 to 9007199254740991"
            }
            (Self::ZhCn, ExpectedValue::AgentAfterRevision) => "0–9007199254740991 的十进制整数",
            (Self::EnUs, ExpectedValue::ProviderId) => "one of the canonical provider ids",
            (Self::ZhCn, ExpectedValue::ProviderId) => "规范 provider ID 之一",
            (Self::EnUs, ExpectedValue::AssetRootPath) => {
                "a nonempty absolute path; at most 4 may be given"
            }
            (Self::ZhCn, ExpectedValue::AssetRootPath) => "非空绝对路径；至多可给出 4 个",
            (Self::EnUs, ExpectedValue::ProfileName) => {
                "a nonempty launch profile name of at most 512 UTF-8 bytes"
            }
            (Self::ZhCn, ExpectedValue::ProfileName) => "不超过 512 UTF-8 字节的非空启动配置名称",
            (Self::EnUs, ExpectedValue::ProfileSelector) => {
                "an exact profile ID or singly-matching name of at most 512 UTF-8 bytes"
            }
            (Self::ZhCn, ExpectedValue::ProfileSelector) => {
                "精确 profile ID 或唯一匹配的名称（不超过 512 UTF-8 字节）"
            }
            (Self::EnUs, ExpectedValue::Provider) => {
                "one of the supported provider ids ('orchbee agent integration list' names them)"
            }
            (Self::ZhCn, ExpectedValue::Provider) => {
                "受支持的 provider ID 之一（'orchbee agent integration list' 列出全部）"
            }
            (Self::EnUs, ExpectedValue::AdapterSource) => {
                "a literal absolute adapter-source directory path"
            }
            (Self::ZhCn, ExpectedValue::AdapterSource) => "一个字面绝对 adapter-source 目录路径",
            (Self::EnUs, ExpectedValue::CommandText) => {
                "one non-empty UTF-8 command string within the 256 KiB paste and 64 KiB argv limits"
            }
            (Self::ZhCn, ExpectedValue::CommandText) => {
                "一个非空 UTF-8 命令字符串，受 256 KiB paste 和 64 KiB argv 限制"
            }
            (Self::EnUs, ExpectedValue::PromptText) => {
                "non-empty UTF-8 prompt within the 256 KiB paste and 64 KiB argv limits"
            }
            (Self::ZhCn, ExpectedValue::PromptText) => {
                "非空 UTF-8 提示，受 256 KiB paste 和 64 KiB argv 限制"
            }
            (Self::EnUs, ExpectedValue::SendKeySequence) => {
                "a key name, f1..f25, space or one character, optionally ctrl/alt/shift/super; at most 64"
            }
            (Self::ZhCn, ExpectedValue::SendKeySequence) => {
                "键名、f1..f25、space 或单个字符，可组合 ctrl/alt/shift/super；至多 64 个"
            }
        };
        let (problem, hint) = match reason {
            UsageReason::Invalid => return self.usage_failure().to_owned(),
            UsageReason::ArgumentEnvelope => (
                match self {
                    Self::EnUs => "Invalid invocation: the argument list must be Unicode and its total size may not exceed 64 KiB.".to_owned(),
                    Self::ZhCn => "调用方式无效：参数必须为 Unicode，且总大小不得超过 64 KiB。".to_owned(),
                },
                hint(UsageContext::Root),
            ),
            UsageReason::UnknownCommand {
                context: ctx,
                word,
            } => (
                match (self, ctx) {
                    (Self::EnUs, UsageContext::Root) => {
                        format!("Unknown command {}.", echo(word))
                    }
                    (Self::EnUs, _) => {
                        format!("Unknown command {} for '{}'.", echo(word), context(*ctx))
                    }
                    (Self::ZhCn, UsageContext::Root) => format!("未知命令 {}。", echo(word)),
                    (Self::ZhCn, _) => {
                        format!("'{}' 中未知命令 {}。", context(*ctx), echo(word))
                    }
                },
                hint(*ctx),
            ),
            UsageReason::UnknownOption { context, option } => (
                match self {
                    Self::EnUs if cfg!(windows) && (option == "--machine" || option.starts_with("--machine=")) => "The --machine routing option must be first: orchbee --machine <target> <command>.".to_owned(),
                    Self::ZhCn if cfg!(windows) && (option == "--machine" || option.starts_with("--machine=")) => "--machine 路由选项必须放在第一个位置：orchbee --machine <target> <command>。".to_owned(),
                    Self::EnUs => format!("Unknown option {}.", echo(option)),
                    Self::ZhCn => format!("未知选项 {}。", echo(option)),
                },
                hint(*context),
            ),
            UsageReason::UnexpectedOption {
                context: ctx,
                option,
            } => (
                match self {
                    Self::EnUs => {
                        format!("'{option}' is not a valid option for '{}'.", context(*ctx))
                    }
                    Self::ZhCn => {
                        format!("'{option}' 不是 '{}' 的有效选项。", context(*ctx))
                    }
                },
                hint(*ctx),
            ),
            UsageReason::DuplicateOption { option } => (
                match self {
                    Self::EnUs => format!("Option '{option}' was given more than once."),
                    Self::ZhCn => format!("选项 '{option}' 被重复提供。"),
                },
                hint(UsageContext::Root),
            ),
            UsageReason::ConflictingOptions {
                context: ctx,
                first,
                second,
            } => (
                match self {
                    Self::EnUs => {
                        format!("Options '{first}' and '{second}' cannot be combined.")
                    }
                    Self::ZhCn => format!("选项 '{first}' 和 '{second}' 不能组合使用。"),
                },
                hint(*ctx),
            ),
            UsageReason::MissingValue { option } => (
                match self {
                    Self::EnUs => format!("Option '{option}' requires a value."),
                    Self::ZhCn => format!("选项 '{option}' 需要一个值。"),
                },
                hint(UsageContext::Root),
            ),
            UsageReason::MissingRequired {
                context: ctx,
                requirement,
            } => (
                match self {
                    Self::EnUs => format!("'{}' requires {requirement}.", context(*ctx)),
                    Self::ZhCn => format!("'{}' 需要 {requirement}。", context(*ctx)),
                },
                hint(*ctx),
            ),
            UsageReason::CallerContextUnavailable => (
                match self {
                    Self::EnUs => "Calling terminal context is missing or invalid. Run inside a newly launched Orchbee terminal, or use 'panel current --panel <panel-id>'.".to_owned(),
                    Self::ZhCn => "调用终端的上下文缺失或无效。请在新启动的 Orchbee 终端内运行，或使用 'panel current --panel <panel-id>'。".to_owned(),
                },
                hint(UsageContext::Path("panel current")),
            ),
            UsageReason::UnexpectedArgument {
                context: ctx,
                value,
            } => (
                match self {
                    Self::EnUs => format!(
                        "'{}' does not take the argument {}.",
                        context(*ctx),
                        echo(value)
                    ),
                    Self::ZhCn => {
                        format!("'{}' 不接受参数 {}。", context(*ctx), echo(value))
                    }
                },
                hint(*ctx),
            ),
            UsageReason::MissingCommand { context: ctx } => (
                match (self, ctx) {
                    (Self::EnUs, UsageContext::Root) => "Missing command.".to_owned(),
                    (Self::EnUs, _) => format!("Missing command after '{}'.", context(*ctx)),
                    (Self::ZhCn, UsageContext::Root) => "缺少命令。".to_owned(),
                    (Self::ZhCn, _) => format!("'{}' 后缺少命令。", context(*ctx)),
                },
                hint(*ctx),
            ),
            UsageReason::UnknownHelpTopic { path } => (
                match self {
                    Self::EnUs => format!("No help topic matches {}.", echo(path)),
                    Self::ZhCn => format!("没有与 {} 匹配的帮助主题。", echo(path)),
                },
                hint(UsageContext::Root),
            ),
            UsageReason::MisplacedFlag {
                context: ctx,
                flag,
            } => (
                match self {
                    Self::EnUs => format!("'{flag}' is not valid in this position."),
                    Self::ZhCn => format!("'{flag}' 在此位置无效。"),
                },
                hint(*ctx),
            ),
            UsageReason::InvalidValue {
                context: ctx, option, expected: ExpectedValue::LaunchEnvironment, ..
            } => (
                match self {
                    Self::EnUs => format!("Invalid '{option}' (expected {}).", expected(ExpectedValue::LaunchEnvironment)),
                    Self::ZhCn => format!("'{option}' 无效（应为 {}）。", expected(ExpectedValue::LaunchEnvironment)),
                },
                hint(*ctx),
            ),
            UsageReason::InvalidValue {
                context: ctx,
                option,
                value,
                expected: shape,
            } => (
                match self {
                    Self::EnUs => format!(
                        "Invalid value {} for '{option}' (expected {}).",
                        echo(value),
                        expected(*shape)
                    ),
                    Self::ZhCn => format!(
                        "'{option}' 的值 {} 无效（应为 {}）。",
                        echo(value),
                        expected(*shape)
                    ),
                },
                hint(*ctx),
            ),
            UsageReason::SendSource {
                option,
                value,
                cause,
            } => {
                let named = value
                    .as_deref()
                    .map(|path| format!(" {}", echo(path)))
                    .unwrap_or_default();
                (
                    match (self, cause) {
                        (Self::EnUs, SendSourceCause::NotRegularFile) => {
                            format!("The '{option}' source{named} is not a regular file.")
                        }
                        (Self::EnUs, SendSourceCause::InteractiveTerminal) => {
                            format!(
                                "The '{option}' source is a terminal; pipe or redirect input instead."
                            )
                        }
                        (Self::EnUs, SendSourceCause::UnsupportedStream) => {
                            format!("The '{option}' source is not a readable file or pipe.")
                        }
                        (Self::EnUs, SendSourceCause::Empty) => {
                            format!("The '{option}' source{named} is empty.")
                        }
                        (Self::EnUs, SendSourceCause::Oversize { limit }) => {
                            format!(
                                "The '{option}' source{named} exceeds the {limit}-byte input bound."
                            )
                        }
                        (Self::EnUs, SendSourceCause::Unreadable) => {
                            format!("The '{option}' source{named} could not be read.")
                        }
                        (Self::ZhCn, SendSourceCause::NotRegularFile) => {
                            format!("'{option}' 源{named}不是常规文件。")
                        }
                        (Self::ZhCn, SendSourceCause::InteractiveTerminal) => {
                            format!("'{option}' 源是终端；请改用管道或重定向输入。")
                        }
                        (Self::ZhCn, SendSourceCause::UnsupportedStream) => {
                            format!("'{option}' 源不是可读的文件或管道。")
                        }
                        (Self::ZhCn, SendSourceCause::Empty) => {
                            format!("'{option}' 源{named}为空。")
                        }
                        (Self::ZhCn, SendSourceCause::Oversize { limit }) => {
                            format!("'{option}' 源{named}超过 {limit} 字节输入上限。")
                        }
                        (Self::ZhCn, SendSourceCause::Unreadable) => {
                            format!("无法读取 '{option}' 源{named}。")
                        }
                    },
                    hint(UsageContext::Path("terminal send")),
                )
            }
            UsageReason::InvalidGeometry {
                context: ctx,
                cols,
                rows,
                cell_width,
                cell_height,
            } => (
                match self {
                    Self::EnUs => format!(
                        "Geometry cols={cols} rows={rows} cell-width={cell_width} cell-height={cell_height} does not satisfy the protocol bounds (columns/rows 1-4096, cell pixel dimensions 0-4096 both zero or both nonzero, cells ≤262144, pixel extents ≤65535)."
                    ),
                    Self::ZhCn => format!(
                        "几何参数 cols={cols} rows={rows} cell-width={cell_width} cell-height={cell_height} 不满足协议约束（columns/rows 1-4096、像素维 0-4096 且须同零或同非零、格子总数 ≤262144、像素边长 ≤65535）。"
                    ),
                },
                hint(*ctx),
            ),
        };
        let suggestions = crate::cli::usage_suggestions(reason);
        let advice = if suggestions.is_empty() {
            String::new()
        } else {
            let candidates = suggestions
                .iter()
                .map(|candidate| format!("\"{candidate}\""))
                .collect::<Vec<_>>()
                .join(", ");
            match self {
                Self::EnUs => format!(" Did you mean {candidates}?"),
                Self::ZhCn => format!("是否要输入 {candidates}？"),
            }
        };
        let mut diagnostic = match self {
            Self::EnUs => format!("{problem}{advice} Run '{hint}'.\n"),
            Self::ZhCn => format!("{problem}{advice}请运行 ‘{hint}’。\n"),
        };
        if let UsageReason::MissingCommand { context } = reason {
            let children = crate::cli::usage_children(*context);
            if !children.is_empty() {
                diagnostic.push_str(self.cli_help_text(crate::localization::HelpText::Commands));
                diagnostic.push('\n');
                for (name, topic) in children {
                    diagnostic.push_str(&format!("  {name}  {}\n", self.cli_help_summary(topic)));
                }
            }
        }
        diagnostic
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

    fn cli_ambiguous(self, resource: &str, target: &str) -> String {
        let escaped = crate::cli::escape::quoted(target);
        match self {
            Self::EnUs => {
                format!("More than one {resource} matches {escaped}; use its exact ID.\n")
            }
            Self::ZhCn => format!("多个 {resource} 与 {escaped} 匹配；请使用精确 ID。\n"),
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
        match (self, stage) {
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
            (Self::EnUs, "viewer") => {
                let reason = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("reason"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                if reason.is_empty() {
                    "The terminal could not be observed for reading right now.\n".to_owned()
                } else {
                    format!("The terminal history could not be read right now ({reason}).\n")
                }
            }
            (Self::ZhCn, "viewer") => {
                let reason = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("reason"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                if reason.is_empty() {
                    "该终端当前无法进行读取观察。\n".to_owned()
                } else {
                    format!("该终端的历史当前无法读取（{reason}）。\n")
                }
            }
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

    /// `server start` service-unavailable diagnostics, routed by the failure's
    /// stage: each one states what the start actually observed.
    fn cli_site_unavailable(self, failure: &Failure) -> String {
        let stage = failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        match (self, stage) {
            (Self::EnUs, "platform") => {
                "This session operation is unavailable on this platform.\n".to_owned()
            }
            (Self::ZhCn, "platform") => "当前平台尚不支持此会话操作。\n".to_owned(),
            (Self::EnUs, "runtime-directory") => self.cli_socket_path_too_long().to_owned(),
            (Self::ZhCn, "runtime-directory") => self.cli_socket_path_too_long().to_owned(),
            (Self::EnUs, "discovery") => {
                "No running local session answered, and its runtime directory could not be created.\n"
                    .to_owned()
            }
            (Self::ZhCn, "discovery") => {
                "没有运行中的本地会话应答，且其运行时目录无法创建。\n".to_owned()
            }
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
            (Self::EnUs, _) => {
                "The local session could not admit the start probe connection.\n".to_owned()
            }
            (Self::ZhCn, _) => "本地会话无法接纳启动探针连接。\n".to_owned(),
        }
    }

    /// `server start` text answer when this invocation performed the
    /// activation and the readiness probe was admitted.
    pub(crate) const fn cli_site_started(self) -> &'static str {
        match self {
            Self::EnUs => "Local session started.\n",
            Self::ZhCn => "本地会话已启动。\n",
        }
    }

    /// `server start` text answer when a running Site admitted the probe.
    pub(crate) const fn cli_site_already_running(self) -> &'static str {
        match self {
            Self::EnUs => "Local session is already running.\n",
            Self::ZhCn => "本地会话已在运行。\n",
        }
    }

    /// `agent integration list` capsule summary line — the machine capsule
    /// state stays English; only the sentence frame is localized.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_maintenance(
        self,
        inspected: usize,
        outdated: usize,
        unresolved: &[&str],
        blocked: &[&str],
    ) -> String {
        let unresolved = if unresolved.is_empty() {
            "-".into()
        } else {
            unresolved.join(", ")
        };
        let blocked = if blocked.is_empty() {
            "-".into()
        } else {
            blocked.join(", ")
        };
        match self {
            Self::EnUs => format!(
                "Inspected {inspected}; update or repair required: {outdated}.\nUnknown inspections: {unresolved}\nConflicting or newer installations: {blocked}\nUse agent integration explain --provider <id> to inspect the repair plan.\n"
            ),
            Self::ZhCn => format!(
                "已检查 {inspected} 项；需更新或修复 {outdated} 项。\n检查未知：{unresolved}\n冲突或更新版本安装：{blocked}\n使用 agent integration explain --provider <id> 查看修复计划。\n"
            ),
        }
    }
    pub(crate) fn cli_integration_capsule_line(self, summary: &str) -> String {
        match self {
            Self::EnUs => format!("Adapter capsule: {summary}\n"),
            Self::ZhCn => format!("适配器胶囊：{summary}\n"),
        }
    }

    /// `agent integration explain` header — the fixed-field first lines.
    /// Vocabulary values stay in their canonical English spelling.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_explain_header(
        self,
        provider: &str,
        state: &str,
        availability: &str,
        issue: &str,
        capsule: &str,
    ) -> String {
        match self {
            Self::EnUs => format!(
                "Provider: {provider}\nState: {state}  availability: {availability}  issue: {issue}\nCapsule: {capsule}\n"
            ),
            Self::ZhCn => format!(
                "提供方：{provider}\n状态：{state}  可用性：{availability}  问题：{issue}\n胶囊：{capsule}\n"
            ),
        }
    }

    #[cfg(any(unix, windows))]
    pub(crate) const fn cli_integration_plan_header(self) -> &'static str {
        match self {
            Self::EnUs => "Plan:\n",
            Self::ZhCn => "计划：\n",
        }
    }

    /// `explain` when every pre-effect gate refuses the vector.
    #[cfg(any(unix, windows))]
    pub(crate) const fn cli_integration_plan_empty(self) -> &'static str {
        match self {
            Self::EnUs => "Plan: no steps — the observed state admits no enable vector.\n",
            Self::ZhCn => "计划：无步骤——当前状态不接受任何启用向量。\n",
        }
    }

    /// Mutation completed with the goal state observed.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_completed(self, direction: &str, provider: &str) -> String {
        match (self, direction) {
            (Self::EnUs, "enable") => format!("Enabled {provider}.\n"),
            (Self::EnUs, _) => format!("Disabled {provider}.\n"),
            (Self::ZhCn, "enable") => format!("已启用 {provider}。\n"),
            (Self::ZhCn, _) => format!("已禁用 {provider}。\n"),
        }
    }

    /// A pre-effect refusal — the canonical refusal code stays English.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_refused(
        self,
        direction: &str,
        provider: &str,
        reason: &str,
    ) -> String {
        match (self, direction) {
            (Self::EnUs, "enable") => {
                format!("Refused to enable {provider} ({reason}); nothing was changed.\n")
            }
            (Self::EnUs, _) => {
                format!("Refused to disable {provider} ({reason}); nothing was changed.\n")
            }
            (Self::ZhCn, "enable") => {
                format!("拒绝启用 {provider}（{reason}）；未做任何改动。\n")
            }
            (Self::ZhCn, _) => {
                format!("拒绝禁用 {provider}（{reason}）；未做任何改动。\n")
            }
        }
    }

    /// A post-effect failure — the typed code and the re-inspected state.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_failed(
        self,
        direction: &str,
        provider: &str,
        code: &str,
        observed: Option<&str>,
    ) -> String {
        let observed = observed.unwrap_or("unknown");
        match (self, direction) {
            (Self::EnUs, "enable") => {
                format!("Enable of {provider} failed ({code}); observed state: {observed}.\n")
            }
            (Self::EnUs, _) => {
                format!("Disable of {provider} failed ({code}); observed state: {observed}.\n")
            }
            (Self::ZhCn, "enable") => {
                format!("启用 {provider} 失败（{code}）；观察到的状态：{observed}。\n")
            }
            (Self::ZhCn, _) => {
                format!("禁用 {provider} 失败（{code}）；观察到的状态：{observed}。\n")
            }
        }
    }

    /// The verdict could not be determined — re-inspection never ran.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_unknown(self, direction: &str, provider: &str) -> String {
        match (self, direction) {
            (Self::EnUs, "enable") => format!(
                "Could not determine whether {provider} was enabled; the post-effect re-inspection did not run.\n"
            ),
            (Self::EnUs, _) => format!(
                "Could not determine whether {provider} was disabled; the post-effect re-inspection did not run.\n"
            ),
            (Self::ZhCn, "enable") => {
                format!("无法确定 {provider} 是否已启用；效应后复检未运行。\n")
            }
            (Self::ZhCn, _) => {
                format!("无法确定 {provider} 是否已禁用；效应后复检未运行。\n")
            }
        }
    }

    /// The termination signal fired — the invocation cancelled at a frontier
    /// boundary.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_cancelled(self, direction: &str, provider: &str) -> String {
        match (self, direction) {
            (Self::EnUs, "enable") => format!("Enable of {provider} cancelled.\n"),
            (Self::EnUs, _) => format!("Disable of {provider} cancelled.\n"),
            (Self::ZhCn, "enable") => format!("已取消启用 {provider}。\n"),
            (Self::ZhCn, _) => format!("已取消禁用 {provider}。\n"),
        }
    }

    /// The `attempted` list's leading word in text output.
    #[cfg(any(unix, windows))]
    pub(crate) const fn cli_integration_attempted_word(self) -> &'static str {
        match self {
            Self::EnUs => "attempted",
            Self::ZhCn => "已尝试",
        }
    }

    /// Header above the residue listing — retained paths are named, never
    /// silently swept.
    #[cfg(any(unix, windows))]
    pub(crate) const fn cli_integration_residue_header(self) -> &'static str {
        match self {
            Self::EnUs => "Retained residue (not removed):\n",
            Self::ZhCn => "保留的残留（未删除）：\n",
        }
    }

    /// The startup diagnostic when a derived Site socket path cannot fit one
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

    /// The text-output marker appended when `--lines` tailed emitted entries
    /// — rows, or logical lines under `--unwrap`. Distinct from the read
    /// budget's own truncation marker.
    pub(crate) fn cli_read_tail_omitted(self, omitted: usize, logical: bool) -> String {
        match (self, logical) {
            (Self::EnUs, false) => format!("… {omitted} earlier rows not shown\n"),
            (Self::EnUs, true) => format!("… {omitted} earlier lines not shown\n"),
            (Self::ZhCn, false) => format!("… 未显示前 {omitted} 行\n"),
            (Self::ZhCn, true) => format!("… 未显示前 {omitted} 个逻辑行\n"),
        }
    }

    /// The text-output marker appended when the read budget omitted frame rows.
    pub(crate) fn cli_read_truncated(self, omitted: usize) -> String {
        match self {
            Self::EnUs => format!("… {omitted} more rows truncated at the read limit\n"),
            Self::ZhCn => format!("… 因读取上限省略了 {omitted} 行\n"),
        }
    }

    // ---- agent read rendering ------------------------------------------------

    /// The notice printed instead of an empty agent table.
    #[cfg(not(windows))]
    pub(crate) const fn cli_agent_empty(self) -> &'static str {
        match self {
            Self::EnUs => "No agent sessions.\n",
            Self::ZhCn => "没有 Agent 会话。\n",
        }
    }

    /// The pagination line emitted after a non-exhaustive agent page.
    pub(crate) fn cli_agent_next_cursor(self, cursor: &str) -> String {
        match self {
            Self::EnUs => format!("More sessions exist; continue with --cursor {cursor}\n"),
            Self::ZhCn => format!("还有更多会话；使用 --cursor {cursor} 继续\n"),
        }
    }

    /// The single line `agent wait` prints on a match: session identity,
    /// the matched condition names in requested order, the satisfying
    /// revision, and the honest elapsed milliseconds — identical field
    /// order in both locales.
    pub(crate) fn cli_agent_wait_matched(
        self,
        session_id: &str,
        conditions: &str,
        revision: u64,
        elapsed_ms: u64,
    ) -> String {
        match self {
            Self::EnUs => format!(
                "session {session_id} matched {conditions} at revision {revision} after {elapsed_ms}ms\n"
            ),
            Self::ZhCn => format!(
                "会话 {session_id} 命中 {conditions}（revision {revision}，用时 {elapsed_ms}ms）\n"
            ),
        }
    }

    /// Physical matches identify a presentation and semantic sequence, without a canonical session.
    #[cfg(windows)]
    pub(crate) fn cli_agent_terminal_wait_matched(
        self,
        presentation: &str,
        conditions: &str,
        sequence: u64,
        elapsed_ms: u64,
    ) -> String {
        match self {
            Self::EnUs => format!(
                "terminal {presentation} matched {conditions} at state sequence {sequence} after {elapsed_ms}ms\n"
            ),
            Self::ZhCn => format!(
                "终端 {presentation} 命中 {conditions}（状态序号 {sequence}，用时 {elapsed_ms}ms）\n"
            ),
        }
    }

    pub(crate) fn cli_agent_seen(self, seen: Option<bool>) -> &'static str {
        match (self, seen) {
            (Self::EnUs, Some(true)) => "seen",
            (Self::EnUs, Some(false)) => "unseen",
            (Self::EnUs, None) => "unknown",
            (Self::ZhCn, Some(true)) => "已读",
            (Self::ZhCn, Some(false)) => "未读",
            (Self::ZhCn, None) => "未知",
        }
    }
    pub(crate) fn cli_agent_client_status(self, status: Option<&str>) -> &'static str {
        match (self, status) {
            (Self::ZhCn, Some("idle")) => "空闲",
            (Self::ZhCn, Some("done")) => "待查看",
            (Self::ZhCn, Some("working")) => "工作中",
            (Self::ZhCn, Some("blocked")) => "等待处理",
            (Self::ZhCn, Some("ended")) => "已结束",
            (Self::ZhCn, Some("stale")) => "已过期",
            (Self::ZhCn, Some("disconnected")) => "已断开",
            (Self::ZhCn, _) => "未知",
            (Self::EnUs, Some("idle")) => "idle",
            (Self::EnUs, Some("done")) => "done",
            (Self::EnUs, Some("working")) => "working",
            (Self::EnUs, Some("blocked")) => "blocked",
            (Self::EnUs, Some("ended")) => "ended",
            (Self::EnUs, Some("stale")) => "stale",
            (Self::EnUs, Some("disconnected")) => "disconnected",
            (Self::EnUs, _) => "unknown",
        }
    }
    pub(crate) fn cli_agent_management(
        self,
        state: &orchbee_agent_observation_protocol::AgentManagement,
    ) -> String {
        use orchbee_agent_observation_protocol::AgentManagement;
        match (self, state) {
            (Self::EnUs, AgentManagement::Unmanaged {}) => "Managed startup: unmanaged\n".into(),
            (Self::EnUs, AgentManagement::Pending { blocked, .. }) => format!(
                "Managed startup: {}\n",
                if *blocked { "blocked" } else { "pending" }
            ),
            (Self::EnUs, AgentManagement::Active { .. }) => {
                "Managed startup: interactive readiness confirmed\n".into()
            }
            (Self::ZhCn, AgentManagement::Unmanaged {}) => "托管启动：非托管\n".into(),
            (Self::ZhCn, AgentManagement::Pending { blocked, .. }) => format!(
                "托管启动：{}\n",
                if *blocked {
                    "等待处理"
                } else {
                    "启动中"
                }
            ),
            (Self::ZhCn, AgentManagement::Active { .. }) => "托管启动：已确认交互就绪\n".into(),
        }
    }
    #[cfg(windows)]
    pub(crate) fn cli_terminal_management(
        self,
        state: &orchbee_agent_observation_protocol::AgentTerminalManagement,
    ) -> String {
        use orchbee_agent_observation_protocol::AgentTerminalManagement;
        match (self, state) {
            (Self::EnUs, AgentTerminalManagement::Unmanaged {}) => {
                "Managed startup: unmanaged\n".into()
            }
            (Self::ZhCn, AgentTerminalManagement::Unmanaged {}) => "托管启动：非托管\n".into(),
            (Self::EnUs, AgentTerminalManagement::Active { provider_id }) => {
                format!("Managed startup: restored ({})\n", provider_id.as_str())
            }
            (Self::ZhCn, AgentTerminalManagement::Active { provider_id }) => {
                format!("托管启动：已恢复（{}）\n", provider_id.as_str())
            }
        }
    }
    pub(crate) fn cli_agent_runtime(self, ready: Option<bool>, reason: Option<&str>) -> String {
        match (self, ready) {
            (Self::EnUs, Some(true)) => "Prompt preflight: ready at this observation\n".into(),
            (Self::EnUs, Some(false)) => "Prompt preflight: not ready at this observation\n".into(),
            (Self::ZhCn, Some(true)) => "提示提交预检：此次观察已就绪\n".into(),
            (Self::ZhCn, Some(false)) => "提示提交预检：此次观察未就绪\n".into(),
            (Self::EnUs, None) => format!(
                "Agent runtime unavailable: {}\n",
                reason.unwrap_or("unknown")
            ),
            (Self::ZhCn, None) => format!("Agent 运行时不可用：{}\n", reason.unwrap_or("unknown")),
        }
    }

    pub(crate) fn cli_agent_client_view(
        self,
        view: &crate::cli::results::AgentClientViewDoc,
    ) -> String {
        let (seen, focused, status) = match self {
            Self::EnUs => (
                match view.seen {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => "unknown",
                },
                if view.focused { "yes" } else { "no" },
                view.status.unwrap_or("not-observed"),
            ),
            Self::ZhCn => (
                match view.seen {
                    Some(true) => "是",
                    Some(false) => "否",
                    None => "未知",
                },
                if view.focused { "是" } else { "否" },
                view.status.unwrap_or("尚未观察"),
            ),
        };
        match self {
            Self::EnUs => format!(
                "Client {}: status {status}; seen {seen}; focused {focused}\n",
                view.client_id
            ),
            Self::ZhCn => format!(
                "Client {}：状态 {status}；已看见 {seen}；聚焦 {focused}\n",
                view.client_id
            ),
        }
    }

    #[cfg(windows)]
    pub(crate) fn cli_terminal_agent_client_view(
        self,
        client: &str,
        status: Option<&str>,
        seen: Option<bool>,
    ) -> String {
        let status = self.cli_agent_client_status(status);
        let seen = self.cli_agent_seen(seen);
        match self {
            Self::EnUs => format!("Client {client}: status {status}; seen {seen}\n"),
            Self::ZhCn => format!("Client {client}：状态 {status}；已看见 {seen}\n"),
        }
    }

    /// The localized text form of one semantic Agent status, including its
    /// structured payload (attention count, active activity, activity hint).
    pub(crate) fn cli_agent_status(
        self,
        status: &orchbee_agent_observation_protocol::AgentSemanticStatus,
    ) -> String {
        use orchbee_agent_observation_protocol::AgentSemanticStatus;
        match status {
            AgentSemanticStatus::NeedsAttention { count } => match self {
                Self::EnUs => format!("needs attention ({} pending)", count.value()),
                Self::ZhCn => format!("需要注意（{} 项待处理）", count.value()),
            },
            AgentSemanticStatus::Failed {} => match self {
                Self::EnUs => "failed".to_owned(),
                Self::ZhCn => "失败".to_owned(),
            },
            AgentSemanticStatus::Succeeded {} => match self {
                Self::EnUs => "succeeded".to_owned(),
                Self::ZhCn => "成功".to_owned(),
            },
            AgentSemanticStatus::Canceled {} => match self {
                Self::EnUs => "canceled".to_owned(),
                Self::ZhCn => "已取消".to_owned(),
            },
            AgentSemanticStatus::Interrupted {} => match self {
                Self::EnUs => "interrupted".to_owned(),
                Self::ZhCn => "已中断".to_owned(),
            },
            AgentSemanticStatus::Working { activity, phase } => {
                let phase_text = self.cli_agent_phase(*phase);
                match activity {
                    Some(activity) => {
                        format!("{} ({})", phase_text, self.cli_agent_activity(*activity))
                    }
                    None => phase_text.to_owned(),
                }
            }
            AgentSemanticStatus::Idle {} => match self {
                Self::EnUs => "idle".to_owned(),
                Self::ZhCn => "空闲".to_owned(),
            },
            AgentSemanticStatus::Unknown { activity_hint } => match (self, activity_hint) {
                (Self::EnUs, Some(activity)) => {
                    format!("unknown (hint: {})", self.cli_agent_activity(*activity))
                }
                (Self::ZhCn, Some(activity)) => {
                    format!("未知（提示：{}）", self.cli_agent_activity(*activity))
                }
                (Self::EnUs, None) => "unknown".to_owned(),
                (Self::ZhCn, None) => "未知".to_owned(),
            },
            AgentSemanticStatus::Ended {} => match self {
                Self::EnUs => "ended".to_owned(),
                Self::ZhCn => "已结束".to_owned(),
            },
        }
    }

    pub(crate) const fn cli_agent_activity(
        self,
        activity: orchbee_agent_observation_protocol::AgentActivity,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentActivity;
        match (self, activity) {
            (Self::EnUs, AgentActivity::Thinking) => "thinking",
            (Self::EnUs, AgentActivity::Tool) => "tool",
            (Self::EnUs, AgentActivity::Editing) => "editing",
            (Self::EnUs, AgentActivity::Command) => "command",
            (Self::EnUs, AgentActivity::Subagent) => "subagent",
            (Self::EnUs, AgentActivity::Compacting) => "compacting",
            (Self::EnUs, AgentActivity::Retrying) => "retrying",
            (Self::EnUs, AgentActivity::Other) => "other",
            (Self::ZhCn, AgentActivity::Thinking) => "思考中",
            (Self::ZhCn, AgentActivity::Tool) => "工具调用",
            (Self::ZhCn, AgentActivity::Editing) => "编辑中",
            (Self::ZhCn, AgentActivity::Command) => "命令执行",
            (Self::ZhCn, AgentActivity::Subagent) => "子代理",
            (Self::ZhCn, AgentActivity::Compacting) => "压缩中",
            (Self::ZhCn, AgentActivity::Retrying) => "重试中",
            (Self::ZhCn, AgentActivity::Other) => "其他",
        }
    }

    pub(crate) const fn cli_agent_phase(
        self,
        phase: orchbee_agent_observation_protocol::AgentTurnPhase,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentTurnPhase;
        match (self, phase) {
            (Self::EnUs, AgentTurnPhase::Queued) => "queued",
            (Self::EnUs, AgentTurnPhase::Working) => "working",
            (Self::EnUs, AgentTurnPhase::WaitingExternal) => "waiting-external",
            (Self::ZhCn, AgentTurnPhase::Queued) => "排队中",
            (Self::ZhCn, AgentTurnPhase::Working) => "工作中",
            (Self::ZhCn, AgentTurnPhase::WaitingExternal) => "等待外部",
        }
    }

    pub(crate) const fn cli_agent_freshness(
        self,
        freshness: orchbee_agent_observation_protocol::AgentObservationFreshness,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentObservationFreshness;
        match (self, freshness) {
            (Self::EnUs, AgentObservationFreshness::Fresh) => "fresh",
            (Self::EnUs, AgentObservationFreshness::Stale) => "stale",
            (Self::EnUs, AgentObservationFreshness::Disconnected) => "disconnected",
            (Self::ZhCn, AgentObservationFreshness::Fresh) => "新鲜",
            (Self::ZhCn, AgentObservationFreshness::Stale) => "过期",
            (Self::ZhCn, AgentObservationFreshness::Disconnected) => "已断开",
        }
    }

    pub(crate) const fn cli_agent_lifecycle(
        self,
        lifecycle: orchbee_agent_observation_protocol::AgentLifecycle,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentLifecycle;
        match (self, lifecycle) {
            (Self::EnUs, AgentLifecycle::Live) => "live",
            (Self::EnUs, AgentLifecycle::Ended) => "ended",
            (Self::ZhCn, AgentLifecycle::Live) => "存活",
            (Self::ZhCn, AgentLifecycle::Ended) => "已结束",
        }
    }

    pub(crate) const fn cli_agent_capability(
        self,
        capability: orchbee_agent_observation_protocol::AgentCapability,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentCapability;
        match (self, capability) {
            (Self::EnUs, AgentCapability::SessionLifecycle) => "sessionLifecycle",
            (Self::EnUs, AgentCapability::TurnLifecycle) => "turnLifecycle",
            (Self::EnUs, AgentCapability::Questions) => "questions",
            (Self::EnUs, AgentCapability::Approvals) => "approvals",
            (Self::EnUs, AgentCapability::Authentication) => "authentication",
            (Self::EnUs, AgentCapability::Activity) => "activity",
            (Self::EnUs, AgentCapability::Subagents) => "subagents",
            (Self::EnUs, AgentCapability::Progress) => "progress",
            (Self::ZhCn, AgentCapability::SessionLifecycle) => "会话生命周期",
            (Self::ZhCn, AgentCapability::TurnLifecycle) => "轮次生命周期",
            (Self::ZhCn, AgentCapability::Questions) => "提问",
            (Self::ZhCn, AgentCapability::Approvals) => "审批",
            (Self::ZhCn, AgentCapability::Authentication) => "认证",
            (Self::ZhCn, AgentCapability::Activity) => "活动",
            (Self::ZhCn, AgentCapability::Subagents) => "子代理",
            (Self::ZhCn, AgentCapability::Progress) => "进度",
        }
    }

    pub(crate) const fn cli_agent_attention_kind(
        self,
        kind: orchbee_agent_observation_protocol::AgentAttentionKind,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentAttentionKind;
        match (self, kind) {
            (Self::EnUs, AgentAttentionKind::Question) => "question",
            (Self::EnUs, AgentAttentionKind::Approval) => "approval",
            (Self::EnUs, AgentAttentionKind::Authentication) => "authentication",
            (Self::EnUs, AgentAttentionKind::Unclassified) => "unclassified",
            (Self::ZhCn, AgentAttentionKind::Question) => "提问",
            (Self::ZhCn, AgentAttentionKind::Approval) => "审批",
            (Self::ZhCn, AgentAttentionKind::Authentication) => "认证",
            (Self::ZhCn, AgentAttentionKind::Unclassified) => "未分类",
        }
    }

    pub(crate) const fn cli_agent_outcome_kind(
        self,
        kind: orchbee_agent_observation_protocol::AgentTurnOutcomeKind,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentTurnOutcomeKind;
        match (self, kind) {
            (Self::EnUs, AgentTurnOutcomeKind::Succeeded) => "succeeded",
            (Self::EnUs, AgentTurnOutcomeKind::Failed) => "failed",
            (Self::EnUs, AgentTurnOutcomeKind::Canceled) => "canceled",
            (Self::EnUs, AgentTurnOutcomeKind::Interrupted) => "interrupted",
            (Self::ZhCn, AgentTurnOutcomeKind::Succeeded) => "成功",
            (Self::ZhCn, AgentTurnOutcomeKind::Failed) => "失败",
            (Self::ZhCn, AgentTurnOutcomeKind::Canceled) => "已取消",
            (Self::ZhCn, AgentTurnOutcomeKind::Interrupted) => "已中断",
        }
    }

    pub(crate) const fn cli_agent_visibility(
        self,
        visibility: orchbee_agent_observation_protocol::AgentPresentationVisibility,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentPresentationVisibility;
        match (self, visibility) {
            (Self::EnUs, AgentPresentationVisibility::Visible) => "visible",
            (Self::EnUs, AgentPresentationVisibility::Hidden) => "hidden",
            (Self::EnUs, AgentPresentationVisibility::Unknown) => "unknown",
            (Self::ZhCn, AgentPresentationVisibility::Visible) => "可见",
            (Self::ZhCn, AgentPresentationVisibility::Hidden) => "隐藏",
            (Self::ZhCn, AgentPresentationVisibility::Unknown) => "未知",
        }
    }

    pub(crate) const fn cli_agent_source(
        self,
        source: orchbee_agent_observation_protocol::AgentObservationSource,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentObservationSource;
        match (self, source) {
            (Self::EnUs, AgentObservationSource::Native) => "native",
            (Self::EnUs, AgentObservationSource::Hook) => "hook",
            (Self::EnUs, AgentObservationSource::Terminal) => "terminal",
            (Self::EnUs, AgentObservationSource::Heuristic) => "heuristic",
            (Self::ZhCn, AgentObservationSource::Native) => "原生",
            (Self::ZhCn, AgentObservationSource::Hook) => "钩子",
            (Self::ZhCn, AgentObservationSource::Terminal) => "终端",
            (Self::ZhCn, AgentObservationSource::Heuristic) => "启发式",
        }
    }

    pub(crate) const fn cli_agent_fidelity(
        self,
        fidelity: orchbee_agent_observation_protocol::AgentObservationFidelity,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentObservationFidelity;
        match (self, fidelity) {
            (Self::EnUs, AgentObservationFidelity::Rich) => "rich",
            (Self::EnUs, AgentObservationFidelity::Basic) => "basic",
            (Self::EnUs, AgentObservationFidelity::Heuristic) => "heuristic",
            (Self::ZhCn, AgentObservationFidelity::Rich) => "丰富",
            (Self::ZhCn, AgentObservationFidelity::Basic) => "基础",
            (Self::ZhCn, AgentObservationFidelity::Heuristic) => "启发式",
        }
    }

    pub(crate) const fn cli_agent_status_basis(
        self,
        basis: orchbee_agent_observation_protocol::AgentStatusBasis,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentStatusBasis;
        match (self, basis) {
            (Self::EnUs, AgentStatusBasis::AttentionRequest) => "attention-request",
            (Self::EnUs, AgentStatusBasis::TurnOutcome) => "turn-outcome",
            (Self::EnUs, AgentStatusBasis::ActiveTurn) => "active-turn",
            (Self::EnUs, AgentStatusBasis::AffirmativeIdle) => "affirmative-idle",
            (Self::EnUs, AgentStatusBasis::SessionEnded) => "session-ended",
            (Self::EnUs, AgentStatusBasis::LimitedActivityHint) => "limited-activity-hint",
            (Self::EnUs, AgentStatusBasis::UnobservedTurnState) => "unobserved-turn-state",
            (Self::ZhCn, AgentStatusBasis::AttentionRequest) => "注意请求",
            (Self::ZhCn, AgentStatusBasis::TurnOutcome) => "轮次结果",
            (Self::ZhCn, AgentStatusBasis::ActiveTurn) => "活跃轮次",
            (Self::ZhCn, AgentStatusBasis::AffirmativeIdle) => "确认空闲",
            (Self::ZhCn, AgentStatusBasis::SessionEnded) => "会话已结束",
            (Self::ZhCn, AgentStatusBasis::LimitedActivityHint) => "有限活动提示",
            (Self::ZhCn, AgentStatusBasis::UnobservedTurnState) => "未观察轮次状态",
        }
    }

    pub(crate) const fn cli_agent_freshness_meaning(
        self,
        meaning: orchbee_agent_observation_protocol::AgentFreshnessMeaning,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentFreshnessMeaning;
        match (self, meaning) {
            (Self::EnUs, AgentFreshnessMeaning::CurrentObservation) => "current-observation",
            (Self::EnUs, AgentFreshnessMeaning::LastObservationMayBeOutdated) => {
                "last-observation-may-be-outdated"
            }
            (Self::EnUs, AgentFreshnessMeaning::NoTrustedCurrentRoute) => {
                "no-trusted-current-route"
            }
            (Self::ZhCn, AgentFreshnessMeaning::CurrentObservation) => "当前观察",
            (Self::ZhCn, AgentFreshnessMeaning::LastObservationMayBeOutdated) => {
                "上次观察可能已过期"
            }
            (Self::ZhCn, AgentFreshnessMeaning::NoTrustedCurrentRoute) => "无可信当前路由",
        }
    }

    /// The localized boolean words used by navigable flags.
    pub(crate) const fn cli_agent_bool(self, value: bool) -> &'static str {
        match (self, value) {
            (Self::EnUs, true) => "yes",
            (Self::EnUs, false) => "no",
            (Self::ZhCn, true) => "是",
            (Self::ZhCn, false) => "否",
        }
    }

    /// The navigate-only facts line on `agent explain`: every v1 action is a
    /// navigation and provider content is never carried by the read.
    pub(crate) const fn cli_agent_navigate_only_facts(self) -> &'static str {
        match self {
            Self::EnUs => {
                "actions are navigate-only; provider content is not available through agent reads\n"
            }
            Self::ZhCn => "操作仅限导航；Agent 读取不提供提供方内容\n",
        }
    }

    /// `agent doctor` hint text by hint id — observation statements only;
    /// every hint cites the facts it was derived from, never a remedy.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_agent_doctor_hint(self, id: &str) -> &'static str {
        match (self, id) {
            (Self::EnUs, "hook-conflict") => {
                "conflicting managed-hook declarations were observed for"
            }
            (Self::ZhCn, "hook-conflict") => "观察到以下提供方存在相互矛盾的托管钩子声明：",
            (Self::EnUs, "hook-unknown") => {
                "managed-hook declarations could not be fully inspected for"
            }
            (Self::ZhCn, "hook-unknown") => "以下提供方的托管钩子声明无法完整检查：",
            (Self::EnUs, "hook-disabled") => {
                "a managed hook is declared but explicitly disabled for"
            }
            (Self::ZhCn, "hook-disabled") => "以下提供方声明了托管钩子但被显式禁用：",
            (Self::EnUs, "hook-update-required") => {
                "a managed hook declares an older contract version for"
            }
            (Self::ZhCn, "hook-update-required") => "以下提供方的托管钩子声明了较旧的契约版本：",
            (Self::EnUs, "hook-unsupported") => {
                "the hook state needs the provider's own command surface, which offline inspection does not run, for"
            }
            (Self::ZhCn, "hook-unsupported") => {
                "以下提供方的钩子状态需要其自身命令面（离线检查不执行命令）："
            }
            (Self::EnUs, "hook-present-executable-absent") => {
                "a managed hook is declared but the provider executable is absent from PATH for"
            }
            (Self::ZhCn, "hook-present-executable-absent") => {
                "以下提供方已声明托管钩子但 PATH 中不存在其可执行文件："
            }
            (Self::EnUs, "hook-absent-executable-present") => {
                "the provider executable is on PATH but no managed hook is declared for"
            }
            (Self::ZhCn, "hook-absent-executable-present") => {
                "以下提供方的可执行文件在 PATH 中但未声明托管钩子："
            }
            (Self::EnUs, "node-absent") => {
                "the hook payload requires node, which is absent from PATH, for"
            }
            (Self::ZhCn, "node-absent") => "以下提供方的钩子负载需要 node，但 PATH 中不存在 node：",
            (Self::EnUs, "endpoint-absent") => {
                "the observation endpoint socket is absent; hooks deliver only while the managed session is running."
            }
            (Self::ZhCn, "endpoint-absent") => "观察端点套接字不存在；钩子仅在托管会话运行时投递。",
            (Self::EnUs, "windows-endpoint-not-inspected") => {
                "Windows endpoint delivery is not inspected by this offline command. Managed hooks and modules support the native transport; configuration and capsule presence do not prove a working Agent connection."
            }
            (Self::ZhCn, "windows-endpoint-not-inspected") => {
                "此离线命令不检查 Windows 端点投递。托管 hook 和模块支持原生传输；检测到配置和胶囊不证明 Agent 已接通。"
            }
            (Self::EnUs, "windows-hook-transport-unavailable") => {
                "The shipped Agent hooks cannot yet deliver events on Windows. Configuration and capsule presence do not establish a working Agent connection."
            }
            (Self::ZhCn, "windows-hook-transport-unavailable") => {
                "随附的 Agent hook 尚不能在 Windows 投递事件。检测到配置和胶囊不代表 Agent 已接通。"
            }
            (Self::EnUs, "endpoint-anomaly") => {
                "the observation endpoint path exists but is not admitted by the consumers' ownership and type checks."
            }
            (Self::ZhCn, "endpoint-anomaly") => {
                "观察端点路径存在但未通过消费者的所有权与类型检查。"
            }
            (Self::EnUs, "env-override-invalid") => {
                "ORCHBEE_AGENT_OBSERVATION_SOCKET_V3 is set but is not an absolute path."
            }
            (Self::ZhCn, "env-override-invalid") => {
                "ORCHBEE_AGENT_OBSERVATION_SOCKET_V3 已设置但不是绝对路径。"
            }
            (Self::EnUs, "root-anomaly") => {
                "an asset root contradicts its own manifest declaration:"
            }
            (Self::ZhCn, "root-anomaly") => "某个资产根与其自身清单声明矛盾：",
            (Self::EnUs, "root-uninspectable") => "an asset root could not be fully inspected:",
            (Self::ZhCn, "root-uninspectable") => "某个资产根无法完整检查：",
            (Self::EnUs, "partial-document") => {
                "the document is partial: the diagnostic budget exhausted before every fact was probed."
            }
            (Self::ZhCn, "partial-document") => "文档不完整：诊断预算在探测完全部事实前已耗尽。",
            (Self::EnUs, _) => "additional facts were observed.",
            (Self::ZhCn, _) => "观察到其他事实。",
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
        match self {
            Self::EnUs => format!("{base} {}", subjects.join(", ")),
            Self::ZhCn => format!("{base}{}", subjects.join("、")),
        }
    }
}

impl Locale {
    fn cli_client_query_failure(self, kind: crate::cli::FailureKind) -> &'static str {
        use crate::cli::FailureKind;
        match (self, kind) {
            (Self::EnUs, FailureKind::Rejected) => {
                "The Client view changed or refused the read. Retry when the TUI is ready.\n"
            }
            (Self::ZhCn, FailureKind::Rejected) => {
                "Client 视图已变化或拒绝读取。请在 TUI 就绪后重试。\n"
            }
            (Self::EnUs, FailureKind::NotFound) => {
                "The selected Client was not found. Check orchbee client list.\n"
            }
            (Self::ZhCn, FailureKind::NotFound) => {
                "未找到指定 Client。请使用 orchbee client list 检查。\n"
            }
            (Self::EnUs, FailureKind::ServiceUnavailable) => {
                "The selected Client is unavailable or not ready.\n"
            }
            (Self::ZhCn, FailureKind::ServiceUnavailable) => "指定 Client 不可用或尚未就绪。\n",
            (Self::EnUs, FailureKind::Timeout) => {
                "The Client query exceeded its five-second deadline.\n"
            }
            (Self::ZhCn, FailureKind::Timeout) => "Client 查询超过了五秒期限。\n",
            (Self::EnUs, FailureKind::Cancelled) => "The Client query was cancelled.\n",
            (Self::ZhCn, FailureKind::Cancelled) => "Client 查询已取消。\n",
            (Self::EnUs, FailureKind::ProtocolInvalid) => {
                "The Client query returned inconsistent or invalid data.\n"
            }
            (Self::ZhCn, FailureKind::ProtocolInvalid) => "Client 查询返回了不一致或无效的数据。\n",
            (Self::EnUs, FailureKind::TransportLoss) => "The Client query connection was lost.\n",
            (Self::ZhCn, FailureKind::TransportLoss) => "Client 查询连接已断开。\n",
            (Self::EnUs, FailureKind::Denied) => "Access to the Client query was denied.\n",
            (Self::ZhCn, FailureKind::Denied) => "Client 查询访问被拒绝。\n",
            (Self::EnUs, _) => "The Client query could not be completed.\n",
            (Self::ZhCn, _) => "无法完成 Client 查询。\n",
        }
    }
}

impl Locale {
    /// The stderr diagnostic for a post-admission Agent read failure: every
    /// kind names the operation's own stage, and `not-found` echoes the
    /// escaped session identity instead of a resource/target pair.
    fn cli_agent_failure(self, failure: &Failure) -> String {
        use crate::cli::FailureKind;
        match failure.kind {
            FailureKind::NotFound => {
                let session = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("sessionId"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let escaped = crate::cli::escape::quoted(session);
                match self {
                    Self::EnUs => format!("No agent session matches {escaped}.\n"),
                    Self::ZhCn => format!("没有与 {escaped} 匹配的 Agent 会话。\n"),
                }
            }
            FailureKind::Denied => match self {
                Self::EnUs => "The local session denied the agent read.\n".to_owned(),
                Self::ZhCn => "本地会话拒绝了本次 Agent 读取。\n".to_owned(),
            },
            FailureKind::Rejected => {
                let reason = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("reason"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                match self {
                    Self::EnUs => {
                        format!("The local session rejected the agent list cursor ({reason}).\n")
                    }
                    Self::ZhCn => {
                        format!("本地会话拒绝了该 Agent 列表游标（{reason}）。\n")
                    }
                }
            }
            FailureKind::ServiceUnavailable => match self {
                Self::EnUs => {
                    "The agent observation runtime is unavailable right now.\n".to_owned()
                }
                Self::ZhCn => "Agent 观察运行时当前不可用。\n".to_owned(),
            },
            FailureKind::Timeout => match self {
                Self::EnUs => {
                    "The agent read did not finish within its 5-second deadline.\n".to_owned()
                }
                Self::ZhCn => "Agent 读取未能在 5 秒期限内完成。\n".to_owned(),
            },
            FailureKind::TransportLoss => match self {
                Self::EnUs => {
                    "The connection to the local session was lost during the agent read.\n"
                        .to_owned()
                }
                Self::ZhCn => "与本地会话的连接在 Agent 读取期间中断。\n".to_owned(),
            },
            FailureKind::Cancelled => match self {
                Self::EnUs => "The agent read was cancelled.\n".to_owned(),
                Self::ZhCn => "Agent 读取已被取消。\n".to_owned(),
            },
            FailureKind::ProtocolInvalid => match self {
                Self::EnUs => {
                    "The local session answered the agent read with an invalid protocol response.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地会话对本次 Agent 读取返回了无效的协议响应。\n".to_owned(),
            },
            _ => match self {
                Self::EnUs => {
                    "The CLI hit an internal failure during the agent read; no data was written.\n"
                        .to_owned()
                }
                Self::ZhCn => "CLI 在 Agent 读取期间发生内部错误；未写出任何数据。\n".to_owned(),
            },
        }
    }
}

fn failure_target(failure: &Failure) -> (&'static str, String) {
    let resource = failure
        .details
        .as_ref()
        .and_then(|details| details.get("resource"))
        .and_then(|value| value.as_str())
        .unwrap_or("project");
    let resource = match resource {
        "workspace" => "workspace",
        "tab" => "tab",
        "panel" => "panel",
        "terminal" => "terminal",
        "schema" => "schema",
        "directory" => "directory",
        "launch-profile" => "launch profile",
        _ => "project",
    };
    let target = failure
        .details
        .as_ref()
        .and_then(|details| details.get("target"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_owned();
    (resource, target)
}

// ---- help topics ---------------------------------------------------------

impl Locale {
    pub(crate) fn cli_command_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::localization::HelpDetails {
        use crate::invocation::CommandPath;
        match (self, path) {
            #[cfg(windows)]
            (_, CommandPath::EventsSubscribe | CommandPath::EventsWait) => self.events_help(path),
            #[cfg(windows)]
            (
                _,
                CommandPath::PanelGraphicsSet
                | CommandPath::PanelGraphicsClear
                | CommandPath::PanelGraphicsInfo
                | CommandPath::PanelGraphicsStream,
            ) => self.pane_graphics_help(path),
            #[cfg(windows)]
            (_, CommandPath::CommandList | CommandPath::CommandInvoke) => {
                self.configured_command_help(path)
            }
            (_, CommandPath::TerminalTitleSet | CommandPath::TerminalTitleClear) => {
                self.window_title_help(path)
            }
            #[cfg(windows)]
            (
                _,
                CommandPath::PluginLink
                | CommandPath::PluginUnlink
                | CommandPath::PluginEnable
                | CommandPath::PluginDisable
                | CommandPath::PluginList
                | CommandPath::PluginConfigDir
                | CommandPath::PluginActionList
                | CommandPath::PluginActionInvoke
                | CommandPath::PluginPaneFocus
                | CommandPath::PluginPaneClose
                | CommandPath::PopupClose
                | CommandPath::PluginPaneOpen
                | CommandPath::PluginLogList
                | CommandPath::PluginInstall
                | CommandPath::PluginUninstall,
            ) => self.plugin_help(path),
            (_, CommandPath::NotificationShow) => self.notification_help(),
            (_, CommandPath::SiteConfig | CommandPath::SiteReloadConfig) => {
                self.runtime_configuration_help(path)
            }
            (Self::EnUs, CommandPath::ClientList) => client_control::HELP_CLIENT_LIST_EN,
            (Self::ZhCn, CommandPath::ClientList) => client_control::HELP_CLIENT_LIST_ZH,
            (Self::EnUs, CommandPath::ClientGet) => client_control::HELP_CLIENT_GET_EN,
            (Self::ZhCn, CommandPath::ClientGet) => client_control::HELP_CLIENT_GET_ZH,
            (Self::EnUs, CommandPath::ClientControlStatus) => {
                client_control::HELP_CLIENT_CONTROL_STATUS_EN
            }
            (Self::ZhCn, CommandPath::ClientControlStatus) => {
                client_control::HELP_CLIENT_CONTROL_STATUS_ZH
            }
            (Self::EnUs, CommandPath::WorkspaceFocus) => client_control::HELP_WORKSPACE_FOCUS_EN,
            (Self::ZhCn, CommandPath::WorkspaceFocus) => client_control::HELP_WORKSPACE_FOCUS_ZH,
            (Self::EnUs, CommandPath::TabFocus) => client_control::HELP_TAB_FOCUS_EN,
            (Self::ZhCn, CommandPath::TabFocus) => client_control::HELP_TAB_FOCUS_ZH,
            (Self::EnUs, CommandPath::PanelResize) => client_control::HELP_PANEL_RESIZE_EN,
            (Self::ZhCn, CommandPath::PanelResize) => client_control::HELP_PANEL_RESIZE_ZH,
            (Self::EnUs, CommandPath::LayoutSetSplitRatio) => client_control::HELP_LAYOUT_RATIO_EN,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::LayoutExport) => layouts::EXPORT_EN,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::LayoutApply) => layouts::APPLY_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::LayoutApply) => layouts::APPLY_ZH,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::LayoutExport) => layouts::EXPORT_ZH,
            (Self::ZhCn, CommandPath::LayoutSetSplitRatio) => client_control::HELP_LAYOUT_RATIO_ZH,
            (Self::EnUs, CommandPath::PanelFocus) => client_control::HELP_PANEL_FOCUS_EN,
            (Self::ZhCn, CommandPath::PanelFocus) => client_control::HELP_PANEL_FOCUS_ZH,
            (Self::EnUs, CommandPath::PanelLayout) => client_layout::HELP_LAYOUT_EN,
            (Self::ZhCn, CommandPath::PanelLayout) => client_layout::HELP_LAYOUT_ZH,
            (Self::EnUs, CommandPath::PanelNeighbor) => client_layout::HELP_NEIGHBOR_EN,
            (Self::ZhCn, CommandPath::PanelNeighbor) => client_layout::HELP_NEIGHBOR_ZH,
            (Self::EnUs, CommandPath::PanelEdges) => client_layout::HELP_EDGES_EN,
            (Self::ZhCn, CommandPath::PanelEdges) => client_layout::HELP_EDGES_ZH,
            (Self::EnUs, CommandPath::PanelZoom) => client_control::HELP_PANEL_ZOOM_EN,
            (Self::ZhCn, CommandPath::PanelZoom) => client_control::HELP_PANEL_ZOOM_ZH,

            (Self::EnUs, CommandPath::Tui) => HELP_TUI_EN,
            (Self::ZhCn, CommandPath::Tui) => HELP_TUI_ZH,
            (_, CommandPath::StatusClient) => runtime_status::help(self, false),
            (_, CommandPath::StatusServer) => runtime_status::help(self, true),
            (Self::EnUs, CommandPath::Status) => HELP_STATUS_EN,
            (Self::ZhCn, CommandPath::Status) => HELP_STATUS_ZH,
            (Self::EnUs, CommandPath::ProjectList) => HELP_PROJECT_LIST_EN,
            (Self::ZhCn, CommandPath::ProjectList) => HELP_PROJECT_LIST_ZH,
            (Self::EnUs, CommandPath::ProjectGet) => HELP_PROJECT_GET_EN,
            (Self::ZhCn, CommandPath::ProjectGet) => HELP_PROJECT_GET_ZH,
            (_, CommandPath::ProjectDissolve) => organization::Help::ProjectDissolve.text(self),
            (_, CommandPath::ProjectCloseAll) => organization::Help::ProjectCloseAll.text(self),
            (_, CommandPath::ProjectCloseStatus) => {
                organization::Help::ProjectCloseStatus.text(self)
            }
            (_, CommandPath::ProjectCloseResume) => {
                organization::Help::ProjectCloseResume.text(self)
            }
            (_, CommandPath::WorkspaceSetProject) => {
                organization::Help::WorkspaceSetProject.text(self)
            }
            (_, CommandPath::WorkspaceCreateProject) => {
                organization::Help::WorkspaceCreateProject.text(self)
            }
            (_, CommandPath::WorkspaceUngroup) => organization::Help::WorkspaceUngroup.text(self),
            (Self::EnUs, CommandPath::ProjectCreate) => HELP_PROJECT_CREATE_EN,
            (Self::ZhCn, CommandPath::ProjectCreate) => HELP_PROJECT_CREATE_ZH,
            (Self::EnUs, CommandPath::ProjectRename) => HELP_PROJECT_RENAME_EN,
            (Self::ZhCn, CommandPath::ProjectRename) => HELP_PROJECT_RENAME_ZH,
            (Self::EnUs, CommandPath::WorkspaceList) => HELP_WORKSPACE_LIST_EN,
            (Self::ZhCn, CommandPath::WorkspaceList) => HELP_WORKSPACE_LIST_ZH,
            (Self::EnUs, CommandPath::WorkspaceGet) => HELP_WORKSPACE_GET_EN,
            (Self::ZhCn, CommandPath::WorkspaceGet) => HELP_WORKSPACE_GET_ZH,
            (Self::EnUs, CommandPath::WorkspaceCreate) => HELP_WORKSPACE_CREATE_EN,
            (Self::ZhCn, CommandPath::WorkspaceCreate) => HELP_WORKSPACE_CREATE_ZH,
            (Self::EnUs, CommandPath::WorkspaceMove) => HELP_WORKSPACE_MOVE_EN,
            (Self::ZhCn, CommandPath::WorkspaceMove) => HELP_WORKSPACE_MOVE_ZH,
            (Self::EnUs, CommandPath::PanelReportAgent) => panel_agent::PANELREPORTAGENT_EN,
            (Self::ZhCn, CommandPath::PanelReportAgent) => panel_agent::PANELREPORTAGENT_ZH,
            (Self::EnUs, CommandPath::PanelReportAgentSession) => {
                panel_agent::PANELREPORTAGENTSESSION_EN
            }
            (Self::ZhCn, CommandPath::PanelReportAgentSession) => {
                panel_agent::PANELREPORTAGENTSESSION_ZH
            }
            (Self::EnUs, CommandPath::PanelClearAgentAuthority) => {
                panel_agent::PANELCLEARAGENTAUTHORITY_EN
            }
            (Self::ZhCn, CommandPath::PanelClearAgentAuthority) => {
                panel_agent::PANELCLEARAGENTAUTHORITY_ZH
            }
            (Self::EnUs, CommandPath::PanelReleaseAgent) => panel_agent::PANELRELEASEAGENT_EN,
            (Self::ZhCn, CommandPath::PanelReleaseAgent) => panel_agent::PANELRELEASEAGENT_ZH,
            (Self::EnUs, CommandPath::PanelReportMetadata) => panel_metadata::HELP_EN,
            (Self::ZhCn, CommandPath::PanelReportMetadata) => panel_metadata::HELP_ZH,
            (Self::EnUs, CommandPath::WorkspaceReportMetadata) => workspace_metadata::HELP_EN,
            (Self::ZhCn, CommandPath::WorkspaceReportMetadata) => workspace_metadata::HELP_ZH,
            (Self::EnUs, CommandPath::WorkspaceMoveBlock) => HELP_WORKSPACE_MOVE_BLOCK_EN,
            (Self::ZhCn, CommandPath::WorkspaceMoveBlock) => HELP_WORKSPACE_MOVE_BLOCK_ZH,
            (Self::EnUs, CommandPath::WorkspaceRename) => HELP_WORKSPACE_RENAME_EN,
            (Self::ZhCn, CommandPath::WorkspaceRename) => HELP_WORKSPACE_RENAME_ZH,
            (Self::EnUs, CommandPath::WorkspaceAcceptDirectory) => {
                HELP_WORKSPACE_ACCEPT_DIRECTORY_EN
            }
            (Self::ZhCn, CommandPath::WorkspaceAcceptDirectory) => {
                HELP_WORKSPACE_ACCEPT_DIRECTORY_ZH
            }
            (Self::EnUs, CommandPath::WorkspaceClose) => HELP_WORKSPACE_CLOSE_EN,
            (Self::ZhCn, CommandPath::WorkspaceClose) => HELP_WORKSPACE_CLOSE_ZH,
            (Self::EnUs, CommandPath::WorkspaceCloseStatus) => HELP_WORKSPACE_CLOSE_STATUS_EN,
            (Self::ZhCn, CommandPath::WorkspaceCloseStatus) => HELP_WORKSPACE_CLOSE_STATUS_ZH,
            (Self::EnUs, CommandPath::WorkspaceCloseResume) => HELP_WORKSPACE_CLOSE_RESUME_EN,
            (Self::ZhCn, CommandPath::WorkspaceCloseResume) => HELP_WORKSPACE_CLOSE_RESUME_ZH,
            (Self::EnUs, CommandPath::TabList) => HELP_TAB_LIST_EN,
            (Self::ZhCn, CommandPath::TabList) => HELP_TAB_LIST_ZH,
            (Self::EnUs, CommandPath::TabGet) => HELP_TAB_GET_EN,
            (Self::ZhCn, CommandPath::TabGet) => HELP_TAB_GET_ZH,
            (Self::EnUs, CommandPath::TabCreate) => HELP_TAB_CREATE_EN,
            (Self::ZhCn, CommandPath::TabCreate) => HELP_TAB_CREATE_ZH,
            (Self::EnUs, CommandPath::PanelSwap) => HELP_PANEL_SWAP_EN,
            (Self::ZhCn, CommandPath::PanelSwap) => HELP_PANEL_SWAP_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::PanelCreateTerminal) => panel_terminal_creation::HELP_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::PanelCreateTerminal) => panel_terminal_creation::HELP_ZH,
            (Self::EnUs, CommandPath::PanelInput) => HELP_PANEL_INPUT_EN,
            (Self::ZhCn, CommandPath::PanelInput) => HELP_PANEL_INPUT_ZH,
            (Self::EnUs, CommandPath::PanelRename) => HELP_PANEL_RENAME_EN,
            (Self::ZhCn, CommandPath::PanelRename) => HELP_PANEL_RENAME_ZH,
            (Self::EnUs, CommandPath::TabRename) => HELP_TAB_RENAME_EN,
            (Self::ZhCn, CommandPath::TabRename) => HELP_TAB_RENAME_ZH,
            (Self::EnUs, CommandPath::TabMove) => HELP_TAB_MOVE_EN,
            (Self::ZhCn, CommandPath::TabMove) => HELP_TAB_MOVE_ZH,
            (Self::EnUs, CommandPath::TabClose) => HELP_TAB_CLOSE_EN,
            (Self::ZhCn, CommandPath::TabClose) => HELP_TAB_CLOSE_ZH,
            (Self::EnUs, CommandPath::PanelList) => HELP_PANEL_LIST_EN,
            (Self::ZhCn, CommandPath::PanelList) => HELP_PANEL_LIST_ZH,
            (Self::EnUs, CommandPath::PanelCurrent) => HELP_PANEL_CURRENT_EN,
            (Self::ZhCn, CommandPath::PanelCurrent) => HELP_PANEL_CURRENT_ZH,
            (Self::EnUs, CommandPath::PanelGet) => HELP_PANEL_GET_EN,
            (Self::ZhCn, CommandPath::PanelGet) => HELP_PANEL_GET_ZH,
            (Self::EnUs, CommandPath::PanelProcessInfo) => HELP_PANEL_PROCESS_INFO_EN,
            (Self::ZhCn, CommandPath::PanelProcessInfo) => HELP_PANEL_PROCESS_INFO_ZH,
            (Self::EnUs, CommandPath::PanelSplit) => HELP_PANEL_SPLIT_EN,
            (Self::ZhCn, CommandPath::PanelSplit) => HELP_PANEL_SPLIT_ZH,
            (Self::EnUs, CommandPath::PanelClose) => HELP_PANEL_CLOSE_EN,
            (Self::ZhCn, CommandPath::PanelClose) => HELP_PANEL_CLOSE_ZH,
            (Self::EnUs, CommandPath::PanelMove) => HELP_PANEL_MOVE_EN,
            (Self::ZhCn, CommandPath::PanelMove) => HELP_PANEL_MOVE_ZH,
            (Self::EnUs, CommandPath::TerminalList) => HELP_TERMINAL_LIST_EN,
            (Self::ZhCn, CommandPath::TerminalList) => HELP_TERMINAL_LIST_ZH,
            (Self::EnUs, CommandPath::TerminalGet) => HELP_TERMINAL_GET_EN,
            (Self::ZhCn, CommandPath::TerminalGet) => HELP_TERMINAL_GET_ZH,
            (Self::EnUs, CommandPath::TerminalAttach) => HELP_TERMINAL_ATTACH_EN,
            (Self::ZhCn, CommandPath::TerminalAttach) => HELP_TERMINAL_ATTACH_ZH,
            (Self::EnUs, CommandPath::AgentAttach) => HELP_AGENT_ATTACH_EN,
            (Self::ZhCn, CommandPath::AgentAttach) => HELP_AGENT_ATTACH_ZH,
            (Self::EnUs, CommandPath::TerminalControl) => HELP_TERMINAL_CONTROL_EN,
            (Self::ZhCn, CommandPath::TerminalControl) => HELP_TERMINAL_CONTROL_ZH,
            (Self::EnUs, CommandPath::TerminalObserve) => HELP_TERMINAL_OBSERVE_EN,
            (Self::ZhCn, CommandPath::TerminalObserve) => HELP_TERMINAL_OBSERVE_ZH,
            (Self::EnUs, CommandPath::TerminalRead) => HELP_TERMINAL_READ_EN,
            (Self::ZhCn, CommandPath::TerminalRead) => HELP_TERMINAL_READ_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalSelectionRead) => HELP_TERMINAL_SELECTION_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalSelectionRead) => HELP_TERMINAL_SELECTION_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalLinkResolve) => terminal_links::HELP_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalLinkResolve) => terminal_links::HELP_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalClear) => terminal_clear::HELP_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalClear) => terminal_clear::HELP_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalEditScrollback) => terminal_editor::HELP_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalEditScrollback) => terminal_editor::HELP_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalLinkActivate) => terminal_links::ACTIVATE_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalLinkActivate) => terminal_links::ACTIVATE_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalCopyMotion) => terminal_copy::HELP_MOTION_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalCopyMotion) => terminal_copy::HELP_MOTION_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::TerminalCopySearch) => terminal_copy::HELP_SEARCH_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::TerminalCopySearch) => terminal_copy::HELP_SEARCH_ZH,

            (Self::EnUs, CommandPath::TerminalWaitOutput) => HELP_TERMINAL_WAIT_OUTPUT_EN,
            (Self::ZhCn, CommandPath::TerminalWaitOutput) => HELP_TERMINAL_WAIT_OUTPUT_ZH,
            (Self::EnUs, CommandPath::TerminalRun) => HELP_TERMINAL_RUN_EN,
            (Self::ZhCn, CommandPath::TerminalRun) => HELP_TERMINAL_RUN_ZH,
            (Self::EnUs, CommandPath::TerminalSend) => HELP_TERMINAL_SEND_EN,
            (Self::ZhCn, CommandPath::TerminalSend) => HELP_TERMINAL_SEND_ZH,
            (Self::EnUs, CommandPath::WorktreeList) => HELP_WORKTREE_LIST_EN,
            (Self::ZhCn, CommandPath::WorktreeList) => HELP_WORKTREE_LIST_ZH,
            (
                _,
                path @ (CommandPath::WorktreeCreate
                | CommandPath::WorktreeOpen
                | CommandPath::WorktreeRemove
                | CommandPath::WorktreeStatus
                | CommandPath::WorktreeResume),
            ) => self.cli_worktree_help(path),
            (Self::EnUs, CommandPath::LaunchProfileList) => HELP_LAUNCH_PROFILE_LIST_EN,
            (Self::ZhCn, CommandPath::LaunchProfileList) => HELP_LAUNCH_PROFILE_LIST_ZH,
            (Self::EnUs, CommandPath::LaunchProfileGet) => HELP_LAUNCH_PROFILE_GET_EN,
            (Self::ZhCn, CommandPath::LaunchProfileGet) => HELP_LAUNCH_PROFILE_GET_ZH,
            (Self::EnUs, CommandPath::LaunchProfileCreate) => HELP_LAUNCH_PROFILE_CREATE_EN,
            (Self::ZhCn, CommandPath::LaunchProfileCreate) => HELP_LAUNCH_PROFILE_CREATE_ZH,
            (Self::EnUs, CommandPath::LaunchProfileDelete) => HELP_LAUNCH_PROFILE_DELETE_EN,
            (Self::ZhCn, CommandPath::LaunchProfileDelete) => HELP_LAUNCH_PROFILE_DELETE_ZH,
            (Self::EnUs, CommandPath::AgentChanges) => crate::localization::HelpDetails::new(
                "orchbee agent changes [--cursor <c>] [--limit <n>] [--format text|json]",
                "Windows: read one bounded Agent change page (limit 1-100, default 50).\nOmit cursor to establish a current-head checkpoint, then reuse its cursor.\nreset-required means history was lost; rebuild your baseline. A previous\nsession boot or invalid cursor is rejected. No subscription or automatic retry.\nTotal read deadline: 5 seconds. JSON schema: orchbee.agent.changes.",
            ),
            (Self::ZhCn, CommandPath::AgentChanges) => crate::localization::HelpDetails::new(
                "orchbee agent changes [--cursor <c>] [--limit <n>] [--format text|json]",
                "Windows：读取一页有界 Agent 变更（limit 1-100，默认 50）。省略 cursor 建立当前头部检查点，再使用其 cursor 继续观察。\nreset-required 表示历史丢失，必须重建基线；先前会话启动实例或无效\ncursor 会被拒绝。不订阅、不自动重试。总读取期限 5 秒。\nJSON schema：orchbee.agent.changes。",
            ),
            #[cfg(windows)]
            (
                locale,
                CommandPath::AgentViewGet | CommandPath::AgentViewSet | CommandPath::AgentViewClear,
            ) => locale.agent_view_help(path),
            (Self::EnUs, CommandPath::AgentList) => HELP_AGENT_LIST_EN,
            (Self::ZhCn, CommandPath::AgentList) => HELP_AGENT_LIST_ZH,
            (Self::EnUs, CommandPath::AgentStart) => crate::localization::HelpDetails::new(
                "orchbee agent start <name> --kind <provider> [--panel <id|@panel:suffix>|--current] [--timeout <ms>] [--format text|json] [-- <args>...]",
                "Windows: start an interactive Agent in an idle terminal, then bind its name after verified readiness.\nThe default target is the caller Panel. Timeout: 3001–300000 ms, default 30000.\nUse --arg=<value> repeatedly as an alternative to trailing arguments. Input is submitted once.\nCancellation or timeout releases the name when confirmed; it never kills the Agent.\nProvider hooks must report readiness. Use agent integration and agent doctor to check setup.\nFor Antigravity, use --kind antigravity here and --provider antigravity-cli in integration/doctor.",
            ),
            (Self::ZhCn, CommandPath::AgentStart) => crate::localization::HelpDetails::new(
                "orchbee agent start <name> --kind <provider> [--panel <id|@panel:suffix>|--current] [--timeout <ms>] [--format text|json] [-- <args>...]",
                "Windows：在空闲终端启动交互式 Agent，确认就绪后绑定名称。默认目标为调用者 Panel。超时范围 3001–300000 毫秒，默认 30000。可重复使用 --arg=<value> 代替尾随参数。启动输入只提交一次。取消或超时在确认后释放名称，但不会终止 Agent。提供方 hook 必须能报告就绪；可用 agent integration 和 agent doctor 检查配置。\nAntigravity 在此使用 --kind antigravity，在 integration/doctor 中使用 --provider antigravity-cli。",
            ),
            (Self::EnUs, CommandPath::AgentPrompt) => crate::localization::HelpDetails::new(
                "orchbee agent prompt <target> <text> [--wait] [--until <condition>]... [--timeout <ms> | --forever] [--format text|json]",
                "Send a prompt to one running Agent. Use --wait to observe its next state.\nFind targets with orchbee agent list; use --panel to select an Agent by its Panel.\n--until, --timeout and --forever require --wait. Inspect the terminal before retrying uncertain input.",
            ),
            (Self::ZhCn, CommandPath::AgentPrompt) => crate::localization::HelpDetails::new(
                "orchbee agent prompt <target> <text> [--wait] [--until <condition>]... [--timeout <ms> | --forever] [--format text|json]",
                "向正在运行的 Agent 发送提示。使用 --wait 观察其后续状态。\n用 orchbee agent list 查找目标，或用 --panel 按面板选择 Agent。\n--until、--timeout 和 --forever 要求 --wait。投递结果不确定时，重试前先检查终端。",
            ),
            (Self::EnUs, CommandPath::AgentRead) if cfg!(windows) => HELP_AGENT_OUTPUT_EN,
            (Self::ZhCn, CommandPath::AgentRead) if cfg!(windows) => HELP_AGENT_OUTPUT_ZH,
            (Self::EnUs, CommandPath::AgentRead) => crate::localization::HelpDetails::new(
                "orchbee agent read <target> [--history --rows <n>] [--unwrap] [--lines <n>] [--format text|json]",
                "Read the terminal bound to one live Agent. Target: Session ID/name/Panel ID, --agent-session <id>,\n--name <alias>, or --panel <id|@panel:suffix>. Panel selects its primary terminal;\notherwise exactly one navigable binding is required. Uses terminal\nread/history envelopes; history excludes visible rows.\n\nExplicit sources: --source visible|recent|recent-unwrapped|detection.\nRecent includes history and live screen (default 80 physical rows); detection\nuses one screen height near the cursor, not Agent state. Source-mode --lines\naccepts 0..4294967295 and clamps to 1000. Visible is this viewer's live screen.\n--plain prints UTF-8 text directly and defaults to recent; it conflicts with\n--format json. Source/plain cannot mix with --history, --rows or --unwrap.\nJSON uses orchbee.terminal.output; a stale sample fails without a hidden retry.\n\n--ansi exports styled text directly (default recent); --format json instead\nuses orchbee.terminal.output v2 with encoding: ansi. It conflicts with\n--plain, --history, --rows and --unwrap. Styled blanks remain content;\nrecent-unwrapped joins soft wraps. Detection stays plain text. This is a\nstyled snapshot with SGR, not original PTY bytes or graphics replay.",
            ),
            (Self::ZhCn, CommandPath::AgentRead) => crate::localization::HelpDetails::new(
                "orchbee agent read <target> [--history --rows <n>] [--unwrap] [--lines <n>] [--format text|json]",
                "读取一个存活 Agent 绑定的终端。目标：Session ID/名称/Panel ID、--agent-session <id>、--name <alias>\n或 --panel <id|@panel:suffix>。Panel 选择主终端。未指定 Panel 时必须有且仅有一个可导航绑定。复用终端 read/history 封套；历史不含可见行。\n\n显式来源：--source visible|recent|recent-unwrapped|detection。\nrecent 合并历史与实时屏幕，默认 80 个物理行；detection 读取光标附近一个屏幕高度，不是 Agent 状态。来源模式 --lines 接受 0..4294967295，钳制到 1000。visible 是当前查看器的实时屏幕。\n--plain 直接输出 UTF-8 文本，默认 recent；与 --format json 冲突。来源/plain 不能与 --history、--rows 或 --unwrap 混用。\nJSON 使用 orchbee.terminal.output；样本过期时失败，不隐藏重试。\n\n--ansi 直接导出带样式文本（默认 recent）；配合 --format json 时改用\norchbee.terminal.output v2，encoding 为 ansi。它与 --plain、--history、\n--rows 和 --unwrap 冲突。保留带样式空格；recent-unwrapped 拼接软换行。\ndetection 仍输出纯文本。这是包含 SGR 的样式快照，不是原始 PTY 字节或图形重放。",
            ),
            (Self::EnUs, CommandPath::AgentSendKeys) => crate::localization::HelpDetails::new(
                "orchbee agent send-keys <target> <key>... [--format text|json]",
                "Send 1..64 semantic keys or combinations (enter, ctrl-c, alt+left, ctrl+shift+a)\nto one live Agent terminal. Target: Session ID/name/Panel ID, --agent-session <id>, --name <alias>,\nor --panel <id|@panel:suffix>. Panel selects its primary terminal.\nUses orchbee.terminal.send receipts. Never replays uncertain input.",
            ),
            (Self::ZhCn, CommandPath::AgentSendKeys) => crate::localization::HelpDetails::new(
                "orchbee agent send-keys <target> <key>... [--format text|json]",
                "向一个存活 Agent 终端发送 1..64 个语义按键或组合（enter、ctrl-c、alt+left、ctrl+shift+a）。目标：Session ID/名称/Panel ID、--agent-session <id>、--name <alias>\n或 --panel <id|@panel:suffix>。Panel 选择主终端。使用 orchbee.terminal.send 回执。不重放不确定输入。",
            ),
            (Self::EnUs, CommandPath::AgentAttention) => client_control::HELP_AGENT_ATTENTION_EN,
            (Self::ZhCn, CommandPath::AgentAttention) => client_control::HELP_AGENT_ATTENTION_ZH,
            (Self::EnUs, CommandPath::AgentFocus) => client_control::HELP_AGENT_FOCUS_EN,
            (Self::ZhCn, CommandPath::AgentFocus) => client_control::HELP_AGENT_FOCUS_ZH,
            (Self::EnUs, CommandPath::AgentGet) => HELP_AGENT_GET_EN,
            (Self::ZhCn, CommandPath::AgentGet) => HELP_AGENT_GET_ZH,
            (Self::EnUs, CommandPath::AgentExplain) => HELP_AGENT_EXPLAIN_EN,
            (Self::ZhCn, CommandPath::AgentExplain) => HELP_AGENT_EXPLAIN_ZH,
            (Self::EnUs, CommandPath::AgentNames) => crate::localization::HelpDetails::new(
                "orchbee agent names [--format text|json]",
                "List this session runtime's live Agent names. Names disappear when their session ends or the session stops.",
            ),
            (Self::ZhCn, CommandPath::AgentNames) => crate::localization::HelpDetails::new(
                "orchbee agent names [--format text|json]",
                "列出当前会话运行时中的 Agent 名称。会话结束或会话停止后名称会释放。",
            ),
            (Self::EnUs, CommandPath::AgentResolve) => crate::localization::HelpDetails::new(
                "orchbee agent resolve <target> [--format text|json]\n       orchbee agent resolve --agent-session <id> | --name <alias> | --panel <id|@panel:suffix>",
                "Resolve an exact Session ID first, then a runtime name, then a Panel ID.\nA Panel selects its primary terminal and requires one live Agent. Use --name to select an alias that collides with an ID.",
            ),
            (Self::ZhCn, CommandPath::AgentResolve) => crate::localization::HelpDetails::new(
                "orchbee agent resolve <target> [--format text|json]\n      orchbee agent resolve --agent-session <id> | --name <alias> | --panel <id|@panel:suffix>",
                "优先按完整 Session ID 定位，再查运行时名称，最后回退到 Panel ID。\nPanel 选择主终端，要求唯一存活 Agent。名称与 ID 冲突时，用 --name 明确选择名称。",
            ),
            (Self::EnUs, CommandPath::AgentRename) => crate::localization::HelpDetails::new(
                "orchbee agent rename <target> <new-name> [--format text|json]\n       orchbee agent rename <target> --clear",
                "Assign or atomically rename one live Agent. Name: [a-z][a-z0-9_-]{0,31}. Duplicate names are rejected. --clear releases only the name. Replace <target> with --agent-session <id>, --name <alias>, or --panel <id|@panel:suffix> to select explicitly. Names reset when the session ends or the session stops.",
            ),
            (Self::ZhCn, CommandPath::AgentRename) => crate::localization::HelpDetails::new(
                "orchbee agent rename <target> <new-name> [--format text|json]\n      orchbee agent rename <target> --clear",
                "为存活 Agent 命名或原子重命名。名称格式为 [a-z][a-z0-9_-]{0,31}，拒绝重名。--clear 只释放名称。可将 <target> 替换成 --agent-session <id> 或 --name <alias>。会话结束或会话停止后名称释放。",
            ),
            (Self::EnUs, CommandPath::AgentWait) => {
                if cfg!(windows) {
                    HELP_AGENT_WAIT_WINDOWS_EN
                } else {
                    HELP_AGENT_WAIT_EN
                }
            }
            (Self::ZhCn, CommandPath::AgentWait) => {
                if cfg!(windows) {
                    HELP_AGENT_WAIT_WINDOWS_ZH
                } else {
                    HELP_AGENT_WAIT_ZH
                }
            }
            (Self::EnUs, CommandPath::AgentDoctor) => HELP_AGENT_DOCTOR_EN,
            (Self::ZhCn, CommandPath::AgentDoctor) => HELP_AGENT_DOCTOR_ZH,
            (Self::EnUs, CommandPath::AgentIntegrationList) => HELP_AGENT_INTEGRATION_LIST_EN,
            (Self::ZhCn, CommandPath::AgentIntegrationList) => HELP_AGENT_INTEGRATION_LIST_ZH,
            (Self::EnUs, CommandPath::AgentIntegrationExplain) => HELP_AGENT_INTEGRATION_EXPLAIN_EN,
            (Self::ZhCn, CommandPath::AgentIntegrationExplain) => HELP_AGENT_INTEGRATION_EXPLAIN_ZH,
            (Self::EnUs, CommandPath::AgentIntegrationEnable) => HELP_AGENT_INTEGRATION_ENABLE_EN,
            (Self::ZhCn, CommandPath::AgentIntegrationEnable) => HELP_AGENT_INTEGRATION_ENABLE_ZH,
            (Self::EnUs, CommandPath::AgentIntegrationDisable) => HELP_AGENT_INTEGRATION_DISABLE_EN,
            (Self::ZhCn, CommandPath::AgentIntegrationDisable) => HELP_AGENT_INTEGRATION_DISABLE_ZH,
            (Self::EnUs, CommandPath::ApiSnapshot) => HELP_API_SNAPSHOT_EN,
            (Self::ZhCn, CommandPath::ApiSnapshot) => HELP_API_SNAPSHOT_ZH,
            (Self::EnUs, CommandPath::ApiSchema) => HELP_API_SCHEMA_EN,
            (Self::ZhCn, CommandPath::ApiSchema) => HELP_API_SCHEMA_ZH,
            #[cfg(windows)]
            (Self::EnUs, CommandPath::ConfigImportHerdr) => HELP_CONFIG_IMPORT_EN,
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::ConfigImportHerdr) => HELP_CONFIG_IMPORT_ZH,
            (Self::EnUs, CommandPath::ConfigResetKeys) => HELP_CONFIG_RESET_KEYS_EN,
            (Self::ZhCn, CommandPath::ConfigResetKeys) => HELP_CONFIG_RESET_KEYS_ZH,
            #[cfg(windows)]
            (_, CommandPath::MachineList) => self.cli_machine_help(path),
            #[cfg(windows)]
            (_, CommandPath::MachineAdd) => self.cli_machine_help(path),
            #[cfg(windows)]
            (_, CommandPath::MachineRename) => self.cli_machine_help(path),
            #[cfg(windows)]
            (_, CommandPath::MachineRemove) => self.cli_machine_help(path),
            #[cfg(windows)]
            (_, CommandPath::MachineEnable) => self.cli_machine_help(path),
            #[cfg(windows)]
            (_, CommandPath::MachineDisable) => self.cli_machine_help(path),
            #[cfg(windows)]
            (Self::EnUs, CommandPath::ChannelShow) => crate::localization::HelpDetails::new(
                "orchbee channel show [--format text|json]",
                "Read the Windows update.channel preference (stable or preview). Defaults to stable.\nValidates the complete config; a missing explicit ORCHBEE_CONFIG_FILE fails.\nDoes not write files, connect to a session, download, or change the installed cohort.",
            ),
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::ChannelShow) => crate::localization::HelpDetails::new(
                "orchbee channel show [--format text|json]",
                "读取 Windows update.channel 偏好（stable 或 preview），默认 stable。校验完整配置；显式 ORCHBEE_CONFIG_FILE 不存在时失败。不写文件、不连接会话、不下载，也不改变已安装 cohort。",
            ),
            #[cfg(windows)]
            (Self::EnUs, CommandPath::ChannelSet) => crate::localization::HelpDetails::new(
                "orchbee channel set <stable|preview> [--format text|json]",
                "Save the channel and immediately run update. Update failure retains the saved channel.\nUses ORCHBEE_UPDATE_MANIFEST; see orchbee update --help.",
            ),
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::ChannelSet) => crate::localization::HelpDetails::new(
                "orchbee channel set <stable|preview> [--format text|json]",
                "保存频道后立即更新；更新失败仍保留已保存的频道。使用 ORCHBEE_UPDATE_MANIFEST；参见 orchbee update --help。",
            ),
            #[cfg(windows)]
            (Self::EnUs, CommandPath::Update) => crate::localization::HelpDetails::new(
                "orchbee update [--format text|json]",
                "Install the configured channel as a complete immutable Windows cohort.\nORCHBEE_UPDATE_MANIFEST: explicit HTTPS URL or absolute local manifest path; {channel} expands to stable or preview.\nORCHBEE_INSTALL_DIR defaults to %LOCALAPPDATA%/Programs/Orchbee.\nValidates every file, publishes a release, then updates user PATH. Existing sessions keep their version.\nTotal deadline: 600 seconds. Ctrl+C cancels; receipts retain published paths and cleanup obligations.",
            ),
            #[cfg(windows)]
            (Self::ZhCn, CommandPath::Update) => crate::localization::HelpDetails::new(
                "orchbee update [--format text|json]",
                "按配置的频道安装完整、独立的 Windows cohort。\nORCHBEE_UPDATE_MANIFEST：显式 HTTPS URL 或本地清单绝对路径；{channel} 替换为 stable 或 preview。\nORCHBEE_INSTALL_DIR 默认为 %LOCALAPPDATA%/Programs/Orchbee。逐文件校验并发布版本，再更新用户 PATH；现有会话保留原版本。总时限 600 秒。Ctrl+C 取消；回执保留已发布路径和待清理项。",
            ),
            (Self::EnUs, CommandPath::ConfigCheck) => HELP_CONFIG_CHECK_EN,
            (Self::ZhCn, CommandPath::ConfigCheck) => HELP_CONFIG_CHECK_ZH,
            #[cfg(windows)]
            (
                _,
                path @ (CommandPath::SessionList
                | CommandPath::SessionAttach
                | CommandPath::SessionStop
                | CommandPath::SessionDelete),
            ) => self.cli_session_command_help(path),
            (Self::EnUs, CommandPath::SiteStart) => HELP_SITE_START_EN,
            (Self::ZhCn, CommandPath::SiteStart) => HELP_SITE_START_ZH,
            (
                _,
                CommandPath::SiteAgentManifests
                | CommandPath::SiteUpdateAgentManifests
                | CommandPath::SiteReloadAgentManifests
                | CommandPath::SiteAgentManifestsStatus,
            ) => self.cli_agent_manifests_help(path),
            (Self::EnUs, CommandPath::SiteDiagnose) => HELP_SITE_DIAGNOSE_EN,
            (Self::ZhCn, CommandPath::SiteDiagnose) => HELP_SITE_DIAGNOSE_ZH,
            (Self::EnUs, CommandPath::SiteStop) => HELP_SITE_STOP_EN,
            (Self::ZhCn, CommandPath::SiteStop) => HELP_SITE_STOP_ZH,
            (Self::EnUs, CommandPath::SiteStopStatus) => HELP_SITE_STOP_STATUS_EN,
            (Self::ZhCn, CommandPath::SiteStopStatus) => HELP_SITE_STOP_STATUS_ZH,
        }
    }

    /// A localized note line for one `server diagnose` advisory token; an
    /// unrecognized token renders itself verbatim — the token is the
    /// machine field, the line is only its human gloss.
    pub(crate) fn cli_site_diagnose_advisory(self, advisory: &str) -> &'static str {
        match (self, advisory) {
            (Self::EnUs, "activation-recorded") => {
                "note: activation recorded; if no existing session can be joined, fresh activation is blocked by current recovery policy.\n"
            }
            (Self::ZhCn, "activation-recorded") => {
                "注：已记录一次激活；若无可加入的现有会话，按当前恢复策略新的激活将被阻止。\n"
            }
            (Self::EnUs, "socket-residue") => {
                "note: client/worker socket residue observed; if no existing session can be joined, fresh activation is blocked by current recovery policy.\n"
            }
            (Self::ZhCn, "socket-residue") => {
                "注：观察到 client/worker 套接字残留；若无可加入的现有会话，按当前恢复策略新的激活将被阻止。\n"
            }
            (Self::EnUs, "pipe-name-present") => {
                "note: a client/worker pipe name was observed without connecting or authenticating its owner; if no session can be joined, current recovery policy blocks fresh activation.\n"
            }
            (Self::ZhCn, "pipe-name-present") => {
                "注：观察到 client/worker 管道名称，未连接或认证其属主；若无可加入的会话，当前恢复策略将阻止重新激活。\n"
            }
            (Self::EnUs, _) => "note: unrecognized advisory.\n",
            (Self::ZhCn, _) => "注：未识别的通告。\n",
        }
    }
}

const HELP_TUI_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tui [--mouse on|off]",
    "Open the terminal interface on Windows or Linux. Running orchbee without a command does the same. Mouse capture defaults to on; --mouse is accepted on this explicit tui route.",
);
const HELP_TUI_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tui [--mouse on|off]",
    "在 Windows 或 Linux 上打开终端界面。不带命令运行 orchbee 也会打开该界面。鼠标捕获默认开启；显式 tui 路由支持 --mouse 选项。",
);

const HELP_STATUS_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee status [--format text|json]",
    "Read the running session's reachability and resource counts. This command does not create or start a session. Use status server for service compatibility details, or status client to inspect this executable offline.",
);
const HELP_STATUS_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee status [--format text|json]",
    "读取正在运行的会话的可达性和资源数量。本命令不创建或启动会话。使用 status server 查看服务兼容性，或使用 status client 离线检查当前程序。",
);

const HELP_TERMINAL_OBSERVE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>] [--format text|json]",
        "Windows: observe one existing terminal as NDJSON (orchbee.terminal.stream v2).\nTarget: complete terminal ID or @terminal:<suffix>. Default viewport: 120x40;\ndimensions: 1..4096, at most 262144 cells. Crops/pads without resizing the source.\nFrames contain full safe ANSI screens encoded as base64, not raw PTY output.\nThe stream does not read stdin or acquire control. Both formats emit NDJSON.\nNo total timeout by default; --timeout: 1..3600000 ms. Ctrl+C exits 130; timeout\nexits 7. Each transport/output stall is bounded to five seconds; cleanup may\ntake two seconds. A disconnected source is a failure, not a successful exit.",
    );
const HELP_TERMINAL_OBSERVE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>] [--format text|json]",
        "Windows：以 NDJSON 持续观察一个已有终端（orchbee.terminal.stream v2）。目标：完整终端 ID 或 @terminal:<suffix>。默认视口为 120x40；单边尺寸为\n1..4096，总计最多 262144 个单元格。仅裁剪或填充，不改变源终端大小。帧包含以 base64 编码的完整安全 ANSI 屏幕，不是原始 PTY 输出。不读取 stdin，也不获取控制权。两种格式均输出 NDJSON。默认没有总超时；--timeout 范围为 1..3600000 毫秒。Ctrl+C 退出码为 130，超时为 7。每次传输或输出停滞最多五秒；清理可能需要两秒。源连接断开视为失败，不会报告成功退出。",
    );

#[cfg(windows)]
const HELP_SITE_START_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee server start [--format text|json]",
    "Ensure the selected local session is running within one 20-second budget.\nTUI startup uses the same activation and recovery path. Complete catalog v2/v3\nregistration permits automatic cold recovery only after all exact PID/creation\nidentities are gone, locks are free and endpoints absent. Background and\nclean-stop checkpoints restore layouts, Launch Profiles, sampled per-terminal\ncwd and unique provider session references. Unreadable snapshots are retained;\nstartup falls back to an older valid copy or opens an empty workspace and warns\non stderr. Live Host, unknown/incomplete identity and legacy Active records\nrefuse without deleting data. Run server diagnose with the same --session selection\nor use orchbee --session recovery-work server start for another name. New activation\npublishes catalog v3 before checkpoint storage; binaries limited to v1/v2 cannot\noperate v3 sessions, even after clean stop. Live Host adoption remains separate.\n\nJSON uses orchbee.site.start: started means this invocation spawned Server;\nalready-running means an existing session admitted it. Resume providers is enabled\nby default; ORCHBEE_RESUME_AGENTS_ON_RESTORE accepts only true/false. Live Agent\nstate, terminal history and screen contents are not restored.",
);
#[cfg(not(windows))]
const HELP_SITE_START_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee server start [--format text|json]",
    "Ensure the implicit per-user local session is running. When no admitted\nsession answers, this invocation performs the existing activation sequence\n— the same locks, marker, Server spawn, and readiness admission the TUI\nstartup uses — and reports 'started'. On Windows, background and clean-stop\ncheckpoints restore layouts, Launch Profiles, sampled per-terminal cwd and\nunique provider session references. Provider resume is enabled by default; set\nORCHBEE_RESUME_AGENTS_ON_RESTORE=false before activation to restore shells\nwithout resuming providers. Only true/false are accepted. Unreadable snapshots\nare retained; startup falls back to an older valid copy or opens an empty\nworkspace and warns on stderr. Live Agent state, terminal history and screen\ncontents are not restored.\nWhen a running session admits the readiness probe,\nit reports 'already-running'. One absolute 20-second budget covers the whole\ninvocation.\n\n'--format json' emits the orchbee.site.start envelope. An interrupted\nor timed-out start may leave activation state behind; a later invocation\nthen answers refused with a recovery-required reason. The command never\nrepairs, stops, or deletes runtime state.",
);
#[cfg(windows)]
const HELP_SITE_START_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee server start [--format text|json]",
    "在一个 20 秒总预算内确保选中的本地会话正在运行。TUI 使用相同激活和恢复路径。仅当清单 v2/v3 登记完整、所有确切 PID／创建时间身份消失、锁空闲且端点缺席时，才自动冷恢复。后台及正常停止 checkpoint 会恢复布局、Launch Profile、采样所得的逐终端 cwd 和唯一 provider 会话引用。不可读存档会保留；启动会回退到较早的有效副本，或打开空工作区并在 stderr 警告。活 Host、未知／不完整身份和旧版\nActive 记录均拒绝且不删除数据。请用相同 --session 选择执行 server diagnose，或用\norchbee --session recovery-work server start 另开命名会话。新激活会先发布 catalog v3\n再使用检查点存储；只支持 v1/v2 的二进制即使在正常停止后也不能操作 v3 会话。活 Host 接管仍另行实现。\n\nJSON 使用 orchbee.site.start：started 表示本次拉起了 Server；\nalready-running 表示既有会话准入了本次调用。默认恢复 provider；\nORCHBEE_RESUME_AGENTS_ON_RESTORE 仅接受 true／false。不恢复存活 Agent 状态、终端历史或屏幕内容。",
);
#[cfg(not(windows))]
const HELP_SITE_START_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee server start [--format text|json]",
    "确保隐式的每用户本地会话正在运行。若没有已准入的会话应答，本次调用执行既有的激活序列——与 TUI 启动相同的锁、标记、Server 拉起与就绪准入——并报告 'started'。Windows 会从后台及正常停止 checkpoint 恢复布局、Launch Profile、采样所得的逐终端 cwd 和唯一 provider 会话引用。默认启用\nprovider 恢复；激活前设置 ORCHBEE_RESUME_AGENTS_ON_RESTORE=false 可只恢复 shell。该变量仅接受 true／false。不可读存档会保留；启动会回退到较早的有效副本，或打开空工作区并在 stderr 警告。不恢复存活 Agent 状态、终端历史或屏幕内容。若运行中的会话准入了就绪探针，则报告 'already-running'。整个调用共享一个 20 秒绝对预算。\n\n'--format json' 输出 orchbee.site.start 封套。被中断或超时的启动可能遗留激活状态；后续调用将以拒绝加 recovery-required 原因应答。本命令绝不修复、停止或删除运行时状态。",
);

const HELP_AGENT_LIST_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee agent list [--include-ended] [--limit <n>] [--cursor <c>] [--client <id>] [--format text|json]",
    "List Agent sessions observed by the running local session. '--include-ended'\nadds ended sessions; '--limit' selects 1-100 rows per page (default 50);\n'--cursor' continues from a previous page's server-minted nextCursor.\n'--format json' emits the orchbee.agent.list envelope.\nWindows --client joins that exact TUI's observed status without focusing it.",
);
const HELP_AGENT_LIST_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee agent list [--include-ended] [--limit <n>] [--cursor <c>] [--client <id>] [--format text|json]",
    "列出运行中的本地会话观察到的 Agent 会话。'--include-ended' 包含已结束会话；'--limit' 每页 1-100 条（默认 50）；'--cursor' 从上一页服务端铸造的 nextCursor 继续。'--format json' 输出 orchbee.agent.list 封套。\nWindows --client 加入指定 TUI 观察到的状态，不移动焦点。",
);

const HELP_AGENT_GET_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee agent get <target> [--client <id>] [--format text|json]",
    "Show one Agent session by its opaque session identity: lifecycle, semantic\nstatus, observation freshness, location, capabilities, pending requests,\nand presentation attachments. '--format json' emits the\norchbee.agent.get envelope.\nTarget: one positional Session ID/name/Panel ID, --agent-session <id>,\n--name <alias>, or --panel <id|@panel:suffix>. Exact retained Session IDs\nand names precede Panel fallback. Panels select their primary terminal;\nmultiple live Agents are refused. A wait stays pinned to its first Session.\n\nWindows --client adds that exact Client's focused/seen and idle/done projection\nwith version 8. Canonical session status stays unchanged. A missing or lagging\nClient observation has null seen; no read acknowledges or changes focus.\nThe same five-second total includes cursor authentication and version joining.\nWindows Get without --client uses version 7. Runtime includes the primary\nTerminal process sample and promptReady submission preflight; cwd/argv remain\nnullable. Binding changes discard the sample. This is not interactive UI\nreadiness or an input lease. Process reads share the original total deadline.\nWindows terminal adds raw/stripped title, cwd/cwdSource and foregroundCwd,\nwith frame/process provenance from one read-only attachment. An unavailable\nprocess sample preserves frame metadata without claiming prompt readiness.\nAgent binding and Workbench revision changes discard sampled metadata.",
);
const HELP_AGENT_GET_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee agent get <target> [--client <id>] [--format text|json]",
    "按会话 ID 或运行时名称显示一个 Agent 会话：生命周期、语义状态、观察新鲜度、位置、能力、待处理请求与 Presentation 附着。'--format json' 输出\norchbee.agent.get 封套。目标：一个位置参数 Session ID/名称/Panel ID、--agent-session <id>、\n--name <alias> 或 --panel <id|@panel:suffix>。Session ID、名称优先于 Panel。\nPanel 选择主终端，多 Agent 匹配时拒绝；等待固定到首次解析的 Session。\n\nWindows --client 以版本 8 附加指定 Client 的聚焦、已看见及 idle/done 投影。规范 session 状态保持不变。Client 观测缺失或滞后时 seen 为空；读取不确认已看见、不改变焦点。游标认证与版本合并共用五秒总预算。\nWindows 不带 --client 的 Get 使用版本 7；runtime 包含主 Terminal 的进程样本及 promptReady 提交预检，cwd/argv 可为空。绑定变化时丢弃样本。这不表示交互 UI 就绪或输入租约；进程读取共用原始总期限。\nWindows terminal 增加原始／整理标题、cwd/cwdSource、foregroundCwd，以及同一次只读附着的帧／进程来源。进程样本不可用时仍保留帧元数据，但不宣称提示就绪；Agent 绑定或 Workbench 修订变化时丢弃样本。",
);

const HELP_AGENT_EXPLAIN_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent explain <target> [--format text|json]",
        "Explain a canonical Agent session or a Windows physical terminal.\nPhysical targets return effective state and a fresh rule evaluation: manifest\nsource/version, winner, all rule matches and bounded region evidence. Live full\nlifecycle authority explicitly skips the screen read. No input or focus change.\nCanonical sessions retain their derived view: the canonical status basis,\nfreshness meaning, capability ceiling, observation source and fidelity,\nand the navigate-only action boundary. '--format json' emits the\norchbee.agent.explain envelope: v1 canonical session, v2 physical terminal, v3 with active rules provenance.\nTarget: one positional Session ID/name/Panel ID, --agent-session <id>,\n--name <alias>, or --panel <id|@panel:suffix>. Exact retained Session IDs\nand names precede Panel fallback. Panels select their primary terminal;\nmultiple live Agents are refused. A wait stays pinned to its first Session.",
    );
const HELP_AGENT_EXPLAIN_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent explain <target> [--format text|json]",
        "解释规范 Agent 会话或 Windows 物理终端。物理目标返回当前有效状态及一次新鲜规则评估：清单来源/版本、胜出规则、每条规则的匹配与有界区域证据；生效的完整生命周期权威明确跳过屏幕读取。不发送输入或改变焦点。规范会话保留推导视图：规范状态依据、新鲜度含义、能力上限、观察来源与保真度，以及仅限导航的操作边界。'--format json' 输出\norchbee.agent.explain 封套：v1 为规范会话，v2 为物理终端，v3 增加生效规则来源。目标：一个位置参数 Session ID/名称/Panel ID、--agent-session <id>、\n--name <alias> 或 --panel <id|@panel:suffix>。Session ID、名称优先于 Panel。\nPanel 选择主终端，多 Agent 匹配时拒绝；等待固定到首次解析的 Session。",
    );

const HELP_AGENT_WAIT_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee agent wait <target> [--until <condition>]... [--after-revision <n>] [--timeout <ms> | --forever] [--format text|json]",
    "Poll one Agent session on a held Client Session until a requested\ncondition matches, the revision fence refuses, the session answer is\nterminal, or the total deadline lands. '--until' is repeatable and takes\none of: blocked, done, needs-attention, failed, succeeded, canceled, interrupted,\nworking, idle, unknown, ended, fresh, stale, disconnected (default:\nneeds-attention failed succeeded canceled interrupted idle ended).\nStatus conditions require fresh freshness; the three freshness\nconditions match the observation directly. '--after-revision' is a\nglobal-revision fence, not a turn identity; '--timeout' bounds the whole\nwait at 1-3600000 ms (default 600000). --forever removes the total\ntimeout and conflicts with --timeout; every read remains bounded.\nblocked matches fresh needs-attention; done matches fresh completed\noutcomes (succeeded/failed/canceled/interrupted), not an unseen marker.\nPolling samples current state: a\ntransition that begins and ends between two completed observations is\nmissed. '--format json' emits the orchbee.agent.wait envelope.\nTarget: one positional Session ID/name/Panel ID, --agent-session <id>,\n--name <alias>, or --panel <id|@panel:suffix>. Exact retained Session IDs\nand names precede Panel fallback. Panels select their primary terminal;\nmultiple live Agents are refused. A wait stays pinned to its first Session.",
);
const HELP_AGENT_WAIT_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee agent wait <target> [--until <条件>]... [--after-revision <n>] [--timeout <毫秒> | --forever] [--format text|json]",
    "在持有的 Client Session 上轮询一个 Agent 会话，直到请求的条件命中、修订栅栏拒绝、会话应答终止，或总截止到达。'--until' 可重复，取值为：\nblocked、done、needs-attention、failed、succeeded、canceled、interrupted、working、\nidle、unknown、ended、fresh、stale、disconnected（默认：\nneeds-attention failed succeeded canceled interrupted idle ended）。状态条件要求 fresh 新鲜度；三个新鲜度条件直接匹配观察。\n'--after-revision' 是全局修订栅栏而非回合身份；'--timeout' 将整个等待界于 1-3600000 毫秒（默认 600000）。--forever 取消总期限，与 --timeout 互斥，单次读取仍有界。blocked 匹配 fresh needs-attention；\ndone 匹配 fresh 的 succeeded/failed/canceled/interrupted，不代表未读。轮询采样当前状态：在两次已完成观察之间开始又结束的跃迁会被错过。'--format json' 输出\norchbee.agent.wait 封套。目标：一个位置参数 Session ID/名称/Panel ID、--agent-session <id>、\n--name <alias> 或 --panel <id|@panel:suffix>。Session ID、名称优先于 Panel。\nPanel 选择主终端，多 Agent 匹配时拒绝；等待固定到首次解析的 Session。",
);
const HELP_SITE_DIAGNOSE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server diagnose [--format text|json]",
        "Report bounded facts about the implicit per-user local session runtime —\nits base and scope directory, the activation marker and locks, and the\nfinite client/worker/pending/observation/host endpoint paths. Every row\nis a pathname or descriptor fact at its own observation instant: 'held'\nmeans some owner held the lifetime lock then — never that a session or any\nprocess is alive — and 'absent' is a pathname fact, never proof that an\nexecution ended. A 'known' marker or a valid client/worker socket leaf\nadds a conditional advisory: if no existing session can be joined, fresh\nactivation is blocked by current recovery policy.\n\nThe command never connects to an endpoint, never creates or deletes a\npath, never acquires the activation lock, and never repairs, stops, or\npredicts a later start. A cooperative five-second budget is checked\nbetween bounded filesystem operations; rows it cannot reach close as\n'uninspectable'. '--format json' emits the orchbee.site.diagnose\nenvelope on Linux; Windows emits orchbee.site.diagnose.windows with\nprivate-DACL and bounded pipe-name observations instead of POSIX facts.",
    );
const HELP_SITE_DIAGNOSE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server diagnose [--format text|json]",
        "报告隐式每用户本地会话运行时的有界事实——基目录与 scope 目录、激活标记与锁，以及有限的 client/worker/pending/observation/host 端点路径。每一行都是其各自观察瞬间的路径名或描述符事实：'held' 表示某个属主在那一刻持有生命周期锁——绝不表示某个会话或进程存活；\n'absent' 是路径名事实，绝不证明某个执行已结束。观察到 'known' 标记或合法的 client/worker 套接字叶时附加一条条件通告：若无可加入的现有会话，按当前恢复策略新的激活将被阻止。\n\n本命令绝不连接任何端点，绝不创建或删除路径，绝不获取激活锁，也绝不修复、停止或预测后续启动。协作式五秒预算在有界的文件系统操作之间检查；无法到达的行以 'uninspectable' 闭合。'--format json'\n在 Linux 输出 orchbee.site.diagnose 封套；Windows 输出\norchbee.site.diagnose.windows，使用私有 DACL 和有界管道名称观察，不虚构 POSIX 事实。",
    );
const HELP_AGENT_DOCTOR_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent doctor [--provider <id>]... [--asset-root <path>]... [--format text|json]",
        "Explain, from bounded filesystem facts alone, why Agent observations may\nnot arrive: each provider's executable and managed-hook state, declared\nasset roots, the observation endpoint, and the environment override.\n'--provider' selects canonical provider ids (repeatable); '--asset-root'\ndeclares absolute capsule roots to inspect (repeatable, at most 4).\nNothing is executed, connected, written, or repaired. '--format json'\nemits the orchbee.agent.doctor envelope (Windows v4, Linux v1). Windows\nreports endpoint delivery as uninspected, separately from installation.",
    );
const HELP_AGENT_DOCTOR_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent doctor [--provider <id>]... [--asset-root <path>]... [--format text|json]",
        "仅凭有界文件系统事实解释 Agent 观察为何可能未到达：各 provider 的可执行文件与托管 hook 状态、声明的 asset root、观察端点以及环境覆盖。\n'--provider' 选择规范 provider ID（可重复）；'--asset-root' 声明要检查的绝对 capsule root（可重复，至多 4 个）。不执行、不连接、不写入、不修复任何内容。'--format json' 输出 orchbee.agent.doctor 封套（Windows v4、Linux v1）。Windows 单独报告尚未检查的端点投递，不会把安装状态当成链路可用。",
    );

const HELP_AGENT_INTEGRATION_LIST_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent integration list [--provider <id>] [--outdated-only] [--adapter-source <abs>] [--format text|json]",
        "Inspect every supported provider's local integration state — or only the\none selected by '--provider'. Reads run the providers' own bounded\ncommands from the ambient PATH where required; nothing is written.\n'--adapter-source' overrides the cohort adapter payload for development\nand tests. Windows --outdated-only shows owned updates/repairs, preserving\nunknown and blocked provider diagnostics. Use enable to apply a guarded repair.\n'--format json' emits the orchbee.agent.integrations envelope.",
    );
const HELP_AGENT_INTEGRATION_LIST_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent integration list [--provider <id>] [--outdated-only] [--adapter-source <abs>] [--format text|json]",
        "检查每个受支持 provider 的本地集成状态，或仅检查 '--provider' 所选的一个。需要时会执行 provider 自身来自环境 PATH 的有界读命令；不写入任何内容。'--adapter-source' 为开发与测试覆盖 cohort adapter 载荷。\nWindows --outdated-only 只显示需更新／修复的自有条目，保留未知和冲突诊断。使用 enable 显式执行受保护的修复。'--format json' 输出 orchbee.agent.integrations 封套。",
    );

const HELP_AGENT_INTEGRATION_EXPLAIN_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent integration explain --provider <id> [--adapter-source <abs>] [--format text|json]",
        "Explain one provider's local integration state and the guard-evaluated\nplan 'enable' would run for the observed dimensions. Reads execute the\nprovider's own bounded commands from the ambient PATH where required;\nnothing is written. '--format json' emits the\norchbee.agent.integration.plan envelope.",
    );
const HELP_AGENT_INTEGRATION_EXPLAIN_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent integration explain --provider <id> [--adapter-source <abs>] [--format text|json]",
        "解释一个 provider 的本地集成状态，以及针对观测到的维度 'enable' 将执行的、经守卫求值的计划。需要时会执行 provider 自身来自环境 PATH 的有界读命令；不写入任何内容。'--format json' 输出\norchbee.agent.integration.plan 封套。",
    );

const HELP_AGENT_INTEGRATION_ENABLE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent integration enable --provider <id> [--adapter-source <abs>] [--format text|json]",
        "Enable one provider's managed observation integration locally: publish\nthe adapter capsule when needed, then install its guarded module or run\nbounded non-interactive provider setup commands. Manual-only entries and\nunavailable providers refuse before effects. Foreign or newer provider\nentries are never replaced; the receipt retains attempted effects and\nthe state observed by re-inspection. '--format json' emits the\norchbee.agent.integration.mutation envelope.",
    );
const HELP_AGENT_INTEGRATION_ENABLE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent integration enable --provider <id> [--adapter-source <abs>] [--format text|json]",
        "在本地启用一个 provider 的受管观测集成：必要时先发布 adapter capsule，然后安装受保护的模块，或执行有界、非交互的 provider 设置命令。仅手动管理或不可用的 provider 在产生效应前拒绝。不会替换外来或更新版本的 provider 条目；回执保留已尝试的效应和复检观察到的状态。\n'--format json' 输出 orchbee.agent.integration.mutation 封套。",
    );

const HELP_AGENT_INTEGRATION_DISABLE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent integration disable --provider <id> [--adapter-source <abs>] [--format text|json]",
        "Disable one provider's managed observation integration locally: run the\nprovider's own bounded removal commands or guarded removal of the owned\nentry. Only entries verifiably owned by Orchbee are ever removed; foreign\nor newer-versioned entries are refused before any effect. '--format json'\nemits the orchbee.agent.integration.mutation envelope.",
    );
const HELP_AGENT_INTEGRATION_DISABLE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent integration disable --provider <id> [--adapter-source <abs>] [--format text|json]",
        "在本地禁用一个 provider 的受管观测集成：执行 provider 自身的有界移除命令，或对所有物执行受保护的移除。只有经核实归 Orchbee 所有的条目才会被移除；外来或更新版本的条目在任何效应前被拒绝。'--format json'\n输出 orchbee.agent.integration.mutation 封套。",
    );

const HELP_PROJECT_LIST_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project list [--format text|json]",
        "List every Project in the running local session: id, label, directory and\nworkspace counts. An empty session prints just the table header.",
    );
const HELP_PROJECT_LIST_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project list [--format text|json]",
        "列出本地会话中所有 Project：ID、label、目录与工作区计数。空会话仅输出表头。",
    );

const HELP_PROJECT_GET_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee project get <target> [--format text|json]",
    "Show one Project: its directories (with worker ids and paths) and its\nworkspaces. The target is an exact 32-hex ID, an @project:<suffix>\nselector, or an exact case-sensitive label; ambiguous labels or suffixes\nreport candidates instead of guessing.",
);
const HELP_PROJECT_GET_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee project get <target> [--format text|json]",
    "显示一个 Project：其目录（含 worker id 与路径）与工作区。target 为精确\n32 位十六进制 ID、@project:<suffix> 选择器或精确区分大小写的 label；歧义 label 或后缀报告候选而非猜测。",
);

const HELP_PROJECT_CREATE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project create --label <label> [--format text|json]",
        "Create one Project with the given label. The command reads one snapshot,\nthen submits exactly one command under a fresh request id. The result\ncarries the committed revision and the Server-allocated project id.\nA new Project has no directory or Workspace, and creating one touches no\nfilesystem path. Labels may repeat across Projects; address the new\nProject by its returned id.",
    );
const HELP_PROJECT_CREATE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project create --label <label> [--format text|json]",
        "以给定 label 创建一个 Project。命令先读取一次快照，然后以全新请求 ID\n只提交一个命令。结果携带已提交修订与 Server 分配的 project id。新 Project 无目录、无 Workspace，创建不触碰任何文件系统路径。\nlabel 允许跨 Project 重复；请按返回的 ID 寻址新 Project。",
    );

const HELP_PROJECT_RENAME_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project rename <target> --label <label> [--format text|json]",
        "Rename one Project selected by exact ID, @project:<suffix>, or exact\nlabel. The command\nreads one snapshot, then submits exactly one command. A committed result\ncarries the new revision; an unchanged one means the label already\nmatched.",
    );
const HELP_PROJECT_RENAME_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee project rename <target> --label <label> [--format text|json]",
        "重命名一个由精确 ID、@project:<suffix> 或精确 label 选择的 Project。命令先读取一次快照，然后只提交一个命令。committed 结果携带新修订；unchanged 表示\nlabel 本已相同。",
    );

const HELP_WORKSPACE_LIST_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace list [--project <t>] [--client <id>] [--format text|json]",
        "List all Workspaces, including ungrouped ones, or filter by the Project selected by\n--project (exact ID, @project:<suffix>, or exact label). Fields: ids,\nlabel, primary directory\n(null means none), and terminal context.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.",
    );
const HELP_WORKSPACE_LIST_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace list [--project <t>] [--client <id>] [--format text|json]",
        "列出所有 Workspace（包括未分组），或仅列 --project（精确 ID、@project:<suffix> 或精确 label）所选\nProject 内的。字段：各 ID、label、主目录（null 表示无）与终端上下文。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。",
    );

const HELP_WORKSPACE_GET_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace get <target> [--client <id>] [--format text|json]",
        "Show one Workspace plus the Tabs that live in its terminal context.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.",
    );
const HELP_WORKSPACE_GET_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace get <target> [--client <id>] [--format text|json]",
        "显示一个 Workspace 及其终端上下文中存在的 Tab。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。",
    );

const HELP_WORKSPACE_CREATE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace create [--project <target>] [--label <label>] [--workspace <source>] [--cwd <path|follow|home|current|workspace>] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--env NAME=VALUE ...] [--exec <absolute-path> --arg <value> ... | --profile <selector>] [--cols <n> --rows <n> --cell-width <n> --cell-height <n>] [--format text|json]",
        "Create a Windows workspace and its first shell. Project and label are optional; no Agent starts automatically. Without --project the workspace is ungrouped. --workspace selects a directory source without inheriting its project.\n\nThe default directory follows the source terminal's verified working directory, then its workspace directory, then Windows home. An explicit path, home or current selection ignores unrelated caller identity. Relative and tilde paths are accepted literally without shell expansion.\n\nThe default label is the directory basename, or Workspace at a root. Occupied automatic labels add (2), (3), etc.; the 512-character limit includes this suffix. An explicit --label is kept unchanged.\n\nDefault: --no-focus. --focus requires an existing ready terminal client and waits for the workspace to appear; intervening navigation stops that focus. Timeout: 1..3600000 ms, default 30000.\n\n--exec/--arg and --profile cannot be combined. Custom geometry requires all four size fields. Environment and launch-profile rules still apply. Creation can succeed before a later terminal or focus failure. Terminal launch reports pending/started/failed/unavailable; use the returned workspace and request IDs to inspect the result. Never automatically retry an unknown outcome.",
    );
const HELP_WORKSPACE_CREATE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace create [--project <target>] [--label <label>] [--workspace <source>] [--cwd <path|follow|home|current|workspace>] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--env NAME=VALUE ...] [--exec <absolute-path> --arg <value> ... | --profile <selector>] [--cols <n> --rows <n> --cell-width <n> --cell-height <n>] [--format text|json]",
        "创建 Windows 工作区及其首个 Shell。项目和名称可选，不自动启动 Agent。不指定 --project 时创建未分组工作区；--workspace 只选择目录来源，不继承其项目。\n\n默认目录依次取来源终端已验证的工作目录、其工作区目录、Windows 用户主目录。显式路径、home 或 current 忽略无关调用者身份；相对路径和波浪号路径按字面接受，不做 Shell 展开。\n\n默认名称为目录末级名称，根目录时为 Workspace。自动名称重名时追加 (2)、(3) 等后缀；512 字符上限包含后缀。显式 --label 保持原样。\n\n默认 --no-focus。--focus 要求已有就绪的终端客户端，并等待工作区显示；期间的其他导航会停止此次聚焦。时限为 1..3600000 毫秒，默认 30000 毫秒。\n\n--exec/--arg 与 --profile 不能混用；自定义尺寸必须提供四个尺寸字段。环境变量和启动配置规则仍适用。后续终端或聚焦失败时，工作区可能已创建。终端启动报告 pending/started/failed/unavailable；使用返回的工作区和请求 ID 检查结果，不自动重试未知结果。",
    );
const HELP_WORKSPACE_RENAME_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace rename <target> --label <label> [--format text|json]",
        "Rename one Workspace selected by exact ID, @workspace:<suffix>, or exact\nlabel. The command\nreads one snapshot, then submits exactly one command. A committed result\ncarries the new revision; an unchanged one means the label already\nmatched.",
    );
const HELP_WORKSPACE_RENAME_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace rename <target> --label <label> [--format text|json]",
        "重命名一个由精确 ID、@workspace:<suffix> 或精确 label 选择的 Workspace。命令先读取一次快照，然后只提交一个命令。committed 结果携带新修订；unchanged 表示\nlabel 本已相同。",
    );
const HELP_WORKSPACE_ACCEPT_DIRECTORY_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace accept-directory <target> --path <path> [--format text|json]",
        "Accept a literal absolute local-platform path as the primary directory of the\nWorkspace selected by exact ID, @workspace:<suffix>, or exact label.\n'--path' is required: a\nliteral absolute path of at most 4,096 UTF-8 bytes without NUL — never\nexpanded ('~', '$'), created, or probed by the CLI. The command reads one\nsnapshot, resolves the target and its owning Project, then submits\nexactly one command under a fresh request id; the Server authoritatively\nprobes the path and rejects a Workspace that already has a directory.\nA committed result carries the new revision, the Server-allocated\ndirectory and worker ids, the resolved project and workspace ids, and the\nrequest's literal path.",
    );
const HELP_WORKSPACE_ACCEPT_DIRECTORY_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace accept-directory <target> --path <path> [--format text|json]",
        "将一个字面本机绝对路径接受为由精确 ID、@workspace:<suffix> 或精确 label 选择的\nWorkspace 的主目录。'--path' 为必填：字面绝对路径，至多 4,096 个\nUTF-8 字节且不含 NUL——CLI 不展开（'~'、'$'）、不创建、不探测。命令先读取一次快照、解析目标及其所属 Project，然后以全新请求 ID 只提交一个命令；由 Server 权威探测该路径，并拒绝已有目录的\nWorkspace。committed 结果携带新修订、Server 分配的 directory 与\nworker ID、已解析的 project 与 workspace ID，以及请求的字面路径。",
    );

const HELP_WORKSPACE_CLOSE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace close <target> --confirm [--format text|json]",
        "Close only the selected Workspace and all its terminals for every Client.\nTargets resolve globally by exact ID, @workspace:<suffix>, or exact label.\n--confirm is required. Checkout directories and branches remain; other Workspaces\nsharing its repository or Project remain open. The accepted receipt carries the\nclose request ID and stable Workspace/context IDs. Observe with workspace close-status;\nonly an explicit workspace close-resume resumes an eligible retained attempt.\nFor all members of a Project use project close-all --confirm.",
    );
const HELP_WORKSPACE_CLOSE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace close <target> --confirm [--format text|json]",
        "仅为所有客户端关闭选定 Workspace 及其所有终端。目标按精确 ID、@workspace:<suffix> 或精确名称全局解析。必须指定 --confirm。checkout 目录与分支保留；同仓库或 Project 的其他 Workspace\n继续运行。已接纳回执包含关闭请求 ID 与稳定的 Workspace/context ID。用 workspace close-status 观测；只有显式 workspace close-resume 才恢复符合条件的保留尝试。关闭 Project 全部成员使用 project close-all --confirm。",
    );

const HELP_WORKSPACE_CLOSE_STATUS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace close-status --workspace <id> [--format text|json]",
        "Read the retained Close operation by complete stable Workspace ID, including after\nthe Workspace disappears. Project membership never participates in this identity.\nThe result carries operation identity, generation, status and member lifecycle facts.\nnot-closing means no retained operation covers this Workspace. JSON: orchbee.workspace.close-status v2.",
    );
const HELP_WORKSPACE_CLOSE_STATUS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace close-status --workspace <id> [--format text|json]",
        "按完整稳定 Workspace ID 读取保留的关闭操作，Workspace 消失后仍可读取。\nProject 归属不参与操作身份。结果携带操作身份、代次、状态与成员生命周期事实。\nnot-closing 表示该 Workspace 没有保留的关闭操作。JSON：orchbee.workspace.close-status v2。",
    );

const HELP_WORKSPACE_CLOSE_RESUME_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace close-resume --workspace <id> --request <close-id> --generation <n> --confirm [--format text|json]",
        "Open one fresh attempt for an eligible retained Workspace Close. Workspace and request\nselectors are complete stable IDs. --generation is the last observed nonzero generation.\nA stale fence refuses; unknown outcomes cannot resume. --confirm is required.\nThe receipt preserves the operation identity and new generation, without Project scope.",
    );
const HELP_WORKSPACE_CLOSE_RESUME_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace close-resume --workspace <id> --request <close-id> --generation <n> --confirm [--format text|json]",
        "为符合条件的保留 Workspace 关闭操作开启一次新尝试。Workspace 与 request\n选择器为完整稳定 ID。--generation 为最近观测到的非零代次。过期栅栏会拒绝；未知结果不可恢复。必须指定 --confirm。回执保留操作身份与新代次，不使用 Project 作用域。",
    );

const HELP_TAB_LIST_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab list [--workspace <t> | --standalone] [--client <id>] [--format text|json]",
    "List Tabs. Filters select one Workspace's context (exact ID,\n@workspace:<suffix>, or label) or\nthe standalone context. Fields: ids, label, pinned flag, group, and owning\ncontext kind.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.",
);
const HELP_TAB_LIST_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab list [--workspace <t> | --standalone] [--client <id>] [--format text|json]",
    "列出 Tab。过滤参数可选择一个 Workspace 的上下文（精确 ID、@workspace:<suffix> 或 label）或独立上下文。字段：各 ID、label、置顶标记、分组与所属上下文类型。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。",
);

const HELP_TAB_GET_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab get <target> [--client <id>] [--format text|json]",
    "Show one Tab including its complete PanelNode root subtree.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.",
);
const HELP_TAB_GET_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab get <target> [--client <id>] [--format text|json]",
    "显示一个 Tab，包括其完整 PanelNode 根子树。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。",
);

const HELP_TAB_CREATE_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab create [--workspace <target> | --standalone] [--label <label>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]",
    "Create one Tab in the selected terminal context: a Workspace chosen by\nexact ID, @workspace:<suffix>, or exact label, or the standalone context.\nThe four geometry\nflags are an all-or-none group: with all four absent the command\nrequests an 80x24 logical grid with unknown cell pixels (0,0) — a\nrequest policy, not a measured display fact; supplying any of the four\nrequires all four. The result carries the committed revision, the new\ntab, panel, and presentation ids, and the terminal spawn outcome.\n'--exec' selects the launched program by absolute path and each '--arg'\nappends one verbatim argument; omitting both launches the default shell.\n'--profile' instead selects a stored Launch Profile by exact ID or\nsingly-matching name, resolved by the Server — it cannot combine with\n'--exec' or '--arg'.\nWindows --cwd defaults to follow: sample the resolved source Panel root process cwd.\nAn absolute path overrides it; home/current use the CLI home/current directory;\nworkspace keeps Server Workspace/Worker-home defaults. Known absent cwd keeps\nthose defaults; read failures stop before creation. Explicit headless Tab context\ndoes not select a Client just to find cwd. Other platforms keep absolute paths. '--env NAME=VALUE' repeats to overlay inherited variables;\nvalues are literal, including empty values. These options also work with\n'--profile' and the default shell. Names: ASCII letters/digits/underscore,\n1..128 bytes, no leading digit; case-insensitive duplicates, ORCHBEE_*,\nTERM, COLORTERM and TERM_PROGRAM are rejected. Limit: 32 entries, 1,024\nUTF-8 name/value bytes total, no NUL. A bad directory fails the launch.\n\nWindows: omitted targets use validated caller context or one current Client selection. Invalid caller metadata never falls back; --client ignores caller environment. Unnamed Tabs display their current one-based position; --label fixes the name, including numeric names. Split labels default to the preflight Panel count plus one. No focus by default; --focus waits for the original Client to focus the new Panel after creation. Explicit targets without Client effects work headlessly. Total timeout: 30000ms (1..3600000). orchbee.creation.result v3 records directory policy/source/path and preserves creation and subsequent phase facts; later failure never undoes creation. Do not blindly repeat.",
);
const HELP_TAB_CREATE_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab create [--workspace <target> | --standalone] [--label <label>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]",
    "在所选终端上下文内创建一个 Tab：由精确 ID、@workspace:<suffix> 或精确 label 选择的\nWorkspace，或独立上下文。四个几何 flag 为全有或全无组：全部省略时请求 80x24 逻辑网格且单元像素为未知 (0,0)——这是请求策略而非实测显示事实；提供其中任何一个就必须提供全部四个。结果携带已提交修订、新的 tab、panel、presentation ID 及终端拉起结果。\n'--exec' 以绝对路径选择启动程序，每个 '--arg' 逐字追加一个参数；两者均省略时启动默认 shell。\n'--profile' 改为按精确 ID 或唯一匹配的名称选择已存储的启动配置，由 Server 解析——不能与 '--exec' 或 '--arg' 组合。\nWindows --cwd 默认为 follow：采样已解析来源 Panel 的根进程目录。绝对路径直接覆盖；\nhome/current 使用 CLI 的主目录／当前目录；workspace 保留 Server Workspace／Worker 主目录默认值。明确缺失 cwd 时保留默认值；读取失败则在创建前停止。显式无界面 Tab 上下文不会为查目录选择 Client。其他平台仍只使用绝对路径。\n'--env NAME=VALUE' 可重复使用，覆盖继承的变量；值逐字保留，允许空值。这些选项也可配合 '--profile' 或默认 shell 使用。名称只允许 ASCII 字母、数字和下划线，1..128 字节且不能以数字开头；拒绝忽略大小写后重复的名称、\nORCHBEE_*、TERM、COLORTERM 和 TERM_PROGRAM。至多 32 项，名称和值合计不超过 1,024 UTF-8 字节，不能含 NUL。目录不可用时启动失败。\n\nWindows：省略目标时使用经校验的调用者上下文，或唯一 current Client 的选择。无效调用者信息不回退；显式 --client 忽略调用者环境。未命名 Tab 显示当前从一开始的位置序号；--label 固定名称，包括数字名称。分屏标签默认为创建前 Panel 数加一。默认不聚焦；--focus 在创建后等待原 Client 聚焦新 Panel。显式目标且无需 Client 操作时可无 TUI 运行。总超时默认 30000ms（1..3600000）。orchbee.creation.result v3 记录目录策略／来源／路径，并保留已创建身份及后续步骤结果；后续失败不会撤销创建，勿盲目重试。",
);

const HELP_PANEL_RENAME_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel rename <panel-id> (--label <text> | --clear) [--format text|json]",
        "Set a shared custom Panel title by exact ID or @panel:<suffix>. Empty or\nwhitespace-only text clears the title and restores its creation label. Input\nis limited to 512 Unicode characters before trimming. No focus change.\nrenamed reports a committed revision; unchanged means the override matched.",
    );
const HELP_PANEL_RENAME_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel rename <panel-id> (--label <text> | --clear) [--format text|json]",
        "用精确 ID 或 @panel:<suffix> 设置共享的窗格自定义标题。空文本或纯空白文本会清除标题，恢复创建时的标签。裁剪前最多 512 个 Unicode 字符。不会切换焦点。\nrenamed 表示已提交新修订；unchanged 表示自定义标题未变。",
    );
const HELP_TAB_RENAME_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab rename <target> --label <label> [--format text|json]",
    "Rename one Tab selected by exact ID, @tab:<suffix>, or exact label.\nA committed result\ncarries the new revision; unchanged means an already fixed name matched. Renaming an automatic Tab fixes its name even when the visible text is equal.",
);
const HELP_TAB_RENAME_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab rename <target> --label <label> [--format text|json]",
    "重命名一个由精确 ID、@tab:<suffix> 或精确 label 选择的 Tab。committed 结果携带新修订；unchanged 表示固定名称本已相同。自动命名 Tab 即使文本相同，重命名也会将其改为固定名称。",
);

const HELP_TAB_MOVE_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab move <target> (--direction <previous|next|first|last> | --insert-index <N>) [--format text|json]",
    "Move one Tab within its strip. A committed result carries the new\nrevision; unchanged means the tab was already in position.\nWindows --insert-index uses a boundary in the original order: 0 is first,\nTab count is last; source/source+1 are unchanged. Out of range is refused.",
);
const HELP_TAB_MOVE_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab move <target> (--direction <previous|next|first|last> | --insert-index <N>) [--format text|json]",
    "在标签条内移动一个 Tab。committed 结果携带新修订；unchanged 表示 Tab\n本已在目标位置。\nWindows --insert-index 使用原顺序的边界：0 为首位，Tab 总数为末尾；源位置及其后一边界无变化。超出当前数量会拒绝。",
);

const HELP_TAB_CLOSE_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab close <target> --confirm [--format text|json]",
    "Close one Tab selected by exact ID, @tab:<suffix>, or exact label.\n'--confirm' is\nrequired. An accepted result carries the accepted revision and the close\nrequest id; the session finishes the close asynchronously.",
);
const HELP_TAB_CLOSE_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee tab close <target> --confirm [--format text|json]",
    "关闭一个由精确 ID、@tab:<suffix> 或精确 label 选择的 Tab。'--confirm' 为必填。\naccepted 结果携带已接受修订与关闭请求 ID；会话异步完成关闭。",
);

const HELP_PANEL_LIST_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel list [--tab <t>] [--client <id>] [--format text|json]",
    "List Panels, optionally inside the Tab selected by --tab (exact ID,\n@tab:<suffix>, or label). Panels carry no label; presentation labels are\nnot panel names.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\nWindows also observes primary terminal title, directory and process facts.\nUnavailable observations are explicit; the query never controls or resizes terminals.",
);
const HELP_PANEL_LIST_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel list [--tab <t>] [--client <id>] [--format text|json]",
    "列出 Panel，或仅列 --tab（精确 ID、@tab:<suffix> 或 label）所选 Tab 内的。Panel 无\nlabel；Presentation 的 label 不是 Panel 名。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\nWindows 还观察主终端标题、目录和进程事实。不可用观察会明确标记；查询不控制或调整终端大小。",
);

const HELP_PANEL_GET_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel get <panel-id> [--client <id>] [--format text|json]",
    "Show one Panel by complete typed ID or @panel:<suffix>: owning tab,\ncontext, slot path, and its primary and child Presentations.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\nWindows also observes primary terminal title, directory and process facts.\nUnavailable observations are explicit; the query never controls or resizes terminals.",
);
const HELP_PANEL_GET_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel get <panel-id> [--client <id>] [--format text|json]",
    "按完整类型化 ID 或 @panel:<suffix> 显示一个 Panel：所属 Tab、上下文、slot 路径及其主与子 Presentation。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\nWindows 还观察主终端标题、目录和进程事实。不可用观察会明确标记；查询不控制或调整终端大小。",
);

const HELP_PANEL_SPLIT_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel split [<panel-id> | --panel <panel-id> | --current] --direction <above|below|left|right> [--label <label>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--right-click orchbee|panel] [--ratio <number>] [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]",
    "Split one Panel, creating a sibling panel on the selected side. The\nfour geometry flags are an all-or-none group: with all four absent the\ncommand requests an 80x24 logical grid with unknown cell pixels (0,0) —\na request policy, not a measured display fact; supplying any of the\nfour requires all four. The result carries the committed revision,\nthe new panel, split, slot, and presentation ids, and the terminal spawn\noutcome. '--exec' selects the launched program by absolute path and each\n'--arg' appends one verbatim argument; omitting both launches the\ndefault shell. '--profile' instead selects a stored Launch Profile by\nexact ID or singly-matching name, resolved by the Server — it cannot\ncombine with '--exec' or '--arg'.\nWindows --cwd defaults to follow: sample the resolved source Panel root process cwd.\nAn absolute path overrides it; home/current use the CLI home/current directory;\nworkspace keeps Server Workspace/Worker-home defaults. Known absent cwd keeps\nthose defaults; read failures stop before creation. Explicit headless Tab context\ndoes not select a Client just to find cwd. Other platforms keep absolute paths. '--env NAME=VALUE' repeats to overlay inherited variables;\nvalues are literal, including empty values. These options also work with\n'--profile' and the default shell. Names: ASCII letters/digits/underscore,\n1..128 bytes, no leading digit; case-insensitive duplicates, ORCHBEE_*,\nTERM, COLORTERM and TERM_PROGRAM are rejected. Limit: 32 entries, 1,024\nUTF-8 name/value bytes total, no NUL. A bad directory fails the launch.\n\nWindows: omitted targets use validated caller context or one current Client selection. Invalid caller metadata never falls back; --client ignores caller environment. Unnamed Tabs display their current one-based position; --label fixes the name, including numeric names. Split labels default to the preflight Panel count plus one. No focus by default; --focus waits for the original Client to focus the new Panel after creation. Explicit targets without Client effects work headlessly. Total timeout: 30000ms (1..3600000). orchbee.creation.result v3 records directory policy/source/path and preserves creation and subsequent phase facts; later failure never undoes creation. Do not blindly repeat. --ratio is the first child share, clamped from a finite number to 0.1..0.9; it is shared across windows, saved for restart and does not imply focus. --right-click sets only the new Panel policy atomically with creation (default orchbee); panel permits unmodified right clicks inside a mouse-reporting terminal.",
);
const HELP_PANEL_SPLIT_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel split [<panel-id> | --panel <panel-id> | --current] --direction <above|below|left|right> [--label <label>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--right-click orchbee|panel] [--ratio <number>] [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]",
    "分裂一个 Panel，在所选侧创建相邻 Panel。四个几何 flag 为全有或全无组：全部省略时请求 80x24 逻辑网格且单元像素为未知 (0,0)——这是请求策略而非实测显示事实；提供其中任何一个就必须提供全部四个。结果携带已提交修订、新的 panel、split、slot、presentation ID 及终端拉起结果。'--exec' 以绝对路径选择启动程序，每个 '--arg' 逐字追加一个参数；两者均省略时启动默认 shell。'--profile' 改为按精确 ID 或唯一匹配的名称选择已存储的启动配置，由 Server 解析——不能与 '--exec'\n或 '--arg' 组合。\nWindows --cwd 默认为 follow：采样已解析来源 Panel 的根进程目录。绝对路径直接覆盖；\nhome/current 使用 CLI 的主目录／当前目录；workspace 保留 Server Workspace／Worker 主目录默认值。明确缺失 cwd 时保留默认值；读取失败则在创建前停止。显式无界面 Tab 上下文不会为查目录选择 Client。其他平台仍只使用绝对路径。\n'--env NAME=VALUE' 可重复使用，覆盖继承的变量；值逐字保留，允许空值。这些选项也可配合 '--profile' 或默认 shell 使用。名称只允许 ASCII 字母、数字和下划线，1..128 字节且不能以数字开头；拒绝忽略大小写后重复的名称、\nORCHBEE_*、TERM、COLORTERM 和 TERM_PROGRAM。至多 32 项，名称和值合计不超过 1,024 UTF-8 字节，不能含 NUL。目录不可用时启动失败。\n\nWindows：省略目标时使用经校验的调用者上下文，或唯一 current Client 的选择。无效调用者信息不回退；显式 --client 忽略调用者环境。未命名 Tab 显示当前从一开始的位置序号；--label 固定名称，包括数字名称。分屏标签默认为创建前 Panel 数加一。默认不聚焦；--focus 在创建后等待原 Client 聚焦新 Panel。显式目标且无需 Client 操作时可无 TUI 运行。总超时默认 30000ms（1..3600000）。orchbee.creation.result v3 记录目录策略／来源／路径，并保留已创建身份及后续步骤结果；后续失败不会撤销创建，勿盲目重试。 --ratio 是第一子节点的份额，有限数值限制到 0.1..0.9；跨窗口共享并保存在重启存档中，不隐含聚焦。 --right-click 随创建原子设置新 Panel 的策略（默认 orchbee）；panel 允许无修饰键的右键在启用鼠标上报的终端内容内透传。",
);

const HELP_PANEL_CLOSE_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel close <panel-id> --confirm [--format text|json]",
    "Close one Panel selected by complete typed ID or @panel:<suffix>.\n'--confirm' is required.\nAn accepted result carries the accepted revision and the close request\nid; the session finishes the close asynchronously.",
);
const HELP_PANEL_CLOSE_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel close <panel-id> --confirm [--format text|json]",
    "关闭一个由完整类型化 ID 或 @panel:<suffix> 选择的 Panel。'--confirm' 为必填。accepted\n结果携带已接受修订与关闭请求 ID；会话异步完成关闭。",
);

#[cfg(not(windows))]
const HELP_PANEL_MOVE_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel move <panel> (--target-panel <panel> | --into-tab <tab> | --new-tab) [--direction above|below|left|right] [--label <text>] [--format text|json]",
    "Move a leaf panel within its current workspace or standalone group. --target-panel places it beside a sibling in the same tab and requires --direction. --into-tab selects a sibling tab and also requires --direction. --new-tab appends a new tab and requires --label.\n\nStacked panels and the last panel in a source tab are refused; empty tabs are not implicitly removed. Terminal identity, process and working directory remain unchanged. A committed result returns the new revision; unchanged means the layout already matched.",
);
#[cfg(not(windows))]
const HELP_PANEL_MOVE_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel move <panel> (--target-panel <panel> | --into-tab <tab> | --new-tab) [--direction above|below|left|right] [--label <text>] [--format text|json]",
    "在当前工作区或独立标签组中移动叶子面板。--target-panel 将其放到同标签页的相邻面板旁，并要求 --direction。--into-tab 选择同组标签页，也要求 --direction。--new-tab 追加新标签页，并要求 --label。\n\n拒绝堆叠面板以及来源标签页的最后一个面板，不隐式移除空标签页。终端标识、进程和工作目录保持不变。committed 返回新修订；unchanged 表示布局已符合要求。",
);

const HELP_TERMINAL_LIST_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal list [--format text|json]",
        "List every terminal bound by a Presentation. Unbound terminals are not\nvisible; a binding is structural evidence, not proof a process is alive.",
    );
const HELP_TERMINAL_LIST_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal list [--format text|json]",
        "列出所有被 Presentation 绑定的终端。未绑定终端不可见；绑定是结构证据，不是进程存活的证明。",
    );

const HELP_TERMINAL_GET_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal get <terminal-id> [--format text|json]",
        "Show one bound terminal by complete typed ID or @terminal:<suffix>:\nworker id and its single boundTo location (presentation, panel, tab,\ncontext).",
    );
const HELP_TERMINAL_GET_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal get <terminal-id> [--format text|json]",
        "按完整类型化 ID 或 @terminal:<suffix> 显示一个已绑定终端：worker id 及其唯一 boundTo 位置（presentation、panel、tab、context）。",
    );

#[cfg(not(windows))]
const HELP_TERMINAL_READ_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal read <terminal-id> [--history --rows <n>] [--unwrap] [--lines <n>] [--format text|json]",
        "Read the current visible content of one bound terminal. The terminal is\nresolved from the Workbench snapshot, then observed once through a short-\nlived read-only viewer on the same session; nothing is created, activated,\nor mutated. Text output prints the grid rows; '--format json' emits the\norchbee.terminal.read envelope with frame metadata and row content.\n\nWith '--history --rows <n>' (both flags required together, 1..=1000), read\nup to n retained rows that have scrolled off the active screen instead of\nthe visible frame; '--format json' emits the orchbee.terminal.history\nenvelope with scrollback metadata and row content.\n\n'--unwrap' is a read-only presentation transform: verified soft-wrapped\nphysical rows join into logical lines (hard breaks and empty lines are\npreserved; a window edge inside a wrapped line reports startsMidLine/\nendsMidLine instead of claiming a complete line). '--lines <n>' (1..=65535)\nshows only the last n emitted entries — logical lines under '--unwrap',\nphysical rows otherwise — and never widens the fetched window.\n\nExplicit sources: --source visible|recent|recent-unwrapped|detection.\nRecent includes history and live screen (default 80 physical rows); detection\nuses one screen height near the cursor, not Agent state. Source-mode --lines\naccepts 0..4294967295 and clamps to 1000. Visible is this viewer's live screen.\n--plain prints UTF-8 text directly and defaults to recent; it conflicts with\n--format json. Source/plain cannot mix with --history, --rows or --unwrap.\nJSON uses orchbee.terminal.output; a stale sample fails without a hidden retry.\n\n--ansi exports styled text directly (default recent); --format json instead\nuses orchbee.terminal.output v2 with encoding: ansi. It conflicts with\n--plain, --history, --rows and --unwrap. Styled blanks remain content;\nrecent-unwrapped joins soft wraps. Detection stays plain text. This is a\nstyled snapshot with SGR, not original PTY bytes or graphics replay.",
    );
#[cfg(not(windows))]
const HELP_TERMINAL_READ_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal read <terminal-id> [--history --rows <n>] [--unwrap] [--lines <n>] [--format text|json]",
        "读取一个已绑定终端的当前可见内容。终端经 Workbench 快照解析后，通过同一会话上短暂存活的只读 viewer 观察一次；不创建、不激活、不修改任何状态。文本输出逐行打印网格内容；'--format json' 输出带帧元数据与行内容的 orchbee.terminal.read 封套。\n\n加 '--history --rows <n>'（两个 flag 必须成对，1..=1000）时，读取已滚出当前活动屏幕的至多 n 行保留历史，而非可见帧；'--format json' 输出带历史元数据与行内容的 orchbee.terminal.history 封套。\n\n'--unwrap' 是只读的呈现变换：经验证的软换行物理行拼接为逻辑行（保留硬换行与空行；窗口边缘落在换行行内部时以 startsMidLine/endsMidLine\n如实标记，而非声称完整行）。'--lines <n>'（1..=65535）只显示最后 n\n条已发出的条目——'--unwrap' 下为逻辑行，否则为物理行——且不会扩大抓取窗口。\n\n显式来源：--source visible|recent|recent-unwrapped|detection。\nrecent 合并历史与实时屏幕，默认 80 个物理行；detection 读取光标附近一个屏幕高度，不是 Agent 状态。来源模式 --lines 接受 0..4294967295，钳制到 1000。visible 是当前查看器的实时屏幕。\n--plain 直接输出 UTF-8 文本，默认 recent；与 --format json 冲突。来源/plain 不能与 --history、--rows 或 --unwrap 混用。\nJSON 使用 orchbee.terminal.output；样本过期时失败，不隐藏重试。\n\n--ansi 直接导出带样式文本（默认 recent）；配合 --format json 时改用\norchbee.terminal.output v2，encoding 为 ansi。它与 --plain、--history、\n--rows 和 --unwrap 冲突。保留带样式空格；recent-unwrapped 拼接软换行。\ndetection 仍输出纯文本。这是包含 SGR 的样式快照，不是原始 PTY 字节或图形重放。",
    );

#[cfg(not(windows))]
const HELP_TERMINAL_WAIT_OUTPUT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal wait-output <terminal-id> (--match <text> | --regex <pattern>) [--rows <n>] [--timeout <ms>] [--format text|json]",
        "Wait until the pattern appears in one bound terminal's retained output —\nexactly one of '--match' or '--regex' is required. '--match' is a literal,\ncase-sensitive substring (1..=4096 bytes); '--regex' is a nonempty UTF-8\nregular expression of at most 1024 bytes, compiled once before any\nconnection (invalid or oversized programs are usage failures). Matching\nchecks the most recent --rows retained rows (1..=1000, default 1000)\nspanning the visible screen and scrollback as one window, joining\nsoft-wrapped rows into logical lines first; a regex match reports its\nbyte length and fragment-edge flags, and '^'/'$' anchor at the observed\nfragment edges. '--timeout' bounds the total wait in milliseconds\n(1..=3600000, default 30000); the command exits 0 on match, 7 on timeout,\nand 130 when cancelled by HUP/INT/QUIT/TERM. Nothing is typed into the\nterminal and nothing is created or mutated. Text output prints the\nmatching line; '--format json' emits the orchbee.terminal.wait-output\nenvelope with match and observation metadata.\n\nSource mode: --source visible|recent|recent-unwrapped|detection, --lines N,\nor --forever. It defaults to recent (80 physical rows), excludes --rows,\nand uses orchbee.terminal.wait-output v3. --lines allows zero and clamps to\n1000. Recent sources join soft wraps; visible/detection match physical lines.\n--forever removes the total deadline, conflicts with --timeout, and retains\nfive-second I/O limits and Ctrl+C cleanup. Zero lines never match.",
    );
#[cfg(not(windows))]
const HELP_TERMINAL_WAIT_OUTPUT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal wait-output <terminal-id> (--match <text> | --regex <pattern>) [--rows <n>] [--timeout <ms>] [--format text|json]",
        "等待一个已绑定终端的保留输出中出现指定模式——'--match' 与 '--regex'\n必须且只能提供一个。'--match' 为字面、区分大小写的子串（1..=4096 字节）；'--regex' 为至多 1024 字节的非空 UTF-8 正则表达式，在任何连接建立前编译一次（非法或超限程序报用法错误）。搜索范围为最近 --rows\n行保留行（1..=1000，默认 1000），可见屏与回滚历史作为一个窗口，并先把软换行行拼接为逻辑行；正则命中会报告字节长度与片段边缘标记，\n'^'/'$' 仅锚定观测到的片段边界。'--timeout' 以毫秒限定总等待预算（1..=3600000，默认 30000）；匹配成功退出 0，超时退出 7，被\nHUP/INT/QUIT/TERM 取消时退出 130。不向终端写入任何输入，不创建、不修改任何状态。文本输出打印匹配行；'--format json' 输出带匹配与观测元数据的 orchbee.terminal.wait-output 封套。\n\n来源模式：--source visible|recent|recent-unwrapped|detection、--lines N\n或 --forever。默认 recent（80 个物理行），与 --rows 互斥，使用\norchbee.terminal.wait-output v3。--lines 允许零，钳制到 1000。\nrecent 来源拼接软换行；visible/detection 匹配物理行。\n--forever 移除总期限，与 --timeout 互斥，仍保留五秒 I/O 上限和\nCtrl+C 清理。零行从不匹配。",
    );
const HELP_TERMINAL_RUN_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal run <target> <command> [--format text|json]",
        "Submit one quoted command string to an existing Terminal as one paste record followed\nby Enter in one request. Target: exact ID or @terminal:<suffix>.\nThe current program interprets the content; there is no Shell-readiness check, process\nlaunch, or command completion/exit-status promise. Whitespace and newlines are preserved.\nUse -- before operands resembling CLI options; put --format before that separator.\nEmpty commands and extra command operands are rejected. Total argv is at most 64 KiB;\nthe paste limit is 256 KiB. The existing five-second input deadline applies.\nThe orchbee.terminal.send receipt reports input and cleanup independently. Partial or\nunknown delivery must not be replayed. No controller, resize or focus is acquired.",
    );
const HELP_TERMINAL_RUN_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal run <target> <command> [--format text|json]",
        "把一个引号内命令字符串以 paste 后接 Enter 的形式，在一个请求中提交到既有 Terminal。目标支持精确 ID 或 @terminal:<suffix>。当前程序解释内容；不检查 Shell 就绪、不启动进程，也不承诺命令完成或退出状态。空白和换行原样保留。操作数像 CLI 选项时，先写 --；--format 放在分隔符之前。拒绝空命令及多余命令操作数。完整 argv 上限 64 KiB，paste 上限 256 KiB，遵循既有五秒输入期限。\norchbee.terminal.send 回执分别报告输入与清理结果。部分或未知提交不得重放。不获取 controller、resize 或焦点权限。",
    );

const HELP_TERMINAL_SEND_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal send <terminal-id> (--text <utf8> [--enter] | --key <name>... | --file <path> [--paste] [--enter] | --stdin [--paste] [--enter]) [--format text|json]",
        "Submit one bounded input sequence to one bound terminal. Choose exactly one\nsource family. --text accepts up to 4096 UTF-8 bytes including an optional\ncarriage return. Repeat --key for up to 64 semantic keys; named keys include\nenter, escape, tab, backspace, delete, up, down, left, right, home, end,\npage-up, page-down, ctrl-c, ctrl-d, insert, space and f1..f25. A single\nprintable Unicode character or punctuation name (plus, semicolon, etc.)\nis also accepted. Combine ctrl/control, alt/option, shift and super/cmd/command\nwith +, for example ctrl+shift+a or super+c. Quote shell punctuation.\nThe Host encodes keys for the terminal's current keyboard mode.\n\n--file reads a regular file. --stdin accepts a redirected regular file or\npipe, including Windows pipes, and refuses interactive terminals/devices.\nFile/stdin preserves raw bytes, including non-UTF-8 data, up to 1 MiB\nincluding an optional --enter carriage return. Empty input is refused.\n--paste is valid only with --file or --stdin, limits content to 256 KiB,\nand sends bracketed paste under the terminal's mode. With --paste, --enter\nadds a separate trailing Enter record. Nothing is sent until the complete\nsource is acquired and validated under the shared five-second deadline.\nWindows pipe cancellation terminates and reaps its owned reader process.\n\nThe temporary input-only attachment never takes the TUI's controller,\nviewer or resize ownership. Inspect inputStatus and cleanupStatus in the\norchbee.terminal.send JSON result independently. Partial acceptance is not\ncomplete success. When the outcome is unknown (exit 8), inspect the terminal\nbefore another action: blindly resending may type twice. No request is\nautomatically replayed.",
    );
const HELP_TERMINAL_SEND_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal send <terminal-id> (--text <utf8> [--enter] | --key <name>... | --file <path> [--paste] [--enter] | --stdin [--paste] [--enter]) [--format text|json]",
        "向一个已绑定终端提交一组有界输入。必须恰好选择一种来源。\n--text 接受不超过 4096 字节的 UTF-8 文字，含可选附加回车。重复 --key 可发送最多 64 个语义按键；具名按键包括 enter、escape、tab、\nbackspace、delete、up、down、left、right、home、end、page-up、\npage-down、ctrl-c、ctrl-d、insert、space 和 f1..f25。也接受单个可打印 Unicode 字符或 plus、semicolon 等标点名称。用 + 组合\nctrl/control、alt/option、shift、super/cmd/command，例如 ctrl+shift+a\n或 super+c；Shell 标点请加引号。由 Host 按终端当前键盘模式编码。\n\n--file 读取普通文件。--stdin 接受重定向普通文件或管道，包括 Windows\n管道，拒绝交互终端和设备。文件/stdin 原样保留字节，含非 UTF-8 数据，连同 --enter 的可选回车上限为 1 MiB。空输入被拒绝。\n--paste 仅用于 --file 或 --stdin，内容上限 256 KiB，按终端模式发送\nbracketed paste。此时 --enter 附加独立的尾随 Enter 记录。来源在共用五秒期限内完整读取并校验后才发送任何内容。\nWindows 管道取消时会终止并回收所属读取进程。\n\n临时仅输入附属通道不获取 TUI 的 controller、viewer 或 resize 所有权。分别检查 orchbee.terminal.send JSON 结果的 inputStatus 与 cleanupStatus。部分接受不等于全部成功。结果未知（退出码 8）时，采取下一步操作前先检查终端：盲目重发可能输入两次。请求不会自动重放。",
    );

const HELP_LAUNCH_PROFILE_LIST_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee launch-profile list [--format text|json]",
        "List every Launch Profile in the running local session's catalog: profile\nID, name, and the stored exec/args launch. '--format json' emits the\norchbee.launch-profile.list envelope with the catalog revision.",
    );
const HELP_LAUNCH_PROFILE_LIST_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee launch-profile list [--format text|json]",
        "列出运行中本地会话目录里的所有启动配置：profile ID、名称及存储的\nexec/args 启动。'--format json' 输出携带目录修订的\norchbee.launch-profile.list 封套。",
    );

const HELP_LAUNCH_PROFILE_GET_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee launch-profile get <selector> [--format text|json]",
        "Show one Launch Profile resolved by the Server: an exact profile ID\nfirst, then a name whose match cardinality must be exactly one — an\nunmatched or ambiguous selector fails, never guesses. '--format json'\nemits the orchbee.launch-profile.get envelope.",
    );
const HELP_LAUNCH_PROFILE_GET_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee launch-profile get <selector> [--format text|json]",
        "显示一个由 Server 解析的启动配置：先按精确 profile ID，再按匹配数必须恰好为一的名称——未命中或歧义的 selector 会失败而非猜测。\n'--format json' 输出 orchbee.launch-profile.get 封套。",
    );

const HELP_LAUNCH_PROFILE_CREATE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee launch-profile create --name <name> --exec <abs> [--arg <v>]... [--format text|json]",
        "Store one named Launch Profile in the running session's catalog. '--name'\nis required (1–512 UTF-8 bytes; uniqueness is not required). '--exec'\nselects the launched program by absolute path and each '--arg' appends\none verbatim argument. The result carries the committed revision, the\ncatalog revision, the minted profile ID, and the stored name.",
    );
const HELP_LAUNCH_PROFILE_CREATE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee launch-profile create --name <name> --exec <abs> [--arg <v>]... [--format text|json]",
        "在运行中会话的目录里存储一个命名启动配置。'--name' 必填（1–512 个\nUTF-8 字节；不要求唯一）。'--exec' 以绝对路径选择启动程序，每个\n'--arg' 逐字追加一个参数。结果携带已提交修订、目录修订、新铸造的\nprofile ID 及存储的名称。",
    );

const HELP_LAUNCH_PROFILE_DELETE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee launch-profile delete <selector> [--format text|json]",
        "Delete one Launch Profile resolved by the Server under the same selector\nrule as 'launch-profile get': exact profile ID first, then a name\nmatching exactly one entry. The result carries the committed revision,\nthe catalog revision, and the deleted profile ID.",
    );
const HELP_LAUNCH_PROFILE_DELETE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee launch-profile delete <selector> [--format text|json]",
        "删除一个由 Server 按与 'launch-profile get' 相同 selector 规则解析的启动配置：先精确 profile ID，再按恰好匹配一个条目的名称。结果携带已提交修订、目录修订与被删除的 profile ID。",
    );

const HELP_WORKTREE_LIST_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee worktree list [--workspace <target>|--cwd <path>] [--trust-repository] [--client <id>] [--format text|json]\norchbee worktree list --directory <directory-id> [--format text|json]",
        "Windows: list Git checkouts and associated Workspaces. With no source selector,\n--client selects an exact ready TUI; otherwise use the validated caller, then a unique\ncurrent ready TUI, then CLI cwd only when neither exists. A Workspace source keeps its exact identity and must have a primary directory. No Project is required for a directory query. Invalid caller identity\nnever falls back. --trust-repository trusts this repository for this query only.\nThe query is read-only, does not activate a session, and has a 5-second total budget.\nJSON source queries emit orchbee.worktree.query; failed Server results keep typed reasons.\nThe globally stable --directory selector keeps orchbee.worktree.list and its failure schema;\nthese selectors cannot be combined with --workspace, --cwd, --client or --trust-repository.\nWorkspace targets accept exact IDs, @workspace:<suffix> or exact labels; ambiguity is rejected.\nBranches keep refs/heads/ prefixes; all-zero HEAD marks an unborn repository.\nWindows --cwd accepts absolute, relative and ~ paths; the final executing CLI resolves its own cwd/home after routing. Home precedence: USERPROFILE, HOMEDRIVE+HOMEPATH, HOME.",
    );
const HELP_WORKTREE_LIST_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee worktree list [--workspace <target>|--cwd <路径>] [--trust-repository] [--client <id>] [--format text|json]\norchbee worktree list --directory <directory-id> [--format text|json]",
        "Windows：列出 Git 检出目录及关联 Workspace。省略来源选择器时，已验证的调用者终端\n优先，其次为唯一当前就绪 TUI，均无时才使用 CLI cwd。--client 选择精确就绪 TUI，\n覆盖隐式调用者。Workspace 来源保留其精确标识且必须有主目录。目录查询无需 Project。无效调用者身份\n绝不回退。--trust-repository 仅在本次查询中信任该仓库。\n查询只读、不激活会话，总预算 5 秒。来源查询 JSON 使用 orchbee.worktree.query，\nServer 失败结果保留类型化原因。全局稳定 --directory 选择器保留 orchbee.worktree.list\n及其失败 Schema；不可混用 --workspace、--cwd、--client 或 --trust-repository。\nWorkspace 目标支持精确 ID、@workspace:<suffix> 或精确标签；拒绝歧义。\n分支保留 refs/heads/ 前缀；全零 HEAD 表示未出生仓库。\nWindows --cwd 接受绝对、相对和 ~ 路径；路由后由最终执行的 CLI 使用自身 cwd／主目录解析。主目录优先级：USERPROFILE、HOMEDRIVE+HOMEPATH、HOME。",
    );

const HELP_API_SNAPSHOT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee api snapshot [--format text|json]",
        "Emit the complete validated workbench.snapshot document inside the\norchbee.api.snapshot envelope. Successful output is JSON even under\n'--format text'; a failure envelope still requires '--format json'.",
    );
const HELP_API_SNAPSHOT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee api snapshot [--format text|json]",
        "在 orchbee.api.snapshot 封套内输出完整已验证 workbench.snapshot 文档。成功输出即使 '--format text' 也为 JSON；失败封套仍需显式 '--format json'。",
    );

const HELP_API_SCHEMA_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee api schema [<schema-id>] [--output <path>] [--format text|json]",
    "Emit the embedded JSON Schema dictionary for every implemented command, or\nthe single schema for <schema-id>. Successful output is always JSON; unknown\nIDs are not-found, and failure envelopes require '--format json'.\nOn Windows, --output writes identical JSON to a literal file path, replacing\nan existing regular file only after complete preparation. Parent directories\nmust exist. The command stays offline and also emits the JSON on stdout.",
);
const HELP_API_SCHEMA_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee api schema [<schema-id>] [--output <path>] [--format text|json]",
    "输出所有已实现命令的内嵌 JSON Schema 字典，或 <schema-id> 的单个 schema。成功输出始终为 JSON；未知 ID 为 not-found，失败封套需显式 '--format json'。\nWindows 的 --output 将相同 JSON 写入字面文件路径，完整准备后才替换已有普通文件。父目录必须存在；全程离线，stdout 同时输出该 JSON。",
);

const HELP_CONFIG_CHECK_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee config check [--format text|json]",
        "Run the five local checks (runtime-base, runtime-directory, cohort, locale,\nsockets) and report pass/warning/fail/not-applicable. Exit 0 when no check\nfails, including when user-config has admitted field warnings; exit 1 on a\nfatal check. Windows user-config reads APPDATA/Orchbee/config.toml or the\nabsolute ORCHBEE_CONFIG_FILE override. Unreadable, oversized, non-UTF-8 or\ninvalid TOML and an invalid supported environment override are fatal. Unknown\nor Herdr-only fields warn and are ignored; an invalid supported leaf warns and\nuses its safe default. A bad key binding disables only that explicit action.\nSupported settings include terminal defaults/cwd, worktree root, restore policy,\nupdate policy, keys, title, notifications and both sidebar profiles.\nORCHBEE_RESUME_AGENTS_ON_RESTORE=true|false overrides the file. JSON v13 adds\nbounded diagnostics and their exact omitted count; versions 1-12 stay closed.\nserver reload-config atomically publishes admitted settings and diagnostics;\nrestore changes require restart. Explicit creation arguments override admitted\nsettings. A missing default file uses defaults; a missing explicit file fails.",
    );
const HELP_CONFIG_CHECK_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee config check [--format text|json]",
        "运行五项本地检查（runtime-base、runtime-directory、cohort、locale、\nsockets），逐项报告 pass/warning/fail/not-applicable。只要没有检查失败就退出 0，包括 user-config 仅含已接纳字段警告时；致命检查退出 1。Windows\nuser-config 读取 APPDATA/Orchbee/config.toml 或 ORCHBEE_CONFIG_FILE 指定的绝对文件。文件不可读、过大、非 UTF-8、TOML 无效或已支持的环境覆盖无效均为致命错误。未知或仅属于 Herdr 的字段会警告并忽略；已支持叶字段无效时会警告并使用其安全默认值。错误快捷键只禁用对应的显式动作。支持的设置包括终端默认值/cwd、worktree 根目录、恢复和更新策略、按键、标题、通知与两种侧栏 profile。ORCHBEE_RESUME_AGENTS_ON_RESTORE=true|false\n覆盖文件。JSON v13 增加有界诊断及精确省略数；版本 1-12 保持封闭。\nserver reload-config 原子发布已接纳设置和诊断；恢复策略变化需要重启。显式创建参数覆盖已接纳设置。默认文件不存在时使用默认值；显式文件不存在则失败。",
    );

// ---- text presentation labels --------------------------------------------

/// Column headers and field names for the human text surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CliLabels {
    pub(crate) bound_terminals_word: &'static str,
    pub(crate) bound_to: &'static str,
    pub(crate) catalog_revision: &'static str,
    pub(crate) child_presentations: &'static str,
    pub(crate) context: &'static str,
    pub(crate) counts: &'static str,
    pub(crate) directories: &'static str,
    pub(crate) directory_id: &'static str,
    pub(crate) group: &'static str,
    pub(crate) id: &'static str,
    pub(crate) launch: &'static str,
    pub(crate) launch_profile: &'static str,
    pub(crate) launch_program: &'static str,
    pub(crate) launch_arguments: &'static str,
    pub(crate) number: &'static str,
    pub(crate) panels_word: &'static str,
    pub(crate) path: &'static str,
    pub(crate) pinned: &'static str,
    pub(crate) primary_directory: &'static str,
    pub(crate) primary_presentation: &'static str,
    pub(crate) project: &'static str,
    pub(crate) project_id: &'static str,
    pub(crate) projects_word: &'static str,
    pub(crate) root: &'static str,
    pub(crate) status_running: &'static str,
    pub(crate) status_session: &'static str,
    pub(crate) status_session_name: &'static str,
    pub(crate) status_client_version: &'static str,
    pub(crate) slot_path: &'static str,
    pub(crate) right_click: &'static str,
    pub(crate) tab: &'static str,
    pub(crate) tab_id: &'static str,
    pub(crate) tabs: &'static str,
    pub(crate) tabs_word: &'static str,
    pub(crate) terminal: &'static str,
    pub(crate) terminal_context: &'static str,
    pub(crate) worker: &'static str,
    pub(crate) workspace: &'static str,
    pub(crate) workspaces: &'static str,
    pub(crate) workspaces_word: &'static str,
    pub(crate) standalone: &'static str,
    pub(crate) project_headers: [&'static str; 4],
    pub(crate) directory_headers: [&'static str; 3],
    pub(crate) launch_profile_headers: [&'static str; 3],
    pub(crate) worktree_headers: [&'static str; 4],
    pub(crate) workspace_ref_headers: [&'static str; 2],
    pub(crate) workspace_headers: [&'static str; 3],
    pub(crate) tab_ref_headers: [&'static str; 2],
    pub(crate) tab_headers: [&'static str; 4],
    pub(crate) panel_headers: [&'static str; 9],
    pub(crate) presentation_headers: [&'static str; 4],
    pub(crate) terminal_headers: [&'static str; 6],
    pub(crate) terminal_list_headers: [&'static str; 6],
    // Agent read field labels and column headers.
    pub(crate) agent_session: &'static str,
    pub(crate) agent_provider: &'static str,
    pub(crate) agent_lifecycle: &'static str,
    pub(crate) agent_status: &'static str,
    pub(crate) agent_freshness: &'static str,
    pub(crate) agent_location: &'static str,
    pub(crate) agent_observation: &'static str,
    pub(crate) agent_generation: &'static str,
    pub(crate) agent_sequence: &'static str,
    pub(crate) agent_capabilities: &'static str,
    pub(crate) agent_last_outcome: &'static str,
    pub(crate) agent_pending_requests: &'static str,
    pub(crate) agent_attachments: &'static str,
    pub(crate) agent_status_basis: &'static str,
    pub(crate) agent_freshness_meaning: &'static str,
    pub(crate) agent_capability_ceiling: &'static str,
    pub(crate) agent_source: &'static str,
    pub(crate) agent_fidelity: &'static str,
    pub(crate) agent_navigate_only: &'static str,
    pub(crate) agent_none: &'static str,
    #[cfg(not(windows))]
    pub(crate) agent_headers: [&'static str; 5],
    pub(crate) agent_request_headers: [&'static str; 3],
    pub(crate) agent_attachment_headers: [&'static str; 4],
    // `agent doctor` text section headers.
    pub(crate) agent_doctor_providers: &'static str,
    pub(crate) agent_doctor_endpoint: &'static str,
    pub(crate) agent_doctor_environment: &'static str,
    pub(crate) agent_doctor_asset_roots: &'static str,
    pub(crate) agent_doctor_hints: &'static str,
    // `agent integration list` columns — provider, its PATH availability,
    // the management mode, the observed state, and the issue when present.
    pub(crate) integration_headers: [&'static str; 5],
    // `workspace close-status` field labels, the member table columns, and
    // the empty-record notice.
    pub(crate) close_request_id: &'static str,
    pub(crate) workspace_id: &'static str,
    pub(crate) attempt_generation: &'static str,
    pub(crate) status: &'static str,
    pub(crate) members: &'static str,
    pub(crate) close_not_closing: &'static str,
    pub(crate) close_member_headers: [&'static str; 4],
}

const EN_US_LABELS: CliLabels = CliLabels {
    bound_terminals_word: "bound terminals",
    bound_to: "Bound to",
    catalog_revision: "Catalog revision",
    child_presentations: "Child presentations",
    context: "Context",
    counts: "Counts",
    directories: "Directories",
    directory_id: "Directory ID",
    group: "Group",
    id: "ID",
    launch: "Launch",
    launch_profile: "Launch profile",
    launch_program: "Launch program",
    launch_arguments: "Launch arguments",
    panels_word: "panels",
    path: "Path",
    pinned: "Pinned",
    primary_directory: "Primary dir",
    primary_presentation: "Primary presentation",
    project: "Project",
    project_id: "Project ID",
    projects_word: "projects",
    root: "Root",
    status_running: "running",
    status_session: "Session",
    status_session_name: "Session name",
    status_client_version: "Client version",
    slot_path: "Slot path",
    tab: "Tab",
    tab_id: "Tab ID",
    tabs: "Tabs",
    tabs_word: "tabs",
    terminal: "Terminal",
    terminal_context: "Context ID",
    worker: "Worker",
    workspace: "Workspace",
    workspaces: "Workspaces",
    workspaces_word: "workspaces",
    standalone: "Standalone",
    project_headers: ["ID", "LABEL", "DIRECTORIES", "WORKSPACES"],
    directory_headers: ["ID", "WORKER", "PATH"],
    launch_profile_headers: ["PROFILE ID", "NAME", "LAUNCH"],
    worktree_headers: ["PATH", "HEAD", "BRANCH", "STATE"],
    workspace_ref_headers: ["ID", "LABEL"],
    number: "Number",
    workspace_headers: ["HANDLE", "NAME", "PROJECT"],
    tab_ref_headers: ["ID", "LABEL"],
    tab_headers: ["HANDLE", "NAME", "PANELS", "WORKSPACE"],
    right_click: "Right click",
    panel_headers: [
        "ID",
        "LABEL",
        "WORKSPACE",
        "TAB",
        "CONTEXT",
        "PRIMARY PRESENTATION",
        "PRESENTATIONS",
        "PRIMARY TERMINAL",
        "RIGHT CLICK",
    ],
    presentation_headers: ["ID", "LABEL", "TERMINAL", "WORKER"],
    terminal_headers: ["ID", "WORKER", "PRESENTATION", "PANEL", "TAB", "CONTEXT"],
    terminal_list_headers: ["#", "HANDLE", "TITLE", "AGENT", "STATE", "CWD"],
    agent_session: "Session",
    agent_provider: "Provider",
    agent_lifecycle: "Lifecycle",
    agent_status: "Status",
    agent_freshness: "Freshness",
    agent_location: "Location",
    agent_observation: "Observation",
    agent_generation: "generation",
    agent_sequence: "sequence",
    agent_capabilities: "Capabilities",
    agent_last_outcome: "Last outcome",
    agent_pending_requests: "Pending requests",
    agent_attachments: "Presentations",
    agent_status_basis: "Status basis",
    agent_freshness_meaning: "Freshness meaning",
    agent_capability_ceiling: "Capability ceiling",
    agent_source: "Source",
    agent_fidelity: "Fidelity",
    agent_navigate_only: "Facts",
    agent_none: "none",
    #[cfg(not(windows))]
    agent_headers: ["SESSION", "PROVIDER", "STATUS", "FRESHNESS", "LOCATION"],
    agent_request_headers: ["KIND", "REQUEST", "TURN"],
    agent_attachment_headers: ["PRESENTATION", "GENERATION", "NAVIGABLE", "VISIBILITY"],
    agent_doctor_providers: "Providers",
    agent_doctor_endpoint: "Observation endpoint",
    agent_doctor_environment: "Environment",
    agent_doctor_asset_roots: "Asset roots",
    agent_doctor_hints: "Hints",
    integration_headers: ["PROVIDER", "AVAILABILITY", "MANAGEMENT", "STATE", "ISSUE"],
    close_request_id: "Close request ID",
    workspace_id: "Workspace ID",
    attempt_generation: "Attempt generation",
    status: "Status",
    members: "Members",
    close_not_closing: "no close operation recorded for this workspace",
    close_member_headers: ["TAB", "PANEL", "PRESENTATION", "STATUS"],
};

const ZH_CN_LABELS: CliLabels = CliLabels {
    bound_terminals_word: "已绑定终端",
    bound_to: "绑定到",
    catalog_revision: "配置目录修订",
    child_presentations: "子 Presentation",
    context: "上下文",
    counts: "计数",
    directories: "目录",
    directory_id: "目录 ID",
    group: "分组",
    id: "ID",
    launch: "启动",
    launch_profile: "启动配置",
    launch_program: "启动程序",
    launch_arguments: "启动参数",
    panels_word: "面板",
    path: "路径",
    pinned: "置顶",
    primary_directory: "主目录",
    primary_presentation: "主 Presentation",
    project: "Project",
    project_id: "Project ID",
    projects_word: "项目",
    root: "根节点",
    status_running: "运行中",
    status_session: "会话",
    status_session_name: "会话名称",
    status_client_version: "客户端版本",
    slot_path: "Slot 路径",
    tab: "Tab",
    tab_id: "Tab ID",
    tabs: "Tab",
    tabs_word: "标签",
    terminal: "终端",
    terminal_context: "上下文 ID",
    worker: "Worker",
    workspace: "工作区",
    workspaces: "工作区",
    workspaces_word: "工作区",
    standalone: "独立上下文",
    project_headers: ["ID", "LABEL", "目录数", "工作区数"],
    directory_headers: ["ID", "WORKER", "路径"],
    launch_profile_headers: ["ID", "名称", "启动"],
    worktree_headers: ["路径", "HEAD", "分支", "状态"],
    workspace_ref_headers: ["ID", "LABEL"],
    number: "编号",
    workspace_headers: ["句柄", "名称", "项目"],
    tab_ref_headers: ["ID", "LABEL"],
    tab_headers: ["句柄", "名称", "面板", "工作区"],
    right_click: "右键",
    panel_headers: [
        "ID",
        "名称",
        "工作区",
        "TAB",
        "上下文",
        "主 PRESENTATION",
        "PRESENTATION 数",
        "主终端",
        "右键",
    ],
    presentation_headers: ["ID", "LABEL", "终端", "WORKER"],
    terminal_headers: ["ID", "WORKER", "PRESENTATION", "PANEL", "TAB", "上下文"],
    terminal_list_headers: ["序号", "句柄", "标题", "Agent", "状态", "目录"],
    agent_session: "会话",
    agent_provider: "提供方",
    agent_lifecycle: "生命周期",
    agent_status: "状态",
    agent_freshness: "新鲜度",
    agent_location: "位置",
    agent_observation: "观察",
    agent_generation: "代次",
    agent_sequence: "序号",
    agent_capabilities: "能力",
    agent_last_outcome: "最近结果",
    agent_pending_requests: "待处理请求",
    agent_attachments: "Presentation",
    agent_status_basis: "状态依据",
    agent_freshness_meaning: "新鲜度含义",
    agent_capability_ceiling: "能力上限",
    agent_source: "来源",
    agent_fidelity: "保真度",
    agent_navigate_only: "事实",
    agent_none: "无",
    #[cfg(not(windows))]
    agent_headers: ["会话", "提供方", "状态", "新鲜度", "位置"],
    agent_request_headers: ["类型", "请求", "轮次"],
    agent_attachment_headers: ["PRESENTATION", "代次", "可导航", "可见性"],
    agent_doctor_providers: "提供方",
    agent_doctor_endpoint: "观察端点",
    agent_doctor_environment: "环境",
    agent_doctor_asset_roots: "资产根",
    agent_doctor_hints: "提示",
    integration_headers: ["提供方", "可用性", "管理方式", "状态", "问题"],
    close_request_id: "关闭请求 ID",
    workspace_id: "工作区 ID",
    attempt_generation: "尝试代次",
    status: "状态",
    members: "成员",
    close_not_closing: "该工作区没有已记录的关闭操作",
    close_member_headers: ["TAB", "PANEL", "PRESENTATION", "状态"],
};

impl Locale {
    pub(crate) const fn cli_labels(self) -> &'static CliLabels {
        match self {
            Self::EnUs => &EN_US_LABELS,
            Self::ZhCn => &ZH_CN_LABELS,
        }
    }

    pub(crate) const fn cli_terminal_list_state(
        self,
        state: crate::cli::results::TerminalListState,
    ) -> &'static str {
        use crate::cli::results::TerminalListState;
        use orchbee_workbench_protocol::PanelAgentState;
        match (self, state) {
            (_, TerminalListState::Observed) => match self {
                Self::EnUs => "observed",
                Self::ZhCn => "已观察",
            },
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Idle)) => "idle",
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Working)) => "working",
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Blocked)) => "blocked",
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Done)) => "done",
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Error)) => "error",
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Unknown)) => "unknown",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Idle)) => "空闲",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Working)) => "工作中",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Blocked)) => "受阻",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Done)) => "已完成",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Error)) => "错误",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Unknown)) => "未知",
        }
    }
}

const HELP_PANEL_CURRENT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel current [--panel <panel-id> | --current] [--client <id>] [--format text|json]",
        "Read a Panel, Tab and Workspace from a running Windows session.\nAn explicit Panel ID or @panel:<suffix> ignores caller environment and needs no TUI.\nOtherwise use validated ORCHBEE_TERMINAL_ID and ORCHBEE_WORKER_ID. If both are absent,\nuse the only current TUI Client; multiple Clients require --client. --client ignores\ncaller environment and cannot combine with --panel. Partial, invalid or stale caller\nmetadata never falls back. The Client must be ready or busy with a valid selected Panel.\nIncludes primary Agent status, terminal title/cwd and process observation.\nA selected Client supplies focus and scroll; changes during the read are rejected.\nFive-second total deadline; Ctrl+C cancels. No focus changes or session activation.",
    );
const HELP_PANEL_CURRENT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel current [--panel <panel-id> | --current] [--client <id>] [--format text|json]",
        "从运行中的 Windows 会话读取 Panel、Tab 和 Workspace。显式 Panel ID 或 @panel:<suffix> 忽略调用者环境，无需 TUI。否则校验 ORCHBEE_TERMINAL_ID 与 ORCHBEE_WORKER_ID；两个变量均缺失时，使用唯一 current TUI Client，多个 Client 必须用 --client 指定。--client 忽略调用者环境，不能与 --panel 同用。部分缺失、无效或过期的调用者信息绝不回退。\nClient 必须处于就绪或忙碌状态，且所选 Panel 仍有效。补齐主 Agent 状态、终端标题/cwd 和进程观察。选定 Client 时包含焦点和滚动信息；读取中途变化则拒绝。总期限五秒；Ctrl+C 取消。不改变焦点，不激活会话。",
    );

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
            (Self::ZhCn, false) => format!("提示输入已送达 Agent {session} 的终端。\n"),
            (Self::ZhCn, true) => {
                format!("提示输入已送达 Agent {session} 的终端，等待条件已匹配。\n")
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
                Some("Agent 仍在启动。请等待 agent start 完成后再提交提示。")
            }
            (Self::EnUs, "agent-blocked") => Some(
                "Agent needs attention. Resolve its pending request before submitting a prompt.",
            ),
            (Self::ZhCn, "agent-blocked") => Some("Agent 需要处理待决请求。处理后再提交提示。"),
            (Self::EnUs, "agent-not-ready") => Some(
                "Agent state is not fresh and known. Inspect agent get/explain before submitting.",
            ),
            (Self::ZhCn, "agent-not-ready") => {
                Some("Agent 状态不够新鲜或仍未知。提交前请检查 agent get/explain。")
            }
            (Self::EnUs, "prompt-stalled" | "effect-deadline") => Some(
                "Input was accepted but no new Agent activity was observed. Inspect the terminal; do not blindly resend.",
            ),
            (Self::ZhCn, "prompt-stalled" | "effect-deadline") => {
                Some("输入已接受，但尚未观察到新的 Agent 活动。请检查终端，不要盲目重发。")
            }
            (Self::EnUs, "client-read-failed") => Some(
                "The selected Client could not be read. Inspect the failure and any retained input receipt before retrying.",
            ),
            (Self::ZhCn, "client-read-failed") => {
                Some("无法读取所选 Client。重试前请检查失败信息及保留的输入回执。")
            }
            _ => None,
        }
    }
}

const HELP_PANEL_PROCESS_INFO_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel process-info [<panel-id> | --panel <panel-id> | --current] [--client <id>] [--format text|json]",
        "Read the primary Terminal's Windows Job process information. Complete Panel IDs\nand @panel:<suffix> are accepted. Explicit Panel selectors are exclusive and conflict\nwith --client. Otherwise use validated caller context; only when both caller variables\nare absent, select the only current TUI Client. --client selects a Client and ignores\ncaller environment. Partial, invalid or stale caller metadata never falls back.\nThe Client must be ready or busy with a valid selected Panel. Each sample reports owned\nprocesses with PID, creation time, image, command line and cwd; unavailable optional\nfields are null. Five-second total deadline; Ctrl+C cancels. No focus or input changes.",
    );
const HELP_PANEL_PROCESS_INFO_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel process-info [<panel-id> | --panel <panel-id> | --current] [--client <id>] [--format text|json]",
        "读取主 Terminal 的 Windows Job 进程信息，接受完整 Panel ID 或 @panel:<suffix>。显式 Panel 选择器互斥，也不能与 --client 同用。否则使用经校验的调用者上下文；仅当两个调用者变量均缺失时，选择唯一 current TUI Client。--client 指定 Client\n并忽略调用者环境。部分缺失、无效或过期的调用者信息绝不回退。Client 必须已就绪且所选 Panel 仍有效。样本列出归属进程的 PID、创建时间、映像、命令行和 cwd；不可用的可选字段为 null。总期限五秒；Ctrl+C 取消。不改变焦点或输入。",
    );

impl Locale {
    pub(crate) const fn cli_process_headers(self) -> [&'static str; 8] {
        match self {
            Self::EnUs => [
                "PID",
                "Parent PID",
                "Created",
                "Image",
                "Command line",
                "Directory",
                "Arguments",
                "Provider",
            ],
            Self::ZhCn => [
                "PID",
                "父 PID",
                "创建时间",
                "映像",
                "命令行",
                "目录",
                "参数",
                "Provider",
            ],
        }
    }
    pub(crate) const fn cli_process_root(self) -> &'static str {
        match self {
            Self::EnUs => "Root PID",
            Self::ZhCn => "根 PID",
        }
    }
    pub(crate) const fn cli_process_foreground(self) -> &'static str {
        match self {
            Self::EnUs => "Foreground estimate (kind · PID · provider)",
            Self::ZhCn => "前台估计（类型 · PID · provider）",
        }
    }
    pub(crate) fn cli_agent_runtime_hint(self, reason: &str) -> Option<&'static str> {
        match (self, reason) {
            (Self::EnUs, "agent-provider-unsupported") => Some(
                "This Agent provider has no runtime identification rule. Inspect panel process-info before using terminal send explicitly.",
            ),
            (Self::ZhCn, "agent-provider-unsupported") => Some(
                "此 Agent provider 尚无运行时识别规则。请先检查 panel process-info，再显式使用 terminal send。",
            ),
            (Self::EnUs, "agent-not-running") => Some(
                "The terminal no longer contains the expected Agent. Inspect panel process-info and agent get before sending input.",
            ),
            (Self::ZhCn, "agent-not-running") => {
                Some("终端中已找不到预期 Agent。发送前请检查 panel process-info 和 agent get。")
            }
            (Self::EnUs, "agent-runtime-ambiguous") => Some(
                "Several Agent processes cannot be resolved to one foreground chain. Inspect panel process-info before sending input.",
            ),
            (Self::ZhCn, "agent-runtime-ambiguous") => {
                Some("多个 Agent 进程无法确定为一条前台链。发送前请检查 panel process-info。")
            }
            (Self::EnUs, "agent-provider-mismatch") => Some(
                "The running Agent differs from the selected session. Inspect panel process-info and select the intended Agent.",
            ),
            (Self::ZhCn, "agent-provider-mismatch") => Some(
                "实际运行的 Agent 与所选会话不同。请检查 panel process-info 并重新选择目标 Agent。",
            ),
            (Self::EnUs, "agent-binding-changed") => Some(
                "The Agent binding changed during preflight. Resolve the target again before sending input.",
            ),
            (Self::ZhCn, "agent-binding-changed") => {
                Some("预检期间 Agent 绑定已变化。发送前请重新解析目标。")
            }
            _ => None,
        }
    }
}

impl Locale {
    pub(crate) const fn cli_agent_names_header(self, terminal_targets: bool) -> &'static str {
        match (self, terminal_targets) {
            (Self::EnUs, true) => "NAME\tTARGET\n",
            (Self::ZhCn, true) => "名称\t目标\n",
            (Self::EnUs, false) => "NAME\tSESSION\n",
            (Self::ZhCn, false) => "名称\t会话\n",
        }
    }
    pub(crate) fn cli_agent_terminal_name_target(self, presentation: &str) -> String {
        match self {
            Self::EnUs => format!("terminal:{presentation}"),
            Self::ZhCn => format!("终端:{presentation}"),
        }
    }
    pub(crate) fn cli_agent_name_assigned(self, name: &str, target: &str) -> String {
        match self {
            Self::EnUs => format!("{name} -> {target}\n"),
            Self::ZhCn => format!("{name} → {target}\n"),
        }
    }
    pub(crate) fn cli_agent_name_cleared(self, target: &str) -> String {
        match self {
            Self::EnUs => format!("{target}: name cleared\n"),
            Self::ZhCn => format!("{target}：名称已清除\n"),
        }
    }
    pub(crate) const fn cli_agent_pending_names_header(self) -> &'static str {
        match self {
            Self::EnUs => "\nPENDING NAME\tPROVIDER\tREMAINING MS\tRESERVATION\n",
            Self::ZhCn => "\n预留名称\t提供方\t剩余毫秒\t预留 ID\n",
        }
    }
}

impl Locale {
    #[cfg(windows)]
    pub(crate) fn cli_agent_started(self, name: &str, session: &str) -> String {
        match self {
            Self::EnUs => format!("Agent {name} is ready ({session}).\n"),
            Self::ZhCn => format!("Agent {name} 已就绪（{session}）。\n"),
        }
    }

    fn cli_agent_start_failure(self, failure: &Failure) -> String {
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
                Self::ZhCn => "未找到目标 Panel。请运行 `orchbee panel list`，再将现有 Panel ID 传给 --panel。\n",
            }.to_owned();
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
                "Agent 名称或终端已被占用。请用 `orchbee agent names` 查看。"
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
                "Agent 未在时限内确认就绪。请检查终端输出和提供方 hook。"
            }
            (Self::EnUs, "agent-blocked") => {
                "The Agent needs attention. Resolve the prompt shown in its terminal."
            }
            (Self::ZhCn, "agent-blocked") => "Agent 需要人工处理，请查看其终端中的提示。",
            (Self::EnUs, "agent-exited") => {
                "The Agent exited before readiness was confirmed. Inspect its terminal output."
            }
            (Self::ZhCn, "agent-exited") => "Agent 在确认就绪前已退出，请检查其终端输出。",
            (Self::EnUs, "agent-provider-mismatch" | "agent-runtime-ambiguous") => {
                "The running Agent does not uniquely match the requested provider."
            }
            (Self::ZhCn, "agent-provider-mismatch" | "agent-runtime-ambiguous") => {
                "运行中的 Agent 无法唯一匹配所请求的提供方。"
            }
            (Self::EnUs, "binding-changed") => "The target terminal changed during startup.",
            (Self::ZhCn, "binding-changed") => "目标终端在启动期间发生了变化。",
            (Self::EnUs, "reply-not-confirmed") => {
                "The startup operation's result is unknown. Inspect Agent names and the terminal before retrying."
            }
            (Self::ZhCn, "reply-not-confirmed") => {
                "启动操作的结果尚不确定。重试前请检查 Agent 名称和终端。"
            }
            (Self::EnUs, "cancelled") => "Agent startup monitoring was cancelled.",
            (Self::ZhCn, "cancelled") => "已取消等待 Agent 启动。",
            (Self::EnUs, _) => {
                "Agent startup could not be confirmed. Inspect the command result and terminal."
            }
            (Self::ZhCn, _) => "无法确认 Agent 启动成功，请检查命令结果和终端。",
        };
        let reservation = match (self, field("reservationState")) {
            (Self::EnUs, "pending" | "unknown") => {
                " The name reservation may remain until expiry; `agent names` shows pending reservations."
            }
            (Self::ZhCn, "pending" | "unknown") => {
                " 名称预留可能保留到到期；`agent names` 可查看尚未完成的预留。"
            }
            (Self::EnUs, "released") => " The name reservation was released.",
            (Self::ZhCn, "released") => " 名称预留已释放。",
            (Self::EnUs, "completed") => " The Agent name was already committed.",
            (Self::ZhCn, "completed") => " Agent 名称已经绑定。",
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
            (Self::ZhCn, true) => " 启动输入已尝试提交一次，Agent 可能仍在运行；没有终止进程。",
            _ => "",
        };
        format!("{message}{reservation}{delivery}\n")
    }
}

const HELP_SITE_STOP_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee server stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>] [--format text|json]",
    "Stop the existing managed Windows session and its terminals. This ends running\nprocesses after a fresh final save of Workbench configuration, sampled\nper-terminal cwd and provider restart references. Save failure keeps preceding\nvalid snapshots, retained invalid originals and running terminals; resolve it\nand explicitly retry. Live Agent state, terminal history and screen contents\nare not saved. --confirm is required; there is no interactive prompt and no\nsession activation. --detach returns the admitted attempt immediately. Otherwise\nwait up to 65000 ms by default (1–3600000 ms). Cancellation/timeout stops\nwaiting, not the accepted shutdown.\n\nA nonzero 32-character lowercase hexadecimal request ID is generated if omitted.\nDuplicate starts observe the existing attempt; --retry requires --request-id\nand explicitly retries only that failed stop. Use server stop-status <id> to\nrecover a result. Completion requires the matching stopped record and a free\nlifetime lock; pipe disappearance alone is insufficient. A later server start\nconsumes the receipt. Linux and macOS are unsupported.",
);
const HELP_SITE_STOP_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee server stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>] [--format text|json]",
    "停止现有托管 Windows 会话及其终端；先对 Workbench 配置、采样所得的逐终端 cwd\n及 provider 恢复引用执行最新 final 保存，再结束进程。保存失败会保留此前的有效存档、无效原件及运行中的终端；解决故障后显式重试。不保存存活 Agent 状态、终端历史或屏幕内容。必须提供 --confirm；不弹出交互确认，也不激活会话。--detach 立即返回已准入尝试；否则默认等待 65000 毫秒（范围 1–3600000）。取消或超时只停止等待，不取消已接受的停止。\n\n省略请求标识时自动生成非零 32 位小写十六进制标识。重复开始只观察现有尝试；\n--retry 必须搭配 --request-id，显式重试该失败停止。使用 server stop-status <id>\n恢复结果。仅准确匹配的已停止记录加空闲生命周期锁能证明完成，管道消失不够。后续 server start 会消费回执。Linux 和 macOS 尚不支持。",
);
const HELP_SITE_STOP_STATUS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server stop-status <request-id> [--format text|json]",
        "Read a managed Windows stop attempt or its completed activation receipt.\nNever starts or stops a session. One five-second deadline bounds the query.\nA later start consumes the completed receipt; not-recorded does not prove\nthat a previous stop failed. Use --format json for phase and failure details.",
    );
const HELP_SITE_STOP_STATUS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server stop-status <request-id> [--format text|json]",
        "读取托管 Windows 停止尝试或其已完成激活回执，绝不启动或停止会话。查询共用五秒截止时间。后续启动会消费完成回执，未记录不代表此前停止失败。使用 --format json 查看阶段和失败详情。",
    );

impl Locale {
    /// Localizes a closed stop outcome and prints its identity for later status recovery.
    pub(crate) fn cli_site_stop_result(
        self,
        outcome: &str,
        id: &str,
        wait: Option<&str>,
    ) -> String {
        let state = match (self, outcome) {
            (Self::EnUs, "stopped") => "stopped",
            (Self::ZhCn, "stopped") => "已停止",
            (Self::EnUs, "pending") => "pending",
            (Self::ZhCn, "pending") => "处理中",
            (Self::EnUs, "failed") => "failed",
            (Self::ZhCn, "failed") => "失败",
            (Self::EnUs, "not-recorded") => "not recorded",
            (Self::ZhCn, "not-recorded") => "未记录",
            (Self::EnUs, "busy") => "another stop owns this session",
            (Self::ZhCn, "busy") => "另一停止请求已占用此会话",
            (Self::EnUs, "unmanaged") => "not a managed session",
            (Self::ZhCn, "unmanaged") => "非托管会话",
            (Self::EnUs, "unavailable") => "unavailable",
            (Self::ZhCn, "unavailable") => "不可用",
            (Self::EnUs, _) => "outcome unknown",
            (Self::ZhCn, _) => "结果未知",
        };
        let waiting = match (self, wait) {
            (Self::EnUs, Some("detached")) => " (detached)",
            (Self::ZhCn, Some("detached")) => "（后台执行）",
            (Self::EnUs, Some("timed-out")) => " (wait timed out)",
            (Self::ZhCn, Some("timed-out")) => "（等待超时）",
            (Self::EnUs, Some("cancelled")) => " (wait cancelled)",
            (Self::ZhCn, Some("cancelled")) => "（已取消等待）",
            _ => "",
        };
        match self {
            Self::EnUs => format!(
                "Session stop: {state}{waiting}\nRequest: {id}\nInspect: orchbee server stop-status {id} --format json\n"
            ),
            Self::ZhCn => format!(
                "会话停止：{state}{waiting}\n请求：{id}\n检查：orchbee server stop-status {id} --format json\n"
            ),
        }
    }
}

const HELP_PANEL_SWAP_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel swap [<source>] (--with <target> | --direction left|right|up|down) [--panel <source>|--current] [--client <id>] [--timeout <ms>] [--format text|json]",
    "Exchange two complete Panel positions in one Tab, then focus the source in the selected Windows TUI. Source selectors are exclusive; omitted source uses valid caller context or the selected Client. Targets are exact IDs or @panel:<suffix>. Multiple Clients require an explicit selection.\n\nSame Panel, cross-Tab pair and no neighbor leave layout and focus unchanged. Positions commit before focus acknowledgment: a later failure preserves the committed receipt; never automatically retry a swap. One total timeout defaults to 30000 ms (1..3600000). No --detach or --request-id.",
);
const HELP_PANEL_SWAP_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel swap [<source>] (--with <target> | --direction left|right|up|down) [--panel <source>|--current] [--client <id>] [--timeout <ms>] [--format text|json]",
    "交换同一 Tab 中两个完整 Panel 的位置，再在所选 Windows TUI 聚焦来源。来源选择器互斥；省略时使用有效调用者上下文或所选 Client。目标为精确 ID 或 @panel:<suffix>。多个 Client 时须明确选择。\n\n同一 Panel、跨 Tab 或没有邻居不改变布局和焦点。位置提交先于聚焦确认；后续失败保留提交回执，绝不自动重试交换。总超时默认 30000 ms（1..3600000）。不支持 --detach 或 --request-id。",
);

const HELP_PANEL_INPUT_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel input (<panel-id> | --panel <panel-id> | --current) --right-click orchbee|panel [--format text|json]",
    "Windows: set the shared Panel right-button policy without changing focus.\n--current uses validated calling Terminal/Worker context; explicit targets\nbypass that context. orchbee reserves right-click to the Client; panel permits\nunmodified right clicks inside a reporting terminal. Existing controller and\nreadiness gates still apply. A policy change applies to the next gesture.\nconfigured or unchanged acknowledges canonical state, not every Client draw.\nAn unknown outcome is never automatically retried.",
);
const HELP_PANEL_INPUT_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel input (<panel-id> | --panel <panel-id> | --current) --right-click orchbee|panel [--format text|json]",
    "Windows：设置共享 Panel 右键策略，不改变焦点。--current 校验调用者\nTerminal/Worker 上下文；显式目标绕过该上下文。orchbee 将右键保留给\nClient；panel 允许无修饰键的右键在启用鼠标上报的终端内容中透传。现有控制权及就绪约束仍适用，策略改变作用于下一个手势。\nconfigured 或 unchanged 确认规范状态，不确认所有 Client 已绘制。未知结果不会自动重试。",
);

impl Locale {
    pub(crate) const fn cli_agent_change_headers(self) -> [&'static str; 4] {
        match self {
            Self::EnUs => ["REVISION", "SESSION", "STATUS", "FRESHNESS"],
            Self::ZhCn => ["修订", "会话", "状态", "新鲜度"],
        }
    }
    pub(crate) fn cli_agent_changes_progress(self, observed: u64, head: u64) -> String {
        match self {
            Self::EnUs => format!("Observed through revision {observed}; sampled head {head}.\n"),
            Self::ZhCn => format!("已观察至修订 {observed}；采样头部为 {head}。\n"),
        }
    }
    pub(crate) fn cli_agent_changes_checkpoint(self, revision: u64, reset: bool) -> String {
        match (self, reset) {
            (Self::EnUs, false) => format!("Current through revision {revision}.\n"),
            (Self::ZhCn, false) => format!("已观察至修订 {revision}。\n"),
            (Self::EnUs, true) => {
                format!("History lost; rebuild the baseline at revision {revision}.\n")
            }
            (Self::ZhCn, true) => format!("历史已丢失；请在修订 {revision} 重建基线。\n"),
        }
    }
}

const HELP_AGENT_WAIT_WINDOWS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent wait <target> [--client <id>] [--until <condition>]... [--after-revision <n>] [--timeout <ms> | --forever] [--format text|json]",
        "Wait on one pinned Agent, using current samples and retained change pages.\nWindows defaults to idle, done, blocked with no total timeout. --timeout\nsets 1-3600000 ms for the entire operation; --forever is an explicit synonym\nfor no total timeout and conflicts with --timeout. Each read is bounded to\nfive seconds. Ctrl+C ends the wait; no read failure reconnects.\nConditions: blocked, done, needs-attention, failed, succeeded, canceled,\ninterrupted, working, idle, unknown, ended, fresh, stale, disconnected.\nStatus conditions require fresh observations. blocked means needs-attention;\ndone means succeeded/failed/canceled/interrupted, without Client unseen state.\n--after-revision is a global fence, not a turn identity.\nTransient matching changes remain evidence even if the latest state differs.\nVersion 3 JSON separates matchedChange from the current session/revision;\nnull matchedChange means the current sample matched. Lost history or a\nforeign cursor fails explicitly. An ended session without a match is rejected.\n--client pins one exact TUI. Its current seen resting state means idle;\nunseen resting state means done. Missing/stale versions keep waiting.\nThese Client states are sampled; canonical idle/done events cannot prove\npast acknowledgements. Version 4 adds the Client projection.\nTarget: Session ID/name/Panel ID, --agent-session <id>, --name <alias>, or\n--panel <id|@panel:suffix>. Resolution occurs once, before waiting.",
    );

const HELP_AGENT_WAIT_WINDOWS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent wait <target> [--client <id>] [--until <条件>]... [--after-revision <n>] [--timeout <毫秒> | --forever] [--format text|json]",
        "固定一个 Agent，结合当前样本和保留变更页等待。\nWindows 默认 idle、done、blocked，无总超时。--timeout 设置整个操作的\n1-3600000 毫秒期限；--forever 是无总超时的显式同义选项，与 --timeout 互斥。每次读取最多五秒；Ctrl+C 结束等待，读取失败不重新连接。条件：blocked、done、needs-attention、failed、succeeded、canceled、\ninterrupted、working、idle、unknown、ended、fresh、stale、disconnected。状态条件要求 fresh；blocked 表示 needs-attention；done 表示\nsucceeded/failed/canceled/interrupted，尚不合并 Client 未看见状态。\n--after-revision 是全局修订门槛，不代表回合身份。即使最新状态不同，短暂命中的变更仍作为证据返回。版本 3 JSON\n将 matchedChange 与当前 session/revision 分开；空 matchedChange 表示当前样本命中。历史丢失或游标跨启动周期时明确失败。会话已结束且没有条件命中时拒绝。\n--client 固定一个 TUI；其当前已看见的静止状态为 idle，未看见为 done。缺失或过期版本继续等待。Client 状态仅采样，规范 idle/done 事件不能证明过去的已读确认。版本 4 增加 Client 投影。目标：Session ID/名称/Panel ID、--agent-session <id>、--name <alias> 或\n--panel <id|@panel:suffix>。等待前只解析一次。",
    );

const HELP_TERMINAL_CONTROL_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal session control <terminal> [--takeover] [--cols N] [--rows N] [--timeout ms] [--format text|json]",
        "Windows: acquire exclusive input/resize control of an existing terminal and\nstream validated ANSI/base64 NDJSON frames. Default geometry 120x40.\n--takeover explicitly revokes the old controller; failed takeover may still\nrevoke it. Read stdin as UTF-8 NDJSON: terminal.input (text XOR base64 bytes),\nterminal.resize (cols,rows), terminal.scroll (direction,lines), terminal.release.\nInput records are bounded to 2 MiB; decoded input to 256 KiB. Invalid commands\nproduce rejection records without effects; input is admitted in ordered 4 KiB\nchunks. Receipts do not mean shell execution. EOF/release detaches; it never\nstops the terminal. Cancellation 130, timeout 7; no reconnect or effect replay.\nOutput stalls/exchanges are bounded to 5s, cleanup 2s. Normal-screen scroll\nuses retained history; application mouse modes and alternate screens use\nthe corresponding input. See schema orchbee.terminal.control.input.",
    );
const HELP_TERMINAL_CONTROL_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal session control <terminal> [--takeover] [--cols N] [--rows N] [--timeout ms] [--format text|json]",
        "Windows：获取既有终端的排他输入/尺寸控制权，持续输出经过验证的\nANSI/base64 NDJSON 帧。默认尺寸120x40。--takeover 显式撤销旧控制者；接管失败也可能撤销旧权限。stdin 为 UTF-8 NDJSON：terminal.input\n（text 或 base64 bytes 二选一）、terminal.resize（cols、rows）、\nterminal.scroll（direction、lines）、terminal.release。每条记录最多2MiB，解码输入最多256KiB。无效命令返回无副作用的拒绝记录；输入按顺序分成4KiB 提交，接收回执不代表 shell 已执行。EOF/release 仅解除附着，不停止终端。取消130、超时7；不重连或重放效果。传输/输出停滞最多5秒，清理2秒。普通屏幕滚动使用历史，应用鼠标模式和备用屏幕使用对应输入。详见 schema orchbee.terminal.control.input。",
    );

impl Locale {
    pub(crate) const fn attach_requires_terminal(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Attach requires interactive terminal stdin and stdout. Use terminal session control for a pipe.\n"
            }
            Self::ZhCn => {
                "attach 需要交互终端的 stdin 和 stdout。管道输入请使用 terminal session control。\n"
            }
        }
    }
    pub(crate) const fn attach_input_unavailable(self) -> &'static str {
        match self {
            Self::EnUs => "Input unavailable",
            Self::ZhCn => "当前输入不可用",
        }
    }
    pub(crate) fn terminal_stream_failure(self, failure: &crate::cli::Failure) -> String {
        use crate::cli::FailureKind;
        let message = match (self, failure.kind) {
            (Self::EnUs, FailureKind::Timeout) => "The terminal stream deadline elapsed.\n",
            (Self::ZhCn, FailureKind::Timeout) => "终端流操作已超时。\n",
            (Self::EnUs, FailureKind::Cancelled) => "The terminal stream was cancelled.\n",
            (Self::ZhCn, FailureKind::Cancelled) => "终端流操作已取消。\n",
            (Self::EnUs, FailureKind::TransportLoss) => {
                "The terminal stream connection was lost.\n"
            }
            (Self::ZhCn, FailureKind::TransportLoss) => "终端流连接已中断。\n",
            (Self::EnUs, FailureKind::OutputTooLarge) => {
                "A terminal frame exceeded the output limit.\n"
            }
            (Self::ZhCn, FailureKind::OutputTooLarge) => "终端帧超过输出上限。\n",
            (Self::EnUs, FailureKind::InternalFailure) => {
                "The terminal stream failed internally. Previously delivered input is not undone.\n"
            }
            (Self::ZhCn, FailureKind::InternalFailure) => {
                "终端流发生内部错误。此前已交付的输入不会被撤销。\n"
            }
            (Self::EnUs, FailureKind::OutcomeUnknown) => {
                "The latest terminal operation could not be confirmed. Input was not replayed.\n"
            }
            (Self::ZhCn, FailureKind::OutcomeUnknown) => {
                "最近一次终端操作的结果无法确认。未重放输入。\n"
            }
            _ => "",
        };
        if !message.is_empty() {
            return message.to_owned();
        }
        if failure.kind == crate::cli::FailureKind::Rejected {
            let reason = failure
                .details
                .as_ref()
                .and_then(|v| v.get("reason"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("rejected");
            return match self {
                Self::EnUs => format!(
                    "Terminal attachment or control was refused ({reason}). Previously delivered input is not undone.\n"
                ),
                Self::ZhCn => {
                    format!("终端接入或控制被拒绝（{reason}）。此前已交付的输入不会被撤销。\n")
                }
            };
        }
        self.cli_failure(failure)
    }
}
const HELP_TERMINAL_ATTACH_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal attach <terminal> [--takeover] [--timeout ms] [--mouse on|off] [--format text|json]",
        "Windows interactive terminal attachment. Requires terminal stdin/stdout.\nUses an existing terminal ID or @terminal:suffix; never starts a session or focuses\na Workbench Panel. Uses the console size and exclusive input/resize control.\n--takeover revokes the old controller and may revoke it even on failure.\nCtrl+B then q detaches; Ctrl+B twice sends a literal Ctrl+B. Ctrl+C reaches\nthe application. Paste is semantic; plain page keys and wheel use history\nor the application's mouse/cursor modes. Mouse is on by default.\n--timeout is 1..3600000 ms; default unlimited, exchanges bounded to 5s, cleanup\nto 2s. No reconnect or input replay. Modes restore before diagnostics.\nOutput is an interactive screen with either --format; use terminal session\nobserve/control for NDJSON.",
    );
const HELP_TERMINAL_ATTACH_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal attach <terminal> [--takeover] [--timeout ms] [--mouse on|off] [--format text|json]",
        "Windows 交互终端附着，需要终端 stdin/stdout。使用既有终端 ID 或\n@terminal:suffix，不启动会话，也不聚焦 Workbench Panel。采用控制台尺寸，取得独占输入/尺寸控制权。--takeover 撤销旧控制者；失败也可能撤销旧权限。\nCtrl+B 后按 q 脱离；连续两次 Ctrl+B 发送字面 Ctrl+B。Ctrl+C 交付应用。粘贴使用语义操作；普通翻页键和滚轮使用历史或应用鼠标/光标模式。默认启用鼠标。--timeout 为 1..3600000 毫秒，默认不限总时长；单次交换最多 5 秒、清理最多 2 秒。不重连或重放输入。输出诊断前恢复终端模式。两种 --format 均显示交互画面；NDJSON 请使用 terminal session observe/control。",
    );
const HELP_AGENT_ATTACH_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent attach <target> [--takeover] [--timeout ms] [--mouse on|off] [--format text|json]",
        "Resolve a unique live Agent terminal, then use the Windows terminal attach\ninterface. Target may be a name, session or Panel; --name, --agent-session and\n--panel are explicit alternatives to the positional target. No focus or launch.\nCtrl+B then q detaches; Ctrl+C reaches the application. Console stdin/stdout\nare required. Default duration unlimited and mouse on. See help terminal attach\nfor takeover, timeout, cleanup and input semantics.",
    );
const HELP_AGENT_ATTACH_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent attach <target> [--takeover] [--timeout ms] [--mouse on|off] [--format text|json]",
        "解析唯一的活动 Agent 终端，再进入 Windows terminal attach 界面。目标可为名称、会话或 Panel；--name、--agent-session 和 --panel 可显式替代位置参数。不聚焦或启动资源。Ctrl+B 后按 q 脱离，Ctrl+C 交付应用；必须使用控制台\nstdin/stdout。默认不限总时长、启用鼠标。接管、超时、清理和输入语义见\nhelp terminal attach。",
    );

const HELP_WORKSPACE_MOVE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace move <workspace> --insert-index <n> [--format text|json]",
        "Reorder one workspace globally on Windows. Project membership, running terminals and every client's selection remain unchanged.\n\n--insert-index selects a boundary in the original order, from 0 through the workspace count (maximum 1023). Selecting the source boundary or its next boundary leaves the order unchanged.\n\nTargets resolve before the change; invalid input has no effect. No-op keeps the revision. Use workspace list to inspect order. An unknown outcome is never retried automatically.",
    );
const HELP_WORKSPACE_MOVE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace move <workspace> --insert-index <n> [--format text|json]",
        "在 Windows 上全局重排一个工作区，保留项目归属、运行中的终端及各客户端的选择。\n\n--insert-index 选择原顺序的插入边界，从 0 到当前工作区数量，最多 1023。选择来源边界或其下一边界时，顺序保持不变。\n\n变更前先解析目标；无效输入不产生变更，无变化时保留修订。使用 workspace list 查看顺序；结果未知时不自动重试。",
    );

#[cfg(windows)]
const HELP_PANEL_MOVE_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel move <panel> --into-tab <tab> --direction above|below|left|right [--target-panel <panel>] [--ratio <n>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\norchbee panel move <panel> --new-tab [--workspace <workspace>|--standalone] [--label <text>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\norchbee panel move <panel> --new-workspace [--project <project>] [--label <text>] [--tab-label <text>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\norchbee panel move <panel> --target-panel <panel> --direction above|below|left|right [--format text|json]",
    "Move a panel while keeping its terminal and process running. An existing destination tab can belong to any workspace. Its target defaults to the selected client's focused panel.\n\nFocus is on by default. With no client, use --no-focus and omit --ratio; the first target leaf and default ratio are used. Ratios are finite and clamped to 0.1..0.9. Timeout defaults to 30000 ms. Zoomed or same-tab relocation leaves the layout unchanged.\n\nA new workspace follows the terminal's working directory, falling back to the CLI working directory when unavailable. Moving the last panel removes the empty source tab and, across contexts, its empty workspace.\n\nDirectory, ratio and focus results are recorded separately from the move. Partial failure preserves committed changes; an uncertain submission is never replayed. The final usage form retains the legacy placement within the same tab.",
);
#[cfg(windows)]
const HELP_PANEL_MOVE_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel move <panel> --into-tab <tab> --direction above|below|left|right [--target-panel <panel>] [--ratio <n>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\norchbee panel move <panel> --new-tab [--workspace <workspace>|--standalone] [--label <text>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\norchbee panel move <panel> --new-workspace [--project <project>] [--label <text>] [--tab-label <text>] [--focus|--no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\norchbee panel move <panel> --target-panel <panel> --direction above|below|left|right [--format text|json]",
    "移动面板并保留其终端和进程。已有目标标签页可以属于任何工作区；目标面板默认取选中客户端的焦点面板。\n\n默认开启聚焦。没有客户端时使用 --no-focus 并省略 --ratio，采用首个目标叶子和默认比例。比例必须是有限值，并限制到 0.1..0.9。默认时限 30000 毫秒。已放大或迁入同一标签页时布局保持不变。\n\n新工作区采用终端工作目录，不可用时回退到 CLI 工作目录。移走最后面板会移除空来源标签页；跨上下文时也移除空来源工作区。\n\n目录、比例和聚焦结果与迁移分别记录。部分失败保留已提交变更，结果不确定时不重放。最后一种用法保留在同一标签页内放置的兼容形式。",
);

#[cfg(windows)]
impl Locale {
    pub(crate) const fn cli_query_client_labels(self) -> [&'static str; 4] {
        match self {
            Self::EnUs => [
                "Client",
                "Selected Tab",
                "Selected Panel",
                "Context / active Tab",
            ],
            Self::ZhCn => ["客户端", "所选标签页", "所选面板", "上下文 / 活动标签页"],
        }
    }

    pub(crate) const fn cli_workbench_agents_heading(self) -> &'static str {
        match self {
            Self::EnUs => "Agent summary: resource / status / sessions",
            Self::ZhCn => "Agent 汇总：资源 / 状态 / 会话数",
        }
    }
    pub(crate) const fn cli_agent_terminal_heading(self) -> &'static str {
        match self {
            Self::EnUs => "Terminal title / directory",
            Self::ZhCn => "终端标题／目录",
        }
    }

    pub(crate) const fn cli_terminal_observations_heading(self) -> &'static str {
        match self {
            Self::EnUs => "Terminal observations: Panel / title / directory",
            Self::ZhCn => "终端观察：面板 / 标题 / 目录",
        }
    }
    pub(crate) const fn cli_scroll_heading(self) -> &'static str {
        match self {
            Self::EnUs => "Client scroll: Panel / state / offset / maximum / rows",
            Self::ZhCn => "客户端滚动：面板 / 状态 / 偏移 / 最大偏移 / 行数",
        }
    }

    pub(crate) const fn cli_workspace_worktrees_heading(self) -> &'static str {
        match self {
            Self::EnUs => "Workspace worktrees",
            Self::ZhCn => "Workspace 工作树",
        }
    }
    pub(crate) const fn cli_workspace_worktree_states(self) -> [&'static str; 4] {
        match self {
            Self::EnUs => [
                "no directory",
                "unavailable",
                "main checkout",
                "linked checkout",
            ],
            Self::ZhCn => ["没有目录", "不可用", "主检出", "链接检出"],
        }
    }
    pub(crate) const fn cli_scroll_states(self) -> [&'static str; 7] {
        match self {
            Self::EnUs => [
                "not attached",
                "frame unavailable",
                "entering history",
                "awaiting window",
                "recovering",
                "live",
                "history",
            ],
            Self::ZhCn => [
                "未连接",
                "帧不可用",
                "进入历史中",
                "等待窗口",
                "恢复中",
                "实时",
                "历史",
            ],
        }
    }

    pub(crate) fn cli_workbench_agent_status(self, status: Option<&str>) -> &'static str {
        match (self, status) {
            (Self::EnUs, Some("blocked")) => "needs attention",
            (Self::EnUs, Some("done")) => "completed",
            (Self::EnUs, Some("error")) => "error",
            (Self::EnUs, Some("working")) => "working",
            (Self::EnUs, Some("idle")) => "idle",
            (Self::EnUs, None) => "state unavailable",
            (Self::EnUs, _) => "unknown",
            (Self::ZhCn, Some("blocked")) => "需要处理",
            (Self::ZhCn, Some("done")) => "已完成",
            (Self::ZhCn, Some("error")) => "错误",
            (Self::ZhCn, Some("working")) => "工作中",
            (Self::ZhCn, Some("idle")) => "空闲",
            (Self::ZhCn, None) => "状态不可用",
            (Self::ZhCn, _) => "未知",
        }
    }

    fn cli_workbench_query_failure(self, reason: Option<&str>) -> &'static str {
        match (self, reason) {
            (Self::EnUs, Some("agent-capacity")) => {
                "The Agent list exceeds the bounded Workbench query capacity. Query Agent pages with orchbee agent list.\n"
            }
            (Self::ZhCn, Some("agent-capacity")) => {
                "Agent 列表超过工作台查询容量。请使用 orchbee agent list 分页查询。\n"
            }
            (Self::EnUs, Some("agent-page-invalid")) => {
                "The Agent list returned inconsistent page data. No Workbench summary was published.\n"
            }
            (Self::ZhCn, Some("agent-page-invalid")) => {
                "Agent 列表返回了不一致的分页数据，未输出工作台汇总。\n"
            }
            (Self::EnUs, Some("agent-revision-changed")) => {
                "Agent state changed between query pages. Run the query again.\n"
            }
            (Self::ZhCn, Some("agent-revision-changed")) => {
                "Agent 状态在分页查询期间发生变化，请重新查询。\n"
            }
            (Self::EnUs, Some("metadata-changed")) => {
                "Workspace metadata changed or expired during the query. Run the query again.\n"
            }
            (Self::ZhCn, Some("metadata-changed")) => {
                "Workspace 元数据在查询期间发生变化或过期，请重新查询。\n"
            }
            (Self::EnUs, Some("workbench-changed")) => {
                "Workbench membership changed during the query. Run the query again.\n"
            }
            (Self::ZhCn, Some("workbench-changed")) => {
                "工作台结构在查询期间发生变化，请重新查询。\n"
            }
            (Self::EnUs, _) => {
                "The selected Client view changed during the query. Run the query again.\n"
            }
            (Self::ZhCn, _) => "所选客户端视图在查询期间发生变化，请重新查询。\n",
        }
    }
}

#[cfg(windows)]
const HELP_TERMINAL_READ_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal read <terminal-id> [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--plain|--ansi] [--format text|json]",
        "Read recent output by default: up to 80 physical rows from history and the live\nscreen. --source visible selects only the live screen; detection selects one\nscreen height near recent output. --lines permits zero and clamps to 1000.\n--unwrap without an explicit source selects recent-unwrapped (joins soft wraps).\n--plain exports UTF-8 text and conflicts with JSON; --ansi exports styled text\nor an ANSI JSON member. Detection stays plain. This is a snapshot, not PTY replay.\n\nJSON uses orchbee.terminal.output v1 (text) or v2 (ANSI), including the selected\nsource and observed range. Explicit --history --rows <1..1000> retains the\nlegacy history-only document and may use --unwrap/--lines. Do not combine\nhistory with --source, --plain or --ansi; explicit source/plain/ANSI excludes\n--unwrap. Use --source visible to request the previous default screen scope.\nSamples remain revision-fenced, read-only and bounded by a five-second deadline.",
    );
#[cfg(windows)]
const HELP_TERMINAL_READ_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal read <terminal-id> [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--plain|--ansi] [--format text|json]",
        "默认读取最近输出：历史与实时屏幕中最多 80 个物理行。--source visible 仅选择实时屏幕；detection 选择最近输出附近一个屏幕高度。--lines 允许零并钳制到 1000。未显式指定来源时，--unwrap 选择 recent-unwrapped（拼接软换行）。\n--plain 导出 UTF-8 文本，与 JSON 冲突；--ansi 导出样式文本或 ANSI JSON 字段。\ndetection 保持纯文本。这是快照，不是 PTY 重放。\n\nJSON 使用 orchbee.terminal.output v1（文本）或 v2（ANSI），含来源和观测范围。显式 --history --rows <1..1000> 保留旧的纯历史文档，可组合 --unwrap/--lines。\nhistory 不能与 --source、--plain、--ansi 混用；显式来源/plain/ANSI 排除 --unwrap。用 --source visible 请求原默认屏幕范围。读取仍固定修订、无输入副作用，总期限五秒。",
    );
const HELP_AGENT_OUTPUT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent read <target> [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--plain|--ansi] [--format text|json]",
        "Read one live Agent's terminal, defaulting to recent output (80 physical rows).\nTargets: Session ID/name/Panel ID, --agent-session <id>, --name <alias>, or --panel <id>.\nA Panel selects its primary terminal; otherwise require one navigable binding.\n--lines permits zero and clamps to 1000. Bare --unwrap selects recent-unwrapped.\n--source visible selects the live screen; detection reads screen-height output,\nnot Agent state. --plain exports UTF-8 and excludes JSON. --ansi exports style.\nJSON uses orchbee.terminal.output v1 (text) or v2 (ANSI).\nExplicit --history --rows preserves the history-only document; it excludes\nsource/plain/ANSI. See terminal read help for bounds, conflicts and cleanup.",
    );
const HELP_AGENT_OUTPUT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent read <target> [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--plain|--ansi] [--format text|json]",
        "读取一个存活 Agent 的终端，默认最近输出（80 个物理行）。目标可用 Session ID、名称、\nPanel ID、--agent-session <id>、--name <alias> 或 --panel <id>。Panel 选择主终端；其他选择要求唯一可导航绑定。--lines 允许零并钳制到 1000。裸 --unwrap 选择 recent-unwrapped。\n--source visible 选择实时屏幕；detection 读取一个屏幕高度的输出，不是 Agent 状态。\n--plain 导出 UTF-8，与 JSON 互斥；--ansi 导出样式。\nJSON 使用 orchbee.terminal.output v1（文本）或 v2（ANSI）。显式 --history --rows 保留纯历史文档，与 source/plain/ANSI 互斥。边界、冲突和清理规则见 terminal read 帮助。",
    );
#[cfg(windows)]
const HELP_TERMINAL_WAIT_OUTPUT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal wait-output <terminal-id> (--match <text>|--regex <pattern>) [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--timeout <ms>|--forever] [--format text|json]",
        "Wait for output, defaulting to recent (80 physical rows) without a total deadline.\n--timeout sets 1..3600000 milliseconds; --forever explicitly selects no total\nceiling and conflicts with --timeout. Every I/O step remains bounded to five\nseconds, and Ctrl+C stops the wait and releases its read-only viewer.\n--lines permits zero (never matches) and clamps to 1000. --rows <1..1000> is a\nrecent-line alias, excluding --source/--lines. Recent sources join soft wraps;\nvisible/detection match physical lines. Existing output can match immediately.\n--match is literal, case sensitive, 1..4096 UTF-8 bytes. --regex is at most 1024\nbytes and compiles before any connection. No input, resize or focus change.\nExit 0 on match, 7 on timeout, 130 on cancellation. JSON uses\norchbee.terminal.wait-output v3 with nullable timeoutMs. To retain a finite\n30-second wait over 1000 recent rows, use --lines 1000 --timeout 30000.",
    );
#[cfg(windows)]
const HELP_TERMINAL_WAIT_OUTPUT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal wait-output <terminal-id> (--match <text>|--regex <pattern>) [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--timeout <ms>|--forever] [--format text|json]",
        "等待输出，默认 recent（80 个物理行），不设总期限。\n--timeout 指定 1..3600000 毫秒；--forever 显式选择无总期限，与 --timeout 互斥。每次 I/O 仍限制五秒，Ctrl+C 停止等待并释放只读 viewer。\n--lines 允许零（从不匹配）并钳制到 1000。--rows <1..1000> 是 recent 行数别名，与 --source/--lines 互斥。recent 来源拼接软换行，visible/detection 匹配物理行。既有输出可立即匹配。--match 是区分大小写的字面值，1..4096 UTF-8 字节；\n--regex 至多 1024 字节，在建立任何连接前编译。不输入、不 resize、不改变焦点。匹配退出 0，超时退出 7，取消退出 130。JSON 使用 orchbee.terminal.wait-output v3，\ntimeoutMs 可为 null。需要 30 秒期限和 1000 个最近行时，用 --lines 1000 --timeout 30000。",
    );

impl super::Locale {
    pub(crate) const fn cli_current_presentation_label(self) -> &'static str {
        match self {
            Self::EnUs => "Current presentation",
            Self::ZhCn => "当前终端成员",
        }
    }
}

const HELP_CONFIG_RESET_KEYS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee config reset-keys [--format text|json]",
        "Back up APPDATA/Orchbee/config.toml (or ORCHBEE_CONFIG_FILE) and remove\ncustom keys. Preserve other settings and comments outside the removed tables.\nMissing file or no keys succeeds without writing. Invalid TOML or an unsafe\nedit fails unchanged. Windows requires a private parent and an exclusively\nopened regular file. Keep the reported backup until you no longer need to\nrestore the configuration. A failed restore reports recoveryRequired=true.\nDoes not connect, launch, or reload a Client. Exit 0 on success/no change,\n1 on failure, 2 on usage error.",
    );
const HELP_CONFIG_RESET_KEYS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee config reset-keys [--format text|json]",
        "备份 APPDATA/Orchbee/config.toml（或 ORCHBEE_CONFIG_FILE 指定文件），并移除自定义键位。保留其他设置及被移除表之外的注释。文件不存在或无 keys\n时成功且不写入；TOML 无效或无法安全编辑时保持不变并失败。Windows 要求私有父目录和可独占打开的普通文件。请保留回执中的备份，直到不再需要恢复配置；恢复失败时报告 recoveryRequired=true。不连接、启动或重载 Client。成功或无变更退出 0，失败退出 1，用法错误退出 2。",
    );

#[cfg(windows)]
const HELP_TERMINAL_SELECTION_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal selection-read <terminal> --anchor ROW:COL --cursor ROW:COL [--content-revision N] [--generation N] [--format text|json]",
        "Windows: read one inclusive screen-buffer selection, independent of Client viewport.\nUse a complete terminal ID or @terminal:<suffix>. Coordinates are zero-based;\nrow 0..4294967295, column 0..65535, at most 4096 selected rows and an 8 MiB\nrow window. Reversed endpoints are ordered. Wide/combining glyphs remain whole;\nverified soft wraps join, hard-line padding is trimmed.\nContent revision is optional full-u64; generation is optional positive full-u64.\nBoth pin expected content before reading. Changed/evicted content fails whole.\nOne cancellable five-second existing-session read; no input, clipboard or user\nselection changes. JSON: orchbee.terminal.selection v1; text escapes controls.",
    );
#[cfg(windows)]
const HELP_TERMINAL_SELECTION_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal selection-read <terminal> --anchor ROW:COL --cursor ROW:COL [--content-revision N] [--generation N] [--format text|json]",
        "Windows：读取包含两个端点的屏幕缓冲区选区，与 Client 视口无关。使用完整终端 ID 或 @terminal:<suffix>。坐标以零开始；行 0..4294967295、列 0..65535，最多选 4096 行及一个 8 MiB 行窗口。反向端点先排序；保留完整宽字／组合字形，连接已验证软换行并移除硬行尾填充。可选内容修订为完整 u64，generation 为正 u64；先检查预期内容，变化或已回收内容整体失败。一次可取消的五秒既有会话读取；不发送输入、不改用户选区／剪贴板。JSON 为 orchbee.terminal.selection v1，文本转义控制字符。",
    );

#[cfg(windows)]
const HELP_CONFIG_IMPORT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee config import-herdr --input-file SOURCE --output-file DEST [--dry-run] [--format text|json]",
        "Convert supported explicit Herdr settings into a new Orchbee file (Windows).\nBoth file operands are required. Relative paths use the caller directory;\n~ uses the current home. Directories inside the source use its parent.\nRead one regular UTF-8 TOML source, at most 64 KiB. Skip unsupported fields\nand report invalid fields without displaying their values. Existing targets\nare never overwritten. Creation requires an existing private local parent.\n--dry-run opens no destination; JSON preview alone includes candidateToml.\nA failed write retains the newly created file and reports recoveryRequired.\nInspect it before recovery; no deletion or retry is performed. A crash can\nleave a new file without a receipt. Select the new file with ORCHBEE_CONFIG_FILE\nfor a new Client; select the old file to roll back. No session discovery, launch,\nor reload; --session and --machine are rejected. Exit 0 on preview/creation,\n1 on failure, 2 on usage error.",
    );

#[cfg(windows)]
const HELP_CONFIG_IMPORT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee config import-herdr --input-file SOURCE --output-file DEST [--dry-run] [--format text|json]",
        "将 herdr 支持的显式设置转换为 Orchbee 新文件（Windows）。两个文件参数必填。相对路径以调用目录为基准；~ 使用当前用户主目录。来源中的目录以来源父目录为基准。仅读取一份普通 UTF-8 TOML 来源，最多 64 KiB。跳过不支持的字段；报告无效字段，不显示其值。绝不覆盖已有目标。创建要求已有的私有本地父目录。\n--dry-run 不打开目标；只有 JSON 预览包含 candidateToml。写入失败会保留本次新建的文件并报告 recoveryRequired。恢复前检查该文件；不删除或重试。崩溃可能留下没有回执的新文件。用 ORCHBEE_CONFIG_FILE 为新 Client 选择新文件；选择旧文件即可回退。不查找、启动或重载会话；拒绝 --session 和 --machine。预览或创建成功退出 0，失败退出 1，用法错误退出 2。",
    );

const HELP_WORKSPACE_MOVE_BLOCK_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace move-block <workspace> ... [--before <workspace>] [--format text|json]",
        "Move 1..1023 distinct workspaces together in caller-supplied order on Windows. Omit --before to append; otherwise select an existing workspace outside the block. Project membership, running terminals and client selections remain unchanged.\n\nAll targets resolve before one atomic change. Invalid input has no effect; a no-op keeps the revision. Use workspace list to inspect order. An unknown outcome is never retried automatically.",
    );

const HELP_WORKSPACE_MOVE_BLOCK_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace move-block <workspace> ... [--before <workspace>] [--format text|json]",
        "在 Windows 上按调用者给出的顺序整体移动 1..1023 个不同工作区。省略 --before 时追加到末尾，否则须选择块外已有工作区。保留项目归属、运行中的终端和客户端选择。\n\n全部目标在一次原子变更前解析。无效输入不产生变更；无变化时保留修订。使用 workspace list 查看顺序；结果未知时不自动重试。",
    );
