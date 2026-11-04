//! Localized retained-history navigation and live-crop feedback; does not read terminal history.

use super::Locale;

impl Locale {
    /// The right-edge badge a pane carries while its retained-history core
    /// owns the leaf.
    pub(crate) const fn terminal_history_badge(self) -> &'static str {
        match self {
            Self::EnUs => "HISTORY",
            Self::ZhCn => "历史",
        }
    }

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

    /// Short action label paired with the effective history-entry binding.
    pub(crate) const fn terminal_view_history(self) -> &'static str {
        match self {
            Self::EnUs => "History",
            Self::ZhCn => "历史",
        }
    }
}
