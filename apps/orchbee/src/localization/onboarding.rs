//! Localized welcome guide actions and persistence feedback; does not own guide state.

use super::Locale;

impl Locale {
    pub(crate) fn onboarding_title(self) -> &'static str {
        match self {
            Self::EnUs => "Welcome to Orchbee",
            Self::ZhCn => "欢迎使用 Orchbee",
        }
    }

    pub(crate) fn onboarding_status(self, failed: bool) -> &'static str {
        match (self, failed) {
            (Self::EnUs, false) => "You can keep typing.",
            (Self::ZhCn, false) => "终端可继续输入",
            (Self::EnUs, true) => "Guide state unreadable.",
            (Self::ZhCn, true) => "引导状态读取失败",
        }
    }

    pub(crate) fn onboarding_label(self, action: crate::onboarding::Action) -> &'static str {
        use crate::onboarding::Action::*;
        match (self, action) {
            (Self::EnUs, Workspace) => "New workspace",
            (Self::ZhCn, Workspace) => "新建工作区",
            (Self::EnUs, Agent) => "Start an Agent",
            (Self::ZhCn, Agent) => "启动智能体",
            (Self::EnUs, Menu) => "All actions",
            (Self::ZhCn, Menu) => "全部操作",
            (Self::EnUs, Help) => "Help",
            (Self::ZhCn, Help) => "帮助",
            (Self::EnUs, Dismiss) => "Dismiss guide",
            (Self::ZhCn, Dismiss) => "关闭引导",
        }
    }
}
