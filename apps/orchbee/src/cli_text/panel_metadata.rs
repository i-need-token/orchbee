//! Formats Panel metadata command guidance and receipts without interpreting them as input authority.
use super::Locale;
use orchbee_workbench_protocol::{
    MetadataReportDisposition, PanelMetadataFailure as Failure, PanelMetadataOutcome as Outcome,
};
pub(super) const HELP_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee panel report-metadata <panel> [--presentation <id>] --source <id> [--title <text>] [--display-agent <text>] [--state-label <state=text>] [--agent <label>] [--applies-to-source <id>] [--clear-title] [--clear-display-agent] [--clear-state-labels] [--token <key=value>] [--clear-token <key>] [--seq <n>] [--ttl-ms <n>] [--format text|json]",
    "Report transient presentation metadata on a Windows Panel primary terminal. Use an exact Panel ID or @panel:0x<hex>. --presentation selects an exact primary or child Presentation in that Panel; omit it for the primary. At least one field/token set or clear is required.\nStates: idle, working, blocked, done, error, unknown. Repeat state-label and token flags; later values win. Text normalizes to at most 80 Unicode scalars. Empty token values clear; empty presentation values are omitted.\nWithout clears, presentation fields replace this source record. Any presentation clear preserves other fields and their priority. Optional guards require an exact Agent label and authority source; absent context does not match.\nSequence: unsigned 64-bit; stale reports succeed without changes. TTL: 1–86400000 ms. Reports follow the bound terminal through moves; a changed binding rejects the report.\nOne existing-session admission, five-second budget, no focus change or automatic retry. An uncertain submitted report exits 8 with its request ID. This does not change OSC title or Agent authority. Panel get/list/current and Agent get expose source-attributed metadata; guarded fields need actual authority context.",
);
pub(super) const HELP_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee panel report-metadata <panel> [--presentation <id>] --source <id> [--title <text>] [--display-agent <text>] [--state-label <state=text>] [--agent <label>] [--applies-to-source <id>] [--clear-title] [--clear-display-agent] [--clear-state-labels] [--token <key=value>] [--clear-token <key>] [--seq <n>] [--ttl-ms <n>] [--format text|json]",
    "向 Windows 面板的主终端上报临时展示元数据。使用精确面板 ID 或 @panel:0x<hex>。--presentation 指定该面板内精确的主项或子项 Presentation；省略则使用主项。至少设置或清除一个展示字段或 token。状态：idle、working、blocked、done、error、unknown。可重复 state-label 和 token 选项，后者覆盖前者。文本规范化为至多 80 个 Unicode 标量；空 token 值表示清除，空展示值省略。无清除标记时替换该来源的展示记录；包含展示清除标记时保留其他字段及其优先级。可选守卫要求智能体名称和权威来源精确匹配，缺少上下文不匹配。序号为无符号 64 位整数；旧上报成功返回但不修改。TTL：1–86400000 毫秒。数据随绑定终端移动；绑定改变时拒绝上报。仅连接已有会话，总期限 5 秒，不改变焦点或自动重试。提交后的结果未知以退出码 8 和请求 ID 返回。不会修改 OSC 标题或智能体权威。面板 get/list/current 和智能体 get 提供带来源的元数据；带守卫字段需要真实权威上下文。",
);
impl super::CliText {
    pub(crate) const fn cli_panel_metadata_heading(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Panel metadata",
            Locale::ZhCn => "面板元数据",
        }
    }
    pub(crate) fn cli_panel_metadata(
        self,
        request_id: &str,
        panel_id: &str,
        outcome: Option<&Outcome>,
        unknown: Option<&str>,
    ) -> String {
        let state = match outcome {
            Some(Outcome::Reported {
                disposition: MetadataReportDisposition::Applied,
                ..
            })
            | Some(Outcome::PresentationReported {
                disposition: MetadataReportDisposition::Applied,
                ..
            }) => match self.locale {
                Locale::EnUs => "Metadata applied",
                Locale::ZhCn => "元数据已应用",
            },
            Some(Outcome::Reported {
                disposition: MetadataReportDisposition::Stale,
                ..
            })
            | Some(Outcome::PresentationReported {
                disposition: MetadataReportDisposition::Stale,
                ..
            }) => match self.locale {
                Locale::EnUs => "Stale report ignored",
                Locale::ZhCn => "已忽略旧序号上报",
            },
            Some(Outcome::Failed { reason }) => match (self.locale, reason) {
                (Locale::EnUs, Failure::InvalidRequest) => "Invalid metadata request",
                (Locale::ZhCn, Failure::InvalidRequest) => "元数据请求无效",
                (Locale::EnUs, Failure::SessionMismatch) => "Session identity changed",
                (Locale::ZhCn, Failure::SessionMismatch) => "会话身份已改变",
                (Locale::EnUs, Failure::Unbound) => "Target member has no bound terminal",
                (Locale::ZhCn, Failure::Unbound) => "目标成员未绑定终端",
                (Locale::EnUs, Failure::BindingChanged) => "Target terminal binding changed",
                (Locale::ZhCn, Failure::BindingChanged) => "目标终端绑定已改变",
                (Locale::EnUs, Failure::NotFound) => "Panel or requested member is unavailable",
                (Locale::ZhCn, Failure::NotFound) => "面板或请求成员已不可用",
                (Locale::EnUs, Failure::Denied) => "Metadata reporting denied",
                (Locale::ZhCn, Failure::Denied) => "元数据上报被拒绝",
                (Locale::EnUs, Failure::Unavailable) => "Metadata service unavailable",
                (Locale::ZhCn, Failure::Unavailable) => "元数据服务不可用",
                (Locale::EnUs, Failure::TokenLimit) => "Panel token limit reached",
                (Locale::ZhCn, Failure::TokenLimit) => "面板 token 数量已达上限",
                (Locale::EnUs, Failure::SourceLimit) => "Sequence source limit reached",
                (Locale::ZhCn, Failure::SourceLimit) => "序号来源数量已达上限",
                (Locale::EnUs, Failure::ResourceLimit) => "Metadata capacity reached",
                (Locale::ZhCn, Failure::ResourceLimit) => "元数据容量已达上限",
                (Locale::EnUs, Failure::RevisionExhausted) => "Metadata revision exhausted",
                (Locale::ZhCn, Failure::RevisionExhausted) => "元数据修订号已耗尽",
                (Locale::EnUs, Failure::DeadlineOverflow) => "TTL deadline cannot be represented",
                (Locale::ZhCn, Failure::DeadlineOverflow) => "无法表示 TTL 截止时间",
                (Locale::EnUs, Failure::ResultTooLarge) => "Metadata response exceeds its limit",
                (Locale::ZhCn, Failure::ResultTooLarge) => "元数据应答超出上限",
            },
            // This query consumes only its requested metadata/report payload; other receipt classes cannot answer that query.
            _ => match self.locale {
                Locale::EnUs => "Report outcome unknown; do not retry automatically",
                Locale::ZhCn => "上报结果未知，请勿自动重试",
            },
        };
        let detail = unknown.map_or_else(String::new, |reason| {
            format!(" ({})", crate::cli::escape::escape(reason))
        });
        format!(
            "{state}{detail}\nPanel: {}\n{}: {}\n",
            crate::cli::escape::escape(panel_id),
            match self.locale {
                Locale::EnUs => "Request",
                Locale::ZhCn => "请求",
            },
            crate::cli::escape::escape(request_id)
        )
    }
}
