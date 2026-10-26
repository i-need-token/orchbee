//! Runtime metadata copy; observed build identifiers remain data.
use super::Locale;

pub(super) fn help(locale: Locale, server: bool) -> crate::localization::HelpDetails {
    match (locale, server) {
        (Locale::EnUs, false) => crate::localization::HelpDetails::new(
            "orchbee status client [--format text|json]",
            "Windows: inspect this executable's version, platform, architecture, path and\nvalidated native cohort without contacting a Server. Missing or invalid cohort\nmetadata is reported explicitly; unavailable cohort I/O details appear in JSON.\nInstalled release identity is bound to its cohort; configured channel preference is\nindependent. One five-second collection budget; creates and changes nothing.\nJSON schema: orchbee.status.client (v1).",
        ),
        (Locale::ZhCn, false) => crate::localization::HelpDetails::new(
            "orchbee status client [--format text|json]",
            "Windows：检查当前程序的版本、平台、架构、路径及已验证的原生 cohort，不联系 Server。明确报告缺失或无效的 cohort 元数据；JSON 会包含不可用 cohort 的 I/O 详情。已安装版本身份绑定对应 cohort，与配置的频道偏好相互独立。采集共用五秒时限，不创建或修改任何内容。\nJSON schema：orchbee.status.client（v1）。",
        ),
        (Locale::EnUs, true) => crate::localization::HelpDetails::new(
            "orchbee status server [--format text|json]",
            "Windows: query the selected local Server's version, PID, terminal protocol,\ncohort and implementation capabilities before ordinary Client admission.\nCompare with the querying Client; unknown local metadata stays unknown.\nDoes not start a session, allocate a Client Connection or restart anything.\nOne five-second collection budget. No Server at this scope exits 0; an old\nServer without metadata support is incompatible (exit 6), not absent.\nJSON schema: orchbee.status.server (v1).",
        ),
        (Locale::ZhCn, true) => crate::localization::HelpDetails::new(
            "orchbee status server [--format text|json]",
            "Windows：在普通 Client 准入前查询选中本地 Server 的版本、PID、终端协议、\ncohort 及已实现能力，并与查询 Client 比较；未知本地元数据保持未知。不启动会话、不分配 客户端连接，也不重启任何内容。采集共用五秒时限。该作用域没有 Server 时退出 0；旧 Server 不支持元数据查询时报告不兼容（退出 6），不误报为不存在。\nJSON schema：orchbee.status.server（v1）。",
        ),
    }
}

impl Locale {
    pub(crate) fn cli_runtime_status_failure(self, kind: &str) -> String {
        match self {
            Self::EnUs => format!("Runtime status query failed ({kind}).\n"),
            Self::ZhCn => format!("运行时状态查询失败（{kind}）。\n"),
        }
    }

    #[cfg(windows)]
    pub(crate) fn cli_runtime_status_unknown(self) -> &'static str {
        match self {
            Self::EnUs => "Unknown / unavailable",
            Self::ZhCn => "未知／不可用",
        }
    }

    #[cfg(windows)]
    pub(crate) fn cli_runtime_status_label(self, label: &str) -> &str {
        let (en, zh) = match label {
            "client" => ("Client version", "Client 版本"),
            "server" => ("Server", "Server 状态"),
            "version" => ("Server version", "Server 版本"),
            "process" => ("Process ID", "进程 ID"),
            "platform" => ("Platform", "平台"),
            "architecture" => ("Architecture", "架构"),
            "profile" => ("Build profile", "构建类型"),
            "delivery" => ("Delivery", "交付方式"),
            "channel" => ("Update channel", "更新渠道"),
            "installation" => ("Installed release", "已安装版本"),
            "executable" => ("Executable", "程序路径"),
            "protocols" => ("Protocols", "协议版本"),
            "local-cohort" => ("Local cohort", "本地 cohort"),
            "server-cohort" => ("Server cohort", "Server cohort"),
            "mode" => ("Mode", "模式"),
            "stopping" => ("Stopping", "正在停止"),
            "scope" => ("Local scope", "本地作用域"),
            "endpoint" => ("Endpoint", "端点"),
            "compatibility" => ("Compatibility", "兼容性"),
            "capabilities" => ("Implementation capabilities", "已实现能力"),
            "admission" => ("Admission", "准入"),
            "terminal" => ("Terminal", "终端"),
            "state" => ("State", "状态"),
            "reason" => ("Reason", "原因"),
            "identity" => ("Identity", "标识"),
            "cohort" => ("Cohort matches", "cohort 匹配"),
            "admissionProtocol" => ("Admission protocol matches", "准入协议匹配"),
            "terminalProtocol" => ("Terminal protocol matches", "终端协议匹配"),
            "compatible" => ("Compatible", "兼容"),
            "restartRequired" => ("Matching-cohort restart required", "需要按匹配 cohort 重启"),
            "terminalRenderRows" => ("Styled terminal reads", "终端样式读取"),
            "clientViewControl" => ("Client view control", "Client 视图控制"),
            "agentObservation" => ("Agent observation", "Agent 观测"),
            "liveHandoff" => ("Live handoff", "在线交接"),
            "managedStop" => ("Managed stop", "托管停止"),
            "unknownBits" => ("Unknown feature bits", "未知特性位"),
            _ => return label,
        };
        match self {
            Self::EnUs => en,
            Self::ZhCn => zh,
        }
    }

    #[cfg(windows)]
    pub(crate) fn cli_runtime_status_value(self, token: &str) -> &str {
        let (en, zh) = match token {
            "true" => ("Yes", "是"),
            "false" => ("No", "否"),
            "running" => ("Running", "正在运行"),
            "not-running" => ("Not running at this scope", "该作用域未运行"),
            "available" => ("Available", "可用"),
            "unavailable" => ("Unavailable", "不可用"),
            "missing" => ("Missing", "缺失"),
            "invalid" => ("Invalid or unsupported manifest", "无效或不支持的清单"),
            "denied" => ("Permission denied", "无权限"),
            "timed-out" => ("Timed out", "超时"),
            "io-error" => ("I/O error", "读写错误"),
            "managed" => ("Managed", "托管"),
            "unmanaged" => ("Unmanaged", "非托管"),
            "development-cohort" => ("Development cohort", "开发 cohort"),
            "native-release" => ("Installed native release", "已安装原生版本"),
            "unknown" => ("Unknown", "未知"),
            "debug" => ("Debug", "调试"),
            "release" => ("Release", "发布"),
            _ => return token,
        };
        match self {
            Self::EnUs => en,
            Self::ZhCn => zh,
        }
    }
}
