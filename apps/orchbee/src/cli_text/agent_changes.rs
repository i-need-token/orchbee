//! Localized Agent change-stream progress and checkpoint feedback; does not read events.

use super::Locale;

impl super::CliText {
    pub(crate) const fn cli_agent_change_headers(self) -> [&'static str; 4] {
        match self.locale {
            Locale::EnUs => ["REVISION", "SESSION", "STATUS", "FRESHNESS"],
            Locale::ZhCn => ["修订", "会话", "状态", "新鲜度"],
        }
    }

    pub(crate) fn cli_agent_changes_progress(self, observed: u64, head: u64) -> String {
        match self.locale {
            Locale::EnUs => format!("Observed through revision {observed}; sampled head {head}.\n"),
            Locale::ZhCn => format!("已观察至修订 {observed}；采样头部为 {head}。\n"),
        }
    }

    pub(crate) fn cli_agent_changes_checkpoint(self, revision: u64, reset: bool) -> String {
        match (self.locale, reset) {
            (Locale::EnUs, false) => format!("Current through revision {revision}.\n"),
            (Locale::ZhCn, false) => format!("已观察至修订 {revision}。\n"),
            (Locale::EnUs, true) => {
                format!("History lost; rebuild the baseline at revision {revision}.\n")
            }
            (Locale::ZhCn, true) => format!("历史已丢失；请在修订 {revision} 重建基线。\n"),
        }
    }
}
