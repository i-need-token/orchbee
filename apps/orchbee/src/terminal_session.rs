#[cfg(windows)]
mod client_control;
#[cfg(test)]
use crate::terminal_projection::TerminalProjection;
use crate::{
    client_view::{ClientAction, SessionActionDisposition},
    local_client::{LocalClient, LocalClientShutdown},
    terminal_input::{TerminalInputPort, TerminalInputUnavailable},
};
use orchbee_client_server_protocol::Geometry;
#[cfg(test)]
use orchbee_client_server_protocol::{
    ControlInput, RequestId, TerminalViewerAction, WorkingDirectory,
};
#[cfg(test)]
use std::io::Read;
use std::{
    io,
    sync::Arc,
    thread::{self, JoinHandle},
};

#[cfg(test)]
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};

#[cfg(windows)]
mod agent_attention;
mod agent_board;
#[cfg(windows)]
mod panel_reports;
#[cfg(all(windows, test))]
pub(crate) use panel_reports::PanelReport;
#[cfg(windows)]
pub(crate) use panel_reports::PanelReportView;
#[cfg(any(target_os = "linux", windows))]
mod agent_read_lane;
mod connection;
mod mailbox;
pub(crate) mod state;
#[cfg(any(target_os = "linux", windows))]
mod status_recovery;
pub(crate) mod terminal_viewport;
mod ui;
mod workbench_projection;

pub(crate) use workbench_projection::{
    NativeContextLocation, NativePanel, NativePanelTree, NativePresentation,
};

pub(crate) use ui::{
    AgentBoardDraw, AgentBoardRead, AgentBoardReadFailure, AgentBoardRow, AgentBoardSummary,
    JumpFailure, ModalPresentation, NativeAgentBoardView, NativeCloseConfirmationKind,
    NativeCloseConfirmationToken, NativeCloseConfirmationView, NativeCloseStatusKind,
    NativeCloseStatusToken, NativeCloseStatusView, NativeDisplayController, NativeDisplayPhase,
    NativeDisplayProvenance, NativeDisplayRetirement, NativeDrawReceipt, NativeLocationLabelStatus,
    NativeLocationLabelTarget, NativePanelCloseConfirmationToken, NativePanelCloseConfirmationView,
    NativeProjectionStatus, NativeRefreshStatus, NativeRenameDraftToken, NativeRenameDraftView,
    NativeRenameEdit, NativeRenameInputContext, NativeSessionNotice, NativeSessionView,
    NativeSetupDraftView, NativeSetupEdit, NativeSetupInputContext, NativeSetupStage,
    NativeSetupStatus, NativeSetupToken, NativeTabView, NativeViewportSurface,
    PresentationPlacement,
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

#[cfg(test)]
#[derive(Clone, Debug)]
pub(super) enum LegacyIntent {
    Create { cwd: Option<WorkingDirectory> },
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

pub(crate) enum TerminalSessionView<'a> {
    #[cfg(test)]
    Legacy {
        projection: Option<&'a TerminalProjection>,
        status: TerminalStatus,
    },
    Native(NativeSessionView<'a>),
}

#[cfg(test)]
pub(super) enum LegacyCommand {
    Input(ControlInput),
    Resize(Geometry),
    Poll,
    Feedback(TerminalViewerAction),
}

#[cfg(test)]
pub(super) enum LegacyEvent {
    Opened(Box<TerminalProjection>, bool),
    Polled(Option<Vec<u8>>),
    FeedbackAccepted,
    Failed(TerminalStatus),
}

#[cfg(test)]
struct LegacySession {
    commands: Option<SyncSender<LegacyCommand>>,
    events: Option<Receiver<LegacyEvent>>,
    status: TerminalStatus,
    projection: Option<TerminalProjection>,
    feedback: Option<TerminalViewerAction>,
    poll_pending: bool,
    geometry: Geometry,
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
    client_control: Option<client_control::ClientControl>,
    backend: SessionBackend,
    task: Option<SessionTask>,
    shutdown: LocalClientShutdown,
    retirement_redraw_pending: bool,
    #[cfg(any(target_os = "linux", windows))]
    origin: Option<crate::local_client::LocalClientOrigin>,
    #[cfg(any(target_os = "linux", windows))]
    recovery_shutdown: Option<connection::RecoveryShutdown>,
}

impl TerminalSession {
    pub(crate) fn start(
        client: LocalClient,
        intent: TerminalIntent,
        geometry: Geometry,
    ) -> io::Result<Self> {
        let shutdown = client.shutdown_handle()?;
        let clock = client.operation_clock();
        let identity_seed = random_identity_seed()?;
        match intent {
            TerminalIntent::Standalone => {
                #[cfg(any(target_os = "linux", windows))]
                let origin = client.origin();
                let mailbox = Arc::new(mailbox::CommandMailbox::new(identity_seed, geometry));
                let worker_mailbox = Arc::clone(&mailbox);
                let task = thread::Builder::new()
                    .name("orchbee-terminal-session".into())
                    .spawn(move || connection::run_native(client, geometry, worker_mailbox))?;
                #[cfg(windows)]
                let client_control = Some(client_control::ClientControl::new(
                    origin.clone(),
                    clock.clone(),
                ));
                let mut native = ui::NativeUi::new(mailbox, clock);
                // The Agent board's read lane reconnects on its own admitted
                // session so an OBAP ambiguity retires the lane, never the
                // primary carrier.
                #[cfg(any(target_os = "linux", windows))]
                native.set_agent_board_origin(origin.clone());
                Ok(Self {
                    #[cfg(windows)]
                    client_control,
                    backend: SessionBackend::Native(Box::new(native)),
                    task: Some(SessionTask::Native(task)),
                    shutdown,
                    retirement_redraw_pending: false,
                    #[cfg(any(target_os = "linux", windows))]
                    origin: Some(origin),
                    #[cfg(any(target_os = "linux", windows))]
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

    #[cfg(test)]
    fn start_legacy(
        client: LocalClient,
        shutdown: LocalClientShutdown,
        intent: LegacyIntent,
        geometry: Geometry,
        identity_seed: u128,
    ) -> io::Result<Self> {
        let (commands, receiver) = mpsc::sync_channel(8);
        let (sender, events) = mpsc::sync_channel(1);
        let task = thread::Builder::new()
            .name("orchbee-terminal-session".into())
            .spawn(move || {
                let result = connection::run_legacy(
                    client,
                    intent,
                    geometry,
                    Identities(identity_seed),
                    receiver,
                    &sender,
                );
                if let Err(status) = result {
                    let _delivered = sender.send(LegacyEvent::Failed(status));
                }
            })?;
        Ok(Self {
            #[cfg(windows)]
            client_control: None,
            backend: SessionBackend::Legacy(Box::new(LegacySession {
                commands: Some(commands),
                events: Some(events),
                status: TerminalStatus::Connecting,
                projection: None,
                feedback: None,
                poll_pending: true,
                geometry,
            })),
            task: Some(SessionTask::Legacy(task)),
            shutdown,
            retirement_redraw_pending: false,
            #[cfg(any(target_os = "linux", windows))]
            origin: None,
            #[cfg(any(target_os = "linux", windows))]
            recovery_shutdown: None,
        })
    }

    #[cfg(any(target_os = "linux", windows))]
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
        let changed = match &mut self.backend {
            SessionBackend::Native(native) => native.tick(),
            #[cfg(test)]
            SessionBackend::Legacy(legacy) => legacy.tick(&self.shutdown),
        };
        self.finalize_native_retirement();
        std::mem::take(&mut self.retirement_redraw_pending) || changed
    }

    // Only the shell's successful draw calls this, so an undrawn frame is never acknowledged.
    #[cfg(windows)]
    pub(crate) fn client_control_tick(
        &mut self,
        view: &mut crate::client_view::ClientView,
        viewport: ratatui::layout::Rect,
    ) -> bool {
        match (&mut self.client_control, &mut self.backend) {
            (Some(control), SessionBackend::Native(native)) => {
                control.observe_geometry(viewport, view.sidebar_surface());
                let changed =
                    control.tick(native, view.overlay().is_some() || view.prefix_active());
                if control.take_shell_mode_dismissal() {
                    view.dismiss_agent_focus_modes();
                }
                changed
            }
            _ => false,
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
            native.after_close_draw(receipt);
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
        #[cfg(any(target_os = "linux", windows))]
        if let Some(shutdown) = self.recovery_shutdown.take() {
            shutdown.close();
        }
        #[cfg(any(target_os = "linux", windows))]
        let mut retired_poll = None;
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
                    #[cfg(any(target_os = "linux", windows))]
                    {
                        retired_poll = returned.take_unavailable_poll();
                    }
                    let setup = returned.take_setup_result();
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
        #[cfg(any(target_os = "linux", windows))]
        let clean_return = pending_create.is_none()
            && pending_move.is_none()
            && pending_panel.is_none()
            && rename_return.is_none()
            && setup_return.is_none()
            && queued_setup_return.is_none()
            && matches!(close_return, state::CloseWorkerReturn::Returned(None));
        let rename_accepted = native.finalize_rename_worker(rename_return);
        let setup_accepted = native.finalize_setup_worker(setup_return);
        let queued_setup_accepted = native.finalize_setup_worker(queued_setup_return);
        #[cfg(any(target_os = "linux", windows))]
        native.finish_retired_peer_join(
            retired_poll,
            clean_return && rename_accepted && setup_accepted && queued_setup_accepted,
        );
        #[cfg(not(any(target_os = "linux", windows)))]
        let _accepted_setup_and_rename = (rename_accepted, setup_accepted, queued_setup_accepted);
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
    fn observe_focus(&mut self, focused: bool) {
        if let Some(native) = self.backend.native_mut() {
            native.observe_focus(focused);
        }
    }

    fn setup_input_context(&mut self) -> Option<NativeSetupInputContext> {
        self.backend
            .native_mut()
            .and_then(ui::NativeUi::setup_input_context)
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

    fn rename_input_context(&mut self) -> Option<NativeRenameInputContext> {
        match &mut self.backend {
            SessionBackend::Native(native) => native.rename_input_context(),
            #[cfg(test)]
            SessionBackend::Legacy(_) => None,
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

    fn capture_confirmation_key(&mut self, key: crate::client_view::ClientKey) -> bool {
        let captured = match &mut self.backend {
            SessionBackend::Native(native) => native.capture_confirmation_key(key),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        };
        self.finalize_native_retirement();
        captured
    }

    fn invalidate_close_display(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.invalidate_close_display();
        }
    }

    fn agent_board_input(&mut self) -> bool {
        match &mut self.backend {
            SessionBackend::Native(native) => native.agent_board_input(),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        }
    }

    fn capture_agent_board_key(&mut self, key: crate::client_view::KeyEvent) -> bool {
        match &mut self.backend {
            SessionBackend::Native(native) => native.capture_agent_board_key(&key),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        }
    }

    fn agent_board_select(
        &mut self,
        session: &orchbee_agent_observation_protocol::AgentSessionId,
    ) -> bool {
        match &mut self.backend {
            SessionBackend::Native(native) => native.agent_board_select(session),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        }
    }

    fn agent_board_scroll(&mut self, up: bool) -> bool {
        match &mut self.backend {
            SessionBackend::Native(native) => native.agent_board_scroll(up),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        }
    }

    fn agent_board_dismiss(&mut self) {
        if let Some(native) = self.backend.native_mut() {
            native.agent_board_dismiss();
        }
    }

    #[cfg(windows)]
    fn observe_input_intent(&mut self) {
        if let Some(control) = &mut self.client_control {
            control.interrupt_agent_focus();
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

    fn history_key(&mut self, event: &crate::client_view::KeyEvent) -> bool {
        let consumed = match &mut self.backend {
            SessionBackend::Native(native) => native.history_key(event),
            #[cfg(test)]
            SessionBackend::Legacy(_) => false,
        };
        self.finalize_native_retirement();
        consumed
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

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        #[cfg(windows)]
        if let Some(control) = &mut self.client_control {
            control.interrupt();
        }
        #[cfg(any(target_os = "linux", windows))]
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

#[cfg(test)]
impl LegacySession {
    fn status(&self) -> TerminalStatus {
        if self
            .projection
            .as_ref()
            .is_some_and(TerminalProjection::blocked)
        {
            TerminalStatus::Blocked
        } else {
            self.status
        }
    }

    fn tick(&mut self, shutdown: &LocalClientShutdown) -> bool {
        let event = self.events.as_ref().map(Receiver::try_recv);
        let changed = match event {
            Some(Ok(LegacyEvent::Opened(projection, controller))) => {
                self.projection = Some(*projection);
                self.poll_pending = false;
                self.status = if controller {
                    TerminalStatus::Controlling
                } else {
                    TerminalStatus::ReadOnly
                };
                true
            }
            Some(Ok(LegacyEvent::Polled(record))) => {
                if let Some(record) = record {
                    let result = self
                        .projection
                        .as_mut()
                        .ok_or(())
                        .and_then(|projection| projection.receive(&record));
                    match result {
                        Ok(feedback) => {
                            self.poll_pending = feedback.is_some();
                            self.feedback = feedback;
                        }
                        Err(()) => self.retire(TerminalStatus::Unavailable, shutdown),
                    }
                    true
                } else {
                    self.poll_pending = false;
                    false
                }
            }
            Some(Ok(LegacyEvent::FeedbackAccepted)) => {
                self.poll_pending = false;
                false
            }
            Some(Ok(LegacyEvent::Failed(status))) => {
                self.retire(status, shutdown);
                true
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.retire(TerminalStatus::Unavailable, shutdown);
                true
            }
            Some(Err(TryRecvError::Empty)) | None => false,
        };
        if !self.poll_pending
            && self.feedback.is_none()
            && matches!(
                self.status,
                TerminalStatus::Controlling | TerminalStatus::ReadOnly
            )
            && self.send(LegacyCommand::Poll).is_ok()
        {
            self.poll_pending = true;
        }
        changed || self.feedback.is_some()
    }

    fn after_draw(&mut self) {
        if let Some(feedback) = self.feedback.take()
            && self
                .send(LegacyCommand::Feedback(feedback.clone()))
                .is_err()
        {
            self.feedback = Some(feedback);
        }
    }

    fn resize(&mut self, geometry: Geometry) {
        if geometry != self.geometry
            && self.status == TerminalStatus::Controlling
            && self.send(LegacyCommand::Resize(geometry)).is_ok()
        {
            self.geometry = geometry;
        }
    }

    fn forward(&self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        if self.status != TerminalStatus::Controlling {
            return Err(TerminalInputUnavailable);
        }
        let input = ControlInput::new(input.to_vec()).map_err(|_| TerminalInputUnavailable)?;
        self.send(LegacyCommand::Input(input))
    }

    /// The single attached pane's pointer facts: one sentinel tab/panel
    /// pair the renderer's hit region re-resolves against, the session's
    /// controller mode, and the current frame's interaction contract.
    /// The pane is always the focused target — a legacy session has no
    /// workbench membership, so no focus switch is ever in flight.
    fn pointer_probe(&self) -> crate::terminal_input::PointerProbe {
        use crate::terminal_input::{PointerFrameFacts, PointerPane, PointerTab};
        let sentinel = crate::terminal_input::legacy_pane_ref();
        let controlling = match self.status {
            TerminalStatus::Controlling => Some(true),
            TerminalStatus::ReadOnly => Some(false),
            _ => None,
        };
        let mut probe = crate::terminal_input::PointerProbe {
            input_ready: self.status == TerminalStatus::Controlling,
            ..Default::default()
        };
        let Some(frame) = self.projection.as_ref().and_then(TerminalProjection::frame) else {
            return probe;
        };
        probe.focus = Some(sentinel.clone());
        probe.desired = Some(sentinel.pane().clone());
        probe.panes = vec![PointerPane {
            right_click: Default::default(),
            right_click_reporting: false,
            record: sentinel,
            attached: true,
            controlling,
            frame_available: true,
        }];
        probe.tabs = vec![PointerTab {
            id: crate::terminal_input::legacy_pane_tab(),
            position: 1,
        }];
        probe.frame = Some(PointerFrameFacts {
            tracking: frame.interaction.mouse_tracking(),
            format: frame.interaction.mouse_format(),
            cell_pixel: (
                frame.geometry.cell_pixel_width,
                frame.geometry.cell_pixel_height,
            ),
            alternate_screen: frame.alternate_screen,
            alternate_scroll: frame.interaction.alternate_scroll(),
            application_cursor: frame.interaction.application_cursor_keys(),
        });
        probe
    }

    fn send(&self, command: LegacyCommand) -> Result<(), TerminalInputUnavailable> {
        self.commands
            .as_ref()
            .ok_or(TerminalInputUnavailable)?
            .try_send(command)
            .map_err(|_| TerminalInputUnavailable)
    }

    fn retire(&mut self, status: TerminalStatus, shutdown: &LocalClientShutdown) {
        self.status = status;
        self.projection = None;
        self.feedback = None;
        self.commands.take();
        self.events.take();
        let _closed = shutdown.close();
    }
}

fn random_identity_seed() -> io::Result<u128> {
    let mut seed = [0_u8; 16];
    getrandom::fill(&mut seed).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(u128::from_be_bytes(seed))
}

#[cfg(test)]
pub(super) struct Identities(u128);

#[cfg(test)]
impl Identities {
    pub(super) fn next(&mut self) -> Result<RequestId, TerminalStatus> {
        self.0 = self.0.checked_add(1).ok_or(TerminalStatus::Unavailable)?;
        RequestId::parse(&format!("{:032x}", self.0)).map_err(|_| TerminalStatus::Unavailable)
    }
}

#[cfg(test)]
mod tests;
