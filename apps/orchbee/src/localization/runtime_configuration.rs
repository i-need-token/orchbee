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
    pub(crate) const fn runtime_configuration_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee site config|reload-config [--format text|json]\n\nRead the running Site's published configuration or reload its frozen config path.\nLive keys, window-title templates, Agent sidebar layouts and notification policy update main TUI Clients; manifest-check policy updates online.\nShell defaults apply to subsequent terminals; changed startup restore policy requires restart. No Site is started.\nFatal file/TOML errors keep the previous publication; admitted field warnings publish with safe defaults. A lost reply is not retried.\n"
            }
            Self::ZhCn => {
                "orchbee site config|reload-config [--format text|json]\n\n读取运行中 Site 已发布的配置，或重载其冻结的配置路径。\n键位、窗口标题模板、Agent 侧栏布局和通知策略在线更新主 TUI Client；manifest-check 策略在线更新。\nShell 默认值用于后续新终端；启动恢复策略变化需要重启。不启动 Site。\n致命文件/TOML 错误保留旧发布；已接纳的字段警告会随安全默认值一起发布。丢失响应不会自动重试。\n"
            }
        }
    }
}
