//! Every message the launcher prints, in `en-US` and `zh-CN`. The locale comes from `LC_ALL`,
//! `LC_MESSAGES` or `LANG`, read only when a message is printed.
#[cfg(windows)]
use crate::resolve::Failure;
use std::ffi::OsString;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Locale {
    EnUs,
    ZhCn,
}

/// The first non-empty locale variable selects the locale: any `zh` value, Traditional Chinese
/// included, is `zh-CN`; anything else, a non-Unicode value, or no value is `en-US`.
pub(crate) fn locale(read: impl Fn(&str) -> Option<OsString>) -> Locale {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = read(name).filter(|value| !value.is_empty()) else {
            continue;
        };
        let chinese = value
            .to_str()
            .and_then(|text| text.get(..2))
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("zh"));
        return if chinese { Locale::ZhCn } else { Locale::EnUs };
    }
    Locale::EnUs
}

/// The one line printed when no Client starts.
#[cfg(windows)]
pub(crate) fn failure(locale: Locale, failure: &Failure) -> String {
    match (failure, locale) {
        (Failure::NoActiveVersion(pointer), Locale::EnUs) => format!(
            "Orchbee has no active version: {} is missing or unreadable. Reinstall Orchbee.",
            pointer.display()
        ),
        (Failure::NoActiveVersion(pointer), Locale::ZhCn) => format!(
            "Orchbee 没有当前版本：{} 缺失或无法读取。请重新安装 Orchbee。",
            pointer.display()
        ),
        (Failure::ClientUnavailable(client), Locale::EnUs) => format!(
            "Cannot start the active Orchbee version: {} is missing or cannot run. Reinstall Orchbee.",
            client.display()
        ),
        (Failure::ClientUnavailable(client), Locale::ZhCn) => format!(
            "无法启动当前 Orchbee 版本：{} 缺失或无法运行。请重新安装 Orchbee。",
            client.display()
        ),
    }
}

#[cfg(not(windows))]
pub(crate) fn unsupported_platform(locale: Locale) -> &'static str {
    match locale {
        Locale::EnUs => "The Orchbee launcher runs only on Windows.",
        Locale::ZhCn => "Orchbee 启动器只在 Windows 上运行。",
    }
}
