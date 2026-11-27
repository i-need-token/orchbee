//! The open subscriptions of one subscribing operation, each an Events call in a Workspace. A
//! subscription joins with its Snapshot and leaves when its Client cancels it, its lane or
//! connection closes, or the process drains; the call is cancelled first, so leaving closes it as
//! cancelled. Snapshot and Updates are queued under the registry's lock, so every Update follows
//! its subscription's Snapshot, and a producer that checks its fact is current under the same lock
//! cannot deliver a stale one after a newer.

use std::sync::{Mutex, MutexGuard, PoisonError};

use orchbee_files_protocol::{CallId, ConnectionId, FilesNotice, LaneId, WorkspaceId};

use crate::calls::{Call, Events};

/// The open subscriptions of one operation. The lock is taken before the scopes' and the outbound
/// queue's, never after; a panic ends the process, so a poisoned lock is never observed.
pub(crate) struct Subscriptions {
    open: Mutex<Vec<Subscription>>,
}

struct Subscription {
    workspace: WorkspaceId,
    events: Events,
}

/// The subscriptions that leave together.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Leaving {
    /// The one of this call.
    Call(CallId),
    /// Every one on this lane of this connection.
    Lane(ConnectionId, LaneId),
    /// Every one of this connection.
    Connection(ConnectionId),
    /// Every one.
    All,
}

impl Subscriptions {
    pub(crate) fn new() -> Self {
        Self {
            open: Mutex::new(Vec::new()),
        }
    }

    /// Opens `call` as a subscription in `workspace` with `snapshot`, when `current` confirms the
    /// snapshot still states the present. A call cancelled meanwhile ends here instead of joining.
    ///
    /// # Errors
    ///
    /// The call back, unanswered, when `current` finds the snapshot stale; its producer reads the
    /// facts again.
    pub(crate) fn join(
        &self,
        workspace: WorkspaceId,
        call: Call,
        snapshot: &FilesNotice,
        current: impl FnOnce() -> bool,
    ) -> Result<(), Call> {
        let mut open = self.lock();
        if call.cancelled() {
            drop(open);
            drop(call);
            return Ok(());
        }
        if !current() {
            return Err(call);
        }
        let events = call.snapshot(snapshot);
        open.push(Subscription { workspace, events });
        Ok(())
    }

    /// Queues `notice` as an Update of every subscription in `workspace`, when `current` confirms
    /// it still states the present.
    pub(crate) fn deliver(
        &self,
        workspace: WorkspaceId,
        notice: &FilesNotice,
        current: impl FnOnce() -> bool,
    ) {
        let open = self.lock();
        if !current() {
            return;
        }
        for subscription in open.iter().filter(|open| open.workspace == workspace) {
            subscription.events.update(notice);
        }
    }

    /// Queues `notice` as an Update of each subscription in `workspace` that `connection` made.
    pub(crate) fn deliver_to(
        &self,
        workspace: WorkspaceId,
        connection: ConnectionId,
        notice: &FilesNotice,
    ) {
        let open = self.lock();
        for subscription in open
            .iter()
            .filter(|open| open.workspace == workspace && open.events.connection() == connection)
        {
            subscription.events.update(notice);
        }
    }

    /// Whether any subscription is open in `workspace`.
    pub(crate) fn any_in(&self, workspace: WorkspaceId) -> bool {
        self.lock().iter().any(|open| open.workspace == workspace)
    }

    /// The connections with a subscription open in `workspace`.
    pub(crate) fn connections_in(&self, workspace: WorkspaceId) -> Vec<ConnectionId> {
        let mut connections: Vec<ConnectionId> = self
            .lock()
            .iter()
            .filter(|open| open.workspace == workspace)
            .map(|open| open.events.connection())
            .collect();
        connections.sort_unstable();
        connections.dedup();
        connections
    }

    /// Ends the `leaving` subscriptions, whose calls are already cancelled.
    pub(crate) fn leave(&self, leaving: Leaving) {
        let left: Vec<Subscription> = {
            let mut open = self.lock();
            let (left, kept) = std::mem::take(&mut *open)
                .into_iter()
                .partition(|subscription| leaves(&subscription.events, leaving));
            *open = kept;
            left
        };
        // Each close takes the calls' lock, which is never taken under this one.
        drop(left);
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Subscription>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn leaves(events: &Events, leaving: Leaving) -> bool {
    match leaving {
        Leaving::Call(id) => events.id() == id,
        Leaving::Lane(connection, lane) => {
            events.connection() == connection && events.lane() == lane
        }
        Leaving::Connection(connection) => events.connection() == connection,
        Leaving::All => true,
    }
}
