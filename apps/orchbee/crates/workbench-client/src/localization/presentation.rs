//! Shared, key-free presentation feedback for geometry-blocked TUI surfaces.

use super::Locale;

#[derive(Clone, Copy)]
pub(crate) enum RecoveryVerb {
    Cancel,
    Close,
    Hide,
}

impl Locale {
    pub(crate) const fn recovery_verb(self, verb: RecoveryVerb) -> &'static str {
        match (self, verb) {
            (Self::EnUs, RecoveryVerb::Cancel) => "Cancel",
            (Self::ZhCn, RecoveryVerb::Cancel) => "取消",
            (Self::EnUs, RecoveryVerb::Close) => "Close",
            (Self::ZhCn, RecoveryVerb::Close) => "关闭",
            (Self::EnUs, RecoveryVerb::Hide) => "Hide",
            (Self::ZhCn, RecoveryVerb::Hide) => "隐藏",
        }
    }

    pub(crate) const fn presentation_notice(self, recovery: bool) -> &'static str {
        match (self, recovery) {
            (Self::EnUs, false) => "Couldn't open it here. Enlarge the terminal.",
            (Self::ZhCn, false) => "没能在这里打开。请放大终端。",
            (Self::EnUs, true) => "Paused: enlarge the terminal.",
            (Self::ZhCn, true) => "已暂停：请放大终端。",
        }
    }
}
