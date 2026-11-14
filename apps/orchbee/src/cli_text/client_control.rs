//! Formats Client navigation, selection and control receipts without choosing targets or executing transitions.
use super::Locale;
use crate::cli_text::escape::escape;
use serde_json::Value;

/// A Panel by the handle a person types next, `-` when there is none.
fn panel(id: &Value) -> String {
    id.as_str()
        .map_or_else(|| "-".to_owned(), crate::cli::handles::panel)
}

pub(super) const HELP_AGENT_FOCUS_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent focus [<target> | --agent-session <id> | --name <name> | --panel <id|@panel:0x<hex>> | --terminal <id|@terminal:0x<hex>>] [--client <id>] [--timeout <ms>] [--format text|json]",
        "Focus the live Agent's unique primary panel in an existing Windows terminal client. Choose exactly one target: a positional name/session/panel, --agent-session, --name or --panel. '@agent:<kind>' selects a unique provider; '@agent:<n>' selects a one-based ordinal in the complete Agent list, not a page. Bare names preserve Server alias precedence before provider matching. Get, read, prompt, send-keys, wait and focus share this resolver; partial projections cannot establish uniqueness. Child-only and ambiguous bindings are refused.\n\n--client selects explicitly; otherwise use the caller or the single current client. Default timeout: 30000 ms; range: 1..3600000.\n\nTimeout or Ctrl+C stops waiting without undoing accepted focus. This command does not start an Agent or activate an OS window. Successful visible paint acknowledges this client's observed Agent state unless the window is known to be unfocused.",
    );
pub(super) const HELP_AGENT_FOCUS_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent focus [<target> | --agent-session <id> | --name <name> | --panel <id|@panel:0x<hex>> | --terminal <id|@terminal:0x<hex>>] [--client <id>] [--timeout <ms>] [--format text|json]",
        "在已有 Windows 终端客户端中聚焦存活智能体唯一对应的主面板。必须选择一个目标：位置参数名称／会话／面板，或 --agent-session、--name、--panel。'@agent:<kind>' 选择唯一提供方；'@agent:<n>' 选择完整智能体列表中的一基序号（不是页内序号）。裸名称仍优先匹配 Server 别名，然后回退到提供方匹配。get、read、prompt、send-keys、wait 和 focus 共用此解析器；部分投影不能证明匹配唯一。拒绝仅有子终端或存在歧义的绑定。\n\n--client 显式选择客户端，否则使用调用者或唯一当前客户端。默认时限 30000 毫秒，范围 1..3600000。\n\n超时或 Ctrl+C 只停止等待，不撤销已接受的聚焦。不启动智能体，也不激活操作系统窗口。成功绘制会确认该客户端观察到的智能体状态，已知窗口未聚焦时除外。",
    );

impl super::CliText {
    pub(super) fn cli_client_selection_failure(self, reason: &str) -> Option<&'static str> {
        Some(match (self.locale, reason) {
            (Locale::EnUs, "client-not-found") => {
                "No current TUI Client matches the selection, so there is no current Panel or Tab to use. Without a TUI, pass an explicit target such as a Panel from `orchbee panel list` (`@panel:0x<hex>`).\n"
            }
            (Locale::ZhCn, "client-not-found") => {
                "没有与选择匹配的当前 TUI 客户端，因此没有可用的当前面板或标签页。没有 TUI 时，请传入显式目标，例如 `orchbee panel list` 中的面板（`@panel:0x<hex>`）。\n"
            }
            (Locale::EnUs, "client-ambiguous") => {
                "More than one current TUI Client matches the selection. Use `orchbee client list` and pass --client <id>.\n"
            }
            (Locale::ZhCn, "client-ambiguous") => {
                "多个当前 TUI 客户端与选择匹配。请用 `orchbee client list` 查看并传入 --client <id>。\n"
            }
            (Locale::EnUs, "client-panel-unavailable") => {
                "The selected TUI Client has no available current panel. Select a panel in that Client or pass an explicit panel target.\n"
            }
            (Locale::ZhCn, "client-panel-unavailable") => {
                "所选 TUI 客户端没有可用的当前面板。请在该客户端选择面板，或传入显式面板目标。\n"
            }
            (Locale::EnUs, "client-stale") => {
                "The selected TUI Client is stale. Inspect `orchbee client list` and choose a current Client.\n"
            }
            (Locale::ZhCn, "client-stale") => {
                "所选 TUI 客户端已过期。请用 `orchbee client list` 查看并选择当前客户端。\n"
            }
            (Locale::EnUs, "client-not-ready") => {
                "The selected TUI Client is not ready; retry after it becomes ready.\n"
            }
            (Locale::ZhCn, "client-not-ready") => "所选 TUI 客户端尚未就绪；请在其就绪后重试。\n",
            _ => return None,
        })
    }

    pub(crate) fn cli_creation(self, document: &Value) -> String {
        let (incomplete, client, ratio, focus) = match self.locale {
            Locale::EnUs => (
                "Requested Client work is incomplete; inspect before retrying.\n",
                "Client",
                "Ratio observation",
                "Focus observation",
            ),
            Locale::ZhCn => (
                "请求的客户端操作尚未完成，重试前请检查。\n",
                "客户端",
                "比例观测",
                "聚焦观测",
            ),
        };
        let result = &document["result"];
        let post = &result["postCreate"];
        let mut text = self.cli_receipt(result);
        if post["outcome"] == "incomplete" {
            text.push_str(incomplete);
        }
        if let Some(value) = post["clientId"].as_str() {
            text.push_str(&format!("{client}: {}\n", escape(value)));
        }
        let (directory_policy, directory_source, directory_path) = match self.locale {
            Locale::EnUs => ("Directory policy", "Directory source", "Directory"),
            Locale::ZhCn => ("目录策略", "目录来源", "目录"),
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
            if let Some(value) = result["directory"][key].as_str() {
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
        let document = &document["result"];
        let (title, client, source, target, canonical, request, revision, reason) =
            match self.locale {
                Locale::EnUs => (
                    "Panel swap",
                    "Client",
                    "Source",
                    "Target",
                    "Positions",
                    "Request",
                    "Revision",
                    "Reason",
                ),
                Locale::ZhCn => (
                    "面板交换",
                    "客户端",
                    "来源",
                    "目标",
                    "位置结果",
                    "请求",
                    "修订",
                    "原因",
                ),
            };
        let outcome = match (
            self.locale,
            document["outcome"].as_str().unwrap_or("not-swapped"),
        ) {
            (Locale::EnUs, "swapped") => "exchanged and focused",
            (Locale::ZhCn, "swapped") => "已交换并聚焦",
            (Locale::EnUs, "unchanged") => "unchanged",
            (Locale::ZhCn, "unchanged") => "未改变",
            (Locale::EnUs, "incomplete") => {
                "incomplete; inspect the committed result before retrying"
            }
            (Locale::ZhCn, "incomplete") => "未完整完成；重试前检查已提交结果",
            (Locale::EnUs, "unknown") => "unknown; inspect current positions before retrying",
            (Locale::ZhCn, "unknown") => "结果未知；重试前检查当前位置",
            (Locale::EnUs, _) => "not exchanged",
            (Locale::ZhCn, _) => "未交换",
        };
        let state = match (
            self.locale,
            document["canonical"]["kind"]
                .as_str()
                .unwrap_or("not-submitted"),
        ) {
            (Locale::EnUs, "committed") => "committed",
            (Locale::ZhCn, "committed") => "已提交",
            (Locale::EnUs, "unchanged") => "unchanged",
            (Locale::ZhCn, "unchanged") => "未改变",
            (Locale::EnUs, "refused") => "refused",
            (Locale::ZhCn, "refused") => "已拒绝",
            (Locale::EnUs, "unknown") => "unknown",
            (Locale::ZhCn, "unknown") => "未知",
            (Locale::EnUs, _) => "not submitted",
            (Locale::ZhCn, _) => "未提交",
        };
        let mut text = format!(
            "{title}: {outcome}\n{client}: {}\n{source}: {}\n{target}: {}\n{canonical}: {state}\n",
            escape(document["clientId"].as_str().unwrap_or("-")),
            escape(&panel(&document["source"]["panelId"])),
            escape(&panel(&document["target"]["panelId"]))
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
            let message = match (self.locale, value) {
                (Locale::ZhCn, "cross-tab") => "两个面板不在同一标签页",
                (Locale::ZhCn, "same-panel") => "来源和目标是同一面板",
                (Locale::ZhCn, "no-neighbor") => "该方向没有邻居",
                (Locale::ZhCn, "layout-unconfirmed") => "尚未确认布局读取",
                (Locale::ZhCn, "layout-revision-changed") => "布局修订已改变",
                (Locale::ZhCn, "source-unavailable") => "来源面板不可用",
                (Locale::ZhCn, "client-unavailable") => "客户端不可用",
                (Locale::ZhCn, "client-selection-changed") => "客户端选择已改变",
                (Locale::EnUs, "cross-tab") => "the Panels belong to different Tabs",
                (Locale::EnUs, "same-panel") => "source and target are the same Panel",
                (Locale::EnUs, "no-neighbor") => "no neighbor in that direction",
                (Locale::EnUs, "layout-unconfirmed") => "layout read is unconfirmed",
                (Locale::EnUs, "layout-revision-changed") => "layout revision changed",
                (Locale::EnUs, "source-unavailable") => "source Panel is unavailable",
                (Locale::EnUs, "client-unavailable") => "Client is unavailable",
                (Locale::EnUs, "client-selection-changed") => "Client selection changed",
                _ => value,
            };
            text.push_str(&format!("{reason}: {}\n", escape(message)));
        }
        if !document["focus"].is_null() {
            let focus = &document["focus"];
            let label = match self.locale {
                Locale::EnUs => "Focus",
                Locale::ZhCn => "聚焦",
            };
            let state = focus["observation"]["result"]["kind"]
                .as_str()
                .or_else(|| focus["observation"]["kind"].as_str())
                .unwrap_or("unavailable");
            text.push_str(&format!("{label}: {}\n", self.client_control_state(state)));
            if let Some(id) = focus["operationId"].as_str() {
                text.push_str(&format!("orchbee client status {}\n", escape(id)));
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
        let body = &document["result"];
        let outcome = if control {
            &body["observation"]["reply"]["outcome"]
        } else {
            &body["outcome"]
        };
        let labels = match self.locale {
            Locale::EnUs => [
                "Client",
                "Operation",
                "Context",
                "Tab",
                "Panel",
                "Maximized",
                "Reason",
                "Waiting",
                "No interactive Clients",
            ],
            Locale::ZhCn => [
                "客户端",
                "操作",
                "上下文",
                "标签页",
                "面板",
                "最大化",
                "原因",
                "等待",
                "没有交互客户端",
            ],
        };
        let mut text = String::new();
        if let Some(id) = body["operationId"].as_str() {
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
                .or_else(|| body["observation"]["kind"].as_str())
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
                    let label = match (self.locale, entry["state"]["seen"].as_bool()) {
                        (Locale::EnUs, Some(true)) => "seen",
                        (Locale::EnUs, Some(false)) => "unseen",
                        (Locale::EnUs, None) => "not observed",
                        (Locale::ZhCn, Some(true)) => "已读",
                        (Locale::ZhCn, Some(false)) => "未读",
                        (Locale::ZhCn, None) => "尚未观察到",
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
                    self.client_control_reason(reason)
                ));
            }
        }
        if let Some(wait) = body["wait"].as_str() {
            text.push_str(&format!(
                "{}: {}\n",
                labels[7],
                self.client_control_state(wait)
            ));
        }
        if control
            && (body["wait"].is_string() || body["observation"]["kind"] == "unknown")
            && let Some(id) = body["operationId"].as_str()
        {
            text.push_str(&format!("orchbee client status {}\n", escape(id)));
        }
        text
    }
    fn client_adjustment_text(self, text: &mut String, adjustment: &Value) {
        let split = &adjustment["change"]["split"];
        let no_split = adjustment["change"]["kind"] == "no-split";
        let changed = !no_split && split["previousRatio"] != split["ratio"];
        let outcome = match (
            self.locale,
            no_split,
            changed,
            adjustment["selected"] == true,
        ) {
            (Locale::EnUs, true, _, _) => "No eligible split; unchanged",
            (Locale::ZhCn, true, _, _) => "没有可调整的分割，保持不变",
            (Locale::EnUs, _, false, _) => "Ratio unchanged",
            (Locale::ZhCn, _, false, _) => "比例未改变",
            (Locale::EnUs, _, true, true) => "Ratio changed and drawn",
            (Locale::ZhCn, _, true, true) => "比例已改变并绘制",
            (Locale::EnUs, _, true, false) => "Ratio stored for inactive Tab",
            (Locale::ZhCn, _, true, false) => "已保存非活动标签页的比例",
        };
        let (target, ratio) = match self.locale {
            Locale::EnUs => ("Target Tab", "Ratio"),
            Locale::ZhCn => ("目标标签页", "比例"),
        };
        text.push_str(&format!(
            "{outcome}\n{target}: {}\n",
            escape(
                &adjustment["tabId"]
                    .as_str()
                    .map(crate::cli::handles::tab)
                    .unwrap_or_default()
            )
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
        // A Terminal Context has no handle of its own; a Tab and a Panel go by theirs.
        let context = selection["terminalContextId"].as_str().map(str::to_owned);
        let tab = selection["tabId"].as_str().map(crate::cli::handles::tab);
        let panel = selection["panelId"]
            .as_str()
            .map(crate::cli::handles::panel);
        for (label, value) in [(labels[2], context), (labels[3], tab), (labels[4], panel)] {
            if let Some(value) = value {
                text.push_str(&format!("  {label}: {}\n", escape(&value)));
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
    /// One Client refusal reason, phrased so the requester knows what to resolve.
    pub(super) fn client_control_reason(self, value: &str) -> &'static str {
        match (self.locale, value) {
            (Locale::EnUs, "busy") => "Client busy or not ready",
            (Locale::ZhCn, "busy") => "客户端忙碌或尚未就绪",
            (Locale::EnUs, "modal-open") => {
                "Another popup, overlay, dialog, or input mode (such as copy mode) is open"
            }
            (Locale::ZhCn, "modal-open") => "已有其他 popup、浮层、对话框或输入模式（如复制模式）",
            (Locale::EnUs, "insufficient-space") => "Client window too small for a popup",
            (Locale::ZhCn, "insufficient-space") => "客户端窗口太小，放不下 popup",
            _ => self.client_control_state(value),
        }
    }
    fn client_control_state(self, value: &str) -> &'static str {
        match (self.locale, value) {
            (Locale::EnUs, "popup-opened") => "Popup opened and drawn",
            (Locale::ZhCn, "popup-opened") => "popup 已打开并绘制",
            (Locale::EnUs, "popup-closed") => "Popup close completed",
            (Locale::ZhCn, "popup-closed") => "popup 关闭操作已完成",
            (Locale::EnUs, "attention-read") => "Client Agent observation",
            (Locale::ZhCn, "attention-read") => "客户端智能体观察",
            (Locale::EnUs, "current") => "Current",
            (Locale::ZhCn, "current") => "当前有效",
            (Locale::EnUs, "stale" | "stale-view") => "Stale view",
            (Locale::ZhCn, "stale" | "stale-view") => "视图已过期",
            (Locale::EnUs, "ready") => "Ready",
            (Locale::ZhCn, "ready") => "就绪",
            (Locale::EnUs, "starting") => "Starting",
            (Locale::ZhCn, "starting") => "正在启动",
            (Locale::EnUs, "busy" | "server-busy") => "Busy",
            (Locale::ZhCn, "busy" | "server-busy") => "忙碌",
            (Locale::EnUs, "queued") => "Queued",
            (Locale::ZhCn, "queued") => "已排队",
            (Locale::EnUs, "delivered") => "Delivered; awaiting display",
            (Locale::ZhCn, "delivered") => "已投递，等待界面确认",
            (Locale::EnUs, "applied") => "Applied and drawn",
            (Locale::ZhCn, "applied") => "已应用并绘制",
            (Locale::EnUs, "direction-focused") => "Neighbor focused and drawn",
            (Locale::ZhCn, "direction-focused") => "已聚焦相邻面板并绘制",
            (Locale::EnUs, "direction-unchanged") => "No neighbor; selection unchanged",
            (Locale::ZhCn, "direction-unchanged") => "该方向没有相邻面板，选择未改变",
            (Locale::EnUs, "layout-adjusted") => "Layout adjustment completed",
            (Locale::ZhCn, "layout-adjusted") => "布局调整已完成",
            (Locale::EnUs, "layout-read") => "Layout observed",
            (Locale::ZhCn, "layout-read") => "已观测布局",
            (Locale::EnUs, "zoom-toggled") => "Panel maximization toggled and drawn",
            (Locale::ZhCn, "zoom-toggled") => "面板最大化已切换并绘制",
            (Locale::EnUs, "zoom-unchanged") => "Single panel; maximization unchanged",
            (Locale::ZhCn, "zoom-unchanged") => "仅一个面板，最大化未改变",
            (Locale::EnUs, "rejected") => "Rejected",
            (Locale::ZhCn, "rejected") => "已拒绝",
            (Locale::EnUs, "incomplete") => "Incomplete; partial changes may remain",
            (Locale::ZhCn, "incomplete") => "未完成，可能已部分改变界面",
            (Locale::EnUs, "unknown") => "Outcome unknown; query the operation",
            (Locale::ZhCn, "unknown") => "结果未知，请查询操作状态",
            (Locale::EnUs, "unobserved") => "Operation not observed; read status again",
            (Locale::ZhCn, "unobserved") => "尚未读到操作状态，请再次查询",
            (Locale::EnUs, "not-submitted") => "Not submitted",
            (Locale::ZhCn, "not-submitted") => "尚未提交",
            (Locale::EnUs, "not-found") => "Not found",
            (Locale::ZhCn, "not-found") => "未找到",
            (Locale::EnUs, "conflict") => "Operation identity conflicts",
            (Locale::ZhCn, "conflict") => "操作标识冲突",
            (Locale::EnUs, "denied") => "Denied",
            (Locale::ZhCn, "denied") => "无权限",
            (Locale::EnUs, "expired" | "timed-out") => "Timed out",
            (Locale::ZhCn, "expired" | "timed-out") => "已超时",
            (Locale::EnUs, "interrupted") => "Interrupted by local interaction",
            (Locale::ZhCn, "interrupted") => "被本地交互中断",
            (Locale::EnUs, "client-disconnected") => "Client disconnected",
            (Locale::ZhCn, "client-disconnected") => "客户端已断开",
            (Locale::EnUs, "target-unavailable") => "Target unavailable",
            (Locale::ZhCn, "target-unavailable") => "目标不可用",
            (Locale::EnUs, "projection-unavailable") => "Projection unavailable",
            (Locale::ZhCn, "projection-unavailable") => "界面数据不可用",
            (Locale::EnUs, "receipt-capacity") => "Receipt capacity reached",
            (Locale::ZhCn, "receipt-capacity") => "回执容量已满",
            (Locale::EnUs, "sequence-exhausted") => "Sequence exhausted",
            (Locale::ZhCn, "sequence-exhausted") => "序列已耗尽",
            (Locale::EnUs, "unsupported") => "Unsupported",
            (Locale::ZhCn, "unsupported") => "不支持",
            (Locale::EnUs, "detached") => "Waiting detached; operation continues",
            (Locale::ZhCn, "detached") => "已停止等待，操作继续",
            (Locale::EnUs, "cancelled") => "Waiting cancelled",
            (Locale::ZhCn, "cancelled") => "等待已取消",
            (Locale::EnUs, "transport-lost" | "observation-failed") => "Observation interrupted",
            (Locale::ZhCn, "transport-lost" | "observation-failed") => "状态观测中断",
            (Locale::EnUs, "on") => "On",
            (Locale::ZhCn, "on") => "开启",
            (Locale::EnUs, "off") => "Off",
            (Locale::ZhCn, "off") => "关闭",
            (Locale::EnUs, _) => "Unavailable",
            (Locale::ZhCn, _) => "不可用",
        }
    }
}

pub(super) const HELP_CLIENT_LIST_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee client list [--format text|json]",
        "List interactive TUI views, including current or stale availability and selected Context/Tab/Panel. Ordinary CLI connections are excluded.",
    );

pub(super) const HELP_CLIENT_LIST_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee client list [--format text|json]",
        "列出交互 TUI 视图，包含当前有效或过期状态及选中的上下文／标签页／面板；不列出普通 CLI 连接。",
    );

pub(super) const HELP_CLIENT_GET_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee client get <client-id> [--format text|json]",
        "Read one live Client by its exact ID. A stale view is an observation, not confirmation that its UI responds.",
    );

pub(super) const HELP_CLIENT_GET_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee client get <client-id> [--format text|json]",
        "按精确标识读取运行中的客户端。过期视图只提供观测，不表示 UI 仍可响应。",
    );

pub(super) const HELP_WORKSPACE_FOCUS_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace focus <target> [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "Select a Workspace in one Windows TUI.\n\nWithout --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Success requires observed selection and a successful draw. Interruption ends waiting only; query client status with the reported operation ID.",
    );

pub(super) const HELP_WORKSPACE_FOCUS_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee workspace focus <target> [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "在一个 Windows TUI 中选中工作区。\n\n省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。成功要求观测到目标选择且成功绘制。中断仅结束等待；使用输出中的操作标识查询 client status。",
    );

pub(super) const HELP_TAB_FOCUS_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee tab focus <target> [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "Select a Tab in one Windows TUI, including its owning Workspace.\n\nWithout --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Success requires observed selection and a successful draw. Interruption ends waiting only; query client status with the reported operation ID.",
    );

pub(super) const HELP_TAB_FOCUS_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee tab focus <target> [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "在一个 Windows TUI 中选中标签页及其所属工作区。\n\n省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。成功要求观测到目标选择且成功绘制。中断仅结束等待；使用输出中的操作标识查询 client status。",
    );

pub(super) const HELP_PANEL_FOCUS_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee panel focus [<panel>] [--current] [--direction left|right|up|down] [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "Select a Panel in one Windows TUI, including its owning Tab and Workspace. With --direction, resolve one neighbor from this source before navigation, preserving maximization. No neighbor returns direction-unchanged without selecting the source; unavailable geometry fails. A found neighbor returns direction-focused after drawing.\n\nWithout --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Focus success requires observed selection and a successful draw; no-neighbor succeeds without changing selection or drawing. Interruption ends waiting only; query client status with the reported operation ID.",
    );

pub(super) const HELP_PANEL_FOCUS_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee panel focus [<panel>] [--current] [--direction left|right|up|down] [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "在一个 Windows TUI 中选中面板及其所属标签页和工作区。--direction 在导航前从来源解析一个邻居，保留最大化。没有邻居时返回 direction-unchanged，不选择来源；几何不可用时失败。找到邻居则在绘制后返回 direction-focused。\n\n省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。聚焦成功要求观测到目标选择且成功绘制；无邻居成功不改变选择，也不要求绘制。中断仅结束等待；使用输出中的操作标识查询 client status。",
    );

pub(super) const HELP_PANEL_MAXIMIZE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee panel maximize [<panel>] [--current] [--toggle | --on | --off] [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "Set the selected Panel's maximized state. Toggle is default and resolves the prior maximized state once after activating the target Tab. Single-panel --on or toggle succeeds with maximize-unchanged.\n\nWithout --client, validated caller Terminal/Worker metadata selects a unique focused TUI; outside a caller, exactly one current TUI is required. Partial or invalid caller metadata fails. Explicit --client ignores caller metadata. Default timeout: 30000ms (1..3600000). Success requires observed selection and a successful draw. Interruption ends waiting only; query client status with the reported operation ID.",
    );

pub(super) const HELP_PANEL_MAXIMIZE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee panel maximize [<panel>] [--current] [--toggle | --on | --off] [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "设置选中面板的最大化状态。默认为切换，在激活目标标签页后只解析一次原最大化状态；单面板开启或切换成功返回 maximize-unchanged。\n\n省略 --client 时，经校验的调用者 Terminal／Worker 信息选择唯一聚焦 TUI；外部调用要求恰好一个当前有效 TUI。部分或无效调用者信息会失败；显式 --client 忽略调用者信息。默认超时 30000ms（1..3600000）。成功要求观测到目标选择且成功绘制。中断仅结束等待；使用输出中的操作标识查询 client status。",
    );

pub(super) const HELP_PANEL_RESIZE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee panel resize [<panel>] [--current] --direction left|right|up|down [--amount <number>] [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "Adjust the nearest eligible split around the source. The finite amount defaults to 0.05, becomes absolute and is capped at 0.5; the resulting ratio is clamped to 0.1..0.9. Requested edge wins, with opposite-edge fallback. No eligible split succeeds unchanged; hidden geometry fails. Focus and maximization are preserved.\n\nSelect one positional/--current target. Without a target, use the validated caller Panel or the selected Client's current Panel. Without --client, caller metadata selects one focused TUI; outside a caller exactly one current TUI is required. Explicit --client ignores caller metadata. Selected changes require a successful draw; inactive changes store without selecting or attaching. Ratios are shared across windows and saved for restart. Concurrent or replaced-split changes fail explicitly; issue a new operation after inspecting the current layout. Default timeout: 30000ms (1..3600000). Interruption stops waiting only; query client status with the reported operation ID.",
    );
pub(super) const HELP_PANEL_RESIZE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee panel resize [<panel>] [--current] --direction left|right|up|down [--amount <number>] [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "调整来源周围最近的可用分割。有限 amount 默认为 0.05，取绝对值并限制为至多 0.5；结果比例限制到 0.1..0.9。优先请求边缘，找不到则回退到对侧边缘。无可用分割成功返回不变；隐藏几何失败。保留焦点及最大化。\n\n位置参数与 --current 互斥。省略目标时使用经校验的调用者面板，或选中客户端的当前面板。省略 --client 时由调用者信息选择唯一聚焦 TUI；外部调用要求恰好一个有效 TUI。显式 --client 忽略调用者信息。选中目标的改变要求成功绘制；非活动目标保存比例，不选择或附着。比例跨窗口共享，并保存在重启存档中。并发修改或分割被替换会明确失败；检查当前布局后发起新操作。默认超时 30000ms（1..3600000）。中断仅停止等待；使用输出中的操作标识查询 client status。",
    );
pub(super) const HELP_LAYOUT_RATIO_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path <root|0.1> --ratio <number> [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "Set one shared split ratio, clamping a finite number to 0.1..0.9. root addresses the Tab root; 0 and 1 select children, separated by dots, at most 16 steps. A leaf or missing path fails. Explicit --tab/--panel without --client commits directly to the Server, requiring no TUI; JSON uses orchbee.layout.ratio v1 and reports the request ID and outcome. Shared changes use exact revision, split and previous-ratio guards; unknown outcomes must be inspected before another request. With --client or an omitted/current target, use the selected Ready Client and its draw acknowledgment. Focus and maximization are preserved. Default timeout: 30000ms (1..3600000). Interruption stops waiting only.",
    );
pub(super) const HELP_LAYOUT_RATIO_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path <root|0.1> --ratio <number> [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "设置共享分割比例，将有限数值限制到 0.1..0.9。root 表示标签页根；0 和 1 选择子节点，用点分隔，至多 16 步。叶节点或不存在的路径失败。显式 --tab／--panel 且省略 --client 时直接提交 Server，无需 TUI；JSON 使用 orchbee.layout.ratio v1，保留请求身份与结果。共享修改校验精确修订、Split 及原比例；结果未知时先检查现状再发新请求。指定 --client 或省略／使用当前目标时，使用所选 Ready 客户端及其绘制确认。保留焦点和最大化。默认超时 30000ms（1..3600000）。中断仅停止等待。",
    );
