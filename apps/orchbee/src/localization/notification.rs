use super::Locale;
impl Locale {
    pub(crate) const fn notification_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee notification show <title> [--body TEXT] [--position top-left|top-right|bottom-left|bottom-right] [--sound none|done|request] [--format text|json]\n\nSend to accepting main-TUI Clients on the running Site; no Site is started.\nTitle/body are normalized to 80/240 characters. Defaults: bottom-right, no sound.\nDelivery confirms queue admission, not display or sound playback. Lost replies are not retried.\n"
            }
            Self::ZhCn => {
                "orchbee notification show <title> [--body TEXT] [--position top-left|top-right|bottom-left|bottom-right] [--sound none|done|request] [--format text|json]\n\n发送到运行中 Site 可接收的主 TUI Client；不启动 Site。\n标题/正文规范化为 80/240 个字符。默认右下角，无声音。\n投递仅确认入队，不保证显示或播放声音。丢失响应不自动重试。\n"
            }
        }
    }
    pub(crate) fn notification_result(self, result: &str) -> &'static str {
        match (self, result) {
            (Self::EnUs, "shown") => "Notification queued for connected Clients.\n",
            (Self::ZhCn, "shown") => "通知已排入连接中的 Client。\n",
            (Self::EnUs, "busy") => "Notification not queued: Clients are busy.\n",
            (Self::ZhCn, "busy") => "通知未入队：Client 繁忙。\n",
            (Self::EnUs, "rate-limited") => "Notification not queued: rate limited.\n",
            (Self::ZhCn, "rate-limited") => "通知未入队：触发限流。\n",
            (Self::EnUs, "no-foreground-client") => {
                "Notification not queued: no active main-TUI Client.\n"
            }
            (Self::ZhCn, "no-foreground-client") => "通知未入队：没有在线主 TUI Client。\n",
            (Self::EnUs, "unknown") => "Notification delivery unknown; it was not retried.\n",
            (Self::ZhCn, "unknown") => "通知投递结果未知；未重试。\n",
            (Self::EnUs, _) => "Notification submission failed.\n",
            (Self::ZhCn, _) => "通知提交失败。\n",
        }
    }
}
