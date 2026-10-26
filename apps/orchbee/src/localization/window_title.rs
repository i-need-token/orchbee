use super::Locale;
impl Locale {
    pub(crate) fn window_title_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::localization::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::TerminalTitleSet => (
                "orchbee terminal title set <title> [--format text|json]",
                "Set the foreground Windows terminal interface title. Control characters are removed; the title is limited to 200 characters and trimmed. Empty titles are invalid. Requires an existing session.\n\nWith no foreground client, the title is saved for a later attachment. Submission does not guarantee the title has been displayed. An unknown result is never retried automatically.",
                "设置前台 Windows 终端界面标题。移除控制字符，截取至 200 个字符并去除两端空白；空标题无效。需要已运行的会话。\n\n没有前台客户端时，为后续连接保存标题。提交成功不代表标题已经显示；结果未知时不自动重试。",
            ),
            C::TerminalTitleClear => (
                "orchbee terminal title clear [--format text|json]",
                "Restore the configured ui.window_title template; an empty result displays Orchbee. Requires an existing session. With no foreground client, the change applies on a later attachment. An unknown result is never retried automatically.",
                "恢复配置中的 ui.window_title 模板；结果为空时显示 Orchbee。需要已运行的会话。没有前台客户端时，在后续连接时应用；结果未知时不自动重试。",
            ),
            _ => unreachable!("command help ownership"),
        };
        crate::localization::HelpDetails::new(
            usage,
            match self {
                Self::EnUs => english,
                Self::ZhCn => chinese,
            },
        )
    }
    pub(crate) fn window_title_result(self, label: &str) -> &'static str {
        match (self, label) {
            (Self::EnUs, "set") => "Title override published for the foreground Client.\n",
            (Self::ZhCn, "set") => "标题覆盖已发布给前台客户端。\n",
            (Self::EnUs, "cleared") => "Title override cleared for the foreground Client.\n",
            (Self::ZhCn, "cleared") => "前台客户端的标题覆盖已清除。\n",
            (Self::EnUs, "no-foreground-client") => {
                "Title policy published; no foreground Client is connected.\n"
            }
            (Self::ZhCn, "no-foreground-client") => "标题策略已发布；尚无前台客户端连接。\n",
            (Self::EnUs, "unknown") => "Title publication unknown; it was not retried.\n",
            (Self::ZhCn, "unknown") => "标题发布结果未知；未重试。\n",
            (Self::EnUs, _) => "Title publication failed.\n",
            (Self::ZhCn, _) => "标题发布失败。\n",
        }
    }
}
