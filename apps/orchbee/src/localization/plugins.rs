use super::Locale;
use orchbee_local_site_runtime::PluginRegistryError as Error;
impl Locale {
    pub(crate) const fn plugin_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee plugin link <path> [--enabled|--disabled]\norchbee plugin unlink|enable|disable|config-dir <plugin-id>\norchbee plugin list [plugin-id]\norchbee plugin action list [plugin-id]\n\nAll commands accept --format text|json and do not start a Site.\nLink accepts a manifest file or a directory containing orchbee-plugin.toml (herdr-plugin.toml fallback).\nLink registers without running commands. Unlink keeps source and user configuration/state files.\nAction list shows enabled, available Windows action definitions.\n"
            }
            Self::ZhCn => {
                "orchbee plugin link <path> [--enabled|--disabled]\norchbee plugin unlink|enable|disable|config-dir <plugin-id>\norchbee plugin list [plugin-id]\norchbee plugin action list [plugin-id]\n\n均支持 --format text|json，不启动 Site。\nlink 接受清单文件，或包含 orchbee-plugin.toml 的目录（缺失时使用 herdr-plugin.toml）。\nlink 只登记，不执行命令。unlink 保留源文件及用户配置/状态。\naction list 展示已启用、源有效且支持 Windows 的 action 定义。\n"
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
