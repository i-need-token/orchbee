use super::Locale;
impl Locale {
    pub(crate) fn agent_notification_title(self, agent: &str, request: bool) -> String {
        match (self, request) {
            (Self::EnUs, false) => format!("{agent} finished"),
            (Self::EnUs, true) => format!("{agent} needs attention"),
            (Self::ZhCn, false) => format!("{agent} 已完成"),
            (Self::ZhCn, true) => format!("{agent} 需要处理"),
        }
    }

    pub(crate) fn agent_notification_context(
        self,
        workspace: Option<&str>,
        project: Option<&str>,
        tab: &str,
        pane: &str,
    ) -> String {
        let location = workspace.or(project);
        match (self, location) {
            (Self::EnUs, Some(location)) => {
                format!("Workspace {location} · Tab {tab} · Pane {pane}")
            }
            (Self::EnUs, None) => format!("Tab {tab} · Pane {pane}"),
            (Self::ZhCn, Some(location)) => {
                format!("工作区 {location} · 标签页 {tab} · 窗格 {pane}")
            }
            (Self::ZhCn, None) => format!("标签页 {tab} · 窗格 {pane}"),
        }
    }

    pub(crate) const fn notification_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee notification show <title> [--body TEXT] [--position top-left|top-right|bottom-left|bottom-right] [--sound none|done|request] [--format text|json]\n\nSend to accepting main-TUI Clients on the running Site; no Site is started.\nTitle/body are normalized to 80/240 characters. Omitted position follows each Client configuration; no sound by default.\nui.toast.delivery selects off (default), orchbee, terminal or system.\nDelivery confirms queue admission, not display or sound playback. Lost replies are not retried.\n"
            }
            Self::ZhCn => {
                "orchbee notification show <title> [--body TEXT] [--position top-left|top-right|bottom-left|bottom-right] [--sound none|done|request] [--format text|json]\n\n发送到运行中 Site 可接收的主 TUI Client；不启动 Site。\n标题/正文规范化为 80/240 个字符。省略位置时使用各 Client 配置，默认无声音。\nui.toast.delivery 可选 off（默认）、orchbee、terminal 或 system。\n投递仅确认入队，不保证显示或播放声音。丢失响应不自动重试。\n"
            }
        }
    }
    pub(crate) fn notification_result(self, result: &str) -> &'static str {
        match (self, result) {
            (Self::EnUs, "disabled") => "Notification disabled and no Client connected.\n",
            (Self::ZhCn, "disabled") => "通知已禁用，且没有连接中的 Client。\n",
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
