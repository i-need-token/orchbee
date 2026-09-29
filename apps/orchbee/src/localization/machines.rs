use super::Locale;
impl Locale {
    pub(crate) fn machine_picker_text(self, key: &str) -> &'static str {
        match (self, key) {
            (Self::EnUs, "local") => "Local",
            (Self::ZhCn, "local") => "本地",
            (Self::EnUs, "title") => " Machines ",
            (Self::ZhCn, "title") => " 机器 ",
            (Self::EnUs, "disabled") => "Disabled",
            (Self::ZhCn, "disabled") => "已禁用",
            (Self::EnUs, "current") => "Current",
            (Self::ZhCn, "current") => "当前",
            (Self::EnUs, "hint") => "↑/↓ Choose · Enter/Click Connect · Esc Close",
            (Self::ZhCn, "hint") => "↑/↓ 选择 · Enter/点击连接 · Esc 关闭",
            (Self::EnUs, "connecting") => "Connecting… Esc cancels; current session is retained.",
            (Self::ZhCn, "connecting") => "正在连接… Esc 取消，当前会话仍保留。",
            (Self::EnUs, "catalog") => "Machine catalog unavailable. Local remains available.",
            (Self::ZhCn, "catalog") => "机器目录不可用，仍可选择本地。",
            (Self::EnUs, "failed") => {
                "Connection failed. Check SSH readiness, then choose to retry."
            }
            (Self::ZhCn, "failed") => "连接失败。请检查 SSH 就绪状态，然后重新选择以重试。",
            (Self::EnUs, "removed") => {
                "Machine removed or disabled. Choose Local or another machine."
            }
            (Self::ZhCn, "removed") => "机器已删除或禁用，请选择本地或其他机器。",
            _ => "",
        }
    }

    pub(crate) fn cli_machine_route_failure(self, unknown: bool) -> &'static str {
        match (self, unknown) {
            (Self::EnUs, false) => "Remote command was not submitted.",
            (Self::ZhCn, false) => "远端命令未提交。",
            (Self::EnUs, true) => {
                "Remote command outcome is unknown. Check remote state before retrying."
            }
            (Self::ZhCn, true) => "远端命令结果未知，请检查远端状态后再决定是否重试。",
        }
    }

    pub(crate) fn cli_machine_help(self, verb: Option<&str>) -> &'static str {
        match (self, verb) {
            (Self::EnUs, None) => {
                "Usage: orchbee machine <list|add|rename|remove|enable|disable>\n\nManage saved Windows SSH machines. Add verifies an installed remote Client, starts the explicit Site, and verifies readiness before saving. SSH authentication and host trust must already work in batch mode. Removing or disabling a profile leaves remote sessions running. Use orchbee --machine <label-or-id> <command> for remote commands. Help is local; duplicate labels and disabled profiles are refused. Terminal --file/--stdin are read locally; paths such as --cwd belong to the remote machine. TUI and terminal streams are not routed. In the Windows TUI, use Ctrl+B then Shift+M or click the machine indicator to connect and switch. The catalog refreshes while open; connection requires selection.\n"
            }
            (Self::ZhCn, None) => {
                "用法：orchbee machine <list|add|rename|remove|enable|disable>\n\n管理已保存的 Windows SSH 机器。Add 检查已安装的远端 Client，启动指定 Site，并在保存前确认就绪。SSH 认证和主机信任须已能以批处理模式使用。删除或禁用配置不会停止远端会话。使用 orchbee --machine <标签或ID> <命令> 执行远端命令。帮助本地显示；拒绝重名标签及已禁用配置。终端 --file/--stdin 在本地读取，--cwd 等路径属于远端。不路由 TUI 和交互式终端流。在 Windows TUI 中按 Ctrl+B 后 Shift+M，或点击机器指示器连接和切换。机器目录实时刷新，选中后才连接。\n"
            }
            (Self::EnUs, Some("list")) => {
                "Usage: orchbee machine list [--json | --format text|json]\n\nRead the saved machine catalog without contacting SSH or creating directories. Enabled is a saved preference, not a live connection observation.\n"
            }
            (Self::ZhCn, Some("list")) => {
                "用法：orchbee machine list [--json | --format text|json]\n\n读取已保存的机器目录，不连接 SSH 或创建目录。启用状态是已保存的偏好，不代表实时连接。\n"
            }
            (Self::EnUs, Some("add")) => {
                "Usage: orchbee machine add <ssh-target> --label <label> [--remote-session <name>] [--format text|json]\n\nWindows remote with an installed compatible Orchbee cohort is required. Session defaults to default. Preparation has a 60-second total deadline. Failed preparation does not save a profile; an already started remote Site is retained. Automatic remote installation and interactive authentication are not yet supported.\n"
            }
            (Self::ZhCn, Some("add")) => {
                "用法：orchbee machine add <ssh-target> --label <label> [--remote-session <name>] [--format text|json]\n\n远端须为 Windows 且已安装兼容的 Orchbee cohort。会话默认 default。准备过程总期限为 60 秒。准备失败不保存配置；已启动的远端 Site 会保留。暂不支持自动安装远端程序及交互认证。\n"
            }
            (Self::EnUs, Some("rename")) => {
                "Usage: orchbee machine rename <profile-id> --label <label> [--format text|json]\n\nRename the exact saved profile. Labels cannot be blank or contain control characters and are limited to 128 UTF-8 bytes.\n"
            }
            (Self::ZhCn, Some("rename")) => {
                "用法：orchbee machine rename <profile-id> --label <label> [--format text|json]\n\n重命名精确 ID 对应的配置。标签不能为空白或含控制字符，最多 128 UTF-8 字节。\n"
            }
            (Self::EnUs, Some("remove")) => {
                "Usage: orchbee machine remove <profile-id> [--format text|json]\n\nRemove only the saved profile. Remote sessions and OpenSSH data remain owned by their existing users.\n"
            }
            (Self::ZhCn, Some("remove")) => {
                "用法：orchbee machine remove <profile-id> [--format text|json]\n\n仅删除已保存的配置。远端会话和 OpenSSH 数据仍由原有用户管理。\n"
            }
            (Self::EnUs, Some("enable")) => {
                "Usage: orchbee machine enable <profile-id> [--format text|json]\n\nEnable the saved profile. This command does not establish a connection or start a remote Site.\n"
            }
            (Self::ZhCn, Some("enable")) => {
                "用法：orchbee machine enable <profile-id> [--format text|json]\n\n启用已保存的配置。本命令不会建立连接或启动远端 Site。\n"
            }
            (Self::EnUs, Some("disable")) => {
                "Usage: orchbee machine disable <profile-id> [--format text|json]\n\nDisable the saved profile without stopping its remote Site.\n"
            }
            (Self::ZhCn, Some("disable")) => {
                "用法：orchbee machine disable <profile-id> [--format text|json]\n\n禁用已保存的配置，不停止远端 Site。\n"
            }
            _ => unreachable!("machine help vocabulary"),
        }
    }
    pub(crate) fn cli_machine_message(self, code: &str) -> &'static str {
        match (self, code) {
            (Self::EnUs, "empty") => "No saved SSH machines.",
            (Self::ZhCn, "empty") => "尚无已保存的 SSH 机器。",
            (Self::EnUs, "enabled") => "enabled",
            (Self::ZhCn, "enabled") => "已启用",
            (Self::EnUs, "disabled") => "disabled",
            (Self::ZhCn, "disabled") => "已禁用",
            (Self::EnUs, "saved") => "Saved machine catalog change:",
            (Self::ZhCn, "saved") => "已保存机器目录变更：",
            (Self::EnUs, "unchanged") => "Machine profile unchanged:",
            (Self::ZhCn, "unchanged") => "机器配置未变化：",
            (Self::EnUs, "already-committed") => {
                "The catalog change was committed before this interruption or failure."
            }
            (Self::ZhCn, "already-committed") => "目录变更已在本次中断或失败前提交。",
            (Self::EnUs, "remote-retained") => {
                "The remote Site may be running; this command does not roll it back."
            }
            (Self::ZhCn, "remote-retained") => "远端 Site 可能仍在运行；本命令不会回滚它。",
            (Self::EnUs, "configuration") => "The Windows configuration location is unavailable.",
            (Self::ZhCn, "configuration") => "Windows 配置位置不可用。",
            (Self::EnUs, "invalid-catalog") => {
                "The machine catalog is invalid; existing data was preserved."
            }
            (Self::ZhCn, "invalid-catalog") => "机器目录无效；已有数据保持不变。",
            (Self::EnUs, "storage") => "The private machine catalog could not be read or saved.",
            (Self::ZhCn, "storage") => "无法读取或保存私有机器目录。",
            (Self::EnUs, "busy") => {
                "Another machine catalog writer is active. Retry after it finishes."
            }
            (Self::ZhCn, "busy") => "另一个机器目录写入者正在运行，请在其结束后重试。",
            (Self::EnUs, "not-found") => {
                "The exact machine profile ID was not found. Run 'orchbee machine list'."
            }
            (Self::ZhCn, "not-found") => "未找到精确机器配置 ID，请运行 ‘orchbee machine list’。",
            (Self::EnUs, "capacity") => {
                "The machine catalog has reached its capacity of 64 profiles."
            }
            (Self::ZhCn, "capacity") => "机器目录已达到 64 个配置的容量上限。",
            (Self::EnUs, "revision-exhausted") => "The machine catalog revision cannot advance.",
            (Self::ZhCn, "revision-exhausted") => "机器目录修订号无法继续增加。",
            (Self::EnUs, "ssh-unavailable") => {
                "Windows OpenSSH Client is unavailable in the system directory."
            }
            (Self::ZhCn, "ssh-unavailable") => "系统目录中的 Windows OpenSSH Client 不可用。",
            (Self::EnUs, "ssh-failed") => {
                "SSH preparation failed. Check OpenSSH batch authentication, host trust, and the installed remote orchbee.exe."
            }
            (Self::ZhCn, "ssh-failed") => {
                "SSH 准备失败，请检查 OpenSSH 批处理认证、主机信任及已安装的远端 orchbee.exe。"
            }
            (Self::EnUs, "remote-protocol") => {
                "The remote Client returned an invalid or unsupported status document."
            }
            (Self::ZhCn, "remote-protocol") => "远端 Client 返回了无效或不支持的状态文档。",
            (Self::EnUs, "remote-incompatible") => {
                "The remote Windows Orchbee version, protocols, or cohort are incompatible. Install the matching cohort first."
            }
            (Self::ZhCn, "remote-incompatible") => {
                "远端 Windows Orchbee 版本、协议或 cohort 不兼容，请先安装匹配的 cohort。"
            }
            (Self::EnUs, "remote-not-ready") => {
                "The remote Site could not be confirmed ready through a new SSH connection."
            }
            (Self::ZhCn, "remote-not-ready") => "无法通过新的 SSH 连接确认远端 Site 已就绪。",
            (Self::EnUs, "cancelled") => "Machine operation interrupted.",
            (Self::ZhCn, "cancelled") => "机器操作已中断。",
            (Self::EnUs, "timeout") => "The machine operation deadline expired.",
            (Self::ZhCn, "timeout") => "机器操作已超时。",
            (Self::EnUs, "physical-unknown") => {
                "SSH process cleanup could not be confirmed. Remote state may have changed."
            }
            (Self::ZhCn, "physical-unknown") => "无法确认 SSH 进程清理完成，远端状态可能已经变化。",
            (Self::EnUs, _) => "Machine operation failed before completion.",
            (Self::ZhCn, _) => "机器操作未能完成。",
        }
    }
}
