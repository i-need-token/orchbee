//! CLI help and result copy; shared native-surface messages remain below this owner.
use super::Locale;
impl super::CliText {
    pub(crate) const fn notification_help(self) -> crate::cli_text::HelpDetails {
        match self.locale {
            Locale::EnUs => crate::cli_text::HelpDetails::new(
                "orchbee notification show <title> [--body TEXT] [--position top-left|top-right|bottom-left|bottom-right] [--sound none|done|request] [--format text|json]",
                "Send to accepting main-TUI Clients on the running session; no session is started.\nTitle/body are normalized to 80/240 characters. Omitted position follows each Client configuration; no sound by default.\nnotifications.cli_delivery selects off (default), orchbee, terminal or system.\nDelivery confirms queue admission, not display or sound playback. Lost replies are not retried.",
            ),
            Locale::ZhCn => crate::cli_text::HelpDetails::new(
                "orchbee notification show <title> [--body TEXT] [--position top-left|top-right|bottom-left|bottom-right] [--sound none|done|request] [--format text|json]",
                "发送到运行中会话可接收的主 TUI 客户端；不启动会话。标题/正文规范化为 80/240 个字符。省略位置时使用各客户端配置，默认无声音。\nnotifications.cli_delivery 可选 off（默认）、orchbee、terminal 或 system。投递仅确认入队，不保证显示或播放声音。丢失响应不自动重试。",
            ),
        }
    }

    pub(crate) fn notification_result(self, result: &str) -> &'static str {
        match (self.locale, result) {
            (Locale::EnUs, "disabled") => "Notification disabled and no Client connected.\n",
            (Locale::ZhCn, "disabled") => "通知已禁用，且没有连接中的客户端。\n",
            (Locale::EnUs, "shown") => "Notification queued for connected Clients.\n",
            (Locale::ZhCn, "shown") => "通知已排入连接中的客户端。\n",
            (Locale::EnUs, "busy") => "Notification not queued: Clients are busy.\n",
            (Locale::ZhCn, "busy") => "通知未入队：客户端繁忙。\n",
            (Locale::EnUs, "rate-limited") => "Notification not queued: rate limited.\n",
            (Locale::ZhCn, "rate-limited") => "通知未入队：触发限流。\n",
            (Locale::EnUs, "no-foreground-client") => {
                "Notification not queued: no active main-TUI Client.\n"
            }
            (Locale::ZhCn, "no-foreground-client") => "通知未入队：没有在线主 TUI 客户端。\n",
            (Locale::EnUs, "unknown") => "Notification delivery unknown; it was not retried.\n",
            (Locale::ZhCn, "unknown") => "通知投递结果未知；未重试。\n",
            (Locale::EnUs, _) => "Notification submission failed.\n",
            (Locale::ZhCn, _) => "通知提交失败。\n",
        }
    }
}
