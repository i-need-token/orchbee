//! Localized Server lifecycle outcomes and command help; does not start or stop the Server.

use super::Locale;
use crate::cli::Failure;

pub(super) const HELP_SERVER_START_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server start [--format text|json]",
        "Ensure the selected local session is running within one 20-second budget.\nTUI startup uses the same activation and recovery path. Complete current catalog\nregistration permits automatic cold recovery only after all exact PID/creation\nidentities are gone, locks are free and endpoints absent. Background and\nclean-stop checkpoints restore layouts, Launch Profiles, sampled per-terminal\ncwd. Unreadable snapshots are retained;\nstartup falls back to an older valid copy or opens an empty workspace and warns\non stderr. Live Host, unknown/incomplete identity and unreadable records\nrefuse without deleting data. Run server diagnose with the same --session selection\nor use orchbee --session recovery-work server start for another name. New activation\npublishes the current version 1 catalog before checkpoint storage. Other record\nversions and shapes remain unreadable without migration. Live Host adoption remains separate.\n\nJSON uses orchbee.server.start: started means this invocation spawned Server;\nalready-running means an existing session admitted it. Live Agent state, terminal history and screen contents are not restored.",
    );

pub(super) const HELP_SERVER_START_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server start [--format text|json]",
        "在一个 20 秒总预算内确保选中的本地会话正在运行。TUI 使用相同激活和恢复路径。仅当当前清单登记完整、所有确切 PID／创建时间身份消失、锁空闲且端点缺席时，才自动冷恢复。后台及正常停止 checkpoint 会恢复布局、Launch Profile、采样所得的逐终端 cwd。不可读存档会保留；启动会回退到较早的有效副本，或打开空工作区并在 stderr 警告。活 Host、未知／不完整身份和不可读记录均拒绝且不删除数据。请用相同 --session 选择执行 server diagnose，或用\norchbee --session recovery-work server start 另开命名会话。新激活会先发布当前版本 1 catalog\n再使用检查点存储；其他记录版本和形状保持不可读，不迁移。活 Host 接管仍另行实现。\n\nJSON 使用 orchbee.server.start：started 表示本次拉起了 Server；\nalready-running 表示既有会话准入了本次调用。不恢复存活智能体状态、终端历史或屏幕内容。",
    );

pub(super) const HELP_SERVER_DIAGNOSE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server diagnose [--format text|json]",
        "Report bounded facts about the implicit per-user local session runtime —\nits base and scope directory, the activation marker and locks, and the\nfinite client/worker/pending/observation/host endpoint paths. Every row\nis a pathname or descriptor fact at its own observation instant: 'held'\nmeans some owner held the lifetime lock then — never that a session or any\nprocess is alive — and 'absent' is a pathname fact, never proof that an\nexecution ended. A 'known' marker or a valid client/worker socket leaf\nadds a conditional advisory: if no existing session can be joined, fresh\nactivation is blocked by current recovery policy.\n\nThe command never connects to an endpoint, never creates or deletes a\npath, never acquires the activation lock, and never repairs, stops, or\npredicts a later start. A cooperative five-second budget is checked\nbetween bounded filesystem operations; rows it cannot reach close as\n'uninspectable'. '--format json' emits the orchbee.server.diagnose\nenvelope on Linux and Windows. Windows reports private-DACL and bounded\npipe-name observations; unsupported members are explicit.",
    );

pub(super) const HELP_SERVER_DIAGNOSE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server diagnose [--format text|json]",
        "报告隐式每用户本地会话运行时的有界事实——基目录与 scope 目录、激活标记与锁，以及有限的 client/worker/pending/observation/host 端点路径。每一行都是其各自观察瞬间的路径名或描述符事实：'held' 表示某个属主在那一刻持有生命周期锁——绝不表示某个会话或进程存活；\n'absent' 是路径名事实，绝不证明某个执行已结束。观察到 'known' 标记或合法的 client/worker 套接字叶时附加一条条件通告：若无可加入的现有会话，按当前恢复策略新的激活将被阻止。\n\n本命令绝不连接任何端点，绝不创建或删除路径，绝不获取激活锁，也绝不修复、停止或预测后续启动。协作式五秒预算在有界的文件系统操作之间检查；无法到达的行以 'uninspectable' 闭合。'--format json'\n在 Linux 与 Windows 均输出 orchbee.server.diagnose 封套。Windows 使用私有 DACL\n和有界管道名称观察；不支持的成员明确标记。",
    );

pub(super) const HELP_SERVER_STOP_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>] [--format text|json]",
        "Stop the existing managed Windows session and its terminals. This ends running\nprocesses after a fresh final save of Workbench configuration, sampled\nper-terminal cwd. Save failure keeps preceding\nvalid snapshots, retained invalid originals and running terminals; resolve it\nand explicitly retry. Live Agent state, terminal history and screen contents\nare not saved. --confirm is required; there is no interactive prompt and no\nsession activation. --detach returns the admitted attempt immediately. Otherwise\nwait up to 65000 ms by default (1–3600000 ms). Cancellation/timeout stops\nwaiting, not the accepted shutdown.\n\nA nonzero 32-character lowercase hexadecimal request ID is generated if omitted.\nDuplicate starts observe the existing attempt; --retry requires --request-id\nand explicitly retries only that failed stop. Use server stop-status <id> to\nrecover a result. Completion requires the matching stopped record and a free\nlifetime lock; pipe disappearance alone is insufficient. A later server start\nconsumes the receipt. Linux and macOS are unsupported.",
    );

pub(super) const HELP_SERVER_STOP_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>] [--format text|json]",
        "停止现有托管 Windows 会话及其终端；先对 Workbench 配置、采样所得的逐终端 cwd 执行最新 final 保存，再结束进程。保存失败会保留此前的有效存档、无效原件及运行中的终端；解决故障后显式重试。不保存存活智能体状态、终端历史或屏幕内容。必须提供 --confirm；不弹出交互确认，也不激活会话。--detach 立即返回已准入尝试；否则默认等待 65000 毫秒（范围 1–3600000）。取消或超时只停止等待，不取消已接受的停止。\n\n省略请求标识时自动生成非零 32 位小写十六进制标识。重复开始只观察现有尝试；\n--retry 必须搭配 --request-id，显式重试该失败停止。使用 server stop-status <id>\n恢复结果。仅准确匹配的已停止记录加空闲生命周期锁能证明完成，管道消失不够。后续 server start 会消费回执。Linux 和 macOS 尚不支持。",
    );

pub(super) const HELP_SERVER_STOP_STATUS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server stop-status <operation-id> [--format text|json]",
        "Read a managed Windows stop attempt or its completed activation receipt.\nNever starts or stops a session. One five-second deadline bounds the query.\nA later start consumes the completed receipt; not-recorded does not prove\nthat a previous stop failed. Use --format json for phase and failure details.",
    );

pub(super) const HELP_SERVER_STOP_STATUS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee server stop-status <operation-id> [--format text|json]",
        "读取托管 Windows 停止尝试或其已完成激活回执，绝不启动或停止会话。查询共用五秒截止时间。后续启动会消费完成回执，未记录不代表此前停止失败。使用 --format json 查看阶段和失败详情。",
    );

impl Locale {
    /// `server start` service-unavailable diagnostics, routed by the failure's
    /// stage: each one states what the start actually observed.
    pub(super) fn cli_server_unavailable(self, failure: &Failure) -> String {
        let stage = failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        match (self, stage) {
            (Self::EnUs, "platform") => {
                "This session operation is unavailable on this platform.\n".to_owned()
            }
            (Self::ZhCn, "platform") => "当前平台尚不支持此会话操作。\n".to_owned(),
            (Self::EnUs, "runtime-directory") => self.cli_socket_path_too_long().to_owned(),
            (Self::ZhCn, "runtime-directory") => self.cli_socket_path_too_long().to_owned(),
            (Self::EnUs, "discovery") => {
                "No running local session answered, and its runtime directory could not be created.\n"
                    .to_owned()
            }
            (Self::ZhCn, "discovery") => {
                "没有运行中的本地会话应答，且其运行时目录无法创建。\n".to_owned()
            }
            (Self::EnUs, "cohort") => self.cli_cohort_diagnostic(
                failure
                    .io_diagnostic
                    .as_ref()
                    .and_then(|observed| observed.missing_artifact),
            ),
            (Self::ZhCn, "cohort") => self.cli_cohort_diagnostic(
                failure
                    .io_diagnostic
                    .as_ref()
                    .and_then(|observed| observed.missing_artifact),
            ),
            (Self::EnUs, _) => {
                "The local session could not admit the start probe connection.\n".to_owned()
            }
            (Self::ZhCn, _) => "本地会话无法接纳启动探针连接。\n".to_owned(),
        }
    }

    /// `server start` text answer when this invocation performed the
    /// activation and the readiness probe was admitted.
    pub(crate) const fn cli_server_started(self) -> &'static str {
        match self {
            Self::EnUs => "Local session started.\n",
            Self::ZhCn => "本地会话已启动。\n",
        }
    }

    /// `server start` text answer when a running Session admitted the probe.
    pub(crate) const fn cli_server_already_running(self) -> &'static str {
        match self {
            Self::EnUs => "Local session is already running.\n",
            Self::ZhCn => "本地会话已在运行。\n",
        }
    }

    // ---- help topics ---------------------------------------------------------
    /// A localized note line for one `server diagnose` advisory token; an
    /// unrecognized token renders itself verbatim — the token is the
    /// machine field, the line is only its human gloss.
    pub(crate) fn cli_server_diagnose_advisory(self, advisory: &str) -> &'static str {
        match (self, advisory) {
            (Self::EnUs, "activation-recorded") => {
                "note: activation recorded; if no existing session can be joined, fresh activation is blocked by current recovery policy.\n"
            }
            (Self::ZhCn, "activation-recorded") => {
                "注：已记录一次激活；若无可加入的现有会话，按当前恢复策略新的激活将被阻止。\n"
            }
            (Self::EnUs, "socket-residue") => {
                "note: client/worker socket residue observed; if no existing session can be joined, fresh activation is blocked by current recovery policy.\n"
            }
            (Self::ZhCn, "socket-residue") => {
                "注：观察到 client/worker 套接字残留；若无可加入的现有会话，按当前恢复策略新的激活将被阻止。\n"
            }
            (Self::EnUs, "pipe-name-present") => {
                "note: a client/worker pipe name was observed without connecting or authenticating its owner; if no session can be joined, current recovery policy blocks fresh activation.\n"
            }
            (Self::ZhCn, "pipe-name-present") => {
                "注：观察到 client/worker 管道名称，未连接或认证其属主；若无可加入的会话，当前恢复策略将阻止重新激活。\n"
            }
            (Self::EnUs, _) => "note: unrecognized advisory.\n",
            (Self::ZhCn, _) => "注：未识别的通告。\n",
        }
    }

    /// Localizes a closed stop outcome and prints its identity for later status recovery.
    pub(crate) fn cli_server_stop_result(
        self,
        outcome: &str,
        id: &str,
        wait: Option<&str>,
    ) -> String {
        let state = match (self, outcome) {
            (Self::EnUs, "stopped") => "stopped",
            (Self::ZhCn, "stopped") => "已停止",
            (Self::EnUs, "pending") => "pending",
            (Self::ZhCn, "pending") => "处理中",
            (Self::EnUs, "failed") => "failed",
            (Self::ZhCn, "failed") => "失败",
            (Self::EnUs, "not-recorded") => "not recorded",
            (Self::ZhCn, "not-recorded") => "未记录",
            (Self::EnUs, "busy") => "another stop owns this session",
            (Self::ZhCn, "busy") => "另一停止请求已占用此会话",
            (Self::EnUs, "unmanaged") => "not a managed session",
            (Self::ZhCn, "unmanaged") => "非托管会话",
            (Self::EnUs, "unavailable") => "unavailable",
            (Self::ZhCn, "unavailable") => "不可用",
            (Self::EnUs, _) => "outcome unknown",
            (Self::ZhCn, _) => "结果未知",
        };
        let waiting = match (self, wait) {
            (Self::EnUs, Some("detached")) => " (detached)",
            (Self::ZhCn, Some("detached")) => "（后台执行）",
            (Self::EnUs, Some("timed-out")) => " (wait timed out)",
            (Self::ZhCn, Some("timed-out")) => "（等待超时）",
            (Self::EnUs, Some("cancelled")) => " (wait cancelled)",
            (Self::ZhCn, Some("cancelled")) => "（已取消等待）",
            _ => "",
        };
        match self {
            Self::EnUs => format!(
                "Session stop: {state}{waiting}\nRequest: {id}\nInspect: orchbee server stop-status {id} --format json\n"
            ),
            Self::ZhCn => format!(
                "会话停止：{state}{waiting}\n请求：{id}\n检查：orchbee server stop-status {id} --format json\n"
            ),
        }
    }
}
