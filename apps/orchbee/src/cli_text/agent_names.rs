//! Localized Agent naming results; does not allocate or modify names.

use super::Locale;

impl super::CliText {
    pub(crate) const fn cli_agent_names_header(self, terminal_targets: bool) -> &'static str {
        match (self.locale, terminal_targets) {
            (Locale::EnUs, true) => "NAME\tTARGET\n",
            (Locale::ZhCn, true) => "名称\t目标\n",
            (Locale::EnUs, false) => "NAME\tSESSION\n",
            (Locale::ZhCn, false) => "名称\t会话\n",
        }
    }

    pub(crate) fn cli_agent_terminal_name_target(self, presentation: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("terminal:{presentation}"),
            Locale::ZhCn => format!("终端:{presentation}"),
        }
    }

    pub(crate) fn cli_agent_name_assigned(self, name: &str, target: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("{name} -> {target}\n"),
            Locale::ZhCn => format!("{name} → {target}\n"),
        }
    }

    pub(crate) fn cli_agent_name_cleared(self, target: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("{target}: name cleared\n"),
            Locale::ZhCn => format!("{target}：名称已清除\n"),
        }
    }

    pub(crate) const fn cli_agent_pending_names_header(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "\nPENDING NAME\tPROVIDER\tREMAINING MS\tRESERVATION\n",
            Locale::ZhCn => "\n预留名称\t提供方\t剩余毫秒\t预留 ID\n",
        }
    }
}
