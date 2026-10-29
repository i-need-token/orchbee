//! Owns the interactive Workbench Client's connection-task lifetime and composes feature
//! lanes, UI state, terminal input, and draw settlement. Server retains canonical membership
//! and Worker retains physical execution; retiring this Client does not terminate its retained
//! terminals.

mod client_control;
mod completion_reminders;
pub(crate) mod input;
pub use input::InputAction;
mod configuration;
mod settings_lane;
mod window_title;
mod worktree_lane;
#[cfg(test)]
use crate::terminal_projection::TerminalProjection;
use crate::{
    client_view::{ClientAction, ClientActionDisposition},
    local_client::{LocalClient, LocalClientShutdown},
    workbench_client::input::{TerminalInputPort, TerminalInputUnavailable},
};
use orchbee_client_server_protocol::Geometry;
use std::{
    io,
    sync::Arc,
    thread::{self, JoinHandle},
};

mod agent_attention;
mod agent_board;
mod agent_notifications;
mod independent_popup;
mod pane_graphics;
mod panel_reports;
pub use pane_graphics::Canvas as PaneGraphicsCanvas;
pub(crate) use pane_graphics::Draw as PaneGraphicsDraw;
mod plugin_overlays;
#[cfg(test)]
pub(crate) use independent_popup::PopupInputContext;
pub(crate) use independent_popup::{
    DrawnPopupClose, PopupView, resolve_geometry as popup_geometry,
};
#[cfg(test)]
pub(crate) use panel_reports::PanelReport;
pub(crate) use panel_reports::PanelReportView;
mod agent_read_lane;
mod connection;
mod mailbox;
mod normal_exit_hold;
mod primary_directory;
mod startup_directory;
mod startup_preference;
pub use startup_directory::StartupDirectory;
pub(crate) mod state;
pub(crate) use state::control_takeover::TakeoverStatus;
pub(crate) use state::workspace_close::WorkspaceDeleteRequest;
pub(crate) use state::workspace_setup::ProjectRenameRequest;
pub(crate) use state::workspace_setup::SidebarNavigationProjection;
pub(crate) use state::workspace_setup::WorkspaceOrganizationRequest;
pub(crate) use state::workspace_setup::quick::ProjectQuickWorkspaceRequest;
pub(crate) use state::worktree_launch::addressed::SidebarWorktreeTarget;
mod status_recovery;
mod terminal_clipboard;
mod terminal_effects;
mod terminal_selection;
mod terminal_tail;
pub(crate) mod terminal_viewport;
mod ui;
pub(crate) use ui::settings::Notice as SettingsNotice;
pub(crate) use ui::{WorkspaceCreationPhase, WorkspaceCreationPreview};
mod wakeup;
mod workbench_projection;
pub use wakeup::UiWakeup;

pub use workbench_projection::{
    ClientContextLocation, ClientContextTarget, ClientMemberTarget, ClientStartupTarget,
};
pub(crate) use workbench_projection::{ClientPanel, ClientPanelTree, ClientPresentation};

pub(crate) use ui::{
    WorktreeEdit, WorktreeMode, WorktreeNotice, WorktreeStage, WorktreeToken, WorktreeView,
};

#[cfg(any(test, feature = "test-support"))]
pub use ui::WorkbenchClientViewTestAccess;
#[cfg(test)]
pub(crate) use ui::WorktreeCaptureContext;
pub(crate) use ui::{
    AgentBoardActualDraw, AgentBoardControl, AgentBoardControlTarget, AgentBoardDraw,
    AgentBoardEpisodeToken, AgentBoardRead, AgentBoardRow, AgentBoardRowTarget, AgentBoardToken,
    AgentBoardView, CaptureReadiness, ClientProjectionStatus, ClientTabView, CloseConfirmationView,
    CloseStatusToken, DisplayController, DisplayProvenance, DrawnPresentationTarget, LaunchFailure,
    LocationLabelStatus, LocationLabelTarget, ModalPresentation, PanelCloseConfirmationToken,
    PanelCloseConfirmationView, PersistentStatus, PresentationPlacement, RefreshStatus,
    RenameDraftToken, RenameDraftView, RenameEdit, SetupDraftView, SetupEdit, SetupStage,
    SetupStatus, SetupToken, ViewportSurface, WorkbenchClientNotice,
    WorkspaceCloseConfirmationView,
};
#[cfg(test)]
pub(crate) use ui::{
    AgentBoardInputContext, CloseConfirmationKind, CloseStatusKind, ConfirmationCaptureContext,
    DisplayRetirement, LaunchProfile, SetupCaptureContext, WorkspaceCloseStatus,
};
pub(crate) use ui::{AgentStartCatalogStatus, AgentStartPhase};
pub use ui::{
    CaptureContext, CloseConfirmationToken, DisplayPhase, DrawReceipt, RetainedFrameBank,
    WorkbenchClientView, WorkspaceCloseToken,
};
#[cfg(any(test, feature = "test-support"))]
pub use workbench_projection::ClientMemberTargetFixture;

pub use crate::localization::TerminalStatus;

/// Admission only: the existing Client still owns the accepted asynchronous handoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextNavigationAdmission {
    Current,
    Admitted,
    Busy,
    Unavailable,
}

enum ClientBackend {
    Native(Box<ui::WorkbenchClientUi>),
}

impl ClientBackend {
    fn native_mut(&mut self) -> Option<&mut ui::WorkbenchClientUi> {
        match self {
            Self::Native(native) => Some(native),
        }
    }
}

enum ClientTask {
    Native(JoinHandle<connection::ClientConnectionExit>),
}

pub struct WorkbenchClient {
    position_writer: Option<startup_preference::Writer>,
    position_startup: Option<startup_preference::Startup>,
    configuration: Option<configuration::Watcher>,
    notifications: Option<crate::notifications::Subscription>,
    window_title: Option<window_title::Subscription>,
    client_control: Option<client_control::ClientControl>,
    backend: ClientBackend,
    task: Option<ClientTask>,
    shutdown: LocalClientShutdown,
    retirement_redraw_pending: bool,
    origin: Option<crate::local_client::LocalClientOrigin>,
    recovery_shutdown: Option<connection::RecoveryShutdown>,
}

impl WorkbenchClient {
    /// Reuse this Client for one physically verified, exact sidebar Agent destination.
    pub fn navigate_agent(
        &mut self,
        target: &crate::machine_agents::Target,
    ) -> ContextNavigationAdmission {
        self.backend
            .native_mut()
            .map_or(ContextNavigationAdmission::Unavailable, |native| {
                native.navigate_agent(target)
            })
    }

    pub fn navigate_context(&mut self, target: &ClientContextTarget) -> ContextNavigationAdmission {
        self.backend
            .native_mut()
            .map_or(ContextNavigationAdmission::Unavailable, |native| {
                native.navigate_context(target)
            })
    }

    /// Bind or release the shell's waiter without transferring mailbox ownership.
    pub fn set_ui_wakeup(&mut self, wakeup: Option<UiWakeup>) {
        if let Some(native) = self.backend.native_mut() {
            native.set_ui_wakeup(wakeup);
        }
    }

    /// Starts Client-owned Agent attention and Panel report reads without awaiting their interval.
    pub fn expedite_agent_reads(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.expedite_agent_reads();
        }
    }

    /// Time until an idle viewer poll becomes eligible; worker results wake sooner.
    pub fn next_poll_delay(&self) -> Option<std::time::Duration> {
        match &self.backend {
            ClientBackend::Native(native) => native.next_poll_delay(),
        }
    }

    pub fn retained_frame_bank(&self) -> Option<RetainedFrameBank> {
        match &self.backend {
            ClientBackend::Native(native) => Some(native.retained_frame_bank()),
        }
    }

    pub fn inherit_retained_frame_bank(&mut self, older: RetainedFrameBank) {
        if let Some(native) = self.backend.native_mut() {
            native.inherit_retained_frame_bank(older);
        }
    }

    pub fn suspend_machine(&mut self) -> Option<bool> {
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
    pub fn resume_machine(&mut self, focus: Option<bool>) {
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

    pub fn start(client: LocalClient, geometry: Geometry) -> io::Result<Self> {
        Self::start_with_context(client, geometry, None)
    }

    pub fn start_target(
        client: LocalClient,
        target: ClientStartupTarget,
        geometry: Geometry,
    ) -> io::Result<Self> {
        Self::start_with_context(client, geometry, Some(target))
    }

    pub fn start_local(
        client: LocalClient,
        geometry: Geometry,
        runtime: &std::path::Path,
        directory: StartupDirectory,
    ) -> io::Result<Self> {
        if !crate::tui_capabilities::Capability::PositionStorage.available() {
            return Self::start(client, geometry);
        }
        let preferences = startup_preference::Preferences::load(runtime);
        let fallback = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let context = Some(ClientStartupTarget::LocalDirectory {
            position: preferences.position.clone(),
            fallback: Arc::clone(&fallback),
            directory: Box::new(directory),
        });
        let mut failed = preferences.failed;
        let saved = preferences.position.clone();
        let writer = match preferences.writer() {
            Ok(writer) => writer,
            Err(_) => {
                failed = true;
                None
            }
        };
        let mut workbench_client = Self::start_with_context(client, geometry, context)?;
        workbench_client.position_writer = writer;
        workbench_client.position_startup = Some(startup_preference::Startup {
            fallback,
            failed,
            saved,
        });
        Ok(workbench_client)
    }

    pub fn finish_local_startup(&mut self) {
        if let Some(startup) = self.position_startup.take()
            && let Some(native) = self.backend.native_mut()
        {
            if startup.failed {
                native.report_position_notice(false);
            } else if startup.fallback.load(std::sync::atomic::Ordering::Acquire) {
                native.report_position_fallback(startup.saved.as_ref());
            }
        }
    }

    pub fn machine_source(
        &self,
    ) -> Option<(
        crate::local_client::LocalClientOrigin,
        crate::local_client::LocalOperationClock,
    )> {
        match &self.backend {
            ClientBackend::Native(native) => {
                Some((self.origin.clone()?, native.configuration_clock()))
            }
        }
    }

    fn start_with_context(
        client: LocalClient,
        geometry: Geometry,
        context: Option<workbench_projection::ClientStartupTarget>,
    ) -> io::Result<Self> {
        let client = client.into_multiplexed()?;
        let shutdown = client.connection_shutdown_handle()?;
        let clock = client.operation_clock();
        let identity_seed = random_identity_seed()?;
        let origin = client.origin();
        let mailbox = Arc::new(mailbox::CommandMailbox::new(identity_seed, geometry));
        let worker_mailbox = Arc::clone(&mailbox);
        let task = thread::Builder::new()
            .name("orchbee-workbench-client".into())
            .spawn(move || connection::run_native(client, geometry, worker_mailbox, context))?;
        let client_control = Some(client_control::ClientControl::new(
            origin.clone(),
            clock.clone(),
        ));
        let mut native = ui::WorkbenchClientUi::with_capabilities(
            mailbox,
            clock,
            crate::tui_capabilities::Capabilities::for_host(),
        );
        // Every feature uses a logical scope on this carrier. An ambiguous Agent read
        // retires that scope; only the shared connection actor owns physical reconnect.
        native.set_agent_board_origin(origin.clone());
        Ok(Self {
            position_writer: None,
            position_startup: None,
            configuration: None,
            notifications: None,
            window_title: None,
            client_control,
            backend: ClientBackend::Native(Box::new(native)),
            task: Some(ClientTask::Native(task)),
            shutdown,
            retirement_redraw_pending: false,
            origin: Some(origin),
            recovery_shutdown: None,
        })
    }

    pub fn startup_draw_ready(&mut self) -> bool {
        self.tick();
        matches!(&self.backend, ClientBackend::Native(native) if native.startup_draw_ready())
    }

    pub fn view(&self) -> WorkbenchClientView<'_> {
        match &self.backend {
            ClientBackend::Native(native) => native.view(),
        }
    }

    pub fn prepare_draw(&mut self, viewport: ratatui::layout::Rect, allow_defer: bool) -> bool {
        match &mut self.backend {
            ClientBackend::Native(native) => native.prepare_draw(viewport, allow_defer),
        }
    }

    pub fn status(&self) -> TerminalStatus {
        match &self.backend {
            ClientBackend::Native(native) => native.status(),
        }
    }

    #[cfg(test)]
    pub(crate) fn projection(&self) -> Option<&TerminalProjection> {
        match &self.backend {
            ClientBackend::Native(native) => native.projection(),
        }
    }

    pub fn tick(&mut self) -> bool {
        self.tick_with_client_capture(crate::tui_composition::ClientCaptureClaims::default())
    }

    pub fn tick_with_client_capture(
        &mut self,
        client_capture: crate::tui_composition::ClientCaptureClaims,
    ) -> bool {
        let position_failed = self
            .position_writer
            .as_ref()
            .is_some_and(startup_preference::Writer::take_failure);
        let changed = match &mut self.backend {
            ClientBackend::Native(native) => native.tick_with_client_capture(client_capture),
        };
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

    #[cfg(any(test, feature = "test-support"))]
    pub fn rename_input_context(&mut self) -> Option<ui::RenameInputContext> {
        match &mut self.backend {
            ClientBackend::Native(native) => native.rename_input_context(),
        }
    }

    // Only the shell's successful draw calls this, so an undrawn frame is never acknowledged.
    pub fn configure_agent_notifications(
        &mut self,
        revision: u64,
        settings: &orchbee_local_runtime::NotificationSettings,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.configure_agent_notifications(revision, settings);
        }
    }

    pub fn automatic_notifications_tick(
        &mut self,
        view: &mut crate::client_view::ClientView,
        locale: crate::localization::Locale,
    ) -> bool {
        let mut changed = view.notifications.tick(std::time::Instant::now());
        if let Some(native) = self.backend.native_mut() {
            let automatic_accepting = view.notifications.empty()
                && (!native.agent_notification_requires_card()
                    || crate::notifications::available(view, Some(native.view())));
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

    pub fn shared_preferences_tick(&mut self, view: &mut crate::client_view::ClientView) -> bool {
        if let Some(native) = self.backend.native_mut()
            && let Some(snapshot) = native.take_settings_snapshot()
        {
            native.apply_user_preferences(&snapshot.values);
            view.apply_user_preferences(snapshot.values);
            return true;
        }
        false
    }

    pub fn client_control_tick(
        &mut self,
        view: &mut crate::client_view::ClientView,
        viewport: ratatui::layout::Rect,
        locale: crate::localization::Locale,
    ) -> bool {
        let mut configuration_changed = false;
        match &mut self.backend {
            ClientBackend::Native(native) => {
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
                    && let Some((session, _)) = native.control_observation(false)
                {
                    self.configuration = configuration::Watcher::spawn(
                        origin.clone(),
                        native.configuration_clock(),
                        session,
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
                    if let Some(defaults) = watcher.take_creation_update() {
                        native.configure_panel_focus(defaults.focus_new_panel);
                        native
                            .configure_worktrees_directory(defaults.worktrees_directory.as_deref());
                    }
                }
                if self.notifications.is_none()
                    && let Some(origin) = &self.origin
                    && let Some((session, _)) = native.control_observation(false)
                {
                    self.notifications = crate::notifications::Subscription::spawn(
                        origin.clone(),
                        native.configuration_clock(),
                        session,
                    )
                    .ok();
                }
                if self.window_title.is_none()
                    && let Some(origin) = &self.origin
                    && let Some((session, _)) = native.control_observation(false)
                {
                    self.window_title = window_title::Subscription::spawn(
                        origin.clone(),
                        native.configuration_clock(),
                        session,
                    )
                    .ok();
                }
                if let Some(watcher) = &self.window_title
                    && let Some(snapshot) = watcher.poll(
                        native.outer_title_focused(),
                        native.view().window_title_context(locale),
                    )
                {
                    view.window_title = Some(snapshot);
                }
                let accepting = (!view.notifications.card_delivery()
                    || crate::notifications::available(view, Some(native.view())))
                    && view.notifications.empty();
                if let Some(listener) = &self.notifications
                    && let Some(message) = listener.poll(accepting)
                {
                    configuration_changed |= view
                        .notifications
                        .install(message, std::time::Instant::now());
                }
            }
        }
        match (&mut self.client_control, &mut self.backend) {
            (Some(control), ClientBackend::Native(native)) => {
                native.observe_host_colors(view.host_default_colors());
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

    pub fn observe_popup_surface(&mut self, area: ratatui::layout::Rect, pixels: (u16, u16)) {
        if let Some(native) = self.backend.native_mut() {
            native.popup_surface(area, pixels);
        }
    }
    pub fn observe_client_viewport(
        &mut self,
        view: &crate::client_view::ClientView,
        viewport: ratatui::layout::Rect,
    ) {
        if let Some(control) = &mut self.client_control {
            control.observe_geometry(viewport, view.sidebar_surface());
        }
    }

    pub fn after_draw(&mut self) {
        self.retirement_redraw_pending = false;
        match &mut self.backend {
            ClientBackend::Native(native) => {
                native.after_draw();
                if let Some(control) = &mut self.client_control {
                    control.after_draw(native);
                }
            }
        }
        self.finalize_native_retirement();
    }

    pub fn after_close_draw(&mut self, receipt: DrawReceipt) {
        if let Some(native) = self.backend.native_mut() {
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
    pub fn deliver_terminal_effect(
        &mut self,
        emit_bell: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        if let Some(native) = self.backend.native_mut() {
            native.deliver_terminal_effect(emit_bell)?;
        }
        Ok(())
    }
    pub fn graphics_covered(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.after_graphics_draw(Vec::new());
        }
    }

    fn finalize_native_retirement(&mut self) {
        let requested = matches!(
            &self.backend,
            ClientBackend::Native(native) if native.retirement_requested()
        );
        if !requested {
            return;
        }
        let _closed = self.shutdown.close();
        if let Some(shutdown) = self.recovery_shutdown.take() {
            shutdown.close();
        }
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
            Some(ClientTask::Native(task)) => match task.join() {
                Ok(returned) => {
                    let mut returned = returned;
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
                        ClientBackend::Native(native) => native.takeover_worker_loss(),
                    };
                    workspace_close_return = match &self.backend {
                        ClientBackend::Native(native) => native.workspace_close_worker_loss(),
                    };
                    let evidence = match &self.backend {
                        ClientBackend::Native(native) => native.close_worker_loss(),
                    };
                    let renamed = match &self.backend {
                        ClientBackend::Native(native) => native.rename_worker_loss(),
                    };
                    let setup = match &self.backend {
                        ClientBackend::Native(native) => native.setup_worker_loss(),
                    };
                    (None, None, None, evidence, renamed, setup)
                }
            },
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
    /// Client backend synchronizes its per-pane viewer membership.
    pub fn observe_panes(
        &mut self,
        panes: &[(
            orchbee_workbench_protocol::WorkbenchPanelId,
            usize,
            Geometry,
        )],
    ) {
        match &mut self.backend {
            ClientBackend::Native(native) => native.observe_panes(panes),
        }
        self.finalize_native_retirement();
    }
}

impl TerminalInputPort for WorkbenchClient {
    fn copy_picker_identifier(&mut self, text: String) -> bool {
        match &mut self.backend {
            ClientBackend::Native(native) => native.copy_picker_identifier(text),
        }
    }

    fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
        match &self.backend {
            ClientBackend::Native(native) => native.capture_claims(),
        }
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::WorkbenchCaptureOwner,
    ) -> Option<CaptureContext> {
        match &self.backend {
            ClientBackend::Native(native) => native.capture_context(owner),
        }
    }

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
    fn click_tab_close(&mut self, token: CloseConfirmationToken, confirm: bool) {
        if let Some(native) = self.backend.native_mut() {
            native.click_tab_close(token, confirm);
        }
        self.finalize_native_retirement();
    }
    fn click_panel_close(&mut self, token: PanelCloseConfirmationToken, confirm: bool) {
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
    fn tab_create_action(
        &mut self,
        target: &crate::action_menu::DrawnTabCreate,
    ) -> Option<ClientAction> {
        self.backend.native_mut()?.tab_create_action(target)
    }
    fn open_action_menu(
        &mut self,
        target: crate::action_menu::MenuTarget,
        anchor: Option<crate::action_menu::MenuAnchor>,
        pointer_released: bool,
    ) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.open_action_menu(target, anchor, pointer_released))
    }
    fn capture_action_menu_key_for(
        &mut self,
        expected: crate::action_menu::MenuInputContext,
        event: crate::client_view::KeyEvent,
    ) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.capture_action_menu_key_for(expected, event))
    }
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
    fn cancel_action_menu(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.cancel_action_menu();
        }
    }
    fn take_action_menu_action(&mut self, pointer_released: bool) -> Option<InputAction> {
        self.backend
            .native_mut()?
            .take_action_menu_action(pointer_released)
    }
    fn observe_host_graphics(&mut self, facts: crate::terminal_graphics::Facts) {
        if let Some(native) = self.backend.native_mut() {
            native.observe_host_graphics(facts);
        }
    }
    fn popup_input(
        &mut self,
        id: &orchbee_workbench_protocol::WorkbenchRequestId,
        item: &crate::terminal_events::DecodedItem,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.popup_input(id, item);
        }
    }
    fn popup_mouse(
        &mut self,
        id: &orchbee_workbench_protocol::WorkbenchRequestId,
        content: ratatui::layout::Rect,
        event: crate::terminal_events::MouseEvent,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.popup_mouse(id, content, event);
        }
    }
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

    fn edit_worktree(&mut self, token: WorktreeToken, edit: WorktreeEdit<'_>) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|ui| ui.edit_worktree(token, edit))
    }
    fn dismiss_worktree(&mut self, token: WorktreeToken) {
        if let Some(ui) = self.backend.native_mut() {
            ui.dismiss_worktree(token);
        }
    }
    fn submit_worktree(&mut self, token: WorktreeToken) {
        if let Some(ui) = self.backend.native_mut() {
            ui.submit_worktree(token);
        }
    }
    fn choose_worktree(&mut self, token: WorktreeToken, index: usize) {
        if let Some(ui) = self.backend.native_mut() {
            ui.choose_worktree(token, index);
        }
    }
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
    fn invalidate_worktree_display(&mut self) {
        if let Some(ui) = self.backend.native_mut() {
            ui.after_worktree_draw(None);
        }
    }

    fn edit_setup_draft(&mut self, token: SetupToken, edit: SetupEdit<'_>) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.edit_setup_draft(token, edit))
    }

    fn cancel_setup_draft(&mut self, token: SetupToken) {
        if let Some(native) = self.backend.native_mut() {
            native.cancel_setup_draft(token);
        }
    }

    fn choose_setup_project(&mut self, token: SetupToken, index: usize) {
        if let Some(native) = self.backend.native_mut() {
            native.choose_setup_project(token, index);
        }
        self.finalize_native_retirement();
    }

    fn scroll_setup_projects(
        &mut self,
        token: SetupToken,
        up: bool,
        lines: usize,
    ) -> Option<SetupToken> {
        self.backend
            .native_mut()
            .and_then(|native| native.scroll_setup_projects(token, up, lines))
    }

    fn submit_setup_draft(&mut self, token: SetupToken) -> Option<ClientActionDisposition> {
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

    fn edit_rename_draft(&mut self, token: RenameDraftToken, edit: RenameEdit<'_>) -> bool {
        match &mut self.backend {
            ClientBackend::Native(native) => native.edit_rename_draft(token, edit),
        }
    }

    fn cancel_rename_draft(&mut self, token: RenameDraftToken) {
        if let Some(native) = self.backend.native_mut() {
            native.cancel_rename_draft(token);
        }
    }

    fn submit_rename_draft(&mut self, token: RenameDraftToken) -> Option<ClientActionDisposition> {
        let disposition = match &mut self.backend {
            ClientBackend::Native(native) => Some(native.submit_rename_draft(token)),
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
        token: CloseConfirmationToken,
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
        token: PanelCloseConfirmationToken,
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
            ClientBackend::Native(native) => native.capture_agent_board_key(episode, &key),
        }
    }

    fn agent_board_select(&mut self, target: &AgentBoardRowTarget) -> bool {
        match &mut self.backend {
            ClientBackend::Native(native) => native.agent_board_select(target),
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
            ClientBackend::Native(native) => native.forward(input),
        };
        self.finalize_native_retirement();
        accepted.then_some(()).ok_or(TerminalInputUnavailable)
    }

    fn terminal_edited(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.terminal_edited();
        }
    }

    fn pointer_probe(&mut self) -> crate::workbench_client::input::PointerProbe {
        match &mut self.backend {
            ClientBackend::Native(native) => native.pointer_probe(),
        }
    }

    fn navigator_catalog(&self) -> Option<crate::client_view::navigator::Catalog> {
        match &self.backend {
            ClientBackend::Native(native) => native.navigator_catalog(),
        }
    }

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
    ) -> ClientActionDisposition {
        let disposition = self
            .backend
            .native_mut()
            .map_or(ClientActionDisposition::KeepNavigator, |native| {
                native.activate_drawn_presentation(target)
            });
        self.finalize_native_retirement();
        disposition
    }

    fn pointer_selection(
        &mut self,
        key: &state::PaneRef,
        intent: crate::workbench_client::input::PointerSelectionIntent,
    ) -> crate::workbench_client::input::PointerSelectionAdmission {
        match &mut self.backend {
            ClientBackend::Native(native) => native.pointer_selection(key, intent),
        }
    }

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

    fn divider_projection_current(
        &self,
        target: &crate::tui_layout::DrawnDivider,
        viewport: (ratatui::layout::Rect, crate::client_view::SidebarSurface),
    ) -> bool {
        match &self.backend {
            ClientBackend::Native(native) => native.divider_projection_current(target, viewport),
        }
    }

    fn sidebar_reorder_source(&mut self) -> Option<crate::tui_pointer::SidebarReorderSource> {
        self.backend.native_mut()?.sidebar_reorder_source()
    }
    fn commit_sidebar_reorder(
        &mut self,
        source: &crate::tui_pointer::SidebarReorderSource,
        operation: orchbee_workbench_protocol::NavigationConfiguration,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.commit_sidebar_reorder(source, operation);
        }
    }

    fn tab_drag_source(&mut self) -> Option<crate::tui_pointer::TabDragSource> {
        self.backend.native_mut()?.tab_drag_source()
    }

    fn request_drawn_tab_rename(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        tab: &orchbee_workbench_protocol::WorkbenchTabId,
    ) -> Option<ClientActionDisposition> {
        self.backend
            .native_mut()?
            .request_drawn_tab_rename(source, tab)
    }

    fn request_drawn_tab_close(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        tab: &orchbee_workbench_protocol::WorkbenchTabId,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.request_drawn_tab_close(source, tab);
        }
    }

    fn page_drawn_tab_strip(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        start: &crate::tui_pointer::TabStripStart,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.page_drawn_tab_strip(source, start);
        }
    }

    fn follow_drawn_tab_strip(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        tab: &orchbee_workbench_protocol::WorkbenchTabId,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.follow_drawn_tab_strip(source, tab);
        }
    }

    fn commit_tab_drag(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        operation: orchbee_workbench_protocol::TabConfiguration,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.commit_tab_drag(source, operation);
        }
    }

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

    fn pointer_link(
        &mut self,
        source: crate::terminal_link_activation::PaintedLinkSource,
        cell: crate::workbench_client::input::PointerSelectionCell,
    ) -> crate::workbench_client::input::PointerLinkAdmission {
        match &mut self.backend {
            ClientBackend::Native(native) => native.pointer_link(source, cell),
        }
    }

    fn history_key(&mut self, event: &crate::client_view::KeyEvent) -> bool {
        let consumed = match &mut self.backend {
            ClientBackend::Native(native) => native.history_key(event),
        };
        self.finalize_native_retirement();
        consumed
    }

    fn copy_mode_active(&self) -> bool {
        match &self.backend {
            ClientBackend::Native(native) => native.copy_mode_active(),
        }
    }

    fn capture_copy_mode_key(&mut self, event: &crate::client_view::KeyEvent) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.capture_copy_mode_key(event))
    }

    fn click_copy_control(
        &mut self,
        token: crate::terminal_copy_mode::CopyModeToken,
        control: crate::terminal_copy_mode::CopyControl,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.click_copy_control(token, control);
        }
    }
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

    fn capture_copy_mode_paste(&mut self, text: Option<&str>) -> bool {
        self.backend
            .native_mut()
            .is_some_and(|native| native.capture_copy_mode_paste(text))
    }

    fn cancel_copy_mode(&mut self) -> bool {
        self.backend
            .native_mut()
            .is_some_and(ui::WorkbenchClientUi::cancel_copy_mode)
    }

    fn history_wheel(&mut self, key: &state::PaneRef, up: bool) -> bool {
        let consumed = match &mut self.backend {
            ClientBackend::Native(native) => native.history_wheel(key, up),
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

    fn apply_client_action(&mut self, command: InputAction) -> Option<ClientActionDisposition> {
        let action = command.intent();
        if let Some(native) = self.backend.native_mut()
            && !native.admit_client_action(&action)
        {
            return Some(ClientActionDisposition::KeepNavigator);
        }
        if let ClientAction::ResizePane { direction, fine } = action {
            let native = self.backend.native_mut()?;
            if let Some(control) = &mut self.client_control {
                control.resize(native, direction, fine);
            }
            return Some(ClientActionDisposition::KeepNavigator);
        }
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
            return Some(ClientActionDisposition::KeepNavigator);
        }
        if let ClientAction::ConfiguredCommand(id) = &action {
            let native = self.backend.native_mut()?;
            if let Err(reason) = self
                .client_control
                .as_mut()
                .ok_or(orchbee_workbench_protocol::ConfiguredCommandFailure::Unavailable)
                .and_then(|control| control.invoke_configured(native, id.clone()))
            {
                native.configured_command_completed(client_control::CommandCompletion::refused(
                    id.clone(),
                    reason,
                ));
            }
            return Some(ClientActionDisposition::KeepNavigator);
        }
        if action == ClientAction::ReloadConfiguration {
            return self
                .configuration
                .as_ref()
                .filter(|watcher| watcher.reload())
                .map(|_| ClientActionDisposition::KeepNavigator);
        }
        if let Some(control) = &mut self.client_control {
            control.interrupt();
        }
        if action == ClientAction::OpenWorkbenchNavigator
            && self.task.is_none()
            && self.start_status_recovery()
        {
            return Some(ClientActionDisposition::KeepNavigator);
        }
        let disposition = match &mut self.backend {
            ClientBackend::Native(native) => Some(native.apply_client_action(command)),
        };
        self.finalize_native_retirement();
        disposition
    }

    fn observe_geometry(&mut self, geometry: Geometry) {
        match &mut self.backend {
            ClientBackend::Native(native) => native.observe_geometry(geometry),
        }
        self.finalize_native_retirement();
    }
}

impl Drop for WorkbenchClient {
    fn drop(&mut self) {
        if let Some(watcher) = &mut self.window_title {
            watcher.close();
        }
        if let Some(listener) = &mut self.notifications {
            listener.close();
        }
        if let Some(watcher) = &mut self.configuration {
            watcher.close();
        }
        if let Some(control) = &mut self.client_control {
            control.close();
        }
        match &mut self.backend {
            ClientBackend::Native(native) => {
                native.close(state::CarrierFailure::Closed);
                // The lane's blocked read must finish before the mailbox is
                // released — the join happens inside finalize_retirement too,
                // but a retired-already session still owes the lane close.
                native.release_agent_lane();
            }
        }
        self.finalize_native_retirement();
        let _closed = self.shutdown.close();
    }
}

fn random_identity_seed() -> io::Result<u128> {
    let mut seed = [0_u8; 16];
    getrandom::fill(&mut seed).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(u128::from_be_bytes(seed))
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
