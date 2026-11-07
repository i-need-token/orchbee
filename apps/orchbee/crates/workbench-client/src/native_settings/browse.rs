//! A deterministic category/search projection; values and persistence never enter the index.
use super::{Page, Row, SettingCategory, metadata::SETTINGS};
use crate::localization::Locale;
use orchbee_agent_integration::IntegrationId;

/// One list landmark or semantic setting; headings never acquire activation identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Entry {
    Heading(SettingCategory),
    Setting(Row),
}

/// `integrations` admits the Agent integrations entry, which only a local origin offers.
pub(super) fn entries(page: Page, query: &str, integrations: bool) -> Vec<Entry> {
    match page {
        Page::Profiles => {
            return page
                .content_rows()
                .iter()
                .copied()
                .map(Entry::Setting)
                .collect();
        }
        Page::Integrations => {
            let mut entries = vec![Entry::Heading(SettingCategory::LocalAgentAdapters)];
            entries.extend(IntegrationId::ALL.map(|id| Entry::Setting(Row::Agent(id))));
            entries.extend([
                Entry::Heading(SettingCategory::RemoteAgentHosts),
                Entry::Setting(Row::RemoteHosts),
            ]);
            return entries;
        }
        Page::Diagnosis(_) => return Vec::new(),
        Page::General => {}
    }
    let terms: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
    let matches = |row: Row, category: SettingCategory, extra: &str| {
        let document = [Locale::EnUs, Locale::ZhCn]
            .map(|locale| {
                format!(
                    "{} {} {} {extra}",
                    locale.settings_row(row),
                    locale.settings_category_name(category),
                    locale.settings_description(row)
                )
            })
            .join(" ")
            .to_lowercase();
        terms.iter().all(|term| document.contains(term))
    };
    let mut entries = Vec::new();
    for category in SettingCategory::ALL {
        let rows: Vec<_> = if *category == SettingCategory::AgentIntegrations {
            // Agent names find the page that manages them, as the GUI's per-Agent entries do.
            let names = IntegrationId::ALL
                .map(|id| Locale::EnUs.agent_integration_name(id))
                .join(" ");
            (integrations && matches(Row::AgentIntegrations, *category, &names))
                .then_some(Row::AgentIntegrations)
                .into_iter()
                .collect()
        } else {
            SETTINGS
                .iter()
                .filter(|setting| setting.category == *category)
                .filter(|setting| matches(setting.row, setting.category, ""))
                .map(|setting| setting.row)
                .collect()
        };
        if !rows.is_empty() {
            entries.push(Entry::Heading(*category));
            entries.extend(rows.into_iter().map(Entry::Setting));
        }
    }
    entries
}
