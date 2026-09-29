use super::Locale;
use orchbee_local_site_runtime::PluginRegistryError as Error;
impl Locale {
    pub(crate) const fn plugin_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee plugin link <path> [--enabled|--disabled]\norchbee plugin unlink|enable|disable|config-dir <plugin-id>\norchbee plugin list [plugin-id]\norchbee plugin action list [plugin-id] [--plugin ID]\norchbee plugin action invoke <action-id> [--plugin ID] [--context JSON]\norchbee plugin log list [plugin-id] [--plugin ID] [--limit 1..200]\n\nAll commands accept --format text|json and do not start a Site. Invoke/logs require an existing Site. Invoke accepts a local or qualified plugin.action ID. Logs default to the newest 20 retained commands, with 64 KiB per output stream and explicit truncation. Accepted work continues after CLI exit; Site shutdown stops it.\nLink accepts a manifest file or a directory containing orchbee-plugin.toml (herdr-plugin.toml fallback).\nLink registers without running commands. Unlink keeps source and user configuration/state files.\nAction list shows enabled, available Windows action definitions.\n"
            }
            Self::ZhCn => {
                "orchbee plugin link <path> [--enabled|--disabled]\norchbee plugin unlink|enable|disable|config-dir <plugin-id>\norchbee plugin list [plugin-id]\norchbee plugin action list [plugin-id] [--plugin ID]\norchbee plugin action invoke <action-id> [--plugin ID] [--context JSON]\norchbee plugin log list [plugin-id] [--plugin ID] [--limit 1..200]\n\n均支持 --format text|json，不启动 Site。invoke/log 需要已有 Site。invoke 支持 action ID 或 plugin.action ID。日志默认展示最近 20 条，每路保留 64 KiB 输出并标明截断。CLI 退出后已受理任务继续运行；停止 Site 会清理它们。\nlink 接受清单文件，或包含 orchbee-plugin.toml 的目录（缺失时使用 herdr-plugin.toml）。\nlink 只登记，不执行命令。unlink 保留源文件及用户配置/状态。\naction list 展示已启用、源有效且支持 Windows 的 action 定义。\n"
            }
        }
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
