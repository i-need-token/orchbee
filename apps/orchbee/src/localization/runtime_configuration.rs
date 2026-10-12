use super::Locale;
impl Locale {
    pub(crate) fn cli_runtime_configuration(
        self,
        outcome: &str,
        revision: Option<u64>,
        restart: usize,
    ) -> String {
        let text = match (self, outcome) {
            (Self::EnUs, "startup") => {
                "Site startup baseline; no live configuration has been published yet."
            }
            (Self::ZhCn, "startup") => "Site 启动基线；尚未发布在线配置。",
            (Self::EnUs, "unknown") => {
                "Configuration outcome unknown; inspect 'site config' before retrying."
            }
            (Self::ZhCn, "unknown") => "配置结果未知；重试前请用 site config 查看当前状态。",
            (Self::EnUs, "failed") => {
                "Configuration reload/read failed; previous published settings retained."
            }
            (Self::ZhCn, "failed") => "配置重载或读取失败；保留此前发布的设置。",
            (Self::EnUs, "warning") => {
                "Configuration published with warnings; inspect 'orchbee config check'."
            }
            (Self::ZhCn, "warning") => "配置已发布但含警告；请运行 'orchbee config check' 查看。",
            (Self::EnUs, "restart") => {
                "Configuration published; some settings require a new Site lifetime."
            }
            (Self::ZhCn, "restart") => "配置已发布；部分设置需要重新启动 Site。",
            (Self::EnUs, _) => "Configuration published for connected Clients.",
            (Self::ZhCn, _) => "配置已发布，连接中的 Client 将接收更新。",
        };
        format!(
            "{text}\nrevision={} restartRequired={restart}\n",
            revision.map_or_else(|| "-".into(), |n| n.to_string())
        )
    }
    pub(crate) fn cli_configuration_shell(self, escaped_shell: &str, mode: &str) -> String {
        let shell = if escaped_shell.is_empty() {
            match self {
                Self::EnUs => "(platform default)",
                Self::ZhCn => "（平台默认）",
            }
        } else {
            escaped_shell
        };
        match self {
            Self::EnUs => format!("New terminal defaults: shell=\"{shell}\" mode={mode}\n"),
            Self::ZhCn => format!("新终端默认值：shell=\"{shell}\" mode={mode}\n"),
        }
    }
    pub(crate) fn cli_configuration_notifications(self, profile: &str) -> String {
        match self {
            Self::EnUs => format!("Notification policy: \"{profile}\"\n"),
            Self::ZhCn => format!("通知策略：\"{profile}\"\n"),
        }
    }
    pub(crate) fn cli_configuration_title(self, escaped_template: &str) -> String {
        match self {
            Self::EnUs => format!("Window title template: \"{escaped_template}\"\n"),
            Self::ZhCn => format!("窗口标题模板：\"{escaped_template}\"\n"),
        }
    }
    pub(crate) fn cli_configuration_sidebar(self, escaped_profile: &str) -> String {
        match self {
            Self::EnUs => format!("Agent sidebar layout: \"{escaped_profile}\"\n"),
            Self::ZhCn => format!("Agent 侧栏布局：\"{escaped_profile}\"\n"),
        }
    }
    pub(crate) fn cli_configuration_diagnostics(self, visible: usize, omitted: u32) -> String {
        match self {
            Self::EnUs => format!("Configuration warnings: visible={visible} omitted={omitted}\n"),
            Self::ZhCn => format!("配置警告：visible={visible} omitted={omitted}\n"),
        }
    }
    pub(crate) fn cli_configuration_diagnostic(
        self,
        escaped_field: &str,
        reason: &str,
        disposition: &str,
    ) -> String {
        match self {
            Self::EnUs => format!(
                "Warning: field=\"{escaped_field}\" reason={reason} disposition={disposition}\n"
            ),
            Self::ZhCn => format!(
                "警告：field=\"{escaped_field}\" reason={reason} disposition={disposition}\n"
            ),
        }
    }
    pub(crate) fn runtime_configuration_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::localization::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::SiteConfig => (
                "orchbee site config [--format text|json]",
                "Read the configuration currently used by the running Site. This command neither reloads configuration nor starts a Site.",
                "读取正在运行的 Site 当前使用的配置。本命令不重新加载配置，也不启动 Site。",
            ),
            C::SiteReloadConfig => (
                "orchbee site reload-config [--format text|json]",
                "Reload the configuration path selected when the Site started. Updates shortcuts, window titles, Agent sidebar layouts, notification policy and manifest checks. Shell defaults affect new terminals; startup restore settings require a restart.\n\nFile or TOML errors preserve the previous configuration. Individual field warnings use safe defaults. Requires an existing Site; a lost reply is never retried automatically.",
                "重新加载 Site 启动时选定的配置路径。更新快捷键、窗口标题、Agent 侧边栏布局、通知策略和清单检查。Shell 默认值影响新终端；启动恢复设置需要重启。\n\n文件或 TOML 错误会保留原配置；单字段警告使用安全默认值。需要已运行的 Site；应答丢失时不自动重试。",
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
}
