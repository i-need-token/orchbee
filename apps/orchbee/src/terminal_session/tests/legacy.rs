//! Retained single-terminal adapter for session and real process/PTY witnesses.

mod connection;

use super::super::{SessionBackend, SessionTask, TerminalSession, TerminalStatus};
use crate::{
    local_client::{LocalClient, LocalClientShutdown},
    terminal_input::TerminalInputUnavailable,
    terminal_projection::TerminalProjection,
};
use orchbee_client_server_protocol::{
    ControlInput, Geometry, RequestId, TerminalViewerAction, WorkingDirectory,
};
use std::{
    io,
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError},
    thread,
};

#[derive(Clone, Debug)]
pub(in crate::terminal_session) enum LegacyIntent {
    Create { cwd: Option<WorkingDirectory> },
    Attach(RequestId),
}

pub(in crate::terminal_session) enum LegacyCommand {
    Input(ControlInput),
    Resize(Geometry),
    Poll,
    Feedback(TerminalViewerAction),
}

pub(in crate::terminal_session) enum LegacyEvent {
    Opened(Box<TerminalProjection>, bool),
    Polled(Option<Vec<u8>>),
    FeedbackAccepted,
    Failed(TerminalStatus),
}

pub(in crate::terminal_session) struct LegacySession {
    pub(in crate::terminal_session) commands: Option<SyncSender<LegacyCommand>>,
    pub(in crate::terminal_session) events: Option<Receiver<LegacyEvent>>,
    pub(in crate::terminal_session) status: TerminalStatus,
    pub(in crate::terminal_session) projection: Option<TerminalProjection>,
    pub(in crate::terminal_session) feedback: Option<TerminalViewerAction>,
    pub(in crate::terminal_session) poll_pending: bool,
    pub(in crate::terminal_session) geometry: Geometry,
}

impl TerminalSession {
    pub(in crate::terminal_session) fn start_legacy(
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
            position_writer: None,
            #[cfg(windows)]
            position_startup: None,
            #[cfg(windows)]
            client_control: None,
            #[cfg(windows)]
            configuration: None,
            #[cfg(windows)]
            notifications: None,
            #[cfg(windows)]
            window_title: None,
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
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            origin: None,
            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            recovery_shutdown: None,
        })
    }
}

impl LegacySession {
    pub(in crate::terminal_session) fn status(&self) -> TerminalStatus {
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

    pub(in crate::terminal_session) fn tick(&mut self, shutdown: &LocalClientShutdown) -> bool {
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

    pub(in crate::terminal_session) fn after_draw(&mut self) {
        if let Some(feedback) = self.feedback.take()
            && self
                .send(LegacyCommand::Feedback(feedback.clone()))
                .is_err()
        {
            self.feedback = Some(feedback);
        }
    }

    pub(in crate::terminal_session) fn resize(&mut self, geometry: Geometry) {
        if geometry != self.geometry
            && self.status == TerminalStatus::Controlling
            && self.send(LegacyCommand::Resize(geometry)).is_ok()
        {
            self.geometry = geometry;
        }
    }

    pub(in crate::terminal_session) fn forward(
        &self,
        input: &[u8],
    ) -> Result<(), TerminalInputUnavailable> {
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
    pub(in crate::terminal_session) fn pointer_probe(&self) -> crate::terminal_input::PointerProbe {
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
        probe.selection_ready = matches!(
            self.status,
            TerminalStatus::Controlling | TerminalStatus::ReadOnly
        );
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

    pub(in crate::terminal_session) fn retire(
        &mut self,
        status: TerminalStatus,
        shutdown: &LocalClientShutdown,
    ) {
        self.status = status;
        self.projection = None;
        self.feedback = None;
        self.commands.take();
        self.events.take();
        let _closed = shutdown.close();
    }
}

struct Identities(u128);

impl Identities {
    fn next(&mut self) -> Result<RequestId, TerminalStatus> {
        self.0 = self.0.checked_add(1).ok_or(TerminalStatus::Unavailable)?;
        RequestId::parse(&format!("{:032x}", self.0)).map_err(|_| TerminalStatus::Unavailable)
    }
}
