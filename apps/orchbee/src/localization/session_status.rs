//! Localized session connection, display, and mutation feedback; does not change session state.

use super::Locale;

impl Locale {
    pub(crate) const fn terminal_status(
        self,
        status: crate::workbench_client::TerminalStatus,
    ) -> &'static str {
        use crate::workbench_client::TerminalStatus;
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

    pub(crate) fn native_status(
        self,
        view: &crate::workbench_client::WorkbenchClientView<'_>,
        columns: u16,
    ) -> String {
        if let Some(notice) = view.worktree_notice() {
            return self.worktree_status_line(notice, view.worktree().map(|v| v.stage()));
        }
        if let Some((status, id)) = view.takeover_status() {
            return self.takeover_status(status, id);
        }
        if let Some(status) = view.workspace_close_status() {
            return self.workspace_close_status(status);
        }
        if view.retirement().is_none()
            && (view.phase() == crate::workbench_client::DisplayPhase::Ready
                && view
                    .focus_panel_id()
                    .and_then(|panel| view.terminal_end(panel))
                    .is_some()
                || view.phase() == crate::workbench_client::DisplayPhase::SelectedWithoutAttachment
                    && view.selected_provenance()
                        == Some(crate::workbench_client::DisplayProvenance::Failed))
        {
            let status = self.native_phase_status(view);
            if crate::terminal_text::display_width(&status) > usize::from(columns)
                && let Some(end) = view
                    .focus_panel_id()
                    .and_then(|panel| view.terminal_end(panel))
            {
                return self.terminal_end_compact(end);
            }
            return status;
        }
        if view.notice().is_none()
            && view.retirement().is_none()
            && view.setup_status().is_none()
            && let Some(progress) = view.duplicate_progress()
        {
            use crate::workbench_client::state::DuplicateStop;
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
                (
                    Self::EnUs,
                    Some(DuplicateStop::TerminalNotStarted | DuplicateStop::Recovery),
                    _,
                ) => "Duplication stopped: check pending, failed or unknown result; no replay",
                (
                    Self::ZhCn,
                    Some(DuplicateStop::TerminalNotStarted | DuplicateStop::Recovery),
                    _,
                ) => "复制已停止：请检查待定、失败或未知结果；不会重放",
            };
            return match self {
                Self::EnUs => format!(
                    "{status} · {}/{} submitted · {} tabs created · {} terminals started",
                    progress.submitted, progress.total, progress.committed, progress.started
                ),
                Self::ZhCn => format!(
                    "{status} · 已提交 {}/{} · 已创建 {} 个标签页 · 已启动 {} 个终端",
                    progress.submitted, progress.total, progress.committed, progress.started
                ),
            };
        }
        self.native_status_with_setup(
            self.native_phase_status(view),
            view.retirement().is_some(),
            view.notice(),
            view.setup_status(),
            view.phase() == crate::workbench_client::DisplayPhase::Empty
                && view.workspace_directory_unconfigured(),
        )
    }

    pub(crate) fn native_phase_status(
        self,
        view: &crate::workbench_client::WorkbenchClientView<'_>,
    ) -> String {
        if view.retirement().is_none()
            && view.phase() == crate::workbench_client::DisplayPhase::Ready
            && let Some(end) = view
                .focus_panel_id()
                .and_then(|panel| view.terminal_end(panel))
        {
            return self.terminal_end(end);
        }
        if let Some(recovery) = view.quick_workspace_recovery() {
            return self.quick_workspace_failure(
                recovery.project,
                recovery.workspace,
                recovery.unknown,
            );
        }
        if view.phase() == crate::workbench_client::DisplayPhase::SelectedWithoutAttachment
            && view.selected_is_unbound()
            && view.retirement().is_none()
        {
            return self.native_unbound_terminal().to_owned();
        }
        let tab = view
            .active_position()
            .and_then(|position| view.tab(position - 1));
        let label = tab.map(|tab| tab.display_label(self));
        let safe_label = label
            .as_deref()
            .map(crate::terminal_text::sanitize_terminal_text);
        self.native_phase_status_values(
            view.phase(),
            safe_label.as_deref(),
            view.controller(),
            view.selected_provenance(),
            view.projection_status(),
            view.retirement(),
        )
    }

    pub(crate) const fn native_unbound_terminal(self) -> &'static str {
        match self {
            Self::EnUs => "Empty terminal slot · Use panel create-terminal to start a terminal",
            Self::ZhCn => "空终端位 · 使用 panel create-terminal 启动终端",
        }
    }

    fn native_phase_status_values(
        self,
        phase: crate::workbench_client::DisplayPhase,
        label: Option<&str>,
        controller: Option<crate::workbench_client::DisplayController>,
        provenance: Option<crate::workbench_client::DisplayProvenance>,
        projection: crate::workbench_client::ClientProjectionStatus,
        retirement: Option<crate::workbench_client::DisplayRetirement>,
    ) -> String {
        use crate::workbench_client::{
            DisplayController as Controller, DisplayPhase as Phase,
            DisplayProvenance as Provenance, DisplayRetirement as Retirement,
        };

        if let Some(retirement) = retirement {
            return match retirement {
                Retirement::OutcomeUnknown => self
                    .terminal_status(crate::workbench_client::TerminalStatus::OutcomeUnknown)
                    .to_owned(),
                Retirement::Unavailable => self
                    .terminal_status(crate::workbench_client::TerminalStatus::Unavailable)
                    .to_owned(),
            };
        }

        match phase {
            Phase::Empty => match self {
                Self::EnUs => "No terminal tabs".to_owned(),
                Self::ZhCn => "没有终端标签".to_owned(),
            },
            Phase::Starting => self.native_starting_status(label, projection),
            Phase::Switching => self.native_switching_status(label, projection),
            Phase::Ready => {
                let Some(label) = label else {
                    return self
                        .terminal_status(crate::workbench_client::TerminalStatus::Unavailable)
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
                Self::ZhCn => "标签已移动 · 需要刷新".to_owned(),
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
                        .terminal_status(crate::workbench_client::TerminalStatus::Unavailable)
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
                    .terminal_status(crate::workbench_client::TerminalStatus::Unavailable)
                    .to_owned(),
            },
            Phase::Retired => self
                .terminal_status(crate::workbench_client::TerminalStatus::Unavailable)
                .to_owned(),
        }
    }

    pub(crate) fn native_refresh_status(
        self,
        status: crate::workbench_client::RefreshStatus,
    ) -> String {
        use crate::workbench_client::RefreshStatus;
        match status {
            RefreshStatus::Refreshing => self.native_refreshing().to_owned(),
            RefreshStatus::Success(revision) => self.native_refresh_success(revision),
            RefreshStatus::Failure(revision) => self.native_refresh_failure(revision),
        }
    }

    /// Adds only the matching operation's sanitized display context to a localized notice.
    pub(crate) fn native_notice_with_context(
        self,
        notice: crate::workbench_client::WorkbenchClientNotice,
        native: &crate::workbench_client::WorkbenchClientView<'_>,
    ) -> String {
        let message = self.native_notice(notice);
        if let Some(context) = native.notice_context() {
            use crate::workbench_client::NoticeContext;
            return match context {
                NoticeContext::LaunchProfile(name) => {
                    format!("{message} · {}", self.launch_profile_label(name.as_deref()))
                }
                NoticeContext::ConfiguredCommand(id) => format!(
                    "{} · {message}",
                    crate::terminal_text::sanitize_terminal_text(id)
                ),
            };
        }
        message
    }

    pub(crate) fn native_notice(
        self,
        notice: crate::workbench_client::WorkbenchClientNotice,
    ) -> String {
        use crate::workbench_client::WorkbenchClientNotice;
        match notice {
            WorkbenchClientNotice::Unsupported(capability) => self.unsupported_capability(capability),
            WorkbenchClientNotice::Settings(notice) => self.settings_notice(notice).to_owned(),
            WorkbenchClientNotice::AgentStart(phase) => self.agent_start_phase(phase).to_owned(),
            WorkbenchClientNotice::LocalBusy => self.native_local_busy().to_owned(),
            WorkbenchClientNotice::MenuUnavailable => match self {
                Self::EnUs => "Menu action cancelled or unavailable · open the menu again".into(),
                Self::ZhCn => "菜单动作已取消或不可用 · 请重新打开菜单".into(),
            },
            WorkbenchClientNotice::NoTabAtPosition(position) => self.native_no_tab(position),
            WorkbenchClientNotice::CreateNotCreated => self.native_create_not_created().to_owned(),
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
            WorkbenchClientNotice::NoWorkspace => self.no_current_workspace().to_owned(),
            WorkbenchClientNotice::RenameNotRenamed => match self {
                Self::EnUs => "Tab not renamed".to_owned(),
                Self::ZhCn => "标签页未重命名".to_owned(),
            },
            WorkbenchClientNotice::MoveNotMoved => match self {
                Self::EnUs => "Tab was not moved".to_owned(),
                Self::ZhCn => "标签未移动".to_owned(),
            },
            WorkbenchClientNotice::MoveOrderConflict => match self {
                Self::EnUs => {
                    "Tab not moved: preserve pinned Tabs and contiguous groups".to_owned()
                }
                Self::ZhCn => "标签未移动：需保留固定标签前缀和分组连续性".to_owned(),
            },
            WorkbenchClientNotice::PanelNotApplied => match self {
                Self::EnUs => "Panel operation was not applied".to_owned(),
                Self::ZhCn => "面板操作未执行".to_owned(),
            },
            WorkbenchClientNotice::ConfirmationCancelled => match self {
                Self::EnUs => "Close confirmation cancelled".to_owned(),
                Self::ZhCn => "关闭确认已取消".to_owned(),
            },
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
                    (Self::EnUs, F::InvalidIntent) => "Invalid request; reload configuration",
                    (Self::ZhCn, F::InvalidIntent) => "请求无效；请重载配置",
                    (Self::EnUs, F::SessionMismatch) => "Session changed; reconnect",
                    (Self::ZhCn, F::SessionMismatch) => "会话已变化；请重新连接",
                    (Self::EnUs, F::Unavailable) => "Unavailable; check Session and retry",
                    (Self::ZhCn, F::Unavailable) => "服务不可用；请检查会话后重试",
                    (Self::EnUs, F::Busy) => "Busy; wait and retry",
                    (Self::ZhCn, F::Busy) => "正在忙；请稍后重试",
                    (Self::EnUs, F::Conflict) => "Configuration changed; reload",
                    (Self::ZhCn, F::Conflict) => "配置已变化；请重载配置",
                    (Self::EnUs, F::NotFound) => "Command changed; reload configuration",
                    (Self::ZhCn, F::NotFound) => "命令已变化；请重载配置",
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
                Self::EnUs => "History is not available on this pane".to_owned(),
                Self::ZhCn => "此窗格历史不可用".to_owned(),
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
                Self::EnUs => "Copy mode is not available on this pane".into(),
                Self::ZhCn => "此窗格暂时无法进入复制模式".into(),
            },
            WorkbenchClientNotice::CopyModeBusy => match self {
                Self::EnUs => "Copy mode is busy; wait before trying again".into(),
                Self::ZhCn => "复制模式正在处理，请稍后再操作".into(),
            },
            WorkbenchClientNotice::CopyModeInvalidated => match self {
                Self::EnUs => "Copy mode ended because its content or pane changed".into(),
                Self::ZhCn => "内容或窗格已改变，复制模式已退出".into(),
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
                    "The saved Tab or pane is unavailable; showing the first available member.".into()
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
        projection: crate::workbench_client::ClientProjectionStatus,
    ) -> String {
        self.native_connection_status(
            label,
            projection,
            crate::workbench_client::TerminalStatus::Connecting,
        )
    }

    fn native_switching_status(
        self,
        label: Option<&str>,
        projection: crate::workbench_client::ClientProjectionStatus,
    ) -> String {
        self.native_connection_status(
            label,
            projection,
            crate::workbench_client::TerminalStatus::Unavailable,
        )
    }

    fn native_connection_status(
        self,
        label: Option<&str>,
        projection: crate::workbench_client::ClientProjectionStatus,
        missing_position: crate::workbench_client::TerminalStatus,
    ) -> String {
        use crate::workbench_client::ClientProjectionStatus;

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

    /// Names how many panes a zoomed pane hides behind it.
    pub(crate) fn zoomed_panes(self, count: usize) -> String {
        match self {
            Self::EnUs => format!("zoomed · {count} panes"),
            Self::ZhCn => format!("已放大 · 共 {count} 个窗格"),
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
