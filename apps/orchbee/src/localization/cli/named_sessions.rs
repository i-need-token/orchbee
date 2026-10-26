use super::Locale;
use crate::cli::named_sessions::SessionState;
use crate::invocation::CommandPath;
use orchbee_local_runtime::NamedSessionDeleteReason as Reason;

impl Locale {
    pub(crate) fn cli_session_state(self, state: SessionState) -> &'static str {
        match self {
            Self::EnUs => match state {
                SessionState::Absent => "not created",
                SessionState::Deleting => "delete incomplete",
                SessionState::RecoveryRequired => "recovery required",
                _ => state.token(),
            },
            Self::ZhCn => match state {
                SessionState::Running => "运行中",
                SessionState::Stopping => "停止中",
                SessionState::Stopped => "已停止",
                SessionState::Absent => "尚未创建",
                SessionState::Starting => "启动中",
                SessionState::Deleting => "删除未完成",
                SessionState::RecoveryRequired => "需要恢复",
                SessionState::Recoverable => "可自动恢复",
                SessionState::Unreadable => "不可读",
            },
        }
    }

    pub(crate) fn cli_session_delete(
        self,
        name: &str,
        outcome: &str,
        reason: Option<Reason>,
    ) -> String {
        let name = crate::cli::escape::escape(name);
        let message = match (self, reason) {
            (Self::EnUs, Some(Reason::DefaultSession)) => "The default session cannot be deleted.",
            (Self::ZhCn, Some(Reason::DefaultSession)) => "不能删除默认会话。",
            (Self::EnUs, Some(Reason::InUse)) => {
                "Session resources are in use; stop the session before deleting it."
            }
            (Self::ZhCn, Some(Reason::InUse)) => "会话资源正在使用；请先停止会话再删除。",
            (Self::EnUs, Some(Reason::RecoveryRequired)) => {
                "The session has no proven stopped state; inspect it with server diagnose."
            }
            (Self::ZhCn, Some(Reason::RecoveryRequired)) => {
                "无法证明会话已停止；请使用 server diagnose 检查。"
            }
            (Self::EnUs, Some(Reason::Filesystem)) => {
                "Session storage could not be admitted or removed; check ownership and file sharing."
            }
            (Self::ZhCn, Some(Reason::Filesystem)) => {
                "无法准入或删除会话存储；请检查所有权和文件占用。"
            }
            (Self::EnUs, Some(Reason::Unsupported)) => {
                "Session deletion is unavailable on this platform."
            }
            (Self::ZhCn, Some(Reason::Unsupported)) => "此平台不支持删除会话。",
            (Self::EnUs, Some(Reason::Interrupted)) => {
                "Deletion was interrupted and may have changed storage; inspect session list or repeat the same delete command."
            }
            (Self::ZhCn, Some(Reason::Interrupted)) => {
                "删除已中断，存储可能已改变；请检查 session list 或再次执行相同删除命令。"
            }
            (Self::EnUs, Some(Reason::TimedOut)) => "Session deletion reached its deadline.",
            (Self::ZhCn, Some(Reason::TimedOut)) => "会话删除已超时。",
            (Self::EnUs, None) if outcome == "absent" => "Session storage is already absent.",
            (Self::ZhCn, None) if outcome == "absent" => "会话存储已不存在。",
            (Self::EnUs, None) => "Deleted session storage.",
            (Self::ZhCn, None) => "已删除会话存储。",
        };
        let recovery = match (self, outcome) {
            (Self::EnUs, "incomplete") => {
                " Deletion remains pending; resolve the cause and repeat the same delete command."
            }
            (Self::ZhCn, "incomplete") => " 删除仍未完成；解决原因后请再次执行相同的删除命令。",
            _ => "",
        };
        format!("{name}: {message}{recovery}\n")
    }

    pub(crate) fn cli_session_command_help(
        self,
        path: CommandPath,
    ) -> crate::localization::HelpDetails {
        match (self, path) {
            (Self::EnUs, CommandPath::SessionList) => crate::localization::HelpDetails::new(
                "orchbee session list [--format text|json]",
                "Read local named sessions plus default without starting or creating them.\nAt most 256 entries, default first then exact name order, within one 5-second deadline.\nStates: running, stopping, stopped, absent, starting, deleting, recoverable, recovery-required.\nRecoverable is a guarded observation of complete registration and all exact owners gone; later startup checks again.\nUncertain state is not reported as stopped. Inventory belongs to the selected runtime base.\nJSON emits orchbee.session.list v1.",
            ),
            (Self::ZhCn, CommandPath::SessionList) => crate::localization::HelpDetails::new(
                "orchbee session list [--format text|json]",
                "只读列出本地命名会话及 default，不启动或创建会话。最多 256 条，default 在前，随后按精确名称排序，共用 5 秒期限。状态：running、stopping、stopped、absent、starting、deleting、recoverable、recovery-required。\nRecoverable 是当次受门保护的观察：登记完整且全部确切所有者消失；稍后启动仍重新检查。不确定状态不会报告为已停止。清单属于所选运行时基目录。\nJSON 输出 orchbee.session.list v1。",
            ),
            (Self::EnUs, CommandPath::SessionAttach) => crate::localization::HelpDetails::new(
                "orchbee session attach <name> [--mouse on|off]",
                "Activate the selected local session if needed and attach the Windows TUI.\nMouse defaults to on. Requires an interactive terminal; --format is not accepted.\nCtrl+B q detaches the client while the session and child processes continue.",
            ),
            (Self::ZhCn, CommandPath::SessionAttach) => crate::localization::HelpDetails::new(
                "orchbee session attach <name> [--mouse on|off]",
                "按需激活所选本地会话并连接 Windows TUI。鼠标默认开启。需要交互终端，不接受 --format。\nCtrl+B q 脱离客户端，会话和子进程继续运行。",
            ),
            (Self::EnUs, CommandPath::SessionStop) => crate::localization::HelpDetails::new(
                "orchbee session stop <name> --confirm [--request-id <id>] [--detach] [--retry] [--timeout <ms>] [--format text|json]",
                "Stop the selected managed Windows session and its terminals using the server stop protocol.\n--retry requires the prior --request-id. Default timeout: 65000 ms; bounds: 1..=3600000.\n--detach returns an accepted receipt; inspect it with --session <name> server stop-status <id>.\nClean stop preserves the workbench checkpoint.",
            ),
            (Self::ZhCn, CommandPath::SessionStop) => crate::localization::HelpDetails::new(
                "orchbee session stop <name> --confirm [--request-id <id>] [--detach] [--retry] [--timeout <ms>] [--format text|json]",
                "通过 server stop 协议停止所选托管 Windows 会话及其终端。\n--retry 要求提供先前的 --request-id。默认超时 65000 毫秒；范围 1..=3600000。\n--detach 返回已接受回执；用 --session <name> server stop-status <id> 查询。正常停止保留工作区检查点。",
            ),
            (Self::EnUs, CommandPath::SessionDelete) => crate::localization::HelpDetails::new(
                "orchbee session delete <name> --confirm [--format text|json]",
                "Delete only the selected stopped/unused named session's owned runtime and checkpoint files, including owned checkpoint.corrupt-*.raw archives.\nDefault, running, uncertain, unsafe or unknown-file state is refused. No processes are killed. Default-session raw archives require a separately authorized repair.\nThe 5-second workflow serializes against activation; partial deletion fences restart until the same command completes it.\nMissing storage succeeds as absent. User project/worktree directories are preserved.",
            ),
            (Self::ZhCn, CommandPath::SessionDelete) => crate::localization::HelpDetails::new(
                "orchbee session delete <name> --confirm [--format text|json]",
                "只删除所选已停止或未使用命名会话拥有的运行时与 checkpoint 文件，包括自有 checkpoint.corrupt-*.raw 归档。拒绝 default、运行中、不确定、不安全或含未知文件的状态；不杀进程。default 会话的 raw 归档需要另行授权的修复。流程期限 5 秒，与激活串行化；部分删除会阻止重启，直到相同命令完成清理。存储不存在时成功返回 absent。保留用户项目与 worktree 目录。",
            ),
            _ => unreachable!("session help ownership"),
        }
    }
}
