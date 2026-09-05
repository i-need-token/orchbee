use crate::{
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
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread::{self, JoinHandle},
};

mod connection;

#[derive(Clone, Debug)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the private connected TUI is staged before endpoint activation"
    )
)]
pub(crate) enum TerminalIntent {
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

enum Command {
    Input(ControlInput),
    Resize(Geometry),
    Poll,
    Feedback(TerminalViewerAction),
}
enum Event {
    Opened(TerminalProjection, bool),
    Polled(Option<Vec<u8>>),
    FeedbackAccepted,
    Failed(TerminalStatus),
}

pub(crate) struct TerminalSession {
    commands: Option<SyncSender<Command>>,
    events: Option<Receiver<Event>>,
    task: Option<JoinHandle<()>>,
    shutdown: LocalClientShutdown,
    status: TerminalStatus,
    projection: Option<TerminalProjection>,
    feedback: Option<TerminalViewerAction>,
    poll_pending: bool,
    geometry: Geometry,
}
impl TerminalSession {
    pub(crate) fn start(
        client: LocalClient,
        intent: TerminalIntent,
        geometry: Geometry,
    ) -> io::Result<Self> {
        let shutdown = client.shutdown_handle()?;
        let mut seed = [0_u8; 16];
        File::open("/dev/urandom")?.read_exact(&mut seed)?;
        let identities = Identities(u128::from_be_bytes(seed));
        let (commands, receiver) = mpsc::sync_channel(8);
        let (sender, events) = mpsc::sync_channel(1);
        let task = thread::Builder::new()
            .name("orchbee-terminal-session".into())
            .spawn(move || {
                let result =
                    connection::run(client, intent, geometry, identities, receiver, &sender);
                if let Err(status) = result {
                    let _delivered = sender.send(Event::Failed(status));
                }
            })?;
        Ok(Self {
            commands: Some(commands),
            events: Some(events),
            task: Some(task),
            shutdown,
            status: TerminalStatus::Connecting,
            projection: None,
            feedback: None,
            poll_pending: true,
            geometry,
        })
    }
    pub(crate) fn status(&self) -> TerminalStatus {
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
    pub(crate) fn projection(&self) -> Option<&TerminalProjection> {
        self.projection.as_ref()
    }
    pub(crate) fn tick(&mut self) -> bool {
        let event = self.events.as_ref().map(Receiver::try_recv);
        let changed = match event {
            Some(Ok(Event::Opened(projection, controller))) => {
                self.projection = Some(projection);
                self.poll_pending = false;
                self.status = if controller {
                    TerminalStatus::Controlling
                } else {
                    TerminalStatus::ReadOnly
                };
                true
            }
            Some(Ok(Event::Polled(record))) => {
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
                        Err(()) => self.retire(TerminalStatus::Unavailable),
                    }
                    true
                } else {
                    self.poll_pending = false;
                    false
                }
            }
            Some(Ok(Event::FeedbackAccepted)) => {
                self.poll_pending = false;
                false
            }
            Some(Ok(Event::Failed(status))) => {
                self.retire(status);
                true
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.retire(TerminalStatus::Unavailable);
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
            && self.send(Command::Poll).is_ok()
        {
            self.poll_pending = true;
        }
        changed || self.feedback.is_some()
    }
    // Only the shell's successful draw calls this, so an undrawn frame is never acknowledged.
    pub(crate) fn after_draw(&mut self) {
        if let Some(feedback) = self.feedback.take()
            && self.send(Command::Feedback(feedback.clone())).is_err()
        {
            self.feedback = Some(feedback);
        }
    }
    pub(crate) fn resize(&mut self, geometry: Geometry) {
        if geometry != self.geometry
            && self.status == TerminalStatus::Controlling
            && self.send(Command::Resize(geometry)).is_ok()
        {
            self.geometry = geometry;
        }
    }
    fn send(&self, command: Command) -> Result<(), TerminalInputUnavailable> {
        self.commands
            .as_ref()
            .ok_or(TerminalInputUnavailable)?
            .try_send(command)
            .map_err(|_| TerminalInputUnavailable)
    }
    fn retire(&mut self, status: TerminalStatus) {
        self.status = status;
        self.projection = None;
        self.feedback = None;
        self.commands.take();
        self.events.take();
        let _closed = self.shutdown.close();
    }
}
impl TerminalInputPort for TerminalSession {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        if self.status != TerminalStatus::Controlling {
            return Err(TerminalInputUnavailable);
        }
        let input = ControlInput::new(input.to_vec()).map_err(|_| TerminalInputUnavailable)?;
        self.send(Command::Input(input))
    }
}
impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.retire(self.status);
        if let Some(task) = self.task.take() {
            let _joined = task.join();
        }
    }
}
struct Identities(u128);
impl Identities {
    fn next(&mut self) -> Result<RequestId, TerminalStatus> {
        self.0 = self.0.checked_add(1).ok_or(TerminalStatus::Unavailable)?;
        RequestId::parse(&format!("{:032x}", self.0)).map_err(|_| TerminalStatus::Unavailable)
    }
}
#[cfg(test)]
mod tests;
