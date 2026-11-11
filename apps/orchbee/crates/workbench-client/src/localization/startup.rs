//! Localized startup progress and typed failure feedback; does not launch or retry processes.

use super::Locale;

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

    pub const fn startup_wait_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Press Ctrl+b then q to exit.\n",
            Self::ZhCn => "按 Ctrl+b 再按 q 退出。\n",
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
            (locale, failure) => locale.startup_failure_text(failure).into(),
        }
    }

    /// A refused macOS or Linux data or socket folder, naming the item at fault and how to fix it.
    fn admission_refusal(self, refusal: &orchbee_local_runtime::AdmissionRefusal) -> String {
        use orchbee_local_runtime::RefusalReason;
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

    const fn startup_failure_text(
        self,
        failure: &crate::startup_status::StartupFailure,
    ) -> &'static str {
        use crate::startup_status::StartupFailure;
        match (self, failure) {
            (Self::EnUs, StartupFailure::AlreadyInSession) => {
                "You are already inside this Session. Return to the existing Client to navigate, or use --session <name> to open another Session. CLI subcommands remain available.\n"
            }
            (Self::ZhCn, StartupFailure::AlreadyInSession) => {
                "当前终端已在此会话内。请返回现有客户端切换位置，或使用 --session <name> 进入其他会话。CLI 子命令仍可正常使用。\n"
            }
            (Self::EnUs, StartupFailure::Configuration) => {
                "Invalid Windows configuration. Run orchbee config check for details, correct the setting and retry.\n"
            }
            (Self::ZhCn, StartupFailure::Configuration) => {
                "Windows 配置无效。运行 orchbee config check 查看详情，修正后重试。\n"
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
            (Self::EnUs, StartupFailure::Cohort) => {
                "This Orchbee installation is missing, changed, or invalid. Stage all four binaries together before running orchbee.\n"
            }
            (Self::ZhCn, StartupFailure::Cohort) => {
                "此 Orchbee 安装缺失、已修改或无效。请将四个二进制一起暂存后再运行 orchbee。\n"
            }
            (Self::EnUs, StartupFailure::Runtime) => {
                "The local session runtime failed its ownership or permission checks.\n"
            }
            (Self::ZhCn, StartupFailure::Runtime) => "本地会话运行时未通过所有者或权限检查。\n",
            (_, StartupFailure::SocketPathTooLong) => self.socket_path_too_long(),
            (Self::EnUs, StartupFailure::RecoveryRequired) => {
                if crate::host_capabilities::startup_recovery_checkpoint() {
                    "Local startup could not confirm existing ownership or startup completion. The checkpoint is retained. Run orchbee server diagnose with the same --session selection; use orchbee --session recovery-work server start for another session. Live tasks require Host adoption; incomplete or legacy records need diagnosis.\n"
                } else {
                    "The local session requires explicit recovery. No replacement process was started.\n"
                }
            }
            (Self::ZhCn, StartupFailure::RecoveryRequired) => {
                if crate::host_capabilities::startup_recovery_checkpoint() {
                    "无法确认既有进程归属或启动完成情况，已保留检查点。请用相同 --session 选择运行 orchbee server diagnose；可执行 orchbee --session recovery-work server start 另开会话。存活任务仍需 Host 接管；不完整或旧版记录需要诊断。\n"
                } else {
                    "本地会话需要显式恢复。未启动替代进程。\n"
                }
            }
            // Rendered with their versions or the refused item above.
            (
                _,
                StartupFailure::ClientTooOld(_)
                | StartupFailure::SessionTooOld(_)
                | StartupFailure::OtherEnvironment
                | StartupFailure::DevelopmentBuildMismatch
                | StartupFailure::Refused(_),
            ) => "",
            (Self::EnUs, StartupFailure::Admission) => {
                "The local session could not admit this Client connection.\n"
            }
            (Self::ZhCn, StartupFailure::Admission) => "本地会话无法准入此客户端连接。\n",
            (Self::EnUs, StartupFailure::TimedOut) => {
                "Local terminal startup timed out. Existing session processes were retained.\n"
            }
            (Self::ZhCn, StartupFailure::TimedOut) => "本地终端启动超时。已有会话进程已保留。\n",
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
