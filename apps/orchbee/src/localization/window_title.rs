use super::Locale;
impl Locale {
    pub(crate) const fn window_title_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee terminal title set <title> [--format text|json]\norchbee terminal title clear [--format text|json]\n\nPublish a Site override for the foreground Windows main TUI. No Site is started.\nSet removes controls, truncates to 200 characters then trims; empty is invalid.\nClear uses Orchbee until configured title templates are supported.\nNo-foreground still commits for later attachment; delivery is not a display acknowledgement.\nUnknown submitted outcomes are not retried.\n"
            }
            Self::ZhCn => {
                "orchbee terminal title set <title> [--format text|json]\norchbee terminal title clear [--format text|json]\n\n发布 Site 覆盖标题，由前台 Windows 主 TUI 接收；不启动 Site。\nSet 移除控制字符、截取 200 个字符后 trim；空标题无效。\nClear 在配置标题模板接入前使用 Orchbee。\n无前台仍提交给后续连接；投递不确认实际显示。\n已提交但结果未知时不自动重试。\n"
            }
        }
    }
    pub(crate) fn window_title_result(self, label: &str) -> &'static str {
        match (self, label) {
            (Self::EnUs, "set") => "Title override published for the foreground Client.\n",
            (Self::ZhCn, "set") => "标题覆盖已发布给前台 Client。\n",
            (Self::EnUs, "cleared") => "Title override cleared for the foreground Client.\n",
            (Self::ZhCn, "cleared") => "前台 Client 的标题覆盖已清除。\n",
            (Self::EnUs, "no-foreground-client") => {
                "Title policy published; no foreground Client is connected.\n"
            }
            (Self::ZhCn, "no-foreground-client") => "标题策略已发布；尚无前台 Client 连接。\n",
            (Self::EnUs, "unknown") => "Title publication unknown; it was not retried.\n",
            (Self::ZhCn, "unknown") => "标题发布结果未知；未重试。\n",
            (Self::EnUs, _) => "Title publication failed.\n",
            (Self::ZhCn, _) => "标题发布失败。\n",
        }
    }
}
