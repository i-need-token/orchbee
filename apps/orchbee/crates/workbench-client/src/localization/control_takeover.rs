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
            (_, TakeoverStatus::Lost) => self.send_typing_here(),
        }
    }

    /// What Type here does. A lost grant does not say who holds the terminal, so the TUI never
    /// claims that another window types here, as the product language reserves that sentence for
    /// a known holder.
    pub(crate) const fn send_typing_here(self) -> &'static str {
        match self {
            Self::EnUs => "Send your typing to this terminal.",
            Self::ZhCn => "把键盘输入发到这个终端。",
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
