//! Routes a Worker's "this viewer holds something to Poll" hint to the Client connection that owns
//! the viewer. A hint moves no record and proves nothing: Poll stays the only way to take one, and
//! a hint without a route is dropped, which costs that viewer only its Client's fallback poll.

use orchbee_capability_node_protocol::OperationId;
use orchbee_server::SessionChangeSignal;
use orchbee_workbench_protocol::MultiplexViewerId;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock, PoisonError},
};

/// Every routed viewer of one Session, across its Worker generations.
#[derive(Debug, Default)]
pub(crate) struct ViewerReadyRoutes(Mutex<BTreeMap<[u8; 16], Arc<ConnectionViewerReady>>>);

impl ViewerReadyRoutes {
    /// Lives until the returned route drops, so a connection can route a viewer before asking the
    /// Worker to attach it. The Worker can report the first frame ahead of its own reply.
    pub(crate) fn route(
        self: &Arc<Self>,
        attachment: OperationId,
        connection: Arc<ConnectionViewerReady>,
    ) -> ViewerReadyRoute {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(attachment.as_bytes(), connection);
        ViewerReadyRoute {
            routes: Arc::clone(self),
            attachment: attachment.as_bytes(),
        }
    }

    pub(crate) fn deliver(&self, attachment: OperationId) {
        let connection = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&attachment.as_bytes())
            .cloned();
        let viewer = MultiplexViewerId::new(u128::from_be_bytes(attachment.as_bytes()));
        if let (Some(connection), Some(viewer)) = (connection, viewer) {
            connection.deliver(viewer);
        }
    }
}

#[derive(Debug)]
pub(crate) struct ViewerReadyRoute {
    routes: Arc<ViewerReadyRoutes>,
    attachment: [u8; 16],
}

impl Drop for ViewerReadyRoute {
    fn drop(&mut self) {
        self.routes
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.attachment);
    }
}

/// One Client connection's unsent hints. A viewer appears once however often it is hinted, so the
/// set is bounded by the connection's viewers.
#[derive(Debug, Default)]
pub(crate) struct ConnectionViewerReady {
    ready: Mutex<Vec<MultiplexViewerId>>,
    signal: OnceLock<Arc<SessionChangeSignal>>,
}

impl ConnectionViewerReady {
    /// Names the connection loop to wake. A hint delivered earlier is sent on its first pass.
    pub(crate) fn bind(&self, signal: &Arc<SessionChangeSignal>) {
        let _bound = self.signal.set(Arc::clone(signal));
    }

    fn deliver(&self, viewer: MultiplexViewerId) {
        {
            let mut ready = self.ready.lock().unwrap_or_else(PoisonError::into_inner);
            if ready.contains(&viewer) {
                return;
            }
            ready.push(viewer);
        }
        if let Some(signal) = self.signal.get() {
            signal.wake();
        }
    }

    pub(crate) fn take(&self) -> Vec<MultiplexViewerId> {
        std::mem::take(&mut *self.ready.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

#[cfg(test)]
mod tests;
