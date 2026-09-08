use crate::{
    client_view::{ClientAction, SessionActionDisposition},
    local_client::{LocalClient, LocalClientShutdown},
    terminal_input::{TerminalInputPort, TerminalInputUnavailable},
    terminal_projection::TerminalProjection,
};
use orchbee_client_server_protocol::{
    ControlInput, Geometry, RequestId, TerminalViewerAction, WorkingDirectory,
};
use std::{
    fs::File,
    io::{self, Read},
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread::{self, JoinHandle},
};

mod connection;
mod mailbox;
mod state;
#[cfg(target_os = "linux")]
mod status_recovery;
mod ui;
mod workbench_projection;

pub(crate) use ui::{
    NativeCloseConfirmationKind, NativeCloseConfirmationToken, NativeCloseConfirmationView,
    NativeCloseStatusKind, NativeCloseStatusToken, NativeCloseStatusView, NativeDisplayController,
    NativeDisplayPhase, NativeDisplayProvenance, NativeDisplayRetirement, NativeDrawReceipt,
    NativeProjectionStatus, NativeRefreshStatus, NativeRenameDraftToken, NativeRenameDraftView,
    NativeRenameEdit, NativeRenameInputContext, NativeSessionNotice, NativeSessionView,
    NativeTabView,
};

#[derive(Clone, Debug)]
pub(crate) enum TerminalIntent {
    Standalone,
    Create { cwd: Option<WorkingDirectory> },
    Attach(RequestId),
}

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
    Legacy {
        projection: Option<&'a TerminalProjection>,
        status: TerminalStatus,
    },
    Native(NativeSessionView<'a>),
}

pub(super) enum LegacyCommand {
    Input(ControlInput),
    Resize(Geometry),
    Poll,
    Feedback(TerminalViewerAction),
}

pub(super) enum LegacyEvent {
    Opened(TerminalProjection, bool),
    Polled(Option<Vec<u8>>),
    FeedbackAccepted,
    Failed(TerminalStatus),
}

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
    Legacy(Box<LegacySession>),
}

enum SessionTask {
    Native(JoinHandle<connection::NativeWorkerExit>),
    Legacy(JoinHandle<()>),
}

pub(crate) struct TerminalSession {
    backend: SessionBackend,
    task: Option<SessionTask>,
    shutdown: LocalClientShutdown,
    retirement_redraw_pending: bool,
    #[cfg(target_os = "linux")]
    origin: Option<crate::local_client::LocalClientOrigin>,
    #[cfg(target_os = "linux")]
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
                #[cfg(target_os = "linux")]
                let origin = client.origin();
                let mailbox = Arc::new(mailbox::CommandMailbox::new(identity_seed, geometry));
                let worker_mailbox = Arc::clone(&mailbox);
                let task = thread::Builder::new()
                    .name("orchbee-terminal-session".into())
                    .spawn(move || connection::run_native(client, geometry, worker_mailbox))?;
                Ok(Self {
                    backend: SessionBackend::Native(Box::new(ui::NativeUi::new(mailbox, clock))),
                    task: Some(SessionTask::Native(task)),
                    shutdown,
                    retirement_redraw_pending: false,
                    #[cfg(target_os = "linux")]
                    origin: Some(origin),
                    #[cfg(target_os = "linux")]
                    recovery_shutdown: None,
                })
            }
            TerminalIntent::Create { cwd } => Self::start_legacy(
                client,
                shutdown,
                LegacyIntent::Create { cwd },
                geometry,
                identity_seed,
            ),
            TerminalIntent::Attach(request_id) => Self::start_legacy(
                client,
                shutdown,
                LegacyIntent::Attach(request_id),
                geometry,
                identity_seed,
            ),
        }
    }

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
            #[cfg(target_os = "linux")]
            origin: None,
            #[cfg(target_os = "linux")]
            recovery_shutdown: None,
        })
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn startup_draw_ready(&mut self) -> bool {
        self.tick();
        matches!(&self.backend, SessionBackend::Native(native) if native.startup_draw_ready())
    }

    pub(crate) fn view(&self) -> TerminalSessionView<'_> {
        match &self.backend {
            SessionBackend::Native(native) => TerminalSessionView::Native(native.view()),
            SessionBackend::Legacy(legacy) => TerminalSessionView::Legacy {
                projection: legacy.projection.as_ref(),
                status: legacy.status(),
            },
        }
    }

    pub(crate) fn status(&self) -> TerminalStatus {
        match &self.backend {
            SessionBackend::Native(native) => native.status(),
            SessionBackend::Legacy(legacy) => legacy.status(),
        }
    }

    #[cfg(test)]
    pub(crate) fn projection(&self) -> Option<&TerminalProjection> {
        match &self.backend {
            SessionBackend::Native(native) => native.projection(),
            SessionBackend::Legacy(legacy) => legacy.projection.as_ref(),
        }
    }

    pub(crate) fn tick(&mut self) -> bool {
        let changed = match &mut self.backend {
            SessionBackend::Native(native) => native.tick(),
            SessionBackend::Legacy(legacy) => legacy.tick(&self.shutdown),
        };
        self.finalize_native_retirement();
        std::mem::take(&mut self.retirement_redraw_pending) || changed
    }

    // Only the shell's successful draw calls this, so an undrawn frame is never acknowledged.
    pub(crate) fn after_draw(&mut self) {
        self.retirement_redraw_pending = false;
        match &mut self.backend {
            SessionBackend::Native(native) => native.after_draw(),
            SessionBackend::Legacy(legacy) => legacy.after_draw(),
        }
        self.finalize_native_retirement();
    }

    pub(crate) fn after_close_draw(&mut self, receipt: NativeDrawReceipt) {
        if let SessionBackend::Native(native) = &mut self.backend {
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
        #[cfg(target_os = "linux")]
        if let Some(shutdown) = self.recovery_shutdown.take() {
            shutdown.close();
        }
        let (pending_create, pending_move, close_return, rename_return) = match self.task.take() {
            Some(SessionTask::Native(task)) => match task.join() {
                Ok(returned) => {
                    let (create, moved, close, renamed) = returned.into_pending_results();
                    (
                        create,
                        moved,
                        state::CloseWorkerReturn::Returned(close),
                        renamed,
                    )
                }
                Err(_) => {
                    let evidence = match &self.backend {
                        SessionBackend::Native(native) => native.close_worker_loss(),
                        SessionBackend::Legacy(_) => state::CloseWorkerReturn::Missing {
                            queued: None,
                            deposited: None,
                        },
                    };
                    let renamed = match &self.backend {
                        SessionBackend::Native(native) => native.rename_worker_loss(),
                        SessionBackend::Legacy(_) => None,
                    };
                    (None, None, evidence, renamed)
                }
            },
            Some(task @ SessionTask::Legacy(_)) => {
                self.task = Some(task);
                return;
            }
            None => return,
        };
        let SessionBackend::Native(native) = &mut self.backend else {
            return;
        };
        let _rename_accepted = native.finalize_rename_worker(rename_return);
        let _accepted = native.finalize_retirement(pending_create, pending_move, close_return);
        native.release_mailbox();
        // Geometry observation can finalize retirement before the shell asks tick for a redraw.
        self.retirement_redraw_pending = true;
    }
}

impl TerminalInputPort for TerminalSession {
    fn rename_input_context(&mut self) -> Option<NativeRenameInputContext> {
        match &mut self.backend {
            SessionBackend::Native(native) => native.rename_input_context(),
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
            SessionBackend::Legacy(_) => false,
        }
    }

    fn cancel_rename_draft(&mut self, token: NativeRenameDraftToken) {
        if let SessionBackend::Native(native) = &mut self.backend {
            native.cancel_rename_draft(token);
        }
    }

    fn submit_rename_draft(
        &mut self,
        token: NativeRenameDraftToken,
    ) -> Option<SessionActionDisposition> {
        let disposition = match &mut self.backend {
            SessionBackend::Native(native) => Some(native.submit_rename_draft(token)),
            SessionBackend::Legacy(_) => None,
        };
        self.finalize_native_retirement();
        disposition
    }

    fn invalidate_rename_display(&mut self) {
        if let SessionBackend::Native(native) = &mut self.backend {
            native.invalidate_rename_display();
        }
    }

    fn capture_confirmation_key(&mut self, key: crate::client_view::ClientKey) -> bool {
        let captured = match &mut self.backend {
            SessionBackend::Native(native) => native.capture_confirmation_key(key),
            SessionBackend::Legacy(_) => false,
        };
        self.finalize_native_retirement();
        captured
    }

    fn invalidate_close_display(&mut self) {
        if let SessionBackend::Native(native) = &mut self.backend {
            native.invalidate_close_display();
        }
    }
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        let accepted = match &mut self.backend {
            SessionBackend::Native(native) => native.forward(input),
            SessionBackend::Legacy(legacy) => legacy.forward(input).is_ok(),
        };
        self.finalize_native_retirement();
        accepted.then_some(()).ok_or(TerminalInputUnavailable)
    }

    fn apply_client_action(&mut self, action: ClientAction) -> Option<SessionActionDisposition> {
        #[cfg(target_os = "linux")]
        if action == ClientAction::OpenWorkbenchNavigator
            && self.task.is_none()
            && self.start_status_recovery()
        {
            return Some(SessionActionDisposition::KeepNavigator);
        }
        let disposition = match &mut self.backend {
            SessionBackend::Native(native) => Some(native.apply_client_action(action)),
            SessionBackend::Legacy(_) => None,
        };
        self.finalize_native_retirement();
        disposition
    }

    fn observe_geometry(&mut self, geometry: Geometry) {
        match &mut self.backend {
            SessionBackend::Native(native) => native.observe_geometry(geometry),
            SessionBackend::Legacy(legacy) => legacy.resize(geometry),
        }
        self.finalize_native_retirement();
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        match &mut self.backend {
            SessionBackend::Native(native) => native.close(state::CarrierFailure::Closed),
            SessionBackend::Legacy(legacy) => legacy.retire(legacy.status, &self.shutdown),
        }
        self.finalize_native_retirement();
        let _closed = self.shutdown.close();
        if let Some(SessionTask::Legacy(task)) = self.task.take() {
            let _joined = task.join();
        }
    }
}

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
                self.projection = Some(projection);
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
    File::open("/dev/urandom")?.read_exact(&mut seed)?;
    Ok(u128::from_be_bytes(seed))
}

pub(super) struct Identities(u128);

impl Identities {
    pub(super) fn next(&mut self) -> Result<RequestId, TerminalStatus> {
        self.0 = self.0.checked_add(1).ok_or(TerminalStatus::Unavailable)?;
        RequestId::parse(&format!("{:032x}", self.0)).map_err(|_| TerminalStatus::Unavailable)
    }
}

#[cfg(test)]
mod tests;
