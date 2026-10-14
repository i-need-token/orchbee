//! Shared, key-free presentation feedback for geometry-blocked TUI surfaces.

use super::Locale;

#[derive(Clone, Copy)]
pub(crate) enum RecoveryVerb {
    Cancel,
    Close,
    #[cfg(windows)]
    Hide,
}

impl Locale {
    pub(crate) const fn recovery_verb(self, verb: RecoveryVerb) -> &'static str {
        match (self, verb) {
            (Self::EnUs, RecoveryVerb::Cancel) => "Cancel",
            (Self::ZhCn, RecoveryVerb::Cancel) => "取消",
            (Self::EnUs, RecoveryVerb::Close) => "Close",
            (Self::ZhCn, RecoveryVerb::Close) => "关闭",
            #[cfg(windows)]
            (Self::EnUs, RecoveryVerb::Hide) => "Hide",
            #[cfg(windows)]
            (Self::ZhCn, RecoveryVerb::Hide) => "隐藏",
        }
    }

    pub(crate) const fn presentation_notice(self, recovery: bool) -> &'static str {
        match (self, recovery) {
            (Self::EnUs, false) => "Cannot open here: enlarge the terminal.",
            (Self::ZhCn, false) => "无法在此处打开：请放大终端。",
            (Self::EnUs, true) => "Paused: enlarge the terminal.",
            (Self::ZhCn, true) => "已暂停：请放大终端。",
        }
    }
}
