//! Localized strings for the read-only CLI query surface. Machine envelope
//! fields stay English; these strings are the human stderr/text-render layer.

use super::Locale;
use crate::cli::Failure;
use crate::invocation::HelpTopic;

impl Locale {
    /// Localized human diagnostic for one CLI failure. User-controlled data is
    /// escaped before it reaches the terminal.
    pub(crate) fn cli_failure(self, failure: &Failure) -> String {
        use crate::cli::FailureKind;
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
            (Self::EnUs, ExpectedValue::Format) => "'text' or 'json'",
            (Self::ZhCn, ExpectedValue::Format) => "'text' 或 'json'",
            (Self::EnUs, ExpectedValue::Label) => "a nonempty label of at most 512 characters",
            (Self::ZhCn, ExpectedValue::Label) => "1–512 个字符的非空标签",
            (Self::EnUs, ExpectedValue::Dimension) => "a decimal integer from 0 to 65535",
            (Self::ZhCn, ExpectedValue::Dimension) => "0–65535 的十进制整数",
            (Self::EnUs, ExpectedValue::TabDirection) => "'previous', 'next', 'first', or 'last'",
            (Self::ZhCn, ExpectedValue::TabDirection) => "'previous'、'next'、'first' 或 'last'",
            (Self::EnUs, ExpectedValue::PanelDirection) => "'above', 'below', 'left', or 'right'",
            (Self::ZhCn, ExpectedValue::PanelDirection) => "'above'、'below'、'left' 或 'right'",
            (Self::EnUs, ExpectedValue::Shell) => "'bash', 'zsh', or 'fish'",
            (Self::ZhCn, ExpectedValue::Shell) => "'bash'、'zsh' 或 'fish'",
            (Self::EnUs, ExpectedValue::HistoryRows) => "a decimal integer from 1 to 1000",
            (Self::ZhCn, ExpectedValue::HistoryRows) => "1–1000 的十进制整数",
            (Self::EnUs, ExpectedValue::DirectoryPath) => {
                "a literal absolute path of at most 4,096 UTF-8 bytes"
            }
            (Self::ZhCn, ExpectedValue::DirectoryPath) => "不超过 4,096 UTF-8 字节的字面绝对路径",
            (Self::EnUs, ExpectedValue::MatchText) => "nonempty literal text of at most 4096 bytes",
            (Self::ZhCn, ExpectedValue::MatchText) => "至多 4096 字节的非空字面文本",
            (Self::EnUs, ExpectedValue::RegexPattern) => {
                "a nonempty regular expression of at most 1024 bytes"
            }
            (Self::ZhCn, ExpectedValue::RegexPattern) => "至多 1024 字节的非空正则表达式",
            (Self::EnUs, ExpectedValue::TailLines) => "a decimal integer from 1 to 65535",
            (Self::ZhCn, ExpectedValue::TailLines) => "1–65535 的十进制整数",
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
            (Self::EnUs, ExpectedValue::LaunchArguments) => {
                "at most 128 arguments totaling at most 2,048 UTF-8 bytes with '--exec'"
            }
            (Self::ZhCn, ExpectedValue::LaunchArguments) => {
                "配合 '--exec' 使用，至多 128 个参数且合计不超过 2,048 UTF-8 字节"
            }
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
                "one of the twelve 'agent wait --until' conditions"
            }
            (Self::ZhCn, ExpectedValue::AgentWaitUntil) => {
                "'agent wait --until' 的十二个条件名之一"
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
            UsageReason::MissingTabContext { context: ctx } => (
                match self {
                    Self::EnUs => format!(
                        "'{}' requires '--workspace' or '--standalone'.",
                        context(*ctx)
                    ),
                    Self::ZhCn => format!(
                        "'{}' 需要 '--workspace' 或 '--standalone'。",
                        context(*ctx)
                    ),
                },
                hint(*ctx),
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
        let stage = failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        match (self, stage) {
            (Self::EnUs, "platform") => {
                "The native CLI query surface is supported on Windows and Linux.\n".to_owned()
            }
            (Self::ZhCn, "platform") => {
                "原生 CLI 查询面在 Windows 和 Linux 上受支持。\n".to_owned()
            }
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
                "site start is supported on Windows and Linux.\n".to_owned()
            }
            (Self::ZhCn, "platform") => "site start 在 Windows 和 Linux 上受支持。\n".to_owned(),
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
    #[cfg(unix)]
    pub(crate) fn cli_integration_capsule_line(self, summary: &str) -> String {
        match self {
            Self::EnUs => format!("Adapter capsule: {summary}\n"),
            Self::ZhCn => format!("适配器胶囊：{summary}\n"),
        }
    }

    /// `agent integration explain` header — the fixed-field first lines.
    /// Vocabulary values stay in their canonical English spelling.
    #[cfg(unix)]
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

    #[cfg(unix)]
    pub(crate) const fn cli_integration_plan_header(self) -> &'static str {
        match self {
            Self::EnUs => "Plan:\n",
            Self::ZhCn => "计划：\n",
        }
    }

    /// `explain` when every pre-effect gate refuses the vector.
    #[cfg(unix)]
    pub(crate) const fn cli_integration_plan_empty(self) -> &'static str {
        match self {
            Self::EnUs => "Plan: no steps — the observed state admits no enable vector.\n",
            Self::ZhCn => "计划：无步骤——当前状态不接受任何启用向量。\n",
        }
    }

    /// Mutation completed with the goal state observed.
    #[cfg(unix)]
    pub(crate) fn cli_integration_completed(self, direction: &str, provider: &str) -> String {
        match (self, direction) {
            (Self::EnUs, "enable") => format!("Enabled {provider}.\n"),
            (Self::EnUs, _) => format!("Disabled {provider}.\n"),
            (Self::ZhCn, "enable") => format!("已启用 {provider}。\n"),
            (Self::ZhCn, _) => format!("已禁用 {provider}。\n"),
        }
    }

    /// A pre-effect refusal — the canonical refusal code stays English.
    #[cfg(unix)]
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
    #[cfg(unix)]
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
    #[cfg(unix)]
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
    #[cfg(unix)]
    pub(crate) fn cli_integration_cancelled(self, direction: &str, provider: &str) -> String {
        match (self, direction) {
            (Self::EnUs, "enable") => format!("Enable of {provider} cancelled.\n"),
            (Self::EnUs, _) => format!("Disable of {provider} cancelled.\n"),
            (Self::ZhCn, "enable") => format!("已取消启用 {provider}。\n"),
            (Self::ZhCn, _) => format!("已取消禁用 {provider}。\n"),
        }
    }

    /// The `attempted` list's leading word in text output.
    #[cfg(unix)]
    pub(crate) const fn cli_integration_attempted_word(self) -> &'static str {
        match self {
            Self::EnUs => "attempted",
            Self::ZhCn => "已尝试",
        }
    }

    /// Header above the residue listing — retained paths are named, never
    /// silently swept.
    #[cfg(unix)]
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
    #[cfg(unix)]
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
    #[cfg(unix)]
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
            (_, HelpTopic::Family(Family::Panel)) => match self {
                Self::EnUs => HELP_PANEL_EN,
                Self::ZhCn => HELP_PANEL_ZH,
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
            (Self::EnUs, CommandPath::Tui) => HELP_TUI_EN,
            (Self::ZhCn, CommandPath::Tui) => HELP_TUI_ZH,
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
            (Self::EnUs, CommandPath::TabRename) => HELP_TAB_RENAME_EN,
            (Self::ZhCn, CommandPath::TabRename) => HELP_TAB_RENAME_ZH,
            (Self::EnUs, CommandPath::TabMove) => HELP_TAB_MOVE_EN,
            (Self::ZhCn, CommandPath::TabMove) => HELP_TAB_MOVE_ZH,
            (Self::EnUs, CommandPath::TabClose) => HELP_TAB_CLOSE_EN,
            (Self::ZhCn, CommandPath::TabClose) => HELP_TAB_CLOSE_ZH,
            (Self::EnUs, CommandPath::PanelList) => HELP_PANEL_LIST_EN,
            (Self::ZhCn, CommandPath::PanelList) => HELP_PANEL_LIST_ZH,
            (Self::EnUs, CommandPath::PanelGet) => HELP_PANEL_GET_EN,
            (Self::ZhCn, CommandPath::PanelGet) => HELP_PANEL_GET_ZH,
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
            (Self::EnUs, CommandPath::TerminalRead) => HELP_TERMINAL_READ_EN,
            (Self::ZhCn, CommandPath::TerminalRead) => HELP_TERMINAL_READ_ZH,
            (Self::EnUs, CommandPath::TerminalWaitOutput) => HELP_TERMINAL_WAIT_OUTPUT_EN,
            (Self::ZhCn, CommandPath::TerminalWaitOutput) => HELP_TERMINAL_WAIT_OUTPUT_ZH,
            (Self::EnUs, CommandPath::TerminalSend) => HELP_TERMINAL_SEND_EN,
            (Self::ZhCn, CommandPath::TerminalSend) => HELP_TERMINAL_SEND_ZH,
            (Self::EnUs, CommandPath::WorktreeList) => HELP_WORKTREE_LIST_EN,
            (Self::ZhCn, CommandPath::WorktreeList) => HELP_WORKTREE_LIST_ZH,
            (Self::EnUs, CommandPath::LaunchProfileList) => HELP_LAUNCH_PROFILE_LIST_EN,
            (Self::ZhCn, CommandPath::LaunchProfileList) => HELP_LAUNCH_PROFILE_LIST_ZH,
            (Self::EnUs, CommandPath::LaunchProfileGet) => HELP_LAUNCH_PROFILE_GET_EN,
            (Self::ZhCn, CommandPath::LaunchProfileGet) => HELP_LAUNCH_PROFILE_GET_ZH,
            (Self::EnUs, CommandPath::LaunchProfileCreate) => HELP_LAUNCH_PROFILE_CREATE_EN,
            (Self::ZhCn, CommandPath::LaunchProfileCreate) => HELP_LAUNCH_PROFILE_CREATE_ZH,
            (Self::EnUs, CommandPath::LaunchProfileDelete) => HELP_LAUNCH_PROFILE_DELETE_EN,
            (Self::ZhCn, CommandPath::LaunchProfileDelete) => HELP_LAUNCH_PROFILE_DELETE_ZH,
            (Self::EnUs, CommandPath::AgentList) => HELP_AGENT_LIST_EN,
            (Self::ZhCn, CommandPath::AgentList) => HELP_AGENT_LIST_ZH,
            (Self::EnUs, CommandPath::AgentGet) => HELP_AGENT_GET_EN,
            (Self::ZhCn, CommandPath::AgentGet) => HELP_AGENT_GET_ZH,
            (Self::EnUs, CommandPath::AgentExplain) => HELP_AGENT_EXPLAIN_EN,
            (Self::ZhCn, CommandPath::AgentExplain) => HELP_AGENT_EXPLAIN_ZH,
            (Self::EnUs, CommandPath::AgentWait) => HELP_AGENT_WAIT_EN,
            (Self::ZhCn, CommandPath::AgentWait) => HELP_AGENT_WAIT_ZH,
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
            (Self::EnUs, CommandPath::SiteStart) => HELP_SITE_START_EN,
            (Self::ZhCn, CommandPath::SiteStart) => HELP_SITE_START_ZH,
            (Self::EnUs, CommandPath::SiteDiagnose) => HELP_SITE_DIAGNOSE_EN,
            (Self::ZhCn, CommandPath::SiteDiagnose) => HELP_SITE_DIAGNOSE_ZH,
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
            (Self::EnUs, _) => "note: unrecognized advisory.\n",
            (Self::ZhCn, _) => "注：未识别的通告。\n",
        }
    }
}

const ROOT_HELP_EN: &str = "Orchbee\n\nUsage:\n  orchbee\n  orchbee tui [--mouse on|off]\n  orchbee status [--format text|json]\n  orchbee <family> <command> [operands] [--format text|json]\n  orchbee api snapshot|schema\n  orchbee site start [--format text|json]\n  orchbee site diagnose [--format text|json]\n  orchbee config check [--format text|json]\n  orchbee help [<family>|<command>]\n  orchbee completion bash|zsh|fish\n  orchbee -h|--help\n  orchbee --version\n\nModes and queries:\n  orchbee              Open the local terminal (Windows/Linux development cohort)\n  orchbee tui          Open the same local terminal explicitly\n  orchbee status       Read the running local Site's Workbench snapshot\n  orchbee project      list | get\n  orchbee workspace    list | get\n  orchbee tab          list | get\n  orchbee panel        list | get\n  orchbee terminal     list | get | read | wait-output | send\n  orchbee worktree     list\n  orchbee launch-profile list | get\n  orchbee agent        list | get | explain | doctor | wait\n  orchbee agent integration\n                       list | explain | enable | disable — local provider\n                       onboarding; never contacts a Site\n  orchbee api          Machine snapshot and embedded schema documents\n\nSite lifecycle:\n  orchbee site start   Ensure the local Site is running (no TTY needed)\n  orchbee site diagnose\n                       Inspect the local Site runtime — read-only facts\n\nMutations (one command per invocation, against the running Site):\n  orchbee project      create | rename\n  orchbee workspace    create | rename | accept-directory\n  orchbee tab          create | rename | move | close\n  orchbee panel        split | move | close\n  orchbee terminal     send\n  orchbee launch-profile create | delete\n\nInspection and help:\n  orchbee config check Inspect local runtime/cohort/locale/socket inputs\n  orchbee help         Show help for a family or complete command path\n  orchbee completion   Emit a static shell completion script\n  orchbee -h|--help    Show this help; after a family or command path, show\n                       that path's help\n  orchbee --version    Show the version\n\nRun 'orchbee help <family>' for filters and target rules, or\n'orchbee help <family> <command>' for one mutation's full syntax.\n";
const ROOT_HELP_ZH: &str = "Orchbee\n\n用法：\n  orchbee\n  orchbee tui [--mouse on|off]\n  orchbee status [--format text|json]\n  orchbee <family> <command> [操作数] [--format text|json]\n  orchbee api snapshot|schema\n  orchbee site start [--format text|json]\n  orchbee site diagnose [--format text|json]\n  orchbee config check [--format text|json]\n  orchbee help [<family>|<command>]\n  orchbee completion bash|zsh|fish\n  orchbee -h|--help\n  orchbee --version\n\n模式与查询：\n  orchbee              打开本地终端（Windows/Linux 开发 cohort）\n  orchbee tui          显式打开同一本地终端\n  orchbee status       读取正在运行的本地 Site 的 Workbench 快照\n  orchbee project      list | get\n  orchbee workspace    list | get\n  orchbee tab          list | get\n  orchbee panel        list | get\n  orchbee terminal     list | get | read | wait-output | send\n  orchbee worktree     list\n  orchbee launch-profile list | get\n  orchbee agent        list | get | explain | doctor | wait\n  orchbee agent integration\n                       list | explain | enable | disable——本地 provider\n                       接入；绝不接触 Site\n  orchbee api          机器快照与内嵌 schema 文档\n\nSite 生命周期：\n  orchbee site start   确保本地 Site 正在运行（无需 TTY）\n  orchbee site diagnose\n                       检查本地 Site 运行时——只读事实\n\n修改命令（每次调用执行一条命令，作用于运行中的 Site）：\n  orchbee project      create | rename\n  orchbee workspace    create | rename | accept-directory\n  orchbee tab          create | rename | move | close\n  orchbee panel        split | move | close\n  orchbee terminal     send\n  orchbee launch-profile create | delete\n\n检查与帮助：\n  orchbee config check 检查本地运行时/cohort/locale/套接字输入\n  orchbee help         显示某个 family 或完整命令路径的帮助\n  orchbee completion   输出静态 shell 补全脚本\n  orchbee -h|--help    显示此帮助；跟在 family 或命令路径后时显示该路径的帮助\n  orchbee --version    显示版本\n\n运行 'orchbee help <family>' 查看过滤参数与 target 规则，或\n'orchbee help <family> <command>' 查看单条修改命令的完整语法。\n";

const HELP_TUI_EN: &str = "orchbee tui [--mouse on|off]\n\nOpen the local terminal interface (Windows/Linux development cohort).\nA bare 'orchbee' invocation does the same.\n\n  --mouse on|off   Host pointer capture for pane hit-testing; on by\n                   default. Only the explicit 'tui' route accepts it.\n";
const HELP_TUI_ZH: &str = "orchbee tui [--mouse on|off]\n\n打开本地终端界面（Windows/Linux 开发 cohort）。\n裸 'orchbee' 调用效果相同。\n\n  --mouse on|off   宿主机指针捕获，用于面板命中；默认开启。\n                   只有显式 'tui' 路由接受该选项。\n";

const HELP_STATUS_EN: &str = "orchbee status\n\nRead a live Workbench snapshot from the running local Site and report\nreachability plus entity counts. The Site is never created, activated, or\nmutated. '--format json' emits the orchbee.status envelope.\n";
const HELP_STATUS_ZH: &str = "orchbee status\n\n从正在运行的本地 Site 读取实时 Workbench 快照并报告可达性与实体计数。\n绝不创建、激活或修改 Site。'--format json' 输出 orchbee.status 封套。\n";

const HELP_PROJECT_EN: &str = "orchbee project list|get|create|rename\n\nQuery and mutate Project resources in the running local Site.\n  project list                       List every Project\n  project get <target>               Show one Project by exact ID, @project:<suffix>, or exact label\n  project create --label <l>         Create one Project\n  project rename <target> --label <l>  Rename one Project\nTargets are case-sensitive; ambiguous labels or suffixes are rejected, never guessed.\nEvery mutation reads one snapshot, then submits exactly one command.\n";
const HELP_PROJECT_ZH: &str = "orchbee project list|get|create|rename\n\n查询并修改本地 Site 中的 Project 资源。\n  project list                       列出所有 Project\n  project get <target>               按精确 ID、@project:<suffix> 或精确 label 显示一个 Project\n  project create --label <l>         创建一个 Project\n  project rename <target> --label <l>  重命名一个 Project\ntarget 区分大小写；歧义 label 或后缀会被拒绝而非猜测。\n每个修改命令先读取一次快照，然后只提交一个命令。\n";

const HELP_WORKSPACE_EN: &str = "orchbee workspace list|get|create|rename|accept-directory|close|close-status|close-resume\n\nQuery and mutate Workspace resources in the running local Site.\n  workspace list [--project <t>]                    List Workspaces, optionally inside one Project\n  workspace get <target>                            Show one Workspace by exact ID, @workspace:<suffix>, or exact label\n  workspace create --project <t> --label <l>        Create one Workspace inside a Project\n  workspace rename <target> --label <l>             Rename one Workspace\n  workspace accept-directory <target> --path <p>    Accept a literal absolute path as the Workspace's primary directory\n  workspace close <target> --confirm                Close every terminal the Workspace's Clients use\n  workspace close-status --project <id> --workspace <id>\n                                                    Read the retained close operation by stable IDs\n  workspace close-resume --project <id> --workspace <id> --request <close-id> --generation <n> --confirm\n                                                    Open one fresh attempt for a resumable close\nEvery mutation reads one snapshot, then submits exactly one command.\n";
const HELP_WORKSPACE_ZH: &str = "orchbee workspace list|get|create|rename|accept-directory|close|close-status|close-resume\n\n查询并修改本地 Site 中的 Workspace 资源。\n  workspace list [--project <t>]                    列出 Workspace，可按某个 Project 过滤\n  workspace get <target>                            按精确 ID、@workspace:<suffix> 或精确 label 显示一个 Workspace\n  workspace create --project <t> --label <l>        在某个 Project 内创建一个 Workspace\n  workspace rename <target> --label <l>             重命名一个 Workspace\n  workspace accept-directory <target> --path <p>    将一个字面绝对路径接受为 Workspace 的主目录\n  workspace close <target> --confirm                关闭该 Workspace 的 Clients 使用的全部终端\n  workspace close-status --project <id> --workspace <id>\n                                                    按稳定 ID 读取保留的关闭操作\n  workspace close-resume --project <id> --workspace <id> --request <close-id> --generation <n> --confirm\n                                                    为可恢复的关闭开启一次新的尝试\n每个修改命令先读取一次快照，然后只提交一个命令。\n";

const HELP_TAB_EN: &str = "orchbee tab list|get|create|rename|move|close\n\nQuery and mutate Tab resources in the running local Site.\n  tab list [--workspace <t> | --standalone]  List Tabs, optionally filtered\n  tab get <target>                           Show one Tab by exact ID, @tab:<suffix>, or label\n  tab create (--workspace <t>|--standalone) --label <l> [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...]\n                                             Create one Tab in one context\n  tab rename <target> --label <l>            Rename one Tab\n  tab move <target> --direction <d>          Move one Tab within its strip\n  tab close <target> --confirm               Close one Tab\n'--workspace' and '--standalone' are mutually exclusive single-use selectors.\n";
const HELP_TAB_ZH: &str = "orchbee tab list|get|create|rename|move|close\n\n查询并修改本地 Site 中的 Tab 资源。\n  tab list [--workspace <t> | --standalone]  列出 Tab，可按 Workspace 或独立上下文过滤\n  tab get <target>                           按精确 ID、@tab:<suffix> 或 label 显示一个 Tab\n  tab create (--workspace <t>|--standalone) --label <l> [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...]\n                                             在某个上下文内创建一个 Tab\n  tab rename <target> --label <l>            重命名一个 Tab\n  tab move <target> --direction <d>          在标签条内移动一个 Tab\n  tab close <target> --confirm               关闭一个 Tab\n'--workspace' 与 '--standalone' 互斥且各只能用一次。\n";

const HELP_PANEL_EN: &str = "orchbee panel list|get|split|close|move\n\nQuery and mutate Panel resources in the running local Site.\n  panel list [--tab <t>]                   List Panels, optionally inside one Tab\n  panel get <panel-id>                     Show one Panel by its complete typed ID or @panel:<suffix>\n  panel split <panel-id> --direction <d> --label <l> [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...]\n                                           Split one Panel, creating a sibling\n  panel close <panel-id> --confirm         Close one Panel\n  panel move <panel-id> --target-panel <panel-id> --direction <d>\n                                           Move one Panel next to a sibling\nPanels have no independent label; complete 32-hex IDs and @panel:<suffix>\nselectors are accepted.\n";
const HELP_PANEL_ZH: &str = "orchbee panel list|get|split|close|move\n\n查询并修改本地 Site 中的 Panel 资源。\n  panel list [--tab <t>]                   列出 Panel，可按某个 Tab 过滤\n  panel get <panel-id>                     按完整类型化 ID 或 @panel:<suffix> 显示一个 Panel\n  panel split <panel-id> --direction <d> --label <l> [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...]\n                                           分裂一个 Panel，创建一个相邻 Panel\n  panel close <panel-id> --confirm         关闭一个 Panel\n  panel move <panel-id> --target-panel <panel-id> --direction <d>\n                                           将一个 Panel 移动到某个相邻 Panel 旁\nPanel 没有独立 label；接受完整 32 位小写十六进制 ID 与 @panel:<suffix> 选择器。\n";

const HELP_TERMINAL_EN: &str = "orchbee terminal list|get|read|wait-output|send\n\nQuery terminals bound by Workbench Presentations in the running local Site,\nor send one input record to one of them.\n  terminal list                List every bound terminal\n  terminal get <terminal-id>   Show one terminal by ID or @terminal:<suffix>\n  terminal read <terminal-id> [--history --rows <n>] [--unwrap] [--lines <n>]\n                               Read the terminal's visible content; the paired\n                               flags read retained history rows instead\n  terminal wait-output <terminal-id> --match <text> | --regex <pattern>\n                               Wait for text in the terminal's retained output\n  terminal send <terminal-id>  Send text or one key to the terminal's input\nTerminal targets accept a complete typed ID or an @terminal:<suffix> selector.\nA terminal invisible to any Presentation binding cannot be listed, read,\nwaited on, or sent to; a binding proves structure only, not that a process\nis alive.\n";
const HELP_TERMINAL_ZH: &str = "orchbee terminal list|get|read|wait-output|send\n\n查询本地 Site 中被 Workbench Presentation 绑定的终端，或向其中一个终端发送一条输入。\n  terminal list                列出所有已绑定终端\n  terminal get <terminal-id>   按完整类型化 ID 或 @terminal:<suffix> 显示一个终端\n  terminal read <terminal-id> [--history --rows <n>] [--unwrap] [--lines <n>]\n                               读取终端当前可见内容；成对 flag 时改为读取\n                               保留历史行\n  terminal wait-output <terminal-id> --match <text> | --regex <pattern>\n                               等待终端保留输出中出现匹配内容\n  terminal send <terminal-id>  向终端输入发送文本或一个按键\n未被任何 Presentation 引用的终端不可见、不可读、不可等待也不可发送；绑定只证明结构，不证明进程存活。\n";

const HELP_SITE_EN: &str = "orchbee site start|diagnose\n\nManage the implicit per-user local Site's lifecycle.\n  site start     Ensure the local Site is running, activating it without a\n                TTY when needed\n  site diagnose  Inspect the local Site runtime's finite paths and locks —\n                read-only facts, never a repair or a process claim\n";
const HELP_SITE_ZH: &str = "orchbee site start|diagnose\n\n管理隐式的每用户本地 Site 生命周期。\n  site start     确保本地 Site 正在运行，必要时在无 TTY 环境下激活\n  site diagnose  检查本地 Site 运行时的有限路径与锁——只读事实，\n                绝不修复，也不对进程下断言\n";

const HELP_SITE_START_EN: &str = "orchbee site start [--format text|json]\n\nEnsure the implicit per-user local Site is running. When no admitted\nSite answers, this invocation performs the existing activation sequence\n— the same locks, marker, Server spawn, and readiness admission the TUI\nstartup uses — and reports 'started'. When a running Site admits the\nreadiness probe, it reports 'already-running'. One absolute 20-second\nbudget covers the whole invocation.\n\n'--format json' emits the orchbee.site.start envelope. An interrupted\nor timed-out start may leave activation state behind; a later invocation\nthen answers refused with a recovery-required reason. The command never\nrepairs, stops, or deletes runtime state.\n";
const HELP_SITE_START_ZH: &str = "orchbee site start [--format text|json]\n\n确保隐式的每用户本地 Site 正在运行。若没有已准入的 Site 应答，本次\n调用执行既有的激活序列——与 TUI 启动相同的锁、标记、Server 拉起\n与就绪准入——并报告 'started'。若运行中的 Site 准入了就绪探针，则\n报告 'already-running'。整个调用共享一个 20 秒绝对预算。\n\n'--format json' 输出 orchbee.site.start 封套。被中断或超时的启动\n可能遗留激活状态；后续调用将以拒绝加 recovery-required 原因应答。\n本命令绝不修复、停止或删除运行时状态。\n";

const HELP_AGENT_EN: &str = "orchbee agent list|get|explain|doctor|wait|integration\n\nObserve Agent sessions tracked by the running local Site, or diagnose\nprovider hooks offline.\n  agent list [--include-ended] [--limit <n>] [--cursor <c>]\n                                List Agent sessions (live only unless\n                                --include-ended; 1-100 per page, default 50)\n  agent get <session-id>        Show one Agent session by its opaque identity\n  agent explain <session-id>    Explain how one session's status and freshness\n                                were derived\n  agent doctor [--provider <id>]... [--asset-root <path>]...\n                                Explain, from bounded filesystem facts alone,\n                                why Agent observations may not arrive\n  agent wait <session-id> [--until <condition>]...\n                                Poll until a condition matches or the\n                                deadline lands\n  agent integration <command>   Local provider onboarding: list | explain |\n                                enable | disable — 'orchbee help agent\n                                integration' has the full sub-family\nA session id is the opaque session identity, never a label; a cursor is the\nserver-minted 'v1.<payload>.<tag>' token from a previous page. The session\nreads and 'doctor' above create, activate, or mutate nothing; 'agent\nintegration' manages local provider files instead and never contacts a\nSite.\n";
const HELP_AGENT_ZH: &str = "orchbee agent list|get|explain|doctor|wait|integration\n\n观察运行中的本地 Site 跟踪的 Agent 会话，或离线诊断 provider 钩子。\n  agent list [--include-ended] [--limit <n>] [--cursor <c>]\n                                列出 Agent 会话（默认仅存活会话；每页\n                                1-100 条，默认 50）\n  agent get <session-id>        按不透明会话身份显示一个 Agent 会话\n  agent explain <session-id>    解释一个会话的状态与新鲜度推导依据\n  agent doctor [--provider <id>]... [--asset-root <path>]...\n                                仅凭有界文件系统事实解释 Agent 观察为何\n                                可能未到达\n  agent wait <session-id> [--until <条件>]...\n                                轮询直到条件命中或截止到达\n  agent integration <命令>      本地 provider 接入：list | explain |\n                                enable | disable——完整子命令族见\n                                'orchbee help agent integration'\n会话 ID 是不透明会话身份而非 label；cursor 是上一页返回的服务端铸造\n'v1.<payload>.<tag>' 令牌。上述会话读取与 'doctor' 不创建、不激活、\n不修改任何状态；'agent integration' 改为管理本地 provider 文件，且\n绝不接触 Site。\n";

const HELP_AGENT_LIST_EN: &str = "orchbee agent list [--include-ended] [--limit <n>] [--cursor <c>] [--format text|json]\n\nList Agent sessions observed by the running local Site. '--include-ended'\nadds ended sessions; '--limit' selects 1-100 rows per page (default 50);\n'--cursor' continues from a previous page's server-minted nextCursor.\n'--format json' emits the orchbee.agent.list envelope.\n";
const HELP_AGENT_LIST_ZH: &str = "orchbee agent list [--include-ended] [--limit <n>] [--cursor <c>] [--format text|json]\n\n列出运行中的本地 Site 观察到的 Agent 会话。'--include-ended' 包含已结束\n会话；'--limit' 每页 1-100 条（默认 50）；'--cursor' 从上一页服务端铸造\n的 nextCursor 继续。'--format json' 输出 orchbee.agent.list 封套。\n";

const HELP_AGENT_GET_EN: &str = "orchbee agent get <session-id> [--format text|json]\n\nShow one Agent session by its opaque session identity: lifecycle, semantic\nstatus, observation freshness, location, capabilities, pending requests,\nand presentation attachments. '--format json' emits the\norchbee.agent.get envelope.\n";
const HELP_AGENT_GET_ZH: &str = "orchbee agent get <session-id> [--format text|json]\n\n按不透明会话身份显示一个 Agent 会话：生命周期、语义状态、观察新鲜度、\n位置、能力、待处理请求与 Presentation 附着。'--format json' 输出\norchbee.agent.get 封套。\n";

const HELP_AGENT_EXPLAIN_EN: &str = "orchbee agent explain <session-id> [--format text|json]\n\nExplain one Agent session's derived view: the canonical status basis,\nfreshness meaning, capability ceiling, observation source and fidelity,\nand the navigate-only action boundary. '--format json' emits the\norchbee.agent.explain envelope.\n";
const HELP_AGENT_EXPLAIN_ZH: &str = "orchbee agent explain <session-id> [--format text|json]\n\n解释一个 Agent 会话的推导视图：规范状态依据、新鲜度含义、能力上限、\n观察来源与保真度，以及仅限导航的操作边界。'--format json' 输出\norchbee.agent.explain 封套。\n";

const HELP_AGENT_WAIT_EN: &str = "orchbee agent wait <session-id> [--until <condition>]... [--after-revision <n>] [--timeout <ms>] [--format text|json]\n\nPoll one Agent session on a held Client Session until a requested\ncondition matches, the revision fence refuses, the session answer is\nterminal, or the total deadline lands. '--until' is repeatable and takes\none of: needs-attention, failed, succeeded, canceled, interrupted,\nworking, idle, unknown, ended, fresh, stale, disconnected (default:\nneeds-attention failed succeeded canceled interrupted idle ended).\nStatus conditions require fresh freshness; the three freshness\nconditions match the observation directly. '--after-revision' is a\nglobal-revision fence, not a turn identity; '--timeout' bounds the whole\nwait at 1-3600000 ms (default 600000). Polling samples current state: a\ntransition that begins and ends between two completed observations is\nmissed. '--format json' emits the orchbee.agent.wait envelope.\n";
const HELP_AGENT_WAIT_ZH: &str = "orchbee agent wait <session-id> [--until <条件>]... [--after-revision <n>] [--timeout <毫秒>] [--format text|json]\n\n在持有的 Client Session 上轮询一个 Agent 会话，直到请求的条件命中、\n修订栅栏拒绝、会话应答终止，或总截止到达。'--until' 可重复，取值为：\nneeds-attention、failed、succeeded、canceled、interrupted、working、\nidle、unknown、ended、fresh、stale、disconnected（默认：\nneeds-attention failed succeeded canceled interrupted idle ended）。\n状态条件要求 fresh 新鲜度；三个新鲜度条件直接匹配观察。\n'--after-revision' 是全局修订栅栏而非回合身份；'--timeout' 将整个\n等待界于 1-3600000 毫秒（默认 600000）。轮询采样当前状态：在两次\n已完成观察之间开始又结束的跃迁会被错过。'--format json' 输出\norchbee.agent.wait 封套。\n";
const HELP_SITE_DIAGNOSE_EN: &str = "orchbee site diagnose [--format text|json]\n\nReport bounded facts about the implicit per-user local Site runtime —\nits base and scope directory, the activation marker and locks, and the\nfinite client/worker/pending/observation/host endpoint paths. Every row\nis a pathname or descriptor fact at its own observation instant: 'held'\nmeans some owner held the lifetime lock then — never that a Site or any\nprocess is alive — and 'absent' is a pathname fact, never proof that an\nexecution ended. A 'known' marker or a valid client/worker socket leaf\nadds a conditional advisory: if no existing Site can be joined, fresh\nactivation is blocked by current recovery policy.\n\nThe command never connects to an endpoint, never creates or deletes a\npath, never acquires the activation lock, and never repairs, stops, or\npredicts a later start. A cooperative five-second budget is checked\nbetween bounded filesystem operations; rows it cannot reach close as\n'uninspectable'. '--format json' emits the orchbee.site.diagnose\nenvelope.\n";
const HELP_SITE_DIAGNOSE_ZH: &str = "orchbee site diagnose [--format text|json]\n\n报告隐式每用户本地 Site 运行时的有界事实——基目录与 scope 目录、\n激活标记与锁，以及有限的 client/worker/pending/observation/host 端点\n路径。每一行都是其各自观察瞬间的路径名或描述符事实：'held' 表示\n某个属主在那一刻持有生命周期锁——绝不表示某个 Site 或进程存活；\n'absent' 是路径名事实，绝不证明某个执行已结束。观察到 'known' 标记\n或合法的 client/worker 套接字叶时附加一条条件通告：若无可加入的\n现有 Site，按当前恢复策略新的激活将被阻止。\n\n本命令绝不连接任何端点，绝不创建或删除路径，绝不获取激活锁，也\n绝不修复、停止或预测后续启动。协作式五秒预算在有界的文件系统操作\n之间检查；无法到达的行以 'uninspectable' 闭合。'--format json'\n输出 orchbee.site.diagnose 封套。\n";
const HELP_AGENT_DOCTOR_EN: &str = "orchbee agent doctor [--provider <id>]... [--asset-root <path>]... [--format text|json]\n\nExplain, from bounded filesystem facts alone, why Agent observations may\nnot arrive: each provider's executable and managed-hook state, declared\nasset roots, the observation endpoint, and the environment override.\n'--provider' selects canonical provider ids (repeatable); '--asset-root'\ndeclares absolute capsule roots to inspect (repeatable, at most 4).\nNothing is executed, connected, written, or repaired. '--format json'\nemits the orchbee.agent.doctor envelope.\n";
const HELP_AGENT_DOCTOR_ZH: &str = "orchbee agent doctor [--provider <id>]... [--asset-root <path>]... [--format text|json]\n\n仅凭有界文件系统事实解释 Agent 观察为何可能未到达：各 provider 的\n可执行文件与托管 hook 状态、声明的 asset root、观察端点以及环境覆盖。\n'--provider' 选择规范 provider ID（可重复）；'--asset-root' 声明要检查\n的绝对 capsule root（可重复，至多 4 个）。不执行、不连接、不写入、\n不修复任何内容。'--format json' 输出 orchbee.agent.doctor 封套。\n";

const HELP_AGENT_INTEGRATION_EN: &str = "orchbee agent integration list|explain|enable|disable\n\nManage the local onboarding of supported agent providers to the Orchbee\nobservation transport. These commands run entirely on the invoking user's\nmachine and PATH — they never contact a Site.\n  agent integration list [--provider <id>]\n                                Inspect every supported provider's local state\n  agent integration explain --provider <id>\n                                Show one provider's state and the plan enable\n                                would run\n  agent integration enable --provider <id>\n                                Install and enable the managed observation\n                                integration\n  agent integration disable --provider <id>\n                                Remove the managed integration\nlist and explain execute each provider's own bounded read commands where\nrequired; enable and disable additionally run bounded, non-interactive\nmutation steps. Provider-internal bookkeeping cannot be guaranteed absent.\n";
const HELP_AGENT_INTEGRATION_ZH: &str = "orchbee agent integration list|explain|enable|disable\n\n管理受支持的 agent provider 到 Orchbee 观测传输的本地接入。这些命令完全\n在调用用户的机器与 PATH 上运行——不接触 Site。\n  agent integration list [--provider <id>]\n                                检查每个受支持 provider 的本地状态\n  agent integration explain --provider <id>\n                                显示一个 provider 的状态及 enable 将执行的\n                                计划\n  agent integration enable --provider <id>\n                                安装并启用受管观测集成\n  agent integration disable --provider <id>\n                                移除受管集成\nlist 与 explain 在需要时会执行各 provider 自身的有界只读命令；enable 与\ndisable 还会执行有界、非交互的变更步骤。provider 内部记账无法保证不\n发生。\n";

const HELP_AGENT_INTEGRATION_LIST_EN: &str = "orchbee agent integration list [--provider <id>] [--adapter-source <abs>] [--format text|json]\n\nInspect every supported provider's local integration state — or only the\none selected by '--provider'. Reads run the providers' own bounded\ncommands from the ambient PATH where required; nothing is written.\n'--adapter-source' overrides the cohort adapter payload for development\nand tests. '--format json' emits the orchbee.agent.integrations envelope.\n";
const HELP_AGENT_INTEGRATION_LIST_ZH: &str = "orchbee agent integration list [--provider <id>] [--adapter-source <abs>] [--format text|json]\n\n检查每个受支持 provider 的本地集成状态，或仅检查 '--provider' 所选的一\n个。需要时会执行 provider 自身来自环境 PATH 的有界读命令；不写入任何\n内容。'--adapter-source' 为开发与测试覆盖 cohort adapter 载荷。\n'--format json' 输出 orchbee.agent.integrations 封套。\n";

const HELP_AGENT_INTEGRATION_EXPLAIN_EN: &str = "orchbee agent integration explain --provider <id> [--adapter-source <abs>] [--format text|json]\n\nExplain one provider's local integration state and the guard-evaluated\nplan 'enable' would run for the observed dimensions. Reads execute the\nprovider's own bounded commands from the ambient PATH where required;\nnothing is written. '--format json' emits the\norchbee.agent.integration.plan envelope.\n";
const HELP_AGENT_INTEGRATION_EXPLAIN_ZH: &str = "orchbee agent integration explain --provider <id> [--adapter-source <abs>] [--format text|json]\n\n解释一个 provider 的本地集成状态，以及针对观测到的维度 'enable' 将执行\n的、经守卫求值的计划。需要时会执行 provider 自身来自环境 PATH 的有界\n读命令；不写入任何内容。'--format json' 输出\norchbee.agent.integration.plan 封套。\n";

const HELP_AGENT_INTEGRATION_ENABLE_EN: &str = "orchbee agent integration enable --provider <id> [--adapter-source <abs>] [--format text|json]\n\nEnable one provider's managed observation integration locally: publish\nthe adapter capsule when needed, then run the provider's own bounded,\nnon-interactive setup commands. Refuses before any effect when the entry\nis foreign, newer-versioned, or manual-only; reports the post-effect\nstate observed by re-inspection. '--format json' emits the\norchbee.agent.integration.mutation envelope.\n";
const HELP_AGENT_INTEGRATION_ENABLE_ZH: &str = "orchbee agent integration enable --provider <id> [--adapter-source <abs>] [--format text|json]\n\n在本地启用一个 provider 的受管观测集成：必要时先发布 adapter capsule，\n然后执行 provider 自身的有界、非交互设置命令。当条目为外来、更新版本\n或仅手动管理时，在任何效应前拒绝；并以复检视测到的效应后状态报告。\n'--format json' 输出 orchbee.agent.integration.mutation 封套。\n";

const HELP_AGENT_INTEGRATION_DISABLE_EN: &str = "orchbee agent integration disable --provider <id> [--adapter-source <abs>] [--format text|json]\n\nDisable one provider's managed observation integration locally: run the\nprovider's own bounded removal commands or guarded removal of the owned\nentry. Only entries verifiably owned by Orchbee are ever removed; foreign\nor newer-versioned entries are refused before any effect. '--format json'\nemits the orchbee.agent.integration.mutation envelope.\n";
const HELP_AGENT_INTEGRATION_DISABLE_ZH: &str = "orchbee agent integration disable --provider <id> [--adapter-source <abs>] [--format text|json]\n\n在本地禁用一个 provider 的受管观测集成：执行 provider 自身的有界移除\n命令，或对所有物执行受保护的移除。只有经核实归 Orchbee 所有的条目\n才会被移除；外来或更新版本的条目在任何效应前被拒绝。'--format json'\n输出 orchbee.agent.integration.mutation 封套。\n";

const HELP_API_EN: &str = "orchbee api snapshot|schema\n\nMachine introspection of the local Workbench surface.\n  api snapshot           Emit the complete workbench.snapshot document\n  api schema [<id>]      List embedded JSON Schemas, or one schema by ID\nSuccessful output is always JSON, whatever '--format' was selected; a\nfailure envelope still requires an explicit '--format json'.\n";
const HELP_API_ZH: &str = "orchbee api snapshot|schema\n\n本地 Workbench 面的机器自省。\n  api snapshot           输出完整 workbench.snapshot 文档\n  api schema [<id>]      列出内嵌 JSON Schema，或按 ID 取单个 schema\n成功输出无论 '--format' 选择如何都是 JSON；失败封套仍需显式\n'--format json'。\n";

const HELP_CONFIG_EN: &str = "orchbee config check\n\nInspect local inputs only: runtime base, Site runtime directory, current\ncohort integrity, effective locale, and socket presence plus the socket\naddress budget. Nothing is connected, started, created, or locked. Any\nfailed check exits 1.\n";
const HELP_CONFIG_ZH: &str = "orchbee config check\n\n仅检查本地输入：运行时基座、Site 运行时目录、当前 cohort 完整性、生效\nlocale、套接字存在性与套接字地址长度预算。不连接、不启动、不创建、\n不加锁。任一检查失败以 1 退出。\n";

const HELP_COMPLETION_EN: &str = "orchbee completion bash|zsh|fish\n\nEmit a static completion script for the implemented grammar to stdout.\nNothing is installed and no user files are written.\n";
const HELP_COMPLETION_ZH: &str = "orchbee completion bash|zsh|fish\n\n向标准输出生成已实现语法的静态补全脚本。不安装、不写入用户文件。\n";

const HELP_PROJECT_LIST_EN: &str = "orchbee project list [--format text|json]\n\nList every Project in the running local Site: id, label, directory and\nworkspace counts. An empty Site prints just the table header.\n";
const HELP_PROJECT_LIST_ZH: &str = "orchbee project list [--format text|json]\n\n列出本地 Site 中所有 Project：ID、label、目录与工作区计数。空 Site 仅\n输出表头。\n";

const HELP_PROJECT_GET_EN: &str = "orchbee project get <target> [--format text|json]\n\nShow one Project: its directories (with worker ids and paths) and its\nworkspaces. The target is an exact 32-hex ID, an @project:<suffix>\nselector, or an exact case-sensitive label; ambiguous labels or suffixes\nreport candidates instead of guessing.\n";
const HELP_PROJECT_GET_ZH: &str = "orchbee project get <target> [--format text|json]\n\n显示一个 Project：其目录（含 worker id 与路径）与工作区。target 为精确\n32 位十六进制 ID、@project:<suffix> 选择器或精确区分大小写的 label；\n歧义 label 或后缀报告候选而非猜测。\n";

const HELP_PROJECT_CREATE_EN: &str = "orchbee project create --label <label> [--format text|json]\n\nCreate one Project with the given label. The command reads one snapshot,\nthen submits exactly one command under a fresh request id. The result\ncarries the committed revision and the Server-allocated project id.\nA new Project has no directory or Workspace, and creating one touches no\nfilesystem path. Labels may repeat across Projects; address the new\nProject by its returned id.\n";
const HELP_PROJECT_CREATE_ZH: &str = "orchbee project create --label <label> [--format text|json]\n\n以给定 label 创建一个 Project。命令先读取一次快照，然后以全新请求 ID\n只提交一个命令。结果携带已提交修订与 Server 分配的 project id。\n新 Project 无目录、无 Workspace，创建不触碰任何文件系统路径。\nlabel 允许跨 Project 重复；请按返回的 ID 寻址新 Project。\n";

const HELP_PROJECT_RENAME_EN: &str = "orchbee project rename <target> --label <label> [--format text|json]\n\nRename one Project selected by exact ID, @project:<suffix>, or exact\nlabel. The command\nreads one snapshot, then submits exactly one command. A committed result\ncarries the new revision; an unchanged one means the label already\nmatched.\n";
const HELP_PROJECT_RENAME_ZH: &str = "orchbee project rename <target> --label <label> [--format text|json]\n\n重命名一个由精确 ID、@project:<suffix> 或精确 label 选择的 Project。命令先读取一次快\n照，然后只提交一个命令。committed 结果携带新修订；unchanged 表示\nlabel 本已相同。\n";

const HELP_WORKSPACE_LIST_EN: &str = "orchbee workspace list [--project <t>] [--format text|json]\n\nList Workspaces across Projects, or only inside the Project selected by\n--project (exact ID, @project:<suffix>, or exact label). Fields: ids,\nlabel, primary directory\n(null means none), and terminal context.\n";
const HELP_WORKSPACE_LIST_ZH: &str = "orchbee workspace list [--project <t>] [--format text|json]\n\n跨 Project 列出 Workspace，或仅列 --project（精确 ID、@project:<suffix> 或精确 label）所选\nProject 内的。字段：各 ID、label、主目录（null 表示无）与终端上下文。\n";

const HELP_WORKSPACE_GET_EN: &str = "orchbee workspace get <target> [--format text|json]\n\nShow one Workspace plus the Tabs that live in its terminal context.\n";
const HELP_WORKSPACE_GET_ZH: &str = "orchbee workspace get <target> [--format text|json]\n\n显示一个 Workspace 及其终端上下文中存在的 Tab。\n";

const HELP_WORKSPACE_CREATE_EN: &str = "orchbee workspace create --project <target> --label <label> [--format text|json]\n\nCreate one Workspace inside the Project selected by --project (exact ID,\n@project:<suffix>, or exact label). The command reads one snapshot,\nresolves the target, then\nsubmits exactly one command under a fresh request id. The result carries\nthe committed revision and the new workspace and terminal context ids.\n";
const HELP_WORKSPACE_CREATE_ZH: &str = "orchbee workspace create --project <target> --label <label> [--format text|json]\n\n在 --project（精确 ID、@project:<suffix> 或精确 label）所选 Project 内创建一个 Workspace。\n命令先读取一次快照、解析目标，然后以全新请求 ID 只提交一个命令。结果\n携带已提交修订以及新的 workspace 与终端上下文 ID。\n";

const HELP_WORKSPACE_RENAME_EN: &str = "orchbee workspace rename <target> --label <label> [--format text|json]\n\nRename one Workspace selected by exact ID, @workspace:<suffix>, or exact\nlabel. The command\nreads one snapshot, then submits exactly one command. A committed result\ncarries the new revision; an unchanged one means the label already\nmatched.\n";
const HELP_WORKSPACE_RENAME_ZH: &str = "orchbee workspace rename <target> --label <label> [--format text|json]\n\n重命名一个由精确 ID、@workspace:<suffix> 或精确 label 选择的 Workspace。命令先读取一次快\n照，然后只提交一个命令。committed 结果携带新修订；unchanged 表示\nlabel 本已相同。\n";
const HELP_WORKSPACE_ACCEPT_DIRECTORY_EN: &str = "orchbee workspace accept-directory <target> --path <path> [--format text|json]\n\nAccept a literal absolute local-platform path as the primary directory of the\nWorkspace selected by exact ID, @workspace:<suffix>, or exact label.\n'--path' is required: a\nliteral absolute path of at most 4,096 UTF-8 bytes without NUL — never\nexpanded ('~', '$'), created, or probed by the CLI. The command reads one\nsnapshot, resolves the target and its owning Project, then submits\nexactly one command under a fresh request id; the Server authoritatively\nprobes the path and rejects a Workspace that already has a directory.\nA committed result carries the new revision, the Server-allocated\ndirectory and worker ids, the resolved project and workspace ids, and the\nrequest's literal path.\n";
const HELP_WORKSPACE_ACCEPT_DIRECTORY_ZH: &str = "orchbee workspace accept-directory <target> --path <path> [--format text|json]\n\n将一个字面本机绝对路径接受为由精确 ID、@workspace:<suffix> 或精确 label 选择的\nWorkspace 的主目录。'--path' 为必填：字面绝对路径，至多 4,096 个\nUTF-8 字节且不含 NUL——CLI 不展开（'~'、'$'）、不创建、不探测。命\n令先读取一次快照、解析目标及其所属 Project，然后以全新请求 ID 只\n提交一个命令；由 Server 权威探测该路径，并拒绝已有目录的\nWorkspace。committed 结果携带新修订、Server 分配的 directory 与\nworker ID、已解析的 project 与 workspace ID，以及请求的字面路径。\n";

const HELP_WORKSPACE_CLOSE_EN: &str = "orchbee workspace close <target> --confirm [--format text|json]\n\nClose every terminal the Workspace's Clients use, selected by exact ID,\n@workspace:<suffix>, or exact label. '--confirm' is required. The\nWorkspace itself, its checkout, branch, and directories are preserved —\nonly the terminal membership in its context closes. An accepted result\ncarries the accepted revision, the close request id, and the resolved\nproject, workspace, and terminal context ids; the Site finishes the\nclose asynchronously and re-running this command is not a resume — use\n'workspace close-resume' for a retained resumable operation.\n";
const HELP_WORKSPACE_CLOSE_ZH: &str = "orchbee workspace close <target> --confirm [--format text|json]\n\n关闭由精确 ID、@workspace:<suffix> 或精确 label 选择的 Workspace 中\nClients 使用的全部终端。'--confirm' 为必填。Workspace 本身及其\ncheckout、branch、目录全部保留——仅关闭其上下文中的终端成员。\naccepted 结果携带已接受修订、关闭请求 ID，以及解析出的 project、\nworkspace 与终端上下文 ID；Site 异步完成关闭，重复执行本命令不等于\n恢复——对保留的可恢复操作请使用 'workspace close-resume'。\n";

const HELP_WORKSPACE_CLOSE_STATUS_EN: &str = "orchbee workspace close-status --project <id> --workspace <id> [--format text|json]\n\nRead the retained Workspace Close operation for one scope, selected by\nstrict stable IDs: '--project' and '--workspace' are complete nonzero\n32-lowercase-hex Workbench IDs, never labels or suffixes. The record\noutlives the Workspace's active projection, so a completed close still\nanswers with its closed tombstone revision. An available result carries\nthe operation identity, attempt generation, scope status, aggregate\ncounts, and every captured member's lifecycle truth; not-closing means\nno retained operation covers the scope. '--format json' emits the\norchbee.workspace.close-status envelope.\n";
const HELP_WORKSPACE_CLOSE_STATUS_ZH: &str = "orchbee workspace close-status --project <id> --workspace <id> [--format text|json]\n\n按严格稳定 ID 读取某个作用域保留的 Workspace 关闭操作：'--project' 与\n'--workspace' 为完整的非零 32 位小写十六进制 Workbench ID，绝不接受\nlabel 或后缀。该记录在 Workspace 离开活动投影后仍然保留，因此已完成\n的关闭仍以其 closed 墓碑修订应答。available 结果携带操作身份、尝试\n代次、作用域状态、聚合计数与每个被捕获成员的生命周期实况；\nnot-closing 表示没有保留操作覆盖该作用域。'--format json' 输出\norchbee.workspace.close-status 封套。\n";

const HELP_WORKSPACE_CLOSE_RESUME_EN: &str = "orchbee workspace close-resume --project <id> --workspace <id> --request <close-id> --generation <n> --confirm [--format text|json]\n\nOpen one fresh attempt window for a retained, resumable Workspace Close.\nAll selectors are strict stable IDs: '--project' and '--workspace' are\ncomplete 32-lowercase-hex Workbench IDs, '--request' is the close\noperation's own request id from 'workspace close' or 'close-status', and\n'--generation' is the nonzero attempt generation last observed — an\noutdated fence answers stale, never a second attempt. '--confirm' is\nrequired. Only resumable lifecycle states resume; unknown outcomes are\nnon-resumable. A resumed result carries the operation identity and the\nnew attempt generation.\n";
const HELP_WORKSPACE_CLOSE_RESUME_ZH: &str = "orchbee workspace close-resume --project <id> --workspace <id> --request <close-id> --generation <n> --confirm [--format text|json]\n\n为一个保留的、可恢复的 Workspace 关闭操作开启一次新的尝试窗口。所有\n选择器均为严格稳定 ID：'--project' 与 '--workspace' 为完整 32 位小写\n十六进制 Workbench ID，'--request' 为来自 'workspace close' 或\n'close-status' 的关闭操作自身请求 ID，'--generation' 为最近观察到的\n非零尝试代次——过期栅栏以 stale 应答，绝不发起第二次尝试。\n'--confirm' 为必填。只有可恢复的生命周期状态可恢复；unknown 结果不可\n恢复。resumed 结果携带操作身份与新的尝试代次。\n";

const HELP_TAB_LIST_EN: &str = "orchbee tab list [--workspace <t> | --standalone] [--format text|json]\n\nList Tabs. Filters select one Workspace's context (exact ID,\n@workspace:<suffix>, or label) or\nthe standalone context. Fields: ids, label, pinned flag, group, and owning\ncontext kind.\n";
const HELP_TAB_LIST_ZH: &str = "orchbee tab list [--workspace <t> | --standalone] [--format text|json]\n\n列出 Tab。过滤参数可选择一个 Workspace 的上下文（精确 ID、@workspace:<suffix> 或 label）或\n独立上下文。字段：各 ID、label、置顶标记、分组与所属上下文类型。\n";

const HELP_TAB_GET_EN: &str = "orchbee tab get <target> [--format text|json]\n\nShow one Tab including its complete PanelNode root subtree.\n";
const HELP_TAB_GET_ZH: &str = "orchbee tab get <target> [--format text|json]\n\n显示一个 Tab，包括其完整 PanelNode 根子树。\n";

const HELP_TAB_CREATE_EN: &str = "orchbee tab create (--workspace <target> | --standalone) --label <label> [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--format text|json]\n\nCreate one Tab in the selected terminal context: a Workspace chosen by\nexact ID, @workspace:<suffix>, or exact label, or the standalone context.\nThe four geometry\nflags are an all-or-none group: with all four absent the command\nrequests an 80x24 logical grid with unknown cell pixels (0,0) — a\nrequest policy, not a measured display fact; supplying any of the four\nrequires all four. The result carries the committed revision, the new\ntab, panel, and presentation ids, and the terminal spawn outcome.\n'--exec' selects the launched program by absolute path and each '--arg'\nappends one verbatim argument; omitting both launches the default shell.\n'--profile' instead selects a stored Launch Profile by exact ID or\nsingly-matching name, resolved by the Server — it cannot combine with\n'--exec' or '--arg'.\n";
const HELP_TAB_CREATE_ZH: &str = "orchbee tab create (--workspace <target> | --standalone) --label <label> [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--format text|json]\n\n在所选终端上下文内创建一个 Tab：由精确 ID、@workspace:<suffix> 或精确 label 选择的\nWorkspace，或独立上下文。四个几何 flag 为全有或全无组：全部省略时\n请求 80x24 逻辑网格且单元像素为未知 (0,0)——这是请求策略而非实测\n显示事实；提供其中任何一个就必须提供全部四个。结果携带已提交修\n订、新的 tab、panel、presentation ID 及终端拉起结果。\n'--exec' 以绝对路径选择启动程序，每个 '--arg' 逐字追加一个参数；\n两者均省略时启动默认 shell。\n'--profile' 改为按精确 ID 或唯一匹配的名称选择已存储的启动配置，\n由 Server 解析——不能与 '--exec' 或 '--arg' 组合。\n";

const HELP_TAB_RENAME_EN: &str = "orchbee tab rename <target> --label <label> [--format text|json]\n\nRename one Tab selected by exact ID, @tab:<suffix>, or exact label.\nA committed result\ncarries the new revision; unchanged means the label already matched.\n";
const HELP_TAB_RENAME_ZH: &str = "orchbee tab rename <target> --label <label> [--format text|json]\n\n重命名一个由精确 ID、@tab:<suffix> 或精确 label 选择的 Tab。committed 结果携带新修\n订；unchanged 表示 label 本已相同。\n";

const HELP_TAB_MOVE_EN: &str = "orchbee tab move <target> --direction <previous|next|first|last> [--format text|json]\n\nMove one Tab within its strip. A committed result carries the new\nrevision; unchanged means the tab was already in position.\n";
const HELP_TAB_MOVE_ZH: &str = "orchbee tab move <target> --direction <previous|next|first|last> [--format text|json]\n\n在标签条内移动一个 Tab。committed 结果携带新修订；unchanged 表示 Tab\n本已在目标位置。\n";

const HELP_TAB_CLOSE_EN: &str = "orchbee tab close <target> --confirm [--format text|json]\n\nClose one Tab selected by exact ID, @tab:<suffix>, or exact label.\n'--confirm' is\nrequired. An accepted result carries the accepted revision and the close\nrequest id; the Site finishes the close asynchronously.\n";
const HELP_TAB_CLOSE_ZH: &str = "orchbee tab close <target> --confirm [--format text|json]\n\n关闭一个由精确 ID、@tab:<suffix> 或精确 label 选择的 Tab。'--confirm' 为必填。\naccepted 结果携带已接受修订与关闭请求 ID；Site 异步完成关闭。\n";

const HELP_PANEL_LIST_EN: &str = "orchbee panel list [--tab <t>] [--format text|json]\n\nList Panels, optionally inside the Tab selected by --tab (exact ID,\n@tab:<suffix>, or label). Panels carry no label; presentation labels are\nnot panel names.\n";
const HELP_PANEL_LIST_ZH: &str = "orchbee panel list [--tab <t>] [--format text|json]\n\n列出 Panel，或仅列 --tab（精确 ID、@tab:<suffix> 或 label）所选 Tab 内的。Panel 无\nlabel；Presentation 的 label 不是 Panel 名。\n";

const HELP_PANEL_GET_EN: &str = "orchbee panel get <panel-id> [--format text|json]\n\nShow one Panel by complete typed ID or @panel:<suffix>: owning tab,\ncontext, slot path, and its primary and child Presentations.\n";
const HELP_PANEL_GET_ZH: &str = "orchbee panel get <panel-id> [--format text|json]\n\n按完整类型化 ID 或 @panel:<suffix> 显示一个 Panel：所属 Tab、上下文、slot 路径及其主与\n子 Presentation。\n";

const HELP_PANEL_SPLIT_EN: &str = "orchbee panel split <panel-id> --direction <above|below|left|right> --label <label> [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--format text|json]\n\nSplit one Panel, creating a sibling panel on the selected side. The\nfour geometry flags are an all-or-none group: with all four absent the\ncommand requests an 80x24 logical grid with unknown cell pixels (0,0) —\na request policy, not a measured display fact; supplying any of the\nfour requires all four. The result carries the committed revision,\nthe new panel, split, slot, and presentation ids, and the terminal spawn\noutcome. '--exec' selects the launched program by absolute path and each\n'--arg' appends one verbatim argument; omitting both launches the\ndefault shell. '--profile' instead selects a stored Launch Profile by\nexact ID or singly-matching name, resolved by the Server — it cannot\ncombine with '--exec' or '--arg'.\n";
const HELP_PANEL_SPLIT_ZH: &str = "orchbee panel split <panel-id> --direction <above|below|left|right> --label <label> [--cols <n> --rows <n> --cell-width <px> --cell-height <px>] [--profile <s> | --exec <abs> [--arg <v>]...] [--format text|json]\n\n分裂一个 Panel，在所选侧创建相邻 Panel。四个几何 flag 为全有或全无\n组：全部省略时请求 80x24 逻辑网格且单元像素为未知 (0,0)——这是请\n求策略而非实测显示事实；提供其中任何一个就必须提供全部四个。\n结果携带已提交修订、新的 panel、split、slot、presentation ID 及\n终端拉起结果。'--exec' 以绝对路径选择启动程序，每个 '--arg' 逐字追加一个\n参数；两者均省略时启动默认 shell。'--profile' 改为按精确 ID 或唯一匹\n配的名称选择已存储的启动配置，由 Server 解析——不能与 '--exec'\n或 '--arg' 组合。\n";

const HELP_PANEL_CLOSE_EN: &str = "orchbee panel close <panel-id> --confirm [--format text|json]\n\nClose one Panel selected by complete typed ID or @panel:<suffix>.\n'--confirm' is required.\nAn accepted result carries the accepted revision and the close request\nid; the Site finishes the close asynchronously.\n";
const HELP_PANEL_CLOSE_ZH: &str = "orchbee panel close <panel-id> --confirm [--format text|json]\n\n关闭一个由完整类型化 ID 或 @panel:<suffix> 选择的 Panel。'--confirm' 为必填。accepted\n结果携带已接受修订与关闭请求 ID；Site 异步完成关闭。\n";

const HELP_PANEL_MOVE_EN: &str = "orchbee panel move <panel-id> (--target-panel <panel-id> | --into-tab <tab> | --new-tab) [--format text|json]\n\nMove one leaf Panel to one exclusive destination:\n  --target-panel <panel-id> --direction <above|below|left|right>\n        Next to a sibling panel inside the same tab (panel.move).\n        Both panels accept complete typed IDs or @panel:<suffix> selectors.\n  --into-tab <tab> --direction <above|below|left|right>\n        Into an existing sibling Tab of the same TerminalContext,\n        resolved by exact ID or label; ambiguous labels are rejected.\n  --new-tab --label <l>\n        Into a fresh Tab appended to the same TerminalContext.\n\nOnly a leaf Panel relocates: a stacked Panel rejects panel-not-leaf, and\nthe source Tab's last leaf rejects last-panel-in-tab — an emptied Tab is\nnever retired implicitly; close it explicitly. Relocation keeps the\nPanel's identity, Presentation, terminal binding, and process: nothing\nrelaunches and the working directory is unchanged. A committed result\ncarries the new revision; unchanged means the layout already matched.\n";
const HELP_PANEL_MOVE_ZH: &str = "orchbee panel move <panel-id> (--target-panel <panel-id> | --into-tab <tab> | --new-tab) [--format text|json]\n\n将一个叶子 Panel 移动到恰好一种目的地：\n  --target-panel <panel-id> --direction <above|below|left|right>\n        移到同一 Tab 内某个相邻 Panel 的所选侧（panel.move）。\n        两个 Panel 均接受完整类型化 ID 或 @panel:<suffix> 选择器。\n  --into-tab <tab> --direction <above|below|left|right>\n        移入同一 TerminalContext 中已存在的兄弟 Tab，按精确 ID 或\n        label 解析；歧义 label 会被拒绝。\n  --new-tab --label <l>\n        移入追加到同一 TerminalContext 的新建 Tab。\n\n只有叶子 Panel 可迁移：堆叠 Panel 以 panel-not-leaf 拒绝，源 Tab 的\n最后一片叶子以 last-panel-in-tab 拒绝——空 Tab 绝不被隐式回收，需\n显式关闭。迁移保留 Panel 身份、Presentation、终端绑定与进程：不重新\n启动，工作目录不变。committed 结果携带新修订；unchanged 表示布局本已相同。\n";

const HELP_TERMINAL_LIST_EN: &str = "orchbee terminal list [--format text|json]\n\nList every terminal bound by a Presentation. Unbound terminals are not\nvisible; a binding is structural evidence, not proof a process is alive.\n";
const HELP_TERMINAL_LIST_ZH: &str = "orchbee terminal list [--format text|json]\n\n列出所有被 Presentation 绑定的终端。未绑定终端不可见；绑定是结构证据，\n不是进程存活的证明。\n";

const HELP_TERMINAL_GET_EN: &str = "orchbee terminal get <terminal-id> [--format text|json]\n\nShow one bound terminal by complete typed ID or @terminal:<suffix>:\nworker id and its single boundTo location (presentation, panel, tab,\ncontext).\n";
const HELP_TERMINAL_GET_ZH: &str = "orchbee terminal get <terminal-id> [--format text|json]\n\n按完整类型化 ID 或 @terminal:<suffix> 显示一个已绑定终端：worker id 及其唯一 boundTo 位置\n（presentation、panel、tab、context）。\n";

const HELP_TERMINAL_READ_EN: &str = "orchbee terminal read <terminal-id> [--history --rows <n>] [--unwrap] [--lines <n>] [--format text|json]\n\nRead the current visible content of one bound terminal. The terminal is\nresolved from the Workbench snapshot, then observed once through a short-\nlived read-only viewer on the same session; nothing is created, activated,\nor mutated. Text output prints the grid rows; '--format json' emits the\norchbee.terminal.read envelope with frame metadata and row content.\n\nWith '--history --rows <n>' (both flags required together, 1..=1000), read\nup to n retained rows that have scrolled off the active screen instead of\nthe visible frame; '--format json' emits the orchbee.terminal.history\nenvelope with scrollback metadata and row content.\n\n'--unwrap' is a read-only presentation transform: verified soft-wrapped\nphysical rows join into logical lines (hard breaks and empty lines are\npreserved; a window edge inside a wrapped line reports startsMidLine/\nendsMidLine instead of claiming a complete line). '--lines <n>' (1..=65535)\nshows only the last n emitted entries — logical lines under '--unwrap',\nphysical rows otherwise — and never widens the fetched window.\n";
const HELP_TERMINAL_READ_ZH: &str = "orchbee terminal read <terminal-id> [--history --rows <n>] [--unwrap] [--lines <n>] [--format text|json]\n\n读取一个已绑定终端的当前可见内容。终端经 Workbench 快照解析后，通过同\n一会话上短暂存活的只读 viewer 观察一次；不创建、不激活、不修改任何\n状态。文本输出逐行打印网格内容；'--format json' 输出带帧元数据与行内\n容的 orchbee.terminal.read 封套。\n\n加 '--history --rows <n>'（两个 flag 必须成对，1..=1000）时，读取已滚\n出当前活动屏幕的至多 n 行保留历史，而非可见帧；'--format json' 输出\n带历史元数据与行内容的 orchbee.terminal.history 封套。\n\n'--unwrap' 是只读的呈现变换：经验证的软换行物理行拼接为逻辑行（保留\n硬换行与空行；窗口边缘落在换行行内部时以 startsMidLine/endsMidLine\n如实标记，而非声称完整行）。'--lines <n>'（1..=65535）只显示最后 n\n条已发出的条目——'--unwrap' 下为逻辑行，否则为物理行——且不会扩大\n抓取窗口。\n";

const HELP_TERMINAL_WAIT_OUTPUT_EN: &str = "orchbee terminal wait-output <terminal-id> (--match <text> | --regex <pattern>) [--rows <n>] [--timeout <ms>] [--format text|json]\n\nWait until the pattern appears in one bound terminal's retained output —\nexactly one of '--match' or '--regex' is required. '--match' is a literal,\ncase-sensitive substring (1..=4096 bytes); '--regex' is a nonempty UTF-8\nregular expression of at most 1024 bytes, compiled once before any\nconnection (invalid or oversized programs are usage failures). Matching\nchecks the most recent --rows retained rows (1..=1000, default 1000)\nspanning the visible screen and scrollback as one window, joining\nsoft-wrapped rows into logical lines first; a regex match reports its\nbyte length and fragment-edge flags, and '^'/'$' anchor at the observed\nfragment edges. '--timeout' bounds the total wait in milliseconds\n(1..=3600000, default 30000); the command exits 0 on match, 7 on timeout,\nand 130 when cancelled by HUP/INT/QUIT/TERM. Nothing is typed into the\nterminal and nothing is created or mutated. Text output prints the\nmatching line; '--format json' emits the orchbee.terminal.wait-output\nenvelope with match and observation metadata.\n";
const HELP_TERMINAL_WAIT_OUTPUT_ZH: &str = "orchbee terminal wait-output <terminal-id> (--match <text> | --regex <pattern>) [--rows <n>] [--timeout <ms>] [--format text|json]\n\n等待一个已绑定终端的保留输出中出现指定模式——'--match' 与 '--regex'\n必须且只能提供一个。'--match' 为字面、区分大小写的子串（1..=4096 字\n节）；'--regex' 为至多 1024 字节的非空 UTF-8 正则表达式，在任何连接\n建立前编译一次（非法或超限程序报用法错误）。搜索范围为最近 --rows\n行保留行（1..=1000，默认 1000），可见屏与回滚历史作为一个窗口，并\n先把软换行行拼接为逻辑行；正则命中会报告字节长度与片段边缘标记，\n'^'/'$' 仅锚定观测到的片段边界。'--timeout' 以毫秒限定总等待预算\n（1..=3600000，默认 30000）；匹配成功退出 0，超时退出 7，被\nHUP/INT/QUIT/TERM 取消时退出 130。不向终端写入任何输入，不创建、不\n修改任何状态。文本输出打印匹配行；'--format json' 输出带匹配与观测\n元数据的 orchbee.terminal.wait-output 封套。\n";
const HELP_TERMINAL_SEND_EN: &str = "orchbee terminal send <terminal-id> (--text <utf8> [--enter] | --key <name> | --file <path> [--enter] | --stdin [--enter]) [--format text|json]\n\nSend exactly one input record to one bound terminal. Exactly one source is\nrequired: the literal bytes of '--text' (UTF-8, at most 4096 bytes including\nthe appended CR); one named key from '--key'; the UTF-8 contents of the\nregular file named by '--file'; or the bytes piped or redirected to\n'--stdin' (a terminal, socket, or other stream is refused). For '--file' and\n'--stdin' the source is read completely under the same five-second budget,\nkept to at most 4096 bytes including the appended CR, and decoded as strict\nUTF-8 before anything is sent. '--enter' folds one carriage return into the\nsame record for every text-bearing form. Accepted key names: enter, escape,\ntab, backspace, delete, up, down, left, right, home, end, page-up,\npage-down, ctrl-c, ctrl-d. Named keys are delivered semantically; the Host\nencodes them under the terminal's current mode, never as canned escape\nbytes.\n\nThe send rides a temporary input-only attachment on the same session — it\nnever takes the TUI's controller, viewer, or resize ownership. The Site\nsubmits the record at most once and never retries an uncertain outcome;\nsends to one terminal serialize in arrival order. When the outcome is\nunknown (exit 8) do not resend blindly — check the terminal first, or you\nmay type twice. '--format json' emits the orchbee.terminal.send envelope\nwith inputStatus, cleanupStatus, and the accepted byte count.\n";
const HELP_TERMINAL_SEND_ZH: &str = "orchbee terminal send <terminal-id> (--text <utf8> [--enter] | --key <name> | --file <path> [--enter] | --stdin [--enter]) [--format text|json]\n\n向一个已绑定终端恰好发送一条输入记录。必须恰好声明一个来源：'--text'\n的字面字节（UTF-8，含附加回车在内不超过 4096 字节）；'--key' 的一个\n按键名；'--file' 所指常规文件的 UTF-8 内容；或经管道、重定向提供给\n'--stdin' 的字节（终端、套接字或其他流会被拒绝）。'--file' 与\n'--stdin' 在同一个五秒预算内把来源完整读入、连同附加回车不超过\n4096 字节、按严格 UTF-8 解码，之后才会发送任何内容。每种承载文本的\n形式都可用 '--enter' 把一个回车并入同一记录。接受的按键名：enter、\nescape、tab、backspace、delete、up、down、left、right、home、end、\npage-up、page-down、ctrl-c、ctrl-d。具名按键按语义递交，由 Host 按\n终端当前模式编码，绝不发送预置转义字节。\n\n发送经由同一会话上的临时仅输入附属通道——绝不接管 TUI 的\ncontroller、viewer 或 resize 所有权。Site 至多提交一次该记录，对不\n确定结果绝不自动重试；同一终端的多个发送按到达顺序串行。当结果为\nunknown（退出码 8）时不要盲目重发——先查看终端，否则可能输入两次。\n'--format json' 输出带 inputStatus、cleanupStatus 与已接受字节数的\norchbee.terminal.send 封套。\n";

const HELP_WORKTREE_EN: &str = "orchbee worktree list\n\nInspect the Git worktrees of one registered project directory.\n  worktree list --project <target> --directory <directory-id>\n                               List the directory's Git worktrees\n--project resolves an exact ID, @project:<suffix>, or exact label;\n--directory is always the\nstrict nonzero 32-lowercase-hex Workbench directory ID, never a path or\nlabel. Directory membership is decided by the running Site, not the CLI.\n";
const HELP_WORKTREE_ZH: &str = "orchbee worktree list\n\n查看一个已注册项目目录的 Git worktree。\n  worktree list --project <target> --directory <directory-id>\n                               列出该目录的 Git worktree\n--project 按精确 ID、@project:<suffix> 或精确标签解析；--directory 只接受严格的非零 32 位\n小写十六进制 Workbench 目录 ID，绝不接受路径或标签。目录归属由运行中\n的 Site 判定，CLI 不做预检。\n";

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

const HELP_WORKTREE_LIST_EN: &str = "orchbee worktree list --project <target> --directory <directory-id> [--format text|json]\n\nList the Git worktrees of one registered project directory. --project takes\nan exact ID, @project:<suffix>, or exact label (ambiguous labels and\nsuffixes are rejected); --directory is\nthe strict nonzero 32-lowercase-hex Workbench directory ID — never a\nfilesystem path. The listing is read-only: it never creates, moves, locks,\nor prunes a worktree. An all-zero HEAD is an unborn repository, a null\nhead marks the bare repository, and branch names keep their full\n'refs/heads/…' form. '--format json' emits the orchbee.worktree.list\nenvelope; failures use the orchbee.cli.failure.worktree envelope and still\nrequire an explicit '--format json' for the stdout document.\n";
const HELP_WORKTREE_LIST_ZH: &str = "orchbee worktree list --project <target> --directory <directory-id> [--format text|json]\n\n列出一个已注册项目目录的 Git worktree。--project 接受精确 ID、@project:<suffix> 或精确标签\n（标签或后缀歧义会被拒绝）；--directory 为严格的非零 32 位小写十六进制\nWorkbench 目录 ID——绝不接受文件系统路径。该列举为只读操作：绝不创建、\n移动、锁定或清理任何 worktree。全零 HEAD 表示未出生的仓库，head 为\nnull 表示裸仓库，分支名保留完整的 'refs/heads/…' 形式。\n'--format json' 输出 orchbee.worktree.list 封套；失败使用\norchbee.cli.failure.worktree 封套，stdout 文档仍需显式 '--format json'。\n";

const HELP_API_SNAPSHOT_EN: &str = "orchbee api snapshot [--format text|json]\n\nEmit the complete validated workbench.snapshot document inside the\norchbee.api.snapshot envelope. Successful output is JSON even under\n'--format text'; a failure envelope still requires '--format json'.\n";
const HELP_API_SNAPSHOT_ZH: &str = "orchbee api snapshot [--format text|json]\n\n在 orchbee.api.snapshot 封套内输出完整已验证 workbench.snapshot 文档。\n成功输出即使 '--format text' 也为 JSON；失败封套仍需显式 '--format json'。\n";

const HELP_API_SCHEMA_EN: &str = "orchbee api schema [<schema-id>] [--format text|json]\n\nEmit the embedded JSON Schema dictionary for every implemented command, or\nthe single schema for <schema-id>. Successful output is always JSON; unknown\nIDs are not-found, and failure envelopes require '--format json'.\n";
const HELP_API_SCHEMA_ZH: &str = "orchbee api schema [<schema-id>] [--format text|json]\n\n输出所有已实现命令的内嵌 JSON Schema 字典，或 <schema-id> 的单个 schema。\n成功输出始终为 JSON；未知 ID 为 not-found，失败封套需显式 '--format json'。\n";

const HELP_CONFIG_CHECK_EN: &str = "orchbee config check [--format text|json]\n\nRun the five local checks (runtime-base, runtime-directory, cohort, locale,\nsockets) and report pass/fail/not-applicable each. Exit 0 when all pass or\nare not-applicable, 1 when any check fails.\n";
const HELP_CONFIG_CHECK_ZH: &str = "orchbee config check [--format text|json]\n\n运行五项本地检查（runtime-base、runtime-directory、cohort、locale、\nsockets）并逐项报告 pass/fail/not-applicable。全部通过或不适用时退出\n0，任一失败时退出 1。\n";

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
    pub(crate) workspace_headers: [&'static str; 5],
    pub(crate) tab_ref_headers: [&'static str; 2],
    pub(crate) tab_headers: [&'static str; 6],
    pub(crate) panel_headers: [&'static str; 6],
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
    workspace_headers: ["ID", "LABEL", "PROJECT", "PRIMARY DIRECTORY", "CONTEXT"],
    tab_ref_headers: ["ID", "LABEL"],
    tab_headers: ["ID", "LABEL", "PINNED", "GROUP", "CONTEXT", "CONTEXT ID"],
    panel_headers: [
        "ID",
        "TAB",
        "CONTEXT",
        "PRIMARY PRESENTATION",
        "PRESENTATIONS",
        "PRIMARY TERMINAL",
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
    workspace_headers: ["ID", "LABEL", "PROJECT", "主目录", "上下文"],
    tab_ref_headers: ["ID", "LABEL"],
    tab_headers: ["ID", "LABEL", "置顶", "分组", "上下文", "上下文 ID"],
    panel_headers: [
        "ID",
        "TAB",
        "上下文",
        "主 PRESENTATION",
        "PRESENTATION 数",
        "主终端",
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
