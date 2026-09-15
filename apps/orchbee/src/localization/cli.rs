//! Localized strings for the read-only CLI query surface. Machine envelope
//! fields stay English; these strings are the human stderr/text-render layer.

use super::Locale;
use crate::cli::Failure;
use crate::invocation::HelpTopic;

impl Locale {
    /// Localized human diagnostic for one CLI failure. User-controlled data is
    /// escaped before it reaches the terminal.
    pub(crate) fn cli_failure(self, failure: &Failure) -> String {
        use crate::cli::FailureKind;
        match failure.kind {
            FailureKind::InvalidUsage => self.usage_failure().to_owned(),
            FailureKind::InvalidIdentifier => {
                let resource = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("resource"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("project");
                let target = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("target"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                self.cli_invalid_identifier(resource, target)
            }
            FailureKind::NotFound => {
                let (resource, target) = failure_target(failure);
                self.cli_not_found(resource, &target)
            }
            FailureKind::Ambiguous => {
                let (resource, target) = failure_target(failure);
                self.cli_ambiguous(resource, &target)
            }
            FailureKind::Denied => {
                let command = failure
                    .source
                    .as_ref()
                    .and_then(|source| source.get("kind"))
                    .and_then(|value| value.as_str())
                    == Some("workbench-command");
                match (self, command) {
                    (Self::EnUs, true) => {
                        "The local Site denied the command; nothing was applied.\n".to_owned()
                    }
                    (Self::EnUs, false) => {
                        "The local Site denied the read request.\n".to_owned()
                    }
                    (Self::ZhCn, true) => {
                        "本地 Site 拒绝了该命令；未应用任何修改。\n".to_owned()
                    }
                    (Self::ZhCn, false) => "本地 Site 拒绝了读取请求。\n".to_owned(),
                }
            }
            FailureKind::Rejected => {
                let reason = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("reason"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                match self {
                    Self::EnUs => {
                        format!("The local Site rejected the command ({reason}); nothing was applied.\n")
                    }
                    Self::ZhCn => {
                        format!("本地 Site 拒绝了该命令（{reason}）；未应用任何修改。\n")
                    }
                }
            }
            FailureKind::OutcomeUnknown => match self {
                Self::EnUs => {
                    "The command's outcome is unknown: the request may or may not have been applied.\nIt was not retried — inspect the Workbench state before deciding whether to repeat it.\n".to_owned()
                }
                Self::ZhCn => {
                    "命令结果未知：请求可能已应用，也可能未应用。\n该命令未被重试——请先检查 Workbench 状态，再决定是否重复执行。\n".to_owned()
                }
            },
            FailureKind::ServiceUnavailable => self.cli_service_unavailable(failure),
            FailureKind::SessionCapacity => match self {
                Self::EnUs => {
                    "The local Site has no Client Session capacity for this query.\n".to_owned()
                }
                Self::ZhCn => "本地 Site 没有可接纳本次查询的 Client Session 容量。\n".to_owned(),
            },
            FailureKind::TransportLoss => {
                let stage = failure
                    .details
                    .as_ref()
                    .and_then(|details| details.get("stage"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                match (self, stage) {
                    (Self::EnUs, "command") => {
                        "The connection to the local Site failed before the command could be submitted; nothing was sent.\n".to_owned()
                    }
                    (Self::ZhCn, "command") => {
                        "与本地 Site 的连接在命令提交前失败；未发送任何内容。\n".to_owned()
                    }
                    (Self::EnUs, "viewer") => {
                        "The connection to the local Site was lost during the terminal read.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, "viewer") => "与本地 Site 的连接在终端读取期间中断。\n".to_owned(),
                    (Self::EnUs, _) => {
                        "The connection to the local Site was lost before a snapshot arrived.\n"
                            .to_owned()
                    }
                    (Self::ZhCn, _) => "与本地 Site 的连接在快照返回前中断。\n".to_owned(),
                }
            }
            FailureKind::ProtocolInvalid => match self {
                Self::EnUs => {
                    "The local Site answered with an invalid protocol response.\n".to_owned()
                }
                Self::ZhCn => "本地 Site 返回了无效的协议响应。\n".to_owned(),
            },
            FailureKind::Incompatible => match self {
                Self::EnUs => {
                    "The running local Site is incompatible with this development cohort.\n"
                        .to_owned()
                }
                Self::ZhCn => "正在运行的本地 Site 与当前开发 cohort 不兼容。\n".to_owned(),
            },
            FailureKind::CheckFailed => self.cli_check_failed(failure),
            FailureKind::OutputTooLarge => match self {
                Self::EnUs => {
                    "The result exceeded the 8 MiB output bound; nothing was written.\n".to_owned()
                }
                Self::ZhCn => "结果超过 8 MiB 输出上限；未写出任何内容。\n".to_owned(),
            },
            FailureKind::InternalFailure => match self {
                Self::EnUs => "The CLI hit an internal failure; no data was written.\n".to_owned(),
                Self::ZhCn => "CLI 发生内部错误；未写出任何数据。\n".to_owned(),
            },
        }
    }

    fn cli_invalid_identifier(self, resource: &str, target: &str) -> String {
        let escaped = crate::cli::escape::quoted(target);
        match self {
            Self::EnUs => {
                format!("Invalid {resource} identifier {escaped}.\n")
            }
            Self::ZhCn => format!("无效的 {resource} 标识符 {escaped}。\n"),
        }
    }

    fn cli_not_found(self, resource: &str, target: &str) -> String {
        let escaped = crate::cli::escape::quoted(target);
        match self {
            Self::EnUs => format!("No {resource} matches {escaped}.\n"),
            Self::ZhCn => format!("没有与 {escaped} 匹配的 {resource}。\n"),
        }
    }

    fn cli_ambiguous(self, resource: &str, target: &str) -> String {
        let escaped = crate::cli::escape::quoted(target);
        match self {
            Self::EnUs => {
                format!("More than one {resource} matches {escaped}; use its exact ID.\n")
            }
            Self::ZhCn => format!("多个 {resource} 与 {escaped} 匹配；请使用精确 ID。\n"),
        }
    }

    fn cli_service_unavailable(self, failure: &Failure) -> String {
        let stage = failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        match (self, stage) {
            (Self::EnUs, "platform") => {
                "The native CLI query surface is supported on Linux only.\n".to_owned()
            }
            (Self::ZhCn, "platform") => "原生 CLI 查询面仅在 Linux 上受支持。\n".to_owned(),
            (Self::EnUs, "discovery") => {
                "No running local Site was found for this user.\n".to_owned()
            }
            (Self::ZhCn, "discovery") => "未找到当前用户正在运行的本地 Site。\n".to_owned(),
            (Self::EnUs, "snapshot") => {
                "The local Site could not produce a snapshot right now.\n".to_owned()
            }
            (Self::ZhCn, "snapshot") => "本地 Site 当前无法产出快照。\n".to_owned(),
            (Self::EnUs, "viewer") => {
                "The terminal could not be observed for reading right now.\n".to_owned()
            }
            (Self::ZhCn, "viewer") => "该终端当前无法进行读取观察。\n".to_owned(),
            (Self::EnUs, "command") => {
                "The local Site could not execute the command right now; nothing was applied.\n"
                    .to_owned()
            }
            (Self::ZhCn, "command") => {
                "本地 Site 当前无法执行该命令；未应用任何修改。\n".to_owned()
            }
            (Self::EnUs, _) => "The local Site could not admit this query connection.\n".to_owned(),
            (Self::ZhCn, _) => "本地 Site 无法接纳本次查询连接。\n".to_owned(),
        }
    }

    fn cli_check_failed(self, failure: &Failure) -> String {
        let stage = failure
            .details
            .as_ref()
            .and_then(|details| details.get("stage"))
            .and_then(|value| value.as_str())
            .unwrap_or("");
        match (self, stage) {
            (Self::EnUs, "cohort") => {
                "The native development cohort is missing, changed, or invalid.\n".to_owned()
            }
            (Self::ZhCn, "cohort") => "原生开发 cohort 缺失、已修改或无效。\n".to_owned(),
            (Self::EnUs, "platform") => "config check is supported on Linux only.\n".to_owned(),
            (Self::ZhCn, "platform") => "config check 仅在 Linux 上受支持。\n".to_owned(),
            (Self::EnUs, "socket") => "A local Site socket failed its safety checks.\n".to_owned(),
            (Self::ZhCn, "socket") => "本地 Site 套接字未通过安全检查。\n".to_owned(),
            (Self::EnUs, "runtime-directory") => {
                "The local Site runtime directory failed its safety checks.\n".to_owned()
            }
            (Self::ZhCn, "runtime-directory") => {
                "本地 Site 运行时目录未通过安全检查。\n".to_owned()
            }
            (Self::EnUs, _) => "The runtime base is missing, invalid, or unsafe.\n".to_owned(),
            (Self::ZhCn, _) => "运行时基座缺失、无效或不安全。\n".to_owned(),
        }
    }

    pub(crate) const fn cli_output_failed(self) -> &'static str {
        match self {
            Self::EnUs => "Writing the result to stdout failed.\n",
            Self::ZhCn => "向标准输出写入结果失败。\n",
        }
    }

    pub(crate) const fn cli_config_failed(self) -> &'static str {
        match self {
            Self::EnUs => "One or more configuration checks failed.\n",
            Self::ZhCn => "一项或多项配置检查失败。\n",
        }
    }

    /// The text-output marker appended when the read budget omitted frame rows.
    pub(crate) fn cli_read_truncated(self, omitted: usize) -> String {
        match self {
            Self::EnUs => format!("… {omitted} more rows truncated at the read limit\n"),
            Self::ZhCn => format!("… 因读取上限省略了 {omitted} 行\n"),
        }
    }
}

fn failure_target(failure: &Failure) -> (&'static str, String) {
    let resource = failure
        .details
        .as_ref()
        .and_then(|details| details.get("resource"))
        .and_then(|value| value.as_str())
        .unwrap_or("project");
    let resource = match resource {
        "workspace" => "workspace",
        "tab" => "tab",
        "panel" => "panel",
        "terminal" => "terminal",
        "schema" => "schema",
        _ => "project",
    };
    let target = failure
        .details
        .as_ref()
        .and_then(|details| details.get("target"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_owned();
    (resource, target)
}

// ---- help topics ---------------------------------------------------------

impl Locale {
    pub(crate) fn cli_help(self, topic: HelpTopic) -> &'static str {
        use crate::invocation::Family;
        match (self, topic) {
            (_, HelpTopic::Family(Family::Project)) => match self {
                Self::EnUs => HELP_PROJECT_EN,
                Self::ZhCn => HELP_PROJECT_ZH,
            },
            (_, HelpTopic::Family(Family::Workspace)) => match self {
                Self::EnUs => HELP_WORKSPACE_EN,
                Self::ZhCn => HELP_WORKSPACE_ZH,
            },
            (_, HelpTopic::Family(Family::Tab)) => match self {
                Self::EnUs => HELP_TAB_EN,
                Self::ZhCn => HELP_TAB_ZH,
            },
            (_, HelpTopic::Family(Family::Panel)) => match self {
                Self::EnUs => HELP_PANEL_EN,
                Self::ZhCn => HELP_PANEL_ZH,
            },
            (_, HelpTopic::Family(Family::Terminal)) => match self {
                Self::EnUs => HELP_TERMINAL_EN,
                Self::ZhCn => HELP_TERMINAL_ZH,
            },
            (_, HelpTopic::Family(Family::Api)) => match self {
                Self::EnUs => HELP_API_EN,
                Self::ZhCn => HELP_API_ZH,
            },
            (_, HelpTopic::Family(Family::Config)) => match self {
                Self::EnUs => HELP_CONFIG_EN,
                Self::ZhCn => HELP_CONFIG_ZH,
            },
            (_, HelpTopic::Family(Family::Completion)) => match self {
                Self::EnUs => HELP_COMPLETION_EN,
                Self::ZhCn => HELP_COMPLETION_ZH,
            },
            (_, HelpTopic::Command(path)) => self.cli_command_help(path),
        }
    }

    fn cli_command_help(self, path: crate::invocation::CommandPath) -> &'static str {
        use crate::invocation::CommandPath;
        match (self, path) {
            (Self::EnUs, CommandPath::Tui) => HELP_TUI_EN,
            (Self::ZhCn, CommandPath::Tui) => HELP_TUI_ZH,
            (Self::EnUs, CommandPath::Status) => HELP_STATUS_EN,
            (Self::ZhCn, CommandPath::Status) => HELP_STATUS_ZH,
            (Self::EnUs, CommandPath::ProjectList) => HELP_PROJECT_LIST_EN,
            (Self::ZhCn, CommandPath::ProjectList) => HELP_PROJECT_LIST_ZH,
            (Self::EnUs, CommandPath::ProjectGet) => HELP_PROJECT_GET_EN,
            (Self::ZhCn, CommandPath::ProjectGet) => HELP_PROJECT_GET_ZH,
            (Self::EnUs, CommandPath::WorkspaceList) => HELP_WORKSPACE_LIST_EN,
            (Self::ZhCn, CommandPath::WorkspaceList) => HELP_WORKSPACE_LIST_ZH,
            (Self::EnUs, CommandPath::WorkspaceGet) => HELP_WORKSPACE_GET_EN,
            (Self::ZhCn, CommandPath::WorkspaceGet) => HELP_WORKSPACE_GET_ZH,
            (Self::EnUs, CommandPath::WorkspaceCreate) => HELP_WORKSPACE_CREATE_EN,
            (Self::ZhCn, CommandPath::WorkspaceCreate) => HELP_WORKSPACE_CREATE_ZH,
            (Self::EnUs, CommandPath::WorkspaceRename) => HELP_WORKSPACE_RENAME_EN,
            (Self::ZhCn, CommandPath::WorkspaceRename) => HELP_WORKSPACE_RENAME_ZH,
            (Self::EnUs, CommandPath::TabList) => HELP_TAB_LIST_EN,
            (Self::ZhCn, CommandPath::TabList) => HELP_TAB_LIST_ZH,
            (Self::EnUs, CommandPath::TabGet) => HELP_TAB_GET_EN,
            (Self::ZhCn, CommandPath::TabGet) => HELP_TAB_GET_ZH,
            (Self::EnUs, CommandPath::TabCreate) => HELP_TAB_CREATE_EN,
            (Self::ZhCn, CommandPath::TabCreate) => HELP_TAB_CREATE_ZH,
            (Self::EnUs, CommandPath::TabRename) => HELP_TAB_RENAME_EN,
            (Self::ZhCn, CommandPath::TabRename) => HELP_TAB_RENAME_ZH,
            (Self::EnUs, CommandPath::TabMove) => HELP_TAB_MOVE_EN,
            (Self::ZhCn, CommandPath::TabMove) => HELP_TAB_MOVE_ZH,
            (Self::EnUs, CommandPath::TabClose) => HELP_TAB_CLOSE_EN,
            (Self::ZhCn, CommandPath::TabClose) => HELP_TAB_CLOSE_ZH,
            (Self::EnUs, CommandPath::PanelList) => HELP_PANEL_LIST_EN,
            (Self::ZhCn, CommandPath::PanelList) => HELP_PANEL_LIST_ZH,
            (Self::EnUs, CommandPath::PanelGet) => HELP_PANEL_GET_EN,
            (Self::ZhCn, CommandPath::PanelGet) => HELP_PANEL_GET_ZH,
            (Self::EnUs, CommandPath::PanelSplit) => HELP_PANEL_SPLIT_EN,
            (Self::ZhCn, CommandPath::PanelSplit) => HELP_PANEL_SPLIT_ZH,
            (Self::EnUs, CommandPath::PanelClose) => HELP_PANEL_CLOSE_EN,
            (Self::ZhCn, CommandPath::PanelClose) => HELP_PANEL_CLOSE_ZH,
            (Self::EnUs, CommandPath::PanelMove) => HELP_PANEL_MOVE_EN,
            (Self::ZhCn, CommandPath::PanelMove) => HELP_PANEL_MOVE_ZH,
            (Self::EnUs, CommandPath::TerminalList) => HELP_TERMINAL_LIST_EN,
            (Self::ZhCn, CommandPath::TerminalList) => HELP_TERMINAL_LIST_ZH,
            (Self::EnUs, CommandPath::TerminalGet) => HELP_TERMINAL_GET_EN,
            (Self::ZhCn, CommandPath::TerminalGet) => HELP_TERMINAL_GET_ZH,
            (Self::EnUs, CommandPath::TerminalRead) => HELP_TERMINAL_READ_EN,
            (Self::ZhCn, CommandPath::TerminalRead) => HELP_TERMINAL_READ_ZH,
            (Self::EnUs, CommandPath::ApiSnapshot) => HELP_API_SNAPSHOT_EN,
            (Self::ZhCn, CommandPath::ApiSnapshot) => HELP_API_SNAPSHOT_ZH,
            (Self::EnUs, CommandPath::ApiSchema) => HELP_API_SCHEMA_EN,
            (Self::ZhCn, CommandPath::ApiSchema) => HELP_API_SCHEMA_ZH,
            (Self::EnUs, CommandPath::ConfigCheck) => HELP_CONFIG_CHECK_EN,
            (Self::ZhCn, CommandPath::ConfigCheck) => HELP_CONFIG_CHECK_ZH,
            (Self::EnUs, CommandPath::Completion) => HELP_COMPLETION_EN,
            (Self::ZhCn, CommandPath::Completion) => HELP_COMPLETION_ZH,
        }
    }
}

const HELP_TUI_EN: &str = "orchbee tui\n\nOpen the local terminal interface (Linux development cohort).\nA bare 'orchbee' invocation does the same.\n";
const HELP_TUI_ZH: &str =
    "orchbee tui\n\n打开本地终端界面（Linux 开发 cohort）。\n裸 'orchbee' 调用效果相同。\n";

const HELP_STATUS_EN: &str = "orchbee status\n\nRead a live Workbench snapshot from the running local Site and report\nreachability plus entity counts. The Site is never created, activated, or\nmutated. '--format json' emits the orchbee.status envelope.\n";
const HELP_STATUS_ZH: &str = "orchbee status\n\n从正在运行的本地 Site 读取实时 Workbench 快照并报告可达性与实体计数。\n绝不创建、激活或修改 Site。'--format json' 输出 orchbee.status 封套。\n";

const HELP_PROJECT_EN: &str = "orchbee project list|get\n\nQuery Project resources in the running local Site.\n  project list            List every Project\n  project get <target>    Show one Project by exact ID or exact label\nTargets are case-sensitive; ambiguous labels are rejected, never guessed.\n";
const HELP_PROJECT_ZH: &str = "orchbee project list|get\n\n查询本地 Site 中的 Project 资源。\n  project list            列出所有 Project\n  project get <target>    按精确 ID 或精确 label 显示一个 Project\ntarget 区分大小写；歧义 label 会被拒绝而非猜测。\n";

const HELP_WORKSPACE_EN: &str = "orchbee workspace list|get|create|rename\n\nQuery and mutate Workspace resources in the running local Site.\n  workspace list [--project <t>]                    List Workspaces, optionally inside one Project\n  workspace get <target>                            Show one Workspace by exact ID or exact label\n  workspace create --project <t> --label <l>        Create one Workspace inside a Project\n  workspace rename <target> --label <l>             Rename one Workspace\nEvery mutation reads one snapshot, then submits exactly one command.\n";
const HELP_WORKSPACE_ZH: &str = "orchbee workspace list|get|create|rename\n\n查询并修改本地 Site 中的 Workspace 资源。\n  workspace list [--project <t>]                    列出 Workspace，可按某个 Project 过滤\n  workspace get <target>                            按精确 ID 或精确 label 显示一个 Workspace\n  workspace create --project <t> --label <l>        在某个 Project 内创建一个 Workspace\n  workspace rename <target> --label <l>             重命名一个 Workspace\n每个修改命令先读取一次快照，然后只提交一个命令。\n";

const HELP_TAB_EN: &str = "orchbee tab list|get|create|rename|move|close\n\nQuery and mutate Tab resources in the running local Site.\n  tab list [--workspace <t> | --standalone]  List Tabs, optionally filtered\n  tab get <target>                           Show one Tab by exact ID or label\n  tab create (--workspace <t>|--standalone) --label <l> --cols <n> --rows <n> --cell-width <px> --cell-height <px>\n                                             Create one Tab in one context\n  tab rename <target> --label <l>            Rename one Tab\n  tab move <target> --direction <d>          Move one Tab within its strip\n  tab close <target> --confirm               Close one Tab\n'--workspace' and '--standalone' are mutually exclusive single-use selectors.\n";
const HELP_TAB_ZH: &str = "orchbee tab list|get|create|rename|move|close\n\n查询并修改本地 Site 中的 Tab 资源。\n  tab list [--workspace <t> | --standalone]  列出 Tab，可按 Workspace 或独立上下文过滤\n  tab get <target>                           按精确 ID 或 label 显示一个 Tab\n  tab create (--workspace <t>|--standalone) --label <l> --cols <n> --rows <n> --cell-width <px> --cell-height <px>\n                                             在某个上下文内创建一个 Tab\n  tab rename <target> --label <l>            重命名一个 Tab\n  tab move <target> --direction <d>          在标签条内移动一个 Tab\n  tab close <target> --confirm               关闭一个 Tab\n'--workspace' 与 '--standalone' 互斥且各只能用一次。\n";

const HELP_PANEL_EN: &str = "orchbee panel list|get|split|close|move\n\nQuery and mutate Panel resources in the running local Site.\n  panel list [--tab <t>]                   List Panels, optionally inside one Tab\n  panel get <panel-id>                     Show one Panel by its complete typed ID\n  panel split <panel-id> --direction <d> --label <l> --cols <n> --rows <n> --cell-width <px> --cell-height <px>\n                                           Split one Panel, creating a sibling\n  panel close <panel-id> --confirm         Close one Panel\n  panel move <panel-id> --target-panel <panel-id> --direction <d>\n                                           Move one Panel next to a sibling\nPanels have no independent label; only complete 32-hex IDs are accepted.\n";
const HELP_PANEL_ZH: &str = "orchbee panel list|get|split|close|move\n\n查询并修改本地 Site 中的 Panel 资源。\n  panel list [--tab <t>]                   列出 Panel，可按某个 Tab 过滤\n  panel get <panel-id>                     按完整类型化 ID 显示一个 Panel\n  panel split <panel-id> --direction <d> --label <l> --cols <n> --rows <n> --cell-width <px> --cell-height <px>\n                                           分裂一个 Panel，创建一个相邻 Panel\n  panel close <panel-id> --confirm         关闭一个 Panel\n  panel move <panel-id> --target-panel <panel-id> --direction <d>\n                                           将一个 Panel 移动到某个相邻 Panel 旁\nPanel 没有独立 label；只接受完整 32 位小写十六进制 ID。\n";

const HELP_TERMINAL_EN: &str = "orchbee terminal list|get|read\n\nQuery terminals bound by Workbench Presentations in the running local Site.\n  terminal list                List every bound terminal\n  terminal get <terminal-id>   Show one terminal by its complete typed ID\n  terminal read <terminal-id>  Read the terminal's current visible content\nA terminal invisible to any Presentation binding cannot be listed or read; a\nbinding proves structure only, not that a process is alive.\n";
const HELP_TERMINAL_ZH: &str = "orchbee terminal list|get|read\n\n查询本地 Site 中被 Workbench Presentation 绑定的终端。\n  terminal list                列出所有已绑定终端\n  terminal get <terminal-id>   按完整类型化 ID 显示一个终端\n  terminal read <terminal-id>  读取终端当前可见内容\n未被任何 Presentation 引用的终端不可见也不可读；绑定只证明结构，不证明进程存活。\n";

const HELP_API_EN: &str = "orchbee api snapshot|schema\n\nMachine introspection of the local Workbench surface.\n  api snapshot           Emit the complete workbench.snapshot document\n  api schema [<id>]      List embedded JSON Schemas, or one schema by ID\nBoth always answer JSON, whatever '--format' was selected.\n";
const HELP_API_ZH: &str = "orchbee api snapshot|schema\n\n本地 Workbench 面的机器自省。\n  api snapshot           输出完整 workbench.snapshot 文档\n  api schema [<id>]      列出内嵌 JSON Schema，或按 ID 取单个 schema\n两者无论 '--format' 选择如何都输出 JSON。\n";

const HELP_CONFIG_EN: &str = "orchbee config check\n\nInspect local inputs only: runtime base, Site runtime directory, current\ncohort integrity, effective locale, and socket presence. Nothing is\nconnected, started, created, or locked. Any failed check exits 1.\n";
const HELP_CONFIG_ZH: &str = "orchbee config check\n\n仅检查本地输入：运行时基座、Site 运行时目录、当前 cohort 完整性、生效\nlocale 与套接字存在性。不连接、不启动、不创建、不加锁。任一检查失败\n以 1 退出。\n";

const HELP_COMPLETION_EN: &str = "orchbee completion bash|zsh|fish\n\nEmit a static completion script for the implemented grammar to stdout.\nNothing is installed and no user files are written.\n";
const HELP_COMPLETION_ZH: &str = "orchbee completion bash|zsh|fish\n\n向标准输出生成已实现语法的静态补全脚本。不安装、不写入用户文件。\n";

const HELP_PROJECT_LIST_EN: &str = "orchbee project list [--format text|json]\n\nList every Project in the running local Site: id, label, directory and\nworkspace counts. An empty Site prints just the table header.\n";
const HELP_PROJECT_LIST_ZH: &str = "orchbee project list [--format text|json]\n\n列出本地 Site 中所有 Project：ID、label、目录与工作区计数。空 Site 仅\n输出表头。\n";

const HELP_PROJECT_GET_EN: &str = "orchbee project get <target> [--format text|json]\n\nShow one Project: its directories (with worker ids and paths) and its\nworkspaces. The target is an exact 32-hex ID or an exact case-sensitive\nlabel; ambiguous labels report candidates instead of guessing.\n";
const HELP_PROJECT_GET_ZH: &str = "orchbee project get <target> [--format text|json]\n\n显示一个 Project：其目录（含 worker id 与路径）与工作区。target 为精确\n32 位十六进制 ID 或精确区分大小写的 label；歧义 label 报告候选而非猜测。\n";

const HELP_WORKSPACE_LIST_EN: &str = "orchbee workspace list [--project <t>] [--format text|json]\n\nList Workspaces across Projects, or only inside the Project selected by\n--project (exact ID or exact label). Fields: ids, label, primary directory\n(null means none), and terminal context.\n";
const HELP_WORKSPACE_LIST_ZH: &str = "orchbee workspace list [--project <t>] [--format text|json]\n\n跨 Project 列出 Workspace，或仅列 --project（精确 ID 或精确 label）所选\nProject 内的。字段：各 ID、label、主目录（null 表示无）与终端上下文。\n";

const HELP_WORKSPACE_GET_EN: &str = "orchbee workspace get <target> [--format text|json]\n\nShow one Workspace plus the Tabs that live in its terminal context.\n";
const HELP_WORKSPACE_GET_ZH: &str = "orchbee workspace get <target> [--format text|json]\n\n显示一个 Workspace 及其终端上下文中存在的 Tab。\n";

const HELP_WORKSPACE_CREATE_EN: &str = "orchbee workspace create --project <target> --label <label> [--format text|json]\n\nCreate one Workspace inside the Project selected by --project (exact ID or\nexact label). The command reads one snapshot, resolves the target, then\nsubmits exactly one command under a fresh request id. The result carries\nthe committed revision and the new workspace and terminal context ids.\n";
const HELP_WORKSPACE_CREATE_ZH: &str = "orchbee workspace create --project <target> --label <label> [--format text|json]\n\n在 --project（精确 ID 或精确 label）所选 Project 内创建一个 Workspace。\n命令先读取一次快照、解析目标，然后以全新请求 ID 只提交一个命令。结果\n携带已提交修订以及新的 workspace 与终端上下文 ID。\n";

const HELP_WORKSPACE_RENAME_EN: &str = "orchbee workspace rename <target> --label <label> [--format text|json]\n\nRename one Workspace selected by exact ID or exact label. The command\nreads one snapshot, then submits exactly one command. A committed result\ncarries the new revision; an unchanged one means the label already\nmatched.\n";
const HELP_WORKSPACE_RENAME_ZH: &str = "orchbee workspace rename <target> --label <label> [--format text|json]\n\n重命名一个由精确 ID 或精确 label 选择的 Workspace。命令先读取一次快\n照，然后只提交一个命令。committed 结果携带新修订；unchanged 表示\nlabel 本已相同。\n";

const HELP_TAB_LIST_EN: &str = "orchbee tab list [--workspace <t> | --standalone] [--format text|json]\n\nList Tabs. Filters select one Workspace's context (exact ID or label) or\nthe standalone context. Fields: ids, label, pinned flag, group, and owning\ncontext kind.\n";
const HELP_TAB_LIST_ZH: &str = "orchbee tab list [--workspace <t> | --standalone] [--format text|json]\n\n列出 Tab。过滤参数可选择一个 Workspace 的上下文（精确 ID 或 label）或\n独立上下文。字段：各 ID、label、置顶标记、分组与所属上下文类型。\n";

const HELP_TAB_GET_EN: &str = "orchbee tab get <target> [--format text|json]\n\nShow one Tab including its complete PanelNode root subtree.\n";
const HELP_TAB_GET_ZH: &str = "orchbee tab get <target> [--format text|json]\n\n显示一个 Tab，包括其完整 PanelNode 根子树。\n";

const HELP_TAB_CREATE_EN: &str = "orchbee tab create (--workspace <target> | --standalone) --label <label> --cols <n> --rows <n> --cell-width <px> --cell-height <px> [--format text|json]\n\nCreate one Tab in the selected terminal context: a Workspace chosen by\nexact ID or exact label, or the standalone context. All four geometry\nflags are required. The result carries the committed revision, the new\ntab, panel, and presentation ids, and the terminal spawn outcome.\n";
const HELP_TAB_CREATE_ZH: &str = "orchbee tab create (--workspace <target> | --standalone) --label <label> --cols <n> --rows <n> --cell-width <px> --cell-height <px> [--format text|json]\n\n在所选终端上下文内创建一个 Tab：由精确 ID 或精确 label 选择的\nWorkspace，或独立上下文。四个几何参数均为必填。结果携带已提交修订、\n新的 tab、panel、presentation ID 及终端拉起结果。\n";

const HELP_TAB_RENAME_EN: &str = "orchbee tab rename <target> --label <label> [--format text|json]\n\nRename one Tab selected by exact ID or exact label. A committed result\ncarries the new revision; unchanged means the label already matched.\n";
const HELP_TAB_RENAME_ZH: &str = "orchbee tab rename <target> --label <label> [--format text|json]\n\n重命名一个由精确 ID 或精确 label 选择的 Tab。committed 结果携带新修\n订；unchanged 表示 label 本已相同。\n";

const HELP_TAB_MOVE_EN: &str = "orchbee tab move <target> --direction <previous|next|first|last> [--format text|json]\n\nMove one Tab within its strip. A committed result carries the new\nrevision; unchanged means the tab was already in position.\n";
const HELP_TAB_MOVE_ZH: &str = "orchbee tab move <target> --direction <previous|next|first|last> [--format text|json]\n\n在标签条内移动一个 Tab。committed 结果携带新修订；unchanged 表示 Tab\n本已在目标位置。\n";

const HELP_TAB_CLOSE_EN: &str = "orchbee tab close <target> --confirm [--format text|json]\n\nClose one Tab selected by exact ID or exact label. '--confirm' is\nrequired. An accepted result carries the accepted revision and the close\nrequest id; the Site finishes the close asynchronously.\n";
const HELP_TAB_CLOSE_ZH: &str = "orchbee tab close <target> --confirm [--format text|json]\n\n关闭一个由精确 ID 或精确 label 选择的 Tab。'--confirm' 为必填。\naccepted 结果携带已接受修订与关闭请求 ID；Site 异步完成关闭。\n";

const HELP_PANEL_LIST_EN: &str = "orchbee panel list [--tab <t>] [--format text|json]\n\nList Panels, optionally inside the Tab selected by --tab (exact ID or\nlabel). Panels carry no label; presentation labels are not panel names.\n";
const HELP_PANEL_LIST_ZH: &str = "orchbee panel list [--tab <t>] [--format text|json]\n\n列出 Panel，或仅列 --tab（精确 ID 或 label）所选 Tab 内的。Panel 无\nlabel；Presentation 的 label 不是 Panel 名。\n";

const HELP_PANEL_GET_EN: &str = "orchbee panel get <panel-id> [--format text|json]\n\nShow one Panel by complete typed ID: owning tab, context, slot path, and\nits primary and child Presentations.\n";
const HELP_PANEL_GET_ZH: &str = "orchbee panel get <panel-id> [--format text|json]\n\n按完整类型化 ID 显示一个 Panel：所属 Tab、上下文、slot 路径及其主与\n子 Presentation。\n";

const HELP_PANEL_SPLIT_EN: &str = "orchbee panel split <panel-id> --direction <above|below|left|right> --label <label> --cols <n> --rows <n> --cell-width <px> --cell-height <px> [--format text|json]\n\nSplit one Panel, creating a sibling panel on the selected side. All four\ngeometry flags are required. The result carries the committed revision,\nthe new panel, split, slot, and presentation ids, and the terminal spawn\noutcome.\n";
const HELP_PANEL_SPLIT_ZH: &str = "orchbee panel split <panel-id> --direction <above|below|left|right> --label <label> --cols <n> --rows <n> --cell-width <px> --cell-height <px> [--format text|json]\n\n分裂一个 Panel，在所选侧创建相邻 Panel。四个几何参数均为必填。结果携\n带已提交修订、新的 panel、split、slot、presentation ID 及终端拉起\n结果。\n";

const HELP_PANEL_CLOSE_EN: &str = "orchbee panel close <panel-id> --confirm [--format text|json]\n\nClose one Panel selected by complete typed ID. '--confirm' is required.\nAn accepted result carries the accepted revision and the close request\nid; the Site finishes the close asynchronously.\n";
const HELP_PANEL_CLOSE_ZH: &str = "orchbee panel close <panel-id> --confirm [--format text|json]\n\n关闭一个由完整类型化 ID 选择的 Panel。'--confirm' 为必填。accepted\n结果携带已接受修订与关闭请求 ID；Site 异步完成关闭。\n";

const HELP_PANEL_MOVE_EN: &str = "orchbee panel move <panel-id> --target-panel <panel-id> --direction <above|below|left|right> [--format text|json]\n\nMove one Panel next to another panel inside the same tab. Both panels\nare complete typed IDs. A committed result carries the new revision;\nunchanged means the layout already matched.\n";
const HELP_PANEL_MOVE_ZH: &str = "orchbee panel move <panel-id> --target-panel <panel-id> --direction <above|below|left|right> [--format text|json]\n\n将一个 Panel 移动到同一 Tab 内另一个 Panel 的所选侧。两个 Panel 均为\n完整类型化 ID。committed 结果携带新修订；unchanged 表示布局本已相同。\n";

const HELP_TERMINAL_LIST_EN: &str = "orchbee terminal list [--format text|json]\n\nList every terminal bound by a Presentation. Unbound terminals are not\nvisible; a binding is structural evidence, not proof a process is alive.\n";
const HELP_TERMINAL_LIST_ZH: &str = "orchbee terminal list [--format text|json]\n\n列出所有被 Presentation 绑定的终端。未绑定终端不可见；绑定是结构证据，\n不是进程存活的证明。\n";

const HELP_TERMINAL_GET_EN: &str = "orchbee terminal get <terminal-id> [--format text|json]\n\nShow one bound terminal by complete typed ID: worker id and its single\nboundTo location (presentation, panel, tab, context).\n";
const HELP_TERMINAL_GET_ZH: &str = "orchbee terminal get <terminal-id> [--format text|json]\n\n按完整类型化 ID 显示一个已绑定终端：worker id 及其唯一 boundTo 位置\n（presentation、panel、tab、context）。\n";

const HELP_TERMINAL_READ_EN: &str = "orchbee terminal read <terminal-id> [--format text|json]\n\nRead the current visible content of one bound terminal. The terminal is\nresolved from the Workbench snapshot, then observed once through a short-\nlived read-only viewer on the same session; nothing is created, activated,\nor mutated. Text output prints the grid rows; '--format json' emits the\norchbee.terminal.read envelope with frame metadata and row content.\n";
const HELP_TERMINAL_READ_ZH: &str = "orchbee terminal read <terminal-id> [--format text|json]\n\n读取一个已绑定终端的当前可见内容。终端经 Workbench 快照解析后，通过同\n一会话上短暂存活的只读 viewer 观察一次；不创建、不激活、不修改任何\n状态。文本输出逐行打印网格内容；'--format json' 输出带帧元数据与行内\n容的 orchbee.terminal.read 封套。\n";

const HELP_API_SNAPSHOT_EN: &str = "orchbee api snapshot [--format text|json]\n\nEmit the complete validated workbench.snapshot document inside the\norchbee.api.snapshot envelope. Always JSON, even under '--format text'.\n";
const HELP_API_SNAPSHOT_ZH: &str = "orchbee api snapshot [--format text|json]\n\n在 orchbee.api.snapshot 封套内输出完整已验证 workbench.snapshot 文档。\n即使 '--format text' 也始终输出 JSON。\n";

const HELP_API_SCHEMA_EN: &str = "orchbee api schema [<schema-id>] [--format text|json]\n\nEmit the embedded JSON Schema dictionary for every implemented command, or\nthe single schema for <schema-id>. Always JSON; unknown IDs are not-found.\n";
const HELP_API_SCHEMA_ZH: &str = "orchbee api schema [<schema-id>] [--format text|json]\n\n输出所有已实现命令的内嵌 JSON Schema 字典，或 <schema-id> 的单个 schema。\n始终输出 JSON；未知 ID 为 not-found。\n";

const HELP_CONFIG_CHECK_EN: &str = "orchbee config check [--format text|json]\n\nRun the five local checks (runtime-base, runtime-directory, cohort, locale,\nsockets) and report pass/fail/not-applicable each. Exit 0 when all pass or\nare not-applicable, 1 when any check fails.\n";
const HELP_CONFIG_CHECK_ZH: &str = "orchbee config check [--format text|json]\n\n运行五项本地检查（runtime-base、runtime-directory、cohort、locale、\nsockets）并逐项报告 pass/fail/not-applicable。全部通过或不适用时退出\n0，任一失败时退出 1。\n";

// ---- text presentation labels --------------------------------------------

/// Column headers and field names for the human text surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CliLabels {
    pub(crate) bound_terminals_word: &'static str,
    pub(crate) bound_to: &'static str,
    pub(crate) child_presentations: &'static str,
    pub(crate) context: &'static str,
    pub(crate) counts: &'static str,
    pub(crate) directories: &'static str,
    pub(crate) group: &'static str,
    pub(crate) id: &'static str,
    pub(crate) panels_word: &'static str,
    pub(crate) pinned: &'static str,
    pub(crate) primary_directory: &'static str,
    pub(crate) primary_presentation: &'static str,
    pub(crate) project: &'static str,
    pub(crate) project_id: &'static str,
    pub(crate) projects_word: &'static str,
    pub(crate) reachable: &'static str,
    pub(crate) revision_label: &'static str,
    pub(crate) root: &'static str,
    pub(crate) site: &'static str,
    pub(crate) site_id: &'static str,
    pub(crate) slot_path: &'static str,
    pub(crate) tab: &'static str,
    pub(crate) tab_id: &'static str,
    pub(crate) tabs: &'static str,
    pub(crate) tabs_word: &'static str,
    pub(crate) terminal: &'static str,
    pub(crate) terminal_context: &'static str,
    pub(crate) worker: &'static str,
    pub(crate) worker_unknown: &'static str,
    pub(crate) workspace: &'static str,
    pub(crate) workspaces: &'static str,
    pub(crate) workspaces_word: &'static str,
    pub(crate) project_headers: [&'static str; 4],
    pub(crate) directory_headers: [&'static str; 3],
    pub(crate) workspace_ref_headers: [&'static str; 2],
    pub(crate) workspace_headers: [&'static str; 5],
    pub(crate) tab_ref_headers: [&'static str; 2],
    pub(crate) tab_headers: [&'static str; 6],
    pub(crate) panel_headers: [&'static str; 6],
    pub(crate) presentation_headers: [&'static str; 4],
    pub(crate) terminal_headers: [&'static str; 6],
}

const EN_US_LABELS: CliLabels = CliLabels {
    bound_terminals_word: "bound terminals",
    bound_to: "Bound to",
    child_presentations: "Child presentations",
    context: "Context",
    counts: "Counts",
    directories: "Directories",
    group: "Group",
    id: "ID",
    panels_word: "panels",
    pinned: "Pinned",
    primary_directory: "Primary dir",
    primary_presentation: "Primary presentation",
    project: "Project",
    project_id: "Project ID",
    projects_word: "projects",
    reachable: "reachable",
    revision_label: "revision",
    root: "Root",
    site: "Site",
    site_id: "Site ID",
    slot_path: "Slot path",
    tab: "Tab",
    tab_id: "Tab ID",
    tabs: "Tabs",
    tabs_word: "tabs",
    terminal: "Terminal",
    terminal_context: "Context ID",
    worker: "Worker",
    worker_unknown: "unknown (not reported by the Workbench snapshot)",
    workspace: "Workspace",
    workspaces: "Workspaces",
    workspaces_word: "workspaces",
    project_headers: ["ID", "LABEL", "DIRECTORIES", "WORKSPACES"],
    directory_headers: ["ID", "WORKER", "PATH"],
    workspace_ref_headers: ["ID", "LABEL"],
    workspace_headers: ["ID", "LABEL", "PROJECT", "PRIMARY DIRECTORY", "CONTEXT"],
    tab_ref_headers: ["ID", "LABEL"],
    tab_headers: ["ID", "LABEL", "PINNED", "GROUP", "CONTEXT", "CONTEXT ID"],
    panel_headers: [
        "ID",
        "TAB",
        "CONTEXT",
        "PRIMARY PRESENTATION",
        "PRESENTATIONS",
        "PRIMARY TERMINAL",
    ],
    presentation_headers: ["ID", "LABEL", "TERMINAL", "WORKER"],
    terminal_headers: ["ID", "WORKER", "PRESENTATION", "PANEL", "TAB", "CONTEXT"],
};

const ZH_CN_LABELS: CliLabels = CliLabels {
    bound_terminals_word: "已绑定终端",
    bound_to: "绑定到",
    child_presentations: "子 Presentation",
    context: "上下文",
    counts: "计数",
    directories: "目录",
    group: "分组",
    id: "ID",
    panels_word: "面板",
    pinned: "置顶",
    primary_directory: "主目录",
    primary_presentation: "主 Presentation",
    project: "Project",
    project_id: "Project ID",
    projects_word: "项目",
    reachable: "可达",
    revision_label: "修订",
    root: "根节点",
    site: "Site",
    site_id: "Site ID",
    slot_path: "Slot 路径",
    tab: "Tab",
    tab_id: "Tab ID",
    tabs: "Tab",
    tabs_word: "标签",
    terminal: "终端",
    terminal_context: "上下文 ID",
    worker: "Worker",
    worker_unknown: "未知（Workbench 快照未上报）",
    workspace: "工作区",
    workspaces: "工作区",
    workspaces_word: "工作区",
    project_headers: ["ID", "LABEL", "目录数", "工作区数"],
    directory_headers: ["ID", "WORKER", "路径"],
    workspace_ref_headers: ["ID", "LABEL"],
    workspace_headers: ["ID", "LABEL", "PROJECT", "主目录", "上下文"],
    tab_ref_headers: ["ID", "LABEL"],
    tab_headers: ["ID", "LABEL", "置顶", "分组", "上下文", "上下文 ID"],
    panel_headers: [
        "ID",
        "TAB",
        "上下文",
        "主 PRESENTATION",
        "PRESENTATION 数",
        "主终端",
    ],
    presentation_headers: ["ID", "LABEL", "终端", "WORKER"],
    terminal_headers: ["ID", "WORKER", "PRESENTATION", "PANEL", "TAB", "上下文"],
};

impl Locale {
    pub(crate) const fn cli_labels(self) -> &'static CliLabels {
        match self {
            Self::EnUs => &EN_US_LABELS,
            Self::ZhCn => &ZH_CN_LABELS,
        }
    }
}
