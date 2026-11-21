//! Localized session connection, display, and mutation feedback; does not change session state.

use super::Locale;
use crate::agent_provider::IntegrationId;
mod parameters;
pub(crate) use parameters::{
    ClientProjectionStatus, DisplayController, DisplayProvenance, DisplayRetirement,
    DuplicateProgress, DuplicateStop, LaunchFailure, LaunchProfile, NoticeContext, RefreshStatus,
    WorkbenchClientNotice,
};
pub use parameters::{DisplayPhase, TerminalStatus};

impl Locale {
    /// The provider's display name, shared by every locale's Agent messages.
    pub(crate) const fn agent_provider_name(
        self,
        id: crate::agent_provider::IntegrationId,
    ) -> &'static str {
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

    pub const fn launch_request_too_large(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Launch request too large. Shorten the custom program's arguments or environment variables before trying again."
            }
            Self::ZhCn => "启动请求过大。请缩短自定义程序的参数或环境变量后再试。",
        }
    }
    pub(crate) const fn terminal_status(
        self,
        status: crate::localization::TerminalStatus,
    ) -> &'static str {
        use crate::localization::TerminalStatus;
        match (self, status) {
            (_, TerminalStatus::Connecting | TerminalStatus::Blocked) => self.connecting_status(),
            (Self::EnUs, TerminalStatus::Controlling) => "Terminal connected",
            (Self::ZhCn, TerminalStatus::Controlling) => "终端已连接",
            (_, TerminalStatus::ReadOnly) => self.send_typing_here(),
            (Self::EnUs, TerminalStatus::Unavailable) => "Terminal disconnected",
            (Self::ZhCn, TerminalStatus::Unavailable) => "终端已断开",
            (_, TerminalStatus::OutcomeUnknown) => self.outcome_unknown(),
        }
    }

    /// Format only the selected duplication observation; priority belongs to Workbench. A finished
    /// duplication shows itself and says nothing.
    pub(crate) fn duplication_progress(self, progress: DuplicateProgress) -> Option<String> {
        Some(match (progress.stop, progress.complete) {
            (_, true) => return None,
            (None, false) => self.say("Duplicating Tabs…", "正在复制标签页…"),
            (Some(DuplicateStop::TerminalNotStarted | DuplicateStop::Recovery), _) => {
                self.outcome_unknown().to_owned()
            }
            (
                Some(DuplicateStop::Changed | DuplicateStop::Deadline | DuplicateStop::NotCreated),
                _,
            ) => self.say("Couldn't duplicate the Tabs.", "没能复制标签页。"),
        })
    }

    pub(crate) const fn native_unbound_terminal(self) -> &'static str {
        match self {
            Self::EnUs => "No terminal here",
            Self::ZhCn => "这里没有终端",
        }
    }

    /// Render the feature-selected phase facts without inspecting or retaining a view.
    pub(crate) fn native_phase_status_values(
        self,
        phase: crate::localization::DisplayPhase,
        label: Option<&str>,
        controller: Option<crate::localization::DisplayController>,
        provenance: Option<crate::localization::DisplayProvenance>,
        retirement: Option<crate::localization::DisplayRetirement>,
    ) -> String {
        use crate::localization::{
            DisplayController as Controller, DisplayPhase as Phase,
            DisplayProvenance as Provenance, DisplayRetirement as Retirement,
        };

        if let Some(retirement) = retirement {
            return match retirement {
                Retirement::LaunchRequestTooLarge => self.launch_request_too_large().to_owned(),
                Retirement::OutcomeUnknown => self
                    .terminal_status(crate::localization::TerminalStatus::OutcomeUnknown)
                    .to_owned(),
                Retirement::Unavailable => self
                    .terminal_status(crate::localization::TerminalStatus::Unavailable)
                    .to_owned(),
            };
        }

        match phase {
            Phase::Empty => match self {
                Self::EnUs => "No terminal tabs".to_owned(),
                Self::ZhCn => "没有终端标签页".to_owned(),
            },
            Phase::Starting => self.native_starting_status(label),
            Phase::Switching => self.native_switching_status(label),
            Phase::Ready => {
                let Some(label) = label else {
                    return self
                        .terminal_status(crate::localization::TerminalStatus::Unavailable)
                        .to_owned();
                };
                match controller {
                    Some(Controller::Controlling) => self.native_ready_controlling(label),
                    Some(Controller::ReadOnly) => self.native_ready_read_only(label),
                    None => self.native_terminal_unavailable(label),
                }
            }
            Phase::Creating | Phase::CreateCommittedNeedDetach => self.native_creating().to_owned(),
            // The change landed but this window cannot show it until the person refreshes; the
            // status line offers the refresh key beside it.
            Phase::ProjectionUnavailable
            | Phase::PanelProjectionUnavailable
            | Phase::RenameProjectionUnavailable => {
                self.say("Couldn't show the latest change.", "没能显示最新的更改。")
            }
            Phase::SelectedWithoutAttachment => {
                let Some(label) = label else {
                    return self
                        .terminal_status(crate::localization::TerminalStatus::Unavailable)
                        .to_owned();
                };
                match provenance {
                    Some(Provenance::Pending) => self.native_terminal_pending(label),
                    Some(Provenance::Unavailable) => self.native_terminal_unavailable(label),
                    Some(Provenance::Failed) => self.native_terminal_failed(label),
                    Some(Provenance::OutcomeUnknown) => self.native_terminal_outcome_unknown(label),
                    Some(Provenance::Started | Provenance::BindingOnly) | None => {
                        self.native_terminal_unavailable(label)
                    }
                }
            }
            Phase::RetryableAttachUnavailable => match (label, provenance) {
                // A committed launch failure observed after selection ends the retry story.
                (Some(label), Some(Provenance::Failed)) => self.native_terminal_failed(label),
                (Some(label), _) => self.native_terminal_unavailable(label),
                (None, _) => self
                    .terminal_status(crate::localization::TerminalStatus::Unavailable)
                    .to_owned(),
            },
            Phase::Retired => self
                .terminal_status(crate::localization::TerminalStatus::Unavailable)
                .to_owned(),
        }
    }

    pub(crate) fn native_refresh_status(
        self,
        status: crate::localization::RefreshStatus,
    ) -> String {
        use crate::localization::RefreshStatus;
        match status {
            RefreshStatus::Refreshing => self.native_refreshing().to_owned(),
            RefreshStatus::Success => String::new(),
            RefreshStatus::Failure => self.say("Couldn't refresh the Tabs.", "没能刷新标签页。"),
        }
    }

    /// A notice as the status line states it, with only the matching operation's sanitized display
    /// context, then the forms that replace it when it does not fit, longest first. The shorter
    /// form keeps the context, then stands alone, so the outcome always shows and the context
    /// shows while it fits.
    pub(crate) fn native_notice_forms(
        self,
        notice: crate::localization::WorkbenchClientNotice,
        context: Option<&NoticeContext>,
    ) -> Vec<String> {
        let full = self.native_notice(notice);
        match (context, self.native_notice_shorter(notice)) {
            (None, None) => vec![full],
            (None, Some(shorter)) => vec![full, shorter],
            (Some(context), None) => vec![self.with_notice_context(&full, context), full],
            (Some(context), Some(shorter)) => vec![
                self.with_notice_context(&full, context),
                self.with_notice_context(&shorter, context),
                shorter,
            ],
        }
    }

    fn with_notice_context(self, message: &str, context: &NoticeContext) -> String {
        match context {
            NoticeContext::LaunchProfile(name) => {
                format!("{message} · {}", self.launch_profile_label(name.as_deref()))
            }
            NoticeContext::ConfiguredCommand(id) => format!(
                "{} · {message}",
                crate::terminal_text::sanitize_terminal_text(id)
            ),
        }
    }

    /// The documented shorter form that replaces a notice too wide for the status line. Every
    /// notice whose full text can exceed the status line at the Compact minimum width has one: it
    /// keeps the outcome and drops the advice.
    pub(crate) fn native_notice_shorter(
        self,
        notice: crate::localization::WorkbenchClientNotice,
    ) -> Option<String> {
        use crate::localization::WorkbenchClientNotice as N;
        match notice {
            N::RecordUnusable(record, problem) => Some(self.record_status(record, problem)),
            N::AgentStart(phase) => self.agent_start_phase_shorter(phase),
            N::Settings(notice) => self.settings_notice_shorter(notice).map(str::to_owned),
            N::Unsupported(capability) => Some(self.unsupported_capability_shorter(capability)),
            N::LaunchRequestTooLarge => Some(self.say("Launch request too large", "启动请求过大")),
            N::TerminalLimitReached => Some(self.say("Terminal limit reached", "已达终端上限")),
            N::NoWorkspace => Some(self.say("No workspace open", "当前未打开工作区")),
            N::LaunchProfileUnavailable => {
                Some(self.say("Couldn't create the terminal.", "没能创建终端。"))
            }
            N::CopyModeSearchLimit => Some(self.say("Couldn't search for that.", "没能搜索。")),
            N::PopupUnavailable => Some(self.say(
                "Popup connection or cleanup uncertain",
                "popup 连接或清理状态不确定",
            )),
            N::CommandRefused(
                orchbee_workbench_protocol::ConfiguredCommandFailure::SourceExited,
            ) => Some(self.say("Terminal exited.", "终端已退出。")),
            _ => None,
        }
    }

    /// A resume that typed nothing, naming the Agent when it is known.
    fn agent_not_resumed(self, agent: Option<crate::agent_provider::IntegrationId>) -> String {
        match (self, agent) {
            (Self::EnUs, Some(id)) => format!(
                "Couldn't resume the {} conversation.",
                self.agent_provider_name(id)
            ),
            (Self::EnUs, None) => "Couldn't resume the Agent conversation.".into(),
            (Self::ZhCn, Some(id)) => {
                format!("没能续接 {} 对话。", self.agent_provider_name(id))
            }
            (Self::ZhCn, None) => "没能续接智能体对话。".into(),
        }
    }

    pub(crate) fn native_notice(
        self,
        notice: crate::localization::WorkbenchClientNotice,
    ) -> String {
        use crate::localization::WorkbenchClientNotice as N;
        let say = |english: &str, chinese: &str| self.say(english, chinese);
        match notice {
            N::Unsupported(capability) => self.unsupported_capability(capability),
            N::Settings(notice) => self.settings_notice(notice).to_owned(),
            N::AgentStart(phase) => self.agent_start_phase(phase),
            N::LocalBusy => say("Couldn't run the action.", "没能执行操作。"),
            N::MenuUnavailable => say("This isn't available right now.", "此操作现在不可用。"),
            N::TerminalClearRefused | N::TerminalClearUnavailable => {
                say("Couldn't clear the terminal.", "没能清除终端。")
            }
            N::TerminalInputNotSent => self.terminal_input_not_sent().to_owned(),
            N::TerminalResizeNotDone => say("Couldn't resize the terminal.", "没能调整终端大小。"),
            N::TerminalClearUnconfirmed
            | N::TerminalClearUnknown
            | N::TerminalActionUnknown
            | N::DuplicatePending
            | N::DuplicateOutcomeUnknown
            | N::ChangeOutcomeUnknown
            | N::CommandUnknown
            | N::CommandRefused(
                orchbee_workbench_protocol::ConfiguredCommandFailure::PhysicalUnknown,
            )
            | N::LinkUnknown
            | N::CopyUnknown
            | N::BellUnknown => self.outcome_unknown().to_owned(),
            N::NoTabAtPosition(position) => self.native_no_tab(position),
            N::CreateNotCreated => self.native_create_not_created().to_owned(),
            N::LaunchRequestTooLarge => self.launch_request_too_large().to_owned(),
            N::TerminalLimitReached => say(
                "This session reached its 1,024-terminal limit. Close some terminals to open new ones.",
                "此会话已达到 1,024 个终端的上限，请关闭一些终端后再打开新终端。",
            ),
            N::CreateOrderConflict => {
                say("Couldn't create the Tab there.", "没能在这里创建标签页。")
            }
            N::DuplicateNotStarted | N::RestoreNotStarted => {
                say("Couldn't start the terminal.", "没能启动终端。")
            }
            N::CloseRequestNotApplied | N::ConfirmationCancelled => {
                say("Couldn't close it.", "没能关闭。")
            }
            N::WorkspaceCloseUnavailable => {
                say("Select a Workspace to close.", "请选择要关闭的工作区。")
            }
            N::WorkspaceChanged | N::DraftCancelled => {
                say("Couldn't apply the change.", "没能应用更改。")
            }
            N::AgentUnavailable => say("Couldn't go to the Agent.", "没能跳到这个智能体。"),
            N::NoWorkspace => self.no_current_workspace().to_owned(),
            N::WorkspaceNotReady => say(
                "Couldn't start the worktree action.",
                "没能开始工作树操作。",
            ),
            N::RenameNotRenamed => say("Couldn't rename the Tab.", "没能重命名标签页。"),
            N::PanelNotApplied => say("Couldn't change the Panel.", "没能更改面板。"),
            N::DialogRefused => self.presentation_notice(false).to_owned(),
            N::ModalOwnsInput => say("Close the open dialog first.", "请先关闭当前对话框。"),
            N::CommandRefused(
                orchbee_workbench_protocol::ConfiguredCommandFailure::SourceExited,
            ) => say(
                "Terminal exited. Focus a running Panel.",
                "终端已退出，请聚焦运行中的面板。",
            ),
            N::CommandFailed | N::CommandRefused(_) => {
                say("Couldn't run the command.", "没能运行命令。")
            }
            N::LaunchProfileUnavailable => say(
                "Couldn't create the terminal. Check its launch profile in Settings.",
                "没能创建终端。请在设置中检查启动配置。",
            ),
            N::RestoreDirectoryUnavailable => say("Couldn't find the folder.", "没能找到目录。"),
            N::RestoreAgentNotResumed => self.agent_not_resumed(None),
            N::AgentNotResumed(agent, _) => self.agent_not_resumed(agent),
            N::LinkNoLink => say("Couldn't find a link there.", "没能在这里找到链接。"),
            N::LinkBusy | N::LinkFailed => say("Couldn't open the link.", "没能打开链接。"),
            // Reopening needs a fresh session, which no notice class says.
            N::PopupUnavailable => say(
                "Popup connection or cleanup uncertain · restart the session before reopening",
                "popup 连接或清理状态不确定 · 重启会话后再打开",
            ),
            N::HistoryUnavailable => say("Couldn't show earlier output.", "没能显示先前的输出。"),
            N::HistoryPositionLost => say(
                "Couldn't keep your place in the output.",
                "没能保留你在输出中的位置。",
            ),
            N::SelectionEvicted => say("Couldn't keep the selection.", "没能保留选区。"),
            N::CopyFailed => say("Couldn't copy to the clipboard.", "没能复制到剪贴板。"),
            N::CopyModeUnavailable => say("Couldn't start copy mode.", "没能进入复制模式。"),
            N::CopyModeBusy => say(
                "Couldn't do that in copy mode yet.",
                "没能在复制模式中执行。",
            ),
            N::CopyModeInvalidated => say(
                "Copy mode ended because its content or panel changed.",
                "内容或面板已改变，复制模式已退出。",
            ),
            N::CopyModeQueryFailed => {
                say("Couldn't read the matching output.", "没能读取匹配的输出。")
            }
            N::CopyModeNoMatch => say("No search match", "没有搜索匹配项"),
            N::CopyModeSearchLimit => say(
                "Couldn't search for that. Use at most 512 characters.",
                "没能搜索。最多输入 512 个字符。",
            ),
            N::FindUnavailable => say("Terminal disconnected", "终端已断开"),
            N::PanelTooSmall => say("Panel too small", "面板太小"),
            N::FindStopped => say("Couldn't finish the search.", "没能完成搜索。"),
            N::FindFailed => say("Couldn't search the terminal output.", "没能搜索终端输出。"),
            N::CopyModeNoSelection => say(
                "Start a selection with v or Space before copying",
                "请先用 v 或空格开始选择，再复制",
            ),
            N::BellFailed => say("Couldn't forward the terminal bell.", "没能转发终端响铃。"),
            N::PositionChanged => say(
                "Couldn't reopen the last Tab or Panel.",
                "没能重新打开上次的标签页或面板。",
            ),
            N::PositionChangedEmpty => say(
                "Couldn't reopen the last Tab.",
                "没能重新打开上次的标签页。",
            ),
            N::PositionLocationChanged | N::PositionLocationChangedEmpty => {
                say("Couldn't return to the last place.", "没能回到上次的位置。")
            }
            N::RecordUnusable(record, problem) => self.record_message(record, problem, None),
            N::PositionUnavailable => {
                say("Couldn't remember where you were.", "没能记住上次的位置。")
            }
        }
    }

    pub(crate) fn say(self, english: &str, chinese: &str) -> String {
        match self {
            Self::EnUs => english,
            Self::ZhCn => chinese,
        }
        .to_owned()
    }

    fn native_starting_status(self, label: Option<&str>) -> String {
        self.native_connection_status(label, crate::localization::TerminalStatus::Connecting)
    }

    fn native_switching_status(self, label: Option<&str>) -> String {
        self.native_connection_status(label, crate::localization::TerminalStatus::Unavailable)
    }

    /// Whatever a connection still waits for, it reads as connecting.
    fn native_connection_status(
        self,
        label: Option<&str>,
        missing_position: crate::localization::TerminalStatus,
    ) -> String {
        match label {
            Some(_) => self.connecting_status(),
            None => self.terminal_status(missing_position),
        }
        .to_owned()
    }

    const fn native_creating(self) -> &'static str {
        match self {
            Self::EnUs => "Creating Tab…",
            Self::ZhCn => "正在创建标签页…",
        }
    }

    const fn native_create_not_created(self) -> &'static str {
        match self {
            Self::EnUs => "Couldn't create the Tab.",
            Self::ZhCn => "没能创建标签页。",
        }
    }

    const fn native_refreshing(self) -> &'static str {
        match self {
            Self::EnUs => "Refreshing Tabs…",
            Self::ZhCn => "正在刷新标签页…",
        }
    }

    fn native_no_tab(self, position: u8) -> String {
        match self {
            Self::EnUs => format!("Couldn't go to Tab {position}."),
            Self::ZhCn => format!("没能切换到第 {position} 个标签页。"),
        }
    }

    /// Labels a one-based automatic Tab position without changing its canonical name.
    pub(crate) fn automatic_tab_label(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position}"),
            Self::ZhCn => format!("标签页 {position}"),
        }
    }

    /// Marks a maximized Panel, which hides the others in its Tab.
    pub(crate) const fn zoomed_panes(self) -> &'static str {
        self.status().zoomed
    }

    fn native_ready_controlling(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Ready"),
            Self::ZhCn => format!("{label} · 已就绪"),
        }
    }

    fn native_ready_read_only(self, label: &str) -> String {
        format!("{label} · {}", self.view_only())
    }

    fn native_terminal_pending(self, label: &str) -> String {
        format!("{label} · {}", self.connecting_status())
    }

    /// The state every terminal shows while the Server replaces a lost Worker, after its Tab's
    /// label when it has one.
    pub(crate) fn terminal_reconnecting(self, label: Option<&str>) -> String {
        let state = self.status().reconnecting;
        label.map_or_else(|| state.to_owned(), |label| format!("{label} · {state}"))
    }

    /// The state every terminal shows once the Server gave up replacing a lost Worker, after its
    /// Tab's label when it has one.
    pub(crate) fn terminal_cant_reconnect(self, label: Option<&str>) -> String {
        let state = match self {
            Self::EnUs => "Can't reconnect",
            Self::ZhCn => "无法重新连接",
        };
        label.map_or_else(|| state.to_owned(), |label| format!("{label} · {state}"))
    }

    /// What reconnecting means for the programs and for what the person types.
    pub(crate) const fn terminal_reconnecting_description(self) -> &'static str {
        match self {
            Self::EnUs => {
                "Orchbee lost contact with its terminals and is reconnecting. Programs keep running; anything you type until then isn't sent."
            }
            Self::ZhCn => {
                "Orchbee 与终端失去联系，正在重新连接。程序会继续运行；在此之前输入的内容不会发送。"
            }
        }
    }

    /// The status notice once the Server gave up; the key that leaves the view fills its slot when
    /// the status line is drawn.
    pub(crate) fn terminal_given_up_notice(self) -> String {
        let detach = super::shell::EXIT_KEY_SLOT;
        match self {
            Self::EnUs => format!(
                "Programs keep running, but Orchbee can't reach them. To stop them, leave with {detach} and run orchbee server stop; the next start reopens your layout."
            ),
            Self::ZhCn => format!(
                "程序仍在运行，但 Orchbee 无法连接它们。如需停止，按 {detach} 离开后运行 orchbee server stop，下次启动时恢复布局。"
            ),
        }
    }

    fn native_terminal_unavailable(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · terminal unavailable"),
            Self::ZhCn => format!("{label} · 终端不可用"),
        }
    }

    fn native_terminal_failed(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Couldn't start the terminal."),
            Self::ZhCn => format!("{label} · 没能启动终端。"),
        }
    }

    fn native_terminal_outcome_unknown(self, label: &str) -> String {
        format!("{label} · {}", self.outcome_unknown())
    }
}

#[cfg(all(test, unix))]
mod tests;
