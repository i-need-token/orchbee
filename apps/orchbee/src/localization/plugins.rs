use super::Locale;
use orchbee_local_site_runtime::PluginRegistryError as Error;
impl Locale {
    pub(crate) fn cli_popup_opened(
        self,
        popup_id: &orchbee_workbench_protocol::WorkbenchRequestId,
    ) -> String {
        match self {
            Self::EnUs => format!("Popup opened: {popup_id}\n"),
            Self::ZhCn => format!("popup 已打开：{popup_id}\n"),
        }
    }
    pub(crate) const fn cli_popup_closed(self, had_popup: bool) -> &'static str {
        match (self, had_popup) {
            (Self::EnUs, true) => "Popup closed.\n",
            (Self::EnUs, false) => "No popup is open.\n",
            (Self::ZhCn, true) => "popup 已关闭。\n",
            (Self::ZhCn, false) => "当前没有 popup。\n",
        }
    }
    pub(crate) const fn plugin_popup_pending(self, closing: bool) -> &'static str {
        match (self, closing) {
            (Self::EnUs, false) => "Opening…",
            (Self::EnUs, true) => "Closing…",
            (Self::ZhCn, false) => "正在打开…",
            (Self::ZhCn, true) => "正在关闭…",
        }
    }
    pub(crate) fn plugin_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::localization::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::PluginInstall => (
                "orchbee plugin install <owner/repo[/subdir]> --yes [--ref <ref>] [--timeout <ms>] [--format text|json]",
                "Download plugin source from GitHub and run its Windows build steps. Git must be available in PATH. --yes is required because the downloaded code will run.\n\nThe work deadline defaults to 600000 ms (maximum 3600000), with up to five additional seconds to settle an active build. A failed build preserves the previous registration. Incomplete cleanup is reported for the next explicit install or uninstall. This command does not start a Site.",
                "从 GitHub 下载插件源码并执行其 Windows 构建步骤。PATH 中必须有 Git；必须指定 --yes，以允许执行下载的代码。\n\n工作时限默认 600000 毫秒，最大 3600000 毫秒；结束正在运行的构建最多再等待五秒。构建失败时保留原注册；未完成的清理会报告给下一次显式安装或卸载。本命令不启动 Site。",
            ),
            C::PluginUninstall => (
                "orchbee plugin uninstall <plugin-id|owner/repo[/subdir]> [--format text|json]",
                "Remove managed plugin source and its registration. User configuration and state remain. Incomplete cleanup is reported for the next explicit install or uninstall. This command does not start a Site.",
                "移除托管的插件源码及其注册，保留用户配置和状态。未完成的清理会报告给下一次显式安装或卸载。本命令不启动 Site。",
            ),
            C::PluginLink => (
                "orchbee plugin link <path> [--enabled|--disabled] [--format text|json]",
                "Register a local manifest file or a directory containing orchbee-plugin.toml (herdr-plugin.toml is also accepted). Registration does not run plugin commands or start a Site.",
                "注册本地清单文件，或包含 orchbee-plugin.toml 的目录；也接受 herdr-plugin.toml。注册不运行插件命令，也不启动 Site。",
            ),
            C::PluginUnlink => (
                "orchbee plugin unlink <plugin-id> [--format text|json]",
                "Remove the registration; keep source, configuration and state files. This command does not start a Site.",
                "移除注册，保留源码、配置和状态文件。本命令不启动 Site。",
            ),
            C::PluginEnable => (
                "orchbee plugin enable <plugin-id> [--format text|json]",
                "Enable the registered plugin. This command does not start a Site.",
                "启用已注册的插件。本命令不启动 Site。",
            ),
            C::PluginDisable => (
                "orchbee plugin disable <plugin-id> [--format text|json]",
                "Disable the registered plugin. This command does not start a Site.",
                "禁用已注册的插件。本命令不启动 Site。",
            ),
            C::PluginConfigDir => (
                "orchbee plugin config-dir <plugin-id> [--format text|json]",
                "Show the plugin configuration directory. This command does not start a Site.",
                "显示插件配置目录。本命令不启动 Site。",
            ),
            C::PluginList => (
                "orchbee plugin list [<plugin-id>] [--format text|json]",
                "List registered plugins, or inspect one plugin. This command does not start a Site.",
                "列出已注册的插件，或查看指定插件。本命令不启动 Site。",
            ),
            C::PluginActionList => (
                "orchbee plugin action list [<plugin-id>] [--plugin <id>] [--format text|json]",
                "List enabled, available Windows plugin actions. This command does not start a Site.",
                "列出已启用且可用的 Windows 插件操作。本命令不启动 Site。",
            ),
            C::PluginActionInvoke => (
                "orchbee plugin action invoke <action-id> [--plugin <id>] [--client <id>] [--context <json>] [--format text|json]",
                "Run an installed plugin action in an existing Site. The ID can be local or qualified as plugin.action. Context follows the caller or the single current client; --client selects explicitly and --context overrides display fields. Accepted work continues after the CLI exits; Site shutdown stops it.",
                "在已运行的 Site 中执行插件操作。ID 可使用本地名称或 plugin.action 完整名称。上下文来自调用者或唯一当前客户端；--client 显式选择客户端，--context 覆盖显示字段。CLI 退出后，已接纳的任务继续运行；Site 关闭时停止。",
            ),
            C::PluginPaneOpen => (
                "orchbee plugin pane open --plugin <id> --entrypoint <id> [--placement tab|split|zoomed|overlay|popup] [--width <cells|percent%>] [--height <cells|percent%>] [--workspace <id>] [--target-pane <id>] [--direction right|down] [--cwd <path>] [--env NAME=VALUE] [--client <id>] [--focus|--no-focus] [--format text|json]",
                "Open a plugin terminal in an existing Site. The default working directory is the plugin root.\n\nPopup placement opens an independent terminal in the selected current client. Width and height apply only to popups: use terminal cells or percentages. Defaults come from the manifest, otherwise half of the available area. Standard terminal geometry options do not apply. Escape and prefix keys reach the popup program. Click its close button or run orchbee popup close to dispose of it.",
                "在已运行的 Site 中打开插件终端，工作目录默认为插件根目录。\n\nPopup 在选中的当前客户端中打开独立终端。宽高仅适用于 popup，可使用终端单元格数量或百分比；默认取清单配置，否则占可用区域的一半。不使用普通终端尺寸选项。Esc 和前缀键会传给弹出窗口中的程序；点击关闭按钮或执行 orchbee popup close 可关闭。",
            ),
            C::PluginPaneFocus => (
                "orchbee plugin pane focus <pane-id> [--client <id>] [--timeout <ms>] [--format text|json]",
                "Focus a pane with recorded plugin membership in an existing Site. Stale or ordinary panes are refused.",
                "在已运行的 Site 中聚焦已记录插件归属的面板；拒绝已失效或普通面板。",
            ),
            C::PluginPaneClose => (
                "orchbee plugin pane close <pane-id> [--timeout <ms>] [--format text|json]",
                "Close a pane with recorded plugin membership in an existing Site and return its retained close receipt. Stale or ordinary panes are refused.",
                "在已运行的 Site 中关闭已记录插件归属的面板，并返回保留的关闭回执；拒绝已失效或普通面板。",
            ),
            C::PopupClose => (
                "orchbee popup close [--client <id>] [--timeout <ms>] [--format text|json]",
                "Close the popup in the selected current client. The receipt waits until the popup terminal is disposed of. Requires an existing Site.",
                "关闭选中当前客户端中的弹出窗口；回执等待弹出终端实际释放。需要已运行的 Site。",
            ),
            C::PluginLogList => (
                "orchbee plugin log list [<plugin-id>] [--plugin <id>] [--limit <1..200>] [--format text|json]",
                "Read retained plugin command logs from an existing Site. Default: the newest 50 commands, displayed oldest to newest. Each output stream is limited to 64 KiB and reports truncation explicitly.",
                "从已运行的 Site 读取保留的插件命令日志。默认显示最近 50 条命令，按从旧到新排序。每个输出流最多 64 KiB，并明确标记截断。",
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
    pub(crate) const fn plugin_result_heading(self) -> &'static str {
        match self {
            Self::EnUs => "Plugin catalog:",
            Self::ZhCn => "插件目录：",
        }
    }
    pub(crate) const fn plugin_enabled(self, enabled: bool) -> &'static str {
        match (self, enabled) {
            (Self::EnUs, true) => "enabled",
            (Self::EnUs, false) => "disabled",
            (Self::ZhCn, true) => "已启用",
            (Self::ZhCn, false) => "已禁用",
        }
    }
    pub(crate) const fn plugin_error(self, error: Error) -> &'static str {
        match (self, error) {
            (Self::EnUs, Error::InvalidManifest) => "Invalid plugin manifest.",
            (Self::ZhCn, Error::InvalidManifest) => "插件清单无效。",
            (Self::EnUs, Error::InvalidIdentifier) => "Invalid plugin identifier.",
            (Self::ZhCn, Error::InvalidIdentifier) => "插件标识符无效。",
            (Self::EnUs, Error::InvalidPath) => {
                "Plugin configuration or source path is unavailable or invalid."
            }
            (Self::ZhCn, Error::InvalidPath) => "插件配置或源路径不可用或无效。",
            (Self::EnUs, Error::InvalidRegistry) => {
                "Plugin catalog is invalid; existing data was retained."
            }
            (Self::ZhCn, Error::InvalidRegistry) => "插件目录无效；保留现有数据。",
            (Self::EnUs, Error::ManifestUnavailable) => {
                "Plugin source is unavailable; retained metadata cannot authorize execution."
            }
            (Self::ZhCn, Error::ManifestUnavailable) => "插件源不可用；保留的元数据不能授权执行。",
            (Self::EnUs, Error::PluginNotFound) => "Plugin is not registered.",
            (Self::ZhCn, Error::PluginNotFound) => "插件尚未登记。",
            (Self::EnUs, Error::LimitExceeded) => "Plugin manifest or catalog exceeds its limit.",
            (Self::ZhCn, Error::LimitExceeded) => "插件清单或目录超过上限。",
            (Self::EnUs, Error::Busy) => {
                "Plugin catalog is being modified; retry after the other operation finishes."
            }
            (Self::ZhCn, Error::Busy) => "插件目录正在修改；请在另一项操作结束后重试。",
            (Self::EnUs, Error::StorageUnavailable) => {
                "Plugin storage could not be safely read or updated."
            }
            (Self::ZhCn, Error::StorageUnavailable) => "无法安全读取或更新插件存储。",
        }
    }
}

impl Locale {
    pub(crate) const fn plugin_runtime_state(self, state: &str) -> &'static str {
        match (self, state.as_bytes()) {
            (Self::EnUs, b"operation-id") => "Operation ID",
            (Self::ZhCn, b"operation-id") => "操作 ID",
            (Self::EnUs, b"exit-code") => "Exit code",
            (Self::ZhCn, b"exit-code") => "退出码",
            (Self::EnUs, b"running") => "running",
            (Self::ZhCn, b"running") => "运行中",
            (Self::EnUs, b"succeeded") => "succeeded",
            (Self::ZhCn, b"succeeded") => "执行成功",
            (Self::EnUs, b"failed") => "failed",
            (Self::ZhCn, b"failed") => "执行失败",
            (Self::EnUs, b"truncated") => "Output exceeded 64 KiB per stream and was truncated.",
            (Self::ZhCn, b"truncated") => "输出超出每路 64 KiB 的保留上限，已截断。",
            (Self::EnUs, _) => {
                "Submission outcome unknown; inspect plugin logs before taking further action."
            }
            (Self::ZhCn, _) => "提交结果未知；请先查询插件日志再进行下一步操作。",
        }
    }
    pub(crate) const fn plugin_runtime_failure(
        self,
        reason: orchbee_workbench_protocol::PluginFailure,
    ) -> &'static str {
        use orchbee_workbench_protocol::PluginFailure as F;
        match (self, reason) {
            (Self::EnUs, F::Denied) => "Plugin operation denied.",
            (Self::ZhCn, F::Denied) => "无权执行插件操作。",
            (Self::EnUs, F::Unavailable) => "Plugin runtime unavailable.",
            (Self::ZhCn, F::Unavailable) => "插件运行时不可用。",
            (Self::EnUs, F::SiteMismatch) => "Plugin Site does not match.",
            (Self::ZhCn, F::SiteMismatch) => "插件 Site 不匹配。",
            (Self::EnUs, F::WorkerChanged) => {
                "Plugin Worker changed; the old log cursor is no longer valid."
            }
            (Self::ZhCn, F::WorkerChanged) => "插件 Worker 已变化，旧日志游标失效。",
            (Self::EnUs, F::InvalidIntent) => "Invalid plugin invocation.",
            (Self::ZhCn, F::InvalidIntent) => "插件调用无效。",
            (Self::EnUs, F::NotFound) => "Plugin action or log not found.",
            (Self::ZhCn, F::NotFound) => "找不到插件 action 或日志。",
            (Self::EnUs, F::Disabled) => "Plugin is disabled.",
            (Self::ZhCn, F::Disabled) => "插件已禁用。",
            (Self::EnUs, F::Unsupported) => "Plugin action is unavailable on Windows.",
            (Self::ZhCn, F::Unsupported) => "插件 action 不支持 Windows。",
            (Self::EnUs, F::SourceUnavailable) => "Plugin source or catalog is unavailable.",
            (Self::ZhCn, F::SourceUnavailable) => "插件源文件或目录不可用。",
            (Self::EnUs, F::Ambiguous) => "Action ID is ambiguous; specify --plugin.",
            (Self::ZhCn, F::Ambiguous) => "action ID 不唯一，请指定 --plugin。",
            (Self::EnUs, F::Busy) => "Plugin execution capacity is full.",
            (Self::ZhCn, F::Busy) => "插件执行容量已满。",
            (Self::EnUs, F::Conflict) => {
                "The operation ID already identifies a different invocation."
            }
            (Self::ZhCn, F::Conflict) => "操作 ID 已用于不同的调用。",
            (Self::EnUs, F::PhysicalUnknown) => {
                "Plugin process cleanup is uncertain; new execution is blocked."
            }
            (Self::ZhCn, F::PhysicalUnknown) => "插件进程清理结果不确定，已阻止新执行。",
        }
    }
    pub(crate) const fn plugin_command_failure(
        self,
        reason: orchbee_workbench_protocol::PluginCommandFailure,
    ) -> &'static str {
        use orchbee_workbench_protocol::PluginCommandFailure as F;
        match (self, reason) {
            (Self::EnUs, F::ExitFailure) => "The command exited unsuccessfully.",
            (Self::ZhCn, F::ExitFailure) => "命令以失败状态退出。",
            (Self::EnUs, F::SpawnUnavailable) => "The command executable was not found.",
            (Self::ZhCn, F::SpawnUnavailable) => "找不到命令可执行文件。",
            (Self::EnUs, F::SpawnFailed) => "The command could not be started.",
            (Self::ZhCn, F::SpawnFailed) => "无法启动命令。",
            (Self::EnUs, F::Io) => "Command output or process observation failed.",
            (Self::ZhCn, F::Io) => "读取命令输出或观察进程失败。",
            (Self::EnUs, F::Cancelled) => "The command was cancelled and its process Job settled.",
            (Self::ZhCn, F::Cancelled) => "命令已取消，其进程 Job 已清理。",
            (Self::EnUs, F::PhysicalUnknown) => "Process Job cleanup is uncertain.",
            (Self::ZhCn, F::PhysicalUnknown) => "进程 Job 的清理结果不确定。",
        }
    }
}

impl Locale {
    pub(crate) const fn plugin_provision_stage(
        self,
        stage: orchbee_local_site_runtime::PluginProvisionStage,
    ) -> &'static str {
        use orchbee_local_site_runtime::PluginProvisionStage as S;
        match (self, stage) {
            (Self::EnUs, S::Prepare) => "Preparing plugin source.",
            (Self::ZhCn, S::Prepare) => "准备插件源。",
            (Self::EnUs, S::Checkout) => "Downloading plugin source.",
            (Self::ZhCn, S::Checkout) => "下载插件源。",
            (Self::EnUs, S::Inspect) => "Validating plugin manifest.",
            (Self::ZhCn, S::Inspect) => "校验插件清单。",
            (Self::EnUs, S::Build) => "Building plugin.",
            (Self::ZhCn, S::Build) => "构建插件。",
            (Self::EnUs, S::Commit) => "Updating plugin registration.",
            (Self::ZhCn, S::Commit) => "更新插件登记。",
            (Self::EnUs, S::Cleanup) => "Cleaning managed source files.",
            (Self::ZhCn, S::Cleanup) => "清理托管源文件。",
            (Self::EnUs, S::Done) => "Plugin operation completed.",
            (Self::ZhCn, S::Done) => "插件操作已完成。",
        }
    }
    pub(crate) const fn plugin_provision_committed(self) -> &'static str {
        match self {
            Self::EnUs => "Registration change committed.",
            Self::ZhCn => "登记变更已提交。",
        }
    }
    pub(crate) const fn plugin_cleanup_pending(self) -> &'static str {
        match self {
            Self::EnUs => "Source cleanup pending",
            Self::ZhCn => "源文件清理未完成",
        }
    }
    pub(crate) const fn plugin_provision_failure(
        self,
        error: &orchbee_local_site_runtime::PluginProvisionFailure,
    ) -> &'static str {
        use orchbee_local_site_runtime::{PluginProvisionFailure as F, WindowsCommandFailure as C};
        match error {
            F::Registry(error) => self.plugin_error(*error),
            F::Command(C::Timeout) => match self {
                Self::EnUs => "Plugin installation reached its time limit.",
                Self::ZhCn => "插件安装达到执行期限。",
            },
            F::Command(reason) => self.plugin_command_failure(match reason {
                C::ExitFailure => orchbee_workbench_protocol::PluginCommandFailure::ExitFailure,
                C::SpawnUnavailable => {
                    orchbee_workbench_protocol::PluginCommandFailure::SpawnUnavailable
                }
                C::SpawnFailed => orchbee_workbench_protocol::PluginCommandFailure::SpawnFailed,
                C::Io | C::Timeout => orchbee_workbench_protocol::PluginCommandFailure::Io,
                C::Cancelled => orchbee_workbench_protocol::PluginCommandFailure::Cancelled,
                C::PhysicalUnknown => {
                    orchbee_workbench_protocol::PluginCommandFailure::PhysicalUnknown
                }
            }),
            F::ManifestChanged => match self {
                Self::EnUs => "Build changed the plugin manifest; installation was not committed.",
                Self::ZhCn => "构建修改了插件清单；未提交安装。",
            },
            F::LinkedConflict => match self {
                Self::EnUs => {
                    "A local source is already linked with this plugin ID; unlink it before installing."
                }
                Self::ZhCn => "此插件 ID 已链接本地源，请先解除链接再安装。",
            },
            F::CleanupIncomplete => match self {
                Self::EnUs => {
                    "Managed source cleanup is incomplete. Close processes using the listed paths before the next install/uninstall."
                }
                Self::ZhCn => {
                    "托管源文件清理未完成。请关闭占用所列路径的进程，再执行 install/uninstall。"
                }
            },
            F::PhysicalUnknown => match self {
                Self::EnUs => {
                    "Source process cleanup is uncertain; installation and cleanup are blocked."
                }
                Self::ZhCn => "源进程清理结果不确定，已阻止安装及文件清理。",
            },
        }
    }
}
