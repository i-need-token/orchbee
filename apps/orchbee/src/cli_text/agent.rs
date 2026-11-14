//! Localized Agent observations and command help; does not reinterpret Agent state.

use super::Locale;
use crate::cli_outcome::Failure;

pub(super) const HELP_AGENT_LIST_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent list [--include-ended] [--limit <n>] [--cursor <c>] [--client <id>] [--format text|json]",
        "List Agent sessions observed by the running local session. '--include-ended'\nadds ended sessions; '--limit' selects 1-100 rows per page (default 50).\nWindows joins canonical sessions with physical terminal Agents; each row keeps\nits source facets, reconciled status, placement and runtime name. '--client'\nadds separate Client observations without changing semantic status. Windows\nJSON uses orchbee.agent.list v1. Its 'u1.<base64url>' cursor is at most 1024 characters;\nold or stale cursors require restarting the list. All supported platforms use v1 with explicit unsupported facets.",
    );

pub(super) const HELP_AGENT_LIST_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent list [--include-ended] [--limit <n>] [--cursor <c>] [--client <id>] [--format text|json]",
        "列出运行中的本地会话观察到的智能体会话。'--include-ended' 包含已结束会话；'--limit' 每页 1-100 条（默认 50）。Windows 会合并规范会话与物理终端智能体；每行保留来源分面、协调后的状态、位置和运行时名称。'--client' 添加独立的客户端观察，不改变语义状态。Windows JSON 使用 orchbee.agent.list v1。其 'u1.<base64url>' 游标最多 1024 个字符；旧游标或过期游标需要重新开始列表。所有支持的平台均使用 v1，并明确不支持的分面。",
    );

pub(super) const HELP_AGENT_GET_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent get (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) [--client <id>] [--format text|json]",
        "Show one Agent session. Windows resolves canonical and physical terminal\nAgents through the same projection as agent list. Windows JSON uses\norchbee.agent.get v1: 'agent' carries provider, semantic status, source,\nplacement and cwd; 'inspection' carries facet-specific detail. '--client' adds\nClient observations without replacing semantic status. Other supported platforms use the same envelope, retaining canonical\ninspection and marking the unified facet unsupported.\n\nWindows targets accept the shared Agent selectors: '@agent:<kind>' selects a\nunique provider, and '@agent:<n>' selects the one-based complete-list ordinal\n(not a page-relative number). Bare names preserve Server alias precedence,\nthen fall back to provider matching. Get, read, prompt, send-keys, wait and\nfocus use this resolver; ambiguous matches are rejected.",
    );

pub(super) const HELP_AGENT_GET_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent get (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) [--client <id>] [--format text|json]",
        "显示一个智能体会话。Windows 会通过与 agent list 相同的投影解析规范会话和物理终端智能体。Windows JSON 使用 orchbee.agent.get v1：'agent' 携带提供方、语义状态、来源、位置和 cwd；'inspection' 携带分面详情。'--client' 添加客户端观察，不替换语义状态。其他支持的平台使用相同封套，保留规范 inspection，并将统一分面标为 unsupported。\n\nWindows 目标使用共享智能体 selector：'@agent:<kind>' 选择唯一提供方；'@agent:<n>' 选择完整列表中的一基序号（不是页内序号）。裸名称仍优先匹配 Server 别名，然后回退到提供方匹配。get、read、prompt、send-keys、wait 和 focus 共用此解析器；有歧义时拒绝。",
    );

pub(super) const HELP_AGENT_PROMPT_WINDOWS_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent prompt (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <text> [--wait] [--until <condition>]... [--timeout <ms> | --forever] [--client <id>] [--format text|json]",
        "Send a prompt to one running Agent. Use --wait to observe its next state.\nWindows accepts '@agent:<kind>' for a unique provider or '@agent:<n>' for a\none-based ordinal in the complete Agent list, not the current page. Bare names\nkeep Server alias precedence before provider matching. Get, read, prompt,\nsend-keys, wait and focus share this resolver.\n--until, --timeout and --forever require --wait. Inspect the terminal before retrying uncertain input.",
    );

pub(super) const HELP_AGENT_PROMPT_WINDOWS_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent prompt (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <text> [--wait] [--until <condition>]... [--timeout <ms> | --forever] [--client <id>] [--format text|json]",
        "向正在运行的智能体发送提示。使用 --wait 观察其后续状态。\nWindows 可用 '@agent:<kind>' 选择唯一提供方，或用 '@agent:<n>' 选择完整智能体列表中的一基序号（不是当前页序号）。裸名称仍优先匹配 Server 别名，然后回退到提供方匹配。get、read、prompt、send-keys、wait 和 focus 共用此解析器。\n--until、--timeout 和 --forever 要求 --wait。投递结果不确定时，重试前先检查终端。",
    );

pub(super) const HELP_AGENT_SEND_KEYS_WINDOWS_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent send-keys (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <key>... [--format text|json]",
        "Send 1..64 semantic keys or combinations (enter, ctrl+c, alt+left, ctrl+shift+a)\nto one live Agent terminal. Windows accepts '@agent:<kind>' for a unique\nprovider or '@agent:<n>' for a one-based ordinal in the complete Agent list,\nnot the current page. Bare names keep Server alias precedence before provider\nmatching. Get, read, prompt, send-keys, wait and focus share this resolver.\nUses orchbee.terminal.send receipts. Never replays uncertain input.",
    );

pub(super) const HELP_AGENT_SEND_KEYS_WINDOWS_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent send-keys (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) <key>... [--format text|json]",
        "向一个存活智能体终端发送 1..64 个语义按键或组合（enter、ctrl+c、alt+left、ctrl+shift+a）。Windows 可用 '@agent:<kind>' 选择唯一提供方，或用 '@agent:<n>' 选择完整智能体列表中的一基序号（不是当前页序号）。裸名称仍优先匹配 Server 别名，然后回退到提供方匹配。get、read、prompt、send-keys、wait 和 focus 共用此解析器。\n使用 orchbee.terminal.send 回执。不重放不确定输入。",
    );

pub(super) const HELP_AGENT_EXPLAIN_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent explain (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) [--format text|json]",
        "Explain a canonical Agent session or a Windows physical terminal.\nPhysical targets return effective state and a fresh rule evaluation: manifest\nsource/version, winner, all rule matches and bounded region evidence. Live full\nlifecycle authority explicitly skips the screen read. No input or focus change.\nCanonical sessions retain their derived view: the canonical status basis,\nfreshness meaning, capability ceiling, observation source and fidelity,\nand the navigate-only action boundary. '--format json' emits the\norchbee.agent.explain version-1 envelope with canonical or physical subject evidence.\nTarget: one positional Session ID/name/Panel ID, --agent-session <id>,\n--name <alias>, or --panel <id|@panel:0x<hex>>. Exact retained Session IDs\nand names precede Panel fallback. Panels select their primary terminal;\nmultiple live Agents are refused. A wait stays pinned to its first Session.",
    );

pub(super) const HELP_AGENT_EXPLAIN_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent explain (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) [--format text|json]",
        "解释规范智能体会话或 Windows 物理终端。物理目标返回当前有效状态及一次新鲜规则评估：清单来源/版本、胜出规则、每条规则的匹配与有界区域证据。不发送输入或改变焦点。规范会话保留推导视图：规范状态依据、新鲜度含义、能力上限、观察来源与保真度，以及仅限导航的操作边界。'--format json' 输出\norchbee.agent.explain 版本 1 封套，包含规范或物理目标证据。目标：一个位置参数会话 ID/名称/面板 ID、--agent-session <id>、\n--name <alias> 或 --panel <id|@panel:0x<hex>>。会话 ID、名称优先于面板。\nPanel 选择主终端，多智能体匹配时拒绝；等待固定到首次解析的会话。",
    );

pub(super) const HELP_AGENT_DOCTOR_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent doctor [--provider <id>]... [--format text|json]",
        "Explain, from bounded filesystem facts alone, why Agent observations may\nnot arrive: each provider's executable and managed-hook state, its asset\nroots, the observation endpoint, and the environment override.\n'--provider' selects providers with an Orchbee integration (repeatable);\nstart-only Agents such as kiro have none to check.\nNothing is executed, connected, written, or repaired. '--format json'\nemits the orchbee.agent.doctor envelope (version 1 on supported platforms). Windows\nmarks the endpoint probe unsupported, separately from installation.",
    );

pub(super) const HELP_AGENT_DOCTOR_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent doctor [--provider <id>]... [--format text|json]",
        "仅凭有界文件系统事实解释智能体观察为何可能未到达：各 provider 的可执行文件与托管 hook 状态、其 asset root、观察端点以及环境覆盖。\n'--provider' 选择具有 Orchbee 集成的 provider（可重复），kiro 等只能启动的智能体没有可检查的集成。不执行、不连接、不写入、不修复任何内容。'--format json' 输出 orchbee.agent.doctor 封套（受支持平台均为版本 1）。Windows 将端点探测标为不支持，不会把安装状态当成链路可用。",
    );

pub(super) const HELP_AGENT_WAIT_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent wait (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) [--client <id>] [--until <condition>]... [--after-revision <n>] [--timeout <ms> | --forever] [--format text|json]",
        "Wait on one pinned Agent, using current samples and retained change pages.\nWindows defaults to idle, done, blocked with no total timeout.\nUnix defaults to needs-attention, failed, succeeded, canceled, interrupted,\nidle, ended and a 600000 ms total timeout. Unix polls current state; transitions\nbetween samples can be missed. Windows retains matching change-page evidence. --timeout\nsets 1-3600000 ms for the entire operation; --forever is an explicit synonym\nfor no total timeout and conflicts with --timeout. Each read is bounded to\nfive seconds. Ctrl+C ends the wait; no read failure reconnects.\nConditions: blocked, done, needs-attention, failed, succeeded, canceled,\ninterrupted, working, idle, unknown, ended, fresh, stale, disconnected.\nStatus conditions require fresh observations. blocked means needs-attention;\ndone means succeeded/failed/canceled/interrupted, without Client unseen state.\n--after-revision is a global fence, not a turn identity.\nTransient matching changes remain evidence even if the latest state differs.\nWindows JSON v1 separates nullable matchedChange from the current\nsession/revision; --client adds the separate Client projection in v1. Lost\nhistory or a foreign cursor fails explicitly. An ended session without a match\nis rejected. Client unseen state is sampled and never changes canonical status.\nTarget: '@agent:<kind>' selects a unique provider; '@agent:<n>' selects a\none-based ordinal in the complete Agent list, not the current page. Bare names\nkeep Server alias precedence before provider matching. Get, read, prompt,\nsend-keys, wait and focus share this resolver. A partial projection cannot\nestablish a unique provider match. Resolution occurs once before waiting;\neffects remain bound to and revalidate the resolved physical Agent.",
    );

pub(super) const HELP_AGENT_WAIT_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent wait (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) [--client <id>] [--until <条件>]... [--after-revision <n>] [--timeout <毫秒> | --forever] [--format text|json]",
        "固定一个智能体，结合当前样本和保留变更页等待。\nWindows 默认 idle、done、blocked，无总超时。\nUnix 默认 needs-attention、failed、succeeded、canceled、interrupted、idle、ended，\n总超时 600000 毫秒。Unix 轮询当前状态，可能错过两次采样之间的变化；\nWindows 保留变更页中的命中证据。--timeout 设置整个操作的\n1-3600000 毫秒期限；--forever 是无总超时的显式同义选项，与 --timeout 互斥。每次读取最多五秒；Ctrl+C 结束等待，读取失败不重新连接。条件：blocked、done、needs-attention、failed、succeeded、canceled、\ninterrupted、working、idle、unknown、ended、fresh、stale、disconnected。状态条件要求 fresh；blocked 表示 needs-attention；done 表示\nsucceeded/failed/canceled/interrupted，不合并客户端未看见状态。\n--after-revision 是全局修订门槛，不代表回合身份。即使最新状态不同，短暂命中的变更仍作为证据返回。Windows JSON v1 将可空 matchedChange 与当前 session/revision 分开；使用 --client 时在 v1 增加独立客户端投影。历史丢失或外部游标明确失败；会话已结束且没有条件命中时拒绝。客户端未看见状态仅为采样，不改变规范状态。目标：'@agent:<kind>' 选择唯一提供方；'@agent:<n>' 选择完整智能体列表中的一基序号（不是当前页序号）。裸名称仍优先匹配 Server 别名，然后回退到提供方匹配。get、read、prompt、send-keys、wait 和 focus 共用此解析器；部分投影不能证明提供方匹配唯一。等待前只解析一次；效果绑定并复核已解析的物理智能体。",
    );

pub(super) const HELP_AGENT_ATTACH_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent attach (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) [--takeover] [--timeout ms] [--mouse on|off] [--format text|json]",
        "Resolve a unique live Agent terminal, then use the Windows terminal attach\ninterface. Target may be a name, session or Panel; --name, --agent-session and\n--panel are explicit alternatives to the positional target. No focus or launch.\nCtrl+b q detaches; Ctrl+C reaches the application. Console stdin/stdout\nare required. Default duration unlimited and mouse on. See help terminal attach\nfor takeover, timeout, cleanup and input semantics.",
    );

pub(super) const HELP_AGENT_ATTACH_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent attach (<target> | --agent-session <id> | --name <name> | --panel <panel> | --terminal <terminal>) [--takeover] [--timeout ms] [--mouse on|off] [--format text|json]",
        "解析唯一的活动智能体终端，再进入 Windows terminal attach 界面。目标可为名称、会话或面板；--name、--agent-session 和 --panel 可显式替代位置参数。不聚焦或启动资源。Ctrl+b q 脱离，Ctrl+C 交付应用；必须使用控制台\nstdin/stdout。默认不限总时长、启用鼠标。接管、超时、清理和输入语义见\nhelp terminal attach。",
    );

pub(super) const HELP_AGENT_OUTPUT_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent read (<target>|--agent-session <id>|--name <name>|--panel <panel>|--terminal <terminal>) [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--plain|--ansi] [--format text|json]",
        "Read a bound terminal through one temporary read-only viewer. Default source: recent, 80 physical rows including history and the live screen. --source visible reads the screen; recent-unwrapped joins verified soft wraps; detection reads a cursor-centered screen-height window. --lines accepts 0..4294967295 and clamps to 1000. No input, focus or resize effect. --plain emits UTF-8 and conflicts with --format json. --ansi emits styled SGR text; JSON always uses orchbee.terminal.output v1 with explicit text/ansi encoding. A nullable output cursor can fence a later --after wait. The cursor is not a command completion marker. Sources never silently widen after a stale sample.",
    );

pub(super) const HELP_AGENT_OUTPUT_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent read (<target>|--agent-session <id>|--name <name>|--panel <panel>|--terminal <terminal>) [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--plain|--ansi] [--format text|json]",
        "通过一个临时只读 viewer 读取已绑定终端。默认来源 recent，包含历史及当前屏幕的 80 个物理行。--source visible 读取屏幕；recent-unwrapped 拼接已验证软换行；detection 读取光标附近一个屏幕高度的窗口。--lines 接受 0..4294967295 并钳制到 1000。不输入、不改焦点或尺寸。--plain 输出 UTF-8，与 --format json 互斥。--ansi 输出带 SGR 样式的文本，JSON 均使用 orchbee.terminal.output v1，并明确 text/ansi 编码。可空输出游标可用于后续 --after 等待，但不表示命令完成。样本陈旧时不会悄悄扩大来源范围。",
    );

impl super::CliText {
    // ---- agent read rendering ------------------------------------------------

    /// The notice printed instead of an empty agent table.
    pub(crate) const fn cli_agent_empty(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "No agent sessions.\n",
            Locale::ZhCn => "没有智能体会话。\n",
        }
    }

    /// The pagination line emitted after a non-exhaustive agent page.
    pub(crate) fn cli_agent_next_cursor(self, cursor: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("More sessions exist; continue with --cursor {cursor}\n"),
            Locale::ZhCn => format!("还有更多会话；使用 --cursor {cursor} 继续\n"),
        }
    }

    /// The single line `agent wait` prints on a match: session identity,
    /// the matched condition names in requested order, the satisfying
    /// revision, and the honest elapsed milliseconds — identical field
    /// order in both locales.
    pub(crate) fn cli_agent_wait_matched(
        self,
        session_id: &str,
        conditions: &str,
        revision: u64,
        elapsed_ms: u64,
    ) -> String {
        match self.locale {
            Locale::EnUs => format!(
                "session {session_id} matched {conditions} at revision {revision} after {elapsed_ms}ms\n"
            ),
            Locale::ZhCn => format!(
                "会话 {session_id} 命中 {conditions}（revision {revision}，用时 {elapsed_ms}ms）\n"
            ),
        }
    }

    /// Physical matches name the terminal by the handle a person targets it
    /// with and a semantic sequence, without a canonical session.
    pub(crate) fn cli_agent_terminal_wait_matched(
        self,
        handle: Option<&str>,
        conditions: &str,
        sequence: u64,
        elapsed_ms: u64,
    ) -> String {
        match (self.locale, handle) {
            (Locale::EnUs, Some(handle)) => format!(
                "{handle} matched {conditions} at state sequence {sequence} after {elapsed_ms}ms\n"
            ),
            (Locale::EnUs, None) => format!(
                "The Agent terminal matched {conditions} at state sequence {sequence} after {elapsed_ms}ms\n"
            ),
            (Locale::ZhCn, Some(handle)) => {
                format!("{handle} 命中 {conditions}（状态序号 {sequence}，用时 {elapsed_ms}ms）\n")
            }
            (Locale::ZhCn, None) => {
                format!("智能体终端命中 {conditions}（状态序号 {sequence}，用时 {elapsed_ms}ms）\n")
            }
        }
    }

    pub(crate) fn cli_agent_seen(self, seen: Option<bool>) -> &'static str {
        match (self.locale, seen) {
            (Locale::EnUs, Some(true)) => "seen",
            (Locale::EnUs, Some(false)) => "unseen",
            (Locale::EnUs, None) => "unknown",
            (Locale::ZhCn, Some(true)) => "已读",
            (Locale::ZhCn, Some(false)) => "未读",
            (Locale::ZhCn, None) => "未知",
        }
    }

    pub(crate) fn cli_agent_client_status(self, status: Option<&str>) -> &'static str {
        match (self.locale, status) {
            (Locale::ZhCn, Some("idle")) => "空闲",
            (Locale::ZhCn, Some("done")) => "待查看",
            (Locale::ZhCn, Some("working")) => "工作中",
            (Locale::ZhCn, Some("blocked")) => "等待处理",
            (Locale::ZhCn, Some("ended")) => "已结束",
            (Locale::ZhCn, Some("stale")) => "已过期",
            (Locale::ZhCn, Some("disconnected")) => "已断开",
            (Locale::ZhCn, _) => "未知",
            (Locale::EnUs, Some("idle")) => "idle",
            (Locale::EnUs, Some("done")) => "done",
            (Locale::EnUs, Some("working")) => "working",
            (Locale::EnUs, Some("blocked")) => "blocked",
            (Locale::EnUs, Some("ended")) => "ended",
            (Locale::EnUs, Some("stale")) => "stale",
            (Locale::EnUs, Some("disconnected")) => "disconnected",
            (Locale::EnUs, _) => "unknown",
        }
    }

    pub(crate) fn cli_agent_management(
        self,
        state: &orchbee_agent_observation_protocol::AgentManagement,
    ) -> String {
        use orchbee_agent_observation_protocol::AgentManagement;
        match (self.locale, state) {
            (Locale::EnUs, AgentManagement::Unmanaged {}) => "Managed startup: unmanaged\n".into(),
            (Locale::EnUs, AgentManagement::Pending { blocked, .. }) => format!(
                "Managed startup: {}\n",
                if *blocked { "blocked" } else { "pending" }
            ),
            (Locale::EnUs, AgentManagement::Active { .. }) => {
                "Managed startup: interactive readiness confirmed\n".into()
            }
            (Locale::ZhCn, AgentManagement::Unmanaged {}) => "托管启动：非托管\n".into(),
            (Locale::ZhCn, AgentManagement::Pending { blocked, .. }) => format!(
                "托管启动：{}\n",
                if *blocked {
                    "等待处理"
                } else {
                    "启动中"
                }
            ),
            (Locale::ZhCn, AgentManagement::Active { .. }) => "托管启动：已确认交互就绪\n".into(),
        }
    }

    pub(crate) fn cli_agent_runtime(self, ready: Option<bool>, reason: Option<&str>) -> String {
        match (self.locale, ready) {
            (Locale::EnUs, Some(true)) => "Prompt preflight: ready at this observation\n".into(),
            (Locale::EnUs, Some(false)) => {
                "Prompt preflight: not ready at this observation\n".into()
            }
            (Locale::ZhCn, Some(true)) => "提示提交预检：此次观察已就绪\n".into(),
            (Locale::ZhCn, Some(false)) => "提示提交预检：此次观察未就绪\n".into(),
            (Locale::EnUs, None) => format!(
                "Agent runtime unavailable: {}\n",
                reason.unwrap_or("unknown")
            ),
            (Locale::ZhCn, None) => {
                format!("智能体运行时不可用：{}\n", reason.unwrap_or("unknown"))
            }
        }
    }

    pub(crate) fn cli_agent_client_view(
        self,
        view: &crate::cli_outcome::AgentClientViewDoc,
    ) -> String {
        let (seen, focused, status) = match self.locale {
            Locale::EnUs => (
                match view.seen {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => "unknown",
                },
                if view.focused { "yes" } else { "no" },
                view.status.unwrap_or("not-observed"),
            ),
            Locale::ZhCn => (
                match view.seen {
                    Some(true) => "是",
                    Some(false) => "否",
                    None => "未知",
                },
                if view.focused { "是" } else { "否" },
                view.status.unwrap_or("尚未观察"),
            ),
        };
        match self.locale {
            Locale::EnUs => format!(
                "Client {}: status {status}; seen {seen}; focused {focused}\n",
                view.client_id
            ),
            Locale::ZhCn => format!(
                "客户端 {}：状态 {status}；已看见 {seen}；聚焦 {focused}\n",
                view.client_id
            ),
        }
    }

    /// The localized text form of one semantic Agent status, including its
    /// structured payload (attention count, active activity, activity hint).
    pub(crate) fn cli_agent_status(
        self,
        status: &orchbee_agent_observation_protocol::AgentSemanticStatus,
    ) -> String {
        use orchbee_agent_observation_protocol::AgentSemanticStatus;
        match status {
            AgentSemanticStatus::NeedsAttention { count } => match self.locale {
                Locale::EnUs => format!("needs attention ({} pending)", count.value()),
                Locale::ZhCn => format!("需要注意（{} 项待处理）", count.value()),
            },
            AgentSemanticStatus::Failed {} => match self.locale {
                Locale::EnUs => "failed".to_owned(),
                Locale::ZhCn => "失败".to_owned(),
            },
            AgentSemanticStatus::Succeeded {} => match self.locale {
                Locale::EnUs => "succeeded".to_owned(),
                Locale::ZhCn => "成功".to_owned(),
            },
            AgentSemanticStatus::Canceled {} => match self.locale {
                Locale::EnUs => "canceled".to_owned(),
                Locale::ZhCn => "已取消".to_owned(),
            },
            AgentSemanticStatus::Interrupted {} => match self.locale {
                Locale::EnUs => "interrupted".to_owned(),
                Locale::ZhCn => "已中断".to_owned(),
            },
            AgentSemanticStatus::Working { activity, phase } => {
                let phase_text = self.cli_agent_phase(*phase);
                match activity {
                    Some(activity) => {
                        format!("{} ({})", phase_text, self.cli_agent_activity(*activity))
                    }
                    None => phase_text.to_owned(),
                }
            }
            AgentSemanticStatus::Idle {} => match self.locale {
                Locale::EnUs => "idle".to_owned(),
                Locale::ZhCn => "空闲".to_owned(),
            },
            AgentSemanticStatus::Unknown { activity_hint } => match (self.locale, activity_hint) {
                (Locale::EnUs, Some(activity)) => {
                    format!("unknown (hint: {})", self.cli_agent_activity(*activity))
                }
                (Locale::ZhCn, Some(activity)) => {
                    format!("未知（提示：{}）", self.cli_agent_activity(*activity))
                }
                (Locale::EnUs, None) => "unknown".to_owned(),
                (Locale::ZhCn, None) => "未知".to_owned(),
            },
            AgentSemanticStatus::Ended {} => match self.locale {
                Locale::EnUs => "ended".to_owned(),
                Locale::ZhCn => "已结束".to_owned(),
            },
        }
    }

    pub(crate) const fn cli_agent_activity(
        self,
        activity: orchbee_agent_observation_protocol::AgentActivity,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentActivity;
        match (self.locale, activity) {
            (Locale::EnUs, AgentActivity::Thinking) => "thinking",
            (Locale::EnUs, AgentActivity::Tool) => "tool",
            (Locale::EnUs, AgentActivity::Editing) => "editing",
            (Locale::EnUs, AgentActivity::Command) => "command",
            (Locale::EnUs, AgentActivity::Subagent) => "subagent",
            (Locale::EnUs, AgentActivity::Compacting) => "compacting",
            (Locale::EnUs, AgentActivity::Retrying) => "retrying",
            (Locale::EnUs, AgentActivity::Other) => "other",
            (Locale::ZhCn, AgentActivity::Thinking) => "思考中",
            (Locale::ZhCn, AgentActivity::Tool) => "工具调用",
            (Locale::ZhCn, AgentActivity::Editing) => "编辑中",
            (Locale::ZhCn, AgentActivity::Command) => "命令执行",
            (Locale::ZhCn, AgentActivity::Subagent) => "子代理",
            (Locale::ZhCn, AgentActivity::Compacting) => "压缩中",
            (Locale::ZhCn, AgentActivity::Retrying) => "重试中",
            (Locale::ZhCn, AgentActivity::Other) => "其他",
        }
    }

    pub(crate) const fn cli_agent_phase(
        self,
        phase: orchbee_agent_observation_protocol::AgentTurnPhase,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentTurnPhase;
        match (self.locale, phase) {
            (Locale::EnUs, AgentTurnPhase::Queued) => "queued",
            (Locale::EnUs, AgentTurnPhase::Working) => "working",
            (Locale::EnUs, AgentTurnPhase::WaitingExternal) => "waiting-external",
            (Locale::ZhCn, AgentTurnPhase::Queued) => "排队中",
            (Locale::ZhCn, AgentTurnPhase::Working) => "工作中",
            (Locale::ZhCn, AgentTurnPhase::WaitingExternal) => "等待外部",
        }
    }

    pub(crate) const fn cli_agent_freshness(
        self,
        freshness: orchbee_agent_observation_protocol::AgentObservationFreshness,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentObservationFreshness;
        match (self.locale, freshness) {
            (Locale::EnUs, AgentObservationFreshness::Fresh) => "fresh",
            (Locale::EnUs, AgentObservationFreshness::Stale) => "stale",
            (Locale::EnUs, AgentObservationFreshness::Disconnected) => "disconnected",
            (Locale::ZhCn, AgentObservationFreshness::Fresh) => "新鲜",
            (Locale::ZhCn, AgentObservationFreshness::Stale) => "过期",
            (Locale::ZhCn, AgentObservationFreshness::Disconnected) => "已断开",
        }
    }

    pub(crate) const fn cli_agent_lifecycle(
        self,
        lifecycle: orchbee_agent_observation_protocol::AgentLifecycle,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentLifecycle;
        match (self.locale, lifecycle) {
            (Locale::EnUs, AgentLifecycle::Live) => "live",
            (Locale::EnUs, AgentLifecycle::Ended) => "ended",
            (Locale::ZhCn, AgentLifecycle::Live) => "存活",
            (Locale::ZhCn, AgentLifecycle::Ended) => "已结束",
        }
    }

    pub(crate) const fn cli_agent_capability(
        self,
        capability: orchbee_agent_observation_protocol::AgentCapability,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentCapability;
        match (self.locale, capability) {
            (Locale::EnUs, AgentCapability::SessionLifecycle) => "sessionLifecycle",
            (Locale::EnUs, AgentCapability::TurnLifecycle) => "turnLifecycle",
            (Locale::EnUs, AgentCapability::Questions) => "questions",
            (Locale::EnUs, AgentCapability::Approvals) => "approvals",
            (Locale::EnUs, AgentCapability::Authentication) => "authentication",
            (Locale::EnUs, AgentCapability::Activity) => "activity",
            (Locale::EnUs, AgentCapability::Subagents) => "subagents",
            (Locale::EnUs, AgentCapability::Progress) => "progress",
            (Locale::ZhCn, AgentCapability::SessionLifecycle) => "会话生命周期",
            (Locale::ZhCn, AgentCapability::TurnLifecycle) => "轮次生命周期",
            (Locale::ZhCn, AgentCapability::Questions) => "提问",
            (Locale::ZhCn, AgentCapability::Approvals) => "审批",
            (Locale::ZhCn, AgentCapability::Authentication) => "认证",
            (Locale::ZhCn, AgentCapability::Activity) => "活动",
            (Locale::ZhCn, AgentCapability::Subagents) => "子代理",
            (Locale::ZhCn, AgentCapability::Progress) => "进度",
        }
    }

    pub(crate) const fn cli_agent_attention_kind(
        self,
        kind: orchbee_agent_observation_protocol::AgentAttentionKind,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentAttentionKind;
        match (self.locale, kind) {
            (Locale::EnUs, AgentAttentionKind::Question) => "question",
            (Locale::EnUs, AgentAttentionKind::Approval) => "approval",
            (Locale::EnUs, AgentAttentionKind::Authentication) => "authentication",
            (Locale::EnUs, AgentAttentionKind::Unclassified) => "unclassified",
            (Locale::ZhCn, AgentAttentionKind::Question) => "提问",
            (Locale::ZhCn, AgentAttentionKind::Approval) => "审批",
            (Locale::ZhCn, AgentAttentionKind::Authentication) => "认证",
            (Locale::ZhCn, AgentAttentionKind::Unclassified) => "未分类",
        }
    }

    pub(crate) const fn cli_agent_outcome_kind(
        self,
        kind: orchbee_agent_observation_protocol::AgentTurnOutcomeKind,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentTurnOutcomeKind;
        match (self.locale, kind) {
            (Locale::EnUs, AgentTurnOutcomeKind::Succeeded) => "succeeded",
            (Locale::EnUs, AgentTurnOutcomeKind::Failed) => "failed",
            (Locale::EnUs, AgentTurnOutcomeKind::Canceled) => "canceled",
            (Locale::EnUs, AgentTurnOutcomeKind::Interrupted) => "interrupted",
            (Locale::ZhCn, AgentTurnOutcomeKind::Succeeded) => "成功",
            (Locale::ZhCn, AgentTurnOutcomeKind::Failed) => "失败",
            (Locale::ZhCn, AgentTurnOutcomeKind::Canceled) => "已取消",
            (Locale::ZhCn, AgentTurnOutcomeKind::Interrupted) => "已中断",
        }
    }

    pub(crate) const fn cli_agent_visibility(
        self,
        visibility: orchbee_agent_observation_protocol::AgentPresentationVisibility,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentPresentationVisibility;
        match (self.locale, visibility) {
            (Locale::EnUs, AgentPresentationVisibility::Visible) => "visible",
            (Locale::EnUs, AgentPresentationVisibility::Hidden) => "hidden",
            (Locale::EnUs, AgentPresentationVisibility::Unknown) => "unknown",
            (Locale::ZhCn, AgentPresentationVisibility::Visible) => "可见",
            (Locale::ZhCn, AgentPresentationVisibility::Hidden) => "隐藏",
            (Locale::ZhCn, AgentPresentationVisibility::Unknown) => "未知",
        }
    }

    pub(crate) const fn cli_agent_source(
        self,
        source: orchbee_agent_observation_protocol::AgentObservationSource,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentObservationSource;
        match (self.locale, source) {
            (Locale::EnUs, AgentObservationSource::Native) => "native",
            (Locale::EnUs, AgentObservationSource::Hook) => "hook",
            (Locale::EnUs, AgentObservationSource::Terminal) => "terminal",
            (Locale::EnUs, AgentObservationSource::Heuristic) => "heuristic",
            (Locale::ZhCn, AgentObservationSource::Native) => "原生",
            (Locale::ZhCn, AgentObservationSource::Hook) => "钩子",
            (Locale::ZhCn, AgentObservationSource::Terminal) => "终端",
            (Locale::ZhCn, AgentObservationSource::Heuristic) => "启发式",
        }
    }

    pub(crate) const fn cli_agent_fidelity(
        self,
        fidelity: orchbee_agent_observation_protocol::AgentObservationFidelity,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentObservationFidelity;
        match (self.locale, fidelity) {
            (Locale::EnUs, AgentObservationFidelity::Rich) => "rich",
            (Locale::EnUs, AgentObservationFidelity::Basic) => "basic",
            (Locale::EnUs, AgentObservationFidelity::Heuristic) => "heuristic",
            (Locale::ZhCn, AgentObservationFidelity::Rich) => "丰富",
            (Locale::ZhCn, AgentObservationFidelity::Basic) => "基础",
            (Locale::ZhCn, AgentObservationFidelity::Heuristic) => "启发式",
        }
    }

    pub(crate) const fn cli_agent_status_basis(
        self,
        basis: orchbee_agent_observation_protocol::AgentStatusBasis,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentStatusBasis;
        match (self.locale, basis) {
            (Locale::EnUs, AgentStatusBasis::AttentionRequest) => "attention-request",
            (Locale::EnUs, AgentStatusBasis::TurnOutcome) => "turn-outcome",
            (Locale::EnUs, AgentStatusBasis::ActiveTurn) => "active-turn",
            (Locale::EnUs, AgentStatusBasis::AffirmativeIdle) => "affirmative-idle",
            (Locale::EnUs, AgentStatusBasis::SessionEnded) => "session-ended",
            (Locale::EnUs, AgentStatusBasis::LimitedActivityHint) => "limited-activity-hint",
            (Locale::EnUs, AgentStatusBasis::UnobservedTurnState) => "unobserved-turn-state",
            (Locale::ZhCn, AgentStatusBasis::AttentionRequest) => "注意请求",
            (Locale::ZhCn, AgentStatusBasis::TurnOutcome) => "轮次结果",
            (Locale::ZhCn, AgentStatusBasis::ActiveTurn) => "活跃轮次",
            (Locale::ZhCn, AgentStatusBasis::AffirmativeIdle) => "确认空闲",
            (Locale::ZhCn, AgentStatusBasis::SessionEnded) => "会话已结束",
            (Locale::ZhCn, AgentStatusBasis::LimitedActivityHint) => "有限活动提示",
            (Locale::ZhCn, AgentStatusBasis::UnobservedTurnState) => "未观察轮次状态",
        }
    }

    pub(crate) const fn cli_agent_freshness_meaning(
        self,
        meaning: orchbee_agent_observation_protocol::AgentFreshnessMeaning,
    ) -> &'static str {
        use orchbee_agent_observation_protocol::AgentFreshnessMeaning;
        match (self.locale, meaning) {
            (Locale::EnUs, AgentFreshnessMeaning::CurrentObservation) => "current-observation",
            (Locale::EnUs, AgentFreshnessMeaning::LastObservationMayBeOutdated) => {
                "last-observation-may-be-outdated"
            }
            (Locale::EnUs, AgentFreshnessMeaning::NoTrustedCurrentRoute) => {
                "no-trusted-current-route"
            }
            (Locale::ZhCn, AgentFreshnessMeaning::CurrentObservation) => "当前观察",
            (Locale::ZhCn, AgentFreshnessMeaning::LastObservationMayBeOutdated) => {
                "上次观察可能已过期"
            }
            (Locale::ZhCn, AgentFreshnessMeaning::NoTrustedCurrentRoute) => "无可信当前路由",
        }
    }

    /// The localized boolean words used by navigable flags.
    pub(crate) const fn cli_agent_bool(self, value: bool) -> &'static str {
        match (self.locale, value) {
            (Locale::EnUs, true) => "yes",
            (Locale::EnUs, false) => "no",
            (Locale::ZhCn, true) => "是",
            (Locale::ZhCn, false) => "否",
        }
    }

    /// The navigate-only facts line on `agent explain`: every v1 action is a
    /// navigation and provider content is never carried by the read.
    pub(crate) const fn cli_agent_navigate_only_facts(self) -> &'static str {
        match self.locale {
            Locale::EnUs => {
                "actions are navigate-only; provider content is not available through agent reads\n"
            }
            Locale::ZhCn => "操作仅限导航；智能体读取不提供提供方内容\n",
        }
    }

    /// The stderr diagnostic for a post-admission Agent read failure: every
    /// kind names the operation's own stage, and `not-found` echoes the
    /// escaped session identity instead of a resource/target pair.
    pub(super) fn cli_agent_failure(self, failure: &Failure) -> String {
        use crate::cli_outcome::FailureKind;
        match failure.kind {
            FailureKind::NotFound => {
                let session = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("sessionId"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let escaped = crate::cli_text::escape::quoted(session);
                let refusal = match self.locale {
                    Locale::EnUs => format!("No agent session matches {escaped}."),
                    Locale::ZhCn => format!("没有与 {escaped} 匹配的智能体会话。"),
                };
                self.cli_with_list_hint(&refusal, "orchbee agent list")
            }
            FailureKind::Denied => match self.locale {
                Locale::EnUs => "The local session denied the agent read.\n".to_owned(),
                Locale::ZhCn => "本地会话拒绝了本次智能体读取。\n".to_owned(),
            },
            FailureKind::Rejected => {
                let reason = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("reason"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                match self.locale {
                    Locale::EnUs => {
                        format!("The local session rejected the agent list cursor ({reason}).\n")
                    }
                    Locale::ZhCn => {
                        format!("本地会话拒绝了该智能体列表游标（{reason}）。\n")
                    }
                }
            }
            FailureKind::ServiceUnavailable => match self.locale {
                Locale::EnUs => {
                    "The agent observation runtime is unavailable right now.\n".to_owned()
                }
                Locale::ZhCn => "智能体观察运行时当前不可用。\n".to_owned(),
            },
            FailureKind::Timeout => self.cli_agent_timeout(failure),
            FailureKind::TransportLoss => match self.locale {
                Locale::EnUs => {
                    "The connection to the local session was lost during the agent read.\n"
                        .to_owned()
                }
                Locale::ZhCn => "与本地会话的连接在智能体读取期间中断。\n".to_owned(),
            },
            FailureKind::Cancelled => match self.locale {
                Locale::EnUs => "The agent read was cancelled.\n".to_owned(),
                Locale::ZhCn => "智能体读取已被取消。\n".to_owned(),
            },
            FailureKind::ProtocolInvalid => match self.locale {
                Locale::EnUs => {
                    "The local session answered the agent read with an invalid protocol response.\n"
                        .to_owned()
                }
                Locale::ZhCn => "本地会话对本次智能体读取返回了无效的协议响应。\n".to_owned(),
            },
            _ => match self.locale {
                Locale::EnUs => {
                    "The CLI hit an internal failure during the agent read; no data was written.\n"
                        .to_owned()
                }
                Locale::ZhCn => "CLI 在智能体读取期间发生内部错误；未写出任何数据。\n".to_owned(),
            },
        }
    }

    /// An expired Agent read or wait names the deadline that actually ended
    /// it and, for a wait, the states it was waiting for.
    pub(super) fn cli_agent_timeout(self, failure: &Failure) -> String {
        let detail = |name: &str| failure.details.as_ref().and_then(|d| d.get(name));
        let timeout_ms = detail("timeoutMs").and_then(serde_json::Value::as_u64);
        let Some(wait) = failure.waited() else {
            let limit = self.cli_duration(timeout_ms.unwrap_or(5_000));
            return match self.locale {
                Locale::EnUs => {
                    format!("The agent read did not finish within its {limit} limit.\n")
                }
                Locale::ZhCn => format!("智能体读取未能在 {limit} 时限内完成。\n"),
            };
        };
        let separator = match self.locale {
            Locale::EnUs => ", ",
            Locale::ZhCn => "、",
        };
        let until = wait
            .until
            .iter()
            .map(|condition| condition.as_str())
            .collect::<Vec<_>>()
            .join(separator);
        let reason = detail("reason").and_then(serde_json::Value::as_str);
        match (wait.step_budget_ms, timeout_ms) {
            // The activity gate after a prompt: `agent prompt` names it
            // `prompt-stalled` for a session and `effect-deadline` for a
            // terminal-bound Agent.
            (Some(budget), _) if matches!(reason, Some("effect-deadline" | "prompt-stalled")) => {
                let budget = self.cli_duration(budget);
                match self.locale {
                    Locale::EnUs => format!(
                        "The agent did not start working within {budget} of the prompt, so the wait for {until} ended there.\n"
                    ),
                    Locale::ZhCn => format!(
                        "提示词发送后 {budget} 内智能体未开始工作，对 {until} 的等待就此结束。\n"
                    ),
                }
            }
            (Some(budget), _) => {
                let budget = self.cli_duration(budget);
                match self.locale {
                    Locale::EnUs => format!(
                        "The wait stopped early: one read of the agent state did not answer within its {budget} limit, and no awaited state ({until}) had appeared.\n"
                    ),
                    Locale::ZhCn => format!(
                        "等待提前结束：一次智能体状态读取未能在 {budget} 时限内返回，所等待的状态（{until}）尚未出现。\n"
                    ),
                }
            }
            (None, Some(timeout)) => {
                let timeout = self.cli_duration(timeout);
                match self.locale {
                    Locale::EnUs => format!(
                        "No awaited state ({until}) appeared within the {timeout} wait deadline set by --timeout.\n"
                    ),
                    Locale::ZhCn => format!(
                        "在 --timeout 设定的 {timeout} 等待期限内，未出现所等待的状态（{until}）。\n"
                    ),
                }
            }
            (None, None) => match self.locale {
                Locale::EnUs => format!(
                    "The wait deadline elapsed before any awaited state ({until}) appeared.\n"
                ),
                Locale::ZhCn => format!("等待期限已到，所等待的状态（{until}）仍未出现。\n"),
            },
        }
    }

    /// Whole seconds read as seconds; any other duration keeps its milliseconds.
    fn cli_duration(self, ms: u64) -> String {
        match (self.locale, ms.is_multiple_of(1_000)) {
            (Locale::EnUs, true) => format!("{} s", ms / 1_000),
            (Locale::EnUs, false) => format!("{ms} ms"),
            (Locale::ZhCn, true) => format!("{} 秒", ms / 1_000),
            (Locale::ZhCn, false) => format!("{ms} 毫秒"),
        }
    }

    pub(crate) const fn cli_agent_terminal_heading(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Terminal title / directory",
            Locale::ZhCn => "终端标题／目录",
        }
    }
}
