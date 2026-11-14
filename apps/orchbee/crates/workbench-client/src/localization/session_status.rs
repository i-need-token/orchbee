//! Localized session connection, display, and mutation feedback; does not change session state.

use super::Locale;
mod parameters;
pub(crate) use parameters::{
    ClientProjectionStatus, DisplayController, DisplayProvenance, DisplayRetirement,
    DuplicateProgress, DuplicateStop, LaunchFailure, LaunchProfile, NoticeContext, RefreshStatus,
    WorkbenchClientNotice,
};
pub use parameters::{DisplayPhase, TerminalStatus};

impl Locale {
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
            (Self::EnUs, TerminalStatus::Connecting) => "Connecting terminal…",
            (Self::ZhCn, TerminalStatus::Connecting) => "正在连接终端…",
            (Self::EnUs, TerminalStatus::Controlling) => "Terminal connected",
            (Self::ZhCn, TerminalStatus::Controlling) => "终端已连接",
            (Self::EnUs, TerminalStatus::ReadOnly) => {
                "Viewing only · another Client controls this terminal"
            }
            (Self::ZhCn, TerminalStatus::ReadOnly) => "仅查看 · 其他客户端正在控制此终端",
            (Self::EnUs, TerminalStatus::Blocked) => {
                "Terminal display paused · waiting for a fresh frame"
            }
            (Self::ZhCn, TerminalStatus::Blocked) => "终端显示已暂停 · 正在等待新帧",
            (Self::EnUs, TerminalStatus::Unavailable) => "Terminal disconnected",
            (Self::ZhCn, TerminalStatus::Unavailable) => "终端已断连",
            (Self::EnUs, TerminalStatus::OutcomeUnknown) => {
                "Terminal operation result unknown · no replay"
            }
            (Self::ZhCn, TerminalStatus::OutcomeUnknown) => "终端操作结果未知 · 不会重放",
        }
    }

    /// Format only the selected duplication observation; priority belongs to Workbench.
    pub(crate) fn duplication_progress(self, progress: DuplicateProgress) -> String {
        let status = match (self, progress.stop, progress.complete) {
            (Self::EnUs, _, true) => "Duplication complete",
            (Self::ZhCn, _, true) => "复制完成",
            (Self::EnUs, None, false) => "Duplicating tabs",
            (Self::ZhCn, None, false) => "正在复制标签页",
            (Self::EnUs, Some(DuplicateStop::Changed), _) => {
                "Duplication stopped: selection or workspace changed"
            }
            (Self::ZhCn, Some(DuplicateStop::Changed), _) => "复制已停止：选择或工作区已改变",
            (Self::EnUs, Some(DuplicateStop::Deadline), _) => "Duplication stopped: time limit",
            (Self::ZhCn, Some(DuplicateStop::Deadline), _) => "复制已停止：已到时限",
            (Self::EnUs, Some(DuplicateStop::NotCreated), _) => {
                "Duplication stopped: tab not created"
            }
            (Self::ZhCn, Some(DuplicateStop::NotCreated), _) => "复制已停止：标签页未创建",
            (Self::EnUs, Some(DuplicateStop::TerminalNotStarted | DuplicateStop::Recovery), _) => {
                "Duplication stopped: check pending, failed or unknown result; no replay"
            }
            (Self::ZhCn, Some(DuplicateStop::TerminalNotStarted | DuplicateStop::Recovery), _) => {
                "复制已停止：请检查待定、失败或未知结果；不会重放"
            }
        };
        match self {
            Self::EnUs => format!(
                "{status} · {}/{} submitted · {} tabs created · {} terminals started",
                progress.submitted, progress.total, progress.committed, progress.started
            ),
            Self::ZhCn => format!(
                "{status} · 已提交 {}/{} · 已创建 {} 个标签页 · 已启动 {} 个终端",
                progress.submitted, progress.total, progress.committed, progress.started
            ),
        }
    }

    pub(crate) const fn native_unbound_terminal(self) -> &'static str {
        match self {
            Self::EnUs => "Empty terminal slot · Use panel create-terminal to start a terminal",
            Self::ZhCn => "空终端位 · 使用 panel create-terminal 启动终端",
        }
    }

    /// Render the feature-selected phase facts without inspecting or retaining a view.
    pub(crate) fn native_phase_status_values(
        self,
        phase: crate::localization::DisplayPhase,
        label: Option<&str>,
        controller: Option<crate::localization::DisplayController>,
        provenance: Option<crate::localization::DisplayProvenance>,
        projection: crate::localization::ClientProjectionStatus,
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
            Phase::Starting => self.native_starting_status(label, projection),
            Phase::Switching => self.native_switching_status(label, projection),
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
            Phase::ProjectionUnavailable => self.native_committed_refresh_required().to_owned(),
            Phase::MoveProjectionUnavailable => match self {
                Self::EnUs => "Tab moved · refresh required".to_owned(),
                Self::ZhCn => "标签页已移动 · 需要刷新".to_owned(),
            },
            Phase::PanelProjectionUnavailable => match self {
                Self::EnUs => "Panel operation committed · refresh required".to_owned(),
                Self::ZhCn => "面板操作已提交 · 需要刷新".to_owned(),
            },
            Phase::RenameProjectionUnavailable => match self {
                Self::EnUs => "Rename committed · refresh required".to_owned(),
                Self::ZhCn => "重命名已提交 · 需要刷新".to_owned(),
            },
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
            RefreshStatus::Success(revision) => self.native_refresh_success(revision),
            RefreshStatus::Failure(revision) => self.native_refresh_failure(revision),
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
        let shorter = |english: &str, chinese: &str| {
            Some(
                match self {
                    Self::EnUs => english,
                    Self::ZhCn => chinese,
                }
                .to_owned(),
            )
        };
        match notice {
            N::RecordUnusable(record, problem) => Some(self.record_status(record, problem)),
            N::AgentNotResumed(agent, _) => Some(self.agent_not_resumed(agent)),
            N::AgentStart(phase) => self.agent_start_phase_shorter(phase),
            N::Settings(notice) => self.settings_notice_shorter(notice).map(str::to_owned),
            N::Unsupported(capability) => Some(self.unsupported_capability_shorter(capability)),
            N::MenuUnavailable => shorter(
                "Menu action cancelled or unavailable",
                "菜单动作已取消或不可用",
            ),
            N::LaunchRequestTooLarge => shorter("Launch request too large", "启动请求过大"),
            N::MoveOrderConflict => shorter(
                "Tab not moved: pinned Tabs and groups stay together",
                "标签页未移动：固定标签页和标签页组需保持连续",
            ),
            N::NoWorkspace => shorter("No workspace open", "当前未打开工作区"),
            N::HistoryPositionLost => {
                shorter("Earlier output no longer saved", "先前输出已不在保留范围")
            }
            N::CopyModeQueryFailed => {
                shorter("Could not read matching content", "无法读取对应内容")
            }
            N::CopyModeSearchLimit => shorter(
                "Search rejected: invalid or too long",
                "拒绝搜索：文本无效或过长",
            ),
            N::FindStopped => shorter("Search stopped", "搜索已停止"),
            N::FindFailed => shorter("Search failed", "搜索失败"),
            N::PositionChanged => shorter(
                "Saved Tab or Panel unavailable",
                "上次的标签页或面板已不可用",
            ),
            N::PositionChangedEmpty => shorter("Saved Tab unavailable", "上次的标签页已不可用"),
            N::PositionLocationChanged | N::PositionLocationChangedEmpty => {
                shorter("Saved location not restored", "无法恢复上次的位置")
            }
            N::PositionUnavailable => shorter("Position memory unavailable", "位置记忆不可用"),
            N::PopupUnavailable => shorter(
                "Popup connection or cleanup uncertain",
                "popup 连接或清理状态不确定",
            ),
            N::LinkUnknown => shorter("Link activation outcome unknown", "链接处理结果不确定"),
            N::LocalBusy
            | N::NoTabAtPosition(_)
            | N::CreateNotCreated
            | N::CreateOrderConflict
            | N::DuplicatePending
            | N::DuplicateNotStarted
            | N::DuplicateOutcomeUnknown
            | N::MoveNotMoved
            | N::PanelNotApplied
            | N::RenameNotRenamed
            | N::TerminalClearing
            | N::TerminalCleared
            | N::TerminalClearUnconfirmed
            | N::TerminalClearRefused
            | N::TerminalClearUnavailable
            | N::TerminalClearUnknown
            | N::CloseRequestNotApplied
            | N::WorkspaceCloseUnavailable
            | N::WorkspaceChanged
            | N::AgentUnavailable
            | N::WorkspaceNotReady
            | N::ConfirmationCancelled
            | N::DraftCancelled
            | N::DialogRefused
            | N::ModalOwnsInput
            | N::HistoryRecovering
            | N::HistoryUnavailable
            | N::CopyModeUnavailable
            | N::CopyModeBusy
            | N::CopyModeInvalidated
            | N::CopyModeNoMatch
            | N::CopyModeNoSelection
            | N::FindUnavailable
            | N::PanelTooSmall
            | N::SelectionEvicted
            | N::CopyFailed
            | N::CopyUnknown
            | N::BellFailed
            | N::BellUnknown
            | N::CommandAccepted
            | N::CommandFailed
            | N::CommandUnknown
            | N::CommandRefused(_)
            | N::LaunchProfileSelected
            | N::LaunchProfileUnavailable
            | N::RestoreDirectoryUnavailable
            | N::RestoreAgentNotResumed
            | N::RestoreNotStarted
            | N::LinkOpened
            | N::LinkHandled
            | N::LinkNoLink
            | N::LinkBusy
            | N::LinkFailed => None,
        }
    }

    /// The headline of a resume that typed nothing, naming the Agent when it is known.
    fn agent_not_resumed(self, agent: Option<crate::agent_provider::IntegrationId>) -> String {
        match (self, agent) {
            (Self::EnUs, Some(id)) => {
                format!(
                    "{} conversation not resumed",
                    self.agent_integration_name(id)
                )
            }
            (Self::EnUs, None) => "Agent conversation not resumed".into(),
            (Self::ZhCn, Some(id)) => format!("未续接 {} 对话", self.agent_integration_name(id)),
            (Self::ZhCn, None) => "未续接智能体对话".into(),
        }
    }

    pub(crate) fn native_notice(
        self,
        notice: crate::localization::WorkbenchClientNotice,
    ) -> String {
        use crate::localization::WorkbenchClientNotice;
        match notice {
            WorkbenchClientNotice::Unsupported(capability) => self.unsupported_capability(capability),
            WorkbenchClientNotice::Settings(notice) => self.settings_notice(notice).to_owned(),
            WorkbenchClientNotice::AgentStart(phase) => self.agent_start_phase(phase),
            WorkbenchClientNotice::LocalBusy => self.native_local_busy().to_owned(),
            WorkbenchClientNotice::TerminalClearing => match self { Self::EnUs => "Clearing terminal…".into(), Self::ZhCn => "正在清除终端…".into() },
            WorkbenchClientNotice::TerminalCleared => match self { Self::EnUs => "Terminal cleared".into(), Self::ZhCn => "终端已清除".into() },
            WorkbenchClientNotice::TerminalClearUnconfirmed => match self { Self::EnUs => "Terminal cleared · cleanup unconfirmed".into(), Self::ZhCn => "终端已清除 · 尚未确认清理完成".into() },
            WorkbenchClientNotice::TerminalClearRefused => match self { Self::EnUs => "Terminal clear refused · nothing cleared".into(), Self::ZhCn => "清除终端被拒绝 · 未清除内容".into() },
            WorkbenchClientNotice::TerminalClearUnavailable => match self { Self::EnUs => "Terminal clear unavailable".into(), Self::ZhCn => "无法清除终端".into() },
            WorkbenchClientNotice::TerminalClearUnknown => match self { Self::EnUs => "Terminal clear outcome unknown · not retried".into(), Self::ZhCn => "无法确认清除终端的结果 · 未重试".into() },
            WorkbenchClientNotice::MenuUnavailable => match self {
                Self::EnUs => "Menu action cancelled or unavailable · open the menu again".into(),
                Self::ZhCn => "菜单动作已取消或不可用 · 请重新打开菜单".into(),
            },
            WorkbenchClientNotice::NoTabAtPosition(position) => self.native_no_tab(position),
            WorkbenchClientNotice::CreateNotCreated => self.native_create_not_created().to_owned(),
            WorkbenchClientNotice::LaunchRequestTooLarge => self.launch_request_too_large().to_owned(),
            WorkbenchClientNotice::CreateOrderConflict => match self {
                Self::EnUs => "Cannot create an unpinned tab inside the pinned tabs".to_owned(),
                Self::ZhCn => "无法在固定标签页之间创建未固定的标签页".to_owned(),
            },
            WorkbenchClientNotice::DuplicatePending => match self {
                Self::EnUs => "Duplicate tab added; terminal start is pending".into(),
                Self::ZhCn => "已添加复制标签页；终端仍在启动中".into(),
            },
            WorkbenchClientNotice::DuplicateNotStarted => match self {
                Self::EnUs => "Duplicate tab added; terminal did not start".into(),
                Self::ZhCn => "已添加复制标签页；终端未能启动".into(),
            },
            WorkbenchClientNotice::DuplicateOutcomeUnknown => match self {
                Self::EnUs => "Duplicate tab added; terminal start outcome is unknown".into(),
                Self::ZhCn => "已添加复制标签页；终端启动结果未知".into(),
            },
            WorkbenchClientNotice::CloseRequestNotApplied => match self {
                Self::EnUs => "Close request was not applied".to_owned(),
                Self::ZhCn => "关闭请求未执行".to_owned(),
            },
            WorkbenchClientNotice::WorkspaceCloseUnavailable => match self {
                Self::EnUs => {
                    "Select a workspace to close"
                        .to_owned()
                }
                Self::ZhCn => "请选择要关闭的工作区".to_owned(),
            },
            WorkbenchClientNotice::WorkspaceChanged => match self {
                Self::EnUs => "Workspace changed; try again".to_owned(),
                Self::ZhCn => "工作区已变化，请重试".to_owned(),
            },
            WorkbenchClientNotice::AgentUnavailable => match self {
                Self::EnUs => "This Agent is no longer available".to_owned(),
                Self::ZhCn => "这个智能体已不可用".to_owned(),
            },
            WorkbenchClientNotice::NoWorkspace => self.no_current_workspace().to_owned(),
            WorkbenchClientNotice::WorkspaceNotReady => match self {
                Self::EnUs => "Workspace not ready · no worktree action started".to_owned(),
                Self::ZhCn => "工作区未就绪 · 未开始工作树操作".to_owned(),
            },
            WorkbenchClientNotice::RenameNotRenamed => match self {
                Self::EnUs => "Tab not renamed".to_owned(),
                Self::ZhCn => "标签页未重命名".to_owned(),
            },
            WorkbenchClientNotice::MoveNotMoved => match self {
                Self::EnUs => "Tab was not moved".to_owned(),
                Self::ZhCn => "标签页未移动".to_owned(),
            },
            WorkbenchClientNotice::MoveOrderConflict => match self {
                Self::EnUs => {
                    "Tab not moved: preserve pinned Tabs and contiguous groups".to_owned()
                }
                Self::ZhCn => "标签页未移动：需保持固定标签页在前、标签页组连续".to_owned(),
            },
            WorkbenchClientNotice::PanelNotApplied => match self {
                Self::EnUs => "Panel operation was not applied".to_owned(),
                Self::ZhCn => "面板操作未执行".to_owned(),
            },
            WorkbenchClientNotice::ConfirmationCancelled => match self {
                Self::EnUs => "Close confirmation cancelled".to_owned(),
                Self::ZhCn => "关闭确认已取消".to_owned(),
            },
            WorkbenchClientNotice::DialogRefused => self.presentation_notice(false).to_owned(),
            WorkbenchClientNotice::DraftCancelled => match self {
                Self::EnUs => "Draft cancelled".to_owned(),
                Self::ZhCn => "草稿已取消".to_owned(),
            },
            WorkbenchClientNotice::ModalOwnsInput => match self {
                Self::EnUs => "Close the active dialog first".to_owned(),
                Self::ZhCn => "请先关闭当前对话框".to_owned(),
            },
            WorkbenchClientNotice::HistoryRecovering => match self {
                Self::EnUs => "Showing the latest output".to_owned(),
                Self::ZhCn => "正在显示最新输出".to_owned(),
            },
            WorkbenchClientNotice::CommandAccepted => match self {
                Self::EnUs => "Command accepted".into(),
                Self::ZhCn => "命令已接受".into(),
            },
            WorkbenchClientNotice::CommandFailed => match self {
                Self::EnUs => "Command or focus failed; inspect result".into(),
                Self::ZhCn => "命令或聚焦失败；请检查结果".into(),
            },
            WorkbenchClientNotice::CommandUnknown => match self {
                Self::EnUs => "Outcome unknown; inspect before retry".into(),
                Self::ZhCn => "结果不确定；请检查后再决定是否重试".into(),
            },
            WorkbenchClientNotice::CommandRefused(reason) => {
                use orchbee_workbench_protocol::ConfiguredCommandFailure as F;
                match (self, reason) {
                    (Self::EnUs, F::Denied) => "Denied by Session permissions",
                    (Self::ZhCn, F::Denied) => "会话权限拒绝；请检查权限",
                    (Self::EnUs, F::InvalidIntent) => "Invalid request",
                    (Self::ZhCn, F::InvalidIntent) => "请求无效",
                    (Self::EnUs, F::SessionMismatch) => "Session changed; reconnect",
                    (Self::ZhCn, F::SessionMismatch) => "会话已变化；请重新连接",
                    (Self::EnUs, F::Unavailable) => "Unavailable; check Session and retry",
                    (Self::ZhCn, F::Unavailable) => "服务不可用；请检查会话后重试",
                    (Self::EnUs, F::Busy) => "Busy; wait and retry",
                    (Self::ZhCn, F::Busy) => "正在忙；请稍后重试",
                    (Self::EnUs, F::Conflict) => "Configuration changed",
                    (Self::ZhCn, F::Conflict) => "配置已变化",
                    (Self::EnUs, F::NotFound) => "Command changed",
                    (Self::ZhCn, F::NotFound) => "命令已变化",
                    (Self::EnUs, F::StaleSource) => "Source changed; return live and retry",
                    (Self::ZhCn, F::StaleSource) => "来源已变化；请返回实时终端后重试",
                    (Self::EnUs, F::PhysicalUnknown) => "Outcome unknown; inspect before retry",
                    (Self::ZhCn, F::PhysicalUnknown) => "结果不确定；请检查后再决定是否重试",
                }
                .into()
            }
            WorkbenchClientNotice::LaunchProfileSelected => match self {
                Self::EnUs => "Launch profile".into(),
                Self::ZhCn => "启动配置".into(),
            },
            WorkbenchClientNotice::LaunchProfileUnavailable => match self {
                Self::EnUs => "Terminal not created · check profile in Settings".into(),
                Self::ZhCn => "终端未创建 · 请在设置中检查启动配置".into(),
            },
            WorkbenchClientNotice::RestoreDirectoryUnavailable => match self {
                Self::EnUs => "Directory still unavailable".into(),
                Self::ZhCn => "目录仍不可用".into(),
            },
            WorkbenchClientNotice::RestoreAgentNotResumed => match self {
                Self::EnUs => "Agent conversation not resumed".into(),
                Self::ZhCn => "未续接智能体对话".into(),
            },
            WorkbenchClientNotice::RestoreNotStarted => match self {
                Self::EnUs => "Terminal not started".into(),
                Self::ZhCn => "终端未启动".into(),
            },
            WorkbenchClientNotice::AgentNotResumed(agent, reason) => {
                use orchbee_workbench_protocol::AgentNotResumedReason as R;
                let headline = self.agent_not_resumed(agent);
                match (self, reason) {
                    (Self::EnUs, R::PromptUnconfirmed) => format!(
                        "{headline}: the shell was not at its prompt, so nothing was typed."
                    ),
                    (Self::EnUs, R::TerminalTooOld) => format!(
                        "{headline}: this terminal was started by an older Orchbee, so nothing was typed."
                    ),
                    (Self::ZhCn, R::PromptUnconfirmed) => {
                        format!("{headline}：shell 不在提示符上，未输入任何内容。")
                    }
                    (Self::ZhCn, R::TerminalTooOld) => {
                        format!("{headline}：这个终端由旧版 Orchbee 启动，未输入任何内容。")
                    }
                }
            }
            WorkbenchClientNotice::LinkOpened => match self {
                Self::EnUs => "Link opened".into(),
                Self::ZhCn => "链接已打开".into(),
            },
            WorkbenchClientNotice::LinkHandled => match self {
                Self::EnUs => "Link sent to a plugin".into(),
                Self::ZhCn => "链接已交给插件处理".into(),
            },
            WorkbenchClientNotice::LinkNoLink => match self {
                Self::EnUs => "No link at that cell".into(),
                Self::ZhCn => "该单元格没有链接".into(),
            },
            WorkbenchClientNotice::LinkBusy => match self {
                Self::EnUs => "Another link activation is still running".into(),
                Self::ZhCn => "另一个链接仍在处理中".into(),
            },
            WorkbenchClientNotice::LinkFailed => match self {
                Self::EnUs => "Link activation failed".into(),
                Self::ZhCn => "链接打开失败".into(),
            },
            WorkbenchClientNotice::LinkUnknown => match self {
                Self::EnUs => {
                    "Link activation outcome unknown · inspect before trying again".into()
                }
                Self::ZhCn => "链接处理结果不确定 · 请检查后再决定是否重试".into(),
            },
            WorkbenchClientNotice::PopupUnavailable => match self {
                Self::EnUs => {
                    "Popup connection or cleanup uncertain · restart the session before reopening"
                        .to_owned()
                }
                Self::ZhCn => "popup 连接或清理状态不确定 · 重启会话后再打开".to_owned(),
            },
            WorkbenchClientNotice::HistoryUnavailable => match self {
                Self::EnUs => "History is not available on this panel".to_owned(),
                Self::ZhCn => "此面板历史不可用".to_owned(),
            },
            WorkbenchClientNotice::HistoryPositionLost => match self {
                Self::EnUs => {
                    "Earlier output is no longer saved; showing the latest output".to_owned()
                }
                Self::ZhCn => "先前输出已不在保留范围，正在显示最新输出".to_owned(),
            },
            WorkbenchClientNotice::SelectionEvicted => match self {
                Self::EnUs => "Selected rows are no longer available".to_owned(),
                Self::ZhCn => "所选行已不可用".to_owned(),
            },
            WorkbenchClientNotice::CopyFailed => match self {
                Self::EnUs => "Could not copy to the clipboard".to_owned(),
                Self::ZhCn => "未能复制到剪贴板".to_owned(),
            },
            WorkbenchClientNotice::CopyUnknown => match self {
                Self::EnUs => "Clipboard delivery could not be confirmed".to_owned(),
                Self::ZhCn => "无法确认剪贴板投递结果".to_owned(),
            },
            WorkbenchClientNotice::CopyModeUnavailable => match self {
                Self::EnUs => "Copy mode is not available on this panel".into(),
                Self::ZhCn => "此面板暂时无法进入复制模式".into(),
            },
            WorkbenchClientNotice::CopyModeBusy => match self {
                Self::EnUs => "Copy mode is busy; wait before trying again".into(),
                Self::ZhCn => "复制模式正在处理，请稍后再操作".into(),
            },
            WorkbenchClientNotice::CopyModeInvalidated => match self {
                Self::EnUs => "Copy mode ended because its content or panel changed".into(),
                Self::ZhCn => "内容或面板已改变，复制模式已退出".into(),
            },
            WorkbenchClientNotice::CopyModeQueryFailed => match self {
                Self::EnUs => {
                    "Could not read matching content; try the motion or search again".into()
                }
                Self::ZhCn => "无法读取对应内容，请重新移动或搜索".into(),
            },
            WorkbenchClientNotice::CopyModeNoMatch => match self {
                Self::EnUs => "No search match".into(),
                Self::ZhCn => "没有搜索匹配项".into(),
            },
            WorkbenchClientNotice::CopyModeSearchLimit => match self {
                Self::EnUs => "Rejected search: invalid text or more than 512 characters".into(),
                Self::ZhCn => "拒绝搜索：文本无效或超过 512 个字符".into(),
            },
            WorkbenchClientNotice::FindUnavailable => match self {
                Self::EnUs => "Terminal disconnected".into(),
                Self::ZhCn => "终端已断连".into(),
            },
            WorkbenchClientNotice::PanelTooSmall => match self {
                Self::EnUs => "Panel too small".into(),
                Self::ZhCn => "面板太小".into(),
            },
            WorkbenchClientNotice::FindStopped => match self {
                Self::EnUs => "Search stopped after reaching the time limit".into(),
                Self::ZhCn => "搜索达到时间限制后已停止".into(),
            },
            WorkbenchClientNotice::FindFailed => match self {
                Self::EnUs => "Could not search the terminal output".into(),
                Self::ZhCn => "无法搜索终端输出".into(),
            },
            WorkbenchClientNotice::CopyModeNoSelection => match self {
                Self::EnUs => "Start a selection with v or Space before copying".into(),
                Self::ZhCn => "请先用 v 或空格开始选择，再复制".into(),
            },
            WorkbenchClientNotice::BellFailed => match self {
                Self::EnUs => "Could not forward the terminal bell".to_owned(),
                Self::ZhCn => "未能转发终端响铃".to_owned(),
            },
            WorkbenchClientNotice::BellUnknown => match self {
                Self::EnUs => "Terminal bell delivery could not be confirmed".to_owned(),
                Self::ZhCn => "无法确认终端响铃投递结果".to_owned(),
            },
            WorkbenchClientNotice::PositionChanged => match self {
                Self::EnUs => {
                    "The saved Tab or Panel is unavailable; showing the first available member.".into()
                }
                Self::ZhCn => "上次的标签页或面板已不可用；已打开首个可用成员。".into(),
            },
            WorkbenchClientNotice::PositionChangedEmpty => match self {
                Self::EnUs => "The saved Tab is unavailable; showing the empty Workspace.".into(),
                Self::ZhCn => "上次的标签页已不可用；已打开原工作区的空视图。".into(),
            },
            WorkbenchClientNotice::PositionLocationChanged => match self {
                Self::EnUs => "The saved location could not be restored; showing the first available Workspace.".into(),
                Self::ZhCn => "无法恢复上次的位置；已打开首个可用工作区。".into(),
            },
            WorkbenchClientNotice::PositionLocationChangedEmpty => match self {
                Self::EnUs => "The saved location could not be restored; showing an empty Workspace.".into(),
                Self::ZhCn => "无法恢复上次的位置；已打开空工作区。".into(),
            },
            WorkbenchClientNotice::RecordUnusable(record, problem) => {
                self.record_message(record, problem, None)
            }
            WorkbenchClientNotice::PositionUnavailable => match self {
                Self::EnUs => {
                    "Position memory is unavailable; terminal tasks are still running.".into()
                }
                Self::ZhCn => "位置记忆不可用；终端任务仍继续运行。".into(),
            },
        }
    }

    fn native_starting_status(
        self,
        label: Option<&str>,
        projection: crate::localization::ClientProjectionStatus,
    ) -> String {
        self.native_connection_status(
            label,
            projection,
            crate::localization::TerminalStatus::Connecting,
        )
    }

    fn native_switching_status(
        self,
        label: Option<&str>,
        projection: crate::localization::ClientProjectionStatus,
    ) -> String {
        self.native_connection_status(
            label,
            projection,
            crate::localization::TerminalStatus::Unavailable,
        )
    }

    fn native_connection_status(
        self,
        label: Option<&str>,
        projection: crate::localization::ClientProjectionStatus,
        missing_position: crate::localization::TerminalStatus,
    ) -> String {
        use crate::localization::ClientProjectionStatus;

        let Some(label) = label else {
            return self.terminal_status(missing_position).to_owned();
        };
        match projection {
            ClientProjectionStatus::WaitingForFullFrame => self.native_waiting_for_frame(label),
            ClientProjectionStatus::Blocked => self.native_display_blocked(label),
            ClientProjectionStatus::Empty | ClientProjectionStatus::FrameAvailable => {
                self.native_connecting(label)
            }
        }
    }

    const fn native_creating(self) -> &'static str {
        match self {
            Self::EnUs => "Creating Tab…",
            Self::ZhCn => "正在创建标签页…",
        }
    }

    const fn native_create_not_created(self) -> &'static str {
        match self {
            Self::EnUs => "Tab not created · previous Tab unchanged",
            Self::ZhCn => "未创建标签页 · 原标签页未更改",
        }
    }

    const fn native_refreshing(self) -> &'static str {
        match self {
            Self::EnUs => "Refreshing Tabs…",
            Self::ZhCn => "正在刷新标签页…",
        }
    }

    fn native_refresh_success(self, revision: u64) -> String {
        match self {
            Self::EnUs => format!("Tabs refreshed · revision {revision}"),
            Self::ZhCn => format!("标签页已刷新 · 修订 {revision}"),
        }
    }

    fn native_refresh_failure(self, revision: u64) -> String {
        match self {
            Self::EnUs => format!("Tab refresh failed · showing revision {revision}"),
            Self::ZhCn => format!("标签页刷新失败 · 正在显示修订 {revision}"),
        }
    }

    const fn native_local_busy(self) -> &'static str {
        match self {
            Self::EnUs => "Client busy · no command sent",
            Self::ZhCn => "客户端正忙 · 未发送命令",
        }
    }

    fn native_no_tab(self, position: u8) -> String {
        match self {
            Self::EnUs => format!("No Tab at position {position}"),
            Self::ZhCn => format!("第 {position} 个标签页不存在"),
        }
    }

    /// Labels a one-based automatic Tab position without changing its canonical name.
    pub(crate) fn automatic_tab_label(self, position: usize) -> String {
        match self {
            Self::EnUs => format!("Tab {position}"),
            Self::ZhCn => format!("标签页 {position}"),
        }
    }

    /// Names how many Panels a maximized Panel hides behind it.
    pub(crate) fn zoomed_panes(self, count: usize) -> String {
        match self {
            Self::EnUs => format!("maximized · {count} panels"),
            Self::ZhCn => format!("已最大化 · 共 {count} 个面板"),
        }
    }

    fn native_connecting(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Connecting…"),
            Self::ZhCn => format!("{label} · 正在连接…"),
        }
    }

    fn native_waiting_for_frame(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Connecting · waiting for full frame"),
            Self::ZhCn => format!("{label} · 正在连接 · 等待完整帧"),
        }
    }

    fn native_display_blocked(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Connecting · display blocked"),
            Self::ZhCn => format!("{label} · 正在连接 · 显示已阻塞"),
        }
    }

    fn native_ready_controlling(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Ready"),
            Self::ZhCn => format!("{label} · 已就绪"),
        }
    }

    fn native_ready_read_only(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · Ready · view only"),
            Self::ZhCn => format!("{label} · 已就绪 · 仅查看"),
        }
    }

    const fn native_committed_refresh_required(self) -> &'static str {
        match self {
            Self::EnUs => "Created Tab committed · refresh required",
            Self::ZhCn => "已提交创建的标签页 · 需要刷新",
        }
    }

    fn native_terminal_pending(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · terminal creation pending · no replay"),
            Self::ZhCn => format!("{label} · 终端创建等待中 · 不会重放"),
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
            Self::EnUs => format!("{label} · terminal creation failed"),
            Self::ZhCn => format!("{label} · 终端创建失败"),
        }
    }

    fn native_terminal_outcome_unknown(self, label: &str) -> String {
        match self {
            Self::EnUs => format!("{label} · terminal result unknown · no replay"),
            Self::ZhCn => format!("{label} · 终端结果未知 · 不会重放"),
        }
    }
}

#[cfg(all(test, unix))]
mod tests;
