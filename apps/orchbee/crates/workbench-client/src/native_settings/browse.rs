//! A deterministic category/search projection; values and persistence never enter the index.
use super::{Page, Row, SettingCategory, metadata::SETTINGS};
use crate::localization::Locale;

/// One list landmark or semantic setting; headings never acquire activation identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Entry {
    Heading(SettingCategory),
    Setting(Row),
}

pub(super) fn entries(page: Page, query: &str) -> Vec<Entry> {
    if page == Page::Profiles {
        return page
            .content_rows()
            .iter()
            .copied()
            .map(Entry::Setting)
            .collect();
    }
    let terms: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut entries = Vec::new();
    for category in SettingCategory::ALL {
        let rows: Vec<_> = SETTINGS
            .iter()
            .filter(|setting| setting.category == *category)
            .filter(|setting| {
                let document = [Locale::EnUs, Locale::ZhCn]
                    .map(|locale| {
                        format!(
                            "{} {} {}",
                            locale.settings_row(setting.row),
                            locale.settings_category_name(setting.category),
                            locale.settings_description(setting.row)
                        )
                    })
                    .join(" ")
                    .to_lowercase();
                terms.iter().all(|term| document.contains(term))
            })
            .map(|setting| setting.row)
            .collect();
        if !rows.is_empty() {
            entries.push(Entry::Heading(*category));
            entries.extend(rows.into_iter().map(Entry::Setting));
        }
    }
    entries
}
