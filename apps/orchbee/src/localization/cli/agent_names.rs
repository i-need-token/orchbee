//! Localized Agent naming results; does not allocate or modify names.

use super::Locale;

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
