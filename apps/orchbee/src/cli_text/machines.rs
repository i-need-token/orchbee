//! Saved-machine CLI help and result messages; native picker text stays with its surface. Raw
//! OpenSSH diagnostics never reach this copy; they only classify a failure, except the prompt of a
//! question `machine connect` shows verbatim, escaped and bounded, because only `ssh` knows what
//! it asks and who asks it.
use super::Locale;

/// What a login question `machine connect` puts on the console asks; every interface words it
/// from the same sentences.
#[cfg(windows)]
pub(crate) use crate::localization::MachineQuestion;
#[cfg(windows)]
use orchbee_workbench_client::application::machine_question::shown_lines;
#[cfg(all(test, windows))]
use orchbee_workbench_client::application::machine_question::{
    PROMPT_LINE_CHARACTERS, PROMPT_LINES,
};

#[cfg(all(test, windows))]
mod tests;

impl super::CliText {
    /// What `machine connect` writes on standard error before it reads an answer for the machine
    /// whose `ssh` target is `target`: each line of `ssh`'s own `prompt`, escaped and bounded, then
    /// one line of Orchbee's wording for the kind; `repeated` says `ssh` refused the previous
    /// answer of this kind.
    #[cfg(windows)]
    pub(crate) fn cli_machine_question(
        self,
        target: &str,
        question: &MachineQuestion<'_>,
        prompt: &str,
        repeated: bool,
    ) -> String {
        let said = self.locale.machine_question_said();
        let mut shown = String::new();
        for line in shown_lines(prompt) {
            shown.push_str(said);
            shown.push_str(&line);
            shown.push('\n');
        }
        let asked = self
            .locale
            .machine_question_asked(target, question, repeated);
        format!("{shown}{asked}")
    }

    /// The line `machine connect` writes when an answer was not delivered: `again` when the answer
    /// held a character `ssh` cannot take and the question is asked again; otherwise its endpoint
    /// was gone or refused it and the login stops.
    #[cfg(windows)]
    pub(crate) fn cli_machine_answer_undelivered(self, again: bool) -> String {
        format!("{}\n", self.locale.machine_answer_undelivered(again))
    }

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

    /// The one line of `--machine <selector> server stop` for the hub's `outcome` on the Machine
    /// `label`; `interrupted` says that an interrupt ended the wait, not the stop.
    pub(crate) fn cli_machine_stop_result(
        self,
        label: &str,
        outcome: &str,
        interrupted: bool,
    ) -> String {
        let label = super::escape::quoted(label);
        let state = match (self.locale, outcome) {
            (Locale::EnUs, "stopped") => "stopped",
            (Locale::ZhCn, "stopped") => "已停止",
            (Locale::EnUs, "not-running") => "no Session was running",
            (Locale::ZhCn, "not-running") => "会话未在运行",
            (Locale::EnUs, "not-online") => "the machine is not online",
            (Locale::ZhCn, "not-online") => "机器未上线",
            (Locale::EnUs, "not-found") => "no such saved machine",
            (Locale::ZhCn, "not-found") => "没有这台已保存的机器",
            (Locale::EnUs, "failed") => "failed",
            (Locale::ZhCn, "failed") => "失败",
            (Locale::EnUs, "stopping") => "the background machine hub is closing; try again",
            (Locale::ZhCn, "stopping") => "后台连接进程正在关闭，请重试",
            (Locale::EnUs, _) => "outcome unknown",
            (Locale::ZhCn, _) => "结果未知",
        };
        let waiting = match (self.locale, interrupted) {
            (_, false) => "",
            (Locale::EnUs, true) => " (wait cancelled; the stop may go on)",
            (Locale::ZhCn, true) => "（已取消等待，停止可能仍在进行）",
        };
        match self.locale {
            Locale::EnUs => format!("Session stop on machine {label}: {state}{waiting}\n"),
            Locale::ZhCn => format!("机器 {label} 的会话停止：{state}{waiting}\n"),
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
                "读取已保存的机器目录，不连接任何机器，也不创建目录。启用状态是已保存的偏好，不代表实时连接。更新版本保存的、使用此版本不认识的连接方式的机器显示为不支持。",
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
                "Enable the saved machine. The command itself connects nothing; a running background machine hub that keeps machines connected then connects it, which may start its remote session.",
                "启用已保存的机器。本命令本身不连接机器；正在运行且保持机器连接的后台连接进程随后会连接它，这可能启动其远端会话。",
            ),
            C::MachineDisable => (
                "orchbee machine disable <id|label> [--format text|json]",
                "Disable the saved machine without stopping its remote session.",
                "停用已保存的机器，不停止远端会话。",
            ),
            C::MachineConnect => (
                "orchbee machine connect <id|label> [--remember] [--format text|json]",
                "Connect an enabled saved machine through the background machine hub, starting the hub when none runs. Returns once the machine is online or with the reason it is not. When standard input and standard error are both consoles, an Orchbee Panel included, it shows each sign-in question OpenSSH asks, such as a password or whether to trust a new host, with OpenSSH's own words, and waits for an answer; an empty answer or Ctrl+C ends the attempt. Otherwise, as with standard input redirected from the null device, which an Agent that cannot answer uses, a sign-in that needs an answer ends the command. The connection stays after the command ends. '--remember' is not available yet.",
                "通过后台连接进程连接一台已启用的已保存机器，没有连接进程时会启动它。机器上线后返回，或返回未能上线的原因。标准输入和标准错误都是控制台时（包括 Orchbee 面板），它会连同 OpenSSH 的原文显示 OpenSSH 提出的每个登录问题，例如密码或是否信任新主机，并等待回答；空回答或 Ctrl+C 结束本次尝试。否则（例如标准输入重定向自空设备，无法回答的 Agent 应这样做），需要回答的登录会结束命令。命令结束后连接仍保持。‘--remember’ 暂不可用。",
            ),
            C::MachineDisconnect => (
                "orchbee machine disconnect <id|label> [--format text|json]",
                "End the connection to a saved machine and stop retrying it. The remote Session keeps running. Starts no machine hub.",
                "断开与一台已保存机器的连接并停止重试。远端会话继续运行。不会启动连接进程。",
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
            (Locale::ZhCn, "disabled") => "已停用",
            (Locale::EnUs, "unsupported") => "unsupported",
            (Locale::ZhCn, "unsupported") => "不支持",
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
            (Locale::EnUs, "attempt-ended") => {
                "The connection attempt was ended by a disconnect of this machine or by a change to its saved target or Session."
            }
            (Locale::ZhCn, "attempt-ended") => {
                "本次连接尝试已结束：这台机器被断开，或其已保存的目标或会话被更改。"
            }
            (Locale::EnUs, "disabled-machine") => {
                "This machine is disabled. Run 'orchbee machine enable' first."
            }
            (Locale::ZhCn, "disabled-machine") => {
                "这台机器已停用，请先运行 ‘orchbee machine enable’。"
            }
            (Locale::EnUs, "hub-unavailable") => {
                "The background machine hub could not be reached or started. Try again."
            }
            (Locale::ZhCn, "hub-unavailable") => "无法连接或启动后台连接进程，请重试。",
            (Locale::EnUs, "host-unreachable") => {
                "The machine could not be reached: its name did not resolve, it refused, or it did not answer in time."
            }
            (Locale::ZhCn, "host-unreachable") => {
                "无法连接这台机器：名称无法解析、连接被拒绝或未及时响应。"
            }
            (Locale::EnUs, "auth-required") => {
                "Signing in to this machine needs an answer, such as a password, and none was given. Run 'orchbee machine connect' in a terminal to answer."
            }
            (Locale::ZhCn, "auth-required") => {
                "登录这台机器需要回答问题，例如密码，但没有得到回答。请在终端中运行 ‘orchbee machine connect’ 来回答。"
            }
            (Locale::EnUs, "auth-failed") => {
                "The machine refused the sign-in: the SSH key, or the answers given."
            }
            (Locale::ZhCn, "auth-failed") => "这台机器拒绝了登录：SSH 密钥或所给的回答有误。",
            (Locale::EnUs, "host-untrusted") => {
                "The machine's host key is not trusted yet. Run 'orchbee machine connect' in a terminal to check and trust it."
            }
            (Locale::ZhCn, "host-untrusted") => {
                "这台机器的主机密钥尚未被信任。请在终端中运行 ‘orchbee machine connect’ 核对并信任。"
            }
            (Locale::EnUs, "host-key-changed") => {
                "The machine's host key differs from the trusted one. Check the machine before you trust the new key."
            }
            (Locale::ZhCn, "host-key-changed") => {
                "这台机器的主机密钥与已信任的不同。请先核实这台机器，再信任新密钥。"
            }
            (Locale::EnUs, "version-incompatible") => {
                "This Orchbee and the other side cannot work together; update the older one."
            }
            (Locale::ZhCn, "version-incompatible") => {
                "本机 Orchbee 与另一端无法协同工作，请更新较旧的一方。"
            }
            (Locale::EnUs, "remote-install-failed") => "Installing Orchbee on the machine failed.",
            (Locale::ZhCn, "remote-install-failed") => "在这台机器上安装 Orchbee 失败。",
            (Locale::EnUs, "connected") => "Machine online:",
            (Locale::ZhCn, "connected") => "机器已上线：",
            (Locale::EnUs, "disconnected") => "Machine disconnected:",
            (Locale::ZhCn, "disconnected") => "机器已断开：",
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
        failure: &crate::cli::machines::hub::StateFailure,
    ) -> String {
        use crate::cli::machines::hub::StateFailure as F;
        use orchbee_workbench_client::application::machine_hub::Incompatible as I;
        match (self.locale, failure) {
            (Locale::EnUs, F::Incompatible(I::Environment { environment })) => format!(
                "The machine hub of this data folder belongs to another Orchbee build ({environment})."
            ),
            (Locale::ZhCn, F::Incompatible(I::Environment { environment })) => {
                format!("此数据文件夹的连接进程属于另一个 Orchbee 构建（{environment}）。")
            }
            (Locale::EnUs, F::Incompatible(I::Held { version })) => format!(
                "The machine hub of version {version} may still be in use by an Orchbee window or a Session, so it was not replaced. Close them and try again."
            ),
            (Locale::ZhCn, F::Incompatible(I::Held { version })) => format!(
                "版本 {version} 的连接进程可能仍被 Orchbee 窗口或会话使用，因此未被替换。请关闭它们后重试。"
            ),
            (locale, failure) => self
                .cli_machine_hub_fixed_failure(locale, failure)
                .to_owned(),
        }
    }

    #[cfg(windows)]
    fn cli_machine_hub_fixed_failure(
        self,
        locale: Locale,
        failure: &crate::cli::machines::hub::StateFailure,
    ) -> &'static str {
        use crate::cli::machines::hub::StateFailure as F;
        match (locale, failure) {
            (Locale::EnUs, F::DataRoot) => {
                "The Orchbee data folder is unavailable; check USERPROFILE or ORCHBEE_HOME."
            }
            (Locale::ZhCn, F::DataRoot) => {
                "Orchbee 数据文件夹不可用；请检查 USERPROFILE 或 ORCHBEE_HOME。"
            }
            (Locale::EnUs, F::Unavailable) => "The machine hub could not be reached or started.",
            (Locale::ZhCn, F::Unavailable) => "无法连接或启动连接进程。",
            (Locale::EnUs, F::Denied) => {
                "Access to the machine hub was denied; it may belong to another user or an elevated process."
            }
            (Locale::ZhCn, F::Denied) => {
                "访问连接进程被拒绝；它可能属于其他用户或以管理员身份运行的进程。"
            }
            (Locale::EnUs, F::Closing) => "The machine hub is closing. Try again.",
            (Locale::ZhCn, F::Closing) => "连接进程正在关闭，请重试。",
            (Locale::EnUs, F::Lost) => "The connection to the machine hub was lost.",
            (Locale::ZhCn, F::Lost) => "与连接进程的连接已断开。",
            (Locale::EnUs, F::Incompatible(_)) => {
                "The running machine hub does not understand this version of Orchbee."
            }
            (Locale::ZhCn, F::Incompatible(_)) => "正在运行的连接进程无法识别此版本的 Orchbee。",
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

    /// An `expiring` hub past its `expiresAt`, which closes once no routed command runs.
    #[cfg(windows)]
    pub(crate) fn cli_machine_hub_overdue(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "expired, closes once no remote command runs",
            Locale::ZhCn => "已到期，远端命令全部结束后关闭",
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

    /// A Machine's state as the hub reports it; `None` when none arrived.
    #[cfg(windows)]
    pub(crate) fn cli_machine_hub_machine_state(
        self,
        state: Option<&orchbee_machine_protocol::hub::MachineState>,
    ) -> &'static str {
        use orchbee_machine_protocol::hub::{MachineState as S, MachineStateName as N};
        let name = match state {
            Some(S::Disabled) => return self.cli_machine_message("disabled"),
            Some(S::Unsupported { .. }) => return self.cli_machine_message("unsupported"),
            Some(state) => state.name(),
            None => N::Unknown,
        };
        match (self.locale, name) {
            (Locale::EnUs, N::Connecting) => "connecting",
            (Locale::ZhCn, N::Connecting) => "连接中",
            (Locale::EnUs, N::Online) => "online",
            (Locale::ZhCn, N::Online) => "在线",
            (Locale::EnUs, N::Reconnecting) => "reconnecting",
            (Locale::ZhCn, N::Reconnecting) => "重连中",
            (Locale::EnUs, N::Offline) => "offline",
            (Locale::ZhCn, N::Offline) => "离线",
            (Locale::EnUs, N::Unresponsive) => "unresponsive",
            (Locale::ZhCn, N::Unresponsive) => "无响应",
            (Locale::EnUs, N::NeedsLogin) => "needs login",
            (Locale::ZhCn, N::NeedsLogin) => "需要登录",
            (Locale::EnUs, N::LoginFailed) => "login failed",
            (Locale::ZhCn, N::LoginFailed) => "登录失败",
            (Locale::EnUs, N::Untrusted) => "untrusted",
            (Locale::ZhCn, N::Untrusted) => "未信任",
            (Locale::EnUs, N::KeyChanged) => "key changed",
            (Locale::ZhCn, N::KeyChanged) => "密钥已变",
            (Locale::EnUs, N::NotInstalled) => "not installed",
            (Locale::ZhCn, N::NotInstalled) => "未安装",
            (Locale::EnUs, N::NeedsUpdate) => "needs update",
            (Locale::ZhCn, N::NeedsUpdate) => "需更新",
            (Locale::EnUs, N::LocalNeedsUpdate) => "this computer needs update",
            (Locale::ZhCn, N::LocalNeedsUpdate) => "本机需更新",
            (Locale::EnUs, N::Incompatible) => "incompatible",
            (Locale::ZhCn, N::Incompatible) => "不兼容",
            (Locale::EnUs, _) => "unknown",
            (Locale::ZhCn, _) => "未知",
        }
    }
}
