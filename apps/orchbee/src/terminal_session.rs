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
use std::{
    fs::File,
    io::{self, Read},
    sync::Arc,
    thread::{self, JoinHandle},
};

#[cfg(test)]
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};

mod connection;
mod mailbox;
mod state;
#[cfg(target_os = "linux")]
mod status_recovery;
mod ui;
mod workbench_projection;

pub(crate) use workbench_projection::{NativePanel, NativePanelTree};

pub(crate) use ui::{
    NativeCloseConfirmationKind, NativeCloseConfirmationToken, NativeCloseConfirmationView,
    NativeCloseStatusKind, NativeCloseStatusToken, NativeCloseStatusView, NativeDisplayController,
    NativeDisplayPhase, NativeDisplayProvenance, NativeDisplayRetirement, NativeDrawReceipt,
    NativeLocationLabelStatus, NativeLocationLabelTarget, NativeProjectionStatus,
    NativeRefreshStatus, NativeRenameDraftToken, NativeRenameDraftView, NativeRenameEdit,
    NativeRenameInputContext, NativeSessionNotice, NativeSessionView, NativeSetupDraftView,
    NativeSetupEdit, NativeSetupInputContext, NativeSetupStage, NativeSetupStatus,
    NativeSetupToken, NativeTabView,
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
    Opened(TerminalProjection, bool),
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
    pub(crate) fn after_draw(&mut self) {
        self.retirement_redraw_pending = false;
        match &mut self.backend {
            SessionBackend::Native(native) => native.after_draw(),
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
        #[cfg(target_os = "linux")]
        if let Some(shutdown) = self.recovery_shutdown.take() {
            shutdown.close();
        }
        #[cfg(target_os = "linux")]
        let mut retired_poll = None;
        let (pending_create, pending_move, close_return, rename_return, setup_return) =
            match self.task.take() {
                Some(SessionTask::Native(task)) => match task.join() {
                    Ok(returned) => {
                        let mut returned = returned;
                        #[cfg(target_os = "linux")]
                        {
                            retired_poll = returned.take_unavailable_poll();
                        }
                        let setup = returned.take_setup_result();
                        let (create, moved, close, renamed) = returned.into_pending_results();
                        (
                            create,
                            moved,
                            state::CloseWorkerReturn::Returned(close),
                            renamed,
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
                        (None, None, evidence, renamed, setup)
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
        #[cfg(target_os = "linux")]
        let clean_return = pending_create.is_none()
            && pending_move.is_none()
            && rename_return.is_none()
            && setup_return.is_none()
            && queued_setup_return.is_none()
            && matches!(close_return, state::CloseWorkerReturn::Returned(None));
        let rename_accepted = native.finalize_rename_worker(rename_return);
        let setup_accepted = native.finalize_setup_worker(setup_return);
        let queued_setup_accepted = native.finalize_setup_worker(queued_setup_return);
        #[cfg(target_os = "linux")]
        native.finish_retired_peer_join(
            retired_poll,
            clean_return && rename_accepted && setup_accepted && queued_setup_accepted,
        );
        #[cfg(not(target_os = "linux"))]
        let _accepted_setup_and_rename = (rename_accepted, setup_accepted, queued_setup_accepted);
        let _accepted = native.finalize_retirement(pending_create, pending_move, close_return);
        native.release_mailbox();
        // Geometry observation can finalize retirement before the shell asks tick for a redraw.
        self.retirement_redraw_pending = true;
    }
}

impl TerminalInputPort for TerminalSession {
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
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        let accepted = match &mut self.backend {
            SessionBackend::Native(native) => native.forward(input),
            #[cfg(test)]
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
        match &mut self.backend {
            SessionBackend::Native(native) => native.close(state::CarrierFailure::Closed),
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
