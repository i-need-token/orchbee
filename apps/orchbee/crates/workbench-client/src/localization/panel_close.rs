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

    pub(crate) fn panel_close_targets(
        self,
        panel_position: usize,
        tab_position: usize,
        panel_label: &str,
        tab_label: &str,
    ) -> [String; 2] {
        let panel = crate::terminal_text::sanitize_terminal_text(panel_label);
        let tab = crate::terminal_text::sanitize_terminal_text(tab_label);
        [
            format!("{} {panel_position} · {panel}", self.shell().panel_title),
            format!("Tab {tab_position} · {tab}"),
        ]
    }

    pub(crate) const fn panel_close_title(self) -> &'static str {
        match self {
            Self::EnUs => "Close terminal Panel?",
            Self::ZhCn => "关闭终端面板？",
        }
    }

    pub(crate) const fn panel_close_warning(self, dissolves_tab: bool) -> &'static str {
        match (self, dissolves_tab) {
            (Self::EnUs, false) => {
                "Closing this Panel ends its execution for all attached Clients."
            }
            (Self::ZhCn, false) => "关闭此面板将终止其执行，影响所有已连接的客户端。",
            (Self::EnUs, true) => {
                "This is the Tab's last Panel; closing it ends the Tab for all attached Clients."
            }
            (Self::ZhCn, true) => {
                "此面板是标签页的最后一个面板；关闭它将结束该标签页，影响所有已连接的客户端。"
            }
        }
    }
}
