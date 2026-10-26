use super::Locale;
use orchbee_workbench_protocol::{
    PanelAgentDisposition, PanelAgentFailure as Failure, PanelAgentOutcome as Outcome,
};
impl Locale {
    #[cfg(windows)]
    pub(crate) const fn cli_agent_manifest_fields(self) -> [&'static str; 4] {
        match self {
            Self::EnUs => [
                "Rules file",
                "Cached version",
                "Local override shadows cache",
                "Rules warning",
            ],
            Self::ZhCn => ["规则文件", "缓存版本", "本地覆盖遮蔽缓存", "规则告警"],
        }
    }
    #[cfg(windows)]
    pub(crate) fn cli_agent_manifest_warning(
        self,
        warning: &orchbee_workbench_protocol::AgentManifestWarning,
    ) -> String {
        use orchbee_workbench_protocol::{
            AgentManifestSource as Source, AgentManifestWarningKind as Kind,
        };
        let source = match (self, warning.source) {
            (Self::EnUs, Source::Bundled) => "Bundled rules",
            (Self::ZhCn, Source::Bundled) => "内置规则",
            (Self::EnUs, Source::LocalOverride) => "Local override",
            (Self::ZhCn, Source::LocalOverride) => "本地覆盖",
            (Self::EnUs, Source::RemoteCache) => "Remote cache",
            (Self::ZhCn, Source::RemoteCache) => "远程缓存",
        };
        let reason = match (self, warning.kind) {
            (Self::EnUs, Kind::Unreadable) => "cannot read file",
            (Self::ZhCn, Kind::Unreadable) => "无法读取文件",
            (Self::EnUs, Kind::NotRegular) => "not a regular file",
            (Self::ZhCn, Kind::NotRegular) => "不是普通文件",
            (Self::EnUs, Kind::TooLarge) => "file exceeds the size limit",
            (Self::ZhCn, Kind::TooLarge) => "文件超过大小上限",
            (Self::EnUs, Kind::InvalidUtf8) => "file is not UTF-8",
            (Self::ZhCn, Kind::InvalidUtf8) => "文件不是 UTF-8 编码",
            (Self::EnUs, Kind::InvalidManifest) => {
                "manifest identity, rules or engine version is invalid"
            }
            (Self::ZhCn, Kind::InvalidManifest) => "清单身份、规则或引擎版本无效",
            (Self::EnUs, Kind::InvalidVersion) => "remote version is not bounded dotted numeric",
            (Self::ZhCn, Kind::InvalidVersion) => "远程版本不是有界点分数字",
            (Self::EnUs, Kind::OlderThanBundled) => "cached version is older than bundled rules",
            (Self::ZhCn, Kind::OlderThanBundled) => "缓存版本低于内置规则",
        };
        format!("{source}: {reason}")
    }
    #[cfg(windows)]
    pub(crate) const fn cli_agent_screen_fields(self) -> [&'static str; 17] {
        match self {
            Self::EnUs => [
                "Agent",
                "Effective state",
                "Rule state",
                "Rules source",
                "Rules version",
                "Matched rule",
                "Reason",
                "Rule",
                "Matched",
                "Priority",
                "Region",
                "Region bytes",
                "Region preview",
                "Contains",
                "Regex",
                "Line regex",
                "Gate counts (all/any/not)",
            ],
            Self::ZhCn => [
                "Agent",
                "当前有效状态",
                "规则判断状态",
                "规则来源",
                "规则版本",
                "胜出规则",
                "原因",
                "规则",
                "是否匹配",
                "优先级",
                "区域",
                "区域字节数",
                "区域预览",
                "包含文本",
                "正则表达式",
                "逐行正则",
                "组合条件数（all/any/not）",
            ],
        }
    }
    #[cfg(windows)]
    pub(crate) const fn cli_agent_screen_reason(self, unchanged: bool) -> &'static str {
        match (self, unchanged) {
            (Self::EnUs, true) => "Matched rule preserves the previous state",
            (Self::ZhCn, true) => "匹配规则要求保留先前状态",
            (Self::EnUs, false) => "No matching rule: known Agent defaults to idle",
            (Self::ZhCn, false) => "没有匹配规则，已知 Agent 默认空闲",
        }
    }
    pub(crate) const fn cli_reported_agent_fields(self) -> [&'static str; 4] {
        match self {
            Self::EnUs => [
                "Reported Agent",
                "Report state",
                "Authority source",
                "Report message",
            ],
            Self::ZhCn => ["上报 Agent", "上报状态", "权威来源", "上报消息"],
        }
    }
    pub(crate) fn cli_panel_agent(
        self,
        request_id: &str,
        panel_id: &str,
        outcome: Option<&Outcome>,
        unknown: Option<&str>,
    ) -> String {
        let state = match outcome {
            Some(Outcome::Reported {
                disposition: PanelAgentDisposition::Applied,
                ..
            })
            | Some(Outcome::PresentationReported {
                disposition: PanelAgentDisposition::Applied,
                ..
            }) => match self {
                Self::EnUs => "Report applied",
                Self::ZhCn => "上报已应用",
            },
            Some(Outcome::Reported {
                disposition: PanelAgentDisposition::Ignored,
                ..
            })
            | Some(Outcome::PresentationReported {
                disposition: PanelAgentDisposition::Ignored,
                ..
            }) => match self {
                Self::EnUs => "Report ignored",
                Self::ZhCn => "上报已忽略",
            },
            Some(Outcome::Failed { reason }) => match (self, reason) {
                (Self::EnUs, Failure::InvalidRequest) => "Invalid Agent report request",
                (Self::ZhCn, Failure::InvalidRequest) => "Agent 上报请求无效",
                (Self::EnUs, Failure::SiteMismatch) => "Session identity changed",
                (Self::ZhCn, Failure::SiteMismatch) => "会话身份已改变",
                (Self::EnUs, Failure::Unbound) => "Target member has no bound terminal",
                (Self::ZhCn, Failure::Unbound) => "目标成员未绑定终端",
                (Self::EnUs, Failure::BindingChanged) => "Target terminal binding changed",
                (Self::ZhCn, Failure::BindingChanged) => "目标终端绑定已改变",
                (Self::EnUs, Failure::NotFound) => "Panel or requested member is unavailable",
                (Self::ZhCn, Failure::NotFound) => "Panel 或请求成员已不可用",
                (Self::EnUs, Failure::Denied) => "Agent report reporting denied",
                (Self::ZhCn, Failure::Denied) => "Agent 上报上报被拒绝",
                (Self::EnUs, Failure::Unavailable) => "Agent report service unavailable",
                (Self::ZhCn, Failure::Unavailable) => "Agent 上报服务不可用",
                (Self::EnUs, Failure::HistoryLimit) => "Agent session history limit reached",
                (Self::ZhCn, Failure::HistoryLimit) => "Agent 会话历史已达上限",
                (Self::EnUs, Failure::SourceLimit) => "Sequence source limit reached",
                (Self::ZhCn, Failure::SourceLimit) => "序号来源数量已达上限",
                (Self::EnUs, Failure::ResourceLimit) => "Agent report capacity reached",
                (Self::ZhCn, Failure::ResourceLimit) => "Agent 上报容量已达上限",
                (Self::EnUs, Failure::RevisionExhausted) => "Agent report revision exhausted",
                (Self::ZhCn, Failure::RevisionExhausted) => "Agent 上报修订号已耗尽",
                (Self::EnUs, Failure::ClockOverflow) => "Observation clock cannot be represented",
                (Self::ZhCn, Failure::ClockOverflow) => "无法表示观察时钟",
                (Self::EnUs, Failure::ResultTooLarge) => "Agent report response exceeds its limit",
                (Self::ZhCn, Failure::ResultTooLarge) => "Agent 上报应答超出上限",
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

pub(super) const PANELREPORTAGENT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel report-agent <panel> [--presentation <id>] --source <id> --agent <label> --state idle|working|blocked|done|error|unknown [--message <text>] [--seq <n>] [--format text|json]",
        "Report explicit Agent state and an optional message. Every source is custom and grants no provider identity.\nUse an exact Panel ID or @panel:0x<hex>. --presentation selects an exact primary or child Presentation in that Panel; omit it for the primary. Sequence accepts unsigned 64-bit decimal input. Sources for report/release are trimmed and must be nonempty; clear preserves its optional source literally. Unknown Agent labels and messages are not truncated. The full request must fit 16 KiB.\nOne existing-session admission, five-second budget, binding fence, no focus change or automatic retry. Applied and ignored receipts exit 0; unknown after submission exits 8 and retains its request ID. Reports do not prove a running process or create a canonical Agent session. Panel get/list/current and Agent get expose reportedAgent and apply metadata guards using its actual context.",
    );
pub(super) const PANELREPORTAGENT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel report-agent <panel> [--presentation <id>] --source <id> --agent <label> --state idle|working|blocked|done|error|unknown [--message <text>] [--seq <n>] [--format text|json]",
        "上报显式 Agent 状态及可选消息。所有来源均按自定义来源处理，不授予 provider 身份。使用精确 Panel ID 或 @panel:0x<hex>。--presentation 指定该 Panel 内精确的主项或子项 Presentation；省略则使用主项。序号接受无符号 64 位十进制输入。report/release 的来源修剪空白后必须非空；clear 原样保留可选来源。未知 Agent 标签和消息不截断，完整请求须不超过 16 KiB。仅连接已有会话，总期限 5 秒，校验终端绑定，不改变焦点或自动重试。已应用和已忽略回执退出 0；提交后结果未知退出 8，并保留请求 ID。上报不证明进程存活，也不创建规范 Agent 会话。Panel get/list/current 与 Agent get 展示 reportedAgent，并使用实际上下文应用元数据守卫。",
    );

pub(super) const PANELCLEARAGENTAUTHORITY_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel clear-agent-authority <panel> [--presentation <id>] [--source <id>] [--seq <n>] [--format text|json]",
        "Clear only the current matching report authority. Omitted source matches the current authority; an explicitly empty source matches only the empty source.\nUse an exact Panel ID or @panel:0x<hex>. --presentation selects an exact primary or child Presentation in that Panel; omit it for the primary. Sequence accepts unsigned 64-bit decimal input. Sources for report/release are trimmed and must be nonempty; clear preserves its optional source literally. Unknown Agent labels and messages are not truncated. The full request must fit 16 KiB.\nOne existing-session admission, five-second budget, binding fence, no focus change or automatic retry. Applied and ignored receipts exit 0; unknown after submission exits 8 and retains its request ID. Reports do not prove a running process or create a canonical Agent session. Panel get/list/current and Agent get expose reportedAgent and apply metadata guards using its actual context.",
    );
pub(super) const PANELCLEARAGENTAUTHORITY_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel clear-agent-authority <panel> [--presentation <id>] [--source <id>] [--seq <n>] [--format text|json]",
        "仅清除当前匹配的上报权威。省略 source 时匹配当前权威；显式空 source 只匹配空来源。使用精确 Panel ID 或 @panel:0x<hex>。--presentation 指定该 Panel 内精确的主项或子项 Presentation；省略则使用主项。序号接受无符号 64 位十进制输入。report/release 的来源修剪空白后必须非空；clear 原样保留可选来源。未知 Agent 标签和消息不截断，完整请求须不超过 16 KiB。仅连接已有会话，总期限 5 秒，校验终端绑定，不改变焦点或自动重试。已应用和已忽略回执退出 0；提交后结果未知退出 8，并保留请求 ID。上报不证明进程存活，也不创建规范 Agent 会话。Panel get/list/current 与 Agent get 展示 reportedAgent，并使用实际上下文应用元数据守卫。",
    );

pub(super) const PANELRELEASEAGENT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel release-agent <panel> [--presentation <id>] --source <id> --agent <label> [--seq <n>] [--format text|json]",
        "Release a custom source Agent report without ending an independently observed process.\nUse an exact Panel ID or @panel:0x<hex>. --presentation selects an exact primary or child Presentation in that Panel; omit it for the primary. Sequence accepts unsigned 64-bit decimal input. Sources for report/release are trimmed and must be nonempty; clear preserves its optional source literally. Unknown Agent labels and messages are not truncated. The full request must fit 16 KiB.\nOne existing-session admission, five-second budget, binding fence, no focus change or automatic retry. Applied and ignored receipts exit 0; unknown after submission exits 8 and retains its request ID. Reports do not prove a running process or create a canonical Agent session. Panel get/list/current and Agent get expose reportedAgent and apply metadata guards using its actual context.",
    );
pub(super) const PANELRELEASEAGENT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel release-agent <panel> [--presentation <id>] --source <id> --agent <label> [--seq <n>] [--format text|json]",
        "释放自定义来源的 Agent 上报，不结束独立观察到的进程。使用精确 Panel ID 或 @panel:0x<hex>。--presentation 指定该 Panel 内精确的主项或子项 Presentation；省略则使用主项。序号接受无符号 64 位十进制输入。report/release 的来源修剪空白后必须非空；clear 原样保留可选来源。未知 Agent 标签和消息不截断，完整请求须不超过 16 KiB。仅连接已有会话，总期限 5 秒，校验终端绑定，不改变焦点或自动重试。已应用和已忽略回执退出 0；提交后结果未知退出 8，并保留请求 ID。上报不证明进程存活，也不创建规范 Agent 会话。Panel get/list/current 与 Agent get 展示 reportedAgent，并使用实际上下文应用元数据守卫。",
    );
