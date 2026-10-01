use super::Locale;
impl Locale {
    pub(crate) fn events_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee events subscribe [--event NAME ... | --filter JSON ...] [--workspace TARGET | --tab TARGET | --panel TARGET] [--client ID] [--after-cursor JSON] [--timeout MS] [--format text|json]\norchbee events wait --event NAME [same selectors] [--until idle|working|blocked|unknown] [--match TEXT | --regex PATTERN] [--source recent|recent-unwrapped|visible|detection] [--lines 1..4096] [--plain|--ansi] [--timeout 0..3600000] [--format text|json]\norchbee events wait --filter JSON [--after-cursor JSON] [--timeout MS] [--format text|json]\n\nSubscribe emits bounded NDJSON records by default. Event names use workspace.*, tab.*, pane.*, worktree.* and layout.updated; a native Panel is a Pane. No filter subscribes to all 26 semantic kinds. Wait admits exactly one filter and defaults to 30 seconds; timeout 0 checks the admitted baseline once; a physical baseline still pending returns busy, not a no-match timeout. Subscribe has no overall timeout unless supplied. Read-only observation joins an existing Site and retains one connection-owned subscription. A cursor contains epoch and decimal-string sequence. History loss is explicit; a new connection never resumes physical probe state. Typed --filter also admits retained history and child Presentations, and owns all scopes and conditions; do not combine it with shorthand selectors. Output-match needs --event pane.output_matched and --panel; scroll uses pane.scroll_changed, --panel and --client. Visible output requires the selected Client. Agent status with a Panel and --until also tests the admitted current state. Ctrl+C releases the subscription without changing terminal input or scrolling.\n"
            }
            Self::ZhCn => {
                "orchbee events subscribe [--event NAME ... | --filter JSON ...] [--workspace TARGET | --tab TARGET | --panel TARGET] [--client ID] [--after-cursor JSON] [--timeout MS] [--format text|json]\norchbee events wait --event NAME [同上选择器] [--until idle|working|blocked|unknown] [--match TEXT | --regex PATTERN] [--source recent|recent-unwrapped|visible|detection] [--lines 1..4096] [--plain|--ansi] [--timeout 0..3600000] [--format text|json]\norchbee events wait --filter JSON [--after-cursor JSON] [--timeout MS] [--format text|json]\n\nSubscribe 默认输出有界 NDJSON 记录。事件名使用 workspace.*、tab.*、pane.*、worktree.* 和 layout.updated；原生 Panel 对应 Pane。省略过滤器订阅全部 26 种语义事件。Wait 要求恰好一个过滤器，默认等 30 秒；timeout 0 检查一次已准入基线；物理基线仍在等待时返回 busy，不报告未匹配超时。Subscribe 未指定 timeout 时持续订阅。只读观察加入已有 Site，保留一个连接拥有的订阅。Cursor 包含 epoch 和十进制字符串 sequence。历史丢失明确报告；新连接不恢复物理探测状态。--filter JSON 还支持保留 history 及子 Presentation，自行声明全部范围和条件，不与简写选择器混用。输出匹配要求 --event pane.output_matched 及 --panel；滚动使用 pane.scroll_changed、--panel、--client。Visible 输出要求指定所选 Client。Agent 状态指定 Panel 和 --until 时也检查已准入当前状态。Ctrl+C 释放订阅，不改变终端输入或滚动。\n"
            }
        }
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
