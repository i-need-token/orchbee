//! Saved-machine CLI help and result messages; native picker text stays with its surface.
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
                "Read the saved machine catalog without contacting SSH or creating directories. Enabled is a saved preference, not a live connection observation.",
                "读取已保存的机器目录，不连接 SSH 或创建目录。启用状态是已保存的偏好，不代表实时连接。",
            ),
            C::MachineAdd => (
                "orchbee machine add <ssh-target> --label <label> [--remote-session <name>] [--format text|json]",
                "Windows remote with an installed compatible Orchbee cohort is required. Session defaults to default. Preparation has a 60-second total deadline. Failed preparation does not save a profile; an already started remote session is retained. Automatic remote installation and interactive authentication are not yet supported.",
                "远端须为 Windows 且已安装兼容的 Orchbee cohort。会话默认 default。准备过程总期限为 60 秒。准备失败不保存配置；已启动的远端会话会保留。暂不支持自动安装远端程序及交互认证。",
            ),
            C::MachineRename => (
                "orchbee machine rename <profile-id> --label <label> [--format text|json]",
                "Rename the exact saved profile. Labels cannot be blank or contain control characters and are limited to 128 UTF-8 bytes.",
                "重命名精确 ID 对应的配置。标签不能为空白或含控制字符，最多 128 UTF-8 字节。",
            ),
            C::MachineRemove => (
                "orchbee machine remove <profile-id> [--format text|json]",
                "Remove only the saved profile. Remote sessions and OpenSSH data remain owned by their existing users.",
                "仅删除已保存的配置。远端会话和 OpenSSH 数据仍由原有用户管理。",
            ),
            C::MachineEnable => (
                "orchbee machine enable <profile-id> [--format text|json]",
                "Enable the saved profile. This command does not establish a connection or start a remote session.",
                "启用已保存的配置。本命令不会建立连接或启动远端会话。",
            ),
            C::MachineDisable => (
                "orchbee machine disable <profile-id> [--format text|json]",
                "Disable the saved profile without stopping its remote session.",
                "禁用已保存的配置，不停止远端会话。",
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
            (Locale::EnUs, "empty") => "No saved SSH machines.",
            (Locale::ZhCn, "empty") => "尚无已保存的 SSH 机器。",
            (Locale::EnUs, "enabled") => "enabled",
            (Locale::ZhCn, "enabled") => "已启用",
            (Locale::EnUs, "disabled") => "disabled",
            (Locale::ZhCn, "disabled") => "已禁用",
            (Locale::EnUs, "saved") => "Saved machine catalog change:",
            (Locale::ZhCn, "saved") => "已保存机器目录变更：",
            (Locale::EnUs, "unchanged") => "Machine profile unchanged:",
            (Locale::ZhCn, "unchanged") => "机器配置未变化：",
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
                "The exact machine profile ID was not found. Run 'orchbee machine list'."
            }
            (Locale::ZhCn, "not-found") => "未找到精确机器配置 ID，请运行 ‘orchbee machine list’。",
            (Locale::EnUs, "capacity") => {
                "The machine catalog has reached its capacity of 64 profiles."
            }
            (Locale::ZhCn, "capacity") => "机器目录已达到 64 个配置的容量上限。",
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
            (Locale::EnUs, "remote-incompatible") => {
                "The remote Windows Orchbee version, protocols, or cohort are incompatible. Install the matching cohort first."
            }
            (Locale::ZhCn, "remote-incompatible") => {
                "远端 Windows Orchbee 版本、协议或 cohort 不兼容，请先安装匹配的 cohort。"
            }
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
}
