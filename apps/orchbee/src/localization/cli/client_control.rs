use super::Locale;
use crate::cli::escape::escape;
use serde_json::Value;

pub(super) const HELP_AGENT_FOCUS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent focus [<target> | --agent-session <id> | --name <name> | --panel <id|@panel:suffix>] [--client <id>] [--request-id <id>] [--detach] [--timeout <ms>] [--format text|json]",
        "Focus the live Agent's unique primary panel in an existing Windows terminal client. Choose exactly one target: a positional name/session/panel, --agent-session, --name or --panel. '@agent:<kind>' selects a unique provider; '@agent:<n>' selects a one-based ordinal in the complete Agent list, not a page. Bare names preserve Server alias precedence before provider matching. Get, read, prompt, send-keys, wait and focus share this resolver; partial projections cannot establish uniqueness. Child-only and ambiguous bindings are refused.\n\n--client selects explicitly; otherwise use the caller or the single current client. --request-id retains an identity for client control-status. Default timeout: 30000 ms; range: 1..3600000. --detach returns after submission without waiting for rendering.\n\nTimeout or Ctrl+C stops waiting without undoing accepted focus. This command does not start an Agent or activate an OS window. Successful visible paint acknowledges this client's observed Agent state unless the window is known to be unfocused.",
    );
pub(super) const HELP_AGENT_FOCUS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent focus [<target> | --agent-session <id> | --name <name> | --panel <id|@panel:suffix>] [--client <id>] [--request-id <id>] [--detach] [--timeout <ms>] [--format text|json]",
        "在已有 Windows 终端客户端中聚焦存活 Agent 唯一对应的主面板。必须选择一个目标：位置参数名称／会话／面板，或 --agent-session、--name、--panel。'@agent:<kind>' 选择唯一提供方；'@agent:<n>' 选择完整 Agent 列表中的一基序号（不是页内序号）。裸名称仍优先匹配 Server 别名，然后回退到提供方匹配。get、read、prompt、send-keys、wait 和 focus 共用此解析器；部分投影不能证明匹配唯一。拒绝仅有子终端或存在歧义的绑定。\n\n--client 显式选择客户端，否则使用调用者或唯一当前客户端。--request-id 保留可用于 client control-status 的标识。默认时限 30000 毫秒，范围 1..3600000；--detach 提交后立即返回，不等待绘制。\n\n超时或 Ctrl+C 只停止等待，不撤销已接受的聚焦。不启动 Agent，也不激活操作系统窗口。成功绘制会确认该客户端观察到的 Agent 状态，已知窗口未聚焦时除外。",
    );

pub(super) const HELP_AGENT_ATTENTION_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent attention [<target> | --agent-session <id> | --name <name> | --panel <id|@panel:suffix>] [--client <id>] [--request-id <id>] [--detach] [--timeout <ms>] [--format text|json]",
        "Read one Windows terminal client's seen/unseen observation for an Agent. Choose exactly one target: a positional session/name/panel, --agent-session, --name or --panel.\n\n--client selects explicitly; otherwise use the caller or the single current client. --request-id retains a read receipt for client control-status. --detach returns after submission. Default timeout: 30000 ms; range: 1..3600000.\n\nThis read does not focus a window or mark an Agent seen. Missing and unavailable observations remain distinct; each client keeps its own acknowledgement.",
    );
pub(super) const HELP_AGENT_ATTENTION_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee agent attention [<target> | --agent-session <id> | --name <name> | --panel <id|@panel:suffix>] [--client <id>] [--request-id <id>] [--detach] [--timeout <ms>] [--format text|json]",
        "读取一个 Windows 终端客户端对 Agent 的已读／未读观察。必须选择一个目标：位置参数会话／名称／面板，或 --session、--name、--panel。\n\n--client 显式选择客户端，否则使用调用者或唯一当前客户端。--request-id 保留可用于 client control-status 的读取回执；--detach 提交后返回。默认时限 30000 毫秒，范围 1..3600000。\n\n此读取不聚焦窗口，也不将 Agent 标为已读。缺失与不可用的观察仍有区别；每个客户端单独保存确认状态。",
    );

impl Locale {
    pub(super) fn cli_client_selection_failure(self, reason: &str) -> Option<&'static str> {
        Some(match (self, reason) {
            (Self::EnUs, "client-not-found") => {
                "No current TUI Client matches the selection. Start `orchbee tui`, inspect `orchbee client list`, or pass --client <id>.\n"
            }
            (Self::ZhCn, "client-not-found") => {
                "没有与选择匹配的当前 TUI 客户端。请启动 `orchbee tui`，用 `orchbee client list` 查看，或传入 --client <id>。\n"
            }
            (Self::EnUs, "client-ambiguous") => {
                "More than one current TUI Client matches the selection. Use `orchbee client list` and pass --client <id>.\n"
            }
            (Self::ZhCn, "client-ambiguous") => {
                "多个当前 TUI 客户端与选择匹配。请用 `orchbee client list` 查看并传入 --client <id>。\n"
            }
            (Self::EnUs, "client-panel-unavailable") => {
                "The selected TUI Client has no available current panel. Select a panel in that Client or pass an explicit panel target.\n"
            }
            (Self::ZhCn, "client-panel-unavailable") => {
                "所选 TUI 客户端没有可用的当前面板。请在该客户端选择面板，或传入显式面板目标。\n"
            }
            (Self::EnUs, "client-stale") => {
                "The selected TUI Client is stale. Inspect `orchbee client list` and choose a current Client.\n"
            }
            (Self::ZhCn, "client-stale") => {
                "所选 TUI 客户端已过期。请用 `orchbee client list` 查看并选择当前客户端。\n"
            }
            (Self::EnUs, "client-not-ready") => {
                "The selected TUI Client is not ready; retry after it becomes ready.\n"
            }
            (Self::ZhCn, "client-not-ready") => "所选 TUI 客户端尚未就绪；请在其就绪后重试。\n",
            _ => return None,
        })
    }

    pub(crate) fn cli_creation(self, document: &Value) -> String {
        let (
            title,
            complete,
            incomplete,
            action,
            request,
            context,
            tab,
            panel,
            revision,
            launch,
            client,
            ratio,
            focus,
        ) = match self {
            Self::EnUs => (
                "Terminal creation",
                "created",
                "created; requested Client work incomplete, inspect before retrying",
                "Action",
                "Request",
                "Context",
                "Tab",
                "Panel",
                "Revision",
                "Launch",
                "Client",
                "Ratio observation",
                "Focus observation",
            ),
            Self::ZhCn => (
                "终端创建",
                "已创建",
                "已创建；请求的客户端操作尚未完成，重试前请检查",
                "操作",
                "请求",
                "上下文",
                "标签页",
                "窗格",
                "修订",
                "启动",
                "客户端",
                "比例观测",
                "聚焦观测",
            ),
        };
        let result = &document["result"];
        let post = &document["postCreate"];
        let mut text = format!(
            "{title}: {}\n",
            if post["outcome"] == "incomplete" {
                incomplete
            } else {
                complete
            }
        );
        for (name, value) in [
            (action, &result["action"]),
            (request, &document["source"]["requestId"]),
            (context, &result["terminalContextId"]),
            (tab, &result["tabId"]),
            (
                panel,
                if result["newPanelId"].is_string() {
                    &result["newPanelId"]
                } else {
                    &result["panelId"]
                },
            ),
            (revision, &result["revision"]),
            (launch, &result["terminal"]["outcome"]),
            (client, &post["clientId"]),
        ] {
            if let Some(value) = value.as_str() {
                text.push_str(&format!("{name}: {}\n", escape(value)));
            }
        }
        let (directory_policy, directory_source, directory_path) = match self {
            Self::EnUs => ("Directory policy", "Directory source", "Directory"),
            Self::ZhCn => ("目录策略", "目录来源", "目录"),
        };
        if result["action"] == "tab.create"
            && let Some(hint) = result["label"]
                .as_str()
                .and_then(|label| self.cli_label_target_hint(label))
        {
            text.push_str(hint);
        }
        for (name, key) in [
            (directory_policy, "policy"),
            (directory_source, "source"),
            (directory_path, "path"),
        ] {
            if let Some(value) = document["directory"][key].as_str() {
                text.push_str(&format!("{name}: {}\n", escape(value)));
            }
        }
        for (name, value) in [(ratio, &post["ratio"]), (focus, &post["focus"])] {
            if value.is_object() {
                text.push_str(&format!("{name}:\n{}", self.cli_client_control(value)));
            }
        }
        text
    }

    pub(crate) fn cli_panel_swap(self, document: &Value) -> String {
        let (title, client, source, target, canonical, request, revision, reason) = match self {
            Self::EnUs => (
                "Panel swap",
                "Client",
                "Source",
                "Target",
                "Positions",
                "Request",
                "Revision",
                "Reason",
            ),
            Self::ZhCn => (
                "窗格交换",
                "客户端",
                "来源",
                "目标",
                "位置结果",
                "请求",
                "修订",
                "原因",
            ),
        };
        let outcome = match (self, document["outcome"].as_str().unwrap_or("not-swapped")) {
            (Self::EnUs, "swapped") => "exchanged and focused",
            (Self::ZhCn, "swapped") => "已交换并聚焦",
            (Self::EnUs, "unchanged") => "unchanged",
            (Self::ZhCn, "unchanged") => "未改变",
            (Self::EnUs, "incomplete") => {
                "incomplete; inspect the committed result before retrying"
            }
            (Self::ZhCn, "incomplete") => "未完整完成；重试前检查已提交结果",
            (Self::EnUs, "unknown") => "unknown; inspect current positions before retrying",
            (Self::ZhCn, "unknown") => "结果未知；重试前检查当前位置",
            (Self::EnUs, _) => "not exchanged",
            (Self::ZhCn, _) => "未交换",
        };
        let state = match (
            self,
            document["canonical"]["kind"]
                .as_str()
                .unwrap_or("not-submitted"),
        ) {
            (Self::EnUs, "committed") => "committed",
            (Self::ZhCn, "committed") => "已提交",
            (Self::EnUs, "unchanged") => "unchanged",
            (Self::ZhCn, "unchanged") => "未改变",
            (Self::EnUs, "refused") => "refused",
            (Self::ZhCn, "refused") => "已拒绝",
            (Self::EnUs, "unknown") => "unknown",
            (Self::ZhCn, "unknown") => "未知",
            (Self::EnUs, _) => "not submitted",
            (Self::ZhCn, _) => "未提交",
        };
        let mut text = format!(
            "{title}: {outcome}\n{client}: {}\n{source}: {}\n{target}: {}\n{canonical}: {state}\n",
            escape(document["clientId"].as_str().unwrap_or("-")),
            escape(document["source"]["panelId"].as_str().unwrap_or("-")),
            escape(document["target"]["panelId"].as_str().unwrap_or("-"))
        );
        for (label, value) in [
            (request, &document["requestId"]),
            (revision, &document["canonical"]["revision"]),
        ] {
            if let Some(value) = value.as_str() {
                text.push_str(&format!("{label}: {}\n", escape(value)));
            }
        }
        if let Some(value) = document["reason"].as_str() {
            let message = match (self, value) {
                (Self::ZhCn, "cross-tab") => "两个窗格不在同一标签页",
                (Self::ZhCn, "same-panel") => "来源和目标是同一窗格",
                (Self::ZhCn, "no-neighbor") => "该方向没有邻居",
                (Self::ZhCn, "layout-unconfirmed") => "尚未确认布局读取",
                (Self::ZhCn, "layout-revision-changed") => "布局修订已改变",
                (Self::ZhCn, "source-unavailable") => "来源窗格不可用",
                (Self::ZhCn, "client-unavailable") => "客户端不可用",
                (Self::ZhCn, "client-selection-changed") => "客户端选择已改变",
                (Self::EnUs, "cross-tab") => "the Panels belong to different Tabs",
                (Self::EnUs, "same-panel") => "source and target are the same Panel",
                (Self::EnUs, "no-neighbor") => "no neighbor in that direction",
                (Self::EnUs, "layout-unconfirmed") => "layout read is unconfirmed",
                (Self::EnUs, "layout-revision-changed") => "layout revision changed",
                (Self::EnUs, "source-unavailable") => "source Panel is unavailable",
                (Self::EnUs, "client-unavailable") => "Client is unavailable",
                (Self::EnUs, "client-selection-changed") => "Client selection changed",
                _ => value,
            };
            text.push_str(&format!("{reason}: {}\n", escape(message)));
        }
        if !document["focus"].is_null() {
            let focus = &document["focus"];
            let label = match self {
                Self::EnUs => "Focus",
                Self::ZhCn => "聚焦",
            };
            let state = focus["observation"]["result"]["kind"]
                .as_str()
                .or_else(|| focus["observation"]["kind"].as_str())
                .unwrap_or("unavailable");
            text.push_str(&format!("{label}: {}\n", self.client_control_state(state)));
            if let Some(id) = focus["operationId"].as_str() {
                text.push_str(&format!("orchbee client control-status {}\n", escape(id)));
            }
            if let Some(wait) = focus["wait"].as_str() {
                text.push_str(self.client_control_state(wait));
                text.push('\n');
            }
        }
        text
    }

    pub(crate) fn cli_client_control(self, document: &Value) -> String {
        let control = document["schema"] == "orchbee.client.control"
            || document["schema"] == "orchbee.popup.result";
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
            if state == "attention-read" {
                for entry in status["attention"]["entries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    let label = match (self, entry["state"]["seen"].as_bool()) {
                        (Self::EnUs, Some(true)) => "seen",
                        (Self::EnUs, Some(false)) => "unseen",
                        (Self::EnUs, None) => "not observed",
                        (Self::ZhCn, Some(true)) => "已读",
                        (Self::ZhCn, Some(false)) => "未读",
                        (Self::ZhCn, None) => "尚未观察到",
                    };
                    text.push_str(&format!(
                        "{}: {label}\n",
                        escape(entry["sessionId"].as_str().unwrap_or(""))
                    ));
                }
            }
            if state == "layout-adjusted" {
                self.client_adjustment_text(&mut text, &status["adjustment"]);
            }
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
    fn client_adjustment_text(self, text: &mut String, adjustment: &Value) {
        let split = &adjustment["change"]["split"];
        let no_split = adjustment["change"]["kind"] == "no-split";
        let changed = !no_split && split["previousRatio"] != split["ratio"];
        let outcome = match (self, no_split, changed, adjustment["selected"] == true) {
            (Self::EnUs, true, _, _) => "No eligible split; unchanged",
            (Self::ZhCn, true, _, _) => "没有可调整的分割，保持不变",
            (Self::EnUs, _, false, _) => "Ratio unchanged",
            (Self::ZhCn, _, false, _) => "比例未改变",
            (Self::EnUs, _, true, true) => "Ratio changed and drawn",
            (Self::ZhCn, _, true, true) => "比例已改变并绘制",
            (Self::EnUs, _, true, false) => "Ratio stored for inactive Tab",
            (Self::ZhCn, _, true, false) => "已保存非活动标签页的比例",
        };
        let (target, ratio) = match self {
            Self::EnUs => ("Target Tab", "Ratio"),
            Self::ZhCn => ("目标标签页", "比例"),
        };
        text.push_str(&format!(
            "{outcome}\n{target}: {}\n",
            escape(adjustment["tabId"].as_str().unwrap_or(""))
        ));
        if !no_split {
            text.push_str(&format!(
                "{ratio}: {} -> {}\n",
                split["previousRatio"], split["ratio"]
            ));
        }
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
            (Self::EnUs, "popup-opened") => "Popup opened and drawn",
            (Self::ZhCn, "popup-opened") => "popup 已打开并绘制",
            (Self::EnUs, "popup-closed") => "Popup close completed",
            (Self::ZhCn, "popup-closed") => "popup 关闭操作已完成",
            (Self::EnUs, "attention-read") => "Client Agent observation",
            (Self::ZhCn, "attention-read") => "客户端 Agent 观察",
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
            (Self::EnUs, "direction-focused") => "Neighbor focused and drawn",
            (Self::ZhCn, "direction-focused") => "已聚焦相邻面板并绘制",
            (Self::EnUs, "direction-unchanged") => "No neighbor; selection unchanged",
            (Self::ZhCn, "direction-unchanged") => "该方向没有相邻面板，选择未改变",
            (Self::EnUs, "layout-adjusted") => "Layout adjustment completed",
            (Self::ZhCn, "layout-adjusted") => "布局调整已完成",
            (Self::EnUs, "layout-read") => "Layout observed",
            (Self::ZhCn, "layout-read") => "已观测布局",
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

pub(super) const HELP_CLIENT_LIST_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee client list [--format text|json]",
        "List interactive TUI views, including current or stale availability and selected Context/Tab/Panel. Ordinary CLI connections are excluded.",
    );

pub(super) const HELP_CLIENT_LIST_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee client list [--format text|json]",
        "列出交互 TUI 视图，包含当前有效或过期状态及选中的上下文／标签页／面板；不列出普通 CLI 连接。",
    );

pub(super) const HELP_CLIENT_GET_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee client get <client-id> [--format text|json]",
        "Read one live Client by its exact ID. A stale view is an observation, not confirmation that its UI responds.",
    );

pub(super) const HELP_CLIENT_GET_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee client get <client-id> [--format text|json]",
        "按精确标识读取运行中的客户端。过期视图只提供观测，不表示 UI 仍可响应。",
    );

pub(super) const HELP_CLIENT_CONTROL_STATUS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee client control-status <operation-id> [--format text|json]",
        "Read retained control status, including after the Client exits. Completed receipts live for 60 seconds; not-found after expiry does not prove no effect.",
    );

pub(super) const HELP_CLIENT_CONTROL_STATUS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee client control-status <operation-id> [--format text|json]",
        "查询保留的控制状态，包括客户端退出后的状态。完成回执保留 60 秒；过期后未找到不能证明此前没有效果。",
    );

pub(super) const HELP_WORKSPACE_FOCUS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace focus <workspace> [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "Select a Workspace in one Windows TUI.\n\nWithout --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Success requires observed selection and a successful draw. --detach or interruption ends waiting only; query client control-status with the operation ID.",
    );

pub(super) const HELP_WORKSPACE_FOCUS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee workspace focus <workspace> [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "在一个 Windows TUI 中选中工作区。\n\n省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。成功要求观测到目标选择且成功绘制。--detach 或中断仅结束等待；使用操作标识查询 client control-status。",
    );

pub(super) const HELP_TAB_FOCUS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee tab focus <tab> [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "Select a Tab in one Windows TUI, including its owning Workspace.\n\nWithout --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Success requires observed selection and a successful draw. --detach or interruption ends waiting only; query client control-status with the operation ID.",
    );

pub(super) const HELP_TAB_FOCUS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee tab focus <tab> [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "在一个 Windows TUI 中选中标签页及其所属工作区。\n\n省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。成功要求观测到目标选择且成功绘制。--detach 或中断仅结束等待；使用操作标识查询 client control-status。",
    );

pub(super) const HELP_PANEL_FOCUS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel focus [panel] [--panel <panel> | --current] [--direction left|right|up|down] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "Select a Panel in one Windows TUI, including its owning Tab and Workspace. With --direction, resolve one neighbor from this source before navigation, preserving zoom. No neighbor returns direction-unchanged without selecting the source; unavailable geometry fails. A found neighbor returns direction-focused after drawing.\n\nWithout --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Focus success requires observed selection and a successful draw; no-neighbor succeeds without changing selection or drawing. --detach or interruption ends waiting only; query client control-status with the operation ID.",
    );

pub(super) const HELP_PANEL_FOCUS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel focus [panel] [--panel <panel> | --current] [--direction left|right|up|down] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "在一个 Windows TUI 中选中面板及其所属标签页和工作区。--direction 在导航前从来源解析一个邻居，保留缩放。没有邻居时返回 direction-unchanged，不选择来源；几何不可用时失败。找到邻居则在绘制后返回 direction-focused。\n\n省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。聚焦成功要求观测到目标选择且成功绘制；无邻居成功不改变选择，也不要求绘制。--detach 或中断仅结束等待；使用操作标识查询 client control-status。",
    );

pub(super) const HELP_PANEL_ZOOM_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel zoom [panel] [--panel <panel> | --current] [--toggle | --on | --off] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "Set the selected Panel zoom. Toggle is default and resolves the prior zoom once after activating the target Tab. Single-panel zoom-on/toggle succeeds with zoom-unchanged.\n\nWithout --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Success requires observed selection and a successful draw. --detach or interruption ends waiting only; query client control-status with the operation ID.",
    );

pub(super) const HELP_PANEL_ZOOM_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel zoom [panel] [--panel <panel> | --current] [--toggle | --on | --off] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "设置选中面板的缩放。默认为切换，在激活目标标签页后只解析一次原缩放状态；单面板开启或切换缩放成功返回 zoom-unchanged。\n\n省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。成功要求观测到目标选择且成功绘制。--detach 或中断仅结束等待；使用操作标识查询 client control-status。",
    );

pub(super) const HELP_PANEL_RESIZE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel resize [panel] [--panel <panel> | --current] --direction left|right|up|down [--amount <number>] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "Adjust the nearest eligible split around the source. The finite amount defaults to 0.05, becomes absolute and is capped at 0.5; the resulting ratio is clamped to 0.1..0.9. Requested edge wins, with opposite-edge fallback. No eligible split succeeds unchanged; hidden geometry fails. Focus and zoom are preserved.\n\nSelect one positional/--panel/--current target. Without a target, use the validated caller Panel or the selected Client's current Panel. Without --client, caller metadata selects one focused TUI; outside a caller exactly one current TUI is required. Explicit --client ignores caller metadata. Selected changes require a successful draw; inactive changes store without selecting or attaching. Ratios are shared across windows and saved for restart. Concurrent or replaced-split changes fail explicitly; issue a new operation after inspecting the current layout. Default timeout: 30000ms (1..3600000). --detach or interruption stops waiting only; query client control-status with the operation ID.",
    );
pub(super) const HELP_PANEL_RESIZE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel resize [panel] [--panel <panel> | --current] --direction left|right|up|down [--amount <number>] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "调整来源周围最近的可用分割。有限 amount 默认为 0.05，取绝对值并限制为至多 0.5；结果比例限制到 0.1..0.9。优先请求边缘，找不到则回退到对侧边缘。无可用分割成功返回不变；隐藏几何失败。保留焦点及 zoom。\n\n位置参数、--panel、--current 三选一。省略目标时使用经校验的调用者面板，或选中客户端的当前面板。省略 --client 时由调用者信息选择唯一聚焦 TUI；外部调用要求恰好一个有效 TUI。显式 --client 忽略调用者信息。选中目标的改变要求成功绘制；非活动目标保存比例，不选择或附着。比例跨窗口共享，并保存在重启存档中。并发修改或分割被替换会明确失败；检查当前布局后发起新操作。默认超时 30000ms（1..3600000）。--detach 或中断仅停止等待；使用操作标识查询 client control-status。",
    );
pub(super) const HELP_LAYOUT_RATIO_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path <root|0.1> --ratio <number> [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "Set one split's nominal ratio, clamping a finite number to 0.1..0.9. root addresses the Tab root; 0 and 1 select first/second children, separated by dots, at most 16 steps. A leaf or missing path fails. Hidden splits are supported. No target uses the caller/current Panel's Tab; explicit --tab addresses a Tab directly. Target selectors are exclusive. Focus and zoom are preserved.\n\nWithout --client, validated caller metadata selects one focused TUI; outside a caller exactly one current TUI is required. Explicit --client ignores caller metadata. Changed selected ratios require successful drawing; inactive changes are stored without navigation or attachment. An unchanged ratio requests no draw. Ratios are shared across windows and saved for restart. Concurrent or replaced-split changes fail explicitly; issue a new operation after inspecting the current layout. Default timeout: 30000ms (1..3600000). --detach or interruption stops waiting only; query client control-status with the operation ID.",
    );
pub(super) const HELP_LAYOUT_RATIO_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path <root|0.1> --ratio <number> [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]",
        "设置一个分割的名义比例，将有限数值限制到 0.1..0.9。root 表示标签页根；0 和 1 选择第一／第二子节点，用点分隔，至多 16 步。叶节点或不存在的路径失败。支持隐藏分割。省略目标时使用调用者／当前面板的标签页；显式 --tab 直接指定标签页。目标选择器互斥。保留焦点及 zoom。\n\n省略 --client 时，经校验的调用者信息选择唯一聚焦 TUI；外部调用要求恰好一个有效 TUI。显式 --client 忽略调用者信息。选中目标的比例改变要求成功绘制；非活动目标保存比例，不导航或附着。比例未变时不请求绘制。比例跨窗口共享，并保存在重启存档中。并发修改或分割被替换会明确失败；检查当前布局后发起新操作。默认超时 30000ms（1..3600000）。--detach 或中断仅停止等待；使用操作标识查询 client control-status。",
    );
