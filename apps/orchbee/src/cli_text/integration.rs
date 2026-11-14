//! Localized Agent integration inspection and mutation results; does not change integrations.

use super::Locale;

pub(super) const HELP_AGENT_INTEGRATION_LIST_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent integration list [--provider <id>] [--outdated-only] [--format text|json]",
        "Inspect every supported provider's local integration state — or only the\none selected by '--provider'. Reads files only, with this shell's home\nand PATH; starts or contacts no Session and starts no Agent program, so a\nprovider whose plugins only its own program reports stays not checked;\nnothing is written. Windows --outdated-only shows owned updates/repairs, preserving\nunknown and blocked provider diagnostics. Use enable to apply a guarded repair.\n'--format json' emits the orchbee.agent.integrations envelope.",
    );

pub(super) const HELP_AGENT_INTEGRATION_LIST_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent integration list [--provider <id>] [--outdated-only] [--format text|json]",
        "检查每个受支持 provider 的本地集成状态，或仅检查 '--provider' 所选的一个。只按当前 shell 的主目录和 PATH 读取文件，不启动也不连接 Session，不启动任何智能体程序，因此插件状态只能由自身程序报告的 provider 保持未检查；不写入任何内容。\nWindows --outdated-only 只显示需更新／修复的自有条目，保留未知和冲突诊断。使用 enable 显式执行受保护的修复。'--format json' 输出 orchbee.agent.integrations 封套。",
    );

pub(super) const HELP_AGENT_INTEGRATION_EXPLAIN_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent integration explain --provider <id> [--format text|json]",
        "Explain one provider's local integration state and the guard-evaluated\nplan 'enable' would run for the observed dimensions. Starts the local\nSession when none is running. Nothing is written, but for claude-code,\ngrok, github-copilot and Windows hermes its Worker starts the provider's\nown program from the Session's PATH to read its plugins, and that program\nmay create its own files. '--format json' emits the\norchbee.agent.integration.plan envelope.",
    );

pub(super) const HELP_AGENT_INTEGRATION_EXPLAIN_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent integration explain --provider <id> [--format text|json]",
        "解释一个 provider 的本地集成状态，以及针对观测到的维度 'enable' 将执行的、经守卫求值的计划。没有运行中的本地 Session 时会先启动它。不写入任何内容，但对于 claude-code、grok、github-copilot 和 Windows 上的 hermes，其 Worker 会从 Session PATH 启动 provider 自身的程序来读取其插件，该程序可能会创建自己的文件。'--format json' 输出\norchbee.agent.integration.plan 封套。",
    );

pub(super) const HELP_AGENT_INTEGRATION_ENABLE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent integration enable --provider <id> [--format text|json]",
        "Enable one provider's managed observation integration through the local\nSession, starting it when none is running: its Worker publishes the\nadapter capsule when needed, then installs its guarded module or runs\nbounded non-interactive provider setup commands. Manual-only entries and\nunavailable providers refuse before effects. Foreign, modified or unreadable entries\nare never replaced; the receipt retains attempted effects and\nthe state observed by re-inspection. '--format json' emits the\norchbee.agent.integration.mutation envelope.",
    );

pub(super) const HELP_AGENT_INTEGRATION_ENABLE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent integration enable --provider <id> [--format text|json]",
        "通过本地 Session 启用一个 provider 的受管观测集成，没有运行中的 Session 时会先启动它：由其 Worker 在必要时先发布 adapter capsule，然后安装受保护的模块，或执行有界、非交互的 provider 设置命令。仅手动管理或不可用的 provider 在产生效应前拒绝。不会替换外来、被修改或不可读的条目；回执保留已尝试的效应和复检观察到的状态。\n'--format json' 输出 orchbee.agent.integration.mutation 封套。",
    );

pub(super) const HELP_AGENT_INTEGRATION_DISABLE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent integration disable --provider <id> [--format text|json]",
        "Disable one provider's managed observation integration through the local\nSession, starting it when none is running: its Worker runs the provider's\nown bounded removal commands or guarded removal of the owned\nentry. Only entries verifiably owned by Orchbee are ever removed; foreign\nor newer-versioned entries are refused before any effect. '--format json'\nemits the orchbee.agent.integration.mutation envelope.",
    );

pub(super) const HELP_AGENT_INTEGRATION_DISABLE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee agent integration disable --provider <id> [--format text|json]",
        "通过本地 Session 禁用一个 provider 的受管观测集成，没有运行中的 Session 时会先启动它：由其 Worker 执行 provider 自身的有界移除命令，或对所有物执行受保护的移除。只有经核实归 Orchbee 所有的条目才会被移除；外来或更新版本的条目在任何效应前被拒绝。'--format json'\n输出 orchbee.agent.integration.mutation 封套。",
    );

impl super::CliText {
    /// `agent integration list` capsule summary line — the machine capsule
    /// state stays English; only the sentence frame is localized.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_maintenance(
        self,
        inspected: usize,
        outdated: usize,
        unresolved: &[&str],
        blocked: &[&str],
    ) -> String {
        let unresolved = if unresolved.is_empty() {
            "-".into()
        } else {
            unresolved.join(", ")
        };
        let blocked = if blocked.is_empty() {
            "-".into()
        } else {
            blocked.join(", ")
        };
        match self.locale {
            Locale::EnUs => format!(
                "Inspected {inspected}; update or repair required: {outdated}.\nUnknown inspections: {unresolved}\nConflicting or newer installations: {blocked}\nUse agent integration explain --provider <id> to inspect the repair plan.\n"
            ),
            Locale::ZhCn => format!(
                "已检查 {inspected} 项；需更新或修复 {outdated} 项。\n检查未知：{unresolved}\n冲突或更新版本安装：{blocked}\n使用 agent integration explain --provider <id> 查看修复计划。\n"
            ),
        }
    }

    /// One closing conclusion for provider readiness listings, plus the one
    /// `agent integration` command that moves the first unready provider on.
    pub(crate) fn cli_integration_summary(
        self,
        counts: [usize; 4],
        not_checked: usize,
        next: Option<(&str, &str)>,
    ) -> String {
        let [ready, setup, attention, missing] = counts;
        let mut out = match (self.locale, not_checked) {
            (Locale::EnUs, 0) => format!(
                "Summary: {ready} ready, {setup} need setup, {attention} need attention, {missing} not installed.\n"
            ),
            (Locale::EnUs, _) => format!(
                "Summary: {ready} ready, {setup} need setup, {attention} need attention, {not_checked} not checked, {missing} not installed.\n"
            ),
            (Locale::ZhCn, 0) => format!(
                "汇总：{ready} 个就绪，{setup} 个待设置，{attention} 个需处理，{missing} 个未安装。\n"
            ),
            (Locale::ZhCn, _) => format!(
                "汇总：{ready} 个就绪，{setup} 个待设置，{attention} 个需处理，{not_checked} 个未检查，{missing} 个未安装。\n"
            ),
        };
        if not_checked > 0 {
            out.push_str(match self.locale {
                Locale::EnUs => {
                    "Not checked: list starts no Agent program; explain checks one by starting it.\n"
                }
                Locale::ZhCn => "未检查：list 不启动任何智能体程序；explain 会启动智能体来检查它。\n",
            });
        }
        if let Some((verb, provider)) = next {
            out.push_str(&match self.locale {
                Locale::EnUs => {
                    format!("Next: orchbee agent integration {verb} --provider {provider}\n")
                }
                Locale::ZhCn => {
                    format!("下一步：orchbee agent integration {verb} --provider {provider}\n")
                }
            });
        }
        out
    }

    pub(crate) fn cli_integration_capsule_line(self, summary: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("Adapter capsule: {summary}\n"),
            Locale::ZhCn => format!("适配器胶囊：{summary}\n"),
        }
    }

    /// `agent integration explain` header — the fixed-field first lines.
    /// Vocabulary values stay in their canonical English spelling.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_explain_header(
        self,
        provider: &str,
        state: &str,
        availability: &str,
        issue: &str,
        capsule: &str,
    ) -> String {
        match self.locale {
            Locale::EnUs => format!(
                "Provider: {provider}\nState: {state}  availability: {availability}  issue: {issue}\nCapsule: {capsule}\n"
            ),
            Locale::ZhCn => format!(
                "提供方：{provider}\n状态：{state}  可用性：{availability}  问题：{issue}\n胶囊：{capsule}\n"
            ),
        }
    }

    /// `explain`'s closing line: what the check changed and which Agent program it may start.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_explain_effects(self, started: Option<&str>) -> String {
        match (self.locale, started) {
            (Locale::EnUs, None) => {
                "Read-only check: nothing was changed and no Agent program was started.\n".into()
            }
            (Locale::ZhCn, None) => "只读检查：未做任何更改，也未启动任何智能体程序。\n".into(),
            (Locale::EnUs, Some(provider)) => format!(
                "Orchbee changed nothing. This check starts {provider} when needed to read its plugins, and {provider} may create its own files.\n"
            ),
            (Locale::ZhCn, Some(provider)) => format!(
                "Orchbee 未做任何更改。此检查会在需要时启动 {provider} 读取其插件，{provider} 可能会创建自己的文件。\n"
            ),
        }
    }

    #[cfg(any(unix, windows))]
    pub(crate) const fn cli_integration_plan_header(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Plan:\n",
            Locale::ZhCn => "计划：\n",
        }
    }

    /// `explain` when every pre-effect gate refuses the vector.
    #[cfg(any(unix, windows))]
    pub(crate) const fn cli_integration_plan_empty(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Plan: no steps — the observed state admits no enable vector.\n",
            Locale::ZhCn => "计划：无步骤——当前状态不接受任何启用向量。\n",
        }
    }

    /// Mutation completed with the goal state observed.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_completed(self, direction: &str, provider: &str) -> String {
        match (self.locale, direction) {
            (Locale::EnUs, "enable") => format!("Enabled {provider}.\n"),
            (Locale::EnUs, _) => format!("Disabled {provider}.\n"),
            (Locale::ZhCn, "enable") => format!("已启用 {provider}。\n"),
            (Locale::ZhCn, _) => format!("已禁用 {provider}。\n"),
        }
    }

    /// A pre-effect refusal — the canonical refusal code stays English.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_refused(
        self,
        direction: &str,
        provider: &str,
        reason: &str,
    ) -> String {
        match (self.locale, direction) {
            (Locale::EnUs, "enable") => {
                format!("Refused to enable {provider} ({reason}); nothing was changed.\n")
            }
            (Locale::EnUs, _) => {
                format!("Refused to disable {provider} ({reason}); nothing was changed.\n")
            }
            (Locale::ZhCn, "enable") => {
                format!("拒绝启用 {provider}（{reason}）；未做任何改动。\n")
            }
            (Locale::ZhCn, _) => {
                format!("拒绝禁用 {provider}（{reason}）；未做任何改动。\n")
            }
        }
    }

    /// A post-effect failure — the typed code and the re-inspected state.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_failed(
        self,
        direction: &str,
        provider: &str,
        code: &str,
        observed: Option<&str>,
    ) -> String {
        let observed = observed.unwrap_or("unknown");
        match (self.locale, direction) {
            (Locale::EnUs, "enable") => {
                format!("Enable of {provider} failed ({code}); observed state: {observed}.\n")
            }
            (Locale::EnUs, _) => {
                format!("Disable of {provider} failed ({code}); observed state: {observed}.\n")
            }
            (Locale::ZhCn, "enable") => {
                format!("启用 {provider} 失败（{code}）；观察到的状态：{observed}。\n")
            }
            (Locale::ZhCn, _) => {
                format!("禁用 {provider} 失败（{code}）；观察到的状态：{observed}。\n")
            }
        }
    }

    /// The verdict could not be determined — re-inspection never ran.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_unknown(self, direction: &str, provider: &str) -> String {
        match (self.locale, direction) {
            (Locale::EnUs, "enable") => format!(
                "Could not determine whether {provider} was enabled; the post-effect re-inspection did not run.\n"
            ),
            (Locale::EnUs, _) => format!(
                "Could not determine whether {provider} was disabled; the post-effect re-inspection did not run.\n"
            ),
            (Locale::ZhCn, "enable") => {
                format!("无法确定 {provider} 是否已启用；效应后复检未运行。\n")
            }
            (Locale::ZhCn, _) => {
                format!("无法确定 {provider} 是否已禁用；效应后复检未运行。\n")
            }
        }
    }

    /// The termination signal fired — the invocation cancelled at a frontier
    /// boundary.
    #[cfg(any(unix, windows))]
    pub(crate) fn cli_integration_cancelled(self, direction: &str, provider: &str) -> String {
        match (self.locale, direction) {
            (Locale::EnUs, "enable") => format!("Enable of {provider} cancelled.\n"),
            (Locale::EnUs, _) => format!("Disable of {provider} cancelled.\n"),
            (Locale::ZhCn, "enable") => format!("已取消启用 {provider}。\n"),
            (Locale::ZhCn, _) => format!("已取消禁用 {provider}。\n"),
        }
    }

    /// The `attempted` list's leading word in text output.
    #[cfg(any(unix, windows))]
    pub(crate) const fn cli_integration_attempted_word(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "attempted",
            Locale::ZhCn => "已尝试",
        }
    }

    /// Header above the residue listing — retained paths are named, never
    /// silently swept.
    #[cfg(any(unix, windows))]
    pub(crate) const fn cli_integration_residue_header(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Retained residue (not removed):\n",
            Locale::ZhCn => "保留的残留（未删除）：\n",
        }
    }
}
