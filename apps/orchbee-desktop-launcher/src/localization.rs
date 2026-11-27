//! Every message the window launcher shows, in `en-US` and `zh-CN`. On Windows the locale follows
//! the Windows display language, as Setup chooses its own, because a window started from a
//! shortcut has no shell locale; on Linux it follows `LC_ALL`, `LC_MESSAGES` or `LANG`. Both are
//! read only when a message is shown. The macOS launcher shows nothing: it runs only for a window
//! that asked it to, and its last step starts the window that asked again.
#[cfg(any(windows, target_os = "linux"))]
use crate::decision::Failure;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Locale {
    EnUs,
    ZhCn,
}

/// The locale a language tag selects: any `zh` tag, Traditional Chinese included, is `zh-CN`;
/// anything else, or no tag, is `en-US`.
pub(crate) fn locale(tag: Option<&str>) -> Locale {
    let chinese = tag
        .and_then(|text| text.get(..2))
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("zh"));
    if chinese { Locale::ZhCn } else { Locale::EnUs }
}

/// The caption of the launcher's message box: the product name, the same in every locale.
#[cfg(windows)]
pub(crate) const CAPTION: &str = "Orchbee";

/// The message shown when no window starts, the command launcher's message for the same failure.
#[cfg(any(windows, target_os = "linux"))]
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
        (Failure::ClientUnavailable(window), Locale::EnUs) => format!(
            "Cannot start the active Orchbee version: {} is missing or cannot run. Reinstall Orchbee.",
            window.display()
        ),
        (Failure::ClientUnavailable(window), Locale::ZhCn) => format!(
            "无法启动当前 Orchbee 版本：{} 缺失或无法运行。请重新安装 Orchbee。",
            window.display()
        ),
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(crate) fn unsupported_platform(locale: Locale) -> &'static str {
    match locale {
        Locale::EnUs => "The Orchbee window launcher runs only on Windows, macOS and Linux.",
        Locale::ZhCn => "Orchbee 窗口启动器只在 Windows、macOS 和 Linux 上运行。",
    }
}
