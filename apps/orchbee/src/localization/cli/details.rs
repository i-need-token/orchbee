//! Human detail views for `workspace get`, `terminal get` and
//! `panel process-info`: one sentence, then aligned labels. Internal
//! identities and raw structures stay in the JSON documents.
use super::Locale;

/// Labels shared by the detail views.
#[derive(Clone, Copy)]
pub(crate) enum DetailLabel {
    Id,
    Project,
    Directory,
    Tabs,
    Panel,
    Tab,
    Launch,
    Terminal,
    Process,
    Pid,
    Started,
    Command,
    Agent,
    Processes,
}

impl Locale {
    pub(crate) const fn cli_detail_label(self, label: DetailLabel) -> &'static str {
        match (self, label) {
            (Self::EnUs, DetailLabel::Id) => "ID",
            (Self::ZhCn, DetailLabel::Id) => "ID",
            (Self::EnUs, DetailLabel::Project) => "Project",
            (Self::ZhCn, DetailLabel::Project) => "项目",
            (Self::EnUs, DetailLabel::Directory) => "Directory",
            (Self::ZhCn, DetailLabel::Directory) => "目录",
            (Self::EnUs, DetailLabel::Tabs) => "Tabs",
            (Self::ZhCn, DetailLabel::Tabs) => "标签页",
            (Self::EnUs, DetailLabel::Panel) => "Panel",
            (Self::ZhCn, DetailLabel::Panel) => "面板",
            (Self::EnUs, DetailLabel::Tab) => "Tab",
            (Self::ZhCn, DetailLabel::Tab) => "标签页",
            (Self::EnUs, DetailLabel::Launch) => "Launch",
            (Self::ZhCn, DetailLabel::Launch) => "启动",
            (Self::EnUs, DetailLabel::Terminal) => "Terminal",
            (Self::ZhCn, DetailLabel::Terminal) => "终端",
            (Self::EnUs, DetailLabel::Process) => "Process",
            (Self::ZhCn, DetailLabel::Process) => "进程",
            (Self::EnUs, DetailLabel::Pid) => "PID",
            (Self::ZhCn, DetailLabel::Pid) => "PID",
            (Self::EnUs, DetailLabel::Started) => "Started",
            (Self::ZhCn, DetailLabel::Started) => "启动时间",
            (Self::EnUs, DetailLabel::Command) => "Command",
            (Self::ZhCn, DetailLabel::Command) => "命令",
            (Self::EnUs, DetailLabel::Agent) => "Agent",
            (Self::ZhCn, DetailLabel::Agent) => "智能体",
            (Self::EnUs, DetailLabel::Processes) => "Processes",
            (Self::ZhCn, DetailLabel::Processes) => "进程列表",
        }
    }

    pub(crate) fn cli_workspace_summary(
        self,
        label: &str,
        number: usize,
        tabs: usize,
        panels: usize,
    ) -> String {
        match self {
            Self::EnUs => format!(
                "Workspace {label} (#{number}) has {tabs} {} and {panels} {}.\n",
                if tabs == 1 { "tab" } else { "tabs" },
                if panels == 1 { "panel" } else { "panels" },
            ),
            Self::ZhCn => {
                format!("工作区 {label}（#{number}）有 {tabs} 个标签页和 {panels} 个面板。\n")
            }
        }
    }

    pub(crate) fn cli_terminal_summary(self, panel: &str) -> String {
        match self {
            Self::EnUs => format!("Terminal shown in panel {panel}.\n"),
            Self::ZhCn => format!("终端显示在面板 {panel} 中。\n"),
        }
    }

    /// A launch state from the closed `state` vocabulary.
    pub(crate) fn cli_launch_state(self, state: &str) -> &'static str {
        match (self, state) {
            (Self::EnUs, "creating") => "creating",
            (Self::ZhCn, "creating") => "创建中",
            (Self::EnUs, "awaiting-start") => "waiting to start",
            (Self::ZhCn, "awaiting-start") => "等待启动",
            (Self::EnUs, "starting") => "starting",
            (Self::ZhCn, "starting") => "启动中",
            (Self::EnUs, "started") => "started",
            (Self::ZhCn, "started") => "已启动",
            (Self::EnUs, "unavailable") => "unavailable",
            (Self::ZhCn, "unavailable") => "不可用",
            (Self::EnUs, "not-tracked") => "not tracked",
            (Self::ZhCn, "not-tracked") => "未跟踪",
            (Self::EnUs, _) => "unknown",
            (Self::ZhCn, _) => "未知",
        }
    }

    /// Which process the sample identifies as being in the foreground.
    pub(crate) fn cli_process_summary(
        self,
        selection: crate::cli::processes::Selection,
        provider: Option<&str>,
    ) -> String {
        use crate::cli::processes::Selection;
        match (self, selection) {
            (Self::EnUs, Selection::AgentChain) => format!(
                "The foreground process is the {} Agent.\n",
                provider.unwrap_or("-")
            ),
            (Self::ZhCn, Selection::AgentChain) => {
                format!("前台进程是 {} 智能体。\n", provider.unwrap_or("-"))
            }
            (Self::EnUs, Selection::Root) => {
                "The foreground process is the terminal's shell or launched program.\n".to_owned()
            }
            (Self::ZhCn, Selection::Root) => "前台进程是终端的 shell 或启动程序。\n".to_owned(),
            (Self::EnUs, Selection::Ambiguous) => {
                "More than one Agent process could be in the foreground.\n".to_owned()
            }
            (Self::ZhCn, Selection::Ambiguous) => "有多个智能体进程可能处于前台。\n".to_owned(),
            (Self::EnUs, Selection::Unavailable) => {
                "The foreground process could not be determined.\n".to_owned()
            }
            (Self::ZhCn, Selection::Unavailable) => "无法确定前台进程。\n".to_owned(),
        }
    }

    pub(crate) const fn cli_process_headers(self) -> [&'static str; 6] {
        match self {
            Self::EnUs => ["PID", "PARENT", "NAME", "AGENT", "STARTED", "DIRECTORY"],
            Self::ZhCn => ["PID", "父 PID", "名称", "智能体", "启动时间", "目录"],
        }
    }
}
