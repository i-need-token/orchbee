//! Localized Agent naming results; does not allocate or modify names.

use super::Locale;

impl super::CliText {
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
}
