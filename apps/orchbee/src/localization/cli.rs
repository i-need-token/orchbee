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
            FailureKind::Denied => match self {
                Self::EnUs => "The local Site denied the read request.\n".to_owned(),
                Self::ZhCn => "本地 Site 拒绝了读取请求。\n".to_owned(),
            },
            FailureKind::ServiceUnavailable => self.cli_service_unavailable(failure),
            FailureKind::SessionCapacity => match self {
                Self::EnUs => {
                    "The local Site has no Client Session capacity for this query.\n".to_owned()
                }
                Self::ZhCn => "本地 Site 没有可接纳本次查询的 Client Session 容量。\n".to_owned(),
            },
            FailureKind::TransportLoss => match self {
                Self::EnUs => {
                    "The connection to the local Site was lost before a snapshot arrived.\n"
                        .to_owned()
                }
                Self::ZhCn => "与本地 Site 的连接在快照返回前中断。\n".to_owned(),
            },
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
            (Self::EnUs, CommandPath::TabList) => HELP_TAB_LIST_EN,
            (Self::ZhCn, CommandPath::TabList) => HELP_TAB_LIST_ZH,
            (Self::EnUs, CommandPath::TabGet) => HELP_TAB_GET_EN,
            (Self::ZhCn, CommandPath::TabGet) => HELP_TAB_GET_ZH,
            (Self::EnUs, CommandPath::PanelList) => HELP_PANEL_LIST_EN,
            (Self::ZhCn, CommandPath::PanelList) => HELP_PANEL_LIST_ZH,
            (Self::EnUs, CommandPath::PanelGet) => HELP_PANEL_GET_EN,
            (Self::ZhCn, CommandPath::PanelGet) => HELP_PANEL_GET_ZH,
            (Self::EnUs, CommandPath::TerminalList) => HELP_TERMINAL_LIST_EN,
            (Self::ZhCn, CommandPath::TerminalList) => HELP_TERMINAL_LIST_ZH,
            (Self::EnUs, CommandPath::TerminalGet) => HELP_TERMINAL_GET_EN,
            (Self::ZhCn, CommandPath::TerminalGet) => HELP_TERMINAL_GET_ZH,
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

const HELP_WORKSPACE_EN: &str = "orchbee workspace list|get\n\nQuery Workspace resources in the running local Site.\n  workspace list [--project <t>]  List Workspaces, optionally inside one Project\n  workspace get <target>          Show one Workspace by exact ID or exact label\n";
const HELP_WORKSPACE_ZH: &str = "orchbee workspace list|get\n\n查询本地 Site 中的 Workspace 资源。\n  workspace list [--project <t>]  列出 Workspace，可按某个 Project 过滤\n  workspace get <target>          按精确 ID 或精确 label 显示一个 Workspace\n";

const HELP_TAB_EN: &str = "orchbee tab list|get\n\nQuery Tab resources in the running local Site.\n  tab list [--workspace <t> | --standalone]  List Tabs, optionally filtered\n  tab get <target>                           Show one Tab by exact ID or label\n'--workspace' and '--standalone' are mutually exclusive single-use filters.\n";
const HELP_TAB_ZH: &str = "orchbee tab list|get\n\n查询本地 Site 中的 Tab 资源。\n  tab list [--workspace <t> | --standalone]  列出 Tab，可按 Workspace 或独立上下文过滤\n  tab get <target>                           按精确 ID 或 label 显示一个 Tab\n'--workspace' 与 '--standalone' 互斥且各只能用一次。\n";

const HELP_PANEL_EN: &str = "orchbee panel list|get\n\nQuery Panel resources in the running local Site.\n  panel list [--tab <t>]   List Panels, optionally inside one Tab\n  panel get <panel-id>     Show one Panel by its complete typed ID\nPanels have no independent label; only complete 32-hex IDs are accepted.\n";
const HELP_PANEL_ZH: &str = "orchbee panel list|get\n\n查询本地 Site 中的 Panel 资源。\n  panel list [--tab <t>]   列出 Panel，可按某个 Tab 过滤\n  panel get <panel-id>     按完整类型化 ID 显示一个 Panel\nPanel 没有独立 label；只接受完整 32 位小写十六进制 ID。\n";

const HELP_TERMINAL_EN: &str = "orchbee terminal list|get\n\nQuery terminals bound by Workbench Presentations in the running local Site.\n  terminal list               List every bound terminal\n  terminal get <terminal-id>  Show one terminal by its complete typed ID\nA terminal invisible to any Presentation binding cannot be listed; a binding\nproves structure only, not that a process is alive.\n";
const HELP_TERMINAL_ZH: &str = "orchbee terminal list|get\n\n查询本地 Site 中被 Workbench Presentation 绑定的终端。\n  terminal list               列出所有已绑定终端\n  terminal get <terminal-id>  按完整类型化 ID 显示一个终端\n未被任何 Presentation 引用的终端不可见；绑定只证明结构，不证明进程存活。\n";

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

const HELP_TAB_LIST_EN: &str = "orchbee tab list [--workspace <t> | --standalone] [--format text|json]\n\nList Tabs. Filters select one Workspace's context (exact ID or label) or\nthe standalone context. Fields: ids, label, pinned flag, group, and owning\ncontext kind.\n";
const HELP_TAB_LIST_ZH: &str = "orchbee tab list [--workspace <t> | --standalone] [--format text|json]\n\n列出 Tab。过滤参数可选择一个 Workspace 的上下文（精确 ID 或 label）或\n独立上下文。字段：各 ID、label、置顶标记、分组与所属上下文类型。\n";

const HELP_TAB_GET_EN: &str = "orchbee tab get <target> [--format text|json]\n\nShow one Tab including its complete PanelNode root subtree.\n";
const HELP_TAB_GET_ZH: &str = "orchbee tab get <target> [--format text|json]\n\n显示一个 Tab，包括其完整 PanelNode 根子树。\n";

const HELP_PANEL_LIST_EN: &str = "orchbee panel list [--tab <t>] [--format text|json]\n\nList Panels, optionally inside the Tab selected by --tab (exact ID or\nlabel). Panels carry no label; presentation labels are not panel names.\n";
const HELP_PANEL_LIST_ZH: &str = "orchbee panel list [--tab <t>] [--format text|json]\n\n列出 Panel，或仅列 --tab（精确 ID 或 label）所选 Tab 内的。Panel 无\nlabel；Presentation 的 label 不是 Panel 名。\n";

const HELP_PANEL_GET_EN: &str = "orchbee panel get <panel-id> [--format text|json]\n\nShow one Panel by complete typed ID: owning tab, context, slot path, and\nits primary and child Presentations.\n";
const HELP_PANEL_GET_ZH: &str = "orchbee panel get <panel-id> [--format text|json]\n\n按完整类型化 ID 显示一个 Panel：所属 Tab、上下文、slot 路径及其主与\n子 Presentation。\n";

const HELP_TERMINAL_LIST_EN: &str = "orchbee terminal list [--format text|json]\n\nList every terminal bound by a Presentation. Unbound terminals are not\nvisible; a binding is structural evidence, not proof a process is alive.\n";
const HELP_TERMINAL_LIST_ZH: &str = "orchbee terminal list [--format text|json]\n\n列出所有被 Presentation 绑定的终端。未绑定终端不可见；绑定是结构证据，\n不是进程存活的证明。\n";

const HELP_TERMINAL_GET_EN: &str = "orchbee terminal get <terminal-id> [--format text|json]\n\nShow one bound terminal by complete typed ID: worker id and its single\nboundTo location (presentation, panel, tab, context).\n";
const HELP_TERMINAL_GET_ZH: &str = "orchbee terminal get <terminal-id> [--format text|json]\n\n按完整类型化 ID 显示一个已绑定终端：worker id 及其唯一 boundTo 位置\n（presentation、panel、tab、context）。\n";

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
