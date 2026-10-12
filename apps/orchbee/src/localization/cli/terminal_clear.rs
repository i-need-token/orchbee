use super::Locale;
use orchbee_client_server_protocol::TerminalControlOutcome;

impl Locale {
    pub(crate) fn cli_terminal_clear(
        self,
        terminal: &str,
        outcome: &TerminalControlOutcome,
    ) -> String {
        let message = match (self, outcome) {
            (
                Self::EnUs,
                TerminalControlOutcome::Cleared {
                    cleanup_confirmed: true,
                    ..
                },
            ) => "Output cleared; prompt preserved.",
            (
                Self::ZhCn,
                TerminalControlOutcome::Cleared {
                    cleanup_confirmed: true,
                    ..
                },
            ) => "输出已清空，提示行已保留。",
            (Self::EnUs, TerminalControlOutcome::Cleared { .. }) => {
                "Output cleared; attachment cleanup unconfirmed. Do not repeat the clear."
            }
            (Self::ZhCn, TerminalControlOutcome::Cleared { .. }) => {
                "输出已清空，附件清理尚未确认。请勿重复清空。"
            }
            (Self::EnUs, TerminalControlOutcome::OutcomeUnknown {}) => {
                "Clear outcome unknown. Do not repeat the clear."
            }
            (Self::ZhCn, TerminalControlOutcome::OutcomeUnknown {}) => {
                "清空结果未知。请勿重复清空。"
            }
            (
                Self::EnUs,
                TerminalControlOutcome::Rejected {
                    reason:
                        orchbee_client_server_protocol::ControlRejectedReason::HostClearAlternateScreen,
                },
            ) => "Clear refused: terminal is using an alternate screen.",
            (
                Self::ZhCn,
                TerminalControlOutcome::Rejected {
                    reason:
                        orchbee_client_server_protocol::ControlRejectedReason::HostClearAlternateScreen,
                },
            ) => "清空被拒绝：终端正在使用备用屏幕。",
            (Self::EnUs, TerminalControlOutcome::Rejected { .. }) => {
                "Clear refused; no clear applied."
            }
            (Self::ZhCn, TerminalControlOutcome::Rejected { .. }) => "清空被拒绝，未执行清空。",
            (Self::EnUs, _) => "Terminal clear unavailable.",
            (Self::ZhCn, _) => "终端清空不可用。",
        };
        format!("{terminal}: {message}\n")
    }
}
pub(super) const HELP_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee terminal clear <terminal> [--format json|text]",
    "Clear history and other output, preserving the complete prompt line. Refuses alternate screens. Requires an existing Site and an exact bound primary or child terminal; no controller takeover, resize or PTY input. Unknown effects must not be repeated.",
);
pub(super) const HELP_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee terminal clear <terminal> [--format json|text]",
    "清除历史及其他输出，保留完整提示行；备用屏幕会被拒绝。需要已有 Site 和精确绑定的主终端或子终端；不接管控制权、不调整尺寸、不发送 PTY 输入。请勿重复结果未知的清空。",
);
