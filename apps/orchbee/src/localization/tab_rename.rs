use super::Locale;

impl Locale {
    pub(crate) const fn rename_title(self) -> &'static str {
        match self {
            Self::EnUs => "Rename Tab",
            Self::ZhCn => "重命名 Tab",
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
            Self::EnUs => "The shared Tab label changes for all Clients.",
            Self::ZhCn => "共享 Tab 的名称会对所有 Client 更改。",
        }
    }

    pub(crate) const fn rename_actions(self) -> &'static str {
        match self {
            Self::EnUs => "Enter · rename    Esc · cancel    Backspace · delete    Ctrl+U · clear",
            Self::ZhCn => "Enter · 重命名    Esc · 取消    Backspace · 删除    Ctrl+U · 清空",
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

    pub(crate) const fn rename_resize_hint(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Enlarge the window to see the complete Rename draft, or press Esc to cancel."
            }
            Self::ZhCn => "请扩大窗口以查看完整重命名草稿，或按 Esc 取消。",
        }
    }
}
