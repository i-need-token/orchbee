//! Agent integration copy for Settings. Shared terms follow the GUI catalog
//! (`packages/i18n`, `agent-integrations` and `agent-observation`); diagnosis copy restates the
//! facts that `orchbee agent integration explain` reports, without paths or command output.
use super::super::Locale;
use orchbee_agent_integration::{
    Availability, CommandPurpose, IntegrationId, Issue, Marketplace, TargetState,
};

/// One Agent row's bounded status, in the GUI's precedence, plus the pending label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntegrationStatus {
    Working,
    ManualOnly,
    InspectionFailed,
    NotFound,
    NotChecked,
    Enabled,
    Disabled,
    NotInstalled,
    UpdateRequired,
    Conflict,
    Unknown,
}

/// The GUI's operation failure vocabulary for the last integration job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntegrationFailure {
    AgentUnavailable,
    Busy,
    Conflict,
    ControlUnavailable,
    ManualActionRequired,
    OperationFailed,
    VerificationFailed,
}

/// Why an Agent's switch cannot change now.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntegrationBlock {
    Pending,
    ManualOnly,
    Conflict,
    NotChecked,
    Unknown,
    AgentMissing,
}

/// What one explain report establishes: a definite finding, or an unknown result and its cause.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntegrationFinding {
    Ready,
    ManualSetup,
    AgentMissing,
    NotInstalled,
    Disabled,
    UpdateRequired,
    Conflict,
    Unknown(IntegrationUnknown),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntegrationUnknown {
    TimedOut,
    InspectionFailed,
    Undetermined,
}

/// The Orchbee adapter capsule as `explain` classifies it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntegrationAdapter {
    Ready,
    NotReady,
    SourceAbsent,
    BindingFailed,
}

/// One step of the enable plan; a command keeps only its executable's file name and what it does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum IntegrationStep {
    PublishAdapter,
    Command {
        program: String,
        purpose: Option<CommandPurpose>,
    },
    WriteEntry,
    RemoveEntry,
}

/// The label of one diagnosis fact line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IntegrationFactLabel {
    Agent,
    Integration,
    Issue,
    Adapter,
    Marketplace,
    Plugin,
    Entry,
    AdapterSource,
    Plan,
}

impl Locale {
    fn integration(self, english: &'static str, chinese: &'static str) -> &'static str {
        match self {
            Self::EnUs => english,
            Self::ZhCn => chinese,
        }
    }

    /// Curated product names; both locales keep the product's own spelling.
    pub(crate) const fn agent_integration_name(self, id: IntegrationId) -> &'static str {
        match id {
            IntegrationId::ClaudeCode => "Claude Code",
            IntegrationId::Codex => "Codex",
            IntegrationId::Grok => "Grok",
            IntegrationId::Opencode => "OpenCode",
            IntegrationId::Kilo => "Kilo",
            IntegrationId::Cursor => "Cursor",
            IntegrationId::QoderCli => "Qoder CLI",
            IntegrationId::AntigravityCli => "Antigravity CLI",
            IntegrationId::Mastracode => "Mastracode",
            IntegrationId::Droid => "Droid",
            IntegrationId::Kimi => "Kimi",
            IntegrationId::Letta => "Letta",
            IntegrationId::Devin => "Devin",
            IntegrationId::Hermes => "Hermes",
            IntegrationId::Pi => "Pi",
            IntegrationId::Omp => "OMP",
            IntegrationId::Gemini => "Gemini CLI",
            IntegrationId::GitHubCopilot => "GitHub Copilot CLI",
            IntegrationId::Cline => "Cline",
            IntegrationId::QwenCode => "Qwen Code",
        }
    }

    pub(crate) fn integrations_title(self) -> &'static str {
        self.integration("Agent integrations", "智能体集成")
    }

    pub(crate) fn integrations_disclosure(self) -> &'static str {
        self.integration(
            "Orchbee changes an integration only when you change its switch. It never installs adapters automatically.",
            "只有你切换开关后，Orchbee 才会更改相应集成；适配器永远不会自动安装。",
        )
    }

    pub(crate) fn integrations_local_section(self) -> &'static str {
        self.integration("Local Agent status adapters", "本地智能体状态适配器")
    }

    pub(crate) fn integrations_remote_section(self) -> &'static str {
        self.integration("Remote hosts", "远端主机")
    }

    pub(crate) fn integrations_remote_title(self) -> &'static str {
        self.integration("Remote hosts stay manual", "远端主机保持手动配置")
    }

    pub(crate) fn integrations_remote_description(self) -> &'static str {
        self.integration(
            "Remote hosts are always manual-only. Run the reviewed setup command on the host where the Agent runs; Orchbee never copies files or changes remote configuration.",
            "远端主机始终只能手动配置。请在智能体实际运行的主机执行经过审阅的设置命令；Orchbee 永远不会复制文件或修改远端配置。",
        )
    }

    pub(crate) fn integrations_refresh(self) -> &'static str {
        self.integration("Refresh status", "刷新状态")
    }

    pub(crate) fn integrations_refresh_description(self) -> &'static str {
        self.integration(
            "Reads every integration's state from files on this device; nothing is changed and no Agent is started.",
            "从本设备上的文件读取每个集成的状态；不做任何更改，也不启动任何智能体。",
        )
    }

    pub(crate) fn integrations_back_description(self) -> &'static str {
        self.integration("Return to the settings list.", "返回设置列表。")
    }

    /// The auxiliary row action that opens one Agent's diagnosis.
    pub(crate) fn integrations_explain(self) -> &'static str {
        self.integration("Explain", "诊断")
    }

    pub(crate) fn integrations_unchecked(self) -> &'static str {
        self.integration(
            "Status not checked yet. Choose Refresh status to read it.",
            "尚未检查状态。选择“刷新状态”读取。",
        )
    }

    pub(crate) fn integration_provider_description(self, agent: &str) -> String {
        match self {
            Self::EnUs => format!(
                "Allows Orchbee to observe {agent} status without reading prompts, terminal output, or tool content."
            ),
            Self::ZhCn => {
                format!("让 Orchbee 观察 {agent} 状态，但不会读取提示词、终端输出或工具内容。")
            }
        }
    }

    pub(crate) fn integration_status(self, status: IntegrationStatus) -> &'static str {
        match status {
            IntegrationStatus::Working => self.integration("Working…", "处理中…"),
            IntegrationStatus::ManualOnly => self.integration("Manual setup", "手动设置"),
            IntegrationStatus::InspectionFailed => {
                self.integration("Status unavailable", "状态不可用")
            }
            IntegrationStatus::NotFound => self.integration("Agent not found", "未找到智能体"),
            IntegrationStatus::NotChecked => self.integration("Not checked", "未检查"),
            IntegrationStatus::Enabled => self.integration("Enabled", "已开启"),
            IntegrationStatus::Disabled => self.integration("Disabled", "已关闭"),
            IntegrationStatus::NotInstalled => self.integration("Not enabled", "未开启"),
            IntegrationStatus::UpdateRequired => {
                self.integration("Integration update needed", "集成需要更新")
            }
            IntegrationStatus::Conflict => self.integration("Conflict", "存在冲突"),
            IntegrationStatus::Unknown => self.integration("Unknown", "未知"),
        }
    }

    pub(crate) fn integration_failure(self, failure: IntegrationFailure) -> &'static str {
        match failure {
            IntegrationFailure::AgentUnavailable => self.integration(
                "This Agent was not found on the local device. Install it first, then refresh.",
                "在本地设备上未找到此智能体。请先安装智能体，然后刷新。",
            ),
            IntegrationFailure::Busy => self.integration(
                "Wait for the current integration operation to finish.",
                "请等待当前集成操作完成。",
            ),
            IntegrationFailure::Conflict => self.integration(
                "Orchbee found a conflicting plugin or file and left it unchanged.",
                "Orchbee 发现了冲突的插件或文件，并且没有改动它。",
            ),
            IntegrationFailure::ControlUnavailable => self.integration(
                "Integration management is unavailable. Reopen Settings and try again.",
                "集成管理暂不可用。请重新打开设置后再试。",
            ),
            IntegrationFailure::ManualActionRequired => self.integration(
                "This step needs manual review in the Agent. No further changes were made automatically.",
                "此步骤需要你在智能体中手动审阅；Orchbee 没有继续自动更改。",
            ),
            IntegrationFailure::OperationFailed => self.integration(
                "The integration command did not complete. The list shows the state detected afterward.",
                "集成命令未能完成；列表中显示的是之后检测到的状态。",
            ),
            IntegrationFailure::VerificationFailed => self.integration(
                "Orchbee could not verify the requested state. Refresh after checking the Agent configuration.",
                "Orchbee 无法验证请求的状态。请检查智能体配置后刷新。",
            ),
        }
    }

    pub(crate) fn integration_block(self, block: IntegrationBlock) -> &'static str {
        match block {
            IntegrationBlock::Pending => self.integration_failure(IntegrationFailure::Busy),
            IntegrationBlock::ManualOnly => self.integration(
                "Orchbee does not manage this integration here; set it up manually in the Agent.",
                "Orchbee 不在此管理该集成；请在智能体中手动设置。",
            ),
            IntegrationBlock::Conflict => self.integration_failure(IntegrationFailure::Conflict),
            IntegrationBlock::NotChecked => self.integration(
                "Not checked: reading this state starts the Agent's own program. Choose Explain to check it.",
                "未检查：读取此状态需要启动智能体自己的程序。选择“诊断”进行检查。",
            ),
            IntegrationBlock::Unknown => self.integration(
                "The state is unknown. Choose Refresh status, or Explain to see why.",
                "状态未知。请选择“刷新状态”，或选择“诊断”查看原因。",
            ),
            IntegrationBlock::AgentMissing => {
                self.integration_failure(IntegrationFailure::AgentUnavailable)
            }
        }
    }

    pub(crate) fn integration_diagnosis_pending(self) -> &'static str {
        self.integration(
            "Checking this integration on this device…",
            "正在检查本设备上的此集成…",
        )
    }

    /// The closing sentence of a check that read files only.
    pub(crate) fn integration_diagnosis_read_only(self) -> &'static str {
        self.integration(
            "Read-only check: nothing was changed and no Agent program was started.",
            "只读检查：未做任何更改，也未启动任何智能体程序。",
        )
    }

    /// The closing sentence of a check that may start the Agent's program.
    pub(crate) fn integration_diagnosis_started(self, agent: &str) -> String {
        match self {
            Self::EnUs => format!(
                "Orchbee changed nothing. This check starts {agent} when needed to read its plugins, and {agent} may create its own files."
            ),
            Self::ZhCn => format!(
                "Orchbee 未做任何更改。此检查会在需要时启动 {agent} 读取其插件，{agent} 可能会创建自己的文件。"
            ),
        }
    }

    /// The question before a check that starts the Agent's program.
    pub(crate) fn integration_consent_title(self, agent: &str) -> String {
        match self {
            Self::EnUs => format!("Check {agent}?"),
            Self::ZhCn => format!("检查 {agent}？"),
        }
    }

    pub(crate) fn integration_consent(self, agent: &str) -> String {
        match self {
            Self::EnUs => format!(
                "This check starts {agent} to read its plugins, and {agent} may create its own files when it starts. Orchbee changes nothing. Choose Explain to start the check."
            ),
            Self::ZhCn => format!(
                "此检查会启动 {agent} 读取其插件，{agent} 启动时可能会创建自己的文件。Orchbee 不做任何更改。选择“诊断”开始检查。"
            ),
        }
    }

    /// The headline: a definite finding is named as such; an unknown result says it is unknown.
    pub(crate) fn integration_finding(self, finding: IntegrationFinding) -> &'static str {
        match finding {
            IntegrationFinding::Ready => self.integration("Ready", "可用"),
            IntegrationFinding::ManualSetup => self.integration("Manual setup", "手动设置"),
            IntegrationFinding::AgentMissing => {
                self.integration("Definitely missing: the Agent", "确定缺失：智能体")
            }
            IntegrationFinding::NotInstalled => {
                self.integration("Definitely missing: the integration", "确定缺失：集成")
            }
            IntegrationFinding::Disabled => self.integration("Turned off", "已关闭"),
            IntegrationFinding::UpdateRequired => self.integration("Update needed", "需要更新"),
            IntegrationFinding::Conflict => self.integration("Blocked by a conflict", "存在冲突"),
            IntegrationFinding::Unknown(_) => self.integration("Result unknown", "结果未知"),
        }
    }

    pub(crate) fn integration_finding_detail(self, finding: IntegrationFinding) -> &'static str {
        match finding {
            IntegrationFinding::Ready => self.integration(
                "The integration is enabled, so Orchbee can show this Agent's status.",
                "集成已开启，Orchbee 可以显示此智能体的状态。",
            ),
            IntegrationFinding::ManualSetup => {
                self.integration_block(IntegrationBlock::ManualOnly)
            }
            IntegrationFinding::AgentMissing => {
                self.integration_failure(IntegrationFailure::AgentUnavailable)
            }
            IntegrationFinding::NotInstalled => self.integration(
                "The Agent is here, but its integration is not installed. Turn it on to install it.",
                "已找到智能体，但其集成尚未安装。开启后即可安装。",
            ),
            IntegrationFinding::Disabled => self.integration(
                "The integration is installed but turned off. Turn it on to use it.",
                "集成已安装但处于关闭状态。开启后即可使用。",
            ),
            IntegrationFinding::UpdateRequired => self.integration(
                "The installed integration is out of date. Turn it on to update it.",
                "已安装的集成版本过旧。开启后即可更新。",
            ),
            IntegrationFinding::Conflict => self.integration_failure(IntegrationFailure::Conflict),
            IntegrationFinding::Unknown(IntegrationUnknown::TimedOut) => self.integration(
                "The check ran out of time, so the state is unknown, not missing. Refresh to check again.",
                "检查超时，因此状态未知，并非确定缺失。请刷新后再检查。",
            ),
            IntegrationFinding::Unknown(IntegrationUnknown::InspectionFailed) => self.integration(
                "Orchbee could not read this integration, so the state is unknown, not missing. Check the Agent configuration, then refresh.",
                "Orchbee 无法读取此集成，因此状态未知，并非确定缺失。请检查智能体配置后刷新。",
            ),
            IntegrationFinding::Unknown(IntegrationUnknown::Undetermined) => self.integration(
                "Orchbee could not determine the state, so it is unknown, not missing. Refresh to check again.",
                "Orchbee 无法确定状态，因此状态未知，并非确定缺失。请刷新后再检查。",
            ),
        }
    }

    pub(crate) fn integration_fact_label(self, label: IntegrationFactLabel) -> &'static str {
        match label {
            IntegrationFactLabel::Agent => {
                self.integration("Agent on this device", "本设备上的智能体")
            }
            IntegrationFactLabel::Integration => self.integration("Integration", "集成"),
            IntegrationFactLabel::Issue => self.integration("Issue", "问题"),
            IntegrationFactLabel::Adapter => self.integration("Orchbee adapter", "Orchbee 适配器"),
            IntegrationFactLabel::Marketplace => self.integration("Plugin marketplace", "插件市场"),
            IntegrationFactLabel::Plugin => self.integration("Plugin", "插件"),
            IntegrationFactLabel::Entry => self.integration("Integration entry", "集成条目"),
            IntegrationFactLabel::AdapterSource => self.integration("Adapter source", "适配器来源"),
            IntegrationFactLabel::Plan => self.integration("Turning on would", "开启将会"),
        }
    }

    /// Joins a fact label and its value as the locale writes them.
    pub(crate) fn integration_fact(self, label: IntegrationFactLabel, value: &str) -> String {
        match self {
            Self::EnUs => format!("{}: {value}", self.integration_fact_label(label)),
            Self::ZhCn => format!("{}：{value}", self.integration_fact_label(label)),
        }
    }

    pub(crate) fn integration_presence(self, availability: Availability) -> &'static str {
        match availability {
            Availability::Available => self.integration("Found", "已找到"),
            Availability::NotFound => self.integration("Not found", "未找到"),
            Availability::Unknown => self.integration("Unknown", "未知"),
        }
    }

    pub(crate) fn integration_issue(self, issue: Issue) -> &'static str {
        match issue {
            Issue::InspectionFailed => self.integration_status(IntegrationStatus::InspectionFailed),
            Issue::ManualActionRequired => {
                self.integration("Needs manual review in the Agent", "需要在智能体中手动审阅")
            }
            Issue::UnsupportedPlatform => {
                self.integration("Not supported on this platform", "此平台不支持")
            }
            Issue::UnsupportedVersion => {
                self.integration("This Agent version is not supported", "不支持此智能体版本")
            }
        }
    }

    pub(crate) fn integration_adapter(self, adapter: IntegrationAdapter) -> &'static str {
        match adapter {
            IntegrationAdapter::Ready => self.integration("Ready", "就绪"),
            IntegrationAdapter::NotReady => self.integration("Not ready", "未就绪"),
            IntegrationAdapter::SourceAbsent => {
                self.integration("Not included in this installation", "此安装未包含")
            }
            IntegrationAdapter::BindingFailed => self.integration(
                "Does not match this Orchbee version",
                "与此 Orchbee 版本不匹配",
            ),
        }
    }

    pub(crate) fn integration_marketplace(self, marketplace: Option<Marketplace>) -> &'static str {
        match marketplace {
            Some(Marketplace::Current) => self.integration("Current", "最新"),
            Some(Marketplace::Outdated) => self.integration("Update needed", "需要更新"),
            Some(Marketplace::Absent) => self.integration("Not added", "未添加"),
            None => self.integration("Unknown", "未知"),
        }
    }

    pub(crate) fn integration_presence_flag(self, present: Option<bool>) -> &'static str {
        match present {
            Some(true) => self.integration("Installed", "已安装"),
            Some(false) => self.integration("Not installed", "未安装"),
            None => self.integration("Unknown", "未知"),
        }
    }

    pub(crate) fn integration_entry(self, target: TargetState) -> &'static str {
        match target {
            TargetState::Current => self.integration("Current", "最新"),
            TargetState::Missing => self.integration("Missing", "缺失"),
            TargetState::UpdateRequired => self.integration("Update needed", "需要更新"),
            TargetState::Conflict => self.integration("Conflict", "存在冲突"),
            TargetState::Unknown => self.integration("Unknown", "未知"),
        }
    }

    pub(crate) fn integration_source(self, ready: bool) -> &'static str {
        if ready {
            self.integration_adapter(IntegrationAdapter::Ready)
        } else {
            self.integration_adapter(IntegrationAdapter::NotReady)
        }
    }

    fn integration_command_purpose(self, purpose: Option<CommandPurpose>) -> &'static str {
        match purpose {
            Some(CommandPurpose::AddPluginSource) => {
                self.integration("add Orchbee's plugin source", "添加 Orchbee 插件来源")
            }
            Some(CommandPurpose::RemovePluginSource) => {
                self.integration("remove the old plugin source", "移除旧插件来源")
            }
            Some(CommandPurpose::InstallPlugin) => {
                self.integration("install the Orchbee plugin", "安装 Orchbee 插件")
            }
            Some(CommandPurpose::RemovePlugin) => {
                self.integration("remove the old plugin", "移除旧插件")
            }
            Some(CommandPurpose::EnablePlugin) => {
                self.integration("turn the plugin on", "开启插件")
            }
            Some(CommandPurpose::ConfirmPlugin) => {
                self.integration("confirm the plugin is on", "确认插件已开启")
            }
            None => self.integration("run a plugin command", "执行插件命令"),
        }
    }

    /// A conjunction list as the GUI's `Intl.ListFormat` writes it for each locale.
    fn integration_conjunction(self, items: &[&str]) -> String {
        match (self, items) {
            (_, []) => String::new(),
            (_, [only]) => (*only).to_owned(),
            (Self::EnUs, [first, second]) => format!("{first} and {second}"),
            (Self::ZhCn, [first, second]) => format!("{first}和{second}"),
            (Self::EnUs, [head @ .., last]) => format!("{}, and {last}", head.join(", ")),
            (Self::ZhCn, [head @ .., last]) => format!("{}和{last}", head.join("、")),
        }
    }

    /// The enable plan as one sentence fragment; an empty plan makes no change. Consecutive
    /// commands of one program name it once and list what they do; a repeated step shows once.
    pub(crate) fn integration_plan(self, plan: &[IntegrationStep]) -> String {
        if plan.is_empty() {
            return self.integration("make no changes", "不做任何更改").into();
        }
        let mut steps: Vec<String> = Vec::new();
        let mut index = 0;
        while index < plan.len() {
            let step = &plan[index];
            index += 1;
            let text = match step {
                IntegrationStep::PublishAdapter => self
                    .integration("publish the Orchbee adapter", "发布 Orchbee 适配器")
                    .to_owned(),
                IntegrationStep::Command { program, purpose } => {
                    let mut purposes = vec![self.integration_command_purpose(*purpose)];
                    while let Some(IntegrationStep::Command {
                        program: next,
                        purpose,
                    }) = plan.get(index)
                        && next == program
                    {
                        let text = self.integration_command_purpose(*purpose);
                        if !purposes.contains(&text) {
                            purposes.push(text);
                        }
                        index += 1;
                    }
                    let purposes = self.integration_conjunction(&purposes);
                    match self {
                        Self::EnUs => format!("use {program} to {purposes}"),
                        Self::ZhCn => format!("用 {program} {purposes}"),
                    }
                }
                IntegrationStep::WriteEntry => self
                    .integration("write the integration entry", "写入集成条目")
                    .to_owned(),
                IntegrationStep::RemoveEntry => self
                    .integration("remove an obsolete entry", "移除过时条目")
                    .to_owned(),
            };
            if steps.last() != Some(&text) {
                steps.push(text);
            }
        }
        steps.join(self.integration("; ", "；"))
    }
}
