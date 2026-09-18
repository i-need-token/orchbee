use super::Locale;

impl Locale {
    pub(crate) const fn panel_close_title(self) -> &'static str {
        match self {
            Self::EnUs => "Close terminal Panel?",
            Self::ZhCn => "关闭终端 Panel？",
        }
    }

    pub(crate) const fn panel_close_warning(self, dissolves_tab: bool) -> &'static str {
        match (self, dissolves_tab) {
            (Self::EnUs, false) => {
                "Closing this Panel ends its execution for all attached Clients."
            }
            (Self::ZhCn, false) => "关闭此 Panel 将终止其执行，影响所有已连接的 Client。",
            (Self::EnUs, true) => {
                "This is the Tab's last Panel; closing it ends the Tab for all attached Clients."
            }
            (Self::ZhCn, true) => {
                "此 Panel 是 Tab 的最后一个 Panel；关闭它将结束该 Tab，影响所有已连接的 Client。"
            }
        }
    }

    pub(crate) const fn panel_close_actions(self) -> &'static str {
        match self {
            Self::EnUs => "y · confirm    Esc · cancel",
            Self::ZhCn => "y · 确认    Esc · 取消",
        }
    }

    pub(crate) const fn panel_close_resize_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Enlarge the terminal to read the confirmation. Esc · cancel",
            Self::ZhCn => "请扩大终端以阅读完整确认内容。Esc · 取消",
        }
    }
}
