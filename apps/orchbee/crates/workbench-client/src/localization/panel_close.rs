use super::Locale;

impl Locale {
    pub(crate) const fn panel_close_cancel_label(self) -> &'static str {
        match self {
            Self::EnUs => "Cancel",
            Self::ZhCn => "取消",
        }
    }

    pub(crate) const fn panel_close_confirm_label(self) -> &'static str {
        match self {
            Self::EnUs => "Close",
            Self::ZhCn => "关闭",
        }
    }

    /// The Panel the close targets, as its frame names it.
    pub(crate) fn panel_close_target(self, panel_position: usize, panel_label: &str) -> String {
        let panel = crate::terminal_text::sanitize_terminal_text(panel_label);
        format!("{} {panel_position} · {panel}", self.shell().panel_title)
    }

    /// The Tab the Panel belongs to, as a context line.
    pub(crate) fn panel_close_tab(self, tab_label: &str) -> String {
        let tab = crate::terminal_text::sanitize_terminal_text(tab_label);
        match self {
            Self::EnUs => format!("Tab: {tab}"),
            Self::ZhCn => format!("标签页：{tab}"),
        }
    }

    pub(crate) const fn panel_close_title(self) -> &'static str {
        match self {
            Self::EnUs => "Close Panel?",
            Self::ZhCn => "关闭面板？",
        }
    }

    /// Closing the Tab's last Panel also closes the Tab.
    pub(crate) const fn panel_close_last_panel(self) -> &'static str {
        match self {
            Self::EnUs => "This is the Tab's last Panel, so the Tab closes too.",
            Self::ZhCn => "这是标签页的最后一个面板，标签页也会一起关闭。",
        }
    }
}
