//! Every human-facing string the Server writes into terminal content. The Server renders it in
//! the person's configured language, so every Client shows the same text.

use orchbee_workbench_protocol::PreferencesLocale;
use std::ffi::OsString;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Locale {
    EnUs,
    ZhCn,
}

impl Locale {
    /// The configured language; System reads `LC_ALL`, `LC_MESSAGES` and `LANG` in that order,
    /// ignoring empty values, and falls back to `en-US`.
    pub(crate) fn configured(
        preference: PreferencesLocale,
        mut read_environment: impl FnMut(&str) -> Option<OsString>,
    ) -> Self {
        match preference {
            PreferencesLocale::EnUs => Self::EnUs,
            PreferencesLocale::ZhCn => Self::ZhCn,
            PreferencesLocale::System => ["LC_ALL", "LC_MESSAGES", "LANG"]
                .into_iter()
                .filter_map(&mut read_environment)
                .find(|value| !value.is_empty())
                .map_or(Self::EnUs, |value| {
                    let value = value.to_string_lossy();
                    let bytes = value.as_bytes();
                    if bytes.len() >= 2 && bytes[..2].eq_ignore_ascii_case(b"zh") {
                        Self::ZhCn
                    } else {
                        Self::EnUs
                    }
                }),
        }
    }
}

/// The line that follows replayed output, naming when that output was kept. A host without a
/// local clock shows the line without the time.
pub(crate) fn output_before_restart(
    locale: Locale,
    saved: Option<orchbee_local_runtime::LocalClock>,
) -> String {
    let time = saved.map(|clock| {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            clock.year, clock.month, clock.day, clock.hour, clock.minute
        )
    });
    match (locale, time) {
        (Locale::EnUs, Some(time)) => format!("Output before restart ({time})"),
        (Locale::EnUs, None) => "Output before restart".to_owned(),
        (Locale::ZhCn, Some(time)) => format!("以上为重启前的输出（{time}）"),
        (Locale::ZhCn, None) => "以上为重启前的输出".to_owned(),
    }
}
