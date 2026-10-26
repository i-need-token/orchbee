//! Localized startup progress and typed failure feedback; does not launch or retry processes.

use super::Locale;

impl Locale {
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(crate) const fn startup_progress(self) -> &'static str {
        match self {
            Self::EnUs => "Starting the local terminal…\n",
            Self::ZhCn => "正在启动本地终端…\n",
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(crate) const fn startup_wait_notice(self) -> &'static str {
        match self {
            Self::EnUs => "The terminal window is too small. Resize it to continue.\n",
            Self::ZhCn => "终端窗口太小。请调整大小后继续。\n",
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(crate) const fn startup_wait_hint(self) -> &'static str {
        match self {
            Self::EnUs => "Press Ctrl+B then q to exit.\n",
            Self::ZhCn => "按 Ctrl+B 再按 q 退出。\n",
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(crate) const fn startup_failure(
        self,
        failure: crate::startup_status::StartupFailure,
    ) -> &'static str {
        use crate::startup_status::StartupFailure;
        match (self, failure) {
            #[cfg(windows)]
            (Self::EnUs, StartupFailure::AlreadyInSession) => {
                "You are already inside this Session. Return to the existing Client to navigate, or use --session <name> to open another Session. CLI subcommands remain available.\n"
            }
            #[cfg(windows)]
            (Self::ZhCn, StartupFailure::AlreadyInSession) => {
                "当前终端已在此会话内。请返回现有客户端切换位置，或使用 --session <name> 进入其他会话。CLI 子命令仍可正常使用。\n"
            }
            #[cfg(windows)]
            (Self::EnUs, StartupFailure::Configuration) => {
                "Invalid Windows configuration. Run orchbee config check for details, correct the setting and retry.\n"
            }
            #[cfg(windows)]
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
                "The native development cohort is missing, changed, or invalid. Stage all four binaries together before running orchbee.\n"
            }
            (Self::ZhCn, StartupFailure::Cohort) => {
                "原生开发 cohort 缺失、已修改或无效。请将四个二进制一起暂存后再运行 orchbee。\n"
            }
            (Self::EnUs, StartupFailure::Runtime) => {
                "The local session runtime failed its ownership or permission checks.\n"
            }
            (Self::ZhCn, StartupFailure::Runtime) => "本地会话运行时未通过所有者或权限检查。\n",
            (_, StartupFailure::SocketPathTooLong) => self.cli_socket_path_too_long(),
            (Self::EnUs, StartupFailure::RecoveryRequired) => {
                if cfg!(windows) {
                    "Local startup could not confirm existing ownership or startup completion. The checkpoint is retained. Run orchbee server diagnose with the same --session selection; use orchbee --session recovery-work server start for another session. Live tasks require Host adoption; incomplete or legacy records need diagnosis.\n"
                } else {
                    "The local session requires explicit recovery. No replacement process was started.\n"
                }
            }
            (Self::ZhCn, StartupFailure::RecoveryRequired) => {
                if cfg!(windows) {
                    "无法确认既有进程归属或启动完成情况，已保留检查点。请用相同 --session 选择运行 orchbee server diagnose；可执行 orchbee --session recovery-work server start 另开会话。存活任务仍需 Host 接管；不完整或旧版记录需要诊断。\n"
                } else {
                    "本地会话需要显式恢复。未启动替代进程。\n"
                }
            }
            (Self::EnUs, StartupFailure::Incompatible) => {
                "The running local session is incompatible with this development cohort.\n"
            }
            (Self::ZhCn, StartupFailure::Incompatible) => {
                "正在运行的本地会话与当前开发 cohort 不兼容。\n"
            }
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
