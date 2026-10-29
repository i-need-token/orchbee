use super::Locale;

/// What a pointer tooltip names that no other surface already says.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TooltipLabel {
    HideAgents,
    ShowAgents,
}

impl Locale {
    pub(crate) const fn tooltip_label(self, label: TooltipLabel) -> &'static str {
        match (self, label) {
            (Self::EnUs, TooltipLabel::HideAgents) => "Hide Agents",
            (Self::ZhCn, TooltipLabel::HideAgents) => "隐藏智能体",
            (Self::EnUs, TooltipLabel::ShowAgents) => "Show Agents",
            (Self::ZhCn, TooltipLabel::ShowAgents) => "显示智能体",
        }
    }
}
