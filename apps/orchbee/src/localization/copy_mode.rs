use super::Locale;
use crate::{client_view::CopyHintLabel, terminal_copy_mode::CopyModeStatus};

impl Locale {
    pub(crate) const fn copy_action_label(self) -> &'static str {
        match self {
            Self::EnUs => "Copy",
            Self::ZhCn => "复制",
        }
    }
    pub(crate) const fn copy_select_label(self) -> &'static str {
        match self {
            Self::EnUs => "Select",
            Self::ZhCn => "选择",
        }
    }
    pub(crate) const fn copy_search_label(self) -> &'static str {
        match self {
            Self::EnUs => "Search",
            Self::ZhCn => "搜索",
        }
    }
    pub(crate) const fn copy_find_label(self) -> &'static str {
        match self {
            Self::EnUs => "Find",
            Self::ZhCn => "查找",
        }
    }
    pub(crate) const fn copy_back_label(self) -> &'static str {
        match self {
            Self::EnUs => "Back",
            Self::ZhCn => "返回",
        }
    }
    pub(crate) const fn copy_reverse_label(self) -> &'static str {
        match self {
            Self::EnUs => "Reverse",
            Self::ZhCn => "反转",
        }
    }
    pub(crate) const fn copy_help_label(self) -> &'static str {
        match self {
            Self::EnUs => "Help",
            Self::ZhCn => "帮助",
        }
    }
    pub(crate) const fn copy_mode_label(
        self,
        status: CopyModeStatus,
        selecting: bool,
    ) -> &'static str {
        use CopyModeStatus as S;
        match (self, status, selecting) {
            (Self::EnUs, S::Entering, _) => "COPY loading",
            (Self::EnUs, S::Selecting, _) => "COPY selecting pending",
            (Self::EnUs, S::Moving, _) => "COPY moving",
            (Self::EnUs, S::Querying, _) => "COPY reading",
            (Self::EnUs, S::Exporting, _) => "COPY exporting",
            (Self::EnUs, S::Clipboard, _) => "COPY pending",
            (Self::EnUs, S::Ready, true) => "COPY selecting",
            (Self::EnUs, S::Ready, false) => "COPY",
            (Self::ZhCn, S::Entering, _) => "复制加载中",
            (Self::ZhCn, S::Selecting, _) => "复制选区待定",
            (Self::ZhCn, S::Moving, _) => "复制移动中",
            (Self::ZhCn, S::Querying, _) => "复制读取中",
            (Self::ZhCn, S::Exporting, _) => "复制导出中",
            (Self::ZhCn, S::Clipboard, _) => "复制待定",
            (Self::ZhCn, S::Ready, true) => "复制选择中",
            (Self::ZhCn, S::Ready, false) => "复制",
        }
    }
    pub(crate) const fn copy_hint_label(self, label: CopyHintLabel) -> &'static str {
        match label {
            CopyHintLabel::Copy => self.copy_action_label(),
            CopyHintLabel::Select => self.copy_select_label(),
            CopyHintLabel::Search => self.copy_search_label(),
            CopyHintLabel::Help => self.copy_help_label(),
            CopyHintLabel::Find => self.copy_find_label(),
            CopyHintLabel::Back => self.copy_back_label(),
            CopyHintLabel::Reverse => self.copy_reverse_label(),
            CopyHintLabel::Exit => match self {
                Self::EnUs => "Exit",
                Self::ZhCn => "退出",
            },
            CopyHintLabel::Repeat => match self {
                Self::EnUs => "Repeat",
                Self::ZhCn => "重复",
            },
            CopyHintLabel::Move => match self {
                Self::EnUs => "Move",
                Self::ZhCn => "移动",
            },
        }
    }
}
