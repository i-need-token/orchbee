//! Formats terminal-clear receipts without applying a clear operation or replaying unknown effects.
use super::Locale;
use orchbee_client_server_protocol::TerminalControlOutcome;

impl super::CliText {
    pub(crate) fn cli_terminal_clear(
        self,
        terminal: &str,
        outcome: &TerminalControlOutcome,
    ) -> String {
        let message = match (self.locale, outcome) {
            (
                Locale::EnUs,
                TerminalControlOutcome::Cleared {
                    cleanup_confirmed: true,
                    ..
                },
            ) => "Output cleared; prompt preserved.",
            (
                Locale::ZhCn,
                TerminalControlOutcome::Cleared {
                    cleanup_confirmed: true,
                    ..
                },
            ) => "输出已清空，提示行已保留。",
            (Locale::EnUs, TerminalControlOutcome::Cleared { .. }) => {
                "Output cleared; attachment cleanup unconfirmed. Do not repeat the clear."
            }
            (Locale::ZhCn, TerminalControlOutcome::Cleared { .. }) => {
                "输出已清空，附件清理尚未确认。请勿重复清空。"
            }
            (Locale::EnUs, TerminalControlOutcome::OutcomeUnknown {}) => {
                "Clear outcome unknown. Do not repeat the clear."
            }
            (Locale::ZhCn, TerminalControlOutcome::OutcomeUnknown {}) => {
                "清空结果未知。请勿重复清空。"
            }
            (
                Locale::EnUs,
                TerminalControlOutcome::Rejected {
                    reason:
                        orchbee_client_server_protocol::ControlRejectedReason::HostClearAlternateScreen,
                },
            ) => "Clear refused: terminal is using an alternate screen.",
            (
                Locale::ZhCn,
                TerminalControlOutcome::Rejected {
                    reason:
                        orchbee_client_server_protocol::ControlRejectedReason::HostClearAlternateScreen,
                },
            ) => "清空被拒绝：终端正在使用备用屏幕。",
            (Locale::EnUs, TerminalControlOutcome::Rejected { .. }) => {
                "Clear refused; no clear applied."
            }
            (Locale::ZhCn, TerminalControlOutcome::Rejected { .. }) => "清空被拒绝，未执行清空。",
            (Locale::EnUs, _) => "Terminal clear unavailable.",
            (Locale::ZhCn, _) => "终端清空不可用。",
        };
        format!(
            "{}: {message}\n",
            crate::cli_text::escape::escape(&crate::cli::handles::terminal(terminal))
        )
    }
}
pub(super) const HELP_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee terminal clear <terminal> [--format json|text]",
    "Clear history and other output, keeping the line the cursor is on. On Windows only the cursor's row stays, so a prompt that wraps keeps its last row; elsewhere the whole line stays, wrapped rows included. Refuses alternate screens. Requires an existing session and an exact bound primary or child terminal; no controller takeover, resize or PTY input. Unknown effects must not be repeated.",
);
pub(super) const HELP_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee terminal clear <terminal> [--format json|text]",
    "清除历史及其他输出，保留光标所在的行。在 Windows 上只保留光标所在的那一屏幕行，因此折行显示的提示符只留下最后一行；在其他平台上保留整行，包括折行的部分。备用屏幕会被拒绝。需要已有会话和精确绑定的主终端或子终端；不接管控制权、不调整尺寸、不发送 PTY 输入。请勿重复结果未知的清空。",
);
