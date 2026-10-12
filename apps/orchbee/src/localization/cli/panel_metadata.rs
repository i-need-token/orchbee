use super::Locale;
use orchbee_workbench_protocol::{
    MetadataReportDisposition, PanelMetadataFailure as Failure, PanelMetadataOutcome as Outcome,
};
pub(super) const HELP_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel report-metadata <panel> [--presentation <id>] --source <id> [--title <text>] [--display-agent <text>] [--state-label <state=text>] [--agent <label>] [--applies-to-source <id>] [--clear-title] [--clear-display-agent] [--clear-state-labels] [--token <key=value>] [--clear-token <key>] [--seq <n>] [--ttl-ms <n>] [--format text|json]",
    "Report transient presentation metadata on a Windows Panel primary terminal. Use an exact Panel ID or @panel:<suffix>. --presentation selects an exact primary or child Presentation in that Panel; omit it for the primary. At least one field/token set or clear is required.\nStates: idle, working, blocked, done, error, unknown. Repeat state-label and token flags; later values win. Text normalizes to at most 80 Unicode scalars. Empty token values clear; empty presentation values are omitted.\nWithout clears, presentation fields replace this source record. Any presentation clear preserves other fields and their priority. Optional guards require an exact Agent label and authority source; absent context does not match.\nSequence: unsigned 64-bit; stale reports succeed without changes. TTL: 1–86400000 ms. Reports follow the bound terminal through moves; a changed binding rejects the report.\nOne existing-Site admission, five-second budget, no focus change or automatic retry. An uncertain submitted report exits 8 with its request ID. This does not change OSC title or Agent authority. Panel get/list/current and Agent get expose source-attributed metadata; guarded fields need actual authority context.",
);
pub(super) const HELP_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel report-metadata <panel> [--presentation <id>] --source <id> [--title <text>] [--display-agent <text>] [--state-label <state=text>] [--agent <label>] [--applies-to-source <id>] [--clear-title] [--clear-display-agent] [--clear-state-labels] [--token <key=value>] [--clear-token <key>] [--seq <n>] [--ttl-ms <n>] [--format text|json]",
    "向 Windows Panel 的主终端上报临时展示元数据。使用精确 Panel ID 或 @panel:<suffix>。--presentation 指定该 Panel 内精确的主项或子项 Presentation；省略则使用主项。至少设置或清除一个展示字段或 token。状态：idle、working、blocked、done、error、unknown。可重复 state-label 和 token 选项，后者覆盖前者。文本规范化为至多 80 个 Unicode 标量；空 token 值表示清除，空展示值省略。无清除标记时替换该来源的展示记录；包含展示清除标记时保留其他字段及其优先级。可选守卫要求 Agent 名称和权威来源精确匹配，缺少上下文不匹配。序号为无符号 64 位整数；旧上报成功返回但不修改。TTL：1–86400000 毫秒。数据随绑定终端移动；绑定改变时拒绝上报。仅连接已有 Site，总期限 5 秒，不改变焦点或自动重试。提交后的结果未知以退出码 8 和请求 ID 返回。不会修改 OSC 标题或 Agent 权威。Panel get/list/current 和 Agent get 提供带来源的元数据；带守卫字段需要真实权威上下文。",
);
impl Locale {
    pub(crate) const fn cli_panel_metadata_heading(self) -> &'static str {
        match self {
            Self::EnUs => "Panel metadata",
            Self::ZhCn => "Panel 元数据",
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
            }) => match self {
                Self::EnUs => "Metadata applied",
                Self::ZhCn => "元数据已应用",
            },
            Some(Outcome::Reported {
                disposition: MetadataReportDisposition::Stale,
                ..
            })
            | Some(Outcome::PresentationReported {
                disposition: MetadataReportDisposition::Stale,
                ..
            }) => match self {
                Self::EnUs => "Stale report ignored",
                Self::ZhCn => "已忽略旧序号上报",
            },
            Some(Outcome::Failed { reason }) => match (self, reason) {
                (Self::EnUs, Failure::InvalidRequest) => "Invalid metadata request",
                (Self::ZhCn, Failure::InvalidRequest) => "元数据请求无效",
                (Self::EnUs, Failure::SiteMismatch) => "Site identity changed",
                (Self::ZhCn, Failure::SiteMismatch) => "Site 身份已改变",
                (Self::EnUs, Failure::Unbound) => "Target member has no bound terminal",
                (Self::ZhCn, Failure::Unbound) => "目标成员未绑定终端",
                (Self::EnUs, Failure::BindingChanged) => "Target terminal binding changed",
                (Self::ZhCn, Failure::BindingChanged) => "目标终端绑定已改变",
                (Self::EnUs, Failure::NotFound) => "Panel or requested member is unavailable",
                (Self::ZhCn, Failure::NotFound) => "Panel 或请求成员已不可用",
                (Self::EnUs, Failure::Denied) => "Metadata reporting denied",
                (Self::ZhCn, Failure::Denied) => "元数据上报被拒绝",
                (Self::EnUs, Failure::Unavailable) => "Metadata service unavailable",
                (Self::ZhCn, Failure::Unavailable) => "元数据服务不可用",
                (Self::EnUs, Failure::TokenLimit) => "Panel token limit reached",
                (Self::ZhCn, Failure::TokenLimit) => "Panel token 数量已达上限",
                (Self::EnUs, Failure::SourceLimit) => "Sequence source limit reached",
                (Self::ZhCn, Failure::SourceLimit) => "序号来源数量已达上限",
                (Self::EnUs, Failure::ResourceLimit) => "Metadata capacity reached",
                (Self::ZhCn, Failure::ResourceLimit) => "元数据容量已达上限",
                (Self::EnUs, Failure::RevisionExhausted) => "Metadata revision exhausted",
                (Self::ZhCn, Failure::RevisionExhausted) => "元数据修订号已耗尽",
                (Self::EnUs, Failure::DeadlineOverflow) => "TTL deadline cannot be represented",
                (Self::ZhCn, Failure::DeadlineOverflow) => "无法表示 TTL 截止时间",
                (Self::EnUs, Failure::ResultTooLarge) => "Metadata response exceeds its limit",
                (Self::ZhCn, Failure::ResultTooLarge) => "元数据应答超出上限",
            },
            _ => match self {
                Self::EnUs => "Report outcome unknown; do not retry automatically",
                Self::ZhCn => "上报结果未知，请勿自动重试",
            },
        };
        let detail = unknown.map_or_else(String::new, |reason| {
            format!(" ({})", crate::cli::escape::escape(reason))
        });
        format!(
            "{state}{detail}\nPanel: {}\n{}: {}\n",
            crate::cli::escape::escape(panel_id),
            match self {
                Self::EnUs => "Request",
                Self::ZhCn => "请求",
            },
            crate::cli::escape::escape(request_id)
        )
    }
}
