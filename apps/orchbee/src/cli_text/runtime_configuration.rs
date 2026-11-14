//! Formats effective-configuration evidence and command guidance without reading configuration or environment state.
use super::Locale;
impl super::CliText {
    pub(crate) fn cli_runtime_configuration(self, outcome: &str, revision: Option<u64>) -> String {
        let text = match (self.locale, outcome) {
            (Locale::EnUs, "startup") => {
                "Session startup baseline; no live configuration has been published yet."
            }
            (Locale::ZhCn, "startup") => "会话启动基线；尚未发布在线配置。",
            (Locale::EnUs, "unknown") => {
                "Configuration outcome unknown; run 'orchbee config reload' to confirm the published settings."
            }
            (Locale::ZhCn, "unknown") => {
                "配置结果未知；请运行 orchbee config reload 确认已发布的设置。"
            }
            (Locale::EnUs, "failed") => {
                "Configuration reload/read failed; previous published settings retained."
            }
            (Locale::ZhCn, "failed") => "配置重载或读取失败；保留此前发布的设置。",
            (Locale::EnUs, "warning") => {
                "Configuration published with warnings; inspect 'orchbee config check'."
            }
            (Locale::ZhCn, "warning") => "配置已发布但含警告；请运行 'orchbee config check' 查看。",
            (Locale::EnUs, _) => "Configuration published for connected Clients.",
            (Locale::ZhCn, _) => "配置已发布，连接中的客户端将接收更新。",
        };
        format!(
            "{text}\nrevision={}\n",
            revision.map_or_else(|| "-".into(), |n| n.to_string())
        )
    }
    pub(crate) fn cli_configuration_notifications(self, profile: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("Notification policy: \"{profile}\"\n"),
            Locale::ZhCn => format!("通知策略：\"{profile}\"\n"),
        }
    }
    pub(crate) fn cli_configuration_title(self, escaped_template: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("Window title template: \"{escaped_template}\"\n"),
            Locale::ZhCn => format!("窗口标题模板：\"{escaped_template}\"\n"),
        }
    }
    pub(crate) fn cli_configuration_sidebar(self, escaped_profile: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("Agent sidebar layout: \"{escaped_profile}\"\n"),
            Locale::ZhCn => format!("智能体边栏布局：\"{escaped_profile}\"\n"),
        }
    }
    pub(crate) fn cli_configuration_diagnostics(self, visible: usize, omitted: u32) -> String {
        match self.locale {
            Locale::EnUs => {
                format!("Configuration warnings: visible={visible} omitted={omitted}\n")
            }
            Locale::ZhCn => format!("配置警告：visible={visible} omitted={omitted}\n"),
        }
    }
    pub(crate) fn cli_configuration_diagnostic(
        self,
        escaped_field: &str,
        reason: &str,
        disposition: &str,
    ) -> String {
        match self.locale {
            Locale::EnUs => format!(
                "Warning: field=\"{escaped_field}\" reason={reason} disposition={disposition}\n"
            ),
            Locale::ZhCn => format!(
                "警告：field=\"{escaped_field}\" reason={reason} disposition={disposition}\n"
            ),
        }
    }
    pub(crate) fn runtime_configuration_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::cli_text::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::ConfigReload => (
                "orchbee config reload [--format text|json]",
                "Reload the configuration path selected when the session started. Updates shortcuts, window titles, Agent sidebar layouts, notification policy and manifest checks. Startup restore settings require a restart.\n\nFile or TOML errors preserve the previous configuration. Individual field warnings use safe defaults. Requires an existing session; a lost reply is never retried automatically.",
                "重新加载会话启动时选定的配置路径。更新快捷键、窗口标题、智能体边栏布局、通知策略和清单检查。启动恢复设置需要重启。\n\n文件或 TOML 错误会保留原配置；单字段警告使用安全默认值。需要已运行的会话；应答丢失时不自动重试。",
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
}
