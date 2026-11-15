//! Localized Server lifecycle outcomes and command help; does not start or stop the Server.

use super::Locale;
use crate::cli_outcome::Failure;

pub(super) const HELP_SERVER_START_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee server start [--format text|json]",
        "Ensure the selected local session is running within one 20-second budget.\nTUI startup uses the same activation and recovery path. Complete current catalog\nregistration permits automatic cold recovery only after all exact PID/creation\nidentities are gone, locks are free and endpoints absent. Background and\nclean-stop checkpoints restore layouts, Launch Profiles, sampled per-terminal\ncwd. Unreadable snapshots are retained;\nstartup falls back to an older valid copy or opens an empty workspace and warns\non stderr. Live Host, unknown/incomplete identity and unreadable records\nrefuse without deleting data. Run orchbee doctor with the same --session selection\nor use orchbee --session recovery-work server start for another name. New activation\npublishes the current version 1 catalog before checkpoint storage. Other record\nversions and shapes remain unreadable without migration. Live Host adoption remains separate.\n\nJSON uses orchbee.server.start: started means this invocation spawned Server;\nalready-running means an existing session admitted it. Live Agent state, terminal history and screen contents are not restored.",
    );

pub(super) const HELP_SERVER_START_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee server start [--format text|json]",
        "在一个 20 秒总预算内确保选中的本地会话正在运行。TUI 使用相同激活和恢复路径。仅当当前清单登记完整、所有确切 PID／创建时间身份消失、锁空闲且端点缺席时，才自动冷恢复。后台及正常停止 checkpoint 会恢复布局、Launch Profile、采样所得的逐终端 cwd。不可读存档会保留；启动会回退到较早的有效副本，或打开空工作区并在 stderr 警告。活 Host、未知／不完整身份和不可读记录均拒绝且不删除数据。请用相同 --session 选择执行 orchbee doctor，或用\norchbee --session recovery-work server start 另开命名会话。新激活会先发布当前版本 1 catalog\n再使用检查点存储；其他记录版本和形状保持不可读，不迁移。活 Host 接管仍另行实现。\n\nJSON 使用 orchbee.server.start：started 表示本次拉起了 Server；\nalready-running 表示既有会话准入了本次调用。不恢复存活智能体状态、终端历史或屏幕内容。",
    );

pub(super) const HELP_SERVER_CONNECT_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee server connect",
        "Start the selected local session when needed, admit this process as a Client, and relay the connection over standard input and output until either side ends.\nBoth streams must be pipes. For a parent process that speaks the Client protocol, such as the Desktop shell.",
    );

pub(super) const HELP_SERVER_CONNECT_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee server connect",
        "需要时启动选中的本地会话，把当前进程接入为 Client，并通过标准输入和输出转发这条连接，直到任意一端结束。\n两个流都必须是管道。供会说 Client 协议的父进程使用，例如 Desktop 外壳。",
    );

pub(super) const HELP_SERVER_STOP_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee server stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>] [--format text|json]",
        "Stop the existing managed Windows session and its terminals. This ends running\nprocesses after a fresh final save of Workbench configuration, sampled\nper-terminal cwd. Save failure keeps preceding\nvalid snapshots, retained invalid originals and running terminals; resolve it\nand explicitly retry. Live Agent state, terminal history and screen contents\nare not saved. Development and test builds require --confirm. The Windows\nrelease build refuses it and runs only in an interactive console after the\nperson types the Session name; every request is logged. There is no session\nactivation. --detach returns the admitted attempt immediately. Otherwise\nwait up to 65000 ms by default (1–3600000 ms). Cancellation/timeout stops\nwaiting, not the accepted shutdown.\n\nA nonzero 32-character lowercase hexadecimal request ID is generated if omitted.\nDuplicate starts observe the existing attempt; --retry requires --request-id\nand explicitly retries only that failed stop. Use operation status <id> to\nrecover a result. Completion requires the matching stopped record and a free\nlifetime lock; pipe disappearance alone is insufficient. A later server start\nconsumes the receipt. Linux and macOS are unsupported.\n\nWith --machine <name-or-id>, the stop is confirmed here as above, and the\nbackground machine hub then stops that saved machine's remote Session and\nevery terminal in it. --request-id, --retry, --detach and --timeout are\nrefused there; '--format json' emits orchbee.machine.server-stop.",
    );

pub(super) const HELP_SERVER_STOP_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee server stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>] [--format text|json]",
        "停止现有托管 Windows 会话及其终端；先对 Workbench 配置、采样所得的逐终端 cwd 执行最新 final 保存，再结束进程。保存失败会保留此前的有效存档、无效原件及运行中的终端；解决故障后显式重试。不保存存活智能体状态、终端历史或屏幕内容。开发版和测试版必须提供 --confirm；Windows 正式版拒绝该选项，只在交互式控制台中由人输入会话名称后执行，每次请求都会记录。不激活会话。--detach 立即返回已准入尝试；否则默认等待 65000 毫秒（范围 1–3600000）。取消或超时只停止等待，不取消已接受的停止。\n\n省略请求标识时自动生成非零 32 位小写十六进制标识。重复开始只观察现有尝试；\n--retry 必须搭配 --request-id，显式重试该失败停止。使用 operation status <id>\n恢复结果。仅准确匹配的已停止记录加空闲生命周期锁能证明完成，管道消失不够。后续 server start 会消费回执。Linux 和 macOS 尚不支持。\n\n使用 --machine <name-or-id> 时，先按上述规则在本机确认，再由后台连接进程停止该已保存机器的远端会话及其中的所有终端。此时拒绝 --request-id、--retry、--detach 和 --timeout；'--format json' 输出 orchbee.machine.server-stop 封套。",
    );

impl super::CliText {
    /// `server start` service-unavailable diagnostics, routed by the failure's
    /// stage: each one states what the start actually observed.
    pub(super) fn cli_server_unavailable(self, failure: &Failure) -> String {
        let stage = failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        match (self.locale, stage) {
            (Locale::EnUs, "platform") => {
                "This session operation is unavailable on this platform.\n".to_owned()
            }
            (Locale::ZhCn, "platform") => "当前平台尚不支持此会话操作。\n".to_owned(),
            (Locale::EnUs, "runtime-directory") => self.cli_socket_path_too_long().to_owned(),
            (Locale::ZhCn, "runtime-directory") => self.cli_socket_path_too_long().to_owned(),
            (Locale::EnUs, "discovery") => {
                "No running local session answered, and its runtime directory could not be created.\n"
                    .to_owned()
            }
            (Locale::ZhCn, "discovery") => {
                "没有运行中的本地会话应答，且其运行时目录无法创建。\n".to_owned()
            }
            (Locale::EnUs, "cohort") => self.cli_cohort_diagnostic(failure.missing_artifact()),
            (Locale::ZhCn, "cohort") => self.cli_cohort_diagnostic(failure.missing_artifact()),
            (Locale::EnUs, _) => {
                "The local session could not admit the start probe connection.\n".to_owned()
            }
            (Locale::ZhCn, _) => "本地会话无法接纳启动探针连接。\n".to_owned(),
        }
    }

    /// `server start` text answer when this invocation performed the
    /// activation and the readiness probe was admitted.
    pub(crate) const fn cli_server_started(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Local session started.\n",
            Locale::ZhCn => "本地会话已启动。\n",
        }
    }

    /// `server start` text answer when a running Session admitted the probe.
    pub(crate) const fn cli_server_already_running(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Local session is already running.\n",
            Locale::ZhCn => "本地会话已在运行。\n",
        }
    }

    // ---- help topics ---------------------------------------------------------
    /// Localizes a closed stop outcome and prints its identity for later status recovery. A Session
    /// started as administrator that refused this standard Client is named with the one way out.
    pub(crate) fn cli_server_stop_result(
        self,
        outcome: &str,
        reason: Option<&str>,
        id: &str,
        wait: Option<&str>,
        elevated_session: bool,
    ) -> String {
        let state = match (self.locale, outcome) {
            (Locale::EnUs, "stopped") => "stopped",
            (Locale::ZhCn, "stopped") => "已停止",
            (Locale::EnUs, "exited") => "ended (no terminal was open)",
            (Locale::ZhCn, "exited") => "已结束（没有打开的终端）",
            (Locale::EnUs, "pending") => "pending",
            (Locale::ZhCn, "pending") => "处理中",
            (Locale::EnUs, "failed") => "failed",
            (Locale::ZhCn, "failed") => "失败",
            (Locale::EnUs, "not-recorded") => "not recorded",
            (Locale::ZhCn, "not-recorded") => "未记录",
            (Locale::EnUs, "busy") => "another stop owns this session",
            (Locale::ZhCn, "busy") => "另一停止请求已占用此会话",
            (Locale::EnUs, "unmanaged") => "not a managed session",
            (Locale::ZhCn, "unmanaged") => "非托管会话",
            (Locale::EnUs, "unavailable") => "unavailable",
            (Locale::ZhCn, "unavailable") => "不可用",
            (Locale::EnUs, _) => "outcome unknown",
            (Locale::ZhCn, _) => "结果未知",
        };
        let waiting = match (self.locale, wait) {
            (Locale::EnUs, Some("detached")) => " (detached)",
            (Locale::ZhCn, Some("detached")) => "（后台执行）",
            (Locale::EnUs, Some("timed-out")) => " (wait timed out)",
            (Locale::ZhCn, Some("timed-out")) => "（等待超时）",
            (Locale::EnUs, Some("cancelled")) => " (wait cancelled)",
            (Locale::ZhCn, Some("cancelled")) => "（已取消等待）",
            _ => "",
        };
        if elevated_session && outcome == "unavailable" {
            let action = self
                .locale
                .startup_failure(&crate::local_startup::StartupFailure::ElevatedSession);
            return match self.locale {
                Locale::EnUs => format!("Session stop: {state}{waiting}\n{action}"),
                Locale::ZhCn => format!("会话停止：{state}{waiting}\n{action}"),
            };
        }
        // An outcome with no stop record names the next action instead of a status read.
        if let Some(action) = self.cli_server_stop_action(outcome, reason) {
            return match self.locale {
                Locale::EnUs => format!("Session stop: {state}{waiting}\n{action}\n"),
                Locale::ZhCn => format!("会话停止：{state}{waiting}\n{action}\n"),
            };
        }
        match self.locale {
            Locale::EnUs => format!(
                "Session stop: {state}{waiting}\nRequest: {id}\nInspect: orchbee operation status {id} --format json\n"
            ),
            Locale::ZhCn => format!(
                "会话停止：{state}{waiting}\n请求：{id}\n检查：orchbee operation status {id} --format json\n"
            ),
        }
    }

    /// The next action after a stop that left no record: an ended Session recovers at its next
    /// start, and a Session this build cannot end goes back to the build that started it.
    const fn cli_server_stop_action(
        self,
        outcome: &str,
        reason: Option<&str>,
    ) -> Option<&'static str> {
        Some(match (self.locale, outcome.as_bytes(), reason) {
            (Locale::EnUs, b"exited", _) => {
                "The next start recovers this Session from its last checkpoint."
            }
            (Locale::ZhCn, b"exited", _) => "下次启动时会从最近的检查点恢复此会话。",
            (Locale::EnUs, b"unavailable", Some(reason)) => match reason.as_bytes() {
                b"predates-handshake" => {
                    "This Session was started by an Orchbee from before version checks, which this build cannot stop. Run `server stop` with the Orchbee that started it, then try again."
                }
                b"exit-unsupported" => {
                    "This Session was started by another Orchbee build, which this build cannot stop. Run `server stop` with the Orchbee that started it, then try again."
                }
                b"terminals-open" => {
                    "This Session was started by another Orchbee build and has open terminals that only that build can end. Run `server stop` with the Orchbee that started it, then try again."
                }
                b"draining" => {
                    "This Session is draining for a restart and accepts no new requests. Try again shortly."
                }
                _ => return None,
            },
            (Locale::ZhCn, b"unavailable", Some(reason)) => match reason.as_bytes() {
                b"predates-handshake" => {
                    "此会话由早于版本检查的 Orchbee 启动，当前构建无法停止它。请用启动它的那个 Orchbee 运行 `server stop`，然后重试。"
                }
                b"exit-unsupported" => {
                    "此会话由另一个 Orchbee 构建启动，当前构建无法停止它。请用启动它的那个 Orchbee 运行 `server stop`，然后重试。"
                }
                b"terminals-open" => {
                    "此会话由另一个 Orchbee 构建启动，且仍有打开的终端，只有该构建能结束它们。请用启动它的那个 Orchbee 运行 `server stop`，然后重试。"
                }
                b"draining" => "此会话正在为重启排空，不再接受新请求。请稍后重试。",
                _ => return None,
            },
            _ => return None,
        })
    }
}
