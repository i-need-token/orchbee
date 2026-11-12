//! Saved-machine CLI help and result messages; native picker text stays with its surface. Raw
//! OpenSSH diagnostics never reach this copy; they only classify a failure.
use super::Locale;
impl super::CliText {
    pub(crate) fn cli_machine_route_failure(self, unknown: bool) -> &'static str {
        match (self.locale, unknown) {
            (Locale::EnUs, false) => "Remote command was not submitted.",
            (Locale::ZhCn, false) => "远端命令未提交。",
            (Locale::EnUs, true) => {
                "Remote command outcome is unknown. Check remote state before retrying."
            }
            (Locale::ZhCn, true) => "远端命令结果未知，请检查远端状态后再决定是否重试。",
        }
    }

    pub(crate) fn cli_machine_help(
        self,
        path: crate::invocation::CommandPath,
    ) -> crate::cli_text::HelpDetails {
        use crate::invocation::CommandPath as C;
        let (usage, english, chinese) = match path {
            C::MachineList => (
                "orchbee machine list [--json | --format text|json]",
                "Read the saved machine catalog without contacting any machine or creating directories. Enabled is a saved preference, not a live connection observation. A machine a later version saved with a connection kind this version does not know is listed as unsupported.",
                "读取已保存的机器目录，不连接任何机器，也不创建目录。启用状态是已保存的偏好，不代表实时连接。更新版本保存的、使用此版本不认识的连接方式的机器显示为此版本不支持。",
            ),
            C::MachineAdd => (
                "orchbee machine add <ssh-target> [--label <label>] [--remote-session <name>] [--no-install] [--format text|json]",
                "The label defaults to the target's host name; names may repeat, and the ID then tells machines apart. The remote must run Windows x64 or ARM64 with an available Orchbee installation of the same environment kind: release with release, a development or test build with a development or test build. Product versions may differ; the remote Session checks each connection. Session defaults to default. Preparation has a 60-second total deadline. Failed preparation does not save the machine; an already started remote session is retained. Automatic remote installation, --no-install and interactive sign-in are not available yet.",
                "名称默认使用目标的主机名；名称可以重复，此时用 ID 区分。远端须为 Windows x64 或 ARM64，且有可用的同类 Orchbee 安装：正式版对正式版，开发版或测试版对开发版或测试版。产品版本可以不同，远端会话会检查每个连接。会话默认 default。准备过程总期限为 60 秒。准备失败不保存机器；已启动的远端会话会保留。自动安装远端程序、--no-install 和交互登录暂不可用。",
            ),
            C::MachineRename => (
                "orchbee machine rename <id|label> --label <label> [--format text|json]",
                "Rename the machine with this ID, or the only machine with this exact name. Labels cannot be blank or contain control characters and are limited to 128 UTF-8 bytes.",
                "重命名具有此 ID 的机器，或唯一使用此完整名称的机器。名称不能为空白或含控制字符，最多 128 UTF-8 字节。",
            ),
            C::MachineRemove => (
                "orchbee machine remove <id|label> [--stop-session] [--format text|json]",
                "Remove only the saved machine. Remote sessions and OpenSSH data remain owned by their existing users. --stop-session is not available yet.",
                "仅删除已保存的机器。远端会话和 OpenSSH 数据仍由原有用户管理。--stop-session 暂不可用。",
            ),
            C::MachineEnable => (
                "orchbee machine enable <id|label> [--format text|json]",
                "Enable the saved machine. This command does not establish a connection or start a remote session.",
                "启用已保存的机器。本命令不会建立连接或启动远端会话。",
            ),
            C::MachineDisable => (
                "orchbee machine disable <id|label> [--format text|json]",
                "Disable the saved machine without stopping its remote session.",
                "禁用已保存的机器，不停止远端会话。",
            ),
            C::MachineConnect => (
                "orchbee machine connect <id|label> [--remember] [--format text|json]",
                "Not available yet: the command is refused before it reads the catalog or contacts any machine.",
                "暂不可用：命令在读取机器目录或连接任何机器之前即被拒绝。",
            ),
            C::MachineDisconnect => (
                "orchbee machine disconnect <id|label> [--format text|json]",
                "Not available yet: the command is refused before it reads the catalog or contacts any machine.",
                "暂不可用：命令在读取机器目录或连接任何机器之前即被拒绝。",
            ),
            _ => unreachable!("command help ownership"),
        };
        crate::cli_text::HelpDetails::new(
            usage,
            match self.locale {
                Locale::EnUs => english,
                Locale::ZhCn => chinese,
            },
        )
    }

    pub(crate) fn cli_machine_message(self, code: &str) -> &'static str {
        match (self.locale, code) {
            (Locale::EnUs, "empty") => "No saved remote machines.",
            (Locale::ZhCn, "empty") => "尚无已保存的远程机器。",
            (Locale::EnUs, "enabled") => "enabled",
            (Locale::ZhCn, "enabled") => "已启用",
            (Locale::EnUs, "disabled") => "disabled",
            (Locale::ZhCn, "disabled") => "已禁用",
            (Locale::EnUs, "unsupported") => "unsupported by this version",
            (Locale::ZhCn, "unsupported") => "此版本不支持",
            (Locale::EnUs, "unsupported-machine") => {
                "This version does not support how this machine is connected."
            }
            (Locale::ZhCn, "unsupported-machine") => "此版本不支持这台机器的连接方式。",
            (Locale::EnUs, "saved") => "Saved machine catalog change:",
            (Locale::ZhCn, "saved") => "已保存机器目录变更：",
            (Locale::EnUs, "unchanged") => "Machine unchanged:",
            (Locale::ZhCn, "unchanged") => "机器未变化：",
            (Locale::EnUs, "already-committed") => {
                "The catalog change was committed before this interruption or failure."
            }
            (Locale::ZhCn, "already-committed") => "目录变更已在本次中断或失败前提交。",
            (Locale::EnUs, "remote-retained") => {
                "The remote session may be running; this command does not roll it back."
            }
            (Locale::ZhCn, "remote-retained") => "远端会话可能仍在运行；本命令不会回滚它。",
            (Locale::EnUs, "configuration") => {
                "The Orchbee data folder is unavailable; check USERPROFILE or ORCHBEE_HOME."
            }
            (Locale::ZhCn, "configuration") => {
                "Orchbee 数据文件夹不可用；请检查 USERPROFILE 或 ORCHBEE_HOME。"
            }
            (Locale::EnUs, "invalid-catalog") => {
                "The machine catalog is invalid; existing data was preserved."
            }
            (Locale::ZhCn, "invalid-catalog") => "机器目录无效；已有数据保持不变。",
            (Locale::EnUs, "storage") => "The private machine catalog could not be read or saved.",
            (Locale::ZhCn, "storage") => "无法读取或保存私有机器目录。",
            (Locale::EnUs, "busy") => {
                "Another machine catalog writer is active. Retry after it finishes."
            }
            (Locale::ZhCn, "busy") => "另一个机器目录写入者正在运行，请在其结束后重试。",
            (Locale::EnUs, "not-found") => {
                "No saved machine has this ID or exact name. Run 'orchbee machine list'."
            }
            (Locale::ZhCn, "not-found") => {
                "没有已保存的机器使用此 ID 或完整名称，请运行 ‘orchbee machine list’。"
            }
            (Locale::EnUs, "ambiguous-machine") => {
                "Several saved machines share this name. Use the ID from 'orchbee machine list'."
            }
            (Locale::ZhCn, "ambiguous-machine") => {
                "多台已保存的机器使用此名称，请改用 ‘orchbee machine list’ 中的 ID。"
            }
            (Locale::EnUs, "not-yet-available") => {
                "This machine command is not available yet. Nothing was read or changed."
            }
            (Locale::ZhCn, "not-yet-available") => "此机器命令暂不可用，未读取或更改任何内容。",
            (Locale::EnUs, "capacity") => {
                "The machine catalog has reached its capacity of 64 machines."
            }
            (Locale::ZhCn, "capacity") => "机器目录已达到 64 台机器的容量上限。",
            (Locale::EnUs, "revision-exhausted") => "The machine catalog revision cannot advance.",
            (Locale::ZhCn, "revision-exhausted") => "机器目录修订号无法继续增加。",
            (Locale::EnUs, "ssh-unavailable") => {
                "Windows OpenSSH Client is unavailable in the system directory."
            }
            (Locale::ZhCn, "ssh-unavailable") => "系统目录中的 Windows OpenSSH 客户端不可用。",
            (Locale::EnUs, "ssh-failed") => {
                "SSH preparation failed. Check OpenSSH batch authentication, host trust, and the installed remote orchbee.exe."
            }
            (Locale::ZhCn, "ssh-failed") => {
                "SSH 准备失败，请检查 OpenSSH 批处理认证、主机信任及已安装的远端 orchbee.exe。"
            }
            (Locale::EnUs, "remote-protocol") => {
                "The remote Client returned an invalid or unsupported status document."
            }
            (Locale::ZhCn, "remote-protocol") => "远端客户端返回了无效或不支持的状态文档。",
            (Locale::EnUs, "remote-unsupported") => {
                "This version supports remote machines running Windows x64 or ARM64 only."
            }
            (Locale::ZhCn, "remote-unsupported") => {
                "此版本只支持运行 Windows x64 或 ARM64 的远程机器。"
            }
            (Locale::EnUs, "remote-not-installed") => {
                "The remote machine has no usable Orchbee installation."
            }
            (Locale::ZhCn, "remote-not-installed") => "远程机器上没有可用的 Orchbee。",
            (Locale::EnUs, "remote-other-environment") => {
                "The remote Orchbee belongs to another environment. Use release on both machines, or builds from the same checkout path on both."
            }
            (Locale::ZhCn, "remote-other-environment") => {
                "远端 Orchbee 属于另一个环境。请两台机器都用正式版，或都用同一检出路径的构建。"
            }
            (Locale::EnUs, "remote-version") => "Remote Orchbee version:",
            (Locale::ZhCn, "remote-version") => "远端 Orchbee 版本：",
            (Locale::EnUs, "remote-not-ready") => {
                "The remote session could not be confirmed ready through a new SSH connection."
            }
            (Locale::ZhCn, "remote-not-ready") => "无法通过新的 SSH 连接确认远端会话已就绪。",
            (Locale::EnUs, "cancelled") => "Machine operation interrupted.",
            (Locale::ZhCn, "cancelled") => "机器操作已中断。",
            (Locale::EnUs, "timeout") => "The machine operation deadline expired.",
            (Locale::ZhCn, "timeout") => "机器操作已超时。",
            (Locale::EnUs, "physical-unknown") => {
                "SSH process cleanup could not be confirmed. Remote state may have changed."
            }
            (Locale::ZhCn, "physical-unknown") => {
                "无法确认 SSH 进程清理完成，远端状态可能已经变化。"
            }
            (Locale::EnUs, _) => "Machine operation failed before completion.",
            (Locale::ZhCn, _) => "机器操作未能完成。",
        }
    }

    /// The machine hub state printer's failure line.
    #[cfg(windows)]
    pub(crate) fn cli_machine_hub_failure(
        self,
        failure: crate::cli::machines::hub::StateFailure,
    ) -> &'static str {
        use crate::cli::machines::hub::StateFailure as F;
        match (self.locale, failure) {
            (Locale::EnUs, F::DataRoot) => {
                "The Orchbee data folder is unavailable; check USERPROFILE or ORCHBEE_HOME."
            }
            (Locale::ZhCn, F::DataRoot) => {
                "Orchbee 数据文件夹不可用；请检查 USERPROFILE 或 ORCHBEE_HOME。"
            }
            (Locale::EnUs, F::Unavailable) => "The machine hub could not be reached or started.",
            (Locale::ZhCn, F::Unavailable) => "无法连接或启动连接进程。",
            (Locale::EnUs, F::Closing) => "The machine hub is closing. Try again.",
            (Locale::ZhCn, F::Closing) => "连接进程正在关闭，请重试。",
            (Locale::EnUs, F::Lost) => "The connection to the machine hub was lost.",
            (Locale::ZhCn, F::Lost) => "与连接进程的连接已断开。",
            (Locale::EnUs, F::Incompatible) => {
                "The running machine hub belongs to another environment or version."
            }
            (Locale::ZhCn, F::Incompatible) => "正在运行的连接进程属于其他环境或版本。",
            (Locale::EnUs, F::Timeout) => "The machine hub did not answer in time.",
            (Locale::ZhCn, F::Timeout) => "连接进程未及时响应。",
        }
    }

    #[cfg(windows)]
    pub(crate) fn cli_machine_hub_held(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "held",
            Locale::ZhCn => "保持中",
        }
    }

    #[cfg(windows)]
    pub(crate) fn cli_machine_hub_expiring(self, seconds: u64) -> String {
        match self.locale {
            Locale::EnUs => format!("expiring, closes in {seconds} s"),
            Locale::ZhCn => format!("将在 {seconds} 秒后关闭"),
        }
    }

    #[cfg(windows)]
    pub(crate) fn cli_machine_hub_closing(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "closing",
            Locale::ZhCn => "正在关闭",
        }
    }

    /// The printer's heading lines: the hub's state, its process id, and the catalog revision.
    #[cfg(windows)]
    pub(crate) fn cli_machine_hub_summary(self, state: &str, pid: u32, revision: u64) -> String {
        match self.locale {
            Locale::EnUs => format!(
                "Machine hub: {state}
Process ID: {pid}
Catalog revision: {revision}
"
            ),
            Locale::ZhCn => format!(
                "连接进程：{state}
进程 ID：{pid}
机器目录修订号：{revision}
"
            ),
        }
    }

    /// A Machine's state as the hub reports it before it can connect; `None` when none arrived.
    #[cfg(windows)]
    pub(crate) fn cli_machine_hub_machine_state(
        self,
        state: Option<&orchbee_machine_protocol::hub::MachineState>,
    ) -> &'static str {
        use orchbee_machine_protocol::hub::MachineState as S;
        match (self.locale, state) {
            (Locale::EnUs, Some(S::Offline { .. })) => "offline",
            (Locale::ZhCn, Some(S::Offline { .. })) => "离线",
            (_, Some(S::Disabled)) => self.cli_machine_message("disabled"),
            (_, Some(S::Unsupported { .. })) => self.cli_machine_message("unsupported"),
            (Locale::EnUs, _) => "unknown",
            (Locale::ZhCn, _) => "未知",
        }
    }
}
