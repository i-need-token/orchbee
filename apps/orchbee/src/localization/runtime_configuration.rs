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
    pub(crate) const fn runtime_configuration_help(self) -> &'static str {
        match self {
            Self::EnUs => {
                "orchbee site config|reload-config [--format text|json]\n\nRead the running Site's published configuration or reload its frozen config path.\nLive keys are published to main TUI Clients; manifest-check policy updates online.\nChanged shell/startup restore policy is reported as restartRequired. No Site is started.\nInvalid configuration keeps the previous publication. A lost reply is not retried.\n"
            }
            Self::ZhCn => {
                "orchbee site config|reload-config [--format text|json]\n\n读取运行中 Site 已发布的配置，或重载其冻结的配置路径。\n键位发布到主 TUI Client；manifest-check 策略在线更新。\nShell/启动恢复策略变化通过 restartRequired 返回。不启动 Site。\n无效配置保留旧发布；丢失响应不会自动重试。\n"
            }
        }
    }
}
