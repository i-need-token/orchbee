use super::Locale;
use crate::terminal_copy_mode::CopyModeStatus;

impl Locale {
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
    pub(crate) const fn copy_mode_hint(self, searching: bool, full: bool) -> &'static str {
        match (self, searching, full) {
            (Self::EnUs, true, _) => "Enter search · Esc exit",
            (Self::ZhCn, true, _) => "Enter 搜索 · Esc 退出",
            (Self::EnUs, false, true) => "arrows · v select · y copy · / search · F1 · Esc exit",
            (Self::ZhCn, false, true) => "方向键 · v 选择 · y 复制 · / 搜索 · F1 · Esc 退出",
            (Self::EnUs, false, false) => "v · y · / · F1 · Esc exit",
            (Self::ZhCn, false, false) => "v · y · / · F1 · Esc 退出",
        }
    }

    pub(crate) const fn copy_mode_exit_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Esc exit",
            Self::ZhCn => "Esc 退出",
        }
    }
}
