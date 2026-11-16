use super::Locale;

impl Locale {
    /// The Update control's label on the Settings row.
    pub(crate) const fn update_control(self) -> &'static str {
        match self {
            Self::EnUs => "Update",
            Self::ZhCn => "更新",
        }
    }

    /// The Update control's effect line: its tooltip, and the sentence its action menu row carries.
    pub(crate) fn update_tooltip(self, version: &str) -> String {
        match self {
            Self::EnUs => format!("Switch to Orchbee {version}. Terminals keep running."),
            Self::ZhCn => format!("切换到 Orchbee {version}，终端继续运行。"),
        }
    }

    /// The one line a TUI started without the launcher prints when it leaves for `version`.
    pub(crate) fn update_without_launcher(self, version: &str) -> String {
        match self {
            Self::EnUs => format!("Run orchbee to open version {version}.\n"),
            Self::ZhCn => format!("运行 orchbee 打开 {version} 版本。\n"),
        }
    }
}
