//! CLI help and result copy; shared native-surface messages remain below this owner.
use super::Locale;
use orchbee_local_runtime::PluginRegistryError as Error;
impl super::CliText {
    pub(super) fn cli_plugin_preparation_failure(self, reason: &str) -> &'static str {
        match (self.locale, reason) {
            (Locale::EnUs, "registry-unavailable") => {
                "The plugin registry could not be read. Inspect `orchbee plugin list` and the user configuration directory.\n"
            }
            (Locale::ZhCn, "registry-unavailable") => {
                "无法读取插件注册表。请用 `orchbee plugin list` 查看，并检查用户配置目录。\n"
            }
            (Locale::EnUs, "plugin-not-found") => {
                "The plugin is not registered. Inspect `orchbee plugin list` and use its exact ID.\n"
            }
            (Locale::ZhCn, "plugin-not-found") => {
                "插件尚未注册。请用 `orchbee plugin list` 查看并使用其精确 ID。\n"
            }
            (Locale::EnUs, "plugin-disabled") => {
                "The plugin is disabled or has a registry warning. Inspect `orchbee plugin list` before opening its Panel.\n"
            }
            (Locale::ZhCn, "plugin-disabled") => {
                "插件已禁用或存在注册表警告。请先用 `orchbee plugin list` 查看，再打开其面板。\n"
            }
            (Locale::EnUs, "panel-not-found") => {
                "No plugin panel matches this target or entrypoint. Inspect `orchbee plugin list` and `orchbee panel list`.\n"
            }
            (Locale::ZhCn, "panel-not-found") => {
                "没有与目标或入口匹配的插件面板。请用 `orchbee plugin list` 和 `orchbee panel list` 查看。\n"
            }
            (Locale::EnUs, "unsupported-platform") => {
                "This plugin panel does not support the current platform.\n"
            }
            (Locale::ZhCn, "unsupported-platform") => "此插件面板不支持当前平台。\n",
            (Locale::EnUs, "invalid-title") => {
                "The plugin panel title is invalid. Check its manifest.\n"
            }
            (Locale::ZhCn, "invalid-title") => "插件面板标题无效。请检查其清单。\n",
            (Locale::EnUs, "invalid-geometry" | "invalid-popup-spec") => {
                "The plugin panel geometry or popup specification is invalid. Inspect `orchbee plugin panel open --help`.\n"
            }
            (Locale::ZhCn, "invalid-geometry" | "invalid-popup-spec") => {
                "插件面板的几何参数或弹窗规格无效。请查看 `orchbee plugin panel open --help`。\n"
            }
            (Locale::EnUs, "popup-focus-required") => {
                "A popup requires focus. Remove --no-focus, or select a tab or split placement.\n"
            }
            (Locale::ZhCn, "popup-focus-required") => {
                "弹窗需要焦点。请移除 --no-focus，或选择 tab／split 放置方式。\n"
            }
            (Locale::EnUs, "popup-target-unsupported") => {
                "This placement uses the current Client selection. Remove explicit target or launch geometry overrides; inspect `orchbee plugin panel open --help`.\n"
            }
            (Locale::ZhCn, "popup-target-unsupported") => {
                "此放置方式使用当前客户端选择。请移除显式目标或启动几何覆盖，并查看 `orchbee plugin panel open --help`。\n"
            }
            (Locale::EnUs, "client-listing-unavailable") => {
                "The TUI Client selection is unavailable or not ready. Inspect `orchbee client list` and pass --client <id>.\n"
            }
            (Locale::ZhCn, "client-listing-unavailable") => {
                "TUI 客户端选择不可用或尚未就绪。请用 `orchbee client list` 查看并传入 --client <id>。\n"
            }
            (Locale::EnUs, "client-not-ready") => {
                "The selected TUI Client is busy or not ready: a popup, dialog, history view, copy mode, pending change, or a window too small is holding it. Resolve it, then retry; `orchbee client list` shows its phase.\n"
            }
            (Locale::ZhCn, "client-not-ready") => {
                "所选 TUI 客户端忙碌或尚未就绪：popup、对话框、历史视图、复制模式、待完成的修改或过小的窗口正占用它。处理后再重试；`orchbee client list` 可查看其阶段。\n"
            }
            (Locale::EnUs, "binding-unavailable") => {
                "The current Client has no available terminal binding for this popup. Select a terminal panel first.\n"
            }
            (Locale::ZhCn, "binding-unavailable") => {
                "当前客户端没有可供此弹窗使用的终端绑定。请先选择终端面板。\n"
            }
            (Locale::EnUs, "client-changed" | "panel-changed") => {
                "The selected Client or plugin panel changed during preparation. Inspect the current selection before retrying.\n"
            }
            (Locale::ZhCn, "client-changed" | "panel-changed") => {
                "所选客户端或插件面板在准备期间发生变化。请检查当前选择后再重试。\n"
            }
            (Locale::EnUs, "context-invalid") => {
                "The plugin invocation context is invalid. Inspect `orchbee plugin action invoke --help`.\n"
            }
            (Locale::ZhCn, "context-invalid") => {
                "插件调用上下文无效。请查看 `orchbee plugin action invoke --help`。\n"
            }
            (Locale::EnUs, "transport-loss" | "invalid-reply" | "plugin-unavailable") => {
                "The local session could not provide the plugin panel information. Inspect its status before retrying.\n"
            }
            (Locale::ZhCn, "transport-loss" | "invalid-reply" | "plugin-unavailable") => {
                "本地会话无法提供插件面板信息。请检查其状态后再重试。\n"
            }
            (Locale::EnUs, _) => {
                "Plugin preparation failed before submission. Inspect the plugin configuration.\n"
            }
            (Locale::ZhCn, _) => "插件准备在提交前失败。请检查插件配置。\n",
        }
    }

    /// A Client refused a plugin request, during preparation or the popup open itself; `reason`
    /// is its closed protocol token.
    pub(super) fn cli_plugin_client_refused(self, reason: &str) -> String {
        let reason = self.client_control_reason(reason);
        match self.locale {
            Locale::EnUs => format!(
                "The TUI Client refused the plugin request ({reason}). Nothing was opened; resolve it and retry.\n"
            ),
            Locale::ZhCn => {
                format!("TUI 客户端拒绝了插件请求（{reason}）。未打开任何内容；处理后再重试。\n")
            }
        }
    }

    pub(crate) fn cli_popup_opened(
        self,
        popup_id: &orchbee_workbench_protocol::WorkbenchRequestId,
    ) -> String {
        match self.locale {
            Locale::EnUs => format!("Popup opened: {popup_id}\n"),
            Locale::ZhCn => format!("popup 已打开：{popup_id}\n"),
        }
    }

    pub(crate) const fn cli_popup_closed(self, had_popup: bool) -> &'static str {
        match (self.locale, had_popup) {
            (Locale::EnUs, true) => "Popup closed.\n",
            (Locale::EnUs, false) => "No popup is open.\n",
            (Locale::ZhCn, true) => "popup 已关闭。\n",
            (Locale::ZhCn, false) => "当前没有 popup。\n",
        }
    }

    pub(crate) fn plugin_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::cli_text::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::PluginInstall => (
                "orchbee plugin install <owner/repo[/subdir]> --yes [--ref <ref>] [--timeout <ms>] [--format text|json]",
                "Download plugin source from GitHub and run its Windows build steps. Git must be available in PATH. --yes is required because the downloaded code will run, including the plugin's startup and event commands, which run automatically in later sessions while it is enabled.\n\nThe work deadline defaults to 600000 ms (maximum 3600000), with up to five additional seconds to settle an active build. A failed build preserves the previous registration. Incomplete cleanup is reported for the next explicit install, update or uninstall. This command does not start a session.",
                "从 GitHub 下载插件源码并执行其 Windows 构建步骤。PATH 中必须有 Git；必须指定 --yes，以允许执行下载的代码，包括插件的启动命令和事件命令：插件启用期间，它们会在之后的会话中自动运行。\n\n工作时限默认 600000 毫秒，最大 3600000 毫秒；结束正在运行的构建最多再等待五秒。构建失败时保留原注册；未完成的清理会报告给下一次显式安装、更新或卸载。本命令不启动会话。",
            ),
            C::PluginUninstall => (
                "orchbee plugin uninstall <plugin-id|owner/repo[/subdir]> [--format text|json]",
                "Remove managed plugin source and its registration. User configuration and state remain. Incomplete cleanup is reported for the next explicit install, update or uninstall. This command does not start a session.",
                "移除托管的插件源码及其注册，保留用户配置和状态。未完成的清理会报告给下一次显式安装、更新或卸载。本命令不启动会话。",
            ),
            C::PluginUpdate => (
                "orchbee plugin update <plugin-id> --yes [--timeout <ms>] [--format text|json]",
                "Download an installed plugin again from its recorded GitHub source and ref, then run its Windows build steps. --yes is required because the downloaded code will run. Enabled state, configuration and state are kept, and nothing changes when the source commit is the same. A linked plugin has no recorded source and is refused.\n\nThe work deadline defaults to 600000 ms (maximum 3600000), with up to five additional seconds to settle an active build. A failed build preserves the previous registration. Startup commands of the updated plugin run from the next session start. This command does not start a session.",
                "按记录的 GitHub 来源和 ref 重新下载已安装的插件，并执行其 Windows 构建步骤。必须指定 --yes，以允许执行下载的代码。保留启用状态、配置和状态；源码提交未变化时不做任何改动。链接的插件没有记录来源，会被拒绝。\n\n工作时限默认 600000 毫秒，最大 3600000 毫秒；结束正在运行的构建最多再等待五秒。构建失败时保留原注册。更新后插件的启动命令从下一次会话启动时运行。本命令不启动会话。",
            ),
            C::PluginLink => (
                "orchbee plugin link <path> [--enabled|--disabled] [--format text|json]",
                "Register a local manifest file or a directory containing orchbee-plugin.toml. Registration does not run plugin commands or start a session; while the plugin is enabled, its startup and event commands run automatically in later sessions.",
                "注册本地清单文件，或包含 orchbee-plugin.toml 的目录。注册不运行插件命令，也不启动会话；插件启用期间，其启动命令和事件命令会在之后的会话中自动运行。",
            ),
            C::PluginUnlink => (
                "orchbee plugin unlink <plugin-id> [--format text|json]",
                "Remove the registration; keep source, configuration and state files. This command does not start a session.",
                "移除注册，保留源码、配置和状态文件。本命令不启动会话。",
            ),
            C::PluginEnable => (
                "orchbee plugin enable <plugin-id> [--format text|json]",
                "Enable the registered plugin. This command does not start a session.",
                "启用已注册的插件。本命令不启动会话。",
            ),
            C::PluginDisable => (
                "orchbee plugin disable <plugin-id> [--format text|json]",
                "Disable the registered plugin. This command does not start a session.",
                "禁用已注册的插件。本命令不启动会话。",
            ),
            C::PluginConfigDir => (
                "orchbee plugin config-dir <plugin-id> [--format text|json]",
                "Show the plugin configuration directory. This command does not start a session.",
                "显示插件配置目录。本命令不启动会话。",
            ),
            C::PluginList => (
                "orchbee plugin list [<plugin-id> | --plugin <id>] [--format text|json]",
                "List registered plugins, or inspect one plugin. This command does not start a session.",
                "列出已注册的插件，或查看指定插件。本命令不启动会话。",
            ),
            C::PluginActionList => (
                "orchbee plugin action list [<plugin-id>] [--plugin <id>] [--format text|json]",
                "List enabled, available Windows plugin actions. This command does not start a session.",
                "列出已启用且可用的 Windows 插件操作。本命令不启动会话。",
            ),
            C::PluginActionInvoke => (
                "orchbee plugin action invoke <action-id> [--plugin <id>] [--client <id>] [--context <json>] [--format text|json]",
                "Run an installed plugin action in an existing session. The ID can be local or qualified as plugin.action. Context follows the caller or the single current client; --client selects explicitly and --context overrides display fields. Inspect orchbee api schema orchbee.plugin.runtime ($defs/context); fields use camelCase, such as selectedText. Accepted work continues after the CLI exits; session shutdown stops it.",
                "在已运行的会话中执行插件操作。ID 可使用本地名称或 plugin.action 完整名称。上下文来自调用者或唯一当前客户端；--client 显式选择客户端，--context 覆盖显示字段。使用 orchbee api schema orchbee.plugin.runtime 查看输入结构（$defs/context）；字段使用 camelCase，例如 selectedText。CLI 退出后，已接纳的任务继续运行；会话关闭时停止。",
            ),
            C::PluginPaneOpen => (
                "orchbee plugin panel open --plugin <id> --entrypoint <id> [--placement tab|split|zoomed|overlay|popup] [--width <cells|percent%>] [--height <cells|percent%>] [--workspace <id>] [--target-panel <id>] [--direction right|down] [--cwd <path>] [--env NAME=VALUE] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--format text|json]",
                "Open a plugin terminal in an existing session. The default working directory is the plugin root.\n\nPopup placement opens an independent terminal in the selected current client and requires focus; --no-focus is refused. Width and height apply only to popups: use terminal cells or percentages. Defaults come from the manifest, otherwise half of the available area. Standard terminal geometry options do not apply. Escape and prefix keys reach the popup program. Click its close button or run orchbee popup close to dispose of it.",
                "在已运行的会话中打开插件终端，工作目录默认为插件根目录。\n\nPopup 在选中的当前客户端中打开独立终端且需要焦点；拒绝 --no-focus。宽高仅适用于 popup，可使用终端单元格数量或百分比；默认取清单配置，否则占可用区域的一半。不使用普通终端尺寸选项。Esc 和前缀键会传给弹出窗口中的程序；点击关闭按钮或执行 orchbee popup close 可关闭。",
            ),
            C::PluginPaneFocus => (
                "orchbee plugin panel focus <panel-id> [--client <id>] [--timeout <ms>] [--format text|json]",
                "Focus a Panel with recorded plugin membership in an existing session. Stale or ordinary Panels are refused.",
                "在已运行的会话中聚焦已记录插件归属的面板；拒绝已失效或普通面板。",
            ),
            C::PluginPaneClose => (
                "orchbee plugin panel close <panel-id> [--timeout <ms>] [--format text|json]",
                "Close a Panel with recorded plugin membership in an existing session and return its retained close receipt. Stale or ordinary Panels are refused.",
                "在已运行的会话中关闭已记录插件归属的面板，并返回保留的关闭回执；拒绝已失效或普通面板。",
            ),
            C::PopupClose => (
                "orchbee popup close [--client <id>] [--timeout <ms>] [--format text|json]",
                "Close the popup in the selected current client. The receipt waits until the popup terminal is disposed of. Requires an existing session.",
                "关闭选中当前客户端中的弹出窗口；回执等待弹出终端实际释放。需要已运行的会话。",
            ),
            C::PluginLogList => (
                "orchbee plugin log list [<plugin-id>] [--plugin <id>] [--limit <1..200>] [--format text|json]",
                "Read retained plugin command logs from an existing session. Default: the newest 50 commands, displayed oldest to newest. Each output stream is limited to 64 KiB and reports truncation explicitly.",
                "从已运行的会话读取保留的插件命令日志。默认显示最近 50 条命令，按从旧到新排序。每个输出流最多 64 KiB，并明确标记截断。",
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

    pub(crate) const fn plugin_result_heading(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Plugin catalog:",
            Locale::ZhCn => "插件目录：",
        }
    }

    pub(crate) const fn plugin_enabled(self, enabled: bool) -> &'static str {
        match (self.locale, enabled) {
            (Locale::EnUs, true) => "enabled",
            (Locale::EnUs, false) => "disabled",
            (Locale::ZhCn, true) => "已启用",
            (Locale::ZhCn, false) => "已禁用",
        }
    }

    pub(crate) const fn plugin_error(self, error: Error) -> &'static str {
        match (self.locale, error) {
            (Locale::EnUs, Error::InvalidManifest) => "Invalid plugin manifest.",
            (Locale::ZhCn, Error::InvalidManifest) => "插件清单无效。",
            (Locale::EnUs, Error::InvalidIdentifier) => "Invalid plugin identifier.",
            (Locale::ZhCn, Error::InvalidIdentifier) => "插件标识符无效。",
            (Locale::EnUs, Error::InvalidPath) => {
                "Plugin configuration or source path is unavailable or invalid."
            }
            (Locale::ZhCn, Error::InvalidPath) => "插件配置或源路径不可用或无效。",
            (Locale::EnUs, Error::InvalidRegistry) => {
                "Plugin catalog is invalid; existing data was retained."
            }
            (Locale::ZhCn, Error::InvalidRegistry) => "插件目录无效；保留现有数据。",
            (Locale::EnUs, Error::UnusableRegistry) => {
                "The plugin registry was saved by a different version of Orchbee; plugins can't be changed with this version."
            }
            (Locale::ZhCn, Error::UnusableRegistry) => {
                "插件注册表由其他版本的 Orchbee 保存；无法用此版本更改插件。"
            }
            (Locale::EnUs, Error::ManifestUnavailable) => {
                "Plugin source is unavailable; retained metadata cannot authorize execution."
            }
            (Locale::ZhCn, Error::ManifestUnavailable) => {
                "插件源不可用；保留的元数据不能授权执行。"
            }
            (Locale::EnUs, Error::PluginNotFound) => "Plugin is not registered.",
            (Locale::ZhCn, Error::PluginNotFound) => "插件尚未登记。",
            (Locale::EnUs, Error::LimitExceeded) => "Plugin manifest or catalog exceeds its limit.",
            (Locale::ZhCn, Error::LimitExceeded) => "插件清单或目录超过上限。",
            (Locale::EnUs, Error::Busy) => {
                "Plugin catalog is being modified; retry after the other operation finishes."
            }
            (Locale::ZhCn, Error::Busy) => "插件目录正在修改；请在另一项操作结束后重试。",
            (Locale::EnUs, Error::Unsupported) => {
                "Plugin storage is not supported on this platform."
            }
            (Locale::ZhCn, Error::Unsupported) => "此平台尚不支持插件存储。",
            (Locale::EnUs, Error::StorageUnavailable) => {
                "Plugin storage could not be safely read or updated."
            }
            (Locale::ZhCn, Error::StorageUnavailable) => "无法安全读取或更新插件存储。",
        }
    }

    pub(crate) const fn plugin_runtime_state(self, state: &str) -> &'static str {
        match (self.locale, state.as_bytes()) {
            (Locale::EnUs, b"operation-id") => "Operation ID",
            (Locale::ZhCn, b"operation-id") => "操作 ID",
            (Locale::EnUs, b"exit-code") => "Exit code",
            (Locale::ZhCn, b"exit-code") => "退出码",
            (Locale::EnUs, b"running") => "running",
            (Locale::ZhCn, b"running") => "运行中",
            (Locale::EnUs, b"succeeded") => "succeeded",
            (Locale::ZhCn, b"succeeded") => "执行成功",
            (Locale::EnUs, b"failed") => "failed",
            (Locale::ZhCn, b"failed") => "执行失败",
            (Locale::EnUs, b"truncated") => "Output exceeded 64 KiB per stream and was truncated.",
            (Locale::ZhCn, b"truncated") => "输出超出每路 64 KiB 的保留上限，已截断。",
            (Locale::EnUs, _) => {
                "Submission outcome unknown; inspect plugin logs before taking further action."
            }
            (Locale::ZhCn, _) => "提交结果未知；请先查询插件日志再进行下一步操作。",
        }
    }

    pub(crate) const fn plugin_runtime_failure(
        self,
        reason: orchbee_workbench_protocol::PluginFailure,
    ) -> &'static str {
        use orchbee_workbench_protocol::PluginFailure as F;
        match (self.locale, reason) {
            (Locale::EnUs, F::Denied) => "Plugin operation denied.",
            (Locale::ZhCn, F::Denied) => "无权执行插件操作。",
            (Locale::EnUs, F::Unavailable) => "Plugin runtime unavailable.",
            (Locale::ZhCn, F::Unavailable) => "插件运行时不可用。",
            (Locale::EnUs, F::SessionMismatch) => "Plugin session does not match.",
            (Locale::ZhCn, F::SessionMismatch) => "插件会话不匹配。",
            (Locale::EnUs, F::WorkerChanged) => {
                "Plugin Worker changed; the old log cursor is no longer valid."
            }
            (Locale::ZhCn, F::WorkerChanged) => "插件 Worker 已变化，旧日志游标失效。",
            (Locale::EnUs, F::InvalidIntent) => "Invalid plugin invocation.",
            (Locale::ZhCn, F::InvalidIntent) => "插件调用无效。",
            (Locale::EnUs, F::NotFound) => "Plugin action or log not found.",
            (Locale::ZhCn, F::NotFound) => "找不到插件 action 或日志。",
            (Locale::EnUs, F::Disabled) => "Plugin is disabled.",
            (Locale::ZhCn, F::Disabled) => "插件已禁用。",
            (Locale::EnUs, F::Unsupported) => "Plugin action is unavailable on Windows.",
            (Locale::ZhCn, F::Unsupported) => "插件 action 不支持 Windows。",
            (Locale::EnUs, F::SourceUnavailable) => "Plugin source or catalog is unavailable.",
            (Locale::ZhCn, F::SourceUnavailable) => "插件源文件或目录不可用。",
            (Locale::EnUs, F::Ambiguous) => "Action ID is ambiguous; specify --plugin.",
            (Locale::ZhCn, F::Ambiguous) => "action ID 不唯一，请指定 --plugin。",
            (Locale::EnUs, F::Busy) => "Plugin execution capacity is full.",
            (Locale::ZhCn, F::Busy) => "插件执行容量已满。",
            (Locale::EnUs, F::Conflict) => {
                "The operation ID already identifies a different invocation."
            }
            (Locale::ZhCn, F::Conflict) => "操作 ID 已用于不同的调用。",
            (Locale::EnUs, F::PhysicalUnknown) => {
                "Plugin process cleanup is uncertain; new execution is blocked."
            }
            (Locale::ZhCn, F::PhysicalUnknown) => "插件进程清理结果不确定，已阻止新执行。",
        }
    }

    pub(crate) const fn plugin_command_failure(
        self,
        reason: orchbee_workbench_protocol::PluginCommandFailure,
    ) -> &'static str {
        use orchbee_workbench_protocol::PluginCommandFailure as F;
        match (self.locale, reason) {
            (Locale::EnUs, F::ExitFailure) => "The command exited unsuccessfully.",
            (Locale::ZhCn, F::ExitFailure) => "命令以失败状态退出。",
            (Locale::EnUs, F::SpawnUnavailable) => "The command executable was not found.",
            (Locale::ZhCn, F::SpawnUnavailable) => "找不到命令可执行文件。",
            (Locale::EnUs, F::SpawnFailed) => "The command could not be started.",
            (Locale::ZhCn, F::SpawnFailed) => "无法启动命令。",
            (Locale::EnUs, F::Io) => "Command output or process observation failed.",
            (Locale::ZhCn, F::Io) => "读取命令输出或观察进程失败。",
            (Locale::EnUs, F::Cancelled) => {
                "The command was cancelled and its process Job settled."
            }
            (Locale::ZhCn, F::Cancelled) => "命令已取消，其进程 Job 已清理。",
            (Locale::EnUs, F::PhysicalUnknown) => "Process Job cleanup is uncertain.",
            (Locale::ZhCn, F::PhysicalUnknown) => "进程 Job 的清理结果不确定。",
        }
    }

    pub(crate) const fn plugin_provision_stage(
        self,
        stage: orchbee_local_runtime::PluginProvisionStage,
    ) -> &'static str {
        use orchbee_local_runtime::PluginProvisionStage as S;
        match (self.locale, stage) {
            (Locale::EnUs, S::Prepare) => "Preparing plugin source.",
            (Locale::ZhCn, S::Prepare) => "准备插件源。",
            (Locale::EnUs, S::Checkout) => "Downloading plugin source.",
            (Locale::ZhCn, S::Checkout) => "下载插件源。",
            (Locale::EnUs, S::Inspect) => "Validating plugin manifest.",
            (Locale::ZhCn, S::Inspect) => "校验插件清单。",
            (Locale::EnUs, S::Build) => "Building plugin.",
            (Locale::ZhCn, S::Build) => "构建插件。",
            (Locale::EnUs, S::Commit) => "Updating plugin registration.",
            (Locale::ZhCn, S::Commit) => "更新插件登记。",
            (Locale::EnUs, S::Cleanup) => "Cleaning managed source files.",
            (Locale::ZhCn, S::Cleanup) => "清理托管源文件。",
            (Locale::EnUs, S::Done) => "Plugin operation completed.",
            (Locale::ZhCn, S::Done) => "插件操作已完成。",
        }
    }

    pub(crate) const fn plugin_provision_committed(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Registration change committed.",
            Locale::ZhCn => "登记变更已提交。",
        }
    }

    pub(crate) const fn plugin_provision_current(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Plugin is already up to date.",
            Locale::ZhCn => "插件已是最新。",
        }
    }

    pub(crate) const fn plugin_cleanup_pending(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Source cleanup pending",
            Locale::ZhCn => "源文件清理未完成",
        }
    }

    pub(crate) const fn plugin_provision_failure(
        self,
        error: &orchbee_local_runtime::PluginProvisionFailure,
    ) -> &'static str {
        use orchbee_local_runtime::{CommandFailure as C, PluginProvisionFailure as F};
        match error {
            F::Registry(error) => self.plugin_error(*error),
            F::Command(C::Timeout) => match self.locale {
                Locale::EnUs => "The plugin source operation reached its time limit.",
                Locale::ZhCn => "插件源操作达到执行期限。",
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
            F::ManifestChanged => match self.locale {
                Locale::EnUs => "Build changed the plugin manifest; the change was not committed.",
                Locale::ZhCn => "构建修改了插件清单；未提交变更。",
            },
            F::LinkedConflict => match self.locale {
                Locale::EnUs => {
                    "A local source is already linked with this plugin ID; unlink it before installing."
                }
                Locale::ZhCn => "此插件 ID 已链接本地源，请先解除链接再安装。",
            },
            F::SourceUnrecorded => match self.locale {
                Locale::EnUs => {
                    "This plugin is linked from a local source and has no recorded install source; update the local source directly."
                }
                Locale::ZhCn => "此插件链接自本地源，没有记录的安装来源；请直接更新本地源。",
            },
            F::IdentityChanged => match self.locale {
                Locale::EnUs => {
                    "The updated source declares a different plugin ID; the update was not committed."
                }
                Locale::ZhCn => "更新后的源码声明了不同的插件 ID；未提交更新。",
            },
            F::CleanupIncomplete => match self.locale {
                Locale::EnUs => {
                    "Managed source cleanup is incomplete. Close processes using the listed paths before the next install/update/uninstall."
                }
                Locale::ZhCn => {
                    "托管源文件清理未完成。请关闭占用所列路径的进程，再执行 install/update/uninstall。"
                }
            },
            F::PhysicalUnknown => match self.locale {
                Locale::EnUs => {
                    "Source process cleanup is uncertain; installation and cleanup are blocked."
                }
                Locale::ZhCn => "源进程清理结果不确定，已阻止安装及文件清理。",
            },
        }
    }
}
