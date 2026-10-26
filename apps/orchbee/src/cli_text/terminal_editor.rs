//! Formats admitted editor action guidance without applying terminal input or changing selection.
pub(super) const HELP_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee terminal edit-scrollback <terminal> [--client <id>] [--timeout <ms>] [--format json|text]",
    "Open the focused terminal's retained plain-text scrollback in the configured editor. Requires an existing Windows session and Ready TUI; the target must be that Client's current native member. Uses the Worker's VISUAL, then EDITOR, otherwise system Notepad. Console editors use an independent popup; GUI editors may open their own window. Default timeout: 30000 ms; range: 1-3600000. Does not control, resize or send input to the source. Retain uncertain operation IDs for client control-status; do not repeat unknown submissions.",
);
pub(super) const HELP_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee terminal edit-scrollback <terminal> [--client <id>] [--timeout <ms>] [--format json|text]",
    "在配置的编辑器中打开聚焦终端保留的纯文本滚动记录。需要已有 Windows 会话和 Ready TUI；目标必须是该客户端当前选择的原生成员。依次使用 Worker 的 VISUAL、EDITOR，否则使用系统 Notepad。控制台编辑器使用独立 popup；GUI 编辑器可能打开自身窗口。默认超时 30000 毫秒，范围 1-3600000。不会控制、缩放来源或向其发送输入。保留不确定操作 ID 供 client control-status 查询，请勿重复未知提交。",
);
