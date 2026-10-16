#[cfg(windows)]
mod client_control;
mod completion_reminders;
pub(crate) use completion_reminders::Reminder as CompletionReminder;
#[cfg(windows)]
mod configuration;
mod settings_lane;
#[cfg(windows)]
mod window_title;
#[cfg(windows)]
mod worktree_lane;
#[cfg(test)]
use crate::terminal_projection::TerminalProjection;
use crate::{
    client_view::{ClientAction, SessionActionDisposition},
    local_client::{LocalClient, LocalClientShutdown},
    terminal_input::{TerminalInputPort, TerminalInputUnavailable},
};
use orchbee_client_server_protocol::Geometry;
#[cfg(test)]
use orchbee_client_server_protocol::{RequestId, WorkingDirectory};
use std::{
    io,
    sync::Arc,
    thread::{self, JoinHandle},
};

#[cfg(test)]
use tests::legacy::{LegacyIntent, LegacySession};

mod agent_attention;
mod agent_board;
mod agent_notifications;
#[cfg(windows)]
mod independent_popup;
#[cfg(windows)]
mod pane_graphics;
#[cfg(windows)]
mod panel_reports;
#[cfg(windows)]
pub(crate) use pane_graphics::{Canvas as PaneGraphicsCanvas, Draw as PaneGraphicsDraw};
#[cfg(windows)]
mod plugin_overlays;
#[cfg(all(windows, test))]
pub(crate) use independent_popup::PopupInputContext;
#[cfg(windows)]
pub(crate) use independent_popup::{
    DrawnPopupClose, PopupView, resolve_geometry as popup_geometry,
};
#[cfg(all(windows, test))]
pub(crate) use panel_reports::PanelReport;
#[cfg(windows)]
pub(crate) use panel_reports::PanelReportView;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod agent_read_lane;
mod connection;
mod mailbox;
#[cfg(windows)]
mod startup_directory;
#[cfg(windows)]
mod startup_preference;
#[cfg(windows)]
pub(crate) use startup_directory::StartupDirectory;
pub(crate) mod state;
pub(crate) use state::control_takeover::TakeoverStatus;
pub(crate) use state::workspace_setup::ProjectRenameRequest;
pub(crate) use state::workspace_setup::WorkspaceOrganizationRequest;
pub(crate) use state::workspace_setup::quick::ProjectQuickWorkspaceRequest;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod status_recovery;
mod terminal_clipboard;
mod terminal_effects;
mod terminal_selection;
mod terminal_tail;
pub(crate) mod terminal_viewport;
mod ui;
pub(crate) use ui::settings::Notice as SettingsNotice;
#[cfg(windows)]
pub(crate) use ui::{WorkspaceCreationPhase, WorkspaceCreationPreview};
mod wakeup;
mod workbench_projection;
pub(crate) use wakeup::UiWakeup;

#[cfg(windows)]
pub(crate) use workbench_projection::NativeContextTarget;
#[cfg(any(windows, test))]
pub(crate) use workbench_projection::NativeStartupTarget;
pub(crate) use workbench_projection::{
    NativeContextLocation, NativeMemberTarget, NativePanel, NativePanelTree, NativePresentation,
};

#[cfg(windows)]
pub(crate) use ui::{
    WorktreeEdit, WorktreeMode, WorktreeNotice, WorktreeStage, WorktreeToken, WorktreeView,
};

#[cfg(windows)]
pub(crate) use ui::RetainedFrameBank;
#[cfg(all(windows, test))]
pub(crate) use ui::WorktreeCaptureContext;
pub(crate) use ui::{
    AgentBoardActualDraw, AgentBoardControl, AgentBoardControlTarget, AgentBoardDraw,
    AgentBoardEpisodeToken, AgentBoardRead, AgentBoardReadFailure, AgentBoardRow,
    AgentBoardRowTarget, AgentBoardSummary, AgentBoardToken, CaptureReadiness,
    DrawnPresentationTarget, JumpFailure, ModalPresentation, NativeAgentBoardView,
    NativeCaptureContext, NativeCloseConfirmationKind, NativeCloseConfirmationToken,
    NativeCloseConfirmationView, NativeCloseStatusKind, NativeCloseStatusToken,
    NativeCloseStatusView, NativeDisplayController, NativeDisplayPhase, NativeDisplayProvenance,
    NativeDisplayRetirement, NativeDrawReceipt, NativeLocationLabelStatus,
    NativeLocationLabelTarget, NativePanelCloseConfirmationToken, NativePanelCloseConfirmationView,
    NativeProjectionStatus, NativeRefreshStatus, NativeRenameDraftToken, NativeRenameDraftView,
    NativeRenameEdit, NativeSessionNotice, NativeSessionView, NativeSetupDraftView,
    NativeSetupEdit, NativeSetupStage, NativeSetupStatus, NativeSetupToken, NativeTabView,
    NativeViewportSurface, PresentationPlacement, WorkspaceCloseConfirmationView,
    WorkspaceCloseStatus, WorkspaceCloseStatusView, WorkspaceCloseToken,
};
#[cfg(test)]
pub(crate) use ui::{
    AgentBoardInputContext, ConfirmationCaptureContext, NativeRenameInputContext,
    SetupCaptureContext,
};

#[derive(Clone, Debug)]
pub(crate) enum TerminalIntent {
    Standalone,
    #[cfg(test)]
    Create {
        cwd: Option<WorkingDirectory>,
    },
    #[cfg(test)]
    Attach(RequestId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalStatus {
    Connecting,
    Controlling,
    ReadOnly,
    Blocked,
    Unavailable,
    OutcomeUnknown,
}

/// Admission only: the existing Session still owns the accepted asynchronous handoff.
#[cfg(windows)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContextNavigationAdmission {
    Current,
    Admitted,
    Busy,
    Unavailable,
}

pub(crate) enum TerminalSessionView<'a> {
    #[cfg(test)]
    Legacy {
        projection: Option<&'a TerminalProjection>,
        status: TerminalStatus,
    },
    Native(NativeSessionView<'a>),
}

enum SessionBackend {
    Native(Box<ui::NativeUi>),
    #[cfg(test)]
    Legacy(Box<LegacySession>),
}

impl SessionBackend {
    fn native_mut(&mut self) -> Option<&mut ui::NativeUi> {
        match self {
            Self::Native(native) => Some(native),
            #[cfg(test)]
            Self::Legacy(_) => None,
        }
    }
}

enum SessionTask {
    Native(JoinHandle<connection::NativeWorkerExit>),
    #[cfg(test)]
    Legacy(JoinHandle<()>),
}

pub(crate) struct TerminalSession {
    #[cfg(windows)]
    position_writer: Option<startup_preference::Writer>,
    #[cfg(windows)]
    position_startup: Option<(Arc<std::sync::atomic::AtomicBool>, bool)>,
    #[cfg(windows)]
    configuration: Option<configuration::Watcher>,
    #[cfg(windows)]
    notifications: Option<crate::notifications::Subscription>,
    #[cfg(windows)]
    window_title: Option<window_title::Subscription>,
    #[cfg(windows)]
    client_control: Option<client_control::ClientControl>,
    backend: SessionBackend,
    task: Option<SessionTask>,
    shutdown: LocalClientShutdown,
    retirement_redraw_pending: bool,
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    origin: Option<crate::local_client::LocalClientOrigin>,
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    recovery_shutdown: Option<connection::RecoveryShutdown>,
}

impl TerminalSession {
    #[cfg(windows)]
    pub(crate) fn navigate_context(
        &mut self,
        target: &NativeContextTarget,
    ) -> ContextNavigationAdmission {
        self.backend
            .native_mut()
            .map_or(ContextNavigationAdmission::Unavailable, |native| {
                native.navigate_context(target)
            })
    }

    /// Bind or release the shell's waiter without transferring mailbox ownership.
    pub(crate) fn set_ui_wakeup(&mut self, wakeup: Option<UiWakeup>) {
        if let Some(native) = self.backend.native_mut() {
            native.set_ui_wakeup(wakeup);
        }
    }

    /// Time until an idle viewer poll becomes eligible; worker results wake sooner.
    pub(crate) fn next_poll_delay(&self) -> Option<std::time::Duration> {
        match &self.backend {
            SessionBackend::Native(native) => native.next_poll_delay(),
            #[cfg(test)]
            SessionBackend::Legacy(_) => None,
        }
    }

    #[cfg(windows)]
    pub(crate) fn retained_frame_bank(&self) -> Option<RetainedFrameBank> {
        match &self.backend {
            SessionBackend::Native(native) => Some(native.retained_frame_bank()),
            #[cfg(test)]
            SessionBackend::Legacy(_) => None,
        }
    }

    #[cfg(windows)]
    pub(crate) fn inherit_retained_frame_bank(&mut self, older: RetainedFrameBank) {
        if let Some(native) = self.backend.native_mut() {
            native.inherit_retained_frame_bank(older);
        }
    }

    #[cfg(windows)]
    pub(crate) fn suspend_machine(&mut self) -> Option<bool> {
        let focus = self.backend.native_mut().and_then(|native| {
            let focus = native.host_focus();
            native.observe_focus(false);
            focus
        });
        if let Some(mut control) = self.client_control.take() {
            control.close();
        }
        if let Some(mut listener) = self.notifications.take() {
            listener.close();
        }
        if let Some(mut watcher) = self.window_title.take() {
            watcher.close();
        }
        focus
    }
    #[cfg(windows)]
    pub(crate) fn resume_machine(&mut self, focus: Option<bool>) {
        if let Some(native) = self.backend.native_mut() {
            native.inherit_host_focus(focus);
            if self.client_control.is_none()
                && let Some(origin) = &self.origin
            {
                self.client_control = Some(client_control::ClientControl::new(
                    origin.clone(),
                    native.configuration_clock(),
                ));
            }
        }
    }

    pub(crate) fn start(
        client: LocalClient,
        intent: TerminalIntent,
        geometry: Geometry,
    ) -> io::Result<Self> {
        Self::start_with_context(client, intent, geometry, None)
    }

    #[cfg(windows)]
    pub(crate) fn start_target(
        client: LocalClient,
        target: NativeStartupTarget,
        geometry: Geometry,
    ) -> io::Result<Self> {
        Self::start_with_context(client, TerminalIntent::Standalone, geometry, Some(target))
    }

    #[cfg(windows)]
    pub(crate) fn start_local(
        client: LocalClient,
        geometry: Geometry,
        runtime: &std::path::Path,
        directory: StartupDirectory,
    ) -> io::Result<Self> {
        let preferences = startup_preference::Preferences::load(runtime);
        let fallback = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let context = Some(NativeStartupTarget::LocalDirectory {
            position: preferences.position.clone(),
            fallback: Arc::clone(&fallback),
            directory: Box::new(directory),
        });
        let mut failed = preferences.failed;
        let writer = match preferences.writer() {
            Ok(writer) => writer,
            Err(_) => {
                failed = true;
                None
            }
        };
        let mut session =
            Self::start_with_context(client, TerminalIntent::Standalone, geometry, context)?;
        session.position_writer = writer;
        session.position_startup = Some((fallback, failed));
        Ok(session)
    }

    #[cfg(windows)]
    pub(crate) fn finish_local_startup(&mut self) {
        if let Some((fallback, failed)) = self.position_startup.take()
            && let Some(native) = self.backend.native_mut()
        {
            if failed {
                native.report_position_notice(false);
            } else if fallback.load(std::sync::atomic::Ordering::Acquire) {
                native.report_position_notice(true);
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn machine_source(
        &self,
    ) -> Option<(
        crate::local_client::LocalClientOrigin,
        crate::local_client::LocalOperationClock,
    )> {
        match &self.backend {
            SessionBackend::Native(native) => {
                Some((self.origin.clone()?, native.configuration_clock()))
            }
            #[cfg(test)]
            SessionBackend::Legacy(_) => None,
        }
    }

    fn start_with_context(
        client: LocalClient,
        intent: TerminalIntent,
        geometry: Geometry,
        context: Option<workbench_projection::NativeStartupTarget>,
    ) -> io::Result<Self> {
        let shutdown = client.shutdown_handle()?;
        let clock = client.operation_clock();
        let identity_seed = random_identity_seed()?;
        match intent {
            TerminalIntent::Standalone => {
                #[cfg(any(target_os = "linux", target_os = "macos", windows))]
                let origin = client.origin();
                let mailbox = Arc::new(mailbox::CommandMailbox::new(identity_seed, geometry));
                let worker_mailbox = Arc::clone(&mailbox);
                let task = thread::Builder::new()
                    .name("orchbee-terminal-session".into())
                    .spawn(move || {
                        connection::run_native(client, geometry, worker_mailbox, context)
                    })?;
                #[cfg(windows)]
                let client_control = Some(client_control::ClientControl::new(
                    origin.clone(),
                    clock.clone(),
                ));
                let mut native = ui::NativeUi::new(mailbox, clock);
                // The Agent board's read lane reconnects on its own admitted
                // session so an OBAP ambiguity retires the lane, never the
                // primary carrier.
                #[cfg(any(target_os = "linux", target_os = "macos", windows))]
                native.set_agent_board_origin(origin.clone());
                Ok(Self {
                    #[cfg(windows)]
                    position_writer: None,
                    #[cfg(windows)]
                    position_startup: None,
                    #[cfg(windows)]
                    configuration: None,
                    #[cfg(windows)]
                    notifications: None,
                    #[cfg(windows)]
                    window_title: None,
                    #[cfg(windows)]
                    client_control,
                    backend: SessionBackend::Native(Box::new(native)),
                    task: Some(SessionTask::Native(task)),
                    shutdown,
                    retirement_redraw_pending: false,
                    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
                    origin: Some(origin),
                    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
                    recovery_shutdown: None,
                })
            }
            #[cfg(test)]
            TerminalIntent::Create { cwd } => Self::start_legacy(
                client,
                shutdown,
                LegacyIntent::Create { cwd },
                geometry,
                identity_seed,
            ),
            #[cfg(test)]
            TerminalIntent::Attach(request_id) => Self::start_legacy(
                client,
                shutdown,
                LegacyIntent::Attach(request_id),
                geometry,
                identity_seed,
            ),
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(crate) fn startup_draw_ready(&mut self) -> bool {
        self.tick();
        matches!(&self.backend, SessionBackend::Native(native) if native.startup_draw_ready())
    }

    pub(crate) fn view(&self) -> TerminalSessionView<'_> {
        match &self.backend {
            SessionBackend::Native(native) => TerminalSessionView::Native(native.view()),
            #[cfg(test)]
            SessionBackend::Legacy(legacy) => TerminalSessionView::Legacy {
                projection: legacy.projection.as_ref(),
                status: legacy.status(),
            },
        }
    }

    pub(crate) fn status(&self) -> TerminalStatus {
        match &self.backend {
            SessionBackend::Native(native) => native.status(),
            #[cfg(test)]
            SessionBackend::Legacy(legacy) => legacy.status(),
        }
    }

    #[cfg(test)]
    pub(crate) fn projection(&self) -> Option<&TerminalProjection> {
        match &self.backend {
            SessionBackend::Native(native) => native.projection(),
            #[cfg(test)]
            SessionBackend::Legacy(legacy) => legacy.projection.as_ref(),
        }
    }

    pub(crate) fn tick(&mut self) -> bool {
        self.tick_with_client_capture(crate::tui_composition::ClientCaptureClaims::default())
    }

    pub(crate) fn tick_with_client_capture(
        &mut self,
        client_capture: crate::tui_composition::ClientCaptureClaims,
    ) -> bool {
        #[cfg(windows)]
        let position_failed = self
            .position_writer
            .as_ref()
            .is_some_and(startup_preference::Writer::take_failure);
        let changed = match &mut self.backend {
            SessionBackend::Native(native) => native.tick_with_client_capture(client_capture),
            #[cfg(test)]
            SessionBackend::Legacy(legacy) => legacy.tick(&self.shutdown),
        };
        #[cfg(windows)]
        let changed = if position_failed {
            if let Some(native) = self.backend.native_mut() {
                native.report_position_notice(false);
            }
            true
        } else {
            changed
        };
        self.finalize_native_retirement();
        std::mem::take(&mut self.retirement_redraw_pending) || changed
    }

    #[cfg(test)]
    pub(crate) fn rename_input_context(&mut self) -> Option<NativeRenameInputContext> {
        match &mut self.backend {
            SessionBackend::Native(native) => native.rename_input_context(),
            SessionBackend::Legacy(_) => None,
        }
    }

    // Only the shell's successful draw calls this, so an undrawn frame is never acknowledged.
    #[cfg(windows)]
    pub(crate) fn configure_agent_notifications(
        &mut self,
        revision: u64,
        settings: &orchbee_local_site_runtime::WindowsNotificationSettings,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.configure_agent_notifications(revision, settings);
        }
    }

    pub(crate) fn automatic_notifications_tick(
        &mut self,
        view: &mut crate::client_view::ClientView,
        locale: crate::localization::Locale,
    ) -> bool {
        let mut changed = view.notifications.tick(std::time::Instant::now());
        if let Some(native) = self.backend.native_mut() {
            let automatic_accepting = view.notifications.empty()
                && (!native.agent_notification_requires_card()
                    || crate::notifications::available(
                        view,
                        Some(TerminalSessionView::Native(native.view())),
                    ));
            if let Some((delivery, message)) =
                native.take_agent_notification(locale, automatic_accepting)
            {
                changed |= view.notifications.install_automatic(
                    message,
                    delivery,
                    std::time::Instant::now(),
                );
            }
        }
        changed
    }

    pub(crate) fn shared_preferences_tick(
        &mut self,
        view: &mut crate::client_view::ClientView,
    ) -> bool {
        if let Some(native) = self.backend.native_mut()
            && let Some(snapshot) = native.take_settings_snapshot()
        {
            native.apply_user_preferences(&snapshot.values);
            view.apply_user_preferences(snapshot.values);
            return true;
        }
        false
    }

    #[cfg(windows)]
    pub(crate) fn client_control_tick(
        &mut self,
        view: &mut crate::client_view::ClientView,
        viewport: ratatui::layout::Rect,
        _locale: crate::localization::Locale,
    ) -> bool {
        let mut configuration_changed = false;
        match &mut self.backend {
            SessionBackend::Native(native) => {
                view.settle_project_quick_entry_intent();
                if !view.project_quick_entry_pending() {
                    native.cancel_project_quick_terminal_entry();
                }
                if native.take_project_quick_terminal_entry() {
                    configuration_changed |= view.complete_project_quick_entry();
                }
                view.settle_copy_mode(native.copy_mode_active());
                if self.configuration.is_none()
                    && let Some(origin) = &self.origin
                    && let Some((site, _)) = native.control_observation(false)
                {
                    self.configuration = configuration::Watcher::spawn(
                        origin.clone(),
                        native.configuration_clock(),
                        site,
                    )
                    .ok();
                }
                if let Some(watcher) = &mut self.configuration {
                    configuration_changed |= watcher.tick(view);
                    if let Some((revision, notifications)) = watcher.take_notification_update() {
                        native.cancel_action_menu();
                        native.cancel_copy_mode();
                        view.settle_copy_mode(false);
                        native.configure_agent_notifications(revision, &notifications);
                    }
                }
                if self.notifications.is_none()
                    && let Some(origin) = &self.origin
                    && let Some((site, _)) = native.control_observation(false)
                {
                    self.notifications = crate::notifications::Subscription::spawn(
                        origin.clone(),
                        native.configuration_clock(),
                        site,
                    )
                    .ok();
                }
                if self.window_title.is_none()
                    && let Some(origin) = &self.origin
                    && let Some((site, _)) = native.control_observation(false)
                {
                    self.window_title = window_title::Subscription::spawn(
                        origin.clone(),
                        native.configuration_clock(),
                        site,
                    )
                    .ok();
                }
                if let Some(watcher) = &self.window_title
                    && let Some(snapshot) = watcher.poll(
                        native.outer_title_focused(),
                        native.view().window_title_context(),
                    )
                {
                    view.window_title = Some(snapshot);
                }
                let accepting = (!view.notifications.card_delivery()
                    || crate::notifications::available(
                        view,
                        Some(TerminalSessionView::Native(native.view())),
                    ))
                    && view.notifications.empty();
                if let Some(listener) = &self.notifications
                    && let Some(message) = listener.poll(accepting)
                {
                    configuration_changed |= view
                        .notifications
                        .install(message, std::time::Instant::now());
                }
            }
            #[cfg(test)]
            SessionBackend::Legacy(_) => {}
        }
        match (&mut self.client_control, &mut self.backend) {
            (Some(control), SessionBackend::Native(native)) => {
                control.observe_geometry(viewport, view.sidebar_surface());
                let changed = control.tick(
                    native,
                    view.overlay().is_some()
                        || view.prefix_active()
                        || view.machines.open
                        || view.resize_mode()
                        || view.copy_mode_active(),
                );
                let mut local_resize_changed =
                    view.resize_pending != control.local_resize_pending();
                view.resize_pending = control.local_resize_pending();
                if let Some(feedback) = control.take_local_resize_feedback() {
                    local_resize_changed = true;
                    match feedback {
                        client_control::LocalResizeFeedback::Clear => view.clear_resize_notice(),
                        client_control::LocalResizeFeedback::Notice(notice) => {
                            view.configuration_notice(notice)
                        }
                    }
                }
                if control.take_shell_mode_dismissal() {
                    view.dismiss_focus_modes();
                }
                changed || configuration_changed || local_resize_changed
            }
            _ => configuration_changed,
        }
    }

    #[cfg(windows)]
    pub(crate) fn observe_popup_surface(
        &mut self,
        area: ratatui::layout::Rect,
        pixels: (u16, u16),
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.popup_surface(area, pixels);
        }
    }
    #[cfg(windows)]
    pub(crate) fn observe_client_viewport(
        &mut self,
        view: &crate::client_view::ClientView,
        viewport: ratatui::layout::Rect,
    ) {
        if let Some(control) = &mut self.client_control {
            control.observe_geometry(viewport, view.sidebar_surface());
        }
    }

    pub(crate) fn after_draw(&mut self) {
        self.retirement_redraw_pending = false;
        match &mut self.backend {
            SessionBackend::Native(native) => {
                native.after_draw();
                #[cfg(windows)]
                if let Some(control) = &mut self.client_control {
                    control.after_draw(native);
                }
            }
            #[cfg(test)]
            SessionBackend::Legacy(legacy) => legacy.after_draw(),
        }
        self.finalize_native_retirement();
    }

    pub(crate) fn after_close_draw(&mut self, receipt: NativeDrawReceipt) {
        if let Some(native) = self.backend.native_mut() {
            #[cfg(windows)]
            if let Some(writer) = self.position_writer.as_mut()
                && let Some(draw) = receipt.position()
                && native.view().accepts_position_draw(draw)
            {
                writer.publish(draw.position.clone());
            }
            native.after_close_draw(receipt);
        }
    }

    /// Delivers the admitted bell after a successful draw; an uncertain write is never replayed.
    ///
    /// # Errors
    /// Returns the terminal writer's failure after settling this attempt as unknown.
    pub(crate) fn deliver_terminal_effect(
        &mut self,
        emit_bell: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        if let Some(native) = self.backend.native_mut() {
            native.deliver_terminal_effect(emit_bell)?;
        }
        Ok(())
    }
    #[cfg(windows)]
    pub(crate) fn graphics_covered(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.after_graphics_draw(Vec::new());
        }
    }

    fn finalize_native_retirement(&mut self) {
        let requested = matches!(
            &self.backend,
            SessionBackend::Native(native) if native.retirement_requested()
        );
        if !requested {
            return;
        }
        let _closed = self.shutdown.close();
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        if let Some(shutdown) = self.recovery_shutdown.take() {
            shutdown.close();
        }
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        let mut retired_poll = None;
        let workspace_close_return;
        let takeover_return;
        let (
            pending_create,
            pending_move,
            pending_panel,
            close_return,
            rename_return,
            setup_return,
        ) = match self.task.take() {
            Some(SessionTask::Native(task)) => match task.join() {
                Ok(returned) => {
                    let mut returned = returned;
                    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
                    {
                        retired_poll = returned.take_unavailable_poll();
                    }
                    let setup = returned.take_setup_result();
                    workspace_close_return = returned.take_workspace_close_result();
                    takeover_return = returned.take_takeover_result();
                    let pending = returned.into_pending_results();
                    (
                        pending.create,
                        pending.moved,
                        pending.panel,
                        state::CloseWorkerReturn::Returned(pending.close),
                        pending.renamed,
                        setup,
                    )
                }
                Err(_) => {
                    takeover_return = match &self.backend {
                        SessionBackend::Native(native) => native.takeover_worker_loss(),
                        #[cfg(test)]
                        SessionBackend::Legacy(_) => None,
                    };
                    workspace_close_return = match &self.backend {
                        SessionBackend::Native(native) => native.workspace_close_worker_loss(),
                        #[cfg(test)]
                        SessionBackend::Legacy(_) => None,
                    };
                    let evidence = match &self.backend {
                        SessionBackend::Native(native) => native.close_worker_loss(),
                        #[cfg(test)]
                        SessionBackend::Legacy(_) => state::CloseWorkerReturn::Missing {
                            queued: None,
                            deposited: None,
                        },
                    };
                    let renamed = match &self.backend {
                        SessionBackend::Native(native) => native.rename_worker_loss(),
                        #[cfg(test)]
                        SessionBackend::Legacy(_) => None,
                    };
                    let setup = match &self.backend {
                        SessionBackend::Native(native) => native.setup_worker_loss(),
                        #[cfg(test)]
                        SessionBackend::Legacy(_) => None,
                    };
                    (None, None, None, evidence, renamed, setup)
                }
            },
            #[cfg(test)]
            Some(task @ SessionTask::Legacy(_)) => {
                self.task = Some(task);
                return;
            }
            None => return,
        };
        let Some(native) = self.backend.native_mut() else {
            return;
        };
        // The UI can admit the next wizard command after settling the previous result,
        // while that previous worker lease is still returning. Preserve both identities.
        let queued_setup_return = native.setup_worker_loss();
        let queued_workspace_close_return = native.workspace_close_worker_loss();
        let queued_takeover_return = native.takeover_worker_loss();
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        let clean_return = pending_create.is_none()
            && pending_move.is_none()
            && pending_panel.is_none()
            && rename_return.is_none()
            && setup_return.is_none()
            && queued_setup_return.is_none()
            && workspace_close_return.is_none()
            && queued_workspace_close_return.is_none()
            && takeover_return.is_none()
            && queued_takeover_return.is_none()
            && matches!(close_return, state::CloseWorkerReturn::Returned(None));
        let rename_accepted = native.finalize_rename_worker(rename_return);
        let setup_accepted = native.finalize_setup_worker(setup_return);
        let queued_setup_accepted = native.finalize_setup_worker(queued_setup_return);
        let workspace_close_accepted =
            native.finalize_workspace_close_worker(workspace_close_return);
        let queued_workspace_close_accepted =
            native.finalize_workspace_close_worker(queued_workspace_close_return);
        let takeover_accepted = native.finalize_takeover_worker(takeover_return);
        let queued_takeover_accepted = native.finalize_takeover_worker(queued_takeover_return);
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        native.finish_retired_peer_join(
            retired_poll,
            clean_return
                && rename_accepted
                && setup_accepted
                && queued_setup_accepted
                && workspace_close_accepted
                && queued_workspace_close_accepted
                && takeover_accepted
                && queued_takeover_accepted,
        );
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        let _accepted_setup_and_rename = (
            rename_accepted,
            setup_accepted,
            queued_setup_accepted,
            workspace_close_accepted,
            queued_workspace_close_accepted,
            takeover_accepted,
            queued_takeover_accepted,
        );
        let _accepted =
            native.finalize_retirement(pending_create, pending_move, pending_panel, close_return);
        // The board lane's blocked thread join precedes mailbox release —
        // draining lane outcomes is over before the mailbox can retire.
        native.release_agent_lane();
        native.release_mailbox();
        // Geometry observation can finalize retirement before the shell asks tick for a redraw.
        self.retirement_redraw_pending = true;
    }

    /// Reports every visible leaf's `(panel_id, panel_index, geometry)` so the
    /// native backend synchronizes its per-pane viewer membership. Only the
    /// Native backend tracks panes; the test Legacy backend keeps its single
    /// resize channel through `observe_geometry`.
    pub(crate) fn observe_panes(
        &mut self,
        panes: &[(
            orchbee_workbench_protocol::WorkbenchPanelId,
            usize,
            Geometry,
        )],
    ) {
        match &mut self.backend {
            SessionBackend::Native(native) => native.observe_panes(panes),
            #[cfg(test)]
            SessionBackend::Legacy(_) => {}
        }
        self.finalize_native_retirement();
    }
}

impl TerminalInputPort for TerminalSession {
    fn native_capture_claims(&self) -> crate::tui_composition::NativeCaptureClaims {
        match &self.backend {
            SessionBackend::Native(native) => native.capture_claims(),
            #[cfg(test)]
            SessionBackend::Legacy(_) => crate::tui_composition::NativeCaptureClaims::default(),
        }
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::NativeCaptureOwner,
    ) -> Option<NativeCaptureContext> {
        match &self.backend {
            SessionBackend::Native(native) => native.capture_context(owner),
            #[cfg(test)]
            SessionBackend::Legacy(_) => None,
        }
    }

    #[cfg(windows)]
    fn click_resize_control(
        &mut self,
        target: &crate::tui_layout::DrawnResize,
        direction: crate::client_view::PanelDirection,
        fine: bool,
    ) {
        if let (Some(native), Some(control)) =
            (self.backend.native_mut(), self.client_control.as_mut())
        {
            control.resize_drawn(native, target, direction, fine);
        }
    }
    fn click_tab_close(&mut self, token: NativeCloseConfirmationToken, confirm: bool) {
        if let Some(native) = self.backend.native_mut() {
            native.click_tab_close(token, confirm);
        }
        self.finalize_native_retirement();
    }
    fn click_panel_close(&mut self, token: NativePanelCloseConfirmationToken, confirm: bool) {
        if let Some(native) = self.backend.native_mut() {
            native.click_panel_close(token, confirm);
        }
        self.finalize_native_retirement();
    }
    fn click_workspace_close(&mut self, token: WorkspaceCloseToken, confirm: bool) {
        if let Some(native) = self.backend.native_mut() {
            native.click_workspace_close(token, confirm);
        }
        self.finalize_native_retirement();
    }
    fn capture_settings_key_for(
        &mut self,
        expected: crate::native_settings::SettingsInputContext,
        event: crate::client_view::KeyEvent,
    ) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.capture_settings_key_for(expected, event))
    }
    fn capture_settings_paste_for(
        &mut self,
        expected: crate::native_settings::SettingsInputContext,
        text: Option<&str>,
    ) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.capture_settings_paste_for(expected, text))
    }
    fn click_settings_control(
        &mut self,
        token: crate::native_settings::Token,
        control: crate::native_settings::Control,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.click_settings_control(token, control);
        }
    }
    fn scroll_settings(
        &mut self,
        token: crate::native_settings::Token,
        up: bool,
        lines: usize,
    ) -> Option<crate::native_settings::Token> {
        self.backend.native_mut()?.scroll_settings(token, up, lines)
    }
    fn click_settings(
        &mut self,
        token: crate::native_settings::Token,
        row: crate::native_settings::Row,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.click_settings(token, row);
        }
    }
    #[cfg(windows)]
    fn tab_create_action(
        &mut self,
        target: &crate::action_menu::DrawnTabCreate,
    ) -> Option<ClientAction> {
        self.backend.native_mut()?.tab_create_action(target)
    }
    #[cfg(windows)]
    fn open_action_menu(
        &mut self,
        target: crate::action_menu::MenuTarget,
        anchor: Option<(u16, u16)>,
        pointer_released: bool,
    ) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.open_action_menu(target, anchor, pointer_released))
    }
    #[cfg(windows)]
    fn capture_action_menu_key_for(
        &mut self,
        expected: crate::action_menu::MenuInputContext,
        event: crate::client_view::KeyEvent,
    ) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.capture_action_menu_key_for(expected, event))
    }
    #[cfg(windows)]
    fn hover_action_menu_row(
        &mut self,
        source: &crate::action_menu::MenuTarget,
        token: crate::action_menu::MenuToken,
        row: usize,
    ) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.hover_action_menu_row(source, token, row))
    }
    #[cfg(windows)]
    fn activate_action_menu_row(
        &mut self,
        source: &crate::action_menu::MenuTarget,
        token: crate::action_menu::MenuToken,
        row: usize,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.activate_action_menu_row(source, token, row);
        }
    }
    #[cfg(windows)]
    fn activate_action_menu_control(
        &mut self,
        source: &crate::action_menu::MenuTarget,
        token: crate::action_menu::MenuToken,
        control: crate::action_menu::MenuControl,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.activate_action_menu_control(source, token, control);
        }
    }
    #[cfg(windows)]
    fn scroll_action_menu(
        &mut self,
        token: crate::action_menu::MenuToken,
        up: bool,
        lines: usize,
    ) -> Option<crate::action_menu::MenuToken> {
        self.backend
            .native_mut()?
            .scroll_action_menu(token, up, lines)
    }
    #[cfg(windows)]
    fn cancel_action_menu(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.cancel_action_menu();
        }
    }
    #[cfg(windows)]
    fn take_action_menu_action(&mut self, pointer_released: bool) -> Option<ClientAction> {
        self.backend
            .native_mut()?
            .take_action_menu_action(pointer_released)
    }
    #[cfg(windows)]
    fn observe_host_graphics(&mut self, facts: crate::terminal_graphics::Facts) {
        if let Some(native) = self.backend.native_mut() {
            native.observe_host_graphics(facts);
        }
    }
    #[cfg(windows)]
    fn popup_input(
        &mut self,
        id: &orchbee_workbench_protocol::WorkbenchRequestId,
        item: &crate::terminal_input::DecodedItem,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.popup_input(id, item);
        }
    }
    #[cfg(windows)]
    fn popup_mouse(
        &mut self,
        id: &orchbee_workbench_protocol::WorkbenchRequestId,
        content: ratatui::layout::Rect,
        event: crate::terminal_input::MouseEvent,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.popup_mouse(id, content, event);
        }
    }
    #[cfg(windows)]
    fn activate_drawn_popup_close(&mut self, target: &DrawnPopupClose) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.activate_drawn_popup_close(target))
    }

    fn observe_focus(&mut self, focused: bool) {
        if let Some(native) = self.backend.native_mut() {
            native.observe_focus(focused);
        }
    }

    #[cfg(windows)]
    fn edit_worktree(&mut self, token: WorktreeToken, edit: WorktreeEdit<'_>) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|ui| ui.edit_worktree(token, edit))
    }
    #[cfg(windows)]
    fn dismiss_worktree(&mut self, token: WorktreeToken) {
        if let Some(ui) = self.backend.native_mut() {
            ui.dismiss_worktree(token);
        }
    }
    #[cfg(windows)]
    fn submit_worktree(&mut self, token: WorktreeToken) {
        if let Some(ui) = self.backend.native_mut() {
            ui.submit_worktree(token);
        }
    }
    #[cfg(windows)]
    fn choose_worktree(&mut self, token: WorktreeToken, index: usize) {
        if let Some(ui) = self.backend.native_mut() {
            ui.choose_worktree(token, index);
        }
    }
    #[cfg(windows)]
    fn scroll_worktree(
        &mut self,
        token: WorktreeToken,
        up: bool,
        lines: usize,
    ) -> Option<WorktreeToken> {
        self.backend
            .native_mut()
            .and_then(|ui| ui.scroll_worktree(token, up, lines))
    }
    #[cfg(windows)]
    fn invalidate_worktree_display(&mut self) {
        if let Some(ui) = self.backend.native_mut() {
            ui.after_worktree_draw(None);
        }
    }

    fn edit_setup_draft(&mut self, token: NativeSetupToken, edit: NativeSetupEdit<'_>) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.edit_setup_draft(token, edit))
    }

    fn cancel_setup_draft(&mut self, token: NativeSetupToken) {
        if let Some(native) = self.backend.native_mut() {
            native.cancel_setup_draft(token);
        }
    }

    fn choose_setup_project(&mut self, token: NativeSetupToken, index: usize) {
        if let Some(native) = self.backend.native_mut() {
            native.choose_setup_project(token, index);
        }
        self.finalize_native_retirement();
    }

    fn scroll_setup_projects(
        &mut self,
        token: NativeSetupToken,
        up: bool,
        lines: usize,
    ) -> Option<NativeSetupToken> {
        self.backend
            .native_mut()
            .and_then(|native| native.scroll_setup_projects(token, up, lines))
    }

    fn submit_setup_draft(&mut self, token: NativeSetupToken) -> Option<SessionActionDisposition> {
        let disposition = self
            .backend
            .native_mut()
            .map(|native| native.submit_setup_draft(token));
        self.finalize_native_retirement();
        disposition
    }

    fn invalidate_setup_display(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.invalidate_setup_display();
        }
    }

    fn edit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
        edit: NativeRenameEdit<'_>,
    ) -> bool {
        match &mut self.backend {
            SessionBackend::Native(native) => native.edit_rename_draft(token, edit),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        }
    }

    fn cancel_rename_draft(&mut self, token: NativeRenameDraftToken) {
        if let Some(native) = self.backend.native_mut() {
            native.cancel_rename_draft(token);
        }
    }

    fn submit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
    ) -> Option<SessionActionDisposition> {
        let disposition = match &mut self.backend {
            SessionBackend::Native(native) => Some(native.submit_rename_draft(token)),
            #[cfg(test)]
            SessionBackend::Legacy(_) => None,
        };
        self.finalize_native_retirement();
        disposition
    }

    fn invalidate_rename_display(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.invalidate_rename_display();
        }
    }

    fn capture_tab_confirmation_key_for(
        &mut self,
        token: NativeCloseConfirmationToken,
        key: crate::client_view::ClientKey,
    ) -> bool {
        let captured = self
            .backend
            .native_mut()
            .is_some_and(|native| native.capture_tab_confirmation_key(token, key));
        self.finalize_native_retirement();
        captured
    }

    fn capture_panel_confirmation_key_for(
        &mut self,
        token: NativePanelCloseConfirmationToken,
        key: crate::client_view::ClientKey,
    ) -> bool {
        let captured = self
            .backend
            .native_mut()
            .is_some_and(|native| native.capture_panel_confirmation_key(token, key));
        self.finalize_native_retirement();
        captured
    }

    fn capture_workspace_confirmation_key_for(
        &mut self,
        token: WorkspaceCloseToken,
        key: crate::client_view::ClientKey,
    ) -> bool {
        let captured = self
            .backend
            .native_mut()
            .is_some_and(|native| native.capture_workspace_close_key(token, key));
        self.finalize_native_retirement();
        captured
    }

    fn invalidate_close_display(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.invalidate_close_display();
        }
    }

    fn capture_agent_board_key_for(
        &mut self,
        episode: AgentBoardEpisodeToken,
        key: crate::client_view::KeyEvent,
    ) -> bool {
        match &mut self.backend {
            SessionBackend::Native(native) => native.capture_agent_board_key(episode, &key),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        }
    }

    fn agent_board_select(&mut self, target: &AgentBoardRowTarget) -> bool {
        match &mut self.backend {
            SessionBackend::Native(native) => native.agent_board_select(target),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        }
    }

    fn scroll_agent_board(
        &mut self,
        token: &AgentBoardToken,
        up: bool,
        lines: usize,
    ) -> Option<AgentBoardToken> {
        self.backend
            .native_mut()?
            .scroll_agent_board(token, up, lines)
    }

    fn click_agent_board_control(&mut self, target: &AgentBoardControlTarget) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.click_agent_board_control(target))
    }

    fn agent_board_dismiss(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.agent_board_dismiss();
        }
    }

    #[cfg(windows)]
    fn observe_input_intent(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.clear_settings_notice();
        }
        if let Some(control) = &mut self.client_control {
            control.interrupt_focus();
        }
    }

    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        let accepted = match &mut self.backend {
            SessionBackend::Native(native) => native.forward(input),
            #[cfg(test)]
            SessionBackend::Legacy(legacy) => legacy.forward(input).is_ok(),
        };
        self.finalize_native_retirement();
        accepted.then_some(()).ok_or(TerminalInputUnavailable)
    }

    fn pointer_probe(&mut self) -> crate::terminal_input::PointerProbe {
        match &mut self.backend {
            SessionBackend::Native(native) => native.pointer_probe(),
            #[cfg(test)]
            SessionBackend::Legacy(legacy) => legacy.pointer_probe(),
        }
    }

    #[cfg(windows)]
    fn navigator_catalog(&self) -> Option<crate::client_view::navigator::Catalog> {
        match &self.backend {
            SessionBackend::Native(native) => native.navigator_catalog(),
            #[cfg(test)]
            SessionBackend::Legacy(_) => None,
        }
    }

    #[cfg(windows)]
    fn activate_navigator(
        &mut self,
        row: &crate::client_view::navigator::Row,
    ) -> crate::client_view::navigator::Admission {
        let result = self.backend.native_mut().map_or(
            crate::client_view::navigator::Admission::Refused,
            |native| native.activate_navigator(row),
        );
        self.finalize_native_retirement();
        result
    }

    fn activate_drawn_presentation(
        &mut self,
        target: &DrawnPresentationTarget,
    ) -> SessionActionDisposition {
        let disposition = self
            .backend
            .native_mut()
            .map_or(SessionActionDisposition::KeepNavigator, |native| {
                native.activate_drawn_presentation(target)
            });
        self.finalize_native_retirement();
        disposition
    }

    fn pointer_selection(
        &mut self,
        key: &state::PaneRef,
        intent: crate::terminal_input::PointerSelectionIntent,
    ) -> crate::terminal_input::PointerSelectionAdmission {
        match &mut self.backend {
            SessionBackend::Native(native) => native.pointer_selection(key, intent),
            #[cfg(test)]
            SessionBackend::Legacy(_) => crate::terminal_input::PointerSelectionAdmission::Rejected,
        }
    }

    #[cfg(windows)]
    fn pointer_divider_ready(
        &mut self,
        target: &crate::tui_layout::DrawnDivider,
    ) -> Result<(), crate::client_view::Notice> {
        let native = self
            .backend
            .native_mut()
            .ok_or(crate::client_view::Notice::ResizeFailed)?;
        self.client_control
            .as_mut()
            .ok_or(crate::client_view::Notice::ResizeFailed)?
            .divider_ready(native, target)
    }

    #[cfg(windows)]
    fn divider_projection_current(
        &self,
        target: &crate::tui_layout::DrawnDivider,
        viewport: (ratatui::layout::Rect, crate::client_view::SidebarSurface),
    ) -> bool {
        match &self.backend {
            SessionBackend::Native(native) => native.divider_projection_current(target, viewport),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        }
    }

    #[cfg(windows)]
    fn sidebar_reorder_source(&mut self) -> Option<crate::tui_pointer::SidebarReorderSource> {
        self.backend.native_mut()?.sidebar_reorder_source()
    }
    #[cfg(windows)]
    fn commit_sidebar_reorder(
        &mut self,
        source: &crate::tui_pointer::SidebarReorderSource,
        operation: orchbee_workbench_protocol::NavigationConfiguration,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.commit_sidebar_reorder(source, operation);
        }
    }

    #[cfg(windows)]
    fn tab_drag_source(&mut self) -> Option<crate::tui_pointer::TabDragSource> {
        self.backend.native_mut()?.tab_drag_source()
    }

    #[cfg(windows)]
    fn commit_tab_drag(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        operation: orchbee_workbench_protocol::TabConfiguration,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.commit_tab_drag(source, operation);
        }
    }

    #[cfg(windows)]
    fn pointer_divider_resize(
        &mut self,
        target: &crate::tui_layout::DrawnDivider,
        ratio: orchbee_workbench_protocol::ClientSplitRatio,
    ) {
        if let Some(native) = self.backend.native_mut()
            && let Some(control) = &mut self.client_control
        {
            control.resize_divider(native, target, ratio);
        }
    }

    #[cfg(windows)]
    fn pending_resize_projection(
        &mut self,
        viewport: ratatui::layout::Rect,
        sidebar: crate::client_view::SidebarSurface,
        presentation_allowed: bool,
    ) -> crate::tui_layout::ResizeProjection {
        match (self.backend.native_mut(), self.client_control.as_mut()) {
            (Some(native), Some(control)) => {
                control.resize_projection(native, viewport, sidebar, presentation_allowed)
            }
            _ => crate::tui_layout::ResizeProjection::Canonical,
        }
    }

    #[cfg(windows)]
    fn pointer_link(
        &mut self,
        source: crate::terminal_link_activation::PaintedLinkSource,
        cell: crate::terminal_input::PointerSelectionCell,
    ) -> crate::terminal_input::PointerLinkAdmission {
        match &mut self.backend {
            SessionBackend::Native(native) => native.pointer_link(source, cell),
            #[cfg(test)]
            SessionBackend::Legacy(_) => crate::terminal_input::PointerLinkAdmission::Rejected,
        }
    }

    fn history_key(&mut self, event: &crate::client_view::KeyEvent) -> bool {
        let consumed = match &mut self.backend {
            SessionBackend::Native(native) => native.history_key(event),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        };
        self.finalize_native_retirement();
        consumed
    }

    #[cfg(windows)]
    fn copy_mode_active(&self) -> bool {
        match &self.backend {
            SessionBackend::Native(native) => native.copy_mode_active(),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        }
    }

    #[cfg(windows)]
    fn capture_copy_mode_key(&mut self, event: &crate::client_view::KeyEvent) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.capture_copy_mode_key(event))
    }

    #[cfg(windows)]
    fn click_copy_control(
        &mut self,
        token: crate::terminal_copy_mode::CopyModeToken,
        control: crate::terminal_copy_mode::CopyControl,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.click_copy_control(token, control);
        }
    }
    #[cfg(windows)]
    fn copy_pointer_cell(
        &mut self,
        token: crate::terminal_copy_mode::CopySurfaceToken,
        cell: crate::terminal_copy_mode::CopyModeCursor,
        start: bool,
    ) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.copy_pointer_cell(token, cell, start))
    }
    #[cfg(windows)]
    fn scroll_copy_mode(
        &mut self,
        token: crate::terminal_copy_mode::CopyModeToken,
        up: bool,
        lines: usize,
    ) -> Option<crate::terminal_copy_mode::CopyModeToken> {
        self.backend
            .native_mut()
            .and_then(|native| native.scroll_copy_mode(token, up, lines))
    }

    #[cfg(windows)]
    fn capture_copy_mode_paste(&mut self, text: Option<&str>) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.capture_copy_mode_paste(text))
    }

    #[cfg(windows)]
    fn cancel_copy_mode(&mut self) -> bool {
        self.backend
            .native_mut()
            .is_some_and(ui::NativeUi::cancel_copy_mode)
    }

    fn history_wheel(&mut self, key: &state::PaneRef, up: bool) -> bool {
        let consumed = match &mut self.backend {
            SessionBackend::Native(native) => native.history_wheel(key, up),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        };
        self.finalize_native_retirement();
        consumed
    }

    fn scrollbar_scroll(
        &mut self,
        key: &state::PaneRef,
        source: crate::terminal_scrollbar::ScrollbarSnapshot,
        offset: u32,
    ) -> bool {
        let consumed = self
            .backend
            .native_mut()
            .is_some_and(|native| native.scrollbar_scroll(key, source, offset));
        self.finalize_native_retirement();
        consumed
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        #[cfg(windows)]
        if let ClientAction::ResizePane { direction, fine } = action {
            let native = self.backend.native_mut()?;
            if let Some(control) = &mut self.client_control {
                control.resize(native, direction, fine);
            }
            return Some(SessionActionDisposition::KeepNavigator);
        }
        #[cfg(windows)]
        if matches!(
            action,
            ClientAction::ExitResizeMode
                | ClientAction::OpenShortcutHelp
                | ClientAction::CloseShortcutHelp
                | ClientAction::PageShortcutHelp
        ) && self
            .client_control
            .as_ref()
            .is_some_and(client_control::ClientControl::local_resize_pending)
        {
            return Some(SessionActionDisposition::KeepNavigator);
        }
        #[cfg(windows)]
        if let ClientAction::ConfiguredCommand(id) = &action {
            let native = self.backend.native_mut()?;
            if !self
                .client_control
                .as_mut()
                .is_some_and(|control| control.invoke_configured(native, id.clone()))
            {
                native.configured_command_completed(client_control::CommandCompletion {
                    reply: Err(false),
                    focus: None,
                });
            }
            return Some(SessionActionDisposition::KeepNavigator);
        }
        #[cfg(windows)]
        if action == ClientAction::ReloadConfiguration {
            return self
                .configuration
                .as_ref()
                .filter(|watcher| watcher.reload())
                .map(|_| SessionActionDisposition::KeepNavigator);
        }
        #[cfg(windows)]
        if let Some(control) = &mut self.client_control {
            control.interrupt();
        }
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        if action == ClientAction::OpenWorkbenchNavigator
            && self.task.is_none()
            && self.start_status_recovery()
        {
            return Some(SessionActionDisposition::KeepNavigator);
        }
        let disposition = match &mut self.backend {
            SessionBackend::Native(native) => Some(native.apply_client_action(action)),
            #[cfg(test)]
            SessionBackend::Legacy(_) => None,
        };
        self.finalize_native_retirement();
        disposition
    }

    fn observe_geometry(&mut self, geometry: Geometry) {
        match &mut self.backend {
            SessionBackend::Native(native) => native.observe_geometry(geometry),
            #[cfg(test)]
            SessionBackend::Legacy(legacy) => legacy.resize(geometry),
        }
        self.finalize_native_retirement();
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(watcher) = &mut self.window_title {
            watcher.close();
        }
        #[cfg(windows)]
        if let Some(listener) = &mut self.notifications {
            listener.close();
        }
        #[cfg(windows)]
        if let Some(watcher) = &mut self.configuration {
            watcher.close();
        }
        #[cfg(windows)]
        if let Some(control) = &mut self.client_control {
            control.close();
        }
        match &mut self.backend {
            SessionBackend::Native(native) => {
                native.close(state::CarrierFailure::Closed);
                // The lane's blocked read must finish before the mailbox is
                // released — the join happens inside finalize_retirement too,
                // but a retired-already session still owes the lane close.
                native.release_agent_lane();
            }
            #[cfg(test)]
            SessionBackend::Legacy(legacy) => legacy.retire(legacy.status, &self.shutdown),
        }
        self.finalize_native_retirement();
        let _closed = self.shutdown.close();
        #[cfg(test)]
        if let Some(SessionTask::Legacy(task)) = self.task.take() {
            let _joined = task.join();
        }
    }
}

fn random_identity_seed() -> io::Result<u128> {
    let mut seed = [0_u8; 16];
    getrandom::fill(&mut seed).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(u128::from_be_bytes(seed))
}

#[cfg(test)]
mod tests;
