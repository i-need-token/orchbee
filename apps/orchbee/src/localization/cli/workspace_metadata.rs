use super::Locale;
use orchbee_workbench_protocol::{
    MetadataReportDisposition, WorkspaceMetadataFailure as Failure,
    WorkspaceMetadataOutcome as Outcome,
};
pub(super) const HELP_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee workspace report-metadata <target> --source <id> [--token <name=value>] [--clear-token <name>] [--seq <n>] [--ttl-ms <n>] [--format text|json]",
    "Report transient display tokens on an existing Workspace. At least one token change is required.\nRepeat token/clear flags in order; later values win. Values normalize to 80 Unicode characters; empty values clear.\nOptional sequence is unsigned 64-bit; stale reports succeed without changing values or TTL. TTL: 1–86400000 ms.\nThis command never starts a session, changes focus or retries an uncertain report. Total deadline: 5 seconds.",
);
pub(super) const HELP_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee workspace report-metadata <target> --source <id> [--token <name=value>] [--clear-token <name>] [--seq <n>] [--ttl-ms <n>] [--format text|json]",
    "向已有工作区上报临时展示 token，至少设置或清除一个键。可按顺序重复 token／clear 选项，后者覆盖前者。值规范化为至多 80 个 Unicode 字符，空值表示清除。可选序号为无符号 64 位整数；旧序号成功返回但不改变值或 TTL。TTL：1–86400000 毫秒。不会启动会话、改变焦点或重试不确定的上报。总期限为 5 秒。",
);
impl Locale {
    pub(crate) const fn cli_workspace_metadata_heading(self) -> &'static str {
        match self {
            Self::EnUs => "Workspace metadata",
            Self::ZhCn => "工作区元数据",
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
            }) => match self {
                Self::EnUs => "Metadata applied",
                Self::ZhCn => "元数据已应用",
            },
            Some(Outcome::Reported {
                disposition: MetadataReportDisposition::Stale,
                ..
            }) => match self {
                Self::EnUs => "Stale report ignored",
                Self::ZhCn => "已忽略旧序号上报",
            },
            Some(Outcome::Failed { reason }) => match (self, reason) {
                (Self::EnUs, Failure::InvalidRequest) => "Invalid metadata request",
                (Self::ZhCn, Failure::InvalidRequest) => "元数据请求无效",
                (Self::EnUs, Failure::SessionMismatch) => "Session identity changed",
                (Self::ZhCn, Failure::SessionMismatch) => "会话身份已改变",
                (Self::EnUs, Failure::NotFound) => "Workspace no longer exists",
                (Self::ZhCn, Failure::NotFound) => "工作区已不存在",
                (Self::EnUs, Failure::Denied) => "Metadata reporting denied",
                (Self::ZhCn, Failure::Denied) => "元数据上报被拒绝",
                (Self::EnUs, Failure::Unavailable) => "Metadata service unavailable",
                (Self::ZhCn, Failure::Unavailable) => "元数据服务不可用",
                (Self::EnUs, Failure::TokenLimit) => "Workspace token limit reached",
                (Self::ZhCn, Failure::TokenLimit) => "工作区 token 数量已达上限",
                (Self::EnUs, Failure::SequenceSourceLimit) => "Sequence source limit reached",
                (Self::ZhCn, Failure::SequenceSourceLimit) => "序号来源数量已达上限",
                (Self::EnUs, Failure::ResourceLimit) => "Metadata capacity reached",
                (Self::ZhCn, Failure::ResourceLimit) => "元数据容量已达上限",
                (Self::EnUs, Failure::RevisionExhausted) => "Metadata revision exhausted",
                (Self::ZhCn, Failure::RevisionExhausted) => "元数据修订号已耗尽",
                (Self::EnUs, Failure::DeadlineOverflow) => "TTL deadline cannot be represented",
                (Self::ZhCn, Failure::DeadlineOverflow) => "无法表示 TTL 截止时间",
                (Self::EnUs, Failure::ResultTooLarge) => "Metadata response exceeds its limit",
                (Self::ZhCn, Failure::ResultTooLarge) => "元数据应答超出上限",
            },
            // This query consumes only its requested metadata/report payload; other receipt classes cannot answer that query.
            _ => match self {
                Self::EnUs => "Report outcome unknown; do not retry automatically",
                Self::ZhCn => "上报结果未知，请勿自动重试",
            },
        };
        let detail = unknown.map_or_else(String::new, |reason| {
            format!(" ({})", crate::cli::escape::escape(reason))
        });
        format!(
            "{state}{detail}\nWorkspace: {}\n{}: {}\n",
            crate::cli::escape::escape(workspace_id),
            match self {
                Self::EnUs => "Request",
                Self::ZhCn => "请求",
            },
            crate::cli::escape::escape(request_id)
        )
    }
}
