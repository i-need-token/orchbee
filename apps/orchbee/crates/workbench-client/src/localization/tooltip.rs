use super::Locale;

/// What a pointer tooltip names that no other surface already says.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TooltipLabel {
    HideAgents,
    ShowAgents,
    /// The Tab strip's leading overflow marker.
    EarlierTabs,
    /// The Tab strip's trailing overflow marker.
    LaterTabs,
    /// The sidebar toggle while the sidebar is visible.
    HideSidebar,
    /// The sidebar toggle while the sidebar is hidden.
    ShowSidebar,
    /// The sidebar's control that opens shortcut help.
    Shortcuts,
    /// The Tab list's way back to the terminal.
    HideTabs,
}

impl Locale {
    pub(crate) const fn tooltip_label(self, label: TooltipLabel) -> &'static str {
        match (self, label) {
            (Self::EnUs, TooltipLabel::HideAgents) => "Hide Agents",
            (Self::ZhCn, TooltipLabel::HideAgents) => "隐藏智能体",
            (Self::EnUs, TooltipLabel::ShowAgents) => "Show Agents",
            (Self::ZhCn, TooltipLabel::ShowAgents) => "显示智能体",
            (Self::EnUs, TooltipLabel::EarlierTabs) => "Show earlier Tabs",
            (Self::ZhCn, TooltipLabel::EarlierTabs) => "显示前面的标签页",
            (Self::EnUs, TooltipLabel::LaterTabs) => "Show later Tabs",
            (Self::ZhCn, TooltipLabel::LaterTabs) => "显示后面的标签页",
            (Self::EnUs, TooltipLabel::HideSidebar) => "Hide sidebar",
            (Self::ZhCn, TooltipLabel::HideSidebar) => "隐藏边栏",
            (Self::EnUs, TooltipLabel::ShowSidebar) => "Show sidebar",
            (Self::ZhCn, TooltipLabel::ShowSidebar) => "显示边栏",
            (Self::EnUs, TooltipLabel::Shortcuts) => "Shortcuts",
            (Self::ZhCn, TooltipLabel::Shortcuts) => "快捷键",
            (Self::EnUs, TooltipLabel::HideTabs) => "Hide Tabs",
            (Self::ZhCn, TooltipLabel::HideTabs) => "隐藏标签页",
        }
    }
}
