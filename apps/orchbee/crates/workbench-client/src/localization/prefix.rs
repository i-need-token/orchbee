//! Localized descriptions of prefix actions; key bindings remain owned by the input model.

use super::Locale;
/// Localization keys retained by input owners for mode-specific descriptions
/// and status hints. Global shortcut discovery uses action metadata instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrefixDesc {
    ActionMenu,
    Settings,
    ConfiguredCommand(u8),
    ResizeLeft,
    ResizeRight,
    ResizeUp,
    ResizeDown,
    CopyMode,
    FindInTerminal,
    CopyMove,
    CopyWord,
    CopyParagraph,
    CopyPage,
    CopyEnds,
    CopySearch,
    CopyRepeatSearch,
    CopyReverseSearch,
    CopySelection,
    CopySubmit,
    CopyExit,
    Machines,
    AgentCycle,
    AgentIndex,
    OpenNotificationTarget,
    AgentConfirmations,
    WorkspaceCreate,
    QuickWorkspaceCreate,
    WorktreeCreate,
    WorktreeOpen,
    WorkspaceClose,
    WorkspaceRename,
    RenameTab,
    CloseTab,
    CreateTab,
    MoveTab,
    SelectTabWrap,
    SelectContext,
    SelectTabDigit,
    PanelFocus,
    PanelZoom,
    SplitPanel,
    ClosePanel,
    RenamePanel,
    LastPanel,
    ClearTerminal,
    MovePanel,
    SidebarToggle,
    SidebarFocus,
    NavigatorToggle,
    StartAgent,
    History,
    PointerCapture,
    Exit,
    LiteralPrefix,
    DismissPrefix,
}

/// The words of a key row in the action menu; only those keys have rows, so the other prefix
/// actions have no words here.
pub(super) const fn en_us_prefix_desc(desc: PrefixDesc) -> Option<&'static str> {
    Some(match desc {
        PrefixDesc::SelectTabDigit => "Go to a Tab by its number",
        PrefixDesc::LiteralPrefix => "Send the prefix key to the terminal",
        PrefixDesc::DismissPrefix => "Dismiss the prefix without an action",
        PrefixDesc::CopyMove => "Move the copy cursor",
        PrefixDesc::CopyWord => "Move by word / whitespace-delimited word",
        PrefixDesc::CopyParagraph => "Move to previous / next paragraph",
        PrefixDesc::CopyPage => "Page older / newer content",
        PrefixDesc::CopyEnds => "Go to oldest / newest content",
        PrefixDesc::CopySearch => "Search forward / backward; Enter confirms",
        PrefixDesc::CopyRepeatSearch => "Repeat search / reverse direction",
        PrefixDesc::CopyReverseSearch => "Reverse the search draft direction",
        PrefixDesc::CopySelection => "Start / cancel selection",
        PrefixDesc::CopySubmit => "Copy selection; return live after success",
        PrefixDesc::CopyExit => "Leave copy mode (Esc also cancels search)",
        _ => return None,
    })
}

pub(super) const fn zh_cn_prefix_desc(desc: PrefixDesc) -> Option<&'static str> {
    Some(match desc {
        PrefixDesc::SelectTabDigit => "按编号切换标签页",
        PrefixDesc::LiteralPrefix => "把前缀键发送给终端",
        PrefixDesc::DismissPrefix => "取消前缀而不执行动作",
        PrefixDesc::CopyMove => "移动复制光标",
        PrefixDesc::CopyWord => "按词 / 空白分隔的大词移动",
        PrefixDesc::CopyParagraph => "移动到上一个 / 下一个段落",
        PrefixDesc::CopyPage => "翻看更早 / 更新内容",
        PrefixDesc::CopyEnds => "跳到最旧 / 最新内容",
        PrefixDesc::CopySearch => "向前 / 向后搜索；Enter 确认",
        PrefixDesc::CopyRepeatSearch => "重复搜索 / 反向跳转",
        PrefixDesc::CopyReverseSearch => "反转搜索草稿方向",
        PrefixDesc::CopySelection => "开始 / 取消选择",
        PrefixDesc::CopySubmit => "复制选区；成功后回实时显示",
        PrefixDesc::CopyExit => "退出复制模式（Esc 也取消搜索）",
        _ => return None,
    })
}

impl Locale {
    /// The action menu's Leave row.
    pub(super) const fn client_detach_label(self) -> &'static str {
        match self {
            Self::EnUs => "Leave view; terminals keep running",
            Self::ZhCn => "离开界面；终端继续运行",
        }
    }
}
