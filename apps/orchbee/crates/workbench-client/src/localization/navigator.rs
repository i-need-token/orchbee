//! Structured Navigator copy; renderers pair these verbs with typed bindings.

use super::Locale;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NavigatorText {
    pub(crate) return_action: &'static str,
    pub(crate) open_action: &'static str,
    pub(crate) select_action: &'static str,
    pub(crate) column_action: &'static str,
    pub(crate) ends_action: &'static str,
    pub(crate) page_action: &'static str,
}

const EN_US: NavigatorText = NavigatorText {
    return_action: "return",
    open_action: "open",
    select_action: "select",
    column_action: "switch column",
    ends_action: "first / last",
    page_action: "page",
};

const ZH_CN: NavigatorText = NavigatorText {
    return_action: "返回",
    open_action: "打开",
    select_action: "选择",
    column_action: "切换列",
    ends_action: "首项 / 末项",
    page_action: "翻页",
};

impl Locale {
    pub(crate) const fn navigator(self) -> NavigatorText {
        match self {
            Self::EnUs => EN_US,
            Self::ZhCn => ZH_CN,
        }
    }
}
