//! Localized retained-history navigation and live-crop feedback; does not read terminal history.

use super::Locale;

impl Locale {
    #[cfg(any(unix, windows))]
    /// The right-edge badge a pane carries while its retained-history core
    /// owns the leaf.
    pub(crate) const fn terminal_history_badge(self) -> &'static str {
        match self {
            Self::EnUs => "HISTORY",
            Self::ZhCn => "历史",
        }
    }

    #[cfg(any(unix, windows))]
    /// The loading indicator while entry or a row window is still in
    /// flight — no retained content may show yet.
    pub(crate) const fn terminal_history_loading(self) -> &'static str {
        match self {
            Self::EnUs => "Loading history…",
            Self::ZhCn => "正在加载历史…",
        }
    }

    #[cfg(any(unix, windows))]
    /// The recovering indicator while the core returns the pane to live.
    pub(crate) const fn terminal_history_recovering(self) -> &'static str {
        match self {
            Self::EnUs => "Returning to live…",
            Self::ZhCn => "正在返回实时…",
        }
    }

    #[cfg(any(unix, windows))]
    /// The retained-window position readout: the window's first retained
    /// row index out of the total retained rows the host reported.
    pub(crate) fn terminal_history_position(self, start_row: u64, total_rows: u64) -> String {
        match self {
            Self::EnUs => format!("Row {start_row} of {total_rows}"),
            Self::ZhCn => format!("第 {start_row} 行 · 共 {total_rows} 行"),
        }
    }

    #[cfg(any(unix, windows))]
    /// The persistent bottom-line hint for leaving history and returning
    /// to live output.
    pub(crate) const fn terminal_history_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Esc back to live",
            Self::ZhCn => "Esc 返回实时",
        }
    }

    #[cfg(any(unix, windows))]
    /// Live viewing is clipped locally; the physical terminal keeps its controller's size.
    pub(crate) const fn terminal_live_crop(self) -> &'static str {
        match self {
            Self::EnUs => "Live crop",
            Self::ZhCn => "实时裁切",
        }
    }

    #[cfg(any(unix, windows))]
    /// Complete compact clipping and history-entry instruction for a narrow live viewer.
    pub(crate) const fn terminal_live_crop_compact(self) -> &'static str {
        match self {
            Self::EnUs => "Crop: scroll up",
            Self::ZhCn => "裁切：向上滚动",
        }
    }

    #[cfg(any(unix, windows))]
    /// Short action label paired with the effective history-entry binding.
    pub(crate) const fn terminal_view_history(self) -> &'static str {
        match self {
            Self::EnUs => "History",
            Self::ZhCn => "历史",
        }
    }
}
