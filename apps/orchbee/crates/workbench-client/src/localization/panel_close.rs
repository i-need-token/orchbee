use super::{CloseImpact, Locale};

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

    /// Closing the Tab's last Panel names the Tab it ends, which holds whatever its terminals
    /// ran; any other Panel names what it ends from its terminals' `impact`.
    pub(crate) fn panel_close_warning(self, dissolves_tab: bool, impact: CloseImpact) -> String {
        match (self, dissolves_tab, impact.running, impact.ended) {
            (Self::EnUs, true, ..) => {
                "This is the Tab's last Panel; closing it ends the Tab for all attached Clients.".into()
            }
            (Self::ZhCn, true, ..) => {
                "此面板是标签页的最后一个面板；关闭它将结束该标签页，影响所有已连接的客户端。".into()
            }
            (Self::EnUs, false, _, 0) => {
                "Closing this Panel ends its execution for all attached Clients.".into()
            }
            (Self::ZhCn, false, _, 0) => "关闭此面板将终止其执行，影响所有已连接的客户端。".into(),
            (Self::EnUs, false, 0, _) => {
                "Closing this Panel discards its retained output for all attached Clients.".into()
            }
            (Self::ZhCn, false, 0, _) => "关闭此面板将丢弃其保留的输出，影响所有已连接的客户端。".into(),
            (Self::EnUs, false, 1, _) => "Closing this Panel ends the running work in 1 terminal and discards the retained output for all attached Clients.".into(),
            (Self::EnUs, false, running, _) => format!("Closing this Panel ends the running work in {running} terminals and discards the retained output for all attached Clients."),
            (Self::ZhCn, false, running, _) => format!("关闭此面板将终止 {running} 个终端中运行的任务，并丢弃保留的输出，影响所有已连接的客户端。"),
        }
    }
}
