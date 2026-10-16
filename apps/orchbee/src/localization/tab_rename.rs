use super::Locale;

impl Locale {
    pub(crate) const fn rename_cancel_label(self) -> &'static str {
        match self {
            Self::EnUs => "Cancel",
            Self::ZhCn => "取消",
        }
    }
    pub(crate) const fn rename_cancel_hint(self) -> &'static str {
        match self {
            Self::EnUs => "cancel",
            Self::ZhCn => "取消",
        }
    }
    pub(crate) const fn rename_submit_label(self) -> &'static str {
        match self {
            Self::EnUs => "Rename",
            Self::ZhCn => "重命名",
        }
    }
    pub(crate) const fn rename_submit_hint(self) -> &'static str {
        match self {
            Self::EnUs => "rename",
            Self::ZhCn => "重命名",
        }
    }
    pub(crate) const fn rename_delete_hint(self) -> &'static str {
        match self {
            Self::EnUs => "delete",
            Self::ZhCn => "删除",
        }
    }
    pub(crate) const fn rename_clear_hint(self) -> &'static str {
        match self {
            Self::EnUs => "clear",
            Self::ZhCn => "清空",
        }
    }
    pub(crate) const fn rename_title(self) -> &'static str {
        match self {
            Self::EnUs => "Rename Tab",
            Self::ZhCn => "重命名标签页",
        }
    }

    pub(crate) const fn rename_label(self) -> &'static str {
        match self {
            Self::EnUs => "New label",
            Self::ZhCn => "新名称",
        }
    }

    pub(crate) const fn rename_warning(self) -> &'static str {
        match self {
            Self::EnUs => "The new Tab name is shared with everyone connected to this workspace.",
            Self::ZhCn => "新的标签页名称会同步给所有连接到此工作区的人。",
        }
    }

    pub(crate) const fn rename_fault(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Input could not be accepted. Press Backspace or Ctrl+U to clear the input error."
            }
            Self::ZhCn => "无法接受输入。请按 Backspace 或 Ctrl+U 清除输入错误。",
        }
    }

    pub(crate) const fn rename_invalid(self) -> &'static str {
        match self {
            Self::EnUs => "Use 1–512 characters.",
            Self::ZhCn => "请输入 1–512 个字符。",
        }
    }
}
