use super::Locale;

impl Locale {
    pub(crate) const fn rename_cancel_label(self) -> &'static str {
        match self {
            Self::EnUs => "Cancel",
            Self::ZhCn => "取消",
        }
    }
    pub(crate) const fn rename_submit_label(self) -> &'static str {
        match self {
            Self::EnUs => "Rename",
            Self::ZhCn => "重命名",
        }
    }
    pub(crate) const fn rename_title(self) -> &'static str {
        match self {
            Self::EnUs => "Rename Tab",
            Self::ZhCn => "重命名标签页",
        }
    }
}
