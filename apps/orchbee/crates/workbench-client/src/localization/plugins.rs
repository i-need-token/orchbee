//! Shared native-surface messages, without CLI grammar or result dependencies.
use super::Locale;
impl Locale {
    pub(crate) const fn plugin_popup_pending(self, closing: bool) -> &'static str {
        match (self, closing) {
            (Self::EnUs, false) => "Opening…",
            (Self::EnUs, true) => "Closing…",
            (Self::ZhCn, false) => "正在打开…",
            (Self::ZhCn, true) => "正在关闭…",
        }
    }

    pub(crate) const fn plugin_popup_resize_recovery(self) -> &'static str {
        match self {
            Self::EnUs => "Enlarge the terminal to restore the popup.",
            Self::ZhCn => "请放大终端以恢复 popup。",
        }
    }
}
