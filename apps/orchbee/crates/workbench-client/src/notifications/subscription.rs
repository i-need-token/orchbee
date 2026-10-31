//! UI-owned notification delivery consumes the shared event stream without a bridge thread.
use crate::local_client::{LocalClientOrigin, LocalOperationClock, multiplex::StreamEvent};
use orchbee_workbench_protocol::Notification;
use orchbee_workbench_protocol::{
    NotificationAction, NotificationOutcome, NotificationRequest, ProtocolVersion, SessionId,
    WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse, read_workbench_response,
};
use std::{
    io::{self, Cursor},
    sync::Mutex,
};
struct Active {
    accepting: bool,
    request_id: WorkbenchRequestId,
    stream: crate::local_client::multiplex::Subscription,
}
#[derive(Default)]
struct Handoff {
    serial: u64,
    message: Option<Notification>,
    active: Option<Active>,
}
pub(crate) struct Subscription {
    source: Option<(LocalClientOrigin, SessionId)>,
    state: Mutex<Handoff>,
}
impl Subscription {
    pub(crate) fn spawn(
        origin: LocalClientOrigin,
        _clock: LocalOperationClock,
        session: SessionId,
    ) -> io::Result<Self> {
        let listener = Self {
            source: Some((origin, session)),
            state: Mutex::default(),
        };
        listener.update(false)?;
        Ok(listener)
    }
    fn update(&self, accepting: bool) -> io::Result<()> {
        let mut state = self.state.lock().map_err(|_| io::ErrorKind::BrokenPipe)?;
        let Some((origin, session)) = &self.source else {
            return Ok(());
        };
        if state
            .active
            .as_ref()
            .is_none_or(|active| active.accepting != accepting)
        {
            state.serial = state
                .serial
                .checked_add(1)
                .ok_or(io::ErrorKind::OutOfMemory)?;
            let request_id = WorkbenchRequestId::parse(&format!("{:032x}", state.serial))
                .map_err(io::Error::other)?;
            let request = WorkbenchRequest::Notification(NotificationRequest {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
                session_id: session.clone(),
                action: NotificationAction::Listen { accepting },
            });
            let stream =
                origin.subscribe(&request, orchbee_workbench_protocol::StreamMode::Events)?;
            state.active = Some(Active {
                accepting,
                request_id,
                stream,
            });
        }
        while let Some(active) = &state.active {
            match active.stream.poll()? {
                Some(StreamEvent::Record(bytes)) => {
                    let WorkbenchResponse::Notification(reply) =
                        read_workbench_response(&mut Cursor::new(bytes))
                            .map_err(io::Error::other)?
                    else {
                        return Err(io::ErrorKind::InvalidData.into());
                    };
                    if reply.request_id != active.request_id || &reply.session_id != session {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    let NotificationOutcome::Listening { notification } = reply.outcome else {
                        return Err(io::ErrorKind::InvalidData.into());
                    };
                    if notification.is_some() && accepting {
                        state.message = notification;
                        break;
                    }
                }
                Some(StreamEvent::Gap | StreamEvent::Disconnected) => state.message = None,
                None => break,
            }
        }
        Ok(())
    }
    pub(crate) fn poll(&self, accepting: bool) -> Option<Notification> {
        if self.update(accepting).is_err() {
            return None;
        }
        let message = self.state.lock().ok()?.message.take();
        if accepting { message } else { None }
    }
    pub(crate) fn close(&mut self) {
        self.state
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .active = None;
        self.source = None;
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        self.close();
    }
}
#[cfg(test)]
mod tests;
