//! Localized retained-history navigation and live-crop feedback; does not read terminal history.

use super::Locale;

impl Locale {
    /// The loading indicator while entry or a row window is still in
    /// flight — no retained content may show yet.
    pub(crate) const fn terminal_history_loading(self) -> &'static str {
        match self {
            Self::EnUs => "Loading history…",
            Self::ZhCn => "正在加载历史…",
        }
    }

    /// The recovering indicator while the core returns the pane to live.
    pub(crate) const fn terminal_history_recovering(self) -> &'static str {
        match self {
            Self::EnUs => "Returning to live…",
            Self::ZhCn => "正在返回实时…",
        }
    }

    /// Live viewing is clipped locally; the physical terminal keeps its controller's size.
    pub(crate) const fn terminal_live_crop(self) -> &'static str {
        match self {
            Self::EnUs => "Live crop",
            Self::ZhCn => "实时裁切",
        }
    }

    /// Complete compact clipping and history-entry instruction for a narrow live viewer.
    pub(crate) const fn terminal_live_crop_compact(self) -> &'static str {
        match self {
            Self::EnUs => "Crop: scroll up",
            Self::ZhCn => "裁切：向上滚动",
        }
    }
}
