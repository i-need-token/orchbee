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
    Identity,
    Processes,
}

impl super::CliText {
    pub(crate) const fn cli_detail_label(self, label: DetailLabel) -> &'static str {
        match (self.locale, label) {
            (Locale::EnUs, DetailLabel::Id) => "ID",
            (Locale::ZhCn, DetailLabel::Id) => "ID",
            (Locale::EnUs, DetailLabel::Project) => "Project",
            (Locale::ZhCn, DetailLabel::Project) => "项目",
            (Locale::EnUs, DetailLabel::Directory) => "Directory",
            (Locale::ZhCn, DetailLabel::Directory) => "目录",
            (Locale::EnUs, DetailLabel::Tabs) => "Tabs",
            (Locale::ZhCn, DetailLabel::Tabs) => "标签页",
            (Locale::EnUs, DetailLabel::Panel) => "Panel",
            (Locale::ZhCn, DetailLabel::Panel) => "面板",
            (Locale::EnUs, DetailLabel::Tab) => "Tab",
            (Locale::ZhCn, DetailLabel::Tab) => "标签页",
            (Locale::EnUs, DetailLabel::Launch) => "Launch",
            (Locale::ZhCn, DetailLabel::Launch) => "启动",
            (Locale::EnUs, DetailLabel::Terminal) => "Terminal",
            (Locale::ZhCn, DetailLabel::Terminal) => "终端",
            (Locale::EnUs, DetailLabel::Process) => "Process",
            (Locale::ZhCn, DetailLabel::Process) => "进程",
            (Locale::EnUs, DetailLabel::Pid) => "PID",
            (Locale::ZhCn, DetailLabel::Pid) => "PID",
            (Locale::EnUs, DetailLabel::Started) => "Started",
            (Locale::ZhCn, DetailLabel::Started) => "启动时间",
            (Locale::EnUs, DetailLabel::Command) => "Command",
            (Locale::ZhCn, DetailLabel::Command) => "命令",
            (Locale::EnUs, DetailLabel::Agent) => "Agent",
            (Locale::ZhCn, DetailLabel::Agent) => "智能体",
            (Locale::EnUs, DetailLabel::Identity) => "Identity",
            (Locale::ZhCn, DetailLabel::Identity) => "识别结果",
            (Locale::EnUs, DetailLabel::Processes) => "Processes",
            (Locale::ZhCn, DetailLabel::Processes) => "进程列表",
        }
    }

    pub(crate) fn cli_workspace_summary(
        self,
        label: &str,
        number: usize,
        tabs: usize,
        panels: usize,
    ) -> String {
        match self.locale {
            Locale::EnUs => format!(
                "Workspace {label} (#{number}) has {tabs} {} and {panels} {}.\n",
                if tabs == 1 { "tab" } else { "tabs" },
                if panels == 1 { "panel" } else { "panels" },
            ),
            Locale::ZhCn => {
                format!("工作区 {label}（#{number}）有 {tabs} 个标签页和 {panels} 个面板。\n")
            }
        }
    }

    pub(crate) fn cli_terminal_summary(self, panel: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("Terminal shown in panel {panel}.\n"),
            Locale::ZhCn => format!("终端显示在面板 {panel} 中。\n"),
        }
    }

    /// A launch state from the closed `state` vocabulary.
    pub(crate) fn cli_launch_state(self, state: &str) -> &'static str {
        match (self.locale, state) {
            (Locale::EnUs, "creating") => "creating",
            (Locale::ZhCn, "creating") => "创建中",
            (Locale::EnUs, "awaiting-start") => "waiting to start",
            (Locale::ZhCn, "awaiting-start") => "等待启动",
            (Locale::EnUs, "starting") => "starting",
            (Locale::ZhCn, "starting") => "启动中",
            (Locale::EnUs, "started") => "started",
            (Locale::ZhCn, "started") => "已启动",
            (Locale::EnUs, "unavailable") => "unavailable",
            (Locale::ZhCn, "unavailable") => "不可用",
            (Locale::EnUs, "not-tracked") => "not tracked",
            (Locale::ZhCn, "not-tracked") => "未跟踪",
            (Locale::EnUs, _) => "unknown",
            (Locale::ZhCn, _) => "未知",
        }
    }

    /// Which process the Server attributes the foreground to.
    pub(crate) fn cli_process_summary(
        self,
        selection: crate::cli::processes::Selection,
        provider: Option<&str>,
    ) -> String {
        use crate::cli::processes::Selection;
        match (self.locale, selection) {
            (Locale::EnUs, Selection::AgentChain) => format!(
                "The foreground process is the {} Agent.\n",
                provider.unwrap_or("-")
            ),
            (Locale::ZhCn, Selection::AgentChain) => {
                format!("前台进程是 {} 智能体。\n", provider.unwrap_or("-"))
            }
            (Locale::EnUs, Selection::Root) => {
                "The foreground process is the terminal's shell or launched program.\n".to_owned()
            }
            (Locale::ZhCn, Selection::Root) => "前台进程是终端的 shell 或启动程序。\n".to_owned(),
            (Locale::EnUs, Selection::Ambiguous) => {
                "More than one Agent process could be in the foreground.\n".to_owned()
            }
            (Locale::ZhCn, Selection::Ambiguous) => "有多个智能体进程可能处于前台。\n".to_owned(),
            (Locale::EnUs, Selection::Unavailable) => {
                "The foreground process could not be determined.\n".to_owned()
            }
            (Locale::ZhCn, Selection::Unavailable) => "无法确定前台进程。\n".to_owned(),
        }
    }

    pub(crate) const fn cli_process_headers(self) -> [&'static str; 6] {
        match self.locale {
            Locale::EnUs => ["PID", "PARENT", "NAME", "AGENT", "STARTED", "DIRECTORY"],
            Locale::ZhCn => ["PID", "父 PID", "名称", "智能体", "启动时间", "目录"],
        }
    }
}
