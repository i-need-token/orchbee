//! UI-owned title state consumes the shared connection subscription without a bridge thread.
use crate::local_client::{LocalClientOrigin, LocalOperationClock, multiplex::StreamEvent};
use orchbee_workbench_protocol::{
    ProtocolVersion, SessionId, WindowTitleAction, WindowTitleContext, WindowTitleOutcome,
    WindowTitleRequest, WindowTitleSnapshot, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, read_workbench_response,
};
use std::{
    io::{self, Cursor},
    sync::Mutex,
};

struct Active {
    focused: bool,
    context: WindowTitleContext,
    request_id: WorkbenchRequestId,
    stream: crate::local_client::multiplex::Subscription,
}
#[derive(Default)]
struct Handoff {
    serial: u64,
    focused: bool,
    context: WindowTitleContext,
    snapshot: Option<WindowTitleSnapshot>,
    last: Option<WindowTitleSnapshot>,
    active: Option<Active>,
}
impl Handoff {
    fn receive(&mut self, snapshot: Option<WindowTitleSnapshot>) {
        if snapshot.is_none() || self.last != snapshot {
            self.snapshot = snapshot.clone();
            self.last = snapshot;
        }
    }
}
pub(super) struct Subscription {
    source: Option<(LocalClientOrigin, SessionId)>,
    state: Mutex<Handoff>,
}
impl Subscription {
    pub(super) fn spawn(
        origin: LocalClientOrigin,
        _clock: LocalOperationClock,
        session: SessionId,
    ) -> io::Result<Self> {
        let listener = Self {
            source: Some((origin, session)),
            state: Mutex::default(),
        };
        listener.update(false, WindowTitleContext::default())?;
        Ok(listener)
    }
    fn update(&self, focused: bool, context: WindowTitleContext) -> io::Result<()> {
        let mut state = self.state.lock().map_err(|_| io::ErrorKind::BrokenPipe)?;
        if focused && !state.focused {
            state.last = None;
        }
        state.focused = focused;
        state.context = context.clone();
        let Some((origin, session)) = &self.source else {
            return Ok(());
        };
        if state
            .active
            .as_ref()
            .is_none_or(|old| old.focused != focused || old.context != context)
        {
            state.serial = state
                .serial
                .checked_add(1)
                .ok_or(io::ErrorKind::OutOfMemory)?;
            let request_id = WorkbenchRequestId::parse(&format!("{:032x}", state.serial))
                .map_err(io::Error::other)?;
            let request = WorkbenchRequest::WindowTitle(WindowTitleRequest {
                protocol_version: ProtocolVersion,
                request_id: request_id.clone(),
                session_id: session.clone(),
                action: WindowTitleAction::Listen {
                    focused,
                    context: context.clone(),
                },
            });
            // Open precedes old cancellation, preserving the connection-owned foreground order.
            let stream =
                origin.subscribe(&request, orchbee_workbench_protocol::StreamMode::State)?;
            state.active = Some(Active {
                focused,
                context,
                request_id,
                stream,
            });
        }
        while let Some(active) = &state.active {
            let event = active.stream.poll()?;
            match event {
                Some(StreamEvent::Record(bytes)) => {
                    let response = read_workbench_response(&mut Cursor::new(bytes))
                        .map_err(io::Error::other)?;
                    let WorkbenchResponse::WindowTitle(reply) = response else {
                        return Err(io::ErrorKind::InvalidData.into());
                    };
                    if reply.request_id != active.request_id || &reply.session_id != session {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    match reply.outcome {
                        WindowTitleOutcome::Listening { snapshot } => state.receive(snapshot),
                        WindowTitleOutcome::Published { .. }
                        | WindowTitleOutcome::Failed { .. } => {
                            return Err(io::ErrorKind::InvalidData.into());
                        }
                    }
                }
                Some(StreamEvent::Gap | StreamEvent::Disconnected) => {
                    state.last = None;
                    state.snapshot = None;
                }
                None => break,
            }
        }
        Ok(())
    }
    pub(super) fn poll(
        &self,
        focused: bool,
        context: WindowTitleContext,
    ) -> Option<WindowTitleSnapshot> {
        if self.update(focused, context).is_err() {
            return None;
        }
        self.state.lock().ok()?.snapshot.take()
    }
    pub(super) fn close(&mut self) {
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
