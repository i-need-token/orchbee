//! Localized startup progress and typed failure feedback; does not launch or retry processes.

use super::Locale;
use crate::host_capabilities::RecoveryCheckpoint;

impl Locale {
    /// The startup diagnostic when a derived Session socket path cannot fit one
    /// socket address on this platform.
    pub const fn socket_path_too_long(self) -> &'static str {
        match self {
            Self::EnUs => {
                "A derived local session socket path is too long for this platform's socket address limit. Set ORCHBEE_RUNTIME_BASE to a shorter absolute directory.\n"
            }
            Self::ZhCn => {
                "派生出的本地会话套接字路径超出本平台的套接字地址长度限制。请将 ORCHBEE_RUNTIME_BASE 设为更短的绝对目录。\n"
            }
        }
    }

    pub const fn startup_progress(self) -> &'static str {
        match self {
            Self::EnUs => "Starting the local terminal…\n",
            Self::ZhCn => "正在启动本地终端…\n",
        }
    }

    pub const fn startup_wait_notice(self) -> &'static str {
        match self {
            Self::EnUs => "The terminal window is too small. Resize it to continue.\n",
            Self::ZhCn => "终端窗口太小。请调整大小后继续。\n",
        }
    }

    /// The startup wait's exit line. The wait runs before any configuration loads, so the key is
    /// the default exit binding, formatted as every other key label is.
    pub fn startup_wait_hint(self) -> String {
        let exit = crate::client_view::Bindings::default()
            .labels(crate::client_view::PrefixDesc::Exit)
            .into_iter()
            .next()
            .unwrap_or_default();
        match self {
            Self::EnUs => format!("Press {exit} to exit.\n"),
            Self::ZhCn => format!("按 {exit} 退出。\n"),
        }
    }

    /// The startup failure copy; a generation refusal names both versions, leaving out one the
    /// Session did not state. Only a Session from before the handshake states none, and only the
    /// build that started it can stop it.
    pub fn startup_failure(
        self,
        failure: &crate::startup_status::StartupFailure,
    ) -> std::borrow::Cow<'static, str> {
        use crate::startup_status::{RefusedVersions, StartupFailure};
        // The Session's version, when it stated one, follows its name in parentheses.
        let session = |versions: &RefusedVersions, name: &str| match (self, &versions.session) {
            (Self::EnUs, Some(version)) => format!("{name} ({version})"),
            (Self::ZhCn, Some(version)) => format!("{name}（{version}）"),
            (_, None) => name.to_owned(),
        };
        match (self, failure) {
            (Self::EnUs, StartupFailure::ClientTooOld(versions)) => format!(
                "Orchbee {} is too old for {}. Update Orchbee, then try again.\n",
                versions.client,
                session(versions, "the running Session")
            )
            .into(),
            (Self::ZhCn, StartupFailure::ClientTooOld(versions)) => format!(
                "Orchbee {} 版本过旧，无法加入{}。请更新 Orchbee 后重试。\n",
                versions.client,
                session(versions, "正在运行的会话")
            )
            .into(),
            (Self::EnUs, StartupFailure::SessionTooOld(versions)) if versions.session.is_none() => {
                "The running Session was started by an Orchbee from before version checks. Run `server stop` with the Orchbee that started it, then try again.\n".into()
            }
            (Self::ZhCn, StartupFailure::SessionTooOld(versions)) if versions.session.is_none() => {
                "正在运行的会话由早于版本检查的 Orchbee 启动。请用启动它的那个 Orchbee 运行 `server stop`，然后重试。\n".into()
            }
            // Windows and macOS restart a Session's Server while its terminals run; the TUI and
            // Desktop try that by themselves first.
            (Self::EnUs, StartupFailure::SessionTooOld(versions))
                if cfg!(any(windows, target_os = "macos")) =>
            {
                format!(
                    "{} is too old for Orchbee {}. Run `orchbee server restart`, then try again. Terminals keep running.\n",
                    session(versions, "The running Session"),
                    versions.client
                )
                .into()
            }
            (Self::ZhCn, StartupFailure::SessionTooOld(versions))
                if cfg!(any(windows, target_os = "macos")) =>
            {
                format!(
                    "{}版本过旧，无法与 Orchbee {} 配合。请运行 `orchbee server restart`，然后重试。终端会继续运行。\n",
                    session(versions, "正在运行的会话"),
                    versions.client
                )
                .into()
            }
            (Self::EnUs, StartupFailure::SessionTooOld(versions)) => format!(
                "{} is too old for Orchbee {}. Run `orchbee server stop`, then try again.\n",
                session(versions, "The running Session"),
                versions.client
            )
            .into(),
            (Self::ZhCn, StartupFailure::SessionTooOld(versions)) => format!(
                "{}版本过旧，无法与 Orchbee {} 配合。请运行 `orchbee server stop`，然后重试。\n",
                session(versions, "正在运行的会话"),
                versions.client
            )
            .into(),
            (Self::EnUs, StartupFailure::OtherEnvironment) => {
                "This copy of Orchbee belongs to another environment and cannot join the running Session.\n".into()
            }
            (Self::ZhCn, StartupFailure::OtherEnvironment) => {
                "此 Orchbee 属于另一个环境，无法加入正在运行的会话。\n".into()
            }
            (Self::EnUs, StartupFailure::DevelopmentBuildMismatch) => {
                "This development build cannot join the running Session. Stop it with `orchbee server stop`, then try again.\n".into()
            }
            (Self::ZhCn, StartupFailure::DevelopmentBuildMismatch) => {
                "此开发构建无法加入正在运行的会话。请运行 `orchbee server stop` 停止它，然后重试。\n".into()
            }
            (locale, StartupFailure::Refused(refusal)) => locale.admission_refusal(refusal).into(),
            (locale, StartupFailure::RecoveryBlocked(blocker)) => {
                locale.recovery_blocked(blocker).into()
            }
            (locale, StartupFailure::ServerNotStarted(failure)) => {
                locale.server_not_started(failure).into()
            }
            (locale, failure) => locale.startup_failure_text(failure).into(),
        }
    }

    /// A recovery refused because a process an earlier start registered still runs, naming its
    /// role, program and process ID and what clears it: the system's own tool for ending a process,
    /// Task Manager on Windows and Activity Monitor on macOS, the two platforms that register
    /// processes. The desktop window says the same through
    /// `session.connection.unavailable.recoveryBlocked`.
    fn recovery_blocked(self, blocker: &orchbee_local_runtime::RecoveryBlocker) -> String {
        use orchbee_local_runtime::BlockerRole;
        // The runtime roles keep their English names in both languages (`docs/GLOSSARY.md`).
        let role = match blocker.role {
            BlockerRole::Server => "Server",
            BlockerRole::Worker => "Worker",
            BlockerRole::TerminalHost => "Terminal Host",
        };
        let program = blocker.role.program().file_name();
        let process = blocker.process_id;
        let macos = cfg!(target_os = "macos");
        match self {
            Self::EnUs => {
                let tool = if macos {
                    "Activity Monitor"
                } else {
                    "Task Manager"
                };
                format!(
                    "Orchbee can't start this Session while the {role} from an earlier start is still running ({program}, process {process}). End that process in {tool} or restart the computer; the next start then recovers the Session by itself.\n"
                )
            }
            Self::ZhCn => {
                let tool = if macos {
                    "活动监视器"
                } else {
                    "任务管理器"
                };
                format!(
                    "之前启动的 {role} 仍在运行（{program}，进程 {process}），Orchbee 无法启动此会话。请在{tool}中结束该进程或重启电脑，下次启动会自动恢复会话。\n"
                )
            }
        }
    }

    /// A Server the system refused to create, saying what the system refused. The desktop window
    /// says the same through the `session.connection.unavailable.serverNotStarted` messages.
    fn server_not_started(self, failure: &crate::startup_status::ServerSpawnFailure) -> String {
        match (self, failure.kind, failure.os_error) {
            (Self::EnUs, std::io::ErrorKind::NotFound, _) => {
                "Orchbee couldn't start this Session: the system couldn't find its program file. Reinstall Orchbee, or check whether security software removed the file.\n".to_owned()
            }
            (Self::ZhCn, std::io::ErrorKind::NotFound, _) => {
                "Orchbee 无法启动此会话：系统找不到它的程序文件。请重新安装 Orchbee，或检查该文件是否被安全软件移除。\n".to_owned()
            }
            (Self::EnUs, std::io::ErrorKind::PermissionDenied, _) => {
                "Orchbee couldn't start this Session: the system denied access to its program file. Security software or a system policy may be blocking it.\n".to_owned()
            }
            (Self::ZhCn, std::io::ErrorKind::PermissionDenied, _) => {
                "Orchbee 无法启动此会话：系统拒绝访问它的程序文件，可能是安全软件或系统策略阻止了它。\n".to_owned()
            }
            (Self::EnUs, _, Some(code)) => format!(
                "Orchbee couldn't start this Session: the system refused to run its program (error {code}).\n"
            ),
            (Self::ZhCn, _, Some(code)) => {
                format!("Orchbee 无法启动此会话：系统拒绝运行它的程序（错误 {code}）。\n")
            }
            (Self::EnUs, _, None) => {
                "Orchbee couldn't start this Session: the system refused to run its program.\n"
                    .to_owned()
            }
            (Self::ZhCn, _, None) => "Orchbee 无法启动此会话：系统拒绝运行它的程序。\n".to_owned(),
        }
    }

    /// A refused data or socket folder, or a file in one, naming the item at fault and how to fix
    /// it in this system's terms. The desktop window says the same through the
    /// `session.connection.unavailable.pathRefused` messages.
    pub(super) fn admission_refusal(
        self,
        refusal: &orchbee_local_runtime::AdmissionRefusal,
    ) -> String {
        use orchbee_local_runtime::RefusalReason;
        if cfg!(windows)
            && let Some(text) = self.windows_admission_refusal(refusal)
        {
            return text;
        }
        let path = refusal.path.display();
        match (self, refusal.reason) {
            (Self::EnUs, RefusalReason::OtherOwner) => format!(
                "Orchbee can't use {path}: it belongs to another user. If you started Orchbee with sudo, run it as yourself, or set ORCHBEE_HOME to a folder you own.\n"
            ),
            (Self::ZhCn, RefusalReason::OtherOwner) => format!(
                "Orchbee 无法使用 {path}：它属于其他用户。如果你用 sudo 启动了 Orchbee，请以自己的身份运行，或将 ORCHBEE_HOME 设为你拥有的文件夹。\n"
            ),
            (Self::EnUs, RefusalReason::OthersCanChange) => format!(
                "Orchbee can't use {path}: other users can change it. Run \"chmod go-w {path}\", then start Orchbee again.\n"
            ),
            (Self::ZhCn, RefusalReason::OthersCanChange) => format!(
                "Orchbee 无法使用 {path}：其他用户可以修改它。请运行 \"chmod go-w {path}\"，然后重新启动 Orchbee。\n"
            ),
            (Self::EnUs, RefusalReason::OthersCanRead) => format!(
                "Orchbee can't use {path}: other users can read it. Run \"chmod -R go-rwx {path}\", then start Orchbee again.\n"
            ),
            (Self::ZhCn, RefusalReason::OthersCanRead) => format!(
                "Orchbee 无法使用 {path}：其他用户可以读取它。请运行 \"chmod -R go-rwx {path}\"，然后重新启动 Orchbee。\n"
            ),
            (Self::EnUs, RefusalReason::PermissionsNotKept) => format!(
                "Orchbee can't keep its data in {path}: this file system doesn't keep file permissions. Set ORCHBEE_HOME to a folder on this computer's own disk.\n"
            ),
            (Self::ZhCn, RefusalReason::PermissionsNotKept) => format!(
                "Orchbee 无法把数据保存在 {path}：此文件系统不保留文件权限。请将 ORCHBEE_HOME 设为本机磁盘上的文件夹。\n"
            ),
            (Self::EnUs, RefusalReason::LocksUnsupported) => format!(
                "Orchbee can't keep its data in {path}: this file system doesn't support file locks. Set ORCHBEE_HOME to a folder on this computer's own disk.\n"
            ),
            (Self::ZhCn, RefusalReason::LocksUnsupported) => format!(
                "Orchbee 无法把数据保存在 {path}：此文件系统不支持文件锁。请将 ORCHBEE_HOME 设为本机磁盘上的文件夹。\n"
            ),
            (Self::EnUs, RefusalReason::SymbolicLink) => format!(
                "Orchbee can't use {path}: it is a symbolic link. Only the data folder itself and config.toml may be links.\n"
            ),
            (Self::ZhCn, RefusalReason::SymbolicLink) => format!(
                "Orchbee 无法使用 {path}：它是符号链接。只有数据文件夹本身和 config.toml 可以是链接。\n"
            ),
            (Self::EnUs, RefusalReason::AccessControlList) => format!(
                "Orchbee can't use {path}: it has an access control list that can let other users in. Run \"chmod -RN {path}\", then start Orchbee again.\n"
            ),
            (Self::ZhCn, RefusalReason::AccessControlList) => format!(
                "Orchbee 无法使用 {path}：它的访问控制列表可能允许其他用户访问。请运行 \"chmod -RN {path}\"，然后重新启动 Orchbee。\n"
            ),
            (Self::EnUs, RefusalReason::UnsafeSocketFolder) => format!(
                "Orchbee can't use {path} for its sockets: it belongs to another user or is open to other users. Remove it, then start Orchbee again.\n"
            ),
            (Self::ZhCn, RefusalReason::UnsafeSocketFolder) => format!(
                "Orchbee 无法用 {path} 存放套接字：它属于其他用户或对其他用户开放。请删除它，然后重新启动 Orchbee。\n"
            ),
        }
    }

    /// The Windows words for what Windows admission refuses, whose remedy is the item's security
    /// settings rather than a command; a refusal Windows never gives keeps the shared words.
    fn windows_admission_refusal(
        self,
        refusal: &orchbee_local_runtime::AdmissionRefusal,
    ) -> Option<String> {
        use orchbee_local_runtime::RefusalReason;
        let path = refusal.path.display();
        Some(match (self, refusal.reason) {
            (Self::EnUs, RefusalReason::OtherOwner) => format!(
                "Orchbee can't use {path}: its owner is an account other than you, SYSTEM or Administrators. Make your account its owner under Properties > Security > Advanced, or set ORCHBEE_HOME to a folder you own.\n"
            ),
            (Self::ZhCn, RefusalReason::OtherOwner) => format!(
                "Orchbee 无法使用 {path}：其所有者不是你、SYSTEM 或 Administrators。请在“属性 > 安全 > 高级”中将你的账户设为其所有者，或将 ORCHBEE_HOME 设为你拥有的文件夹。\n"
            ),
            (Self::EnUs, RefusalReason::OthersCanChange) => format!(
                "Orchbee can't use {path}: it has no permissions, so every account can change it. Set ORCHBEE_HOME to a folder on this computer's own disk.\n"
            ),
            (Self::ZhCn, RefusalReason::OthersCanChange) => format!(
                "Orchbee 无法使用 {path}：它没有权限设置，任何用户都可以修改它。请将 ORCHBEE_HOME 设为本机磁盘上的文件夹。\n"
            ),
            (Self::EnUs, RefusalReason::OthersCanRead) => format!(
                "Orchbee can't use {path}: its permissions let in an account other than you, SYSTEM or Administrators. Remove that access under Properties > Security > Advanced, or set ORCHBEE_HOME to a folder only you can open.\n"
            ),
            (Self::ZhCn, RefusalReason::OthersCanRead) => format!(
                "Orchbee 无法使用 {path}：其权限允许你、SYSTEM 和 Administrators 以外的用户访问。请在“属性 > 安全 > 高级”中移除该访问权限，或将 ORCHBEE_HOME 设为只有你能打开的文件夹。\n"
            ),
            (Self::EnUs, RefusalReason::AccessControlList) => format!(
                "Orchbee can't use {path}: its permissions hold an entry Orchbee can't check. Remove that entry under Properties > Security > Advanced, or set ORCHBEE_HOME to a folder only you can open.\n"
            ),
            (Self::ZhCn, RefusalReason::AccessControlList) => format!(
                "Orchbee 无法使用 {path}：其权限中有 Orchbee 无法核对的条目。请在“属性 > 安全 > 高级”中移除该条目，或将 ORCHBEE_HOME 设为只有你能打开的文件夹。\n"
            ),
            (Self::EnUs, RefusalReason::SymbolicLink) => format!(
                "Orchbee can't use {path}: it is a link or junction. Orchbee keeps its data only in real folders; set ORCHBEE_HOME to one.\n"
            ),
            (Self::ZhCn, RefusalReason::SymbolicLink) => format!(
                "Orchbee 无法使用 {path}：它是链接或联接点。Orchbee 只在真实的文件夹中保存数据；请将 ORCHBEE_HOME 设为这样的文件夹。\n"
            ),
            (
                _,
                RefusalReason::PermissionsNotKept
                | RefusalReason::LocksUnsupported
                | RefusalReason::UnsafeSocketFolder,
            ) => return None,
        })
    }

    const fn startup_failure_text(
        self,
        failure: &crate::startup_status::StartupFailure,
    ) -> &'static str {
        use crate::startup_status::StartupFailure;
        match (self, failure) {
            (Self::EnUs, StartupFailure::AlreadyInSession) => {
                "You are already inside this Session. Return to the existing window to navigate, or use --session <name> to open another Session. CLI subcommands remain available.\n"
            }
            (Self::ZhCn, StartupFailure::AlreadyInSession) => {
                "当前终端已在此会话内。请返回现有窗口切换位置，或使用 --session <name> 进入其他会话。CLI 子命令仍可正常使用。\n"
            }
            (Self::EnUs, StartupFailure::Configuration) => {
                "Invalid configuration. Run orchbee config check for details, correct the setting and retry.\n"
            }
            (Self::ZhCn, StartupFailure::Configuration) => {
                "配置无效。运行 orchbee config check 查看详情，修正后重试。\n"
            }
            (Self::EnUs, StartupFailure::Interface) => {
                "The terminal interface failed. Existing session processes were retained.\n"
            }
            (Self::ZhCn, StartupFailure::Interface) => "终端界面运行失败。已有会话进程已保留。\n",
            (Self::EnUs, StartupFailure::Terminal) => {
                "Orchbee requires the same interactive terminal on standard input and output, with valid dimensions.\n"
            }
            (Self::ZhCn, StartupFailure::Terminal) => {
                "Orchbee 的标准输入和输出需要连接同一交互终端，并提供有效尺寸。\n"
            }
            // A release user can only reinstall; staging advice is for the people who build it.
            (Self::EnUs, StartupFailure::Cohort) => match orchbee_local_runtime::environment_kind()
            {
                orchbee_local_runtime::EnvironmentKind::Release => {
                    "This Orchbee installation is damaged or incomplete. Reinstall Orchbee, then try again.\n"
                }
                orchbee_local_runtime::EnvironmentKind::Development
                | orchbee_local_runtime::EnvironmentKind::Test => {
                    "This Orchbee installation is missing, changed, or invalid. Stage all four binaries together before running orchbee.\n"
                }
            },
            (Self::ZhCn, StartupFailure::Cohort) => match orchbee_local_runtime::environment_kind()
            {
                orchbee_local_runtime::EnvironmentKind::Release => {
                    "此 Orchbee 安装已损坏或不完整。请重新安装 Orchbee，然后重试。\n"
                }
                orchbee_local_runtime::EnvironmentKind::Development
                | orchbee_local_runtime::EnvironmentKind::Test => {
                    "此 Orchbee 安装缺失、已修改或无效。请将四个二进制一起暂存后再运行 orchbee。\n"
                }
            },
            (Self::EnUs, StartupFailure::Runtime) => {
                "The local session runtime failed its ownership or permission checks.\n"
            }
            (Self::ZhCn, StartupFailure::Runtime) => "本地会话运行时未通过所有者或权限检查。\n",
            (_, StartupFailure::SocketPathTooLong) => self.socket_path_too_long(),
            (Self::EnUs, StartupFailure::RecoveryRequired) => {
                match crate::host_capabilities::startup_recovery_checkpoint() {
                    RecoveryCheckpoint::Diagnosable => {
                        "Local startup could not confirm existing ownership or startup completion. The checkpoint is retained. Run orchbee doctor with the same --session selection; use orchbee --session recovery-work server start for another session. Live tasks require Host adoption; incomplete or legacy records need diagnosis.\n"
                    }
                    RecoveryCheckpoint::Retained => {
                        "The local session requires explicit recovery. The checkpoint is retained. No replacement process was started.\n"
                    }
                    RecoveryCheckpoint::Absent => {
                        "The local session requires explicit recovery. No replacement process was started.\n"
                    }
                }
            }
            (Self::ZhCn, StartupFailure::RecoveryRequired) => {
                match crate::host_capabilities::startup_recovery_checkpoint() {
                    RecoveryCheckpoint::Diagnosable => {
                        "无法确认既有进程归属或启动完成情况，已保留检查点。请用相同 --session 选择运行 orchbee doctor；可执行 orchbee --session recovery-work server start 另开会话。存活任务仍需 Host 接管；不完整或旧版记录需要诊断。\n"
                    }
                    RecoveryCheckpoint::Retained => {
                        "本地会话需要显式恢复。已保留检查点。未启动替代进程。\n"
                    }
                    RecoveryCheckpoint::Absent => "本地会话需要显式恢复。未启动替代进程。\n",
                }
            }
            // Rendered with their versions, the refused item, the blocking process or the system's
            // answer above.
            (
                _,
                StartupFailure::ClientTooOld(_)
                | StartupFailure::SessionTooOld(_)
                | StartupFailure::OtherEnvironment
                | StartupFailure::DevelopmentBuildMismatch
                | StartupFailure::Refused(_)
                | StartupFailure::RecoveryBlocked(_)
                | StartupFailure::ServerNotStarted(_),
            ) => "",
            (Self::EnUs, StartupFailure::ElevatedSession) => {
                "This Session runs as administrator. Run `orchbee server stop` in an administrator terminal, then open Orchbee again.\n"
            }
            (Self::ZhCn, StartupFailure::ElevatedSession) => {
                "此会话以管理员身份运行。请在管理员终端中运行 `orchbee server stop`，然后重新打开 Orchbee。\n"
            }
            (Self::EnUs, StartupFailure::Admission) => {
                "The local session could not admit this window.\n"
            }
            (Self::ZhCn, StartupFailure::Admission) => "本地会话无法接入此窗口。\n",
            (Self::EnUs, StartupFailure::TimedOut) => {
                "Local terminal startup timed out. Existing session processes were retained.\n"
            }
            (Self::ZhCn, StartupFailure::TimedOut) => "本地终端启动超时。已有会话进程已保留。\n",
            (Self::EnUs, StartupFailure::SessionStopping) => {
                "The local session is still finishing an earlier stop. Try again shortly; if it persists, run orchbee doctor.\n"
            }
            (Self::ZhCn, StartupFailure::SessionStopping) => {
                "本地会话仍在完成先前的停止。请稍后重试；若一直如此，请运行 orchbee doctor。\n"
            }
            (Self::EnUs, StartupFailure::TerminalUnavailable) => {
                "The local terminal is unavailable. No terminal creation was retried.\n"
            }
            (Self::ZhCn, StartupFailure::TerminalUnavailable) => {
                "本地终端不可用。未重试终端创建。\n"
            }
            (Self::EnUs, StartupFailure::OutcomeUnknown) => {
                "The terminal operation outcome is unknown. It was not replayed.\n"
            }
            (Self::ZhCn, StartupFailure::OutcomeUnknown) => "终端操作结果未知。未重放该操作。\n",
        }
    }
}
