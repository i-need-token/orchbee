//! Localized terminal attachment and stream failures; does not own the input transport.

use super::Locale;

impl super::CliText {
    pub(crate) const fn attach_requires_terminal(self) -> &'static str {
        match self.locale {
            Locale::EnUs => {
                "Attach requires interactive terminal stdin and stdout. Use terminal session control for a pipe.\n"
            }
            Locale::ZhCn => {
                "attach 需要交互终端的 stdin 和 stdout。管道输入请使用 terminal session control。\n"
            }
        }
    }

    pub(crate) const fn attach_input_unavailable(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Input unavailable",
            Locale::ZhCn => "当前输入不可用",
        }
    }

    pub(crate) fn terminal_stream_failure(self, failure: &crate::cli_outcome::Failure) -> String {
        use crate::cli_outcome::FailureKind;
        let message = match (self.locale, failure.kind) {
            (Locale::EnUs, FailureKind::Timeout) => "The terminal stream deadline elapsed.\n",
            (Locale::ZhCn, FailureKind::Timeout) => "终端流操作已超时。\n",
            (Locale::EnUs, FailureKind::Cancelled) => "The terminal stream was cancelled.\n",
            (Locale::ZhCn, FailureKind::Cancelled) => "终端流操作已取消。\n",
            (Locale::EnUs, FailureKind::TransportLoss) => {
                "The terminal stream connection was lost.\n"
            }
            (Locale::ZhCn, FailureKind::TransportLoss) => "终端流连接已中断。\n",
            (Locale::EnUs, FailureKind::OutputTooLarge) => {
                "A terminal frame exceeded the output limit.\n"
            }
            (Locale::ZhCn, FailureKind::OutputTooLarge) => "终端帧超过输出上限。\n",
            (Locale::EnUs, FailureKind::InternalFailure) => {
                "The terminal stream failed internally. Previously delivered input is not undone.\n"
            }
            (Locale::ZhCn, FailureKind::InternalFailure) => {
                "终端流发生内部错误。此前已交付的输入不会被撤销。\n"
            }
            (Locale::EnUs, FailureKind::OutcomeUnknown) => {
                "The latest terminal operation could not be confirmed. Input was not replayed.\n"
            }
            (Locale::ZhCn, FailureKind::OutcomeUnknown) => {
                "最近一次终端操作的结果无法确认。未重放输入。\n"
            }
            _ => "",
        };
        if !message.is_empty() {
            return message.to_owned();
        }
        if failure.kind == crate::cli_outcome::FailureKind::Rejected {
            let reason = failure
                .details
                .as_ref()
                .and_then(|v| v.get("reason"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("rejected");
            return match self.locale {
                Locale::EnUs => format!(
                    "Terminal attachment or control was refused ({reason}). Previously delivered input is not undone.\n"
                ),
                Locale::ZhCn => {
                    format!("终端接入或控制被拒绝（{reason}）。此前已交付的输入不会被撤销。\n")
                }
            };
        }
        self.cli_failure(failure)
    }
}
