//! Formats Workspace metadata command guidance and receipts without modifying metadata.
use super::Locale;
use orchbee_workbench_protocol::{
    MetadataReportDisposition, WorkspaceMetadataFailure as Failure,
    WorkspaceMetadataOutcome as Outcome,
};
pub(super) const HELP_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee workspace report-metadata <target> --source <id> [--token <name=value>] [--clear-token <name>] [--seq <n>] [--ttl-ms <n>] [--format text|json]",
    "Report transient display tokens on an existing Workspace. At least one token change is required.\nRepeat token/clear flags in order; later values win. Values normalize to 80 Unicode characters; empty values clear.\nOptional sequence is unsigned 64-bit; stale reports succeed without changing values or TTL. TTL: 1–86400000 ms.\nThis command never starts a session, changes focus or retries an uncertain report. Total deadline: 5 seconds.",
);
pub(super) const HELP_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee workspace report-metadata <target> --source <id> [--token <name=value>] [--clear-token <name>] [--seq <n>] [--ttl-ms <n>] [--format text|json]",
    "向已有工作区上报临时展示 token，至少设置或清除一个键。可按顺序重复 token／clear 选项，后者覆盖前者。值规范化为至多 80 个 Unicode 字符，空值表示清除。可选序号为无符号 64 位整数；旧序号成功返回但不改变值或 TTL。TTL：1–86400000 毫秒。不会启动会话、改变焦点或重试不确定的上报。总期限为 5 秒。",
);
impl super::CliText {
    pub(crate) const fn cli_workspace_metadata_heading(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Workspace metadata",
            Locale::ZhCn => "工作区元数据",
        }
    }

    pub(crate) fn cli_workspace_metadata(
        self,
        request_id: &str,
        workspace_id: &str,
        outcome: Option<&Outcome>,
        unknown: Option<&str>,
    ) -> String {
        let state = match outcome {
            Some(Outcome::Reported {
                disposition: MetadataReportDisposition::Applied,
                ..
            }) => match self.locale {
                Locale::EnUs => "Metadata applied",
                Locale::ZhCn => "元数据已应用",
            },
            Some(Outcome::Reported {
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
                (Locale::EnUs, Failure::NotFound) => "Workspace no longer exists",
                (Locale::ZhCn, Failure::NotFound) => "工作区已不存在",
                (Locale::EnUs, Failure::Denied) => "Metadata reporting denied",
                (Locale::ZhCn, Failure::Denied) => "元数据上报被拒绝",
                (Locale::EnUs, Failure::Unavailable) => "Metadata service unavailable",
                (Locale::ZhCn, Failure::Unavailable) => "元数据服务不可用",
                (Locale::EnUs, Failure::TokenLimit) => "Workspace token limit reached",
                (Locale::ZhCn, Failure::TokenLimit) => "工作区 token 数量已达上限",
                (Locale::EnUs, Failure::SequenceSourceLimit) => "Sequence source limit reached",
                (Locale::ZhCn, Failure::SequenceSourceLimit) => "序号来源数量已达上限",
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
            format!(" ({})", crate::cli_text::escape::escape(reason))
        });
        format!(
            "{state}{detail}\nWorkspace: {}\n{}: {}\n",
            crate::cli_text::escape::escape(workspace_id),
            match self.locale {
                Locale::EnUs => "Request",
                Locale::ZhCn => "请求",
            },
            crate::cli_text::escape::escape(request_id)
        )
    }
}
