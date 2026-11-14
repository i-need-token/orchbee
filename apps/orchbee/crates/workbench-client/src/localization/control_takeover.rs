use super::Locale;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TakeoverStatus {
    Pending,
    Refused,
    Unknown,
    Granted,
    WaitingForFrame,
    Lost,
}

impl Locale {
    /// A takeover that succeeds says nothing; the window simply types and sizes the terminal.
    pub(crate) const fn takeover_status(self, status: TakeoverStatus) -> &'static str {
        match (self, status) {
            (_, TakeoverStatus::Pending | TakeoverStatus::WaitingForFrame) => {
                self.connecting_status()
            }
            (Self::EnUs, TakeoverStatus::Refused) => "Couldn't type here.",
            (Self::ZhCn, TakeoverStatus::Refused) => "没能在这里输入。",
            (_, TakeoverStatus::Unknown | TakeoverStatus::Granted) => self.outcome_unknown(),
            (_, TakeoverStatus::Lost) => self.another_window_typing(),
        }
    }

    /// Another window holds the terminal's input and size.
    pub(crate) const fn another_window_typing(self) -> &'static str {
        match self {
            Self::EnUs => "Another window is typing here.",
            Self::ZhCn => "另一个窗口正在这里输入。",
        }
    }

    /// This window shows the terminal but does not type into it.
    pub(crate) const fn view_only(self) -> &'static str {
        match self {
            Self::EnUs => "View only",
            Self::ZhCn => "仅查看",
        }
    }
}
