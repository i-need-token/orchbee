use super::Locale;
impl Locale {
    pub(crate) fn configured_command_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee command list [--format text|json]\norchbee command invoke <command-id> [--workspace ID] [--tab ID] [--panel ID] [--client ID] [--context JSON] [--anchor ROW:COL --cursor ROW:COL --content-revision N] [--timeout MS] [--format text|json]\n\nCommands come from the running Site's [[keys.command]] configuration. List shows opaque IDs, action, bindings and description. Invoke resolves that same installed ID before focusing an optional source target. Shell runs in the background, pane creates and focuses a split, popup opens an independent terminal in the selected Client, and plugin_action invokes an installed plugin action. Source cwd and environment follow the observed terminal. Plugin selection requires both coordinates and the exact terminal content revision. Changed configuration invalidates old IDs. An uncertain submission is never replayed; cancellation stops waiting, not an accepted command.\n"
            }
            Self::ZhCn => {
                "orchbee command list [--format text|json]\norchbee command invoke <command-id> [--workspace ID] [--tab ID] [--panel ID] [--client ID] [--context JSON] [--anchor ROW:COL --cursor ROW:COL --content-revision N] [--timeout MS] [--format text|json]\n\n命令来自运行中 Site 的 [[keys.command]] 配置。List 显示不透明 ID、动作、快捷键与说明。Invoke 先解析同一个已安装 ID，再聚焦可选来源目标。Shell 在后台运行，pane 创建并聚焦分屏，popup 在选定 Client 打开独立终端，plugin_action 调用已安装插件动作。来源 cwd 和环境来自实际终端观察。插件选区要求同时给出两个坐标与准确的终端内容版本。配置变化后旧 ID 失效；提交结果未知时不重放。取消只停止等待，不撤销已受理命令。\n"
            }
        }
    }
    pub(crate) fn configured_command_status(self, status: u8) -> &'static str {
        match (self, status) {
            (Self::EnUs, 0) => "Command completed its requested action.\n",
            (Self::ZhCn, 0) => "命令已完成请求的动作。\n",
            (Self::EnUs, 8) => {
                "Command outcome is unknown or still pending; it was not replayed.\n"
            }
            (Self::ZhCn, 8) => "命令结果未知或仍在等待；没有重放提交。\n",
            (Self::EnUs, _) => "Command was refused or failed.\n",
            (Self::ZhCn, _) => "命令被拒绝或执行失败。\n",
        }
    }
}
