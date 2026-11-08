//! Runtime status copy; observed versions and build identifiers remain data.
use super::Locale;

pub(super) fn help(locale: Locale, server: bool) -> crate::cli_text::HelpDetails {
    match (locale, server) {
        (Locale::EnUs, false) => crate::cli_text::HelpDetails::new(
            "orchbee status client [--format text|json]",
            "Windows: inspect this executable's version, platform, architecture, path and\nvalidated installation files without contacting a Server. Missing or invalid\ninstallation files are reported explicitly; their I/O details appear in JSON.\nInstalled release identity is bound to its installation files; configured channel\npreference is independent. One five-second collection budget; creates and changes nothing.\nJSON schema: orchbee.status.client (v1).",
        ),
        (Locale::ZhCn, false) => crate::cli_text::HelpDetails::new(
            "orchbee status client [--format text|json]",
            "Windows：检查当前程序的版本、平台、架构、路径及已验证的安装文件，不联系 Server。明确报告缺失或无效的安装文件；JSON 会包含其 I/O 详情。已安装版本身份绑定对应安装文件，与配置的频道偏好相互独立。采集共用五秒时限，不创建或修改任何内容。\nJSON schema：orchbee.status.client（v1）。",
        ),
        (Locale::EnUs, true) => crate::cli_text::HelpDetails::new(
            "orchbee status server [--format text|json]",
            "Windows: list the selected local Session's processes (Server, its Worker, then\nthe Terminal Hosts connected to it) with each version, PID, state and terminal\ncount, and whether Restart Session keeps terminals. A Host that does not answer\nin time is left out. Uses one read-only request, never a Client Connection: does\nnot start a session or restart anything. One five-second collection budget. No\nServer at this scope exits 0; a Session too old to answer or from another\nenvironment is incompatible (exit 6), not absent.\nJSON schema: orchbee.status.server (v1).",
        ),
        (Locale::ZhCn, true) => crate::cli_text::HelpDetails::new(
            "orchbee status server [--format text|json]",
            "Windows：列出选中本地会话的进程（Server、其 Worker，以及连接到该 Worker 的 Terminal Host），包括各自的版本、PID、状态和终端数，以及重启会话是否保留终端。未及时应答的 Terminal Host 不会列出。只发送一次只读请求，不建立客户端连接，不启动会话，也不重启任何内容。采集共用五秒时限。该作用域没有 Server 时退出 0；会话版本过旧无法应答或属于另一环境时报告不兼容（退出 6），不误报为不存在。\nJSON schema：orchbee.status.server（v1）。",
        ),
    }
}

impl super::CliText {
    pub(crate) fn cli_runtime_status_failure(self, kind: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("Runtime status query failed ({kind}).\n"),
            Locale::ZhCn => format!("运行时状态查询失败（{kind}）。\n"),
        }
    }

    pub(crate) fn cli_runtime_status_unknown(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Unknown / unavailable",
            Locale::ZhCn => "未知／不可用",
        }
    }

    pub(crate) fn cli_runtime_status_label(self, label: &str) -> &str {
        let (en, zh) = match label {
            "client" => ("Client version", "客户端版本"),
            "server" => ("Server", "Server 状态"),
            "version" => ("Version", "版本"),
            "processId" => ("Process ID", "进程 ID"),
            "terminals" => ("Terminals", "终端数"),
            "role-server" => ("Server process", "Server 进程"),
            "role-worker" => ("Worker process", "Worker 进程"),
            "role-terminal-host" => ("Terminal Host process", "Terminal Host 进程"),
            "role-unknown" => ("Unknown process", "未知进程"),
            "restartKeepsTerminals" => ("Restart keeps terminals", "重启保留终端"),
            "platform" => ("Platform", "平台"),
            "architecture" => ("Architecture", "架构"),
            "profile" => ("Build profile", "构建类型"),
            "delivery" => ("Delivery", "交付方式"),
            "channel" => ("Update channel", "更新渠道"),
            "installation" => ("Installed release", "已安装版本"),
            "executable" => ("Executable", "程序路径"),
            "protocols" => ("Protocols", "协议版本"),
            "local-cohort" => ("Installation files", "安装文件"),
            "scope" => ("Local scope", "本地作用域"),
            "endpoint" => ("Endpoint", "端点"),
            "admission" => ("Admission", "准入"),
            "terminal" => ("Terminal", "终端"),
            "state" => ("State", "状态"),
            "reason" => ("Reason", "原因"),
            "identity" => ("Identity", "标识"),
            _ => return label,
        };
        match self.locale {
            Locale::EnUs => en,
            Locale::ZhCn => zh,
        }
    }

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
            "ready" => ("Ready", "就绪"),
            "draining" => ("Draining", "正在排空"),
            "stopping" => ("Stopping", "正在停止"),
            "development-cohort" => ("Development build", "开发构建"),
            "native-release" => ("Installed native release", "已安装原生版本"),
            "unknown" => ("Unknown", "未知"),
            "debug" => ("Debug", "调试"),
            "release" => ("Release", "发布"),
            _ => return token,
        };
        match self.locale {
            Locale::EnUs => en,
            Locale::ZhCn => zh,
        }
    }
}
