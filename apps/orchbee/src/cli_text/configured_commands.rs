//! Formats configured-command help and invocation outcomes without resolving or executing a command.
use super::Locale;
impl super::CliText {
    pub(crate) fn configured_command_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::cli_text::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::CommandList => (
                "orchbee command list [--format text|json]",
                "List commands from the running session's [[keys.command]] configuration, including their IDs, actions, shortcuts, descriptions and the command text each runs. Use the returned ID with orchbee command invoke. Changed configuration invalidates old IDs.",
                "列出正在运行的会话在 [[keys.command]] 中配置的命令，包括 ID、操作、快捷键、说明和各自运行的命令文本。将返回的 ID 用于 orchbee command invoke；配置变更会使旧 ID 失效。",
            ),
            C::CommandInvoke => (
                "orchbee command invoke <command-id> [--workspace <id>] [--tab <id>] [--panel <id>] [--client <id>] [--context <json>] [--anchor ROW:COL --cursor ROW:COL --content-revision <n>] [--timeout <ms>] [--format text|json]",
                "Run a command returned by orchbee command list in an existing session. An optional source target is focused first. Shell actions run in the background; panel actions open a split; popup actions open an independent terminal; plugin_action runs an installed plugin action. Working directory and environment follow the source terminal.\n\nPlugin selection requires both coordinates and the exact terminal content revision. Configuration changes invalidate old command IDs. An uncertain submission is never replayed; cancellation stops waiting and does not cancel accepted work.",
                "在已运行的会话中执行 orchbee command list 返回的命令，可先聚焦指定来源。shell 在后台运行；panel 打开分屏；popup 打开独立终端；plugin_action 执行已安装的插件操作。工作目录和环境来自来源终端。\n\n插件选区要求两端坐标及精确终端内容修订。配置变化会使旧命令 ID 失效。结果不确定时不重放提交；取消只结束等待，不取消已接纳的任务。",
            ),
            _ => unreachable!("command help ownership"),
        };
        crate::cli_text::HelpDetails::new(
            usage,
            match self.locale {
                Locale::EnUs => english,
                Locale::ZhCn => chinese,
            },
        )
    }
    pub(crate) fn configured_command_status(self, status: u8) -> &'static str {
        match (self.locale, status) {
            (Locale::EnUs, 0) => "Command completed its requested action.\n",
            (Locale::ZhCn, 0) => "命令已完成请求的动作。\n",
            (Locale::EnUs, 8) => {
                "Command outcome is unknown or still pending; it was not replayed.\n"
            }
            (Locale::ZhCn, 8) => "命令结果未知或仍在等待；没有重放提交。\n",
            (Locale::EnUs, _) => "Command was refused or failed.\n",
            (Locale::ZhCn, _) => "命令被拒绝或执行失败。\n",
        }
    }
}
