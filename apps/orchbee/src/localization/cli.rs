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
        match failure.kind {
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
            FailureKind::Timeout => match self {
                Self::EnUs => {
                    "The wait deadline elapsed before the text appeared in the terminal.\n"
                        .to_owned()
                }
                Self::ZhCn => "等待期限内终端中未出现目标文本。\n".to_owned(),
            },
            FailureKind::Cancelled => match self {
                Self::EnUs => "The wait was cancelled; no match is reported.\n".to_owned(),
                Self::ZhCn => "等待已被取消；未报告任何匹配。\n".to_owned(),
            },
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

    /// The localized `invalid-usage` stderr diagnostic: one concrete cause
    /// plus a help hint that always names a real path. `None` and the
    /// `Invalid` fallback keep the original generic text.
    fn cli_usage(self, usage: Option<&crate::invocation::UsageReason>) -> String {
        use crate::invocation::{ExpectedValue, UsageContext, UsageReason};
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
                "The native CLI query surface is supported on Linux only.\n".to_owned()
            }
            (Self::ZhCn, "platform") => "原生 CLI 查询面仅在 Linux 上受支持。\n".to_owned(),
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
            (Self::EnUs, "cohort") => {
                "The native development cohort is missing, changed, or invalid.\n".to_owned()
            }
            (Self::ZhCn, "cohort") => "原生开发 cohort 缺失、已修改或无效。\n".to_owned(),
            (Self::EnUs, "platform") => "config check is supported on Linux only.\n".to_owned(),
            (Self::ZhCn, "platform") => "config check 仅在 Linux 上受支持。\n".to_owned(),
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

    /// The text-output marker appended when the read budget omitted frame rows.
    pub(crate) fn cli_read_truncated(self, omitted: usize) -> String {
        match self {
            Self::EnUs => format!("… {omitted} more rows truncated at the read limit\n"),
            Self::ZhCn => format!("… 因读取上限省略了 {omitted} 行\n"),
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
            (Self::EnUs, CommandPath::ApiSnapshot) => HELP_API_SNAPSHOT_EN,
            (Self::ZhCn, CommandPath::ApiSnapshot) => HELP_API_SNAPSHOT_ZH,
            (Self::EnUs, CommandPath::ApiSchema) => HELP_API_SCHEMA_EN,
            (Self::ZhCn, CommandPath::ApiSchema) => HELP_API_SCHEMA_ZH,
            (Self::EnUs, CommandPath::ConfigCheck) => HELP_CONFIG_CHECK_EN,
            (Self::ZhCn, CommandPath::ConfigCheck) => HELP_CONFIG_CHECK_ZH,
        }
    }
}

const ROOT_HELP_EN: &str = "Orchbee\n\nUsage:\n  orchbee\n  orchbee tui [--mouse on|off]\n  orchbee status [--format text|json]\n  orchbee <family> <command> [operands] [--format text|json]\n  orchbee api snapshot|schema\n  orchbee config check [--format text|json]\n  orchbee help [<family>|<command>]\n  orchbee completion bash|zsh|fish\n  orchbee -h|--help\n  orchbee --version\n\nModes and queries:\n  orchbee              Open the local terminal (Linux development cohort)\n  orchbee tui          Open the same local terminal explicitly\n  orchbee status       Read the running local Site's Workbench snapshot\n  orchbee project      list | get\n  orchbee workspace    list | get\n  orchbee tab          list | get\n  orchbee panel        list | get\n  orchbee terminal     list | get | read | wait-output | send\n  orchbee api          Machine snapshot and embedded schema documents\n\nMutations (one command per invocation, against the running Site):\n  orchbee project      create\n  orchbee workspace    create | rename | accept-directory\n  orchbee tab          create | rename | move | close\n  orchbee panel        split | move | close\n  orchbee terminal     send\n\nInspection and help:\n  orchbee config check Inspect local runtime/cohort/locale/socket inputs\n  orchbee help         Show help for a family or complete command path\n  orchbee completion   Emit a static shell completion script\n  orchbee -h|--help    Show this help; after a family or command path, show\n                       that path's help\n  orchbee --version    Show the version\n\nRun 'orchbee help <family>' for filters and target rules, or\n'orchbee help <family> <command>' for one mutation's full syntax.\n";
const ROOT_HELP_ZH: &str = "Orchbee\n\n用法：\n  orchbee\n  orchbee tui [--mouse on|off]\n  orchbee status [--format text|json]\n  orchbee <family> <command> [操作数] [--format text|json]\n  orchbee api snapshot|schema\n  orchbee config check [--format text|json]\n  orchbee help [<family>|<command>]\n  orchbee completion bash|zsh|fish\n  orchbee -h|--help\n  orchbee --version\n\n模式与查询：\n  orchbee              打开本地终端（Linux 开发 cohort）\n  orchbee tui          显式打开同一本地终端\n  orchbee status       读取正在运行的本地 Site 的 Workbench 快照\n  orchbee project      list | get\n  orchbee workspace    list | get\n  orchbee tab          list | get\n  orchbee panel        list | get\n  orchbee terminal     list | get | read | wait-output | send\n  orchbee api          机器快照与内嵌 schema 文档\n\n修改命令（每次调用执行一条命令，作用于运行中的 Site）：\n  orchbee project      create\n  orchbee workspace    create | rename | accept-directory\n  orchbee tab          create | rename | move | close\n  orchbee panel        split | move | close\n  orchbee terminal     send\n\n检查与帮助：\n  orchbee config check 检查本地运行时/cohort/locale/套接字输入\n  orchbee help         显示某个 family 或完整命令路径的帮助\n  orchbee completion   输出静态 shell 补全脚本\n  orchbee -h|--help    显示此帮助；跟在 family 或命令路径后时显示该路径的帮助\n  orchbee --version    显示版本\n\n运行 'orchbee help <family>' 查看过滤参数与 target 规则，或\n'orchbee help <family> <command>' 查看单条修改命令的完整语法。\n";
const HELP_TUI_EN: &str = "orchbee tui [--mouse on|off]\n\nOpen the local terminal interface (Linux development cohort).\nA bare 'orchbee' invocation does the same.\n\n  --mouse on|off   Host pointer capture for pane hit-testing; on by\n                   default. Only the explicit 'tui' route accepts it.\n";
const HELP_TUI_ZH: &str = "orchbee tui [--mouse on|off]\n\n打开本地终端界面（Linux 开发 cohort）。\n裸 'orchbee' 调用效果相同。\n\n  --mouse on|off   宿主机指针捕获，用于面板命中；默认开启。\n                   只有显式 'tui' 路由接受该选项。\n";

const HELP_STATUS_EN: &str = "orchbee status\n\nRead a live Workbench snapshot from the running local Site and report\nreachability plus entity counts. The Site is never created, activated, or\nmutated. '--format json' emits the orchbee.status envelope.\n";
const HELP_STATUS_ZH: &str = "orchbee status\n\n从正在运行的本地 Site 读取实时 Workbench 快照并报告可达性与实体计数。\n绝不创建、激活或修改 Site。'--format json' 输出 orchbee.status 封套。\n";

const HELP_PROJECT_EN: &str = "orchbee project list|get|create\n\nQuery and mutate Project resources in the running local Site.\n  project list                       List every Project\n  project get <target>               Show one Project by exact ID or exact label\n  project create --label <l>         Create one Project\nTargets are case-sensitive; ambiguous labels are rejected, never guessed.\nEvery mutation reads one snapshot, then submits exactly one command.\n";
const HELP_PROJECT_ZH: &str = "orchbee project list|get|create\n\n查询并修改本地 Site 中的 Project 资源。\n  project list                       列出所有 Project\n  project get <target>               按精确 ID 或精确 label 显示一个 Project\n  project create --label <l>         创建一个 Project\ntarget 区分大小写；歧义 label 会被拒绝而非猜测。\n每个修改命令先读取一次快照，然后只提交一个命令。\n";

const HELP_WORKSPACE_EN: &str = "orchbee workspace list|get|create|rename|accept-directory\n\nQuery and mutate Workspace resources in the running local Site.\n  workspace list [--project <t>]                    List Workspaces, optionally inside one Project\n  workspace get <target>                            Show one Workspace by exact ID or exact label\n  workspace create --project <t> --label <l>        Create one Workspace inside a Project\n  workspace rename <target> --label <l>             Rename one Workspace\n  workspace accept-directory <target> --path <p>    Accept a literal absolute path as the Workspace's primary directory\nEvery mutation reads one snapshot, then submits exactly one command.\n";
const HELP_WORKSPACE_ZH: &str = "orchbee workspace list|get|create|rename|accept-directory\n\n查询并修改本地 Site 中的 Workspace 资源。\n  workspace list [--project <t>]                    列出 Workspace，可按某个 Project 过滤\n  workspace get <target>                            按精确 ID 或精确 label 显示一个 Workspace\n  workspace create --project <t> --label <l>        在某个 Project 内创建一个 Workspace\n  workspace rename <target> --label <l>             重命名一个 Workspace\n  workspace accept-directory <target> --path <p>    将一个字面绝对路径接受为 Workspace 的主目录\n每个修改命令先读取一次快照，然后只提交一个命令。\n";

const HELP_TAB_EN: &str = "orchbee tab list|get|create|rename|move|close\n\nQuery and mutate Tab resources in the running local Site.\n  tab list [--workspace <t> | --standalone]  List Tabs, optionally filtered\n  tab get <target>                           Show one Tab by exact ID or label\n  tab create (--workspace <t>|--standalone) --label <l> --cols <n> --rows <n> --cell-width <px> --cell-height <px>\n                                             Create one Tab in one context\n  tab rename <target> --label <l>            Rename one Tab\n  tab move <target> --direction <d>          Move one Tab within its strip\n  tab close <target> --confirm               Close one Tab\n'--workspace' and '--standalone' are mutually exclusive single-use selectors.\n";
const HELP_TAB_ZH: &str = "orchbee tab list|get|create|rename|move|close\n\n查询并修改本地 Site 中的 Tab 资源。\n  tab list [--workspace <t> | --standalone]  列出 Tab，可按 Workspace 或独立上下文过滤\n  tab get <target>                           按精确 ID 或 label 显示一个 Tab\n  tab create (--workspace <t>|--standalone) --label <l> --cols <n> --rows <n> --cell-width <px> --cell-height <px>\n                                             在某个上下文内创建一个 Tab\n  tab rename <target> --label <l>            重命名一个 Tab\n  tab move <target> --direction <d>          在标签条内移动一个 Tab\n  tab close <target> --confirm               关闭一个 Tab\n'--workspace' 与 '--standalone' 互斥且各只能用一次。\n";

const HELP_PANEL_EN: &str = "orchbee panel list|get|split|close|move\n\nQuery and mutate Panel resources in the running local Site.\n  panel list [--tab <t>]                   List Panels, optionally inside one Tab\n  panel get <panel-id>                     Show one Panel by its complete typed ID\n  panel split <panel-id> --direction <d> --label <l> --cols <n> --rows <n> --cell-width <px> --cell-height <px>\n                                           Split one Panel, creating a sibling\n  panel close <panel-id> --confirm         Close one Panel\n  panel move <panel-id> --target-panel <panel-id> --direction <d>\n                                           Move one Panel next to a sibling\nPanels have no independent label; only complete 32-hex IDs are accepted.\n";
const HELP_PANEL_ZH: &str = "orchbee panel list|get|split|close|move\n\n查询并修改本地 Site 中的 Panel 资源。\n  panel list [--tab <t>]                   列出 Panel，可按某个 Tab 过滤\n  panel get <panel-id>                     按完整类型化 ID 显示一个 Panel\n  panel split <panel-id> --direction <d> --label <l> --cols <n> --rows <n> --cell-width <px> --cell-height <px>\n                                           分裂一个 Panel，创建一个相邻 Panel\n  panel close <panel-id> --confirm         关闭一个 Panel\n  panel move <panel-id> --target-panel <panel-id> --direction <d>\n                                           将一个 Panel 移动到某个相邻 Panel 旁\nPanel 没有独立 label；只接受完整 32 位小写十六进制 ID。\n";

const HELP_TERMINAL_EN: &str = "orchbee terminal list|get|read|wait-output|send\n\nQuery terminals bound by Workbench Presentations in the running local Site,\nor send one input record to one of them.\n  terminal list                List every bound terminal\n  terminal get <terminal-id>   Show one terminal by its complete typed ID\n  terminal read <terminal-id> [--history --rows <n>]\n                               Read the terminal's visible content; the paired\n                               flags read retained history rows instead\n  terminal wait-output <terminal-id> --match <text>\n                               Wait for text in the terminal's retained output\n  terminal send <terminal-id>  Send text or one key to the terminal's input\nA terminal invisible to any Presentation binding cannot be listed, read,\nwaited on, or sent to; a binding proves structure only, not that a process\nis alive.\n";
const HELP_TERMINAL_ZH: &str = "orchbee terminal list|get|read|wait-output|send\n\n查询本地 Site 中被 Workbench Presentation 绑定的终端，或向其中一个终端发送一条输入。\n  terminal list                列出所有已绑定终端\n  terminal get <terminal-id>   按完整类型化 ID 显示一个终端\n  terminal read <terminal-id> [--history --rows <n>]\n                               读取终端当前可见内容；成对 flag 时改为读取\n                               保留历史行\n  terminal wait-output <terminal-id> --match <text>\n                               等待终端保留输出中出现指定文本\n  terminal send <terminal-id>  向终端输入发送文本或一个按键\n未被任何 Presentation 引用的终端不可见、不可读、不可等待也不可发送；绑定只证明结构，不证明进程存活。\n";

const HELP_API_EN: &str = "orchbee api snapshot|schema\n\nMachine introspection of the local Workbench surface.\n  api snapshot           Emit the complete workbench.snapshot document\n  api schema [<id>]      List embedded JSON Schemas, or one schema by ID\nSuccessful output is always JSON, whatever '--format' was selected; a\nfailure envelope still requires an explicit '--format json'.\n";
const HELP_API_ZH: &str = "orchbee api snapshot|schema\n\n本地 Workbench 面的机器自省。\n  api snapshot           输出完整 workbench.snapshot 文档\n  api schema [<id>]      列出内嵌 JSON Schema，或按 ID 取单个 schema\n成功输出无论 '--format' 选择如何都是 JSON；失败封套仍需显式\n'--format json'。\n";

const HELP_CONFIG_EN: &str = "orchbee config check\n\nInspect local inputs only: runtime base, Site runtime directory, current\ncohort integrity, effective locale, and socket presence plus the socket\naddress budget. Nothing is connected, started, created, or locked. Any\nfailed check exits 1.\n";
const HELP_CONFIG_ZH: &str = "orchbee config check\n\n仅检查本地输入：运行时基座、Site 运行时目录、当前 cohort 完整性、生效\nlocale、套接字存在性与套接字地址长度预算。不连接、不启动、不创建、\n不加锁。任一检查失败以 1 退出。\n";

const HELP_COMPLETION_EN: &str = "orchbee completion bash|zsh|fish\n\nEmit a static completion script for the implemented grammar to stdout.\nNothing is installed and no user files are written.\n";
const HELP_COMPLETION_ZH: &str = "orchbee completion bash|zsh|fish\n\n向标准输出生成已实现语法的静态补全脚本。不安装、不写入用户文件。\n";

const HELP_PROJECT_LIST_EN: &str = "orchbee project list [--format text|json]\n\nList every Project in the running local Site: id, label, directory and\nworkspace counts. An empty Site prints just the table header.\n";
const HELP_PROJECT_LIST_ZH: &str = "orchbee project list [--format text|json]\n\n列出本地 Site 中所有 Project：ID、label、目录与工作区计数。空 Site 仅\n输出表头。\n";

const HELP_PROJECT_GET_EN: &str = "orchbee project get <target> [--format text|json]\n\nShow one Project: its directories (with worker ids and paths) and its\nworkspaces. The target is an exact 32-hex ID or an exact case-sensitive\nlabel; ambiguous labels report candidates instead of guessing.\n";
const HELP_PROJECT_GET_ZH: &str = "orchbee project get <target> [--format text|json]\n\n显示一个 Project：其目录（含 worker id 与路径）与工作区。target 为精确\n32 位十六进制 ID 或精确区分大小写的 label；歧义 label 报告候选而非猜测。\n";

const HELP_PROJECT_CREATE_EN: &str = "orchbee project create --label <label> [--format text|json]\n\nCreate one Project with the given label. The command reads one snapshot,\nthen submits exactly one command under a fresh request id. The result\ncarries the committed revision and the Server-allocated project id.\nA new Project has no directory or Workspace, and creating one touches no\nfilesystem path. Labels may repeat across Projects; address the new\nProject by its returned id.\n";
const HELP_PROJECT_CREATE_ZH: &str = "orchbee project create --label <label> [--format text|json]\n\n以给定 label 创建一个 Project。命令先读取一次快照，然后以全新请求 ID\n只提交一个命令。结果携带已提交修订与 Server 分配的 project id。\n新 Project 无目录、无 Workspace，创建不触碰任何文件系统路径。\nlabel 允许跨 Project 重复；请按返回的 ID 寻址新 Project。\n";

const HELP_WORKSPACE_LIST_EN: &str = "orchbee workspace list [--project <t>] [--format text|json]\n\nList Workspaces across Projects, or only inside the Project selected by\n--project (exact ID or exact label). Fields: ids, label, primary directory\n(null means none), and terminal context.\n";
const HELP_WORKSPACE_LIST_ZH: &str = "orchbee workspace list [--project <t>] [--format text|json]\n\n跨 Project 列出 Workspace，或仅列 --project（精确 ID 或精确 label）所选\nProject 内的。字段：各 ID、label、主目录（null 表示无）与终端上下文。\n";

const HELP_WORKSPACE_GET_EN: &str = "orchbee workspace get <target> [--format text|json]\n\nShow one Workspace plus the Tabs that live in its terminal context.\n";
const HELP_WORKSPACE_GET_ZH: &str = "orchbee workspace get <target> [--format text|json]\n\n显示一个 Workspace 及其终端上下文中存在的 Tab。\n";

const HELP_WORKSPACE_CREATE_EN: &str = "orchbee workspace create --project <target> --label <label> [--format text|json]\n\nCreate one Workspace inside the Project selected by --project (exact ID or\nexact label). The command reads one snapshot, resolves the target, then\nsubmits exactly one command under a fresh request id. The result carries\nthe committed revision and the new workspace and terminal context ids.\n";
const HELP_WORKSPACE_CREATE_ZH: &str = "orchbee workspace create --project <target> --label <label> [--format text|json]\n\n在 --project（精确 ID 或精确 label）所选 Project 内创建一个 Workspace。\n命令先读取一次快照、解析目标，然后以全新请求 ID 只提交一个命令。结果\n携带已提交修订以及新的 workspace 与终端上下文 ID。\n";

const HELP_WORKSPACE_RENAME_EN: &str = "orchbee workspace rename <target> --label <label> [--format text|json]\n\nRename one Workspace selected by exact ID or exact label. The command\nreads one snapshot, then submits exactly one command. A committed result\ncarries the new revision; an unchanged one means the label already\nmatched.\n";
const HELP_WORKSPACE_RENAME_ZH: &str = "orchbee workspace rename <target> --label <label> [--format text|json]\n\n重命名一个由精确 ID 或精确 label 选择的 Workspace。命令先读取一次快\n照，然后只提交一个命令。committed 结果携带新修订；unchanged 表示\nlabel 本已相同。\n";
const HELP_WORKSPACE_ACCEPT_DIRECTORY_EN: &str = "orchbee workspace accept-directory <target> --path <path> [--format text|json]\n\nAccept a literal absolute POSIX path as the primary directory of the\nWorkspace selected by exact ID or exact label. '--path' is required: a\nliteral absolute path of at most 4,096 UTF-8 bytes without NUL — never\nexpanded ('~', '$'), created, or probed by the CLI. The command reads one\nsnapshot, resolves the target and its owning Project, then submits\nexactly one command under a fresh request id; the Server authoritatively\nprobes the path and rejects a Workspace that already has a directory.\nA committed result carries the new revision, the Server-allocated\ndirectory and worker ids, the resolved project and workspace ids, and the\nrequest's literal path.\n";
const HELP_WORKSPACE_ACCEPT_DIRECTORY_ZH: &str = "orchbee workspace accept-directory <target> --path <path> [--format text|json]\n\n将一个字面绝对 POSIX 路径接受为由精确 ID 或精确 label 选择的\nWorkspace 的主目录。'--path' 为必填：字面绝对路径，至多 4,096 个\nUTF-8 字节且不含 NUL——CLI 不展开（'~'、'$'）、不创建、不探测。命\n令先读取一次快照、解析目标及其所属 Project，然后以全新请求 ID 只\n提交一个命令；由 Server 权威探测该路径，并拒绝已有目录的\nWorkspace。committed 结果携带新修订、Server 分配的 directory 与\nworker ID、已解析的 project 与 workspace ID，以及请求的字面路径。\n";

const HELP_TAB_LIST_EN: &str = "orchbee tab list [--workspace <t> | --standalone] [--format text|json]\n\nList Tabs. Filters select one Workspace's context (exact ID or label) or\nthe standalone context. Fields: ids, label, pinned flag, group, and owning\ncontext kind.\n";
const HELP_TAB_LIST_ZH: &str = "orchbee tab list [--workspace <t> | --standalone] [--format text|json]\n\n列出 Tab。过滤参数可选择一个 Workspace 的上下文（精确 ID 或 label）或\n独立上下文。字段：各 ID、label、置顶标记、分组与所属上下文类型。\n";

const HELP_TAB_GET_EN: &str = "orchbee tab get <target> [--format text|json]\n\nShow one Tab including its complete PanelNode root subtree.\n";
const HELP_TAB_GET_ZH: &str = "orchbee tab get <target> [--format text|json]\n\n显示一个 Tab，包括其完整 PanelNode 根子树。\n";

const HELP_TAB_CREATE_EN: &str = "orchbee tab create (--workspace <target> | --standalone) --label <label> --cols <n> --rows <n> --cell-width <px> --cell-height <px> [--format text|json]\n\nCreate one Tab in the selected terminal context: a Workspace chosen by\nexact ID or exact label, or the standalone context. All four geometry\nflags are required. The result carries the committed revision, the new\ntab, panel, and presentation ids, and the terminal spawn outcome.\n";
const HELP_TAB_CREATE_ZH: &str = "orchbee tab create (--workspace <target> | --standalone) --label <label> --cols <n> --rows <n> --cell-width <px> --cell-height <px> [--format text|json]\n\n在所选终端上下文内创建一个 Tab：由精确 ID 或精确 label 选择的\nWorkspace，或独立上下文。四个几何参数均为必填。结果携带已提交修订、\n新的 tab、panel、presentation ID 及终端拉起结果。\n";

const HELP_TAB_RENAME_EN: &str = "orchbee tab rename <target> --label <label> [--format text|json]\n\nRename one Tab selected by exact ID or exact label. A committed result\ncarries the new revision; unchanged means the label already matched.\n";
const HELP_TAB_RENAME_ZH: &str = "orchbee tab rename <target> --label <label> [--format text|json]\n\n重命名一个由精确 ID 或精确 label 选择的 Tab。committed 结果携带新修\n订；unchanged 表示 label 本已相同。\n";

const HELP_TAB_MOVE_EN: &str = "orchbee tab move <target> --direction <previous|next|first|last> [--format text|json]\n\nMove one Tab within its strip. A committed result carries the new\nrevision; unchanged means the tab was already in position.\n";
const HELP_TAB_MOVE_ZH: &str = "orchbee tab move <target> --direction <previous|next|first|last> [--format text|json]\n\n在标签条内移动一个 Tab。committed 结果携带新修订；unchanged 表示 Tab\n本已在目标位置。\n";

const HELP_TAB_CLOSE_EN: &str = "orchbee tab close <target> --confirm [--format text|json]\n\nClose one Tab selected by exact ID or exact label. '--confirm' is\nrequired. An accepted result carries the accepted revision and the close\nrequest id; the Site finishes the close asynchronously.\n";
const HELP_TAB_CLOSE_ZH: &str = "orchbee tab close <target> --confirm [--format text|json]\n\n关闭一个由精确 ID 或精确 label 选择的 Tab。'--confirm' 为必填。\naccepted 结果携带已接受修订与关闭请求 ID；Site 异步完成关闭。\n";

const HELP_PANEL_LIST_EN: &str = "orchbee panel list [--tab <t>] [--format text|json]\n\nList Panels, optionally inside the Tab selected by --tab (exact ID or\nlabel). Panels carry no label; presentation labels are not panel names.\n";
const HELP_PANEL_LIST_ZH: &str = "orchbee panel list [--tab <t>] [--format text|json]\n\n列出 Panel，或仅列 --tab（精确 ID 或 label）所选 Tab 内的。Panel 无\nlabel；Presentation 的 label 不是 Panel 名。\n";

const HELP_PANEL_GET_EN: &str = "orchbee panel get <panel-id> [--format text|json]\n\nShow one Panel by complete typed ID: owning tab, context, slot path, and\nits primary and child Presentations.\n";
const HELP_PANEL_GET_ZH: &str = "orchbee panel get <panel-id> [--format text|json]\n\n按完整类型化 ID 显示一个 Panel：所属 Tab、上下文、slot 路径及其主与\n子 Presentation。\n";

const HELP_PANEL_SPLIT_EN: &str = "orchbee panel split <panel-id> --direction <above|below|left|right> --label <label> --cols <n> --rows <n> --cell-width <px> --cell-height <px> [--format text|json]\n\nSplit one Panel, creating a sibling panel on the selected side. All four\ngeometry flags are required. The result carries the committed revision,\nthe new panel, split, slot, and presentation ids, and the terminal spawn\noutcome.\n";
const HELP_PANEL_SPLIT_ZH: &str = "orchbee panel split <panel-id> --direction <above|below|left|right> --label <label> --cols <n> --rows <n> --cell-width <px> --cell-height <px> [--format text|json]\n\n分裂一个 Panel，在所选侧创建相邻 Panel。四个几何参数均为必填。结果携\n带已提交修订、新的 panel、split、slot、presentation ID 及终端拉起\n结果。\n";

const HELP_PANEL_CLOSE_EN: &str = "orchbee panel close <panel-id> --confirm [--format text|json]\n\nClose one Panel selected by complete typed ID. '--confirm' is required.\nAn accepted result carries the accepted revision and the close request\nid; the Site finishes the close asynchronously.\n";
const HELP_PANEL_CLOSE_ZH: &str = "orchbee panel close <panel-id> --confirm [--format text|json]\n\n关闭一个由完整类型化 ID 选择的 Panel。'--confirm' 为必填。accepted\n结果携带已接受修订与关闭请求 ID；Site 异步完成关闭。\n";

const HELP_PANEL_MOVE_EN: &str = "orchbee panel move <panel-id> --target-panel <panel-id> --direction <above|below|left|right> [--format text|json]\n\nMove one Panel next to another panel inside the same tab. Both panels\nare complete typed IDs. A committed result carries the new revision;\nunchanged means the layout already matched.\n";
const HELP_PANEL_MOVE_ZH: &str = "orchbee panel move <panel-id> --target-panel <panel-id> --direction <above|below|left|right> [--format text|json]\n\n将一个 Panel 移动到同一 Tab 内另一个 Panel 的所选侧。两个 Panel 均为\n完整类型化 ID。committed 结果携带新修订；unchanged 表示布局本已相同。\n";

const HELP_TERMINAL_LIST_EN: &str = "orchbee terminal list [--format text|json]\n\nList every terminal bound by a Presentation. Unbound terminals are not\nvisible; a binding is structural evidence, not proof a process is alive.\n";
const HELP_TERMINAL_LIST_ZH: &str = "orchbee terminal list [--format text|json]\n\n列出所有被 Presentation 绑定的终端。未绑定终端不可见；绑定是结构证据，\n不是进程存活的证明。\n";

const HELP_TERMINAL_GET_EN: &str = "orchbee terminal get <terminal-id> [--format text|json]\n\nShow one bound terminal by complete typed ID: worker id and its single\nboundTo location (presentation, panel, tab, context).\n";
const HELP_TERMINAL_GET_ZH: &str = "orchbee terminal get <terminal-id> [--format text|json]\n\n按完整类型化 ID 显示一个已绑定终端：worker id 及其唯一 boundTo 位置\n（presentation、panel、tab、context）。\n";

const HELP_TERMINAL_READ_EN: &str = "orchbee terminal read <terminal-id> [--history --rows <n>] [--format text|json]\n\nRead the current visible content of one bound terminal. The terminal is\nresolved from the Workbench snapshot, then observed once through a short-\nlived read-only viewer on the same session; nothing is created, activated,\nor mutated. Text output prints the grid rows; '--format json' emits the\norchbee.terminal.read envelope with frame metadata and row content.\n\nWith '--history --rows <n>' (both flags required together, 1..=1000), read\nup to n retained rows that have scrolled off the active screen instead of\nthe visible frame; '--format json' emits the orchbee.terminal.history\nenvelope with scrollback metadata and row content.\n";
const HELP_TERMINAL_READ_ZH: &str = "orchbee terminal read <terminal-id> [--history --rows <n>] [--format text|json]\n\n读取一个已绑定终端的当前可见内容。终端经 Workbench 快照解析后，通过同\n一会话上短暂存活的只读 viewer 观察一次；不创建、不激活、不修改任何\n状态。文本输出逐行打印网格内容；'--format json' 输出带帧元数据与行内\n容的 orchbee.terminal.read 封套。\n\n加 '--history --rows <n>'（两个 flag 必须成对，1..=1000）时，读取已滚\n出当前活动屏幕的至多 n 行保留历史，而非可见帧；'--format json' 输出\n带历史元数据与行内容的 orchbee.terminal.history 封套。\n";

const HELP_TERMINAL_WAIT_OUTPUT_EN: &str = "orchbee terminal wait-output <terminal-id> --match <text> [--rows <n>] [--timeout <ms>] [--format text|json]\n\nWait until <text> appears in one bound terminal's retained output. The\ntext is a literal, case-sensitive substring (1..=4096 bytes); matching\nchecks the most recent --rows retained rows (1..=1000, default 1000)\nspanning the visible screen and scrollback as one window, joining\nsoft-wrapped rows into logical lines first. '--timeout' bounds the total\nwait in milliseconds (1..=3600000, default 30000); the command exits 0 on\nmatch, 7 on timeout, and 130 when cancelled by HUP/INT/QUIT/TERM. Nothing\nis typed into the terminal and nothing is created or mutated. Text output\nprints the matching line; '--format json' emits the\norchbee.terminal.wait-output envelope with match and observation\nmetadata.\n";
const HELP_TERMINAL_WAIT_OUTPUT_ZH: &str = "orchbee terminal wait-output <terminal-id> --match <text> [--rows <n>] [--timeout <ms>] [--format text|json]\n\n等待一个已绑定终端的保留输出中出现 <text>。匹配文本为字面、区分大小\n写的子串（1..=4096 字节）；搜索范围为最近 --rows 行保留行（1..=1000，\n默认 1000），可见屏与回滚历史作为一个窗口，并先把软换行行拼接为逻\n辑行。'--timeout' 以毫秒限定总等待预算（1..=3600000，默认 30000）；\n匹配成功退出 0，超时退出 7，被 HUP/INT/QUIT/TERM 取消时退出 130。不\n向终端写入任何输入，不创建、不修改任何状态。文本输出打印匹配行；\n'--format json' 输出带匹配与观测元数据的\norchbee.terminal.wait-output 封套。\n";
const HELP_TERMINAL_SEND_EN: &str = "orchbee terminal send <terminal-id> (--text <utf8> [--enter] | --key <name>) [--format text|json]\n\nSend exactly one input record to one bound terminal: the literal bytes of\n'--text' (UTF-8, at most 4096 bytes including the appended CR), optionally\nwith '--enter' folding one carriage return into the same record, or one\nnamed key from '--key'. Accepted key names: enter, escape, tab, backspace,\ndelete, up, down, left, right, home, end, page-up, page-down, ctrl-c,\nctrl-d. Named keys are delivered semantically; the Host encodes them under\nthe terminal's current mode, never as canned escape bytes.\n\nThe send rides a temporary input-only attachment on the same session — it\nnever takes the TUI's controller, viewer, or resize ownership. The Site\nsubmits the record at most once and never retries an uncertain outcome;\nsends to one terminal serialize in arrival order. When the outcome is\nunknown (exit 8) do not resend blindly — check the terminal first, or you\nmay type twice. '--format json' emits the orchbee.terminal.send envelope\nwith inputStatus, cleanupStatus, and the accepted byte count.\n";
const HELP_TERMINAL_SEND_ZH: &str = "orchbee terminal send <terminal-id> (--text <utf8> [--enter] | --key <name>) [--format text|json]\n\n向一个已绑定终端恰好发送一条输入记录：'--text' 的字面字节（UTF-8，\n含附加回车在内不超过 4096 字节），可选 '--enter' 把一个回车并入同一\n记录；或 '--key' 的一个按键名。接受的按键名：enter、escape、tab、\nbackspace、delete、up、down、left、right、home、end、page-up、\npage-down、ctrl-c、ctrl-d。具名按键按语义递交，由 Host 按终端当前\n模式编码，绝不发送预置转义字节。\n\n发送经由同一会话上的临时仅输入附属通道——绝不接管 TUI 的\ncontroller、viewer 或 resize 所有权。Site 至多提交一次该记录，对不\n确定结果绝不自动重试；同一终端的多个发送按到达顺序串行。当结果为\nunknown（退出码 8）时不要盲目重发——先查看终端，否则可能输入两次。\n'--format json' 输出带 inputStatus、cleanupStatus 与已接受字节数的\norchbee.terminal.send 封套。\n";

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
    pub(crate) child_presentations: &'static str,
    pub(crate) context: &'static str,
    pub(crate) counts: &'static str,
    pub(crate) directories: &'static str,
    pub(crate) group: &'static str,
    pub(crate) id: &'static str,
    pub(crate) panels_word: &'static str,
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
    pub(crate) workspace_ref_headers: [&'static str; 2],
    pub(crate) workspace_headers: [&'static str; 5],
    pub(crate) tab_ref_headers: [&'static str; 2],
    pub(crate) tab_headers: [&'static str; 6],
    pub(crate) panel_headers: [&'static str; 6],
    pub(crate) presentation_headers: [&'static str; 4],
    pub(crate) terminal_headers: [&'static str; 6],
}

const EN_US_LABELS: CliLabels = CliLabels {
    bound_terminals_word: "bound terminals",
    bound_to: "Bound to",
    child_presentations: "Child presentations",
    context: "Context",
    counts: "Counts",
    directories: "Directories",
    group: "Group",
    id: "ID",
    panels_word: "panels",
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
};

const ZH_CN_LABELS: CliLabels = CliLabels {
    bound_terminals_word: "已绑定终端",
    bound_to: "绑定到",
    child_presentations: "子 Presentation",
    context: "上下文",
    counts: "计数",
    directories: "目录",
    group: "分组",
    id: "ID",
    panels_word: "面板",
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
};

impl Locale {
    pub(crate) const fn cli_labels(self) -> &'static CliLabels {
        match self {
            Self::EnUs => &EN_US_LABELS,
            Self::ZhCn => &ZH_CN_LABELS,
        }
    }
}
