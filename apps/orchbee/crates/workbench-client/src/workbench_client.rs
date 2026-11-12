//! Owns the interactive Workbench Client's connection-task lifetime and composes feature
//! lanes, UI state, terminal input, and draw settlement. Server retains canonical membership
//! and Worker retains physical execution; retiring this Client does not terminate its retained
//! terminals.

mod agent_catalog;
mod client_control;
mod command_notice;
mod completion_reminders;
mod copy_episode;
mod diagnostic_events;
pub(crate) mod input;
pub use input::InputAction;
mod configuration;
mod integration_lane;
mod settings_lane;
mod settings_notice;
mod terminal_titles;
mod window_title;
mod worktree_lane;
#[cfg(test)]
use crate::terminal_projection::TerminalProjection;
use crate::tui_hit_map::ListScrollTarget;
use crate::{
    background_thread::BackgroundThread,
    client_view::{ClientAction, ClientActionDisposition},
    local_client::{LocalClient, LocalClientShutdown},
    workbench_client::input::{
        LayoutAction, PaneAdmission, PaneInput, RenameAction, SetupAction, SurfaceAction,
        SurfaceInput, TerminalInputPort, TerminalInputUnavailable, TerminalObservation,
    },
};
use orchbee_client_server_protocol::Geometry;
use std::{io, sync::Arc};

mod agent_attention;
mod agent_notifications;
mod pane_graphics;
mod panel_reports;
pub use pane_graphics::Canvas as PaneGraphicsCanvas;
pub(crate) use pane_graphics::Draw as PaneGraphicsDraw;
mod plugin_overlays;
#[cfg(test)]
pub(crate) use crate::independent_popup::PopupInputContext;
pub(crate) use crate::independent_popup::resolve_geometry as popup_geometry;
#[cfg(test)]
pub(crate) use panel_reports::PanelReport;
pub(crate) use panel_reports::PanelReportView;
mod agent_read_lane;
mod connection;
mod mailbox;
mod normal_exit_hold;
pub use crate::startup_directory::{ClientStartupTarget, StartupDirectory};
mod status_recovery;
mod terminal_clipboard;
mod terminal_effects;
mod terminal_selection;
mod terminal_tail;
mod ui;
pub use crate::ui_wakeup::UiWakeup;

pub use crate::workbench_projection::{
    ClientContextLocation, ClientContextTarget, ClientMemberTarget,
};
pub(crate) use crate::workbench_projection::{ClientPanel, ClientPanelTree, ClientPresentation};

#[cfg(any(test, feature = "test-support"))]
pub use crate::workbench_projection::ClientMemberTargetFixture;
#[cfg(test)]
pub(crate) use ui::KeyedProjection;
#[cfg(any(test, feature = "test-support"))]
pub use ui::WorkbenchClientViewTestAccess;
#[cfg(test)]
pub(crate) use ui::WorktreeCaptureContext;
#[cfg(test)]
pub(crate) use ui::{
    AgentBoardInputContext, CloseConfirmationKind, CloseStatusKind, ConfirmationCaptureContext,
    DisplayRetirement, LaunchProfile, SetupCaptureContext, WorkspaceCloseStatus,
};
pub use ui::{
    CaptureContext, DisplayPhase, DrawReceipt, NavigationHold, RetainedFrameBank,
    WorkbenchClientView, notification_available,
};
pub(crate) use ui::{
    CaptureReadiness, ClientTabView, CloseConfirmationView, HeldRestoreView,
    PanelCloseConfirmationView, RenameDraftView, RenameEdit, ViewportSurface,
};
#[cfg(any(test, feature = "test-support"))]
pub use ui::{CloseConfirmationToken, WorkspaceCloseToken};

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

/// The mailbox samples the Client Connection's operation clock through its own clock port.
impl mailbox::MailboxClock for crate::local_client::LocalOperationClock {
    fn now(&self) -> std::time::Instant {
        crate::local_client::LocalOperationClock::now(self)
    }
}

impl ClientBackend {
    fn native_mut(&mut self) -> Option<&mut ui::WorkbenchClientUi> {
        match self {
            Self::Native(native) => Some(native),
        }
    }
}

enum ClientTask {
    Native(BackgroundThread<connection::ClientConnectionExit>),
}

pub struct WorkbenchClient {
    position_writer: Option<crate::startup_preference::store::Writer>,
    position_startup: Option<crate::startup_preference::Startup>,
    configuration: Option<configuration::Watcher>,
    notifications: Option<crate::notifications::subscription::Subscription>,
    window_title: Option<window_title::Subscription>,
    terminal_titles: Option<terminal_titles::Subscription>,
    client_control: Option<ui::control_steps::ClientControl>,
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

    /// Reports the user's direct navigation that the shell still holds for this Client because
    /// an earlier switch or a read keeps it from admission, or `None` once it was admitted or the
    /// selection ended. Meanwhile keys aim at its destination and held invocations wait for it.
    pub fn observe_direct_navigation(&mut self, target: Option<&ClientContextTarget>) {
        if let Some(native) = self.backend.native_mut() {
            native.observe_direct_navigation(target);
        }
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
        if let Some(mut titles) = self.terminal_titles.take() {
            titles.close();
        }
        focus
    }
    pub fn resume_machine(&mut self, focus: Option<bool>) {
        if let Some(native) = self.backend.native_mut() {
            native.inherit_host_focus(focus);
            if self.client_control.is_none()
                && let Some(origin) = &self.origin
            {
                self.client_control = Some(ui::control_steps::ClientControl::new(
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
            // Without a remembered position an existing Session opens on its first Workspace, as
            // `start` does; an empty one still starts in the launch directory.
            let context = Some(ClientStartupTarget::LocalDirectory {
                position: None,
                fallback: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                directory: Box::new(directory),
            });
            return Self::start_with_context(client, geometry, context);
        }
        let preferences = crate::startup_preference::store::Preferences::load(runtime);
        let fallback = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let context = Some(ClientStartupTarget::LocalDirectory {
            position: preferences.position.clone(),
            fallback: Arc::clone(&fallback),
            directory: Box::new(directory),
        });
        let mut failed = preferences.failed;
        let unreadable = preferences.unreadable;
        let saved = preferences.position.clone();
        let writer = match preferences.writer() {
            Ok(writer) => writer,
            Err(_) => {
                failed = true;
                None
            }
        };
        // Records the Server holds are read there; a read-only inspection shows them here too, so a
        // refused record never leaves the TUI silently empty.
        let session = runtime.file_name().and_then(|name| name.to_str());
        let record = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name))
            .ok()
            .map(|root| orchbee_local_runtime::inspect_records(&root, session))
            .unwrap_or_default()
            .into_iter()
            .chain(orchbee_local_runtime::reported_record_notices())
            .next();
        let mut workbench_client = Self::start_with_context(client, geometry, context)?;
        workbench_client.position_writer = writer;
        workbench_client.position_startup = Some(crate::startup_preference::Startup {
            record,
            fallback,
            failed,
            unreadable,
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
            } else if startup.unreadable
                || startup.fallback.load(std::sync::atomic::Ordering::Acquire)
            {
                native.report_position_fallback(startup.saved.as_ref());
            }
            if let Some(record) = startup.record {
                native.report_record_notice(record.record, record.problem);
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
        context: Option<crate::startup_directory::ClientStartupTarget>,
    ) -> io::Result<Self> {
        let client = client.into_multiplexed()?;
        let shutdown = client.connection_shutdown_handle()?;
        let clock = client.operation_clock();
        let identity_seed = random_identity_seed()?;
        let origin = client.origin();
        let mailbox = Arc::new(mailbox::CommandMailbox::new(identity_seed, geometry));
        let worker_mailbox = Arc::clone(&mailbox);
        let task = BackgroundThread::spawn("orchbee-workbench-client", move || {
            connection::run_native(client, geometry, worker_mailbox, context)
        })?;
        let client_control = Some(ui::control_steps::ClientControl::new(
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
            terminal_titles: None,
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

    pub fn prepare_draw(
        &mut self,
        viewport: ratatui::layout::Rect,
        sidebar: crate::client_view::SidebarSurface,
        allow_defer: bool,
    ) -> NavigationHold {
        match &mut self.backend {
            ClientBackend::Native(native) => native.prepare_draw(viewport, sidebar, allow_defer),
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
            .is_some_and(crate::startup_preference::store::Writer::take_failure);
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
                    || ui::notification_available(view, Some(native.view())));
            if let Some((delivery, message, target)) =
                native.take_agent_notification(locale, automatic_accepting)
            {
                changed |= view.notifications.install_automatic(
                    message,
                    delivery,
                    target,
                    std::time::Instant::now(),
                );
            }
            native.observe_notification_target(view.notifications.agent_card().is_some());
        }
        changed
    }

    pub fn shared_preferences_tick(&mut self, view: &mut crate::client_view::ClientView) -> bool {
        let Some(native) = self.backend.native_mut() else {
            return false;
        };
        let repaint = native.configure_reduced_color(!view.named_themes_available());
        if let Some(snapshot) = native.take_settings_snapshot() {
            // ClientView is the single writer of confirmed preferences; the UI receives the
            // owner's values in the same settlement, before any input or paint observes them.
            view.settings_file(&snapshot.file);
            view.apply_user_preferences(orchbee_local_runtime::snapshot_values(&snapshot));
            if let Some(values) = view.user_preferences() {
                native.apply_user_preferences(values);
            }
            return true;
        }
        repaint
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
                // A held zoom or focus move that the native tick replayed changed the drawn
                // panes: the Shell re-observes geometry and the pointer map as it does after a
                // dispatched one.
                configuration_changed |= native.take_replayed_layout_change();
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
                    // The watcher configured `view.notifications`, the policy's single writer;
                    // the UI receives that policy with the revision that admitted it.
                    if let Some(revision) = watcher.take_notification_update() {
                        native.cancel_action_menu();
                        native.cancel_copy_mode();
                        view.settle_copy_mode(native.copy_mode_active());
                        native.configure_agent_notifications(revision, view.notifications.policy());
                    }
                    if let Some(defaults) = watcher.take_creation_update() {
                        native.configure_panel_focus(defaults.focus_new_panel);
                        native
                            .configure_worktrees_directory(defaults.worktrees_directory.as_deref());
                    }
                }
                native.observe_session_agents(crate::agent_board::SessionAgents::from_rows(
                    view.machines.current_agent_rows(),
                ));
                if self.notifications.is_none()
                    && let Some(origin) = &self.origin
                    && let Some((session, _)) = native.control_observation(false)
                {
                    self.notifications = crate::notifications::subscription::Subscription::spawn(
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
                if let Some(origin) = &self.origin
                    && let Some((session, _)) = native.control_observation(false)
                    && self
                        .terminal_titles
                        .as_ref()
                        .is_none_or(|titles| titles.follows_another(&session))
                {
                    self.terminal_titles = Some(terminal_titles::Subscription::spawn(
                        origin.clone(),
                        native.configuration_clock(),
                        session,
                    ));
                }
                if let Some(titles) = &mut self.terminal_titles {
                    let rows = titles.poll(native.title_presentations());
                    configuration_changed |= native.receive_presentation_titles(rows);
                }
                let accepting = (!view.notifications.card_delivery()
                    || ui::notification_available(view, Some(native.view())))
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
                native.observe_host_colors(view.terminal_query_colors());
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
                        ui::control_steps::LocalResizeFeedback::Clear => view.clear_resize_notice(),
                        ui::control_steps::LocalResizeFeedback::Notice(notice) => {
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

    /// Delivers the admitted bell, or a copy routed through the outer terminal, after a successful
    /// draw; an uncertain write is never replayed.
    ///
    /// # Errors
    /// Returns the terminal writer's failure after settling this attempt as unknown.
    pub fn deliver_terminal_effect(
        &mut self,
        emit: &mut dyn FnMut(crate::terminal_input::ModeRequest) -> io::Result<()>,
    ) -> io::Result<()> {
        if let Some(native) = self.backend.native_mut() {
            native.deliver_terminal_effect(emit)?;
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
            // Retirement and reconnection still wait for the connection task's pending results.
            Some(ClientTask::Native(mut task)) => {
                match task.join().map_or(Err(()), |joined| joined.map_err(drop)) {
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
                            crate::workbench_state::CloseWorkerReturn::Returned(pending.close),
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
                }
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
            && matches!(
                close_return,
                crate::workbench_state::CloseWorkerReturn::Returned(None)
            );
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

    /// Applies one measured host geometry, then settles any retirement it made due.
    pub(crate) fn observe_geometry(&mut self, geometry: Geometry) {
        match &mut self.backend {
            ClientBackend::Native(native) => native.observe_geometry(geometry),
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

    fn open_notification_target(
        &mut self,
        target: &crate::machine_agents::Target,
    ) -> crate::client_view::notifications::TargetOpening {
        match &mut self.backend {
            ClientBackend::Native(native) => native.open_notification_target(target),
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

    fn surface_action(&mut self, action: SurfaceAction<'_>) -> Option<ClientActionDisposition> {
        // Close decisions, rename and setup submissions, setup choices, and presentation
        // activation may request native retirement, which settles before the dispatcher observes
        // the result. No other committed result retires the native Client.
        let settles = matches!(
            action,
            SurfaceAction::Close(_)
                | SurfaceAction::Rename(RenameAction::Submit(_))
                | SurfaceAction::Setup(SetupAction::Submit(_) | SetupAction::Choose { .. })
                | SurfaceAction::Presentation(_)
        );
        let ClientBackend::Native(native) = &mut self.backend;
        let disposition = match action {
            // Layout resizing hands off to the client-control lane that this composition owns.
            SurfaceAction::Layout(LayoutAction::ResizeControl {
                target,
                direction,
                fine,
            }) => {
                if let Some(control) = self.client_control.as_mut() {
                    control.resize_drawn(native, target, direction, fine);
                }
                None
            }
            SurfaceAction::Layout(LayoutAction::Divider { target, ratio }) => {
                if let Some(control) = self.client_control.as_mut() {
                    control.resize_divider(native, target, ratio);
                }
                None
            }
            action => native.surface_action(action),
        };
        if settles {
            self.finalize_native_retirement();
        }
        disposition
    }

    fn surface_input(&mut self, input: SurfaceInput<'_>) -> bool {
        // A close-confirmation key may request native retirement, which settles before the
        // dispatcher observes the result. No other local input retires the native Client.
        let settles = matches!(
            input,
            SurfaceInput::TabClose { .. }
                | SurfaceInput::PanelClose { .. }
                | SurfaceInput::WorkspaceClose { .. }
        );
        let ClientBackend::Native(native) = &mut self.backend;
        let consumed = native.surface_input(input);
        if settles {
            self.finalize_native_retirement();
        }
        consumed
    }
    fn scroll_list(
        &mut self,
        target: &ListScrollTarget,
        up: bool,
        lines: usize,
    ) -> Option<ListScrollTarget> {
        let ClientBackend::Native(native) = &mut self.backend;
        native.scroll_list(target, up, lines)
    }
    fn tab_create_action(
        &mut self,
        target: &crate::action_menu::DrawnTabCreate,
    ) -> Option<ClientAction> {
        self.backend.native_mut()?.tab_create_action(target)
    }
    fn observe_sidebar_git(
        &mut self,
        directory: Option<orchbee_workbench_protocol::WorkbenchDirectoryPath>,
    ) {
        if let Some(native) = self.backend.native_mut() {
            native.observe_sidebar_git(directory);
        }
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
    fn take_action_menu_action(&mut self, pointer_released: bool) -> Option<InputAction> {
        self.backend
            .native_mut()?
            .take_action_menu_action(pointer_released)
    }

    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        let accepted = match &mut self.backend {
            ClientBackend::Native(native) => native.forward(input),
        };
        self.finalize_native_retirement();
        accepted.then_some(()).ok_or(TerminalInputUnavailable)
    }

    fn navigation_pending(&self) -> bool {
        match &self.backend {
            ClientBackend::Native(native) => native.navigation_pending(),
        }
    }

    fn unresolved_press(&mut self) -> bool {
        self.backend
            .native_mut()
            .is_some_and(ui::WorkbenchClientUi::unresolved_press)
    }

    fn observe_terminal(&mut self, observation: TerminalObservation) {
        match observation {
            TerminalObservation::Focus(focused) => {
                if let Some(native) = self.backend.native_mut() {
                    native.observe_focus(focused);
                }
            }
            TerminalObservation::Geometry(geometry) => self.observe_geometry(geometry),
            TerminalObservation::HostGraphics(facts) => {
                if let Some(native) = self.backend.native_mut() {
                    native.observe_host_graphics(facts);
                }
            }
            TerminalObservation::InputIntent => {
                if let Some(native) = self.backend.native_mut() {
                    native.clear_settings_notice();
                }
                if let Some(control) = &mut self.client_control {
                    control.interrupt_focus();
                }
            }
            TerminalObservation::KeyboardIntent => {
                if let Some(native) = self.backend.native_mut() {
                    native.observe_keyboard();
                }
            }
            TerminalObservation::DisplayInvalidated => {
                if let Some(native) = self.backend.native_mut() {
                    native.invalidate_close_display();
                    native.invalidate_rename_display();
                    native.invalidate_setup_display();
                    native.after_worktree_draw(None);
                }
            }
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

    fn pane_input(&mut self, input: PaneInput<'_>) -> PaneAdmission {
        // Selection and link steps never retire native state, so only history and scrollbar
        // steps settle retirement.
        let settles = !matches!(input, PaneInput::Selection { .. } | PaneInput::Link { .. });
        let ClientBackend::Native(native) = &mut self.backend;
        let admission = native.pane_input(input);
        if settles {
            self.finalize_native_retirement();
        }
        admission
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
            ClientBackend::Native(native) => {
                native.divider_projection_current(target, viewport)
                    || self
                        .client_control
                        .as_ref()
                        .is_some_and(|control| control.settling_paints(native, target, viewport))
            }
        }
    }

    fn sidebar_reorder_source(&mut self) -> Option<input::SidebarReorderSource> {
        self.backend.native_mut()?.sidebar_reorder_source()
    }

    fn tab_drag_source(&mut self) -> Option<input::TabDragSource> {
        self.backend.native_mut()?.tab_drag_source()
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

    fn copy_mode_active(&self) -> bool {
        match &self.backend {
            ClientBackend::Native(native) => native.copy_mode_active(),
        }
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
            .is_some_and(ui::control_steps::ClientControl::local_resize_pending)
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
}

impl WorkbenchClient {
    /// Process exit: asks every session-owned background thread to stop. Never waits, and
    /// abandons the connection task's pending results.
    pub fn request_stop(&mut self) {
        if let Some(watcher) = &mut self.window_title {
            watcher.close();
        }
        if let Some(titles) = &mut self.terminal_titles {
            titles.close();
        }
        if let Some(listener) = &mut self.notifications {
            listener.close();
        }
        if let Some(watcher) = &mut self.configuration {
            watcher.close();
        }
        if let Some(control) = &mut self.client_control {
            control.request_stop();
        }
        if let Some(native) = self.backend.native_mut() {
            native.request_stop();
        }
        let _closed = self.shutdown.close();
        if let Some(shutdown) = &self.recovery_shutdown {
            shutdown.close();
        }
        if let Some(writer) = &mut self.position_writer {
            writer.request_stop();
        }
    }

    /// Process exit: after every owner was asked to stop, observes each owner's thread against
    /// one deadline. Every owner is observed even after a failure; Ok means every thread ended.
    ///
    /// # Errors
    /// TimedOut when a thread is still running at the deadline; it is detached on drop.
    pub fn stop_before(&mut self, deadline: std::time::Instant) -> io::Result<()> {
        self.request_stop();
        let control = self
            .client_control
            .as_mut()
            .map_or(Ok(()), |control| control.stop_before(deadline));
        let native = self
            .backend
            .native_mut()
            .map_or(Ok(()), |native| native.stop_before(deadline));
        let task = match &mut self.task {
            Some(ClientTask::Native(task)) => task.join_before(deadline),
            None => Ok(()),
        };
        let writer = self
            .position_writer
            .as_mut()
            .map_or(Ok(()), |writer| writer.stop_before(deadline));
        control.and(native).and(task).and(writer)
    }
}

impl Drop for WorkbenchClient {
    fn drop(&mut self) {
        // stop_before owns completion at process exit. Dropping only requests stop; a thread
        // that has not ended is detached and finishes on its own.
        self.request_stop();
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
