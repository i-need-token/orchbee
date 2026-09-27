//! Localized strings for the read-only CLI query surface. Machine envelope
//! fields stay English; these strings are the human stderr/text-render layer.

mod client_control;
mod client_layout;
#[cfg(windows)]
mod layouts;
#[cfg(windows)]
mod named_sessions;
#[cfg(windows)]
mod panel_relocation;
mod runtime_status;
#[cfg(windows)]
mod schema_export;
use super::Locale;
use crate::cli::Failure;
use crate::invocation::HelpTopic;
#[cfg(windows)]
mod workspace_creation;
mod workspace_group_close;
mod worktree;

impl Locale {
    /// Localized human diagnostic for one CLI failure. User-controlled data is
    /// escaped before it reaches the terminal.
    pub(crate) fn cli_failure(self, failure: &Failure) -> String {
        use crate::cli::FailureKind;
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
        // `site start` failures carry the local-runtime source; the shared
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
                Self::EnUs => {
                    "The local Site did not become ready within the time limit; a Server may still be starting, or activation state may have been left behind. No recovery was attempted.\n"
                        .to_owned()
                }
                Self::ZhCn => {
                    "本地 Site 未在时限内就绪：Server 可能仍在启动，也可能遗留了激活状态。未尝试任何恢复。\n"
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
                    Self::EnUs => {
                        format!("The local Site refused the start ({reason}); its activation state requires recovery and no repair was attempted.\n")
                    }
                    Self::ZhCn => {
                        format!("本地 Site 拒绝了启动（{reason}）：其激活状态需要恢复，未尝试任何修复。\n")
                    }
                }
            }
            FailureKind::Denied if local_runtime => match self {
                Self::EnUs => {
                    "The local Site refused the start; the native development cohort is not permitted.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地 Site 拒绝了启动：原生开发 cohort 未获许可。\n".to_owned(),
            },
            FailureKind::ServiceUnavailable if local_runtime => {
                self.cli_site_unavailable(failure)
            }
            FailureKind::SessionCapacity if local_runtime => match self {
                Self::EnUs => {
                    "The local Site has no Client Session capacity for the start probe.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地 Site 没有可接纳启动探针的 Client Session 容量。\n".to_owned(),
            },
            FailureKind::TransportLoss if local_runtime => match self {
                Self::EnUs => {
                    "The connection to the local Site was lost during the start admission.\n"
                        .to_owned()
                }
                Self::ZhCn => "与本地 Site 的连接在启动准入期间中断。\n".to_owned(),
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
                        "The local Site denied the command; nothing was applied.\n".to_owned()
                    }
                    (Self::EnUs, false) => {
                        "The local Site denied the read request.\n".to_owned()
                    }
                    (Self::ZhCn, true) => {
                        "本地 Site 拒绝了该命令；未应用任何修改。\n".to_owned()
                    }
                    (Self::ZhCn, false) => "本地 Site 拒绝了读取请求。\n".to_owned(),
                }
            }
            FailureKind::Rejected => {
                let reason = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("reason"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                if reason == "workspace-group-close-required" {
                    return match self {
                        Self::EnUs => "This main Workspace has associated worktree Workspaces. Use 'workspace close <target> --group --confirm' to close the captured group; Git checkouts are preserved.\n".to_owned(),
                        Self::ZhCn => "此主 Workspace 有关联的 worktree Workspace。使用 'workspace close <target> --group --confirm' 关闭捕获的工作区组；Git checkout 会保留。\n".to_owned(),
                    };
                }
                match self {
                    Self::EnUs => {
                        format!("The local Site rejected the command ({reason}); nothing was applied.\n")
                    }
                    Self::ZhCn => {
                        format!("本地 Site 拒绝了该命令（{reason}）；未应用任何修改。\n")
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
                    "The local Site has no Client Session capacity for this query.\n".to_owned()
                }
                Self::ZhCn => "本地 Site 没有可接纳本次查询的 Client Session 容量。\n".to_owned(),
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
                        "The connection to the local Site failed before the command could be submitted; nothing was sent.\n".to_owned()
                    }
                    (Self::ZhCn, "command") => {
                        "与本地 Site 的连接在命令提交前失败；未发送任何内容。\n".to_owned()
                    }
                    (Self::EnUs, "viewer") => {
                        "The connection to the local Site was lost during the terminal read.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, "viewer") => "与本地 Site 的连接在终端读取期间中断。\n".to_owned(),
                    (Self::EnUs, _) => {
                        "The connection to the local Site was lost before a snapshot arrived.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, _) => "与本地 Site 的连接在快照返回前中断。\n".to_owned(),
                }
            }
            FailureKind::ProtocolInvalid => match self {
                Self::EnUs => {
                    "The local Site answered with an invalid protocol response.\n".to_owned()
                }
                Self::ZhCn => "本地 Site 返回了无效的协议响应。\n".to_owned(),
            },
            FailureKind::Incompatible => match self {
                Self::EnUs => {
                    "The running local Site is incompatible with this development cohort.\n"
                        .to_owned()
                }
                Self::ZhCn => "正在运行的本地 Site 与当前开发 cohort 不兼容。\n".to_owned(),
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
                    "The local Site rejected the input ({}); nothing was sent.{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
                Self::ZhCn => format!(
                    "本地 Site 拒绝了该输入（{}）；未发送任何内容。{cleanup_note}\n",
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
                Self::EnUs => "The local Site denied the worktree listing.\n".to_owned(),
                Self::ZhCn => "本地 Site 拒绝了该 worktree 列举。\n".to_owned(),
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
                        "The local Site could not run the worktree listing right now.\n".to_owned()
                    }
                    (Self::ZhCn, _) => "本地 Site 当前无法执行 worktree 列举。\n".to_owned(),
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
                    "The connection to the local Site was lost during the worktree listing.\n"
                        .to_owned()
                }
                Self::ZhCn => "与本地 Site 的连接在 worktree 列举期间中断。\n".to_owned(),
            },
            FailureKind::ProtocolInvalid => match self {
                Self::EnUs => {
                    "The local Site answered the worktree listing with an invalid protocol response.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地 Site 对 worktree 列举返回了无效的协议响应。\n".to_owned(),
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
                Self::EnUs => "The local Site denied the launch profile request.\n".to_owned(),
                Self::ZhCn => "本地 Site 拒绝了该启动配置请求。\n".to_owned(),
            },
            FailureKind::ServiceUnavailable => match self {
                Self::EnUs => {
                    "The local Site could not answer the launch profile request right now.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地 Site 当前无法应答该启动配置请求。\n".to_owned(),
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
                    "The connection to the local Site was lost during the launch profile request.\n"
                        .to_owned()
                }
                Self::ZhCn => "与本地 Site 的连接在启动配置请求期间中断。\n".to_owned(),
            },
            FailureKind::ProtocolInvalid => {
                let reason = detail("reason");
                match (self, reason) {
                    (Self::EnUs, "correlation-mismatch") => {
                        "The local Site answered a different launch profile request.\n".to_owned()
                    }
                    (Self::ZhCn, "correlation-mismatch") => {
                        "本地 Site 应答了另一个启动配置请求。\n".to_owned()
                    }
                    (Self::EnUs, _) => {
                        "The local Site answered the launch profile request with an invalid protocol response.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, _) => {
                        "本地 Site 对启动配置请求返回了无效的协议响应。\n".to_owned()
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
            (Self::EnUs, ExpectedValue::OutputFilePath) => {
                "a literal file path of 1–4096 UTF-8 bytes"
            }
            (Self::ZhCn, ExpectedValue::OutputFilePath) => "1–4096 UTF-8 字节的字面文件路径",
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
                "an absolute path of at most 4,096 UTF-8 bytes; Windows also accepts caller-relative paths and ~"
            }
            (Self::ZhCn, ExpectedValue::WorktreePath) => {
                "不超过 4,096 UTF-8 字节的绝对路径；Windows 也接受相对调用者目录的路径及 ~"
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
            (Self::EnUs, ExpectedValue::ProjectId) => {
                "a nonzero 32-character lowercase hexadecimal project ID"
            }
            (Self::ZhCn, ExpectedValue::ProjectId) => "非零的 32 位小写十六进制 project ID",
            (Self::EnUs, ExpectedValue::WorkspaceId) => {
                "a nonzero 32-character lowercase hexadecimal workspace ID"
            }
            (Self::ZhCn, ExpectedValue::WorkspaceId) => "非零的 32 位小写十六进制 workspace ID",
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
                "a key name or 'ctrl+alt+shift+key' combination; repeatable, at most 64"
            }
            (Self::ZhCn, ExpectedValue::SendKeySequence) => {
                "键名或 'ctrl+alt+shift+键' 组合；可重复，至多 64 个"
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
            UsageReason::UnknownOption { option } => (
                match self {
                    Self::EnUs => format!("Unknown option {}.", echo(option)),
                    Self::ZhCn => format!("未知选项 {}。", echo(option)),
                },
                hint(UsageContext::Root),
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
        match self {
            Self::EnUs => format!("{problem} Run '{hint}'.\n"),
            Self::ZhCn => format!("{problem}请运行 ‘{hint}’。\n"),
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
        if failure.failure_schema == Some("orchbee.cli.failure.client-control")
            && failure
                .details
                .as_ref()
                .and_then(|details| details.get("reason"))
                .and_then(|reason| reason.as_str())
                == Some("client-not-ready")
        {
            return match self {
                Self::EnUs => {
                    "The selected TUI Client is not ready; retry after it becomes ready.\n"
                }
                Self::ZhCn => "所选 TUI 客户端尚未就绪；请在其就绪后重试。\n",
            }
            .to_owned();
        }
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
            (Self::EnUs, "discovery") => {
                "No running local Site was found for this user.\n".to_owned()
            }
            (Self::ZhCn, "discovery") => "未找到当前用户正在运行的本地 Site。\n".to_owned(),
            (Self::EnUs, "snapshot") => {
                "The local Site could not produce a snapshot right now.\n".to_owned()
            }
            (Self::ZhCn, "snapshot") => "本地 Site 当前无法产出快照。\n".to_owned(),
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
                "The local Site could not execute the command right now; nothing was applied.\n"
                    .to_owned()
            }
            (Self::ZhCn, "command") => {
                "本地 Site 当前无法执行该命令；未应用任何修改。\n".to_owned()
            }
            (Self::EnUs, _) => "The local Site could not admit this query connection.\n".to_owned(),
            (Self::ZhCn, _) => "本地 Site 无法接纳本次查询连接。\n".to_owned(),
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
            // The cohort check retains its observed io::ErrorKind (and the OS
            // error number when one exists) as private diagnostic data: the
            // message reports exactly what was observed — never a guess that
            // the artifacts are missing or corrupted.
            (Self::EnUs, "cohort") => match &failure.io_diagnostic {
                Some(observed) => format!(
                    "The native development cohort could not be validated (I/O kind: {:?}{}).\n",
                    observed.kind,
                    observed
                        .raw_os_error
                        .map(|code| format!(", OS error: {code}"))
                        .unwrap_or_default(),
                ),
                None => "The native development cohort could not be validated.\n".to_owned(),
            },
            (Self::ZhCn, "cohort") => match &failure.io_diagnostic {
                Some(observed) => format!(
                    "无法验证原生开发 cohort（I/O 类型：{:?}{}）。\n",
                    observed.kind,
                    observed
                        .raw_os_error
                        .map(|code| format!("，OS 错误码：{code}"))
                        .unwrap_or_default(),
                ),
                None => "无法验证原生开发 cohort。\n".to_owned(),
            },
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
            (Self::EnUs, "socket") => "A local Site socket failed its safety checks.\n".to_owned(),
            (Self::ZhCn, "socket") => "本地 Site 套接字未通过安全检查。\n".to_owned(),
            (Self::EnUs, "runtime-directory") => {
                "The local Site runtime directory failed its safety checks.\n".to_owned()
            }
            (Self::ZhCn, "runtime-directory") => {
                "本地 Site 运行时目录未通过安全检查。\n".to_owned()
            }
            (Self::EnUs, _) => "The runtime base is missing, invalid, or unsafe.\n".to_owned(),
            (Self::ZhCn, _) => "运行时基座缺失、无效或不安全。\n".to_owned(),
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

    /// `site start` service-unavailable diagnostics, routed by the failure's
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
                "This Site operation is unavailable on this platform.\n".to_owned()
            }
            (Self::ZhCn, "platform") => "当前平台尚不支持此 Site 操作。\n".to_owned(),
            (Self::EnUs, "runtime-directory") => self.cli_socket_path_too_long().to_owned(),
            (Self::ZhCn, "runtime-directory") => self.cli_socket_path_too_long().to_owned(),
            (Self::EnUs, "discovery") => {
                "No running local Site answered, and its runtime directory could not be created.\n"
                    .to_owned()
            }
            (Self::ZhCn, "discovery") => {
                "没有运行中的本地 Site 应答，且其运行时目录无法创建。\n".to_owned()
            }
            (Self::EnUs, "cohort") => {
                "The native development cohort artifacts are missing.\n".to_owned()
            }
            (Self::ZhCn, "cohort") => "原生开发 cohort 工件缺失。\n".to_owned(),
            (Self::EnUs, _) => {
                "The local Site could not admit the start probe connection.\n".to_owned()
            }
            (Self::ZhCn, _) => "本地 Site 无法接纳启动探针连接。\n".to_owned(),
        }
    }

    /// `site start` text answer when this invocation performed the
    /// activation and the readiness probe was admitted.
    pub(crate) const fn cli_site_started(self) -> &'static str {
        match self {
            Self::EnUs => "Local Site started.\n",
            Self::ZhCn => "本地 Site 已启动。\n",
        }
    }

    /// `site start` text answer when a running Site admitted the probe.
    pub(crate) const fn cli_site_already_running(self) -> &'static str {
        match self {
            Self::EnUs => "Local Site is already running.\n",
            Self::ZhCn => "本地 Site 已在运行。\n",
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
                "A derived local Site socket path is too long for this platform's socket address limit. Set ORCHBEE_RUNTIME_BASE to a shorter absolute directory.\n"
            }
            Self::ZhCn => {
                "派生出的本地 Site 套接字路径超出本平台的套接字地址长度限制。请将 ORCHBEE_RUNTIME_BASE 设为更短的绝对目录。\n"
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

    pub(crate) fn cli_agent_list_client_headers(self) -> [&'static str; 2] {
        match self {
            Self::EnUs => ["CLIENT STATUS", "SEEN"],
            Self::ZhCn => ["CLIENT 状态", "已看见"],
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
                "the observation endpoint socket is absent; hooks deliver only while the managed Site is running."
            }
            (Self::ZhCn, "endpoint-absent") => {
                "观察端点套接字不存在；钩子仅在托管 Site 运行时投递。"
            }
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
                Self::EnUs => "The local Site denied the agent read.\n".to_owned(),
                Self::ZhCn => "本地 Site 拒绝了本次 Agent 读取。\n".to_owned(),
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
                        format!("The local Site rejected the agent list cursor ({reason}).\n")
                    }
                    Self::ZhCn => {
                        format!("本地 Site 拒绝了该 Agent 列表游标（{reason}）。\n")
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
                    "The connection to the local Site was lost during the agent read.\n".to_owned()
                }
                Self::ZhCn => "与本地 Site 的连接在 Agent 读取期间中断。\n".to_owned(),
            },
            FailureKind::Cancelled => match self {
                Self::EnUs => "The agent read was cancelled.\n".to_owned(),
                Self::ZhCn => "Agent 读取已被取消。\n".to_owned(),
            },
            FailureKind::ProtocolInvalid => match self {
                Self::EnUs => {
                    "The local Site answered the agent read with an invalid protocol response.\n"
                        .to_owned()
                }
                Self::ZhCn => "本地 Site 对本次 Agent 读取返回了无效的协议响应。\n".to_owned(),
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
    /// Root `--help`/`help` body: the terminal modes, every query family, the
    /// workspace/tab/panel mutations a running Site accepts, and where each
    /// topic's own help lives.
    pub(crate) const fn cli_root_help(self) -> &'static str {
        match self {
            Self::EnUs => ROOT_HELP_EN,
            Self::ZhCn => ROOT_HELP_ZH,
        }
    }

    pub(crate) fn cli_help(self, topic: HelpTopic) -> &'static str {
        use crate::invocation::Family;
        match (self, topic) {
            (Self::EnUs, HelpTopic::Family(Family::Client)) => client_control::HELP_CLIENT_EN,
            (Self::ZhCn, HelpTopic::Family(Family::Client)) => client_control::HELP_CLIENT_ZH,
            (_, HelpTopic::Family(Family::Project)) => match self {
                Self::EnUs => HELP_PROJECT_EN,
                Self::ZhCn => HELP_PROJECT_ZH,
            },
            (_, HelpTopic::Family(Family::Workspace)) => match self {
                Self::EnUs => HELP_WORKSPACE_EN,
                Self::ZhCn => HELP_WORKSPACE_ZH,
            },
            (_, HelpTopic::Family(Family::Tab)) => match self {
                Self::EnUs => HELP_TAB_EN,
                Self::ZhCn => HELP_TAB_ZH,
            },
            #[cfg(windows)]
            (_, HelpTopic::Family(Family::Layout)) => match self {
                Self::EnUs => layouts::FAMILY_EN,
                Self::ZhCn => layouts::FAMILY_ZH,
            },
            #[cfg(not(windows))]
            (_, HelpTopic::Family(Family::Layout)) => match self {
                Self::EnUs => client_control::HELP_LAYOUT_EN,
                Self::ZhCn => client_control::HELP_LAYOUT_ZH,
            },
            (_, HelpTopic::Family(Family::Panel)) => match self {
                Self::EnUs => HELP_PANEL_EN,
                Self::ZhCn => HELP_PANEL_ZH,
            },
            (_, HelpTopic::Family(Family::TerminalSession)) => match self {
                Self::EnUs => {
                    "orchbee terminal session <observe|control> <terminal>\n\nobserve: read-only NDJSON frames.\ncontrol: NDJSON input/resize/scroll/release, with optional --takeover.\nUse '<command> --help' for details.\n"
                }
                Self::ZhCn => {
                    "orchbee terminal session <observe|control> <terminal>\n\nobserve：只读 NDJSON 帧。\ncontrol：NDJSON 输入/尺寸/滚动/释放，可选 --takeover 接管。\n使用 '<command> --help' 查看详情。\n"
                }
            },
            (_, HelpTopic::Family(Family::Terminal)) => match self {
                Self::EnUs => HELP_TERMINAL_EN,
                Self::ZhCn => HELP_TERMINAL_ZH,
            },
            (_, HelpTopic::Family(Family::Worktree)) => match self {
                Self::EnUs => HELP_WORKTREE_EN,
                Self::ZhCn => HELP_WORKTREE_ZH,
            },
            (_, HelpTopic::Family(Family::LaunchProfile)) => match self {
                Self::EnUs => HELP_LAUNCH_PROFILE_EN,
                Self::ZhCn => HELP_LAUNCH_PROFILE_ZH,
            },
            #[cfg(windows)]
            (_, HelpTopic::Family(Family::Session)) => self.cli_session_family_help(),
            (_, HelpTopic::Family(Family::Site)) => match self {
                Self::EnUs => HELP_SITE_EN,
                Self::ZhCn => HELP_SITE_ZH,
            },
            (_, HelpTopic::Family(Family::Agent)) => match self {
                Self::EnUs => HELP_AGENT_EN,
                Self::ZhCn => HELP_AGENT_ZH,
            },
            (_, HelpTopic::Family(Family::AgentIntegration)) => match self {
                Self::EnUs => HELP_AGENT_INTEGRATION_EN,
                Self::ZhCn => HELP_AGENT_INTEGRATION_ZH,
            },
            (_, HelpTopic::Family(Family::Api)) => match self {
                Self::EnUs => HELP_API_EN,
                Self::ZhCn => HELP_API_ZH,
            },
            (_, HelpTopic::Family(Family::Config)) => match self {
                Self::EnUs => HELP_CONFIG_EN,
                Self::ZhCn => HELP_CONFIG_ZH,
            },
            (_, HelpTopic::Family(Family::Completion)) => match self {
                Self::EnUs => HELP_COMPLETION_EN,
                Self::ZhCn => HELP_COMPLETION_ZH,
            },
            (_, HelpTopic::Command(path)) => self.cli_command_help(path),
        }
    }

    fn cli_command_help(self, path: crate::invocation::CommandPath) -> &'static str {
        use crate::invocation::CommandPath;
        match (self, path) {
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
            (Self::EnUs, CommandPath::WorkspaceMove | CommandPath::WorkspaceMoveBlock) => {
                HELP_WORKSPACE_MOVE_EN
            }
            (Self::ZhCn, CommandPath::WorkspaceMove | CommandPath::WorkspaceMoveBlock) => {
                HELP_WORKSPACE_MOVE_ZH
            }
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
            (Self::EnUs, CommandPath::AgentChanges) => {
                "orchbee agent changes [--cursor <c>] [--limit <n>] [--format text|json]\n\nWindows: read one bounded Agent change page (limit 1-100, default 50).\nOmit cursor to establish a current-head checkpoint, then reuse its cursor.\nreset-required means history was lost; rebuild your baseline. A previous\nSite boot or invalid cursor is rejected. No subscription or automatic retry.\nTotal read deadline: 5 seconds. JSON schema: orchbee.agent.changes.\n"
            }
            (Self::ZhCn, CommandPath::AgentChanges) => {
                "orchbee agent changes [--cursor <c>] [--limit <n>] [--format text|json]\n\nWindows：读取一页有界 Agent 变更（limit 1-100，默认 50）。\n省略 cursor 建立当前头部检查点，再使用其 cursor 继续观察。\nreset-required 表示历史丢失，必须重建基线；先前 Site 启动实例或无效\ncursor 会被拒绝。不订阅、不自动重试。总读取期限 5 秒。\nJSON schema：orchbee.agent.changes。\n"
            }
            (Self::EnUs, CommandPath::AgentList) => HELP_AGENT_LIST_EN,
            (Self::ZhCn, CommandPath::AgentList) => HELP_AGENT_LIST_ZH,
            (Self::EnUs, CommandPath::AgentStart) => {
                "Usage: orchbee agent start <name> --kind <provider> [--panel <id|@panel:suffix>|--current] [--timeout <ms>] [--format text|json] [-- <args>...]\n\nWindows: start an interactive Agent in an idle terminal, then bind its name after verified readiness.\nThe default target is the caller Panel. Timeout: 3001–300000 ms, default 30000.\nUse --arg=<value> repeatedly as an alternative to trailing arguments. Input is submitted once.\nCancellation or timeout releases the name when confirmed; it never kills the Agent.\nProvider hooks must report readiness. Use agent integration and agent doctor to check setup.\n"
            }
            (Self::ZhCn, CommandPath::AgentStart) => {
                "用法：orchbee agent start <name> --kind <provider> [--panel <id|@panel:suffix>|--current] [--timeout <ms>] [--format text|json] [-- <args>...]\n\nWindows：在空闲终端启动交互式 Agent，确认就绪后绑定名称。\n默认目标为调用者 Panel。超时范围 3001–300000 毫秒，默认 30000。\n可重复使用 --arg=<value> 代替尾随参数。启动输入只提交一次。\n取消或超时在确认后释放名称，但不会终止 Agent。\n提供方 hook 必须能报告就绪；可用 agent integration 和 agent doctor 检查配置。\n"
            }
            (Self::EnUs, CommandPath::AgentPrompt) => {
                "orchbee agent prompt <target> <text> [--wait] [--until <condition>]... [--timeout <ms> | --forever] [--format text|json]\n\nResolve one live Agent, refuse pending attention, and submit literal paste plus Enter.\nTargets: Session ID/name/Panel ID, --session <id>, --name <alias>, or --panel <id|@panel:suffix>. --until/--timeout/--forever require --wait.\nWait first observes activity, then a matching condition. Windows defaults to\nidle/done/blocked with no total timeout; Linux retains 600000 ms.\n--forever removes the total timeout and conflicts with --timeout.\nWindows Codex adds a Right key between paste and Enter.\nA wait failure retains the accepted input receipt. Never replay uncertain input.\nA matching state does not identify a prompt-specific turn.\n"
            }
            (Self::ZhCn, CommandPath::AgentPrompt) => {
                "orchbee agent prompt <target> <text> [--wait] [--until <condition>]... [--timeout <ms> | --forever] [--format text|json]\n\n定位存活 Agent，有待处理请求时拒绝，再提交字面 paste 加 Enter。\n目标：Session ID/名称/Panel ID、--session <id>、--name <alias>\n或 --panel <id|@panel:suffix>。Panel 选择主终端。--until/--timeout/--forever 要求 --wait。\n先观察活动再等待条件匹配；Windows 默认 idle/done/blocked 且无总超时，Linux 保持 600000 毫秒。--forever 取消总期限，与 --timeout 互斥。\nWindows Codex 在 paste 和 Enter 之间增加 Right 键。等待失败保留已接受输入回执。\n不重放不确定输入；状态匹配不标识本次提示自己的轮次。\n"
            }
            (Self::EnUs, CommandPath::AgentRead) => {
                "orchbee agent read <target> [--history --rows <n>] [--unwrap] [--lines <n>] [--format text|json]\n\nRead the terminal bound to one live Agent. Target: Session ID/name/Panel ID, --session <id>,\n--name <alias>, or --panel <id|@panel:suffix>. Panel selects its primary terminal;\notherwise exactly one navigable binding is required. Uses terminal\nread/history envelopes; history excludes visible rows.\n\nExplicit sources: --source visible|recent|recent-unwrapped|detection.\nRecent includes history and live screen (default 80 physical rows); detection\nuses one screen height near the cursor, not Agent state. Source-mode --lines\naccepts 0..4294967295 and clamps to 1000. Visible is this viewer's live screen.\n--plain prints UTF-8 text directly and defaults to recent; it conflicts with\n--format json. Source/plain cannot mix with --history, --rows or --unwrap.\nJSON uses orchbee.terminal.output; a stale sample fails without a hidden retry.\n\n--ansi exports styled text directly (default recent); --format json instead\nuses orchbee.terminal.output v2 with encoding: ansi. It conflicts with\n--plain, --history, --rows and --unwrap. Styled blanks remain content;\nrecent-unwrapped joins soft wraps. Detection stays plain text. This is a\nstyled snapshot with SGR, not original PTY bytes or graphics replay.\n"
            }
            (Self::ZhCn, CommandPath::AgentRead) => {
                "orchbee agent read <target> [--history --rows <n>] [--unwrap] [--lines <n>] [--format text|json]\n\n读取一个存活 Agent 绑定的终端。目标：Session ID/名称/Panel ID、--session <id>、--name <alias>\n或 --panel <id|@panel:suffix>。Panel 选择主终端。\n未指定 Panel 时必须有且仅有一个可导航绑定。复用终端 read/history 封套；历史不含可见行。\n\n显式来源：--source visible|recent|recent-unwrapped|detection。\nrecent 合并历史与实时屏幕，默认 80 个物理行；detection 读取光标附近\n一个屏幕高度，不是 Agent 状态。来源模式 --lines 接受 0..4294967295，\n钳制到 1000。visible 是当前查看器的实时屏幕。\n--plain 直接输出 UTF-8 文本，默认 recent；与 --format json 冲突。\n来源/plain 不能与 --history、--rows 或 --unwrap 混用。\nJSON 使用 orchbee.terminal.output；样本过期时失败，不隐藏重试。\n\n--ansi 直接导出带样式文本（默认 recent）；配合 --format json 时改用\norchbee.terminal.output v2，encoding 为 ansi。它与 --plain、--history、\n--rows 和 --unwrap 冲突。保留带样式空格；recent-unwrapped 拼接软换行。\ndetection 仍输出纯文本。这是包含 SGR 的样式快照，不是原始 PTY 字节或图形重放。\n"
            }
            (Self::EnUs, CommandPath::AgentSendKeys) => {
                "orchbee agent send-keys <target> <key>... [--format text|json]\n\nSend 1..64 semantic keys or combinations (enter, ctrl-c, alt+left, ctrl+shift+a)\nto one live Agent terminal. Target: Session ID/name/Panel ID, --session <id>, --name <alias>,\nor --panel <id|@panel:suffix>. Panel selects its primary terminal.\nUses orchbee.terminal.send receipts. Never replays uncertain input.\n"
            }
            (Self::ZhCn, CommandPath::AgentSendKeys) => {
                "orchbee agent send-keys <target> <key>... [--format text|json]\n\n向一个存活 Agent 终端发送 1..64 个语义按键或组合（enter、ctrl-c、alt+left、ctrl+shift+a）。\n目标：Session ID/名称/Panel ID、--session <id>、--name <alias>\n或 --panel <id|@panel:suffix>。Panel 选择主终端。使用 orchbee.terminal.send 回执。\n不重放不确定输入。\n"
            }
            (Self::EnUs, CommandPath::AgentAttention) => client_control::HELP_AGENT_ATTENTION_EN,
            (Self::ZhCn, CommandPath::AgentAttention) => client_control::HELP_AGENT_ATTENTION_ZH,
            (Self::EnUs, CommandPath::AgentFocus) => client_control::HELP_AGENT_FOCUS_EN,
            (Self::ZhCn, CommandPath::AgentFocus) => client_control::HELP_AGENT_FOCUS_ZH,
            (Self::EnUs, CommandPath::AgentGet) => HELP_AGENT_GET_EN,
            (Self::ZhCn, CommandPath::AgentGet) => HELP_AGENT_GET_ZH,
            (Self::EnUs, CommandPath::AgentExplain) => HELP_AGENT_EXPLAIN_EN,
            (Self::ZhCn, CommandPath::AgentExplain) => HELP_AGENT_EXPLAIN_ZH,
            (Self::EnUs, CommandPath::AgentNames) => {
                "Usage: orchbee agent names [--format text|json]\n\nList this Site runtime's live Agent names. Names disappear when their session ends or the Site stops.\n"
            }
            (Self::ZhCn, CommandPath::AgentNames) => {
                "用法：orchbee agent names [--format text|json]\n\n列出当前 Site 运行时中的 Agent 名称。会话结束或 Site 停止后名称会释放。\n"
            }
            (Self::EnUs, CommandPath::AgentResolve) => {
                "Usage: orchbee agent resolve <target> [--format text|json]\n       orchbee agent resolve --session <id> | --name <alias> | --panel <id|@panel:suffix>\n\nResolve an exact Session ID first, then a runtime name, then a Panel ID.\nA Panel selects its primary terminal and requires one live Agent. Use --name to select an alias that collides with an ID.\n"
            }
            (Self::ZhCn, CommandPath::AgentResolve) => {
                "用法：orchbee agent resolve <target> [--format text|json]\n      orchbee agent resolve --session <id> | --name <alias> | --panel <id|@panel:suffix>\n\n优先按完整 Session ID 定位，再查运行时名称，最后回退到 Panel ID。\nPanel 选择主终端，要求唯一存活 Agent。名称与 ID 冲突时，用 --name 明确选择名称。\n"
            }
            (Self::EnUs, CommandPath::AgentRename) => {
                "Usage: orchbee agent rename <target> <new-name> [--format text|json]\n       orchbee agent rename <target> --clear\n\nAssign or atomically rename one live Agent. Name: [a-z][a-z0-9_-]{0,31}. Duplicate names are rejected. --clear releases only the name. Replace <target> with --session <id>, --name <alias>, or --panel <id|@panel:suffix> to select explicitly. Names reset when the session ends or the Site stops.\n"
            }
            (Self::ZhCn, CommandPath::AgentRename) => {
                "用法：orchbee agent rename <target> <new-name> [--format text|json]\n      orchbee agent rename <target> --clear\n\n为存活 Agent 命名或原子重命名。名称格式为 [a-z][a-z0-9_-]{0,31}，拒绝重名。--clear 只释放名称。可将 <target> 替换成 --session <id> 或 --name <alias>。会话结束或 Site 停止后名称释放。\n"
            }
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
            (Self::EnUs, CommandPath::SiteDiagnose) => HELP_SITE_DIAGNOSE_EN,
            (Self::ZhCn, CommandPath::SiteDiagnose) => HELP_SITE_DIAGNOSE_ZH,
            (Self::EnUs, CommandPath::SiteStop) => HELP_SITE_STOP_EN,
            (Self::ZhCn, CommandPath::SiteStop) => HELP_SITE_STOP_ZH,
            (Self::EnUs, CommandPath::SiteStopStatus) => HELP_SITE_STOP_STATUS_EN,
            (Self::ZhCn, CommandPath::SiteStopStatus) => HELP_SITE_STOP_STATUS_ZH,
        }
    }

    /// A localized note line for one `site diagnose` advisory token; an
    /// unrecognized token renders itself verbatim — the token is the
    /// machine field, the line is only its human gloss.
    pub(crate) fn cli_site_diagnose_advisory(self, advisory: &str) -> &'static str {
        match (self, advisory) {
            (Self::EnUs, "activation-recorded") => {
                "note: activation recorded; if no existing Site can be joined, fresh activation is blocked by current recovery policy.\n"
            }
            (Self::ZhCn, "activation-recorded") => {
                "注：已记录一次激活；若无可加入的现有 Site，按当前恢复策略新的激活将被阻止。\n"
            }
            (Self::EnUs, "socket-residue") => {
                "note: client/worker socket residue observed; if no existing Site can be joined, fresh activation is blocked by current recovery policy.\n"
            }
            (Self::ZhCn, "socket-residue") => {
                "注：观察到 client/worker 套接字残留；若无可加入的现有 Site，按当前恢复策略新的激活将被阻止。\n"
            }
            (Self::EnUs, "pipe-name-present") => {
                "note: a client/worker pipe name was observed without connecting or authenticating its owner; if no Site can be joined, current recovery policy blocks fresh activation.\n"
            }
            (Self::ZhCn, "pipe-name-present") => {
                "注：观察到 client/worker 管道名称，未连接或认证其属主；若无可加入的 Site，当前恢复策略将阻止重新激活。\n"
            }
            (Self::EnUs, _) => "note: unrecognized advisory.\n",
            (Self::ZhCn, _) => "注：未识别的通告。\n",
        }
    }
}

const ROOT_HELP_EN: &str = "Orchbee\n\nWindows: --site <name> selects a local Site (default: default).\nNames: 1-64 ASCII letters, digits, dot, underscore or hyphen; not . or .. .\nExplicit --site overrides inherited routing and ORCHBEE_SITE.\n\nUsage:\n  orchbee\n  orchbee tui [--mouse on|off]\n  orchbee status [--format text|json]\n  orchbee <family> <command> [operands] [--format text|json]\n  orchbee api snapshot|schema\n  orchbee site start [--format text|json]\n  orchbee site diagnose [--format text|json]\n  orchbee site stop --confirm [--detach] [--format text|json]\n  orchbee site stop-status <request-id> [--format text|json]\n  orchbee config check [--format text|json]\n  orchbee help [<family>|<command>]\n  orchbee completion bash|zsh|fish|powershell|elvish\n  orchbee -h|--help\n  orchbee --version\n  orchbee --skill\n\nModes and queries:\n  orchbee              Open the local terminal (Windows/Linux development cohort)\n  orchbee tui          Open the same local terminal explicitly\n  orchbee status       Read the running local Site's Workbench snapshot\n  orchbee project      list | get\n  orchbee workspace    list | get | close-status\n  orchbee tab          list | get\n  orchbee panel        list | get | current | process-info\n                       layout | neighbor | edges\n  orchbee client       list | get | control-status\n  orchbee terminal     list | get | read | wait-output | send\n  orchbee worktree     list | status\n  orchbee launch-profile list | get\n  orchbee agent        list | get | explain | doctor | wait | names | resolve\n                       read | send-keys | prompt | rename | start\n  orchbee agent integration\n                       list | explain | enable | disable — local provider\n                       onboarding; never contacts a Site\n  orchbee api          Machine snapshot and embedded schema documents\n\nSite lifecycle:\n  orchbee session      list | attach | stop | delete (Windows)\n  orchbee site start   Ensure the local Site is running (no TTY needed)\n  orchbee site diagnose\n                       Inspect the local Site runtime — read-only facts\n  orchbee site stop --confirm\n                       Stop the managed Windows Site and its terminals\n  orchbee site stop-status <id>\n                       Inspect a stop attempt or completed receipt\n\nMutations (one command per invocation, against the running Site):\n  orchbee project      create | rename\n  orchbee workspace    create | rename | accept-directory\n                       close | close-resume | focus\n  orchbee tab          create | rename | move | close | focus\n  orchbee panel        split | rename | swap | move | close | focus | zoom | resize\n  orchbee layout       set-split-ratio (Windows TUI)\n  orchbee terminal     send\n  orchbee launch-profile create | delete\n  orchbee worktree     create | open | remove | resume (Windows)\n\nInspection and help:\n  orchbee config check Inspect runtime inputs and Windows user configuration\n  orchbee help         Show help for a family or complete command path\n  orchbee completion   Emit a static shell completion script\n  orchbee -h|--help    Show this help; after a family or command path, show\n                       that path's help\n  orchbee --version    Show the version\n  orchbee --skill      Print the embedded Agent guide (offline)\n\nRun 'orchbee help <family>' for filters and target rules, or\n'orchbee help <family> <command>' for one mutation's full syntax.\n";
const ROOT_HELP_ZH: &str = "Orchbee\n\nWindows：--site <name> 选择本地 Site（默认 default）。\n名称：1-64 个 ASCII 字母、数字、点、下划线或连字符；不能是 . 或 ..。\n显式 --site 覆盖继承的路由与 ORCHBEE_SITE。\n\n用法：\n  orchbee\n  orchbee tui [--mouse on|off]\n  orchbee status [--format text|json]\n  orchbee <family> <command> [操作数] [--format text|json]\n  orchbee api snapshot|schema\n  orchbee site start [--format text|json]\n  orchbee site diagnose [--format text|json]\n  orchbee site stop --confirm [--detach] [--format text|json]\n  orchbee site stop-status <request-id> [--format text|json]\n  orchbee config check [--format text|json]\n  orchbee help [<family>|<command>]\n  orchbee completion bash|zsh|fish|powershell|elvish\n  orchbee -h|--help\n  orchbee --version\n  orchbee --skill\n\n模式与查询：\n  orchbee              打开本地终端（Windows/Linux 开发 cohort）\n  orchbee tui          显式打开同一本地终端\n  orchbee status       读取正在运行的本地 Site 的 Workbench 快照\n  orchbee project      list | get\n  orchbee workspace    list | get | close-status\n  orchbee tab          list | get\n  orchbee panel        list | get | current | process-info\n                       layout | neighbor | edges\n  orchbee client       list | get | control-status\n  orchbee terminal     list | get | read | wait-output | send\n  orchbee worktree     list | status\n  orchbee launch-profile list | get\n  orchbee agent        list | get | explain | doctor | wait | names | resolve\n                       read | send-keys | prompt | rename | start\n  orchbee agent integration\n                       list | explain | enable | disable——本地 provider\n                       接入；绝不接触 Site\n  orchbee api          机器快照与内嵌 schema 文档\n\nSite 生命周期：\n  orchbee session      list | attach | stop | delete (Windows)\n  orchbee site start   确保本地 Site 正在运行（无需 TTY）\n  orchbee site diagnose\n                       检查本地 Site 运行时——只读事实\n  orchbee site stop --confirm\n                       停止托管 Windows Site 及其终端\n  orchbee site stop-status <id>\n                       查询停止尝试或完成回执\n\n修改命令（每次调用执行一条命令，作用于运行中的 Site）：\n  orchbee project      create | rename\n  orchbee workspace    create | rename | accept-directory\n                       close | close-resume | focus\n  orchbee tab          create | rename | move | close | focus\n  orchbee panel        split | rename | swap | move | close | focus | zoom | resize\n  orchbee layout       set-split-ratio (Windows TUI)\n  orchbee terminal     send\n  orchbee launch-profile create | delete\n  orchbee worktree     create | open | remove | resume (Windows)\n\n检查与帮助：\n  orchbee config check 检查运行时输入及 Windows 用户配置\n  orchbee help         显示某个 family 或完整命令路径的帮助\n  orchbee completion   输出静态 shell 补全脚本\n  orchbee -h|--help    显示此帮助；跟在 family 或命令路径后时显示该路径的帮助\n  orchbee --version    显示版本\n  orchbee --skill      离线输出内嵌 Agent 指引\n\n运行 'orchbee help <family>' 查看过滤参数与 target 规则，或\n'orchbee help <family> <command>' 查看单条修改命令的完整语法。\n";

const HELP_TUI_EN: &str = "orchbee tui [--mouse on|off]\n\nOpen the local terminal interface (Windows/Linux development cohort).\nA bare 'orchbee' invocation does the same.\n\n  --mouse on|off   Host pointer capture for pane hit-testing; on by\n                   default. Only the explicit 'tui' route accepts it.\n";
const HELP_TUI_ZH: &str = "orchbee tui [--mouse on|off]\n\n打开本地终端界面（Windows/Linux 开发 cohort）。\n裸 'orchbee' 调用效果相同。\n\n  --mouse on|off   宿主机指针捕获，用于面板命中；默认开启。\n                   只有显式 'tui' 路由接受该选项。\n";

const HELP_STATUS_EN: &str = "orchbee status\n\nRead a live Workbench snapshot from the running local Site and report\nreachability plus entity counts. The Site is never created, activated, or\nmutated. '--format json' emits the orchbee.status envelope.\n\nWindows runtime metadata (no Site activation):\n  status server  Query the running Server, protocol and cohort compatibility\n  status client  Inspect this executable and its local cohort offline\n";
const HELP_STATUS_ZH: &str = "orchbee status\n\n从正在运行的本地 Site 读取实时 Workbench 快照并报告可达性与实体计数。\n绝不创建、激活或修改 Site。'--format json' 输出 orchbee.status 封套。\n\nWindows 运行时元数据（不激活 Site）：\n  status server  查询运行中 Server、协议和 cohort 兼容性\n  status client  离线检查当前程序及本地 cohort\n";

const HELP_PROJECT_EN: &str = "orchbee project list|get|create|rename\n\nQuery and mutate Project resources in the running local Site.\n  project list                       List every Project\n  project get <target>               Show one Project by exact ID, @project:<suffix>, or exact label\n  project create --label <l>         Create one Project\n  project rename <target> --label <l>  Rename one Project\nTargets are case-sensitive; ambiguous labels or suffixes are rejected, never guessed.\nEvery mutation reads one snapshot, then submits exactly one command.\n";
const HELP_PROJECT_ZH: &str = "orchbee project list|get|create|rename\n\n查询并修改本地 Site 中的 Project 资源。\n  project list                       列出所有 Project\n  project get <target>               按精确 ID、@project:<suffix> 或精确 label 显示一个 Project\n  project create --label <l>         创建一个 Project\n  project rename <target> --label <l>  重命名一个 Project\ntarget 区分大小写；歧义 label 或后缀会被拒绝而非猜测。\n每个修改命令先读取一次快照，然后只提交一个命令。\n";

const HELP_WORKSPACE_EN: &str = "orchbee workspace list|get|create|rename|move|move-block|accept-directory|close|close-status|close-resume|focus\n\nQuery and mutate Workspace resources in the running local Site.\n  workspace list [--project <t>]                    List Workspaces, optionally inside one Project\n  workspace get <target>                            Show one Workspace by exact ID, @workspace:<suffix>, or exact label\n  workspace create [--project <t>] [--label <l>]    Create a Workspace (Windows launch defaults)\n  workspace rename <target> --label <l>             Rename one Workspace\n  workspace accept-directory <target> --path <p>    Accept a literal absolute path as the Workspace's primary directory\n  workspace close <target> --confirm                Close every terminal the Workspace's Clients use\n  workspace close-status --project <id> --workspace <id>\n                                                    Read the retained close operation by stable IDs\n  workspace close-resume --project <id> --workspace <id> --request <close-id> --generation <n> --confirm\n                                                    Open one fresh attempt for a resumable close\nEvery mutation reads one snapshot, then submits exactly one command.\nWindows worktree groups: add --group to close; use close-status/close-resume --group --request <operation-id> to observe or resume the retained group.\n\n  workspace focus <target>\n    Navigate one Windows TUI; see command help for --client and receipts.\n";
const HELP_WORKSPACE_ZH: &str = "orchbee workspace list|get|create|rename|move|move-block|accept-directory|close|close-status|close-resume|focus\n\n查询并修改本地 Site 中的 Workspace 资源。\n  workspace list [--project <t>]                    列出 Workspace，可按某个 Project 过滤\n  workspace get <target>                            按精确 ID、@workspace:<suffix> 或精确 label 显示一个 Workspace\n  workspace create [--project <t>] [--label <l>]    创建工作区（Windows 支持默认启动）\n  workspace rename <target> --label <l>             重命名一个 Workspace\n  workspace accept-directory <target> --path <p>    将一个字面绝对路径接受为 Workspace 的主目录\n  workspace close <target> --confirm                关闭该 Workspace 的 Clients 使用的全部终端\n  workspace close-status --project <id> --workspace <id>\n                                                    按稳定 ID 读取保留的关闭操作\n  workspace close-resume --project <id> --workspace <id> --request <close-id> --generation <n> --confirm\n                                                    为可恢复的关闭开启一次新的尝试\n每个修改命令先读取一次快照，然后只提交一个命令。\nWindows worktree 组：close 添加 --group；使用 close-status/close-resume --group --request <operation-id> 观测或恢复保留组。\n\n  workspace focus <target>\n    导航一个 Windows TUI；--client 及回执规则见命令帮助。\n";

const HELP_TAB_EN: &str = "orchbee tab list|get|create|rename|move|close|focus\n\nQuery and mutate Tab resources in the running local Site.\n  tab list [--workspace <t> | --standalone]  List Tabs, optionally filtered\n  tab get <target>                           Show one Tab by exact ID, @tab:<suffix>, or label\n  tab create [--workspace <t>|--standalone] [--label <l>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...]\n                                             Create one Tab in one context\n  tab rename <target> --label <l>            Rename one Tab\n  tab move <target> --direction <d>          Move one Tab within its strip\n  tab close <target> --confirm               Close one Tab\n'--workspace' and '--standalone' are mutually exclusive single-use selectors.\n\n  tab focus <target>\n    Navigate one Windows TUI; see command help for --client and receipts.\n";
const HELP_TAB_ZH: &str = "orchbee tab list|get|create|rename|move|close|focus\n\n查询并修改本地 Site 中的 Tab 资源。\n  tab list [--workspace <t> | --standalone]  列出 Tab，可按 Workspace 或独立上下文过滤\n  tab get <target>                           按精确 ID、@tab:<suffix> 或 label 显示一个 Tab\n  tab create [--workspace <t>|--standalone] [--label <l>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...]\n                                             在某个上下文内创建一个 Tab\n  tab rename <target> --label <l>            重命名一个 Tab\n  tab move <target> --direction <d>          在标签条内移动一个 Tab\n  tab close <target> --confirm               关闭一个 Tab\n'--workspace' 与 '--standalone' 互斥且各只能用一次。\n\n  tab focus <target>\n    导航一个 Windows TUI；--client 及回执规则见命令帮助。\n";

const HELP_PANEL_EN: &str = "orchbee panel list|get|current|process-info|layout|neighbor|edges|resize|rename|input|swap|split|close|move|focus|zoom\n\nQuery and mutate Panel resources in the running local Site.\n  panel layout|neighbor|edges [<panel>]   Read Windows TUI geometry without changing focus\n                                           neighbor requires --direction left|right|up|down\n  panel list [--tab <t>]                   List Panels, optionally inside one Tab\n  panel current [--panel <panel-id> | --current]\n                                           Locate the calling Panel from its current binding\n  panel process-info [<panel-id>]          Inspect primary Terminal process information\n  panel get <panel-id>                     Show one Panel by its complete typed ID or @panel:<suffix>\n  panel split [<panel-id>] --direction <d> [--label <l>] [--right-click orchbee|panel] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...]\n                                           Split one Panel, creating a sibling\n  panel close <panel-id> --confirm         Close one Panel\n  panel move <panel-id> --target-panel <panel-id> --direction <d>\n                                           Move one Panel next to a sibling\n  panel rename <panel-id> (--label <text> | --clear)\n                                           Set or clear the shared custom title\n  panel input (<panel-id>|--panel <panel-id>|--current) --right-click orchbee|panel\n                                           Set the shared right-button owner\nComplete 32-hex IDs and @panel:<suffix> selectors are accepted.\n\n  panel swap [<source>] (--with <target> | --direction left|right|up|down)\n  panel resize [<target>] --direction left|right|up|down [--amount <number>]\n  layout set-split-ratio --path <root|0.1> --ratio <number>\n  panel focus [<target>] [--direction left|right|up|down] | zoom [<target>]\n    Navigate one Windows TUI; see command help for --client and receipts.\n";
const HELP_PANEL_ZH: &str = "orchbee panel list|get|current|process-info|layout|neighbor|edges|resize|rename|input|swap|split|close|move|focus|zoom\n\n查询并修改本地 Site 中的 Panel 资源。\n  panel layout|neighbor|edges [<panel>]   读取 Windows TUI 几何，不改变焦点\n                                           neighbor 要求 --direction left|right|up|down\n  panel list [--tab <t>]                   列出 Panel，可按某个 Tab 过滤\n  panel current [--panel <panel-id> | --current]\n                                           从当前绑定定位调用者 Panel\n  panel process-info [<panel-id>]          查询主 Terminal 的进程信息\n  panel get <panel-id>                     按完整类型化 ID 或 @panel:<suffix> 显示一个 Panel\n  panel split [<panel-id>] --direction <d> [--label <l>] [--right-click orchbee|panel] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...]\n                                           分裂一个 Panel，创建一个相邻 Panel\n  panel close <panel-id> --confirm         关闭一个 Panel\n  panel move <panel-id> --target-panel <panel-id> --direction <d>\n                                           将一个 Panel 移动到某个相邻 Panel 旁\n  panel rename <panel-id> (--label <text> | --clear)\n                                           设置或清除共享自定义标题\n  panel input (<panel-id>|--panel <panel-id>|--current) --right-click orchbee|panel\n                                           设置共享右键归属\n接受完整 32 位小写十六进制 ID 与 @panel:<suffix> 选择器。\n\n  panel swap [<source>] (--with <target> | --direction left|right|up|down)\n  panel resize [<target>] --direction left|right|up|down [--amount <number>]\n  layout set-split-ratio --path <root|0.1> --ratio <number>\n  panel focus [<target>] [--direction left|right|up|down] | zoom [<target>]\n    导航一个 Windows TUI；--client 及回执规则见命令帮助。\n";

const HELP_TERMINAL_OBSERVE_EN: &str = "Usage: orchbee terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]\n\nWindows: observe one existing terminal as NDJSON (orchbee.terminal.stream v2).\nTarget: complete terminal ID or @terminal:<suffix>. Default viewport: 120x40;\ndimensions: 1..4096, at most 262144 cells. Crops/pads without resizing the source.\nFrames contain full safe ANSI screens encoded as base64, not raw PTY output.\nThe stream does not read stdin or acquire control. Both formats emit NDJSON.\nNo total timeout by default; --timeout: 1..3600000 ms. Ctrl+C exits 130; timeout\nexits 7. Each transport/output stall is bounded to five seconds; cleanup may\ntake two seconds. A disconnected source is a failure, not a successful exit.\n";
const HELP_TERMINAL_OBSERVE_ZH: &str = "用法：orchbee terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]\n\nWindows：以 NDJSON 持续观察一个已有终端（orchbee.terminal.stream v2）。\n目标：完整终端 ID 或 @terminal:<suffix>。默认视口为 120x40；单边尺寸为\n1..4096，总计最多 262144 个单元格。仅裁剪或填充，不改变源终端大小。\n帧包含以 base64 编码的完整安全 ANSI 屏幕，不是原始 PTY 输出。\n不读取 stdin，也不获取控制权。两种格式均输出 NDJSON。\n默认没有总超时；--timeout 范围为 1..3600000 毫秒。Ctrl+C 退出码为 130，\n超时为 7。每次传输或输出停滞最多五秒；清理可能需要两秒。\n源连接断开视为失败，不会报告成功退出。\n";
const HELP_TERMINAL_EN: &str = "orchbee terminal list|get|read|wait-output|send|run|session|attach\n\nQuery terminals bound by Workbench Presentations in the running local Site,\nor send one input record to one of them.\n  terminal list                List every bound terminal\n  terminal get <terminal-id>   Show one terminal by ID or @terminal:<suffix>\n  terminal read <terminal-id> [--history --rows <n>] [--unwrap] [--lines <n>]\n                               Read the terminal's visible content; the paired\n                               flags read retained history rows instead\n  terminal wait-output <terminal-id> --match <text> | --regex <pattern>\n                               Wait for text in the terminal's retained output\n  terminal run <target> <command>  Paste a quoted command then press Enter\n  terminal send <terminal-id>  Send text or one key to the terminal's input\n  terminal attach <terminal>  Windows interactive control; Ctrl+B then q detaches\nTerminal targets accept a complete typed ID or an @terminal:<suffix> selector.\nA terminal invisible to any Presentation binding cannot be listed, read,\nwaited on, or sent to; a binding proves structure only, not that a process\nis alive.\n";
const HELP_TERMINAL_ZH: &str = "orchbee terminal list|get|read|wait-output|send|run|session|attach\n\n查询本地 Site 中被 Workbench Presentation 绑定的终端，或向其中一个终端发送一条输入。\n  terminal list                列出所有已绑定终端\n  terminal get <terminal-id>   按完整类型化 ID 或 @terminal:<suffix> 显示一个终端\n  terminal read <terminal-id> [--history --rows <n>] [--unwrap] [--lines <n>]\n                               读取终端当前可见内容；成对 flag 时改为读取\n                               保留历史行\n  terminal wait-output <terminal-id> --match <text> | --regex <pattern>\n                               等待终端保留输出中出现匹配内容\n  terminal run <target> <command>  粘贴一个引号内命令，再按 Enter\n  terminal send <terminal-id>  向终端输入发送文本或一个按键\n  terminal attach <terminal>  Windows 交互控制；Ctrl+B 后按 q 脱离\n未被任何 Presentation 引用的终端不可见、不可读、不可等待也不可发送；绑定只证明结构，不证明进程存活。\n";

const HELP_SITE_EN: &str = "orchbee site start|stop|stop-status|diagnose\n\nManage the implicit per-user local Site's lifecycle.\n  site start     Ensure the local Site is running, activating it without a\n                TTY when needed\n  site stop      Stop the managed Windows Site and its terminals (--confirm)\n  site stop-status <id>  Inspect a stop attempt or completed receipt\n  site diagnose  Inspect the local Site runtime's finite paths and locks —\n                read-only facts, never a repair or a process claim\n";
const HELP_SITE_ZH: &str = "orchbee site start|stop|stop-status|diagnose\n\n管理隐式的每用户本地 Site 生命周期。\n  site start     确保本地 Site 正在运行，必要时在无 TTY 环境下激活\n  site stop      停止托管 Windows Site 及其终端（必须 --confirm）\n  site stop-status <id>  查询停止尝试或完成回执\n  site diagnose  检查本地 Site 运行时的有限路径与锁——只读事实，\n                绝不修复，也不对进程下断言\n";

const HELP_SITE_START_EN: &str = "orchbee site start [--format text|json]\n\nEnsure the implicit per-user local Site is running. When no admitted\nSite answers, this invocation performs the existing activation sequence\n— the same locks, marker, Server spawn, and readiness admission the TUI\nstartup uses — and reports 'started'. On Windows, a clean-stop checkpoint\nrestores layouts and Launch Profiles and starts fresh default shells; corrupt\ndata refuses startup. Agent state, history and per-shell cwd are not restored.\nWhen a running Site admits the\nreadiness probe, it reports 'already-running'. One absolute 20-second\nbudget covers the whole invocation.\n\n'--format json' emits the orchbee.site.start envelope. An interrupted\nor timed-out start may leave activation state behind; a later invocation\nthen answers refused with a recovery-required reason. The command never\nrepairs, stops, or deletes runtime state.\n";
const HELP_SITE_START_ZH: &str = "orchbee site start [--format text|json]\n\n确保隐式的每用户本地 Site 正在运行。若没有已准入的 Site 应答，本次\n调用执行既有的激活序列——与 TUI 启动相同的锁、标记、Server 拉起\n与就绪准入——并报告 'started'。Windows 会恢复正常停止保存的布局和 Launch Profile，\n启动新的默认 shell；损坏数据拒绝启动，不恢复 Agent 状态、历史或各 shell 实际 cwd。\n若运行中的 Site 准入了就绪探针，则\n报告 'already-running'。整个调用共享一个 20 秒绝对预算。\n\n'--format json' 输出 orchbee.site.start 封套。被中断或超时的启动\n可能遗留激活状态；后续调用将以拒绝加 recovery-required 原因应答。\n本命令绝不修复、停止或删除运行时状态。\n";

const HELP_AGENT_EN: &str = "orchbee agent attach|attention|changes|list|get|focus|explain|doctor|wait|names|resolve|rename|read|send-keys|prompt|start|integration\n\nObserve Agent sessions tracked by the running local Site, or diagnose\nprovider hooks offline.\n  agent start <name> --kind <provider> [--panel <id>|--current]\n                                Start in an idle Windows terminal and bind after readiness\n  agent list [--include-ended] [--limit <n>] [--cursor <c>] [--client <id>]\n                                List Agent sessions (live only unless\n                                --include-ended; 1-100 per page, default 50)\n  agent attention <target> [--client <id>]\n  agent changes [--cursor <c>] [--limit <n>]\n                                Windows bounded change page or checkpoint\n  agent get <target>        Show one Agent session by ID or runtime name\n  agent explain <target>    Explain how one session's status and freshness\n                                were derived\n  agent doctor [--provider <id>]... [--asset-root <path>]...\n                                Explain, from bounded filesystem facts alone,\n                                why Agent observations may not arrive\n  agent wait <target> [--until <condition>]...\n                                Poll until a condition matches or the\n                                deadline lands\n  agent prompt <target> <text>  Submit a prompt; --wait observes activity and state\n  agent attach <target>         Windows interactive control; Ctrl+B then q detaches\n  agent read <target>           Read the bound terminal\n  agent send-keys <target> <key>... Send semantic keys\n  agent names                  List runtime-scoped session names\n  agent resolve <target>       Resolve a session ID or name\n  agent rename <target> <name>  Assign or rename a live session; --clear releases\n  agent integration <command>   Local provider onboarding: list | explain |\n                                enable | disable — 'orchbee help agent\n                                integration' has the full sub-family\nTargets accept Session IDs, names or Panel IDs; --session, --name and\n--panel select explicitly. A Panel selects its primary terminal. A cursor is the\nserver-minted 'v1.<payload>.<tag>' token from a previous page. The session\nreads and 'doctor' above create, activate, or mutate nothing; 'agent\nintegration' manages local provider files instead and never contacts a\nSite.\n";
const HELP_AGENT_ZH: &str = "orchbee agent attach|attention|changes|list|get|focus|explain|doctor|wait|names|resolve|rename|read|send-keys|prompt|start|integration\n\n观察运行中的本地 Site 跟踪的 Agent 会话，或离线诊断 provider 钩子。\n  agent start <name> --kind <provider> [--panel <id>|--current]\n                                在空闲 Windows 终端启动，就绪后绑定名称\n  agent list [--include-ended] [--limit <n>] [--cursor <c>] [--client <id>]\n                                列出 Agent 会话（默认仅存活会话；每页\n                                1-100 条，默认 50）\n  agent attention <target> [--client <id>]\n  agent changes [--cursor <c>] [--limit <n>]\n                                Windows 有界变更页或检查点\n  agent get <target>        按会话 ID 或运行时名称显示一个 Agent 会话\n  agent explain <target>    解释一个会话的状态与新鲜度推导依据\n  agent doctor [--provider <id>]... [--asset-root <path>]...\n                                仅凭有界文件系统事实解释 Agent 观察为何\n                                可能未到达\n  agent wait <target> [--until <条件>]...\n                                轮询直到条件命中或截止到达\n  agent prompt <target> <text>  提交提示；--wait 观察活动及状态\n  agent attach <target>         Windows 交互控制；Ctrl+B 后按 q 脱离\n  agent read <target>           读取绑定终端\n  agent send-keys <target> <key>... 发送语义按键\n  agent names                  列出运行时会话名称\n  agent resolve <target>       按会话 ID 或名称定位\n  agent rename <target> <name> 为存活会话命名；--clear 释放名称\n  agent integration <命令>      本地 provider 接入：list | explain |\n                                enable | disable——完整子命令族见\n                                'orchbee help agent integration'\n目标支持会话 ID、名称或 Panel ID；--session、--name、--panel 可显式选择。\nPanel 选择其主终端。\ncursor 是上一页返回的服务端铸造\n'v1.<payload>.<tag>' 令牌。上述会话读取与 'doctor' 不创建、不激活、\n不修改任何状态；'agent integration' 改为管理本地 provider 文件，且\n绝不接触 Site。\n";

const HELP_AGENT_LIST_EN: &str = "orchbee agent list [--include-ended] [--limit <n>] [--cursor <c>] [--client <id>] [--format text|json]\n\nList Agent sessions observed by the running local Site. '--include-ended'\nadds ended sessions; '--limit' selects 1-100 rows per page (default 50);\n'--cursor' continues from a previous page's server-minted nextCursor.\n'--format json' emits the orchbee.agent.list envelope.\nWindows --client joins that exact TUI's observed status without focusing it.\n";
const HELP_AGENT_LIST_ZH: &str = "orchbee agent list [--include-ended] [--limit <n>] [--cursor <c>] [--client <id>] [--format text|json]\n\n列出运行中的本地 Site 观察到的 Agent 会话。'--include-ended' 包含已结束\n会话；'--limit' 每页 1-100 条（默认 50）；'--cursor' 从上一页服务端铸造\n的 nextCursor 继续。'--format json' 输出 orchbee.agent.list 封套。\nWindows --client 加入指定 TUI 观察到的状态，不移动焦点。\n";

const HELP_AGENT_GET_EN: &str = "orchbee agent get <target> [--client <id>] [--format text|json]\n\nShow one Agent session by its opaque session identity: lifecycle, semantic\nstatus, observation freshness, location, capabilities, pending requests,\nand presentation attachments. '--format json' emits the\norchbee.agent.get envelope.\nTarget: one positional Session ID/name/Panel ID, --session <id>,\n--name <alias>, or --panel <id|@panel:suffix>. Exact retained Session IDs\nand names precede Panel fallback. Panels select their primary terminal;\nmultiple live Agents are refused. A wait stays pinned to its first Session.\n\nWindows --client adds that exact Client's focused/seen and idle/done projection\nwith version 4. Canonical session status stays unchanged. A missing or lagging\nClient observation has null seen; no read acknowledges or changes focus.\nThe same five-second total includes cursor authentication and version joining.\nWindows Get without --client uses version 3. Runtime includes the primary\nTerminal process sample and promptReady submission preflight; cwd/argv remain\nnullable. Binding changes discard the sample. This is not interactive UI\nreadiness or an input lease. Process reads share the original total deadline.\n";
const HELP_AGENT_GET_ZH: &str = "orchbee agent get <target> [--client <id>] [--format text|json]\n\n按会话 ID 或运行时名称显示一个 Agent 会话：生命周期、语义状态、观察新鲜度、\n位置、能力、待处理请求与 Presentation 附着。'--format json' 输出\norchbee.agent.get 封套。\n目标：一个位置参数 Session ID/名称/Panel ID、--session <id>、\n--name <alias> 或 --panel <id|@panel:suffix>。Session ID、名称优先于 Panel。\nPanel 选择主终端，多 Agent 匹配时拒绝；等待固定到首次解析的 Session。\n\nWindows --client 以版本 4 附加指定 Client 的聚焦、已看见及 idle/done 投影。\n规范 session 状态保持不变。Client 观测缺失或滞后时 seen 为空；\n读取不确认已看见、不改变焦点。游标认证与版本合并共用五秒总预算。\nWindows 不带 --client 的 Get 使用版本 3；runtime 包含主 Terminal 的进程\n样本及 promptReady 提交预检，cwd/argv 可为空。绑定变化时丢弃样本。\n这不表示交互 UI 就绪或输入租约；进程读取共用原始总期限。\n";

const HELP_AGENT_EXPLAIN_EN: &str = "orchbee agent explain <target> [--format text|json]\n\nExplain one Agent session's derived view: the canonical status basis,\nfreshness meaning, capability ceiling, observation source and fidelity,\nand the navigate-only action boundary. '--format json' emits the\norchbee.agent.explain envelope.\nTarget: one positional Session ID/name/Panel ID, --session <id>,\n--name <alias>, or --panel <id|@panel:suffix>. Exact retained Session IDs\nand names precede Panel fallback. Panels select their primary terminal;\nmultiple live Agents are refused. A wait stays pinned to its first Session.\n";
const HELP_AGENT_EXPLAIN_ZH: &str = "orchbee agent explain <target> [--format text|json]\n\n解释一个 Agent 会话的推导视图：规范状态依据、新鲜度含义、能力上限、\n观察来源与保真度，以及仅限导航的操作边界。'--format json' 输出\norchbee.agent.explain 封套。\n目标：一个位置参数 Session ID/名称/Panel ID、--session <id>、\n--name <alias> 或 --panel <id|@panel:suffix>。Session ID、名称优先于 Panel。\nPanel 选择主终端，多 Agent 匹配时拒绝；等待固定到首次解析的 Session。\n";

const HELP_AGENT_WAIT_EN: &str = "orchbee agent wait <target> [--until <condition>]... [--after-revision <n>] [--timeout <ms> | --forever] [--format text|json]\n\nPoll one Agent session on a held Client Session until a requested\ncondition matches, the revision fence refuses, the session answer is\nterminal, or the total deadline lands. '--until' is repeatable and takes\none of: blocked, done, needs-attention, failed, succeeded, canceled, interrupted,\nworking, idle, unknown, ended, fresh, stale, disconnected (default:\nneeds-attention failed succeeded canceled interrupted idle ended).\nStatus conditions require fresh freshness; the three freshness\nconditions match the observation directly. '--after-revision' is a\nglobal-revision fence, not a turn identity; '--timeout' bounds the whole\nwait at 1-3600000 ms (default 600000). --forever removes the total\ntimeout and conflicts with --timeout; every read remains bounded.\nblocked matches fresh needs-attention; done matches fresh completed\noutcomes (succeeded/failed/canceled/interrupted), not an unseen marker.\nPolling samples current state: a\ntransition that begins and ends between two completed observations is\nmissed. '--format json' emits the orchbee.agent.wait envelope.\nTarget: one positional Session ID/name/Panel ID, --session <id>,\n--name <alias>, or --panel <id|@panel:suffix>. Exact retained Session IDs\nand names precede Panel fallback. Panels select their primary terminal;\nmultiple live Agents are refused. A wait stays pinned to its first Session.\n";
const HELP_AGENT_WAIT_ZH: &str = "orchbee agent wait <target> [--until <条件>]... [--after-revision <n>] [--timeout <毫秒> | --forever] [--format text|json]\n\n在持有的 Client Session 上轮询一个 Agent 会话，直到请求的条件命中、\n修订栅栏拒绝、会话应答终止，或总截止到达。'--until' 可重复，取值为：\nblocked、done、needs-attention、failed、succeeded、canceled、interrupted、working、\nidle、unknown、ended、fresh、stale、disconnected（默认：\nneeds-attention failed succeeded canceled interrupted idle ended）。\n状态条件要求 fresh 新鲜度；三个新鲜度条件直接匹配观察。\n'--after-revision' 是全局修订栅栏而非回合身份；'--timeout' 将整个\n等待界于 1-3600000 毫秒（默认 600000）。--forever 取消总期限，\n与 --timeout 互斥，单次读取仍有界。blocked 匹配 fresh needs-attention；\ndone 匹配 fresh 的 succeeded/failed/canceled/interrupted，不代表未读。\n轮询采样当前状态：在两次\n已完成观察之间开始又结束的跃迁会被错过。'--format json' 输出\norchbee.agent.wait 封套。\n目标：一个位置参数 Session ID/名称/Panel ID、--session <id>、\n--name <alias> 或 --panel <id|@panel:suffix>。Session ID、名称优先于 Panel。\nPanel 选择主终端，多 Agent 匹配时拒绝；等待固定到首次解析的 Session。\n";
const HELP_SITE_DIAGNOSE_EN: &str = "orchbee site diagnose [--format text|json]\n\nReport bounded facts about the implicit per-user local Site runtime —\nits base and scope directory, the activation marker and locks, and the\nfinite client/worker/pending/observation/host endpoint paths. Every row\nis a pathname or descriptor fact at its own observation instant: 'held'\nmeans some owner held the lifetime lock then — never that a Site or any\nprocess is alive — and 'absent' is a pathname fact, never proof that an\nexecution ended. A 'known' marker or a valid client/worker socket leaf\nadds a conditional advisory: if no existing Site can be joined, fresh\nactivation is blocked by current recovery policy.\n\nThe command never connects to an endpoint, never creates or deletes a\npath, never acquires the activation lock, and never repairs, stops, or\npredicts a later start. A cooperative five-second budget is checked\nbetween bounded filesystem operations; rows it cannot reach close as\n'uninspectable'. '--format json' emits the orchbee.site.diagnose\nenvelope on Linux; Windows emits orchbee.site.diagnose.windows with\nprivate-DACL and bounded pipe-name observations instead of POSIX facts.\n";
const HELP_SITE_DIAGNOSE_ZH: &str = "orchbee site diagnose [--format text|json]\n\n报告隐式每用户本地 Site 运行时的有界事实——基目录与 scope 目录、\n激活标记与锁，以及有限的 client/worker/pending/observation/host 端点\n路径。每一行都是其各自观察瞬间的路径名或描述符事实：'held' 表示\n某个属主在那一刻持有生命周期锁——绝不表示某个 Site 或进程存活；\n'absent' 是路径名事实，绝不证明某个执行已结束。观察到 'known' 标记\n或合法的 client/worker 套接字叶时附加一条条件通告：若无可加入的\n现有 Site，按当前恢复策略新的激活将被阻止。\n\n本命令绝不连接任何端点，绝不创建或删除路径，绝不获取激活锁，也\n绝不修复、停止或预测后续启动。协作式五秒预算在有界的文件系统操作\n之间检查；无法到达的行以 'uninspectable' 闭合。'--format json'\n在 Linux 输出 orchbee.site.diagnose 封套；Windows 输出\norchbee.site.diagnose.windows，使用私有 DACL 和有界管道名称观察，\n不虚构 POSIX 事实。\n";
const HELP_AGENT_DOCTOR_EN: &str = "orchbee agent doctor [--provider <id>]... [--asset-root <path>]... [--format text|json]\n\nExplain, from bounded filesystem facts alone, why Agent observations may\nnot arrive: each provider's executable and managed-hook state, declared\nasset roots, the observation endpoint, and the environment override.\n'--provider' selects canonical provider ids (repeatable); '--asset-root'\ndeclares absolute capsule roots to inspect (repeatable, at most 4).\nNothing is executed, connected, written, or repaired. '--format json'\nemits the orchbee.agent.doctor envelope (Windows v3, Linux v1). Windows\nreports endpoint delivery as uninspected, separately from installation.\n";
const HELP_AGENT_DOCTOR_ZH: &str = "orchbee agent doctor [--provider <id>]... [--asset-root <path>]... [--format text|json]\n\n仅凭有界文件系统事实解释 Agent 观察为何可能未到达：各 provider 的\n可执行文件与托管 hook 状态、声明的 asset root、观察端点以及环境覆盖。\n'--provider' 选择规范 provider ID（可重复）；'--asset-root' 声明要检查\n的绝对 capsule root（可重复，至多 4 个）。不执行、不连接、不写入、\n不修复任何内容。'--format json' 输出 orchbee.agent.doctor 封套\n（Windows v3、Linux v1）。Windows 单独报告尚未检查的端点投递，\n不会把安装状态当成链路可用。\n";

const HELP_AGENT_INTEGRATION_EN: &str = "orchbee agent integration list|explain|enable|disable\n\nManage the local onboarding of supported agent providers to the Orchbee\nobservation transport. These commands run entirely on the invoking user's\nmachine and PATH — they never contact a Site.\n  agent integration list [--provider <id>]\n                                Inspect every supported provider's local state\n  agent integration explain --provider <id>\n                                Show one provider's state and the plan enable\n                                would run\n  agent integration enable --provider <id>\n                                Install and enable the managed observation\n                                integration\n  agent integration disable --provider <id>\n                                Remove the managed integration\nlist and explain execute each provider's own bounded read commands where\nrequired; enable and disable additionally run bounded, non-interactive\nmutation steps. Provider-internal bookkeeping cannot be guaranteed absent.\n";
const HELP_AGENT_INTEGRATION_ZH: &str = "orchbee agent integration list|explain|enable|disable\n\n管理受支持的 agent provider 到 Orchbee 观测传输的本地接入。这些命令完全\n在调用用户的机器与 PATH 上运行——不接触 Site。\n  agent integration list [--provider <id>]\n                                检查每个受支持 provider 的本地状态\n  agent integration explain --provider <id>\n                                显示一个 provider 的状态及 enable 将执行的\n                                计划\n  agent integration enable --provider <id>\n                                安装并启用受管观测集成\n  agent integration disable --provider <id>\n                                移除受管集成\nlist 与 explain 在需要时会执行各 provider 自身的有界只读命令；enable 与\ndisable 还会执行有界、非交互的变更步骤。provider 内部记账无法保证不\n发生。\n";

const HELP_AGENT_INTEGRATION_LIST_EN: &str = "orchbee agent integration list [--provider <id>] [--outdated-only] [--adapter-source <abs>] [--format text|json]\n\nInspect every supported provider's local integration state — or only the\none selected by '--provider'. Reads run the providers' own bounded\ncommands from the ambient PATH where required; nothing is written.\n'--adapter-source' overrides the cohort adapter payload for development\nand tests. Windows --outdated-only shows owned updates/repairs, preserving\nunknown and blocked provider diagnostics. Use enable to apply a guarded repair.\n'--format json' emits the orchbee.agent.integrations envelope.\n";
const HELP_AGENT_INTEGRATION_LIST_ZH: &str = "orchbee agent integration list [--provider <id>] [--outdated-only] [--adapter-source <abs>] [--format text|json]\n\n检查每个受支持 provider 的本地集成状态，或仅检查 '--provider' 所选的一\n个。需要时会执行 provider 自身来自环境 PATH 的有界读命令；不写入任何\n内容。'--adapter-source' 为开发与测试覆盖 cohort adapter 载荷。\nWindows --outdated-only 只显示需更新／修复的自有条目，保留未知和冲突诊断。\n使用 enable 显式执行受保护的修复。'--format json' 输出 orchbee.agent.integrations 封套。\n";

const HELP_AGENT_INTEGRATION_EXPLAIN_EN: &str = "orchbee agent integration explain --provider <id> [--adapter-source <abs>] [--format text|json]\n\nExplain one provider's local integration state and the guard-evaluated\nplan 'enable' would run for the observed dimensions. Reads execute the\nprovider's own bounded commands from the ambient PATH where required;\nnothing is written. '--format json' emits the\norchbee.agent.integration.plan envelope.\n";
const HELP_AGENT_INTEGRATION_EXPLAIN_ZH: &str = "orchbee agent integration explain --provider <id> [--adapter-source <abs>] [--format text|json]\n\n解释一个 provider 的本地集成状态，以及针对观测到的维度 'enable' 将执行\n的、经守卫求值的计划。需要时会执行 provider 自身来自环境 PATH 的有界\n读命令；不写入任何内容。'--format json' 输出\norchbee.agent.integration.plan 封套。\n";

const HELP_AGENT_INTEGRATION_ENABLE_EN: &str = "orchbee agent integration enable --provider <id> [--adapter-source <abs>] [--format text|json]\n\nEnable one provider's managed observation integration locally: publish\nthe adapter capsule when needed, then install its guarded module or run\nbounded non-interactive provider setup commands. Manual-only entries and\nunavailable providers refuse before effects. Foreign or newer provider\nentries are never replaced; the receipt retains attempted effects and\nthe state observed by re-inspection. '--format json' emits the\norchbee.agent.integration.mutation envelope.\n";
const HELP_AGENT_INTEGRATION_ENABLE_ZH: &str = "orchbee agent integration enable --provider <id> [--adapter-source <abs>] [--format text|json]\n\n在本地启用一个 provider 的受管观测集成：必要时先发布 adapter capsule，\n然后安装受保护的模块，或执行有界、非交互的 provider 设置命令。\n仅手动管理或不可用的 provider 在产生效应前拒绝。不会替换外来或更新\n版本的 provider 条目；回执保留已尝试的效应和复检观察到的状态。\n'--format json' 输出 orchbee.agent.integration.mutation 封套。\n";

const HELP_AGENT_INTEGRATION_DISABLE_EN: &str = "orchbee agent integration disable --provider <id> [--adapter-source <abs>] [--format text|json]\n\nDisable one provider's managed observation integration locally: run the\nprovider's own bounded removal commands or guarded removal of the owned\nentry. Only entries verifiably owned by Orchbee are ever removed; foreign\nor newer-versioned entries are refused before any effect. '--format json'\nemits the orchbee.agent.integration.mutation envelope.\n";
const HELP_AGENT_INTEGRATION_DISABLE_ZH: &str = "orchbee agent integration disable --provider <id> [--adapter-source <abs>] [--format text|json]\n\n在本地禁用一个 provider 的受管观测集成：执行 provider 自身的有界移除\n命令，或对所有物执行受保护的移除。只有经核实归 Orchbee 所有的条目\n才会被移除；外来或更新版本的条目在任何效应前被拒绝。'--format json'\n输出 orchbee.agent.integration.mutation 封套。\n";

const HELP_API_EN: &str = "orchbee api snapshot|schema\n\nMachine introspection of the local Workbench surface.\n  api snapshot           Emit the complete workbench.snapshot document\n  api schema [<id>]      List embedded JSON Schemas, or one schema by ID\nSuccessful output is always JSON, whatever '--format' was selected; a\nfailure envelope still requires an explicit '--format json'.\n";
const HELP_API_ZH: &str = "orchbee api snapshot|schema\n\n本地 Workbench 面的机器自省。\n  api snapshot           输出完整 workbench.snapshot 文档\n  api schema [<id>]      列出内嵌 JSON Schema，或按 ID 取单个 schema\n成功输出无论 '--format' 选择如何都是 JSON；失败封套仍需显式\n'--format json'。\n";

const HELP_CONFIG_EN: &str = "orchbee config check\n\nInspect local inputs only: runtime base, Site runtime directory, current\ncohort integrity, effective locale, and socket presence plus the socket\naddress budget. Nothing is connected, started, created, or locked. Any\nfailed check exits 1. Windows also validates APPDATA/Orchbee/config.toml;\nORCHBEE_CONFIG_FILE selects an absolute configuration file.\n";
const HELP_CONFIG_ZH: &str = "orchbee config check\n\n仅检查本地输入：运行时基座、Site 运行时目录、当前 cohort 完整性、生效\nlocale、套接字存在性与套接字地址长度预算。不连接、不启动、不创建、\n不加锁。任一检查失败以 1 退出。Windows 还校验 APPDATA/Orchbee/config.toml；\nORCHBEE_CONFIG_FILE 可指定绝对配置文件路径。\n";

const HELP_COMPLETION_EN: &str = "orchbee completion bash|zsh|fish|powershell|elvish\n\nEmit a static completion script for the implemented grammar to stdout.\nNothing is installed and no user files are written.\n";
const HELP_COMPLETION_ZH: &str = "orchbee completion bash|zsh|fish|powershell|elvish\n\n向标准输出生成已实现语法的静态补全脚本。不安装、不写入用户文件。\n";

const HELP_PROJECT_LIST_EN: &str = "orchbee project list [--format text|json]\n\nList every Project in the running local Site: id, label, directory and\nworkspace counts. An empty Site prints just the table header.\n";
const HELP_PROJECT_LIST_ZH: &str = "orchbee project list [--format text|json]\n\n列出本地 Site 中所有 Project：ID、label、目录与工作区计数。空 Site 仅\n输出表头。\n";

const HELP_PROJECT_GET_EN: &str = "orchbee project get <target> [--format text|json]\n\nShow one Project: its directories (with worker ids and paths) and its\nworkspaces. The target is an exact 32-hex ID, an @project:<suffix>\nselector, or an exact case-sensitive label; ambiguous labels or suffixes\nreport candidates instead of guessing.\n";
const HELP_PROJECT_GET_ZH: &str = "orchbee project get <target> [--format text|json]\n\n显示一个 Project：其目录（含 worker id 与路径）与工作区。target 为精确\n32 位十六进制 ID、@project:<suffix> 选择器或精确区分大小写的 label；\n歧义 label 或后缀报告候选而非猜测。\n";

const HELP_PROJECT_CREATE_EN: &str = "orchbee project create --label <label> [--format text|json]\n\nCreate one Project with the given label. The command reads one snapshot,\nthen submits exactly one command under a fresh request id. The result\ncarries the committed revision and the Server-allocated project id.\nA new Project has no directory or Workspace, and creating one touches no\nfilesystem path. Labels may repeat across Projects; address the new\nProject by its returned id.\n";
const HELP_PROJECT_CREATE_ZH: &str = "orchbee project create --label <label> [--format text|json]\n\n以给定 label 创建一个 Project。命令先读取一次快照，然后以全新请求 ID\n只提交一个命令。结果携带已提交修订与 Server 分配的 project id。\n新 Project 无目录、无 Workspace，创建不触碰任何文件系统路径。\nlabel 允许跨 Project 重复；请按返回的 ID 寻址新 Project。\n";

const HELP_PROJECT_RENAME_EN: &str = "orchbee project rename <target> --label <label> [--format text|json]\n\nRename one Project selected by exact ID, @project:<suffix>, or exact\nlabel. The command\nreads one snapshot, then submits exactly one command. A committed result\ncarries the new revision; an unchanged one means the label already\nmatched.\n";
const HELP_PROJECT_RENAME_ZH: &str = "orchbee project rename <target> --label <label> [--format text|json]\n\n重命名一个由精确 ID、@project:<suffix> 或精确 label 选择的 Project。命令先读取一次快\n照，然后只提交一个命令。committed 结果携带新修订；unchanged 表示\nlabel 本已相同。\n";

const HELP_WORKSPACE_LIST_EN: &str = "orchbee workspace list [--project <t>] [--client <id>] [--format text|json]\n\nList Workspaces across Projects, or only inside the Project selected by\n--project (exact ID, @project:<suffix>, or exact label). Fields: ids,\nlabel, primary directory\n(null means none), and terminal context.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\n";
const HELP_WORKSPACE_LIST_ZH: &str = "orchbee workspace list [--project <t>] [--client <id>] [--format text|json]\n\n跨 Project 列出 Workspace，或仅列 --project（精确 ID、@project:<suffix> 或精确 label）所选\nProject 内的。字段：各 ID、label、主目录（null 表示无）与终端上下文。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。\n使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\n";

const HELP_WORKSPACE_GET_EN: &str = "orchbee workspace get <target> [--client <id>] [--format text|json]\n\nShow one Workspace plus the Tabs that live in its terminal context.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\n";
const HELP_WORKSPACE_GET_ZH: &str = "orchbee workspace get <target> [--client <id>] [--format text|json]\n\n显示一个 Workspace 及其终端上下文中存在的 Tab。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。\n使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\n";

const HELP_WORKSPACE_CREATE_EN: &str = "orchbee workspace create [--project <target>] [--label <label>] [--workspace <source>] [--cwd <path|follow|home|current|workspace>] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--env <NAME=VALUE>...] [--exec <absolute-path> --arg <value>... | --profile <selector>] [--cols <n> --rows <n> --cell-width <n> --cell-height <n>] [--format text|json]\n\nWindows: optional inputs launch a Workspace and its first terminal. Follow the proven source terminal root cwd, then its Workspace directory, then Windows home. Explicit path/home/current ignores unrelated caller/source identity. Relative and tilde paths are accepted literally without shell expansion. --workspace chooses a source; --project chooses the destination. Missing Project uses the source Project, the sole Project, or creates Default only when no Projects exist; ambiguity fails. Missing label uses the directory basename.\nDefault --no-focus. --focus requires an existing ready TUI before creation and waits for its rendered selection; navigation changes stop focus. Timeout: 1..3600000 ms, default 30000. JSON orchbee.workspace.creation preserves separate Project, Workspace and Client receipts, including pending/started/failed/unavailable terminal facts. Failed or unknown later work retains earlier commits. Never automatically retry an unknown result.\nCompatibility: --project plus --label with no new options creates an empty Workspace; adding an absolute --cwd retains the existing mutation-v4 launch. Add --no-focus to explicitly choose the new launch workflow. Env/program/profile/geometry options without cwd also use the new defaults. Mac/Linux behavior is unchanged.\n";
const HELP_WORKSPACE_CREATE_ZH: &str = "orchbee workspace create [--project <target>] [--label <label>] [--workspace <source>] [--cwd <path|follow|home|current|workspace>] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--env <NAME=VALUE>...] [--exec <absolute-path> --arg <value>... | --profile <selector>] [--cols <n> --rows <n> --cell-width <n> --cell-height <n>] [--format text|json]\n\nWindows：可省略参数，创建 Workspace 并启动首个终端。依次继承经验证的来源终端根进程 cwd、来源 Workspace 目录、Windows 主目录。显式路径/home/current 忽略无关调用者和来源身份。支持相对路径和波浪号，不做 shell 展开。--workspace 选择来源，--project 选择目标项目。省略 Project 时使用来源项目、唯一项目，或仅在零项目时创建默认项目；歧义则拒绝。省略名称时使用目录末段。\n默认 --no-focus。--focus 在创建前要求现有就绪 TUI，并等待选择绘制；用户导航变化时停止聚焦。超时 1..3600000 毫秒，默认 30000。JSON orchbee.workspace.creation 分别保留项目、工作区和 Client 回执，包含 pending/started/failed/unavailable 终端事实。后续失败或未知时保留先前提交，不自动重试未知结果。\n兼容：只给 --project 和 --label 且没有新选项时仍创建空 Workspace；加绝对 --cwd 时保留原 mutation-v4 启动流程。显式加 --no-focus 可选择新启动流程。不带 cwd 的环境/程序/profile/尺寸选项也使用新默认行为。Mac/Linux 行为不变。\n";
const HELP_WORKSPACE_RENAME_EN: &str = "orchbee workspace rename <target> --label <label> [--format text|json]\n\nRename one Workspace selected by exact ID, @workspace:<suffix>, or exact\nlabel. The command\nreads one snapshot, then submits exactly one command. A committed result\ncarries the new revision; an unchanged one means the label already\nmatched.\n";
const HELP_WORKSPACE_RENAME_ZH: &str = "orchbee workspace rename <target> --label <label> [--format text|json]\n\n重命名一个由精确 ID、@workspace:<suffix> 或精确 label 选择的 Workspace。命令先读取一次快\n照，然后只提交一个命令。committed 结果携带新修订；unchanged 表示\nlabel 本已相同。\n";
const HELP_WORKSPACE_ACCEPT_DIRECTORY_EN: &str = "orchbee workspace accept-directory <target> --path <path> [--format text|json]\n\nAccept a literal absolute local-platform path as the primary directory of the\nWorkspace selected by exact ID, @workspace:<suffix>, or exact label.\n'--path' is required: a\nliteral absolute path of at most 4,096 UTF-8 bytes without NUL — never\nexpanded ('~', '$'), created, or probed by the CLI. The command reads one\nsnapshot, resolves the target and its owning Project, then submits\nexactly one command under a fresh request id; the Server authoritatively\nprobes the path and rejects a Workspace that already has a directory.\nA committed result carries the new revision, the Server-allocated\ndirectory and worker ids, the resolved project and workspace ids, and the\nrequest's literal path.\n";
const HELP_WORKSPACE_ACCEPT_DIRECTORY_ZH: &str = "orchbee workspace accept-directory <target> --path <path> [--format text|json]\n\n将一个字面本机绝对路径接受为由精确 ID、@workspace:<suffix> 或精确 label 选择的\nWorkspace 的主目录。'--path' 为必填：字面绝对路径，至多 4,096 个\nUTF-8 字节且不含 NUL——CLI 不展开（'~'、'$'）、不创建、不探测。命\n令先读取一次快照、解析目标及其所属 Project，然后以全新请求 ID 只\n提交一个命令；由 Server 权威探测该路径，并拒绝已有目录的\nWorkspace。committed 结果携带新修订、Server 分配的 directory 与\nworker ID、已解析的 project 与 workspace ID，以及请求的字面路径。\n";

const HELP_WORKSPACE_CLOSE_EN: &str = "orchbee workspace close <target> --confirm [--format text|json]\n\nClose every terminal the Workspace's Clients use, selected by exact ID,\n@workspace:<suffix>, or exact label. '--confirm' is required. The\nWorkspace and its terminal context are removed after all terminals close;\nits checkout, branch, and directories are preserved. An accepted result\ncarries the accepted revision, the close request id, and the resolved\nproject, workspace, and terminal context ids; the Site finishes the\nclose asynchronously and re-running this command is not a resume — use\n'workspace close-resume' for a retained resumable operation.\n\nWindows group Close: add --group to close the captured main/linked Workspace group, source last. A linked Workspace closes only itself. Git directories and branches stay intact. Group mode requires --confirm and accepts --request-id <operation-id>, --detach and --timeout <1..3600000ms> (default 120000). Waiting cancellation does not cancel accepted closure. JSON uses orchbee.workspace.group-close.\n";
const HELP_WORKSPACE_CLOSE_ZH: &str = "orchbee workspace close <target> --confirm [--format text|json]\n\n关闭由精确 ID、@workspace:<suffix> 或精确 label 选择的 Workspace 中\nClients 使用的全部终端。'--confirm' 为必填。全部终端关闭后移除\nWorkspace 及其终端上下文；checkout、branch 和磁盘目录保留。\naccepted 结果携带已接受修订、关闭请求 ID，以及解析出的 project、\nworkspace 与终端上下文 ID；Site 异步完成关闭，重复执行本命令不等于\n恢复——对保留的可恢复操作请使用 'workspace close-resume'。\n\nWindows 组关闭：添加 --group 关闭捕获的主/关联 Workspace 组，源工作区最后关闭。关联 Workspace 只关闭自身。Git 目录和分支保留。组模式必须带 --confirm，可带 --request-id <operation-id>、--detach 和 --timeout <1..3600000ms>（默认 120000）。取消等待不取消已接纳的关闭。JSON 使用 orchbee.workspace.group-close。\n";

const HELP_WORKSPACE_CLOSE_STATUS_EN: &str = "orchbee workspace close-status --project <id> --workspace <id> [--format text|json]\n\nRead the retained Workspace Close operation for one scope, selected by\nstrict stable IDs: '--project' and '--workspace' are complete nonzero\n32-lowercase-hex Workbench IDs, never labels or suffixes. The record\noutlives the Workspace's active projection, so a completed close still\nanswers with its closed tombstone revision. An available result carries\nthe operation identity, attempt generation, scope status, aggregate\ncounts, and every captured member's lifecycle truth; not-closing means\nno retained operation covers the scope. '--format json' emits the\norchbee.workspace.close-status envelope.\n\nFor a retained group use: workspace close-status --group --request <operation-id>. Do not combine group mode with --project or --workspace. This five-second pure read works after the source Workspace disappears and reports every captured member.\n";
const HELP_WORKSPACE_CLOSE_STATUS_ZH: &str = "orchbee workspace close-status --project <id> --workspace <id> [--format text|json]\n\n按严格稳定 ID 读取某个作用域保留的 Workspace 关闭操作：'--project' 与\n'--workspace' 为完整的非零 32 位小写十六进制 Workbench ID，绝不接受\nlabel 或后缀。该记录在 Workspace 离开活动投影后仍然保留，因此已完成\n的关闭仍以其 closed 墓碑修订应答。available 结果携带操作身份、尝试\n代次、作用域状态、聚合计数与每个被捕获成员的生命周期实况；\nnot-closing 表示没有保留操作覆盖该作用域。'--format json' 输出\norchbee.workspace.close-status 封套。\n\n读取保留组：workspace close-status --group --request <operation-id>。组模式不能与 --project 或 --workspace 混用。此五秒纯读取在源 Workspace 消失后仍可使用，并报告全部捕获成员。\n";

const HELP_WORKSPACE_CLOSE_RESUME_EN: &str = "orchbee workspace close-resume --project <id> --workspace <id> --request <close-id> --generation <n> --confirm [--format text|json]\n\nOpen one fresh attempt window for a retained, resumable Workspace Close.\nAll selectors are strict stable IDs: '--project' and '--workspace' are\ncomplete 32-lowercase-hex Workbench IDs, '--request' is the close\noperation's own request id from 'workspace close' or 'close-status', and\n'--generation' is the nonzero attempt generation last observed — an\noutdated fence answers stale, never a second attempt. '--confirm' is\nrequired. Only resumable lifecycle states resume; unknown outcomes are\nnon-resumable. A resumed result carries the operation identity and the\nnew attempt generation.\n\nFor a retained group use: workspace close-resume --group --request <operation-id> --generation <1..64> --confirm [--detach] [--timeout <ms>]. Scope ID selectors cannot be combined with group mode. The captured members and accepted child Close IDs stay fixed; unknown outcomes cannot resume.\n";
const HELP_WORKSPACE_CLOSE_RESUME_ZH: &str = "orchbee workspace close-resume --project <id> --workspace <id> --request <close-id> --generation <n> --confirm [--format text|json]\n\n为一个保留的、可恢复的 Workspace 关闭操作开启一次新的尝试窗口。所有\n选择器均为严格稳定 ID：'--project' 与 '--workspace' 为完整 32 位小写\n十六进制 Workbench ID，'--request' 为来自 'workspace close' 或\n'close-status' 的关闭操作自身请求 ID，'--generation' 为最近观察到的\n非零尝试代次——过期栅栏以 stale 应答，绝不发起第二次尝试。\n'--confirm' 为必填。只有可恢复的生命周期状态可恢复；unknown 结果不可\n恢复。resumed 结果携带操作身份与新的尝试代次。\n\n恢复保留组：workspace close-resume --group --request <operation-id> --generation <1..64> --confirm [--detach] [--timeout <ms>]。组模式不能混用作用域 ID 选择器。捕获的成员及已接纳子 Close ID 保持不变；结果未知时不可恢复。\n";

const HELP_TAB_LIST_EN: &str = "orchbee tab list [--workspace <t> | --standalone] [--client <id>] [--format text|json]\n\nList Tabs. Filters select one Workspace's context (exact ID,\n@workspace:<suffix>, or label) or\nthe standalone context. Fields: ids, label, pinned flag, group, and owning\ncontext kind.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\n";
const HELP_TAB_LIST_ZH: &str = "orchbee tab list [--workspace <t> | --standalone] [--client <id>] [--format text|json]\n\n列出 Tab。过滤参数可选择一个 Workspace 的上下文（精确 ID、@workspace:<suffix> 或 label）或\n独立上下文。字段：各 ID、label、置顶标记、分组与所属上下文类型。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。\n使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\n";

const HELP_TAB_GET_EN: &str = "orchbee tab get <target> [--client <id>] [--format text|json]\n\nShow one Tab including its complete PanelNode root subtree.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\n";
const HELP_TAB_GET_ZH: &str = "orchbee tab get <target> [--client <id>] [--format text|json]\n\n显示一个 Tab，包括其完整 PanelNode 根子树。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。\n使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\n";

const HELP_TAB_CREATE_EN: &str = "orchbee tab create [--workspace <target> | --standalone] [--label <label>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\n\nCreate one Tab in the selected terminal context: a Workspace chosen by\nexact ID, @workspace:<suffix>, or exact label, or the standalone context.\nThe four geometry\nflags are an all-or-none group: with all four absent the command\nrequests an 80x24 logical grid with unknown cell pixels (0,0) — a\nrequest policy, not a measured display fact; supplying any of the four\nrequires all four. The result carries the committed revision, the new\ntab, panel, and presentation ids, and the terminal spawn outcome.\n'--exec' selects the launched program by absolute path and each '--arg'\nappends one verbatim argument; omitting both launches the default shell.\n'--profile' instead selects a stored Launch Profile by exact ID or\nsingly-matching name, resolved by the Server — it cannot combine with\n'--exec' or '--arg'.\nWindows --cwd defaults to follow: sample the resolved source Panel root process cwd.\nAn absolute path overrides it; home/current use the CLI home/current directory;\nworkspace keeps Server Workspace/Worker-home defaults. Known absent cwd keeps\nthose defaults; read failures stop before creation. Explicit headless Tab context\ndoes not select a Client just to find cwd. Other platforms keep absolute paths. '--env NAME=VALUE' repeats to overlay inherited variables;\nvalues are literal, including empty values. These options also work with\n'--profile' and the default shell. Names: ASCII letters/digits/underscore,\n1..128 bytes, no leading digit; case-insensitive duplicates, ORCHBEE_*,\nTERM, COLORTERM and TERM_PROGRAM are rejected. Limit: 32 entries, 1,024\nUTF-8 name/value bytes total, no NUL. A bad directory fails the launch.\n\nWindows: omitted targets use validated caller context or one current Client selection. Invalid caller metadata never falls back; --client ignores caller environment. Unnamed Tabs display their current one-based position; --label fixes the name, including numeric names. Split labels default to the preflight Panel count plus one. No focus by default; --focus waits for the original Client to focus the new Panel after creation. Explicit targets without Client effects work headlessly. Total timeout: 30000ms (1..3600000). orchbee.creation.result v2 records directory policy/source/path and preserves creation and subsequent phase facts; later failure never undoes creation. Do not blindly repeat.\n";
const HELP_TAB_CREATE_ZH: &str = "orchbee tab create [--workspace <target> | --standalone] [--label <label>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\n\n在所选终端上下文内创建一个 Tab：由精确 ID、@workspace:<suffix> 或精确 label 选择的\nWorkspace，或独立上下文。四个几何 flag 为全有或全无组：全部省略时\n请求 80x24 逻辑网格且单元像素为未知 (0,0)——这是请求策略而非实测\n显示事实；提供其中任何一个就必须提供全部四个。结果携带已提交修\n订、新的 tab、panel、presentation ID 及终端拉起结果。\n'--exec' 以绝对路径选择启动程序，每个 '--arg' 逐字追加一个参数；\n两者均省略时启动默认 shell。\n'--profile' 改为按精确 ID 或唯一匹配的名称选择已存储的启动配置，\n由 Server 解析——不能与 '--exec' 或 '--arg' 组合。\nWindows --cwd 默认为 follow：采样已解析来源 Panel 的根进程目录。绝对路径直接覆盖；\nhome/current 使用 CLI 的主目录／当前目录；workspace 保留 Server Workspace／Worker 主目录默认值。\n明确缺失 cwd 时保留默认值；读取失败则在创建前停止。显式无界面 Tab 上下文不会为查目录选择 Client。\n其他平台仍只使用绝对路径。\n'--env NAME=VALUE' 可重复使用，覆盖继承的变量；值逐字保留，允许空值。\n这些选项也可配合 '--profile' 或默认 shell 使用。名称只允许 ASCII 字母、\n数字和下划线，1..128 字节且不能以数字开头；拒绝忽略大小写后重复的名称、\nORCHBEE_*、TERM、COLORTERM 和 TERM_PROGRAM。至多 32 项，名称和值合计\n不超过 1,024 UTF-8 字节，不能含 NUL。目录不可用时启动失败。\n\nWindows：省略目标时使用经校验的调用者上下文，或唯一 current Client 的选择。无效调用者信息不回退；显式 --client 忽略调用者环境。未命名 Tab 显示当前从一开始的位置序号；--label 固定名称，包括数字名称。分屏标签默认为创建前 Panel 数加一。默认不聚焦；--focus 在创建后等待原 Client 聚焦新 Panel。显式目标且无需 Client 操作时可无 TUI 运行。总超时默认 30000ms（1..3600000）。orchbee.creation.result v2 记录目录策略／来源／路径，并保留已创建身份及后续步骤结果；后续失败不会撤销创建，勿盲目重试。\n";

const HELP_PANEL_RENAME_EN: &str = "orchbee panel rename <panel-id> (--label <text> | --clear) [--format text|json]\n\nSet a shared custom Panel title by exact ID or @panel:<suffix>. Empty or\nwhitespace-only text clears the title and restores its creation label. Input\nis limited to 512 Unicode characters before trimming. No focus change.\nrenamed reports a committed revision; unchanged means the override matched.\n";
const HELP_PANEL_RENAME_ZH: &str = "orchbee panel rename <panel-id> (--label <text> | --clear) [--format text|json]\n\n用精确 ID 或 @panel:<suffix> 设置共享的窗格自定义标题。空文本或纯空白文本\n会清除标题，恢复创建时的标签。裁剪前最多 512 个 Unicode 字符。不会切换焦点。\nrenamed 表示已提交新修订；unchanged 表示自定义标题未变。\n";
const HELP_TAB_RENAME_EN: &str = "orchbee tab rename <target> --label <label> [--format text|json]\n\nRename one Tab selected by exact ID, @tab:<suffix>, or exact label.\nA committed result\ncarries the new revision; unchanged means an already fixed name matched. Renaming an automatic Tab fixes its name even when the visible text is equal.\n";
const HELP_TAB_RENAME_ZH: &str = "orchbee tab rename <target> --label <label> [--format text|json]\n\n重命名一个由精确 ID、@tab:<suffix> 或精确 label 选择的 Tab。committed 结果携带新修\n订；unchanged 表示固定名称本已相同。自动命名 Tab 即使文本相同，重命名也会将其改为固定名称。\n";

const HELP_TAB_MOVE_EN: &str = "orchbee tab move <target> (--direction <previous|next|first|last> | --insert-index <N>) [--format text|json]\n\nMove one Tab within its strip. A committed result carries the new\nrevision; unchanged means the tab was already in position.\nWindows --insert-index uses a boundary in the original order: 0 is first,\nTab count is last; source/source+1 are unchanged. Out of range is refused.\n";
const HELP_TAB_MOVE_ZH: &str = "orchbee tab move <target> (--direction <previous|next|first|last> | --insert-index <N>) [--format text|json]\n\n在标签条内移动一个 Tab。committed 结果携带新修订；unchanged 表示 Tab\n本已在目标位置。\nWindows --insert-index 使用原顺序的边界：0 为首位，Tab 总数为末尾；\n源位置及其后一边界无变化。超出当前数量会拒绝。\n";

const HELP_TAB_CLOSE_EN: &str = "orchbee tab close <target> --confirm [--format text|json]\n\nClose one Tab selected by exact ID, @tab:<suffix>, or exact label.\n'--confirm' is\nrequired. An accepted result carries the accepted revision and the close\nrequest id; the Site finishes the close asynchronously.\n";
const HELP_TAB_CLOSE_ZH: &str = "orchbee tab close <target> --confirm [--format text|json]\n\n关闭一个由精确 ID、@tab:<suffix> 或精确 label 选择的 Tab。'--confirm' 为必填。\naccepted 结果携带已接受修订与关闭请求 ID；Site 异步完成关闭。\n";

const HELP_PANEL_LIST_EN: &str = "orchbee panel list [--tab <t>] [--client <id>] [--format text|json]\n\nList Panels, optionally inside the Tab selected by --tab (exact ID,\n@tab:<suffix>, or label). Panels carry no label; presentation labels are\nnot panel names.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\n";
const HELP_PANEL_LIST_ZH: &str = "orchbee panel list [--tab <t>] [--client <id>] [--format text|json]\n\n列出 Panel，或仅列 --tab（精确 ID、@tab:<suffix> 或 label）所选 Tab 内的。Panel 无\nlabel；Presentation 的 label 不是 Panel 名。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。\n使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\n";

const HELP_PANEL_GET_EN: &str = "orchbee panel get <panel-id> [--client <id>] [--format text|json]\n\nShow one Panel by complete typed ID or @panel:<suffix>: owning tab,\ncontext, slot path, and its primary and child Presentations.\n\nWindows --client <id> adds that exact TUI selection and focused flags; Workspace results include remembered active Tab. No Client is guessed, and the query never navigates. Total read timeout: 5000 ms.\nWindows results include matched live Agent sessions and their aggregate status.\nUse --client to distinguish seen idle from unseen done; unavailable read state is null.\n";
const HELP_PANEL_GET_ZH: &str = "orchbee panel get <panel-id> [--client <id>] [--format text|json]\n\n按完整类型化 ID 或 @panel:<suffix> 显示一个 Panel：所属 Tab、上下文、slot 路径及其主与\n子 Presentation。\n\nWindows --client <id> 附加指定 TUI 的选择与焦点；Workspace 结果包含记忆的活动 Tab。不猜测 Client，查询不导航。读取总超时为 5000 毫秒。\nWindows 结果包含关联的存活 Agent 会话及汇总状态。\n使用 --client 区分空闲已读和完成待查看；无法确认已读状态时返回 null。\n";

const HELP_PANEL_SPLIT_EN: &str = "orchbee panel split [<panel-id> | --panel <panel-id> | --current] --direction <above|below|left|right> [--label <label>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--right-click orchbee|panel] [--ratio <number>] [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\n\nSplit one Panel, creating a sibling panel on the selected side. The\nfour geometry flags are an all-or-none group: with all four absent the\ncommand requests an 80x24 logical grid with unknown cell pixels (0,0) —\na request policy, not a measured display fact; supplying any of the\nfour requires all four. The result carries the committed revision,\nthe new panel, split, slot, and presentation ids, and the terminal spawn\noutcome. '--exec' selects the launched program by absolute path and each\n'--arg' appends one verbatim argument; omitting both launches the\ndefault shell. '--profile' instead selects a stored Launch Profile by\nexact ID or singly-matching name, resolved by the Server — it cannot\ncombine with '--exec' or '--arg'.\nWindows --cwd defaults to follow: sample the resolved source Panel root process cwd.\nAn absolute path overrides it; home/current use the CLI home/current directory;\nworkspace keeps Server Workspace/Worker-home defaults. Known absent cwd keeps\nthose defaults; read failures stop before creation. Explicit headless Tab context\ndoes not select a Client just to find cwd. Other platforms keep absolute paths. '--env NAME=VALUE' repeats to overlay inherited variables;\nvalues are literal, including empty values. These options also work with\n'--profile' and the default shell. Names: ASCII letters/digits/underscore,\n1..128 bytes, no leading digit; case-insensitive duplicates, ORCHBEE_*,\nTERM, COLORTERM and TERM_PROGRAM are rejected. Limit: 32 entries, 1,024\nUTF-8 name/value bytes total, no NUL. A bad directory fails the launch.\n\nWindows: omitted targets use validated caller context or one current Client selection. Invalid caller metadata never falls back; --client ignores caller environment. Unnamed Tabs display their current one-based position; --label fixes the name, including numeric names. Split labels default to the preflight Panel count plus one. No focus by default; --focus waits for the original Client to focus the new Panel after creation. Explicit targets without Client effects work headlessly. Total timeout: 30000ms (1..3600000). orchbee.creation.result v2 records directory policy/source/path and preserves creation and subsequent phase facts; later failure never undoes creation. Do not blindly repeat. --ratio is the first child share, clamped from a finite number to 0.1..0.9; it affects only the selected Client and does not imply focus. --right-click sets only the new Panel policy atomically with creation (default orchbee); panel permits unmodified right clicks inside a mouse-reporting terminal.\n";
const HELP_PANEL_SPLIT_ZH: &str = "orchbee panel split [<panel-id> | --panel <panel-id> | --current] --direction <above|below|left|right> [--label <label>] [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--cwd <abs|follow|home|current|workspace>] [--env NAME=VALUE]... [--right-click orchbee|panel] [--ratio <number>] [--focus | --no-focus] [--client <id>] [--timeout <ms>] [--format text|json]\n\n分裂一个 Panel，在所选侧创建相邻 Panel。四个几何 flag 为全有或全无\n组：全部省略时请求 80x24 逻辑网格且单元像素为未知 (0,0)——这是请\n求策略而非实测显示事实；提供其中任何一个就必须提供全部四个。\n结果携带已提交修订、新的 panel、split、slot、presentation ID 及\n终端拉起结果。'--exec' 以绝对路径选择启动程序，每个 '--arg' 逐字追加一个\n参数；两者均省略时启动默认 shell。'--profile' 改为按精确 ID 或唯一匹\n配的名称选择已存储的启动配置，由 Server 解析——不能与 '--exec'\n或 '--arg' 组合。\nWindows --cwd 默认为 follow：采样已解析来源 Panel 的根进程目录。绝对路径直接覆盖；\nhome/current 使用 CLI 的主目录／当前目录；workspace 保留 Server Workspace／Worker 主目录默认值。\n明确缺失 cwd 时保留默认值；读取失败则在创建前停止。显式无界面 Tab 上下文不会为查目录选择 Client。\n其他平台仍只使用绝对路径。\n'--env NAME=VALUE' 可重复使用，覆盖继承的变量；值逐字保留，允许空值。\n这些选项也可配合 '--profile' 或默认 shell 使用。名称只允许 ASCII 字母、\n数字和下划线，1..128 字节且不能以数字开头；拒绝忽略大小写后重复的名称、\nORCHBEE_*、TERM、COLORTERM 和 TERM_PROGRAM。至多 32 项，名称和值合计\n不超过 1,024 UTF-8 字节，不能含 NUL。目录不可用时启动失败。\n\nWindows：省略目标时使用经校验的调用者上下文，或唯一 current Client 的选择。无效调用者信息不回退；显式 --client 忽略调用者环境。未命名 Tab 显示当前从一开始的位置序号；--label 固定名称，包括数字名称。分屏标签默认为创建前 Panel 数加一。默认不聚焦；--focus 在创建后等待原 Client 聚焦新 Panel。显式目标且无需 Client 操作时可无 TUI 运行。总超时默认 30000ms（1..3600000）。orchbee.creation.result v2 记录目录策略／来源／路径，并保留已创建身份及后续步骤结果；后续失败不会撤销创建，勿盲目重试。 --ratio 是第一子节点的份额，有限数值限制到 0.1..0.9；仅影响所选 Client，不隐含聚焦。 --right-click 随创建原子设置新 Panel 的策略（默认 orchbee）；panel 允许无修饰键的右键在启用鼠标上报的终端内容内透传。\n";

const HELP_PANEL_CLOSE_EN: &str = "orchbee panel close <panel-id> --confirm [--format text|json]\n\nClose one Panel selected by complete typed ID or @panel:<suffix>.\n'--confirm' is required.\nAn accepted result carries the accepted revision and the close request\nid; the Site finishes the close asynchronously.\n";
const HELP_PANEL_CLOSE_ZH: &str = "orchbee panel close <panel-id> --confirm [--format text|json]\n\n关闭一个由完整类型化 ID 或 @panel:<suffix> 选择的 Panel。'--confirm' 为必填。accepted\n结果携带已接受修订与关闭请求 ID；Site 异步完成关闭。\n";

#[cfg(not(windows))]
const HELP_PANEL_MOVE_EN: &str = "orchbee panel move <panel-id> (--target-panel <panel-id> | --into-tab <tab> | --new-tab) [--format text|json]\n\nMove one leaf Panel to one exclusive destination:\n  --target-panel <panel-id> --direction <above|below|left|right>\n        Next to a sibling panel inside the same tab (panel.move).\n        Both panels accept complete typed IDs or @panel:<suffix> selectors.\n  --into-tab <tab> --direction <above|below|left|right>\n        Into an existing sibling Tab of the same TerminalContext,\n        resolved by exact ID or label; ambiguous labels are rejected.\n  --new-tab --label <l>\n        Into a fresh Tab appended to the same TerminalContext.\n\nOnly a leaf Panel relocates: a stacked Panel rejects panel-not-leaf, and\nthe source Tab's last leaf rejects last-panel-in-tab — an emptied Tab is\nnever retired implicitly; close it explicitly. Relocation keeps the\nPanel's identity, Presentation, terminal binding, and process: nothing\nrelaunches and the working directory is unchanged. A committed result\ncarries the new revision; unchanged means the layout already matched.\n";
#[cfg(not(windows))]
const HELP_PANEL_MOVE_ZH: &str = "orchbee panel move <panel-id> (--target-panel <panel-id> | --into-tab <tab> | --new-tab) [--format text|json]\n\n将一个叶子 Panel 移动到恰好一种目的地：\n  --target-panel <panel-id> --direction <above|below|left|right>\n        移到同一 Tab 内某个相邻 Panel 的所选侧（panel.move）。\n        两个 Panel 均接受完整类型化 ID 或 @panel:<suffix> 选择器。\n  --into-tab <tab> --direction <above|below|left|right>\n        移入同一 TerminalContext 中已存在的兄弟 Tab，按精确 ID 或\n        label 解析；歧义 label 会被拒绝。\n  --new-tab --label <l>\n        移入追加到同一 TerminalContext 的新建 Tab。\n\n只有叶子 Panel 可迁移：堆叠 Panel 以 panel-not-leaf 拒绝，源 Tab 的\n最后一片叶子以 last-panel-in-tab 拒绝——空 Tab 绝不被隐式回收，需\n显式关闭。迁移保留 Panel 身份、Presentation、终端绑定与进程：不重新\n启动，工作目录不变。committed 结果携带新修订；unchanged 表示布局本已相同。\n";

const HELP_TERMINAL_LIST_EN: &str = "orchbee terminal list [--format text|json]\n\nList every terminal bound by a Presentation. Unbound terminals are not\nvisible; a binding is structural evidence, not proof a process is alive.\n";
const HELP_TERMINAL_LIST_ZH: &str = "orchbee terminal list [--format text|json]\n\n列出所有被 Presentation 绑定的终端。未绑定终端不可见；绑定是结构证据，\n不是进程存活的证明。\n";

const HELP_TERMINAL_GET_EN: &str = "orchbee terminal get <terminal-id> [--format text|json]\n\nShow one bound terminal by complete typed ID or @terminal:<suffix>:\nworker id and its single boundTo location (presentation, panel, tab,\ncontext).\n";
const HELP_TERMINAL_GET_ZH: &str = "orchbee terminal get <terminal-id> [--format text|json]\n\n按完整类型化 ID 或 @terminal:<suffix> 显示一个已绑定终端：worker id 及其唯一 boundTo 位置\n（presentation、panel、tab、context）。\n";

const HELP_TERMINAL_READ_EN: &str = "orchbee terminal read <terminal-id> [--history --rows <n>] [--unwrap] [--lines <n>] [--format text|json]\n\nRead the current visible content of one bound terminal. The terminal is\nresolved from the Workbench snapshot, then observed once through a short-\nlived read-only viewer on the same session; nothing is created, activated,\nor mutated. Text output prints the grid rows; '--format json' emits the\norchbee.terminal.read envelope with frame metadata and row content.\n\nWith '--history --rows <n>' (both flags required together, 1..=1000), read\nup to n retained rows that have scrolled off the active screen instead of\nthe visible frame; '--format json' emits the orchbee.terminal.history\nenvelope with scrollback metadata and row content.\n\n'--unwrap' is a read-only presentation transform: verified soft-wrapped\nphysical rows join into logical lines (hard breaks and empty lines are\npreserved; a window edge inside a wrapped line reports startsMidLine/\nendsMidLine instead of claiming a complete line). '--lines <n>' (1..=65535)\nshows only the last n emitted entries — logical lines under '--unwrap',\nphysical rows otherwise — and never widens the fetched window.\n\nExplicit sources: --source visible|recent|recent-unwrapped|detection.\nRecent includes history and live screen (default 80 physical rows); detection\nuses one screen height near the cursor, not Agent state. Source-mode --lines\naccepts 0..4294967295 and clamps to 1000. Visible is this viewer's live screen.\n--plain prints UTF-8 text directly and defaults to recent; it conflicts with\n--format json. Source/plain cannot mix with --history, --rows or --unwrap.\nJSON uses orchbee.terminal.output; a stale sample fails without a hidden retry.\n\n--ansi exports styled text directly (default recent); --format json instead\nuses orchbee.terminal.output v2 with encoding: ansi. It conflicts with\n--plain, --history, --rows and --unwrap. Styled blanks remain content;\nrecent-unwrapped joins soft wraps. Detection stays plain text. This is a\nstyled snapshot with SGR, not original PTY bytes or graphics replay.\n";
const HELP_TERMINAL_READ_ZH: &str = "orchbee terminal read <terminal-id> [--history --rows <n>] [--unwrap] [--lines <n>] [--format text|json]\n\n读取一个已绑定终端的当前可见内容。终端经 Workbench 快照解析后，通过同\n一会话上短暂存活的只读 viewer 观察一次；不创建、不激活、不修改任何\n状态。文本输出逐行打印网格内容；'--format json' 输出带帧元数据与行内\n容的 orchbee.terminal.read 封套。\n\n加 '--history --rows <n>'（两个 flag 必须成对，1..=1000）时，读取已滚\n出当前活动屏幕的至多 n 行保留历史，而非可见帧；'--format json' 输出\n带历史元数据与行内容的 orchbee.terminal.history 封套。\n\n'--unwrap' 是只读的呈现变换：经验证的软换行物理行拼接为逻辑行（保留\n硬换行与空行；窗口边缘落在换行行内部时以 startsMidLine/endsMidLine\n如实标记，而非声称完整行）。'--lines <n>'（1..=65535）只显示最后 n\n条已发出的条目——'--unwrap' 下为逻辑行，否则为物理行——且不会扩大\n抓取窗口。\n\n显式来源：--source visible|recent|recent-unwrapped|detection。\nrecent 合并历史与实时屏幕，默认 80 个物理行；detection 读取光标附近\n一个屏幕高度，不是 Agent 状态。来源模式 --lines 接受 0..4294967295，\n钳制到 1000。visible 是当前查看器的实时屏幕。\n--plain 直接输出 UTF-8 文本，默认 recent；与 --format json 冲突。\n来源/plain 不能与 --history、--rows 或 --unwrap 混用。\nJSON 使用 orchbee.terminal.output；样本过期时失败，不隐藏重试。\n\n--ansi 直接导出带样式文本（默认 recent）；配合 --format json 时改用\norchbee.terminal.output v2，encoding 为 ansi。它与 --plain、--history、\n--rows 和 --unwrap 冲突。保留带样式空格；recent-unwrapped 拼接软换行。\ndetection 仍输出纯文本。这是包含 SGR 的样式快照，不是原始 PTY 字节或图形重放。\n";

const HELP_TERMINAL_WAIT_OUTPUT_EN: &str = "orchbee terminal wait-output <terminal-id> (--match <text> | --regex <pattern>) [--rows <n>] [--timeout <ms>] [--format text|json]\n\nWait until the pattern appears in one bound terminal's retained output —\nexactly one of '--match' or '--regex' is required. '--match' is a literal,\ncase-sensitive substring (1..=4096 bytes); '--regex' is a nonempty UTF-8\nregular expression of at most 1024 bytes, compiled once before any\nconnection (invalid or oversized programs are usage failures). Matching\nchecks the most recent --rows retained rows (1..=1000, default 1000)\nspanning the visible screen and scrollback as one window, joining\nsoft-wrapped rows into logical lines first; a regex match reports its\nbyte length and fragment-edge flags, and '^'/'$' anchor at the observed\nfragment edges. '--timeout' bounds the total wait in milliseconds\n(1..=3600000, default 30000); the command exits 0 on match, 7 on timeout,\nand 130 when cancelled by HUP/INT/QUIT/TERM. Nothing is typed into the\nterminal and nothing is created or mutated. Text output prints the\nmatching line; '--format json' emits the orchbee.terminal.wait-output\nenvelope with match and observation metadata.\n\nSource mode: --source visible|recent|recent-unwrapped|detection, --lines N,\nor --forever. It defaults to recent (80 physical rows), excludes --rows,\nand uses orchbee.terminal.wait-output v3. --lines allows zero and clamps to\n1000. Recent sources join soft wraps; visible/detection match physical lines.\n--forever removes the total deadline, conflicts with --timeout, and retains\nfive-second I/O limits and Ctrl+C cleanup. Zero lines never match.\n";
const HELP_TERMINAL_WAIT_OUTPUT_ZH: &str = "orchbee terminal wait-output <terminal-id> (--match <text> | --regex <pattern>) [--rows <n>] [--timeout <ms>] [--format text|json]\n\n等待一个已绑定终端的保留输出中出现指定模式——'--match' 与 '--regex'\n必须且只能提供一个。'--match' 为字面、区分大小写的子串（1..=4096 字\n节）；'--regex' 为至多 1024 字节的非空 UTF-8 正则表达式，在任何连接\n建立前编译一次（非法或超限程序报用法错误）。搜索范围为最近 --rows\n行保留行（1..=1000，默认 1000），可见屏与回滚历史作为一个窗口，并\n先把软换行行拼接为逻辑行；正则命中会报告字节长度与片段边缘标记，\n'^'/'$' 仅锚定观测到的片段边界。'--timeout' 以毫秒限定总等待预算\n（1..=3600000，默认 30000）；匹配成功退出 0，超时退出 7，被\nHUP/INT/QUIT/TERM 取消时退出 130。不向终端写入任何输入，不创建、不\n修改任何状态。文本输出打印匹配行；'--format json' 输出带匹配与观测\n元数据的 orchbee.terminal.wait-output 封套。\n\n来源模式：--source visible|recent|recent-unwrapped|detection、--lines N\n或 --forever。默认 recent（80 个物理行），与 --rows 互斥，使用\norchbee.terminal.wait-output v3。--lines 允许零，钳制到 1000。\nrecent 来源拼接软换行；visible/detection 匹配物理行。\n--forever 移除总期限，与 --timeout 互斥，仍保留五秒 I/O 上限和\nCtrl+C 清理。零行从不匹配。\n";
const HELP_TERMINAL_RUN_EN: &str = r#"orchbee terminal run <target> <command> [--format text|json]

Submit one quoted command string to an existing Terminal as one paste record followed
by Enter in one request. Target: exact ID or @terminal:<suffix>.
The current program interprets the content; there is no Shell-readiness check, process
launch, or command completion/exit-status promise. Whitespace and newlines are preserved.
Use -- before operands resembling CLI options; put --format before that separator.
Empty commands and extra command operands are rejected. Total argv is at most 64 KiB;
the paste limit is 256 KiB. The existing five-second input deadline applies.
The orchbee.terminal.send receipt reports input and cleanup independently. Partial or
unknown delivery must not be replayed. No controller, resize or focus is acquired.
"#;
const HELP_TERMINAL_RUN_ZH: &str = r#"orchbee terminal run <target> <command> [--format text|json]

把一个引号内命令字符串以 paste 后接 Enter 的形式，在一个请求中提交到既有 Terminal。
目标支持精确 ID 或 @terminal:<suffix>。当前程序解释内容；不检查 Shell 就绪、
不启动进程，也不承诺命令完成或退出状态。空白和换行原样保留。
操作数像 CLI 选项时，先写 --；--format 放在分隔符之前。
拒绝空命令及多余命令操作数。完整 argv 上限 64 KiB，paste 上限 256 KiB，遵循既有五秒输入期限。
orchbee.terminal.send 回执分别报告输入与清理结果。部分或未知提交不得重放。
不获取 controller、resize 或焦点权限。
"#;

const HELP_TERMINAL_SEND_EN: &str = r#"orchbee terminal send <terminal-id> (--text <utf8> [--enter] | --key <name>... | --file <path> [--paste] [--enter] | --stdin [--paste] [--enter]) [--format text|json]

Submit one bounded input sequence to one bound terminal. Choose exactly one
source family. --text accepts up to 4096 UTF-8 bytes including an optional
carriage return. Repeat --key for up to 64 semantic keys; named keys include
enter, escape, tab, backspace, delete, up, down, left, right, home, end,
page-up, page-down, ctrl-c and ctrl-d. Modifier combinations such as
ctrl+shift+a are accepted. The Host encodes keys for the terminal's mode.

--file reads a regular file. --stdin accepts a redirected regular file or
pipe, including Windows pipes, and refuses interactive terminals/devices.
File/stdin preserves raw bytes, including non-UTF-8 data, up to 1 MiB
including an optional --enter carriage return. Empty input is refused.
--paste is valid only with --file or --stdin, limits content to 256 KiB,
and sends bracketed paste under the terminal's mode. With --paste, --enter
adds a separate trailing Enter record. Nothing is sent until the complete
source is acquired and validated under the shared five-second deadline.
Windows pipe cancellation terminates and reaps its owned reader process.

The temporary input-only attachment never takes the TUI's controller,
viewer or resize ownership. Inspect inputStatus and cleanupStatus in the
orchbee.terminal.send JSON result independently. Partial acceptance is not
complete success. When the outcome is unknown (exit 8), inspect the terminal
before another action: blindly resending may type twice. No request is
automatically replayed.
"#;
const HELP_TERMINAL_SEND_ZH: &str = r#"orchbee terminal send <terminal-id> (--text <utf8> [--enter] | --key <name>... | --file <path> [--paste] [--enter] | --stdin [--paste] [--enter]) [--format text|json]

向一个已绑定终端提交一组有界输入。必须恰好选择一种来源。
--text 接受不超过 4096 字节的 UTF-8 文字，含可选附加回车。
重复 --key 可发送最多 64 个语义按键；具名按键包括 enter、escape、tab、
backspace、delete、up、down、left、right、home、end、page-up、
page-down、ctrl-c 和 ctrl-d。接受 ctrl+shift+a 等修饰键组合，
由 Host 按终端当前模式编码。

--file 读取普通文件。--stdin 接受重定向普通文件或管道，包括 Windows
管道，拒绝交互终端和设备。文件/stdin 原样保留字节，含非 UTF-8 数据，
连同 --enter 的可选回车上限为 1 MiB。空输入被拒绝。
--paste 仅用于 --file 或 --stdin，内容上限 256 KiB，按终端模式发送
bracketed paste。此时 --enter 附加独立的尾随 Enter 记录。
来源在共用五秒期限内完整读取并校验后才发送任何内容。
Windows 管道取消时会终止并回收所属读取进程。

临时仅输入附属通道不获取 TUI 的 controller、viewer 或 resize 所有权。
分别检查 orchbee.terminal.send JSON 结果的 inputStatus 与 cleanupStatus。
部分接受不等于全部成功。结果未知（退出码 8）时，采取下一步操作前先
检查终端：盲目重发可能输入两次。请求不会自动重放。
"#;

const HELP_WORKTREE_EN: &str = "orchbee worktree list|create|open|remove|status|resume\n\nWindows: manage Git checkouts and their Workspace association.\n  list [--workspace <target>|--cwd <absolute-path>] [--trust-repository]\n  list --project <target> --directory <directory-id>\n  create [--workspace <target>|--cwd <absolute-path>] [--project <target>]\n  open (--path <absolute-path>|--branch <name>) [--workspace <target>|--cwd <absolute-path>]\n  remove --workspace <target> --confirm [--force]\n  status <operation-id>\n  resume <operation-id> --generation <attempt> --confirm\n\nMutations retain operation IDs across CLI disconnection. Status is read-only.\nUse orchbee help worktree <command> for defaults, waiting and recovery rules.\n";
const HELP_WORKTREE_ZH: &str = "orchbee worktree list|create|open|remove|status|resume\n\nWindows：管理 Git 检出目录及其 Workspace 关联。\n  list [--workspace <target>|--cwd <absolute-path>] [--trust-repository]\n  list --project <target> --directory <directory-id>\n  create [--workspace <target>|--cwd <绝对路径>] [--project <target>]\n  open (--path <绝对路径>|--branch <名称>) [--workspace <target>|--cwd <绝对路径>]\n  remove --workspace <target> --confirm [--force]\n  status <操作标识>\n  resume <操作标识> --generation <尝试序号> --confirm\n\n变更操作可跨 CLI 断线保留标识；status 为只读查询。\n用 orchbee help worktree <command> 查看默认值、等待与恢复规则。\n";

const HELP_LAUNCH_PROFILE_EN: &str = "orchbee launch-profile list|get|create|delete\n\nQuery and mutate the running local Site's Launch Profile catalog.\n  launch-profile list                              List every profile\n  launch-profile get <selector>                    Show one profile\n  launch-profile create --name <n> --exec <abs> [--arg <v>]...\n                                                   Store one named launch\n  launch-profile delete <selector>                 Delete one profile\nA selector is an exact profile ID first, then a name; name matches must\nresolve to exactly one entry. Every selector resolves under the Server's\ncatalog lock — the CLI never pre-reads the catalog or guesses a match.\n'tab create --profile' and 'panel split --profile' launch through the\nsame resolution, mutually exclusive with '--exec'/'--arg'.\n";
const HELP_LAUNCH_PROFILE_ZH: &str = "orchbee launch-profile list|get|create|delete\n\n查询并修改运行中本地 Site 的启动配置（Launch Profile）目录。\n  launch-profile list                              列出所有配置\n  launch-profile get <selector>                    显示一个配置\n  launch-profile create --name <n> --exec <abs> [--arg <v>]...\n                                                   存储一个命名启动\n  launch-profile delete <selector>                 删除一个配置\nselector 先按精确 profile ID 解析，再按名称；名称匹配必须恰好解析到一\n个条目。每个 selector 都在 Server 目录锁下解析——CLI 绝不预读目录或猜\n测匹配。'tab create --profile' 与 'panel split --profile' 通过同一解析\n启动，且与 '--exec'/'--arg' 互斥。\n";

const HELP_LAUNCH_PROFILE_LIST_EN: &str = "orchbee launch-profile list [--format text|json]\n\nList every Launch Profile in the running local Site's catalog: profile\nID, name, and the stored exec/args launch. '--format json' emits the\norchbee.launch-profile.list envelope with the catalog revision.\n";
const HELP_LAUNCH_PROFILE_LIST_ZH: &str = "orchbee launch-profile list [--format text|json]\n\n列出运行中本地 Site 目录里的所有启动配置：profile ID、名称及存储的\nexec/args 启动。'--format json' 输出携带目录修订的\norchbee.launch-profile.list 封套。\n";

const HELP_LAUNCH_PROFILE_GET_EN: &str = "orchbee launch-profile get <selector> [--format text|json]\n\nShow one Launch Profile resolved by the Server: an exact profile ID\nfirst, then a name whose match cardinality must be exactly one — an\nunmatched or ambiguous selector fails, never guesses. '--format json'\nemits the orchbee.launch-profile.get envelope.\n";
const HELP_LAUNCH_PROFILE_GET_ZH: &str = "orchbee launch-profile get <selector> [--format text|json]\n\n显示一个由 Server 解析的启动配置：先按精确 profile ID，再按匹配数必须\n恰好为一的名称——未命中或歧义的 selector 会失败而非猜测。\n'--format json' 输出 orchbee.launch-profile.get 封套。\n";

const HELP_LAUNCH_PROFILE_CREATE_EN: &str = "orchbee launch-profile create --name <name> --exec <abs> [--arg <v>]... [--format text|json]\n\nStore one named Launch Profile in the running Site's catalog. '--name'\nis required (1–512 UTF-8 bytes; uniqueness is not required). '--exec'\nselects the launched program by absolute path and each '--arg' appends\none verbatim argument. The result carries the committed revision, the\ncatalog revision, the minted profile ID, and the stored name.\n";
const HELP_LAUNCH_PROFILE_CREATE_ZH: &str = "orchbee launch-profile create --name <name> --exec <abs> [--arg <v>]... [--format text|json]\n\n在运行中 Site 的目录里存储一个命名启动配置。'--name' 必填（1–512 个\nUTF-8 字节；不要求唯一）。'--exec' 以绝对路径选择启动程序，每个\n'--arg' 逐字追加一个参数。结果携带已提交修订、目录修订、新铸造的\nprofile ID 及存储的名称。\n";

const HELP_LAUNCH_PROFILE_DELETE_EN: &str = "orchbee launch-profile delete <selector> [--format text|json]\n\nDelete one Launch Profile resolved by the Server under the same selector\nrule as 'launch-profile get': exact profile ID first, then a name\nmatching exactly one entry. The result carries the committed revision,\nthe catalog revision, and the deleted profile ID.\n";
const HELP_LAUNCH_PROFILE_DELETE_ZH: &str = "orchbee launch-profile delete <selector> [--format text|json]\n\n删除一个由 Server 按与 'launch-profile get' 相同 selector 规则解析的\n启动配置：先精确 profile ID，再按恰好匹配一个条目的名称。结果携带已\n提交修订、目录修订与被删除的 profile ID。\n";

const HELP_WORKTREE_LIST_EN: &str = "orchbee worktree list [--workspace <target>|--cwd <path>] [--trust-repository] [--client <id>] [--format text|json]\norchbee worktree list --project <target> --directory <directory-id> [--format text|json]\n\nWindows: list Git checkouts and associated Workspaces. With no source selector,\n--client selects an exact ready TUI; otherwise use the validated caller, then a unique\ncurrent ready TUI, then CLI cwd only when neither exists. Missing Workspace directory\nuses only the proven terminal root cwd. No Project is required for a directory query. Invalid caller identity\nnever falls back. --trust-repository trusts this repository for this query only.\nThe query is read-only, does not activate a Site, and has a 5-second total budget.\nJSON source queries emit orchbee.worktree.query; failed Server results keep typed reasons.\nThe legacy --project/--directory pair keeps orchbee.worktree.list and its failure schema;\nthese selectors cannot be combined with --workspace, --cwd, --client or --trust-repository.\nWorkspace targets accept exact IDs, @workspace:<suffix> or exact labels; ambiguity is rejected.\nBranches keep refs/heads/ prefixes; all-zero HEAD marks an unborn repository.\nWindows --cwd accepts absolute, caller-relative and ~ paths; home precedence: USERPROFILE, HOMEDRIVE+HOMEPATH, HOME.\n";
const HELP_WORKTREE_LIST_ZH: &str = "orchbee worktree list [--workspace <target>|--cwd <路径>] [--trust-repository] [--client <id>] [--format text|json]\norchbee worktree list --project <target> --directory <directory-id> [--format text|json]\n\nWindows：列出 Git 检出目录及关联 Workspace。省略来源选择器时，已验证的调用者终端\n优先，其次为唯一当前就绪 TUI，均无时才使用 CLI cwd。--client 选择精确就绪 TUI，\n覆盖隐式调用者。Workspace 无主目录时只用已证明终端根进程 cwd。目录查询无需 Project。无效调用者身份\n绝不回退。--trust-repository 仅在本次查询中信任该仓库。\n查询只读、不激活 Site，总预算 5 秒。来源查询 JSON 使用 orchbee.worktree.query，\nServer 失败结果保留类型化原因。原有 --project/--directory 组合保留 orchbee.worktree.list\n及其失败 Schema；不可混用 --workspace、--cwd、--client 或 --trust-repository。\nWorkspace 目标支持精确 ID、@workspace:<suffix> 或精确标签；拒绝歧义。\n分支保留 refs/heads/ 前缀；全零 HEAD 表示未出生仓库。\nWindows --cwd 接受绝对、相对调用者目录和 ~ 路径；主目录优先级：USERPROFILE、HOMEDRIVE+HOMEPATH、HOME。\n";

const HELP_API_SNAPSHOT_EN: &str = "orchbee api snapshot [--format text|json]\n\nEmit the complete validated workbench.snapshot document inside the\norchbee.api.snapshot envelope. Successful output is JSON even under\n'--format text'; a failure envelope still requires '--format json'.\n";
const HELP_API_SNAPSHOT_ZH: &str = "orchbee api snapshot [--format text|json]\n\n在 orchbee.api.snapshot 封套内输出完整已验证 workbench.snapshot 文档。\n成功输出即使 '--format text' 也为 JSON；失败封套仍需显式 '--format json'。\n";

const HELP_API_SCHEMA_EN: &str = "orchbee api schema [<schema-id>] [--output <path>] [--format text|json]\n\nEmit the embedded JSON Schema dictionary for every implemented command, or\nthe single schema for <schema-id>. Successful output is always JSON; unknown\nIDs are not-found, and failure envelopes require '--format json'.\nOn Windows, --output writes identical JSON to a literal file path, replacing\nan existing regular file only after complete preparation. Parent directories\nmust exist. The command stays offline and also emits the JSON on stdout.\n";
const HELP_API_SCHEMA_ZH: &str = "orchbee api schema [<schema-id>] [--output <path>] [--format text|json]\n\n输出所有已实现命令的内嵌 JSON Schema 字典，或 <schema-id> 的单个 schema。\n成功输出始终为 JSON；未知 ID 为 not-found，失败封套需显式 '--format json'。\nWindows 的 --output 将相同 JSON 写入字面文件路径，完整准备后才替换已有普通文件。\n父目录必须存在；全程离线，stdout 同时输出该 JSON。\n";

const HELP_CONFIG_CHECK_EN: &str = "orchbee config check [--format text|json]\n\nRun the five local checks (runtime-base, runtime-directory, cohort, locale,\nsockets) and report pass/fail/not-applicable each. Exit 0 when all pass or\nare not-applicable, 1 when any check fails. Windows adds user-config: validate\nAPPDATA/Orchbee/config.toml, or the absolute ORCHBEE_CONFIG_FILE override.\nSupported TOML keys: terminal.new_cwd (follow/home/current/workspace/path),\nworktrees.directory (checkout root). Explicit creation arguments override\nvalid settings. A missing default file uses defaults; a missing explicit file fails.\n";
const HELP_CONFIG_CHECK_ZH: &str = "orchbee config check [--format text|json]\n\n运行五项本地检查（runtime-base、runtime-directory、cohort、locale、\nsockets）并逐项报告 pass/fail/not-applicable。全部通过或不适用时退出\n0，任一失败时退出 1。Windows 增加 user-config：校验\nAPPDATA/Orchbee/config.toml 或 ORCHBEE_CONFIG_FILE 指定的绝对文件路径。\n支持 TOML 键：terminal.new_cwd（follow/home/current/workspace/路径），\nworktrees.directory（检出根目录）。显式创建参数覆盖有效配置。\n默认文件不存在时使用默认值；显式文件不存在则失败。\n";

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
    pub(crate) reachable: &'static str,
    pub(crate) revision_label: &'static str,
    pub(crate) root: &'static str,
    pub(crate) site: &'static str,
    pub(crate) site_id: &'static str,
    pub(crate) slot_path: &'static str,
    pub(crate) right_click: &'static str,
    pub(crate) tab: &'static str,
    pub(crate) tab_id: &'static str,
    pub(crate) tabs: &'static str,
    pub(crate) tabs_word: &'static str,
    pub(crate) terminal: &'static str,
    pub(crate) terminal_context: &'static str,
    pub(crate) worker: &'static str,
    pub(crate) worker_unknown: &'static str,
    pub(crate) workspace: &'static str,
    pub(crate) workspaces: &'static str,
    pub(crate) workspaces_word: &'static str,
    pub(crate) project_headers: [&'static str; 4],
    pub(crate) directory_headers: [&'static str; 3],
    pub(crate) launch_profile_headers: [&'static str; 3],
    pub(crate) worktree_headers: [&'static str; 4],
    pub(crate) workspace_ref_headers: [&'static str; 2],
    pub(crate) workspace_headers: [&'static str; 8],
    pub(crate) tab_ref_headers: [&'static str; 2],
    pub(crate) tab_headers: [&'static str; 8],
    pub(crate) panel_headers: [&'static str; 9],
    pub(crate) presentation_headers: [&'static str; 4],
    pub(crate) terminal_headers: [&'static str; 6],
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
    reachable: "reachable",
    revision_label: "revision",
    root: "Root",
    site: "Site",
    site_id: "Site ID",
    slot_path: "Slot path",
    tab: "Tab",
    tab_id: "Tab ID",
    tabs: "Tabs",
    tabs_word: "tabs",
    terminal: "Terminal",
    terminal_context: "Context ID",
    worker: "Worker",
    worker_unknown: "unknown (not reported by the Workbench snapshot)",
    workspace: "Workspace",
    workspaces: "Workspaces",
    workspaces_word: "workspaces",
    project_headers: ["ID", "LABEL", "DIRECTORIES", "WORKSPACES"],
    directory_headers: ["ID", "WORKER", "PATH"],
    launch_profile_headers: ["PROFILE ID", "NAME", "LAUNCH"],
    worktree_headers: ["PATH", "HEAD", "BRANCH", "STATE"],
    workspace_ref_headers: ["ID", "LABEL"],
    number: "Number",
    workspace_headers: [
        "NUMBER",
        "ID",
        "LABEL",
        "TABS",
        "PANELS",
        "PROJECT",
        "PRIMARY DIRECTORY",
        "CONTEXT",
    ],
    tab_ref_headers: ["ID", "LABEL"],
    tab_headers: [
        "NUMBER",
        "ID",
        "LABEL",
        "PANELS",
        "PINNED",
        "GROUP",
        "CONTEXT",
        "CONTEXT ID",
    ],
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
    reachable: "可达",
    revision_label: "修订",
    root: "根节点",
    site: "Site",
    site_id: "Site ID",
    slot_path: "Slot 路径",
    tab: "Tab",
    tab_id: "Tab ID",
    tabs: "Tab",
    tabs_word: "标签",
    terminal: "终端",
    terminal_context: "上下文 ID",
    worker: "Worker",
    worker_unknown: "未知（Workbench 快照未上报）",
    workspace: "工作区",
    workspaces: "工作区",
    workspaces_word: "工作区",
    project_headers: ["ID", "LABEL", "目录数", "工作区数"],
    directory_headers: ["ID", "WORKER", "路径"],
    launch_profile_headers: ["ID", "名称", "启动"],
    worktree_headers: ["路径", "HEAD", "分支", "状态"],
    workspace_ref_headers: ["ID", "LABEL"],
    number: "编号",
    workspace_headers: [
        "编号",
        "ID",
        "LABEL",
        "标签数",
        "面板数",
        "PROJECT",
        "主目录",
        "上下文",
    ],
    tab_ref_headers: ["ID", "LABEL"],
    tab_headers: [
        "编号",
        "ID",
        "LABEL",
        "面板数",
        "置顶",
        "分组",
        "上下文",
        "上下文 ID",
    ],
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
}

const HELP_PANEL_CURRENT_EN: &str = "orchbee panel current [--panel <panel-id> | --current] [--client <id>] [--format text|json]\n\nRead a Panel, Tab and Workspace from a running Windows Site.\nAn explicit Panel ID or @panel:<suffix> ignores caller environment and needs no TUI.\nOtherwise use validated ORCHBEE_TERMINAL_ID and ORCHBEE_WORKER_ID. If both are absent,\nuse the only current TUI Client; multiple Clients require --client. --client ignores\ncaller environment and cannot combine with --panel. Partial, invalid or stale caller\nmetadata never falls back. The Client must be ready with a valid selected Panel.\nFive-second total deadline; Ctrl+C cancels. No focus changes or Site activation.\n";
const HELP_PANEL_CURRENT_ZH: &str = "orchbee panel current [--panel <panel-id> | --current] [--client <id>] [--format text|json]\n\n从运行中的 Windows Site 读取 Panel、Tab 和 Workspace。\n显式 Panel ID 或 @panel:<suffix> 忽略调用者环境，无需 TUI。\n否则校验 ORCHBEE_TERMINAL_ID 与 ORCHBEE_WORKER_ID；两个变量均缺失时，\n使用唯一 current TUI Client，多个 Client 必须用 --client 指定。--client 忽略\n调用者环境，不能与 --panel 同用。部分缺失、无效或过期的调用者信息绝不回退。\nClient 必须已就绪且所选 Panel 仍有效。总期限五秒；Ctrl+C 取消。\n不改变焦点，不激活 Site。\n";

impl Locale {
    #[cfg(any(target_os = "linux", windows))]
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
            (Self::EnUs, "prompt-stalled") => Some(
                "Input was accepted but no new Agent activity was observed. Inspect the terminal; do not blindly resend.",
            ),
            (Self::ZhCn, "prompt-stalled") => {
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

const HELP_PANEL_PROCESS_INFO_EN: &str = "orchbee panel process-info [<panel-id> | --panel <panel-id> | --current] [--client <id>] [--format text|json]\n\nRead the primary Terminal's Windows Job process information. Complete Panel IDs\nand @panel:<suffix> are accepted. Explicit Panel selectors are exclusive and conflict\nwith --client. Otherwise use validated caller context; only when both caller variables\nare absent, select the only current TUI Client. --client selects a Client and ignores\ncaller environment. Partial, invalid or stale caller metadata never falls back.\nThe Client must be ready with a valid selected Panel. Each sample reports owned\nprocesses with PID, creation time, image, command line and cwd; unavailable optional\nfields are null. Five-second total deadline; Ctrl+C cancels. No focus or input changes.\n";
const HELP_PANEL_PROCESS_INFO_ZH: &str = "orchbee panel process-info [<panel-id> | --panel <panel-id> | --current] [--client <id>] [--format text|json]\n\n读取主 Terminal 的 Windows Job 进程信息，接受完整 Panel ID 或 @panel:<suffix>。\n显式 Panel 选择器互斥，也不能与 --client 同用。否则使用经校验的调用者上下文；\n仅当两个调用者变量均缺失时，选择唯一 current TUI Client。--client 指定 Client\n并忽略调用者环境。部分缺失、无效或过期的调用者信息绝不回退。Client 必须已就绪\n且所选 Panel 仍有效。样本列出归属进程的 PID、创建时间、映像、命令行和 cwd；\n不可用的可选字段为 null。总期限五秒；Ctrl+C 取消。不改变焦点或输入。\n";

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
        let field = |key| {
            failure
                .details
                .as_ref()
                .and_then(|details| details.get(key))
                .and_then(|value| value.as_str())
                .unwrap_or("")
        };
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

const HELP_SITE_STOP_EN: &str = "orchbee site stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>] [--format text|json]\n\nStop the existing managed Windows Site and its terminals. This ends running\nprocesses after saving Workbench configuration for restart. Save failure keeps\nthe previous checkpoint and running terminals; resolve it and explicitly retry.\nAgent state, history and per-shell cwd are not saved. --confirm is required;\nthere is no interactive prompt and no Site activation. --detach returns the\nadmitted attempt immediately. Otherwise wait up to 65000 ms by default\n(1–3600000 ms). Cancellation/timeout stops waiting, not the accepted shutdown.\n\nA nonzero 32-character lowercase hexadecimal request ID is generated if omitted.\nDuplicate starts observe the existing attempt; --retry requires --request-id\nand explicitly retries only that failed stop. Use site stop-status <id> to\nrecover a result. Completion requires the matching stopped record and a free\nlifetime lock; pipe disappearance alone is insufficient. A later site start\nconsumes the receipt. Linux and macOS are unsupported.\n";
const HELP_SITE_STOP_ZH: &str = "orchbee site stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>] [--format text|json]\n\n停止现有托管 Windows Site 及其终端，先保存 Workbench 配置供重启恢复，再结束进程。\n保存失败保留旧检查点及运行中的终端；解决故障后显式重试。\n不保存 Agent 状态、终端历史或各 shell 实际 cwd。\n必须提供 --confirm；不弹出交互确认，也不激活 Site。--detach 立即返回已准入尝试；\n否则默认等待 65000 毫秒（范围 1–3600000）。取消或超时只停止等待，不取消已接受的停止。\n\n省略请求标识时自动生成非零 32 位小写十六进制标识。重复开始只观察现有尝试；\n--retry 必须搭配 --request-id，显式重试该失败停止。使用 site stop-status <id>\n恢复结果。仅准确匹配的已停止记录加空闲生命周期锁能证明完成，管道消失不够。\n后续 site start 会消费回执。Linux 和 macOS 尚不支持。\n";
const HELP_SITE_STOP_STATUS_EN: &str = "orchbee site stop-status <request-id> [--format text|json]\n\nRead a managed Windows stop attempt or its completed activation receipt.\nNever starts or stops a Site. One five-second deadline bounds the query.\nA later start consumes the completed receipt; not-recorded does not prove\nthat a previous stop failed. Use --format json for phase and failure details.\n";
const HELP_SITE_STOP_STATUS_ZH: &str = "orchbee site stop-status <request-id> [--format text|json]\n\n读取托管 Windows 停止尝试或其已完成激活回执，绝不启动或停止 Site。\n查询共用五秒截止时间。后续启动会消费完成回执，未记录不代表此前停止失败。\n使用 --format json 查看阶段和失败详情。\n";

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
            (Self::EnUs, "busy") => "another stop owns this Site",
            (Self::ZhCn, "busy") => "另一停止请求已占用此 Site",
            (Self::EnUs, "unmanaged") => "not a managed Site",
            (Self::ZhCn, "unmanaged") => "非托管 Site",
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
                "Site stop: {state}{waiting}\nRequest: {id}\nInspect: orchbee site stop-status {id} --format json\n"
            ),
            Self::ZhCn => format!(
                "Site 停止：{state}{waiting}\n请求：{id}\n检查：orchbee site stop-status {id} --format json\n"
            ),
        }
    }
}

const HELP_PANEL_SWAP_EN: &str = "Usage: orchbee panel swap [<source>] (--with <target> | --direction left|right|up|down) [--panel <source>|--current] [--client <id>] [--timeout <ms>] [--format text|json]\n\nExchange two complete Panel positions in one Tab, then focus the source in the selected Windows TUI. Source selectors are exclusive; omitted source uses valid caller context or the selected Client. Targets are exact IDs or @panel:<suffix>. Multiple Clients require an explicit selection.\n\nSame Panel, cross-Tab pair and no neighbor leave layout and focus unchanged. Positions commit before focus acknowledgment: a later failure preserves the committed receipt; never automatically retry a swap. One total timeout defaults to 30000 ms (1..3600000). No --detach or --request-id.\n";
const HELP_PANEL_SWAP_ZH: &str = "用法：orchbee panel swap [<source>] (--with <target> | --direction left|right|up|down) [--panel <source>|--current] [--client <id>] [--timeout <ms>] [--format text|json]\n\n交换同一 Tab 中两个完整 Panel 的位置，再在所选 Windows TUI 聚焦来源。来源选择器互斥；省略时使用有效调用者上下文或所选 Client。目标为精确 ID 或 @panel:<suffix>。多个 Client 时须明确选择。\n\n同一 Panel、跨 Tab 或没有邻居不改变布局和焦点。位置提交先于聚焦确认；后续失败保留提交回执，绝不自动重试交换。总超时默认 30000 ms（1..3600000）。不支持 --detach 或 --request-id。\n";

const HELP_PANEL_INPUT_EN: &str = "orchbee panel input (<panel-id> | --panel <panel-id> | --current) --right-click orchbee|panel [--format text|json]\n\nWindows: set the shared Panel right-button policy without changing focus.\n--current uses validated calling Terminal/Worker context; explicit targets\nbypass that context. orchbee reserves right-click to the Client; panel permits\nunmodified right clicks inside a reporting terminal. Existing controller and\nreadiness gates still apply. A policy change applies to the next gesture.\nconfigured or unchanged acknowledges canonical state, not every Client draw.\nAn unknown outcome is never automatically retried.\n";
const HELP_PANEL_INPUT_ZH: &str = "orchbee panel input (<panel-id> | --panel <panel-id> | --current) --right-click orchbee|panel [--format text|json]\n\nWindows：设置共享 Panel 右键策略，不改变焦点。--current 校验调用者\nTerminal/Worker 上下文；显式目标绕过该上下文。orchbee 将右键保留给\nClient；panel 允许无修饰键的右键在启用鼠标上报的终端内容中透传。\n现有控制权及就绪约束仍适用，策略改变作用于下一个手势。\nconfigured 或 unchanged 确认规范状态，不确认所有 Client 已绘制。\n未知结果不会自动重试。\n";

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

const HELP_AGENT_WAIT_WINDOWS_EN: &str = "orchbee agent wait <target> [--client <id>] [--until <condition>]... [--after-revision <n>] [--timeout <ms> | --forever] [--format text|json]\n\nWait on one pinned Agent, using current samples and retained change pages.\nWindows defaults to idle, done, blocked with no total timeout. --timeout\nsets 1-3600000 ms for the entire operation; --forever is an explicit synonym\nfor no total timeout and conflicts with --timeout. Each read is bounded to\nfive seconds. Ctrl+C ends the wait; no read failure reconnects.\nConditions: blocked, done, needs-attention, failed, succeeded, canceled,\ninterrupted, working, idle, unknown, ended, fresh, stale, disconnected.\nStatus conditions require fresh observations. blocked means needs-attention;\ndone means succeeded/failed/canceled/interrupted, without Client unseen state.\n--after-revision is a global fence, not a turn identity.\nTransient matching changes remain evidence even if the latest state differs.\nVersion 3 JSON separates matchedChange from the current session/revision;\nnull matchedChange means the current sample matched. Lost history or a\nforeign cursor fails explicitly. An ended session without a match is rejected.\n--client pins one exact TUI. Its current seen resting state means idle;\nunseen resting state means done. Missing/stale versions keep waiting.\nThese Client states are sampled; canonical idle/done events cannot prove\npast acknowledgements. Version 4 adds the Client projection.\nTarget: Session ID/name/Panel ID, --session <id>, --name <alias>, or\n--panel <id|@panel:suffix>. Resolution occurs once, before waiting.\n";

const HELP_AGENT_WAIT_WINDOWS_ZH: &str = "orchbee agent wait <target> [--client <id>] [--until <条件>]... [--after-revision <n>] [--timeout <毫秒> | --forever] [--format text|json]\n\n固定一个 Agent，结合当前样本和保留变更页等待。\nWindows 默认 idle、done、blocked，无总超时。--timeout 设置整个操作的\n1-3600000 毫秒期限；--forever 是无总超时的显式同义选项，与 --timeout 互斥。\n每次读取最多五秒；Ctrl+C 结束等待，读取失败不重新连接。\n条件：blocked、done、needs-attention、failed、succeeded、canceled、\ninterrupted、working、idle、unknown、ended、fresh、stale、disconnected。\n状态条件要求 fresh；blocked 表示 needs-attention；done 表示\nsucceeded/failed/canceled/interrupted，尚不合并 Client 未看见状态。\n--after-revision 是全局修订门槛，不代表回合身份。\n即使最新状态不同，短暂命中的变更仍作为证据返回。版本 3 JSON\n将 matchedChange 与当前 session/revision 分开；空 matchedChange 表示\n当前样本命中。历史丢失或游标跨启动周期时明确失败。\n会话已结束且没有条件命中时拒绝。\n--client 固定一个 TUI；其当前已看见的静止状态为 idle，未看见为 done。\n缺失或过期版本继续等待。Client 状态仅采样，规范 idle/done 事件\n不能证明过去的已读确认。版本 4 增加 Client 投影。\n目标：Session ID/名称/Panel ID、--session <id>、--name <alias> 或\n--panel <id|@panel:suffix>。等待前只解析一次。\n";

const HELP_TERMINAL_CONTROL_EN: &str = "orchbee terminal session control <terminal> [--takeover] [--cols N] [--rows N] [--timeout ms]\n\nWindows: acquire exclusive input/resize control of an existing terminal and\nstream validated ANSI/base64 NDJSON frames. Default geometry 120x40.\n--takeover explicitly revokes the old controller; failed takeover may still\nrevoke it. Read stdin as UTF-8 NDJSON: terminal.input (text XOR base64 bytes),\nterminal.resize (cols,rows), terminal.scroll (direction,lines), terminal.release.\nInput records are bounded to 2 MiB; decoded input to 256 KiB. Invalid commands\nproduce rejection records without effects; input is admitted in ordered 4 KiB\nchunks. Receipts do not mean shell execution. EOF/release detaches; it never\nstops the terminal. Cancellation 130, timeout 7; no reconnect or effect replay.\nOutput stalls/exchanges are bounded to 5s, cleanup 2s. Normal-screen scroll\nuses retained history; application mouse modes and alternate screens use\nthe corresponding input. See schema orchbee.terminal.control.input.\n";
const HELP_TERMINAL_CONTROL_ZH: &str = "orchbee terminal session control <terminal> [--takeover] [--cols N] [--rows N] [--timeout ms]\n\nWindows：获取既有终端的排他输入/尺寸控制权，持续输出经过验证的\nANSI/base64 NDJSON 帧。默认尺寸120x40。--takeover 显式撤销旧控制者；\n接管失败也可能撤销旧权限。stdin 为 UTF-8 NDJSON：terminal.input\n（text 或 base64 bytes 二选一）、terminal.resize（cols、rows）、\nterminal.scroll（direction、lines）、terminal.release。\n每条记录最多2MiB，解码输入最多256KiB。无效命令返回无副作用的拒绝记录；\n输入按顺序分成4KiB 提交，接收回执不代表 shell 已执行。EOF/release 仅\n解除附着，不停止终端。取消130、超时7；不重连或重放效果。\n传输/输出停滞最多5秒，清理2秒。普通屏幕滚动使用历史，应用鼠标模式和\n备用屏幕使用对应输入。详见 schema orchbee.terminal.control.input。\n";

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
const HELP_TERMINAL_ATTACH_EN: &str = "Usage: orchbee terminal attach <terminal> [--takeover] [--timeout ms] [--mouse on|off]\n\nWindows interactive terminal attachment. Requires terminal stdin/stdout.\nUses an existing terminal ID or @terminal:suffix; never starts a Site or focuses\na Workbench Panel. Uses the console size and exclusive input/resize control.\n--takeover revokes the old controller and may revoke it even on failure.\nCtrl+B then q detaches; Ctrl+B twice sends a literal Ctrl+B. Ctrl+C reaches\nthe application. Paste is semantic; plain page keys and wheel use history\nor the application's mouse/cursor modes. Mouse is on by default.\n--timeout is 1..3600000 ms; default unlimited, exchanges bounded to 5s, cleanup\nto 2s. No reconnect or input replay. Modes restore before diagnostics.\nOutput is an interactive screen with either --format; use terminal session\nobserve/control for NDJSON.\n";
const HELP_TERMINAL_ATTACH_ZH: &str = "用法：orchbee terminal attach <terminal> [--takeover] [--timeout ms] [--mouse on|off]\n\nWindows 交互终端附着，需要终端 stdin/stdout。使用既有终端 ID 或\n@terminal:suffix，不启动 Site，也不聚焦 Workbench Panel。采用控制台尺寸，\n取得独占输入/尺寸控制权。--takeover 撤销旧控制者；失败也可能撤销旧权限。\nCtrl+B 后按 q 脱离；连续两次 Ctrl+B 发送字面 Ctrl+B。Ctrl+C 交付应用。\n粘贴使用语义操作；普通翻页键和滚轮使用历史或应用鼠标/光标模式。\n默认启用鼠标。--timeout 为 1..3600000 毫秒，默认不限总时长；单次交换\n最多 5 秒、清理最多 2 秒。不重连或重放输入。输出诊断前恢复终端模式。\n两种 --format 均显示交互画面；NDJSON 请使用 terminal session observe/control。\n";
const HELP_AGENT_ATTACH_EN: &str = "Usage: orchbee agent attach <target> [--takeover] [--timeout ms] [--mouse on|off]\n\nResolve a unique live Agent terminal, then use the Windows terminal attach\ninterface. Target may be a name, session or Panel; --name, --session and\n--panel are explicit alternatives to the positional target. No focus or launch.\nCtrl+B then q detaches; Ctrl+C reaches the application. Console stdin/stdout\nare required. Default duration unlimited and mouse on. See help terminal attach\nfor takeover, timeout, cleanup and input semantics.\n";
const HELP_AGENT_ATTACH_ZH: &str = "用法：orchbee agent attach <target> [--takeover] [--timeout ms] [--mouse on|off]\n\n解析唯一的活动 Agent 终端，再进入 Windows terminal attach 界面。\n目标可为名称、会话或 Panel；--name、--session 和 --panel 可显式替代位置参数。\n不聚焦或启动资源。Ctrl+B 后按 q 脱离，Ctrl+C 交付应用；必须使用控制台\nstdin/stdout。默认不限总时长、启用鼠标。接管、超时、清理和输入语义见\nhelp terminal attach。\n";

const HELP_WORKSPACE_MOVE_EN: &str = r#"orchbee workspace move <target> --insert-index <N> [--format text|json]
orchbee workspace move-block <target>... [--before <target>] [--format text|json]

Reorder Workspaces globally on Windows, preserving Project ownership, running
terminals and every Client's selection. Targets are exact IDs, unique exact
labels or @workspace:<suffix>. Indices name insertion boundaries in the original
order (0 through the current Workspace count, at most 1023). The source boundary
and the next boundary leave the order unchanged. Blocks contain 1–1023 distinct
resolved Workspaces, use caller order, and append when --before is omitted.
The before-target must exist outside the block. All targets resolve before one
atomic command; invalid input has no effect. No-op preserves the revision.
Use workspace list to inspect order. Commands never retry an unknown outcome.
"#;
const HELP_WORKSPACE_MOVE_ZH: &str = r#"orchbee workspace move <target> --insert-index <N> [--format text|json]
orchbee workspace move-block <target>... [--before <target>] [--format text|json]

在 Windows 上全局重排 Workspace，保留 Project 归属、运行中的终端及各
Client 的选择。目标支持精确 ID、唯一精确标签或 @workspace:<suffix>。
索引是原顺序中的插入边界（0 到当前 Workspace 数量，最多 1023），来源
边界及其下一边界不改变顺序。块包含 1–1023 个不同的已解析 Workspace，
按调用者顺序移动；省略 --before 时追加到末尾。before 目标须存在且位于块外。
全部目标在一次原子提交前解析，无效输入不产生变更，无变化时保留修订。
使用 workspace list 查看顺序。结果未知时不会自动重试。
"#;

#[cfg(windows)]
const HELP_PANEL_MOVE_EN: &str = "orchbee panel move <panel-id> [destination] [--format text|json]\n\nDestinations:\n  --into-tab <tab> --direction above|below|left|right [--target-panel <panel-id>] [--ratio <number>]\n  --new-tab [--workspace <workspace> | --standalone] [--label <text>]\n  --new-workspace [--project <project>] [--label <text>] [--tab-label <text>]\n\nOptions: --focus (default) | --no-focus; --client <id>; --timeout <ms> (30000).\nAn existing Tab can belong to any Workspace. Its target defaults to the selected Client's focused Panel.\nNo Client: use --no-focus without --ratio; the first target leaf and default ratio are used.\nRatios are finite and clamped to 0.1..0.9. Zoomed or same-Tab relocation is unchanged.\nNew Workspace directory follows the terminal cwd, falling back to this process cwd when unavailable.\nMoving the last leaf removes the empty source Tab and, across contexts, its empty Workspace.\nThe terminal and process survive. Directory, ratio and focus acknowledgements are separate from the retained move.\nA partial failure keeps known committed facts; an uncertain submission is never replayed.\n\nLegacy same-Tab placement: --target-panel <panel-id> --direction above|below|left|right.\n";
#[cfg(windows)]
const HELP_PANEL_MOVE_ZH: &str = "orchbee panel move <panel-id> [目标] [--format text|json]\n\n目标：\n  --into-tab <tab> --direction above|below|left|right [--target-panel <panel-id>] [--ratio <number>]\n  --new-tab [--workspace <workspace> | --standalone] [--label <text>]\n  --new-workspace [--project <project>] [--label <text>] [--tab-label <text>]\n\n选项：--focus（默认）| --no-focus；--client <id>；--timeout <ms>（30000）。\n现有 Tab 可属于任意工作区；目标默认采用所选客户端记忆的焦点 Panel。\n没有客户端时使用不带 --ratio 的 --no-focus，采用首个目标叶子和默认比例。\n比例必须有限并钳制到 0.1..0.9。已缩放或迁入同 Tab 时保持不变。\n新工作区目录采用终端 cwd，缺失时回退到本进程 cwd。\n移走最后叶子会移除空来源 Tab；跨上下文时还会移除空来源工作区。\n终端和进程保留。目录、比例、焦点确认与迁移提交分别记录。\n部分失败保留已知提交事实；结果不确定的提交不会重放。\n\n既有同 Tab 放置：--target-panel <panel-id> --direction above|below|left|right。\n";

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

    pub(crate) fn cli_workbench_agent_status(self, status: Option<&str>) -> &'static str {
        match (self, status) {
            (Self::EnUs, Some("blocked")) => "needs attention",
            (Self::EnUs, Some("done")) => "done, unseen",
            (Self::EnUs, Some("working")) => "working",
            (Self::EnUs, Some("idle")) => "idle, seen",
            (Self::EnUs, None) => "read state unavailable",
            (Self::EnUs, _) => "unknown",
            (Self::ZhCn, Some("blocked")) => "需要处理",
            (Self::ZhCn, Some("done")) => "完成待查看",
            (Self::ZhCn, Some("working")) => "工作中",
            (Self::ZhCn, Some("idle")) => "空闲已读",
            (Self::ZhCn, None) => "已读状态不可用",
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
