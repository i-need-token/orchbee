use super::Locale;
impl Locale {
    pub(crate) fn events_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::localization::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::EventsSubscribe => (
                "orchbee events subscribe [--event <name> ... | --filter <json> ...] [--workspace <target> | --tab <target> | --panel <target>] [--client <id>] [--after-cursor <json>] [--timeout <ms>] [--format text|json]",
                "Stream NDJSON events from an existing session. Event names use workspace.*, tab.*, pane.*, worktree.* and layout.updated; panel events use the pane prefix. Without filters, observe all supported event kinds. There is no overall deadline unless --timeout is supplied.\n\nA cursor contains epoch and a decimal-string sequence; history loss is reported explicitly. A typed --filter owns its scopes and conditions and cannot be combined with shorthand selectors. It also supports retained history and child terminal surfaces. Ctrl+C releases the subscription without changing terminal input or scrolling.",
                "从已运行的会话持续读取 NDJSON 事件。事件名称使用 workspace.*、tab.*、pane.*、worktree.* 和 layout.updated；面板事件使用 pane 前缀。不设置筛选时观察所有受支持的事件类型；只有显式传入 --timeout 才设置总时限。\n\n游标包含 epoch 和十进制字符串序号；历史丢失会明确报告。结构化 --filter 独占作用域和条件，不可与简写选择器混用，也支持保留历史和子终端画面。Ctrl+C 释放订阅，不改变终端输入或滚动。",
            ),
            C::EventsWait => (
                "orchbee events wait (--event <name> | --filter <json>) [--workspace <target> | --tab <target> | --panel <target>] [--client <id>] [--after-cursor <json>] [--until idle|working|blocked|done|error|unknown] [--match <text> | --regex <pattern>] [--source recent|recent-unwrapped|visible|detection] [--lines <1..4096>] [--plain|--ansi] [--timeout <0..3600000>] [--format text|json]",
                "Wait for exactly one event filter in an existing session. Default timeout: 30000 ms. Timeout 0 checks the admitted baseline once; a baseline still being prepared returns busy. History loss is reported explicitly.\n\nOutput matching requires --event pane.output_matched and --panel. Scroll observation requires --event pane.scroll_changed, --panel and --client. Visible output needs the selected client. Agent state with a panel and --until also tests the current state. A typed --filter owns all scopes and conditions: do not combine it with shorthand selectors. A new connection does not resume a previous physical probe. Ctrl+C releases the subscription without changing terminal input or scrolling.",
                "在已运行的会话中等待一个事件筛选条件。默认时限 30000 毫秒；timeout 为 0 时检查一次已接纳的基线，基线尚在准备时返回 busy。历史丢失会明确报告。\n\n输出匹配需要 --event pane.output_matched 和 --panel。滚动观察需要 --event pane.scroll_changed、--panel 和 --client。可见输出需要选中的客户端；Agent 状态带面板和 --until 时，也检查当前状态。结构化 --filter 独占所有作用域和条件，不可与简写选择器混用。新连接不会恢复上次的物理探测。Ctrl+C 释放订阅，不改变终端输入或滚动。",
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
    pub(crate) fn events_status(self, status: u8) -> &'static str {
        match (self, status) {
            (Self::EnUs, 0) => "Event matched.\n",
            (Self::ZhCn, 0) => "已匹配事件。\n",
            (Self::EnUs, 7) => "Event wait timed out.\n",
            (Self::ZhCn, 7) => "事件等待超时。\n",
            (Self::EnUs, 8) => "Event history was lost or observation ownership is unknown.\n",
            (Self::ZhCn, 8) => "事件历史已丢失，或观察所有权不确定。\n",
            (Self::EnUs, _) => "Event observation was refused or interrupted.\n",
            (Self::ZhCn, _) => "事件观察被拒绝或中断。\n",
        }
    }
}
