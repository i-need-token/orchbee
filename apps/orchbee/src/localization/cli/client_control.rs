use super::Locale;
use crate::cli::escape::escape;
use serde_json::Value;

impl Locale {
    pub(crate) fn cli_client_control(self, document: &Value) -> String {
        let control = document["schema"] == "orchbee.client.control";
        let outcome = if control {
            &document["observation"]["reply"]["outcome"]
        } else {
            &document["result"]["outcome"]
        };
        let labels = match self {
            Self::EnUs => [
                "Client",
                "Operation",
                "Context",
                "Tab",
                "Panel",
                "Zoom",
                "Reason",
                "Waiting",
                "No interactive Clients",
            ],
            Self::ZhCn => [
                "客户端",
                "操作",
                "上下文",
                "标签页",
                "面板",
                "缩放",
                "原因",
                "等待",
                "没有交互客户端",
            ],
        };
        let mut text = String::new();
        if let Some(id) = document["operationId"].as_str() {
            text.push_str(&format!("{}: {}\n", labels[1], escape(id)));
        }
        if let Some(clients) = outcome["clients"].as_array() {
            if clients.is_empty() {
                text.push_str(labels[8]);
                text.push('\n');
            }
            for client in clients {
                self.client_record_text(&mut text, client, &labels);
            }
        } else if !outcome["client"].is_null() {
            self.client_record_text(&mut text, &outcome["client"], &labels);
        } else {
            let status = &outcome["operation"]["status"];
            let state = status["kind"]
                .as_str()
                .or_else(|| outcome["kind"].as_str())
                .or_else(|| document["observation"]["kind"].as_str())
                .unwrap_or("unavailable");
            text.push_str(self.client_control_state(state));
            text.push('\n');
            if let Some(id) = outcome["operation"]["command"]["clientId"].as_str() {
                text.push_str(&format!("{}: {}\n", labels[0], escape(id)));
            }
            self.client_selection_text(&mut text, &status["view"]["selection"], &labels);
            if let Some(reason) = status["reason"]
                .as_str()
                .or_else(|| outcome["reason"].as_str())
            {
                text.push_str(&format!(
                    "{}: {}\n",
                    labels[6],
                    self.client_control_state(reason)
                ));
            }
        }
        if let Some(wait) = document["wait"].as_str() {
            text.push_str(&format!(
                "{}: {}\n",
                labels[7],
                self.client_control_state(wait)
            ));
        }
        if control
            && (document["wait"].is_string() || document["observation"]["kind"] == "unknown")
            && let Some(id) = document["operationId"].as_str()
        {
            text.push_str(&format!("orchbee client control-status {}\n", escape(id)));
        }
        text
    }
    fn client_record_text(self, text: &mut String, client: &Value, labels: &[&str; 9]) {
        text.push_str(&format!(
            "{}: {}  {}  {}\n",
            labels[0],
            escape(client["clientId"].as_str().unwrap_or("")),
            self.client_control_state(client["availability"].as_str().unwrap_or("stale")),
            self.client_control_state(client["view"]["phase"].as_str().unwrap_or("unavailable"))
        ));
        self.client_selection_text(text, &client["view"]["selection"], labels);
    }
    fn client_selection_text(self, text: &mut String, selection: &Value, labels: &[&str; 9]) {
        for (field, label) in [
            ("terminalContextId", labels[2]),
            ("tabId", labels[3]),
            ("panelId", labels[4]),
        ] {
            if let Some(value) = selection[field].as_str() {
                text.push_str(&format!("  {label}: {}\n", escape(value)));
            }
        }
        if let Some(zoomed) = selection["zoomed"].as_bool() {
            text.push_str(&format!(
                "  {}: {}\n",
                labels[5],
                self.client_control_state(if zoomed { "on" } else { "off" })
            ));
        }
    }
    fn client_control_state(self, value: &str) -> &'static str {
        match (self, value) {
            (Self::EnUs, "current") => "Current",
            (Self::ZhCn, "current") => "当前有效",
            (Self::EnUs, "stale" | "stale-view") => "Stale view",
            (Self::ZhCn, "stale" | "stale-view") => "视图已过期",
            (Self::EnUs, "ready") => "Ready",
            (Self::ZhCn, "ready") => "就绪",
            (Self::EnUs, "starting") => "Starting",
            (Self::ZhCn, "starting") => "正在启动",
            (Self::EnUs, "busy" | "server-busy") => "Busy",
            (Self::ZhCn, "busy" | "server-busy") => "忙碌",
            (Self::EnUs, "queued") => "Queued",
            (Self::ZhCn, "queued") => "已排队",
            (Self::EnUs, "delivered") => "Delivered; awaiting display",
            (Self::ZhCn, "delivered") => "已投递，等待界面确认",
            (Self::EnUs, "applied") => "Applied and drawn",
            (Self::ZhCn, "applied") => "已应用并绘制",
            (Self::EnUs, "zoom-toggled") => "Zoom toggled and drawn",
            (Self::ZhCn, "zoom-toggled") => "缩放已切换并绘制",
            (Self::EnUs, "zoom-unchanged") => "Single panel; zoom unchanged",
            (Self::ZhCn, "zoom-unchanged") => "仅一个面板，缩放未改变",
            (Self::EnUs, "rejected") => "Rejected",
            (Self::ZhCn, "rejected") => "已拒绝",
            (Self::EnUs, "incomplete") => "Incomplete; partial changes may remain",
            (Self::ZhCn, "incomplete") => "未完成，可能已部分改变界面",
            (Self::EnUs, "unknown") => "Outcome unknown; query the operation",
            (Self::ZhCn, "unknown") => "结果未知，请查询操作状态",
            (Self::EnUs, "unobserved") => "Operation not observed; read status again",
            (Self::ZhCn, "unobserved") => "尚未读到操作状态，请再次查询",
            (Self::EnUs, "not-submitted") => "Not submitted",
            (Self::ZhCn, "not-submitted") => "尚未提交",
            (Self::EnUs, "not-found") => "Not found",
            (Self::ZhCn, "not-found") => "未找到",
            (Self::EnUs, "conflict") => "Operation identity conflicts",
            (Self::ZhCn, "conflict") => "操作标识冲突",
            (Self::EnUs, "denied") => "Denied",
            (Self::ZhCn, "denied") => "无权限",
            (Self::EnUs, "expired" | "timed-out") => "Timed out",
            (Self::ZhCn, "expired" | "timed-out") => "已超时",
            (Self::EnUs, "interrupted") => "Interrupted by local interaction",
            (Self::ZhCn, "interrupted") => "被本地交互中断",
            (Self::EnUs, "client-disconnected") => "Client disconnected",
            (Self::ZhCn, "client-disconnected") => "客户端已断开",
            (Self::EnUs, "target-unavailable") => "Target unavailable",
            (Self::ZhCn, "target-unavailable") => "目标不可用",
            (Self::EnUs, "projection-unavailable") => "Projection unavailable",
            (Self::ZhCn, "projection-unavailable") => "界面数据不可用",
            (Self::EnUs, "receipt-capacity") => "Receipt capacity reached",
            (Self::ZhCn, "receipt-capacity") => "回执容量已满",
            (Self::EnUs, "sequence-exhausted") => "Sequence exhausted",
            (Self::ZhCn, "sequence-exhausted") => "序列已耗尽",
            (Self::EnUs, "unsupported") => "Unsupported",
            (Self::ZhCn, "unsupported") => "不支持",
            (Self::EnUs, "detached") => "Waiting detached; operation continues",
            (Self::ZhCn, "detached") => "已停止等待，操作继续",
            (Self::EnUs, "cancelled") => "Waiting cancelled",
            (Self::ZhCn, "cancelled") => "等待已取消",
            (Self::EnUs, "transport-lost" | "observation-failed") => "Observation interrupted",
            (Self::ZhCn, "transport-lost" | "observation-failed") => "状态观测中断",
            (Self::EnUs, "on") => "On",
            (Self::ZhCn, "on") => "开启",
            (Self::EnUs, "off") => "Off",
            (Self::ZhCn, "off") => "关闭",
            (Self::EnUs, _) => "Unavailable",
            (Self::ZhCn, _) => "不可用",
        }
    }
}

pub(super) const HELP_CLIENT_EN: &str = r"Usage: orchbee client list | get <client-id> | control-status <operation-id> [--format text|json]

Discover Windows TUI Clients and read retained control operations. Client IDs are exact nonzero 32-character lowercase hex. No operating-system window is activated.
";

pub(super) const HELP_CLIENT_ZH: &str = r"用法: orchbee client list | get <client-id> | control-status <operation-id> [--format text|json]

发现 Windows TUI 客户端并查询保留的控制操作。Client 标识为精确的非零 32 位小写十六进制；不激活操作系统窗口。
";

pub(super) const HELP_CLIENT_LIST_EN: &str = r"Usage: orchbee client list [--format text|json]

List interactive TUI views, including current or stale availability and selected Context/Tab/Panel. Ordinary CLI connections are excluded.
";

pub(super) const HELP_CLIENT_LIST_ZH: &str = r"用法: orchbee client list [--format text|json]

列出交互 TUI 视图，包含当前有效或过期状态及选中的上下文／标签页／面板；不列出普通 CLI 连接。
";

pub(super) const HELP_CLIENT_GET_EN: &str = r"Usage: orchbee client get <client-id> [--format text|json]

Read one live Client by its exact ID. A stale view is an observation, not confirmation that its UI responds.
";

pub(super) const HELP_CLIENT_GET_ZH: &str = r"用法: orchbee client get <client-id> [--format text|json]

按精确标识读取运行中的客户端。过期视图只提供观测，不表示 UI 仍可响应。
";

pub(super) const HELP_CLIENT_CONTROL_STATUS_EN: &str = r"Usage: orchbee client control-status <operation-id> [--format text|json]

Read retained control status, including after the Client exits. Completed receipts live for 60 seconds; not-found after expiry does not prove no effect.
";

pub(super) const HELP_CLIENT_CONTROL_STATUS_ZH: &str = r"用法: orchbee client control-status <operation-id> [--format text|json]

查询保留的控制状态，包括客户端退出后的状态。完成回执保留 60 秒；过期后未找到不能证明此前没有效果。
";

pub(super) const HELP_WORKSPACE_FOCUS_EN: &str = r"Usage: orchbee workspace focus <workspace> [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]

Select a Workspace in one Windows TUI.

Without --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Success requires observed selection and a successful draw. --detach or interruption ends waiting only; query client control-status with the operation ID.
";

pub(super) const HELP_WORKSPACE_FOCUS_ZH: &str = r"用法: orchbee workspace focus <workspace> [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]

在一个 Windows TUI 中选中工作区。

省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。成功要求观测到目标选择且成功绘制。--detach 或中断仅结束等待；使用操作标识查询 client control-status。
";

pub(super) const HELP_TAB_FOCUS_EN: &str = r"Usage: orchbee tab focus <tab> [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]

Select a Tab in one Windows TUI, including its owning Workspace.

Without --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Success requires observed selection and a successful draw. --detach or interruption ends waiting only; query client control-status with the operation ID.
";

pub(super) const HELP_TAB_FOCUS_ZH: &str = r"用法: orchbee tab focus <tab> [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]

在一个 Windows TUI 中选中标签页及其所属工作区。

省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。成功要求观测到目标选择且成功绘制。--detach 或中断仅结束等待；使用操作标识查询 client control-status。
";

pub(super) const HELP_PANEL_FOCUS_EN: &str = r"Usage: orchbee panel focus [panel] [--panel <panel> | --current] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]

Select a Panel in one Windows TUI, including its owning Tab and Workspace.

Without --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Success requires observed selection and a successful draw. --detach or interruption ends waiting only; query client control-status with the operation ID.
";

pub(super) const HELP_PANEL_FOCUS_ZH: &str = r"用法: orchbee panel focus [panel] [--panel <panel> | --current] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]

在一个 Windows TUI 中选中面板及其所属标签页和工作区。

省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。成功要求观测到目标选择且成功绘制。--detach 或中断仅结束等待；使用操作标识查询 client control-status。
";

pub(super) const HELP_PANEL_ZOOM_EN: &str = r"Usage: orchbee panel zoom [panel] [--panel <panel> | --current] [--toggle | --on | --off] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]

Set the selected Panel zoom. Toggle is default and resolves the prior zoom once after activating the target Tab. Single-panel zoom-on/toggle succeeds with zoom-unchanged.

Without --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Success requires observed selection and a successful draw. --detach or interruption ends waiting only; query client control-status with the operation ID.
";

pub(super) const HELP_PANEL_ZOOM_ZH: &str = r"用法: orchbee panel zoom [panel] [--panel <panel> | --current] [--toggle | --on | --off] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]

设置选中面板的缩放。默认为切换，在激活目标标签页后只解析一次原缩放状态；单面板开启或切换缩放成功返回 zoom-unchanged。

省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。成功要求观测到目标选择且成功绘制。--detach 或中断仅结束等待；使用操作标识查询 client control-status。
";
