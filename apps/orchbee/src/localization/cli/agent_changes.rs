//! Localized Agent change-stream progress and checkpoint feedback; does not read events.

use super::Locale;

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
